use super::synth::synth_to_string;
use super::types::{
    BuiltInField, CellValue, ClassPattern, ComparisonOp, FieldRef, Query, QueryError, QueryResult,
    QueryStatement, SelectClause, TraversalFunction, Value, WhereClause,
    MAX_MULTI_CLASS_FROM_LIST_SIZE,
};
use crate::{
    analysis::string_analysis::extract_string_value,
    graph::{gc_root_path::shortest_gc_root_path, DominatorTree, VIRTUAL_ROOT_ID},
    hprof::{field_types, read_field, FieldValue, ObjectGraph, ObjectId},
};
use regex::Regex;
use std::collections::HashSet;

/// Bounded-walk-with-cycle-guard depth for `dominators(...)` chain
/// resolution. Mirrors the pattern established by M13's
/// `resolve_loader_chain` and M15 15.B's `resolve_superclass_key` walk.
/// Dominator chains cannot actually cycle by construction (the dominator
/// tree is a tree), but the bound and visited-set guard cost nothing and
/// match this codebase's established discipline for chain walks.
const DOMINATOR_CHAIN_MAX_DEPTH: usize = 64;

/// Executor-side mirror of `parser::MAX_SUBQUERY_NESTING_DEPTH` (M15 Slice
/// 15.E): defense-in-depth against a `Query` assembled directly (bypassing
/// the parser, e.g. via the MCP surface) with more than one level of
/// `ClassPattern::Subquery` nesting -- returns a structured `QueryError`
/// rather than recursing without bound, same discipline as 15.D's
/// independent regex-compile defense-in-depth check.
const MAX_SUBQUERY_NESTING_DEPTH: usize = 1;
const SUBQUERY_NESTING_DEPTH_EXCEEDED_MESSAGE: &str =
    "subquery nesting depth exceeded: OQL subqueries support only one level of nesting";
const DOMINATORS_OVERVIEW_HINT: &str =
    "re-run with --mode deep; dominators(...) requires a deep-mode dominator tree.";

const GC_ROOT_PATH_MAX_DEPTH: usize = 32;
const GC_ROOT_PATH_OVERVIEW_HINT: &str =
    "re-run with --mode deep; @gcRootPath requires deep-mode object graph traversal.";
const OBJECTS_OVERVIEW_HINT: &str =
    "re-run with --mode deep; OBJECTS requires deep-mode object field traversal.";
const NULL_PREDICATE_OVERVIEW_HINT: &str =
    "re-run with --mode deep; IS NULL and IS NOT NULL require deep-mode instance field data.";
const RETAINED_SIZE_OVERVIEW_HINT: &str =
    "re-run with --mode deep or use @shallowSize for a per-object approximation.";
const STRING_PREDICATE_OVERVIEW_HINT: &str =
    "re-run with --mode deep; LIKE, CONTAINS, and =~ on instance fields require deep-mode field data.";
const TO_STRING_OVERVIEW_HINT: &str =
    "re-run with --mode deep; @toString requires deep-mode field data.";

pub fn execute_query(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> Result<QueryResult, QueryError> {
    validate_supported_query(query, dominator)?;

    // Compile any `=~` regex patterns in the WHERE clause exactly once, here,
    // before either candidate loop below -- not once per candidate object.
    // The parser already validated pattern syntax eagerly (fail-fast, before
    // any graph work), but a `Query` can also be built directly by a caller
    // that bypassed the string parser (e.g. the MCP surface), so this is
    // also the defense-in-depth compile point (M15 Slice 15.D).
    let regexes: Vec<Option<Regex>> = match &query.filter {
        Some(filter) => compile_regex_conditions(filter)?,
        None => Vec::new(),
    };

    let columns = projected_columns(&query.select);
    let mut matched_ids = resolve_matched_ids(query, graph, dominator, &regexes)?;
    matched_ids.sort_unstable();

    finalize_query_result(
        &query.select,
        query.limit,
        graph,
        dominator,
        matched_ids,
        columns,
    )
}

/// Evaluates a top-level OQL statement -- a single `Query`, or two `Query`s
/// joined by `UNION` (M15 Slice 15.E). Pairs with `parser::parse_query_statement`;
/// `execute_query` above is unchanged and keeps handling a bare `Query`
/// exactly as before for every caller that doesn't need `UNION`.
pub fn execute_query_statement(
    statement: &QueryStatement,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> Result<QueryResult, QueryError> {
    match statement {
        QueryStatement::Single(query) => execute_query(query, graph, dominator),
        QueryStatement::Union(left, right) => execute_union_query(left, right, graph, dominator),
    }
}

/// Runs `left` and `right` independently through the ordinary
/// candidate-resolution pipeline (`resolve_matched_ids` -- the same routine
/// every `FromClause` source, including a one-level subquery, already
/// funnels through), each bounded by its own `LIMIT` if it has one, unions
/// the two resulting object-id sets with duplicates removed by object id
/// (M15 Slice 15.E §4.1 item 5), and projects the combined id set through
/// `left`'s own `SELECT`.
///
/// `UNION` cannot simply concatenate two already-projected `QueryResult`s:
/// deduplication is defined "by object id" (§4.1 item 5), but a projected
/// row does not always carry an identifiable object id (e.g.
/// `SELECT @className FROM ...`) -- so the dedup has to happen at the
/// id-set level, before either side's `SELECT` runs, exactly mirroring how
/// `ClassPattern::Subquery`'s inner query contributes a bare id set rather
/// than a projected result.
///
/// Each side's own `LIMIT` (the grammar lets either `<query1>` or
/// `<query2>` carry one, since each is parsed as a complete, independent
/// query body) bounds *that side's own contribution* before the merge --
/// i.e. `<query> LIMIT n` on one side of `UNION` behaves exactly as it
/// would if that side were run standalone, not as a limit on the final
/// merged/deduplicated total. There is no separate statement-level `LIMIT`
/// concept for the combined result, so `left`'s `SELECT` wins for the
/// combined projection (matching SQL `UNION`'s expectation that both sides
/// share a column shape -- this implementation does not require it, but
/// the shape mismatch is the caller's to avoid) and no further limit is
/// applied to the already-bounded, deduplicated merge.
fn execute_union_query(
    left: &Query,
    right: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> Result<QueryResult, QueryError> {
    // `resolve_ids_bounded_by_own_limit` validates each side independently
    // (overview-mode feature gates included) before resolving it, so no
    // separate `validate_supported_query` call is needed here.
    let left_ids = resolve_ids_bounded_by_own_limit(left, graph, dominator, 0)?;
    let right_ids = resolve_ids_bounded_by_own_limit(right, graph, dominator, 0)?;

    let mut seen: HashSet<ObjectId> = HashSet::with_capacity(left_ids.len() + right_ids.len());
    let mut merged = Vec::with_capacity(left_ids.len() + right_ids.len());
    for object_id in left_ids.into_iter().chain(right_ids) {
        if seen.insert(object_id) {
            merged.push(object_id);
        }
    }
    merged.sort_unstable();

    let columns = projected_columns(&left.select);
    finalize_query_result(&left.select, None, graph, dominator, merged, columns)
}

/// Resolves `query`'s `FromClause` source to its matched-and-filtered
/// object-id set: a plain class-name/glob scan, a traversal function
/// (`outbounds`/`inbounds`/`dominators`), or a one-level subquery -- every
/// variant converges on the same "candidate set, then apply this query's
/// own WHERE" shape. Used directly by `execute_query` (a top-level query is
/// never itself nested inside a subquery, hence depth 0) and indirectly, via
/// `resolve_ids_bounded_by_own_limit`, by `execute_union_query` and
/// `resolve_subquery_candidates`.
fn resolve_matched_ids(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    regexes: &[Option<Regex>],
) -> Result<Vec<ObjectId>, QueryError> {
    resolve_matched_ids_at_depth(query, graph, dominator, regexes, 0)
}

fn resolve_matched_ids_at_depth(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    regexes: &[Option<Regex>],
    depth: usize,
) -> Result<Vec<ObjectId>, QueryError> {
    match &query.from.class_pattern {
        ClassPattern::Traversal(traversal) => {
            // Traversal FROM sources (outbounds/inbounds/dominators) already
            // resolve to an explicit object-id set -- no full-graph class-name
            // scan needed, just filter that set through the ordinary WHERE
            // pipeline exactly like a class-pattern FROM source does.
            let candidates = resolve_traversal_candidates(*traversal, graph, dominator)?;
            let mut ids = Vec::with_capacity(candidates.len());
            for object_id in candidates {
                if graph.get_object(object_id).is_none() {
                    // Dangling reference in the source object's edge list;
                    // silently skip, consistent with OBJECTS projection's
                    // handling of unresolved targets elsewhere in this module.
                    continue;
                }
                if matches_filter(query, graph, dominator, object_id, regexes)? {
                    ids.push(object_id);
                }
            }
            Ok(ids)
        }
        ClassPattern::Subquery(inner) => {
            if depth >= MAX_SUBQUERY_NESTING_DEPTH {
                return Err(QueryError::Unsupported(
                    SUBQUERY_NESTING_DEPTH_EXCEEDED_MESSAGE.into(),
                ));
            }
            let candidates = resolve_subquery_candidates(inner, graph, dominator, depth + 1)?;
            let mut ids = Vec::with_capacity(candidates.len());
            for object_id in candidates {
                if graph.get_object(object_id).is_none() {
                    continue;
                }
                if matches_filter(query, graph, dominator, object_id, regexes)? {
                    ids.push(object_id);
                }
            }
            Ok(ids)
        }
        ClassPattern::Exact(_) | ClassPattern::Glob(_) => {
            resolve_class_pattern_candidates(
                graph,
                dominator,
                query,
                std::slice::from_ref(&query.from.class_pattern),
                regexes,
            )
        }
        ClassPattern::Multi(patterns) => {
            if patterns.len() > MAX_MULTI_CLASS_FROM_LIST_SIZE {
                return Err(QueryError::Unsupported(format!(
                    "multi-class FROM list exceeds limit of {MAX_MULTI_CLASS_FROM_LIST_SIZE} class patterns"
                )));
            }
            resolve_class_pattern_candidates(graph, dominator, query, patterns, regexes)
        }
    }
}

/// Evaluates a subquery's own FROM+WHERE+LIMIT pipeline -- never its
/// `SELECT` clause, which is irrelevant here: like `outbounds`/`inbounds`/
/// `dominators`, a subquery FROM source contributes a bare object-id set,
/// not a projected result. The outer query performs its own independent
/// `SELECT` projection over whichever candidates survive both the inner
/// and outer `WHERE` clauses (M15 Slice 15.E §4.1 item 4). `depth` is the
/// nesting depth `inner` itself is evaluated at; see
/// `MAX_SUBQUERY_NESTING_DEPTH` for the bound this enforces.
fn resolve_subquery_candidates(
    inner: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    depth: usize,
) -> Result<Vec<ObjectId>, QueryError> {
    resolve_ids_bounded_by_own_limit(inner, graph, dominator, depth)
}

/// Resolves `query`'s own FROM+WHERE candidate set (via
/// `resolve_matched_ids_at_depth`, starting the subquery-nesting count at
/// `depth`) and bounds it by `query`'s own `LIMIT`, if it has one. Shared by
/// two call sites that both need "the id set this query would contribute,
/// standalone" rather than a projected `QueryResult`: a `ClassPattern::Subquery`
/// FROM source (`resolve_subquery_candidates`, `depth` threading the parser's
/// one-level nesting bound through) and each independent side of a `UNION`
/// (`execute_union_query`, always at `depth` 0 -- a `UNION` side is never
/// itself inside a subquery's nesting count).
fn resolve_ids_bounded_by_own_limit(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    depth: usize,
) -> Result<Vec<ObjectId>, QueryError> {
    validate_supported_query(query, dominator)?;

    let regexes: Vec<Option<Regex>> = match &query.filter {
        Some(filter) => compile_regex_conditions(filter)?,
        None => Vec::new(),
    };

    let mut ids = resolve_matched_ids_at_depth(query, graph, dominator, &regexes, depth)?;
    ids.sort_unstable();
    if let Some(limit) = query.limit {
        ids.truncate(limit);
    }
    Ok(ids)
}

/// Shared tail of `execute_query`/`execute_union_query`: projects an
/// already-resolved, already-sorted `matched_ids` candidate set through the
/// given `select`/`limit`. Takes `select`/`limit` directly rather than a
/// whole `Query` so `execute_union_query` can pass `left`'s `SELECT` with
/// `limit: None` (each side's own `LIMIT` was already applied to its own
/// contribution before the merge -- see `execute_union_query`'s doc
/// comment -- so the combined, deduplicated id set is not truncated
/// again here). Still reuses the identical `OBJECTS` vs.
/// ordinary-projection branching and `total_matched`/`truncated`
/// bookkeeping `execute_query` always used.
fn finalize_query_result(
    select: &SelectClause,
    limit: Option<usize>,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    mut matched_ids: Vec<ObjectId>,
    columns: Vec<String>,
) -> Result<QueryResult, QueryError> {
    if let SelectClause::Objects(field) = select {
        return execute_objects_projection(limit, graph, field, matched_ids, columns);
    }

    let total_before_limit = matched_ids.len();
    if let Some(limit) = limit {
        matched_ids.truncate(limit);
    }

    let mut rows = Vec::with_capacity(matched_ids.len());
    for object_id in matched_ids {
        rows.push(project_row(select, graph, dominator, object_id));
    }

    Ok(QueryResult {
        columns,
        rows,
        total_matched: total_before_limit.min(limit.unwrap_or(total_before_limit)),
        truncated: limit.is_some_and(|limit| total_before_limit > limit),
    })
}

fn projected_columns(select: &SelectClause) -> Vec<String> {
    match select {
        SelectClause::All => vec!["@objectId".into(), "@className".into()],
        SelectClause::Fields(fields) => fields.iter().map(field_label).collect(),
        SelectClause::Objects(_) => vec!["@objectId".into(), "@className".into()],
    }
}

fn field_label(field: &FieldRef) -> String {
    match field {
        FieldRef::BuiltIn(BuiltInField::ObjectId) => "@objectId".into(),
        FieldRef::BuiltIn(BuiltInField::ClassName) => "@className".into(),
        FieldRef::BuiltIn(BuiltInField::ShallowSize) => "@shallowSize".into(),
        FieldRef::BuiltIn(BuiltInField::RetainedSize) => "@retainedSize".into(),
        FieldRef::BuiltIn(BuiltInField::ObjectAddress) => "@objectAddress".into(),
        FieldRef::BuiltIn(BuiltInField::ToString) => "@toString".into(),
        FieldRef::BuiltIn(BuiltInField::GcRootPath) => "@gcRootPath".into(),
        FieldRef::InstanceField(name) => name.clone(),
    }
}

fn validate_supported_query(
    query: &Query,
    dominator: Option<&DominatorTree>,
) -> Result<(), QueryError> {
    if dominator.is_none() {
        if query_references_gc_root_path(query) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "@gcRootPath",
                GC_ROOT_PATH_OVERVIEW_HINT,
            ));
        }

        if matches!(query.select, SelectClause::Objects(_)) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "OBJECTS",
                OBJECTS_OVERVIEW_HINT,
            ));
        }

        if query_uses_null_predicates(query) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "IS NULL / IS NOT NULL",
                NULL_PREDICATE_OVERVIEW_HINT,
            ));
        }

        if query_uses_retained_size(query) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "@retainedSize",
                RETAINED_SIZE_OVERVIEW_HINT,
            ));
        }

        if query_uses_to_string(query) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "@toString",
                TO_STRING_OVERVIEW_HINT,
            ));
        }

        if let Some(feature) = query_uses_instance_string_predicates(query) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                feature,
                STRING_PREDICATE_OVERVIEW_HINT,
            ));
        }

        if matches!(
            query.from.class_pattern,
            ClassPattern::Traversal(TraversalFunction::Dominators(_))
        ) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "dominators(...)",
                DOMINATORS_OVERVIEW_HINT,
            ));
        }
    }

    Ok(())
}

fn query_references_gc_root_path(query: &Query) -> bool {
    select_references_built_in(&query.select, BuiltInField::GcRootPath)
        || query.filter.as_ref().is_some_and(|filter| {
            filter
                .conditions
                .iter()
                .any(|condition| condition.field == FieldRef::BuiltIn(BuiltInField::GcRootPath))
        })
}

fn query_uses_null_predicates(query: &Query) -> bool {
    query.filter.as_ref().is_some_and(|filter| {
        filter
            .conditions
            .iter()
            .any(|condition| matches!(condition.op, ComparisonOp::IsNull | ComparisonOp::IsNotNull))
    })
}

fn query_uses_to_string(query: &Query) -> bool {
    select_references_built_in(&query.select, BuiltInField::ToString)
        || query.filter.as_ref().is_some_and(|filter| {
            filter
                .conditions
                .iter()
                .any(|condition| condition.field == FieldRef::BuiltIn(BuiltInField::ToString))
        })
}

fn query_uses_instance_string_predicates(query: &Query) -> Option<String> {
    query.filter.as_ref().and_then(|filter| {
        filter.conditions.iter().find_map(|condition| {
            if !matches!(
                condition.op,
                ComparisonOp::Like | ComparisonOp::Contains | ComparisonOp::RegexMatch
            ) {
                return None;
            }

            match &condition.field {
                FieldRef::InstanceField(name) => Some(name.clone()),
                _ => None,
            }
        })
    })
}

fn query_uses_retained_size(query: &Query) -> bool {
    select_references_built_in(&query.select, BuiltInField::RetainedSize)
        || query.filter.as_ref().is_some_and(|filter| {
            filter
                .conditions
                .iter()
                .any(|condition| condition.field == FieldRef::BuiltIn(BuiltInField::RetainedSize))
        })
}

fn select_references_built_in(select: &SelectClause, built_in: BuiltInField) -> bool {
    match select {
        SelectClause::All => false,
        SelectClause::Fields(fields) => fields.contains(&FieldRef::BuiltIn(built_in)),
        SelectClause::Objects(field) => *field == FieldRef::BuiltIn(built_in),
    }
}

fn execute_objects_projection(
    limit: Option<usize>,
    graph: &ObjectGraph,
    field: &FieldRef,
    matched_ids: Vec<ObjectId>,
    columns: Vec<String>,
) -> Result<QueryResult, QueryError> {
    let mut rows = Vec::with_capacity(matched_ids.len());

    for object_id in matched_ids {
        // Match MAT-style OBJECTS behavior: null and dangling refs do not emit a row.
        let Some(target_id) = resolve_objects_projection_target(field, graph, object_id)? else {
            continue;
        };

        rows.push(project_row(&SelectClause::All, graph, None, target_id));
    }

    let total_before_limit = rows.len();
    if let Some(limit) = limit {
        rows.truncate(limit);
    }

    Ok(QueryResult {
        columns,
        rows,
        total_matched: total_before_limit.min(limit.unwrap_or(total_before_limit)),
        truncated: limit.is_some_and(|limit| total_before_limit > limit),
    })
}

fn resolve_objects_projection_target(
    field: &FieldRef,
    graph: &ObjectGraph,
    object_id: ObjectId,
) -> Result<Option<ObjectId>, QueryError> {
    let FieldRef::InstanceField(path) = field else {
        return Err(QueryError::Unsupported(
            "OBJECTS requires an instance field expression".into(),
        ));
    };

    let field_name = normalize_objects_field_name(path)?;
    let Some(object) = graph.get_object(object_id) else {
        return Ok(None);
    };
    let class_name = graph
        .class_name(object.class_id)
        .unwrap_or("<unknown>")
        .replace('/', ".");

    let Some(field_type) = lookup_instance_field_type(graph, object.class_id, field_name) else {
        return Err(QueryError::Unsupported(format!(
            "OBJECTS field '{field_name}' does not exist on class '{class_name}'"
        )));
    };

    if field_type != field_types::OBJECT {
        return Err(QueryError::Unsupported(format!(
            "OBJECTS field '{field_name}' on class '{class_name}' is not an object-reference field"
        )));
    }

    match read_field(object, &graph.classes, field_name, graph.identifier_size) {
        Some(FieldValue::ObjectRef(Some(target_id))) if graph.get_object(target_id).is_some() => {
            Ok(Some(target_id))
        }
        Some(FieldValue::ObjectRef(Some(_))) | Some(FieldValue::ObjectRef(None)) => Ok(None),
        Some(_) => Err(QueryError::Unsupported(format!(
            "OBJECTS field '{field_name}' on class '{class_name}' is not an object-reference field"
        ))),
        None => Err(QueryError::Unsupported(format!(
            "OBJECTS field '{field_name}' could not be read from class '{class_name}'"
        ))),
    }
}

fn normalize_objects_field_name(path: &str) -> Result<&str, QueryError> {
    let segments: Vec<&str> = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();

    match segments.as_slice() {
        [field_name] => Ok(field_name),
        [_, field_name] => Ok(field_name),
        _ => Err(QueryError::NotImplemented(format!(
            "multi-hop OBJECTS not yet supported: '{path}'"
        ))),
    }
}

fn lookup_instance_field_type(
    graph: &ObjectGraph,
    class_id: ObjectId,
    field_name: &str,
) -> Option<u8> {
    fn collect_class_ids(graph: &ObjectGraph, class_id: ObjectId, class_ids: &mut Vec<ObjectId>) {
        if class_id == 0 {
            return;
        }

        let Some(class_info) = graph.classes.get(&class_id) else {
            return;
        };

        collect_class_ids(graph, class_info.super_class_id, class_ids);
        class_ids.push(class_id);
    }

    let mut class_ids = Vec::new();
    collect_class_ids(graph, class_id, &mut class_ids);

    class_ids.into_iter().find_map(|candidate| {
        graph.classes.get(&candidate).and_then(|class_info| {
            class_info
                .instance_fields
                .iter()
                .find(|descriptor| descriptor.name.as_deref() == Some(field_name))
                .map(|descriptor| descriptor.field_type)
        })
    })
}

/// Resolves an `outbounds(id)` / `inbounds(id)` / `dominators(id)` `FROM`
/// source to its candidate object-id set (M15 Slice 15.C).
///
/// `outbounds`/`inbounds` are pure composition over `ObjectGraph::get_references`
/// / `get_referrers` -- both already return an empty `Vec` for an
/// unknown/nonexistent object id (no new "not found" error path), which
/// keeps this consistent with how a `FROM <class-pattern>` matching zero
/// classes already produces an empty-but-valid `QueryResult` rather than a
/// structured error.
///
/// `dominators` requires a deep-mode `DominatorTree`; `validate_supported_query`
/// already rejects the query before this is reached in overview mode, so the
/// `None` arm here is a defensive fallback, never hit in practice.
fn resolve_traversal_candidates(
    traversal: TraversalFunction,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> Result<Vec<ObjectId>, QueryError> {
    match traversal {
        TraversalFunction::Outbounds(id) => Ok(graph.get_references(id)),
        TraversalFunction::Inbounds(id) => Ok(graph.get_referrers(id)),
        TraversalFunction::Dominators(id) => match dominator {
            Some(dom) => Ok(resolve_dominator_chain(dom, id, DOMINATOR_CHAIN_MAX_DEPTH)),
            None => Err(QueryError::feature_unavailable_in_overview_mode(
                "dominators(...)",
                DOMINATORS_OVERVIEW_HINT,
            )),
        },
    }
}

/// Walks `DominatorTree::immediate_dominator` repeatedly starting from
/// `object_id`'s own immediate dominator (not including `object_id` itself),
/// up to `max_depth` entries. Stops early when an object has no immediate
/// dominator, when the walk reaches the virtual super-root, or when a cycle
/// is detected (a previously-visited id would be revisited) -- same
/// bounded-walk-with-cycle-guard shape as M13's `resolve_loader_chain`.
fn resolve_dominator_chain(
    dominator: &DominatorTree,
    object_id: ObjectId,
    max_depth: usize,
) -> Vec<ObjectId> {
    let mut chain = Vec::new();
    let mut visited: HashSet<ObjectId> = HashSet::new();
    visited.insert(object_id);

    let mut current = object_id;
    while chain.len() < max_depth {
        let Some(next) = dominator.immediate_dominator(current) else {
            break;
        };
        if next == VIRTUAL_ROOT_ID {
            break;
        }
        if !visited.insert(next) {
            // Cycle detected -- cannot happen by construction (the
            // dominator tree is a tree), but terminate defensively rather
            // than trust that invariant blindly.
            break;
        }
        chain.push(next);
        current = next;
    }

    chain
}

/// Resolves one or more class-name patterns to a deduplicated object-id set.
/// Each pattern uses the same `matches_class_pattern` path as a standalone
/// single-class `FROM`; when multiple patterns match the same object, it
/// appears once (M22 Slice 22.B).
fn resolve_class_pattern_candidates(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    query: &Query,
    patterns: &[ClassPattern],
    regexes: &[Option<Regex>],
) -> Result<Vec<ObjectId>, QueryError> {
    let mut seen: HashSet<ObjectId> = HashSet::new();
    let mut ids = Vec::new();

    for pattern in patterns {
        for (&object_id, object) in &graph.objects {
            if seen.contains(&object_id) {
                continue;
            }
            if !matches_class_pattern(graph, object.class_id, pattern, query.from.instanceof) {
                continue;
            }
            if !matches_filter(query, graph, dominator, object_id, regexes)? {
                continue;
            }
            seen.insert(object_id);
            ids.push(object_id);
        }
    }

    Ok(ids)
}

fn matches_class_pattern(
    graph: &ObjectGraph,
    class_id: u64,
    pattern: &ClassPattern,
    include_superclasses: bool,
) -> bool {
    let mut current = Some(class_id);
    while let Some(candidate) = current {
        if class_name_matches(graph, candidate, pattern) {
            return true;
        }

        if !include_superclasses {
            break;
        }

        current = graph.classes.get(&candidate).and_then(|class_info| {
            (class_info.super_class_id != 0).then_some(class_info.super_class_id)
        });
    }

    false
}

fn class_name_matches(graph: &ObjectGraph, class_id: u64, pattern: &ClassPattern) -> bool {
    let Some(class_name) = graph
        .class_name(class_id)
        .map(|name| name.replace('/', "."))
    else {
        return false;
    };

    match pattern {
        ClassPattern::Exact(expected) => class_name == *expected,
        ClassPattern::Glob(glob) => glob_match(glob, &class_name),
        // `resolve_matched_ids_at_depth` resolves `ClassPattern::Traversal`
        // and `ClassPattern::Subquery` sources directly (via
        // `resolve_traversal_candidates` / `resolve_subquery_candidates`)
        // before this function is ever reached, and
        // `matches_instanceof_condition` never constructs either variant.
        // Kept for match exhaustiveness; neither is a valid class-name
        // pattern to match against, so these arms always return `false`.
        ClassPattern::Traversal(_) => false,
        ClassPattern::Subquery(_) => false,
        ClassPattern::Multi(_) => false,
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        value.starts_with(prefix) && value.ends_with(suffix)
    } else {
        pattern == value
    }
}

/// Compiles every `=~` regex pattern in `filter.conditions` once, up front,
/// returning a `Vec` parallel to `filter.conditions` (`None` for
/// non-regex conditions). Called exactly once per `execute_query` call --
/// deliberately *not* once per candidate object -- so a WHERE clause with a
/// regex predicate does not pay recompilation cost per object on large
/// heaps (M15 Slice 15.D). A non-string value or a pattern that fails to
/// compile is a structured `QueryError`, never a panic; this is also the
/// defense-in-depth check for a `Query` assembled directly (bypassing the
/// parser's own eager validation), e.g. via the MCP surface.
fn compile_regex_conditions(filter: &WhereClause) -> Result<Vec<Option<Regex>>, QueryError> {
    filter
        .conditions
        .iter()
        .map(|condition| {
            if condition.op != ComparisonOp::RegexMatch {
                return Ok(None);
            }
            let Value::Str(pattern) = &condition.value else {
                return Err(QueryError::Unsupported(
                    "=~ requires a string regex pattern".into(),
                ));
            };
            Regex::new(pattern).map(Some).map_err(|err| {
                QueryError::Unsupported(format!("invalid regex pattern '{pattern}': {err}"))
            })
        })
        .collect()
}

fn matches_filter(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    regexes: &[Option<Regex>],
) -> Result<bool, QueryError> {
    let Some(filter) = &query.filter else {
        return Ok(true);
    };

    let mut result = evaluate_condition(
        &filter.conditions[0],
        graph,
        dominator,
        object_id,
        regexes.first().and_then(Option::as_ref),
    )?;
    for (idx, op) in filter.operators.iter().enumerate() {
        let next = evaluate_condition(
            &filter.conditions[idx + 1],
            graph,
            dominator,
            object_id,
            regexes.get(idx + 1).and_then(Option::as_ref),
        )?;
        result = match op {
            super::types::LogicalOp::And => result && next,
            super::types::LogicalOp::Or => result || next,
        };
    }
    Ok(result)
}

fn evaluate_condition(
    condition: &super::types::Condition,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    regex: Option<&Regex>,
) -> Result<bool, QueryError> {
    if condition.op == ComparisonOp::InstanceOf {
        return Ok(matches_instanceof_condition(
            &condition.field,
            &condition.value,
            graph,
            object_id,
        ));
    }

    if matches!(condition.op, ComparisonOp::IsNull | ComparisonOp::IsNotNull) {
        return evaluate_null_condition(
            &condition.field,
            condition.op,
            graph,
            dominator,
            object_id,
        );
    }

    let left = resolve_field_value(&condition.field, graph, dominator, object_id);
    Ok(compare_values(left, condition.op, &condition.value, regex))
}

fn resolve_field_value(
    field: &FieldRef,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> CellValue {
    let Some(object) = graph.get_object(object_id) else {
        return CellValue::Null;
    };

    match field {
        FieldRef::BuiltIn(BuiltInField::ObjectId) => CellValue::Id(object_id),
        FieldRef::BuiltIn(BuiltInField::ClassName) => CellValue::Str(
            graph
                .class_name(object.class_id)
                .unwrap_or("<unknown>")
                .replace('/', "."),
        ),
        FieldRef::BuiltIn(BuiltInField::ShallowSize) => {
            CellValue::Int(i64::from(object.shallow_size))
        }
        FieldRef::BuiltIn(BuiltInField::RetainedSize) => {
            let retained = dominator
                .map(|dom| dom.retained_size(object_id))
                .unwrap_or(0);
            CellValue::Int(retained as i64)
        }
        FieldRef::BuiltIn(BuiltInField::ObjectAddress) => {
            CellValue::Str(format!("0x{object_id:08X}"))
        }
        FieldRef::BuiltIn(BuiltInField::ToString) => synth_to_string(graph, object_id),
        FieldRef::BuiltIn(BuiltInField::GcRootPath) => resolve_gc_root_path_value(graph, object_id),
        FieldRef::InstanceField(name) => resolve_instance_field_value(object, name, graph),
    }
}

fn resolve_gc_root_path_value(graph: &ObjectGraph, object_id: ObjectId) -> CellValue {
    // TODO(m7-4e-perf): consider per-query caching for repeated @gcRootPath lookups if this
    // becomes a hot path on large result sets. Keep slice E uncached for now.
    let Some(path) = shortest_gc_root_path(graph, object_id, GC_ROOT_PATH_MAX_DEPTH) else {
        return CellValue::Null;
    };

    let mut frames = Vec::with_capacity(path.frames.len() + 1);
    frames.push(format!("GcRoot/{:?}", path.root_kind));
    frames.extend(
        path.frames
            .into_iter()
            .map(|frame_id| gc_root_path_class_name(graph, frame_id)),
    );

    CellValue::Str(frames.join(" -> "))
}

fn gc_root_path_class_name(graph: &ObjectGraph, object_id: ObjectId) -> String {
    let Some(object) = graph.get_object(object_id) else {
        return format!("<unknown object id={object_id}>");
    };

    graph
        .class_name(object.class_id)
        .unwrap_or("<unknown>")
        .replace('/', ".")
}

fn resolve_instance_field_value(
    object: &crate::hprof::HeapObject,
    field_name: &str,
    graph: &ObjectGraph,
) -> CellValue {
    let Some(value) = read_field(object, &graph.classes, field_name, graph.identifier_size) else {
        return CellValue::Null;
    };

    match value {
        FieldValue::Boolean(value) => CellValue::Bool(value),
        FieldValue::Byte(value) => CellValue::Int(i64::from(value)),
        FieldValue::Short(value) => CellValue::Int(i64::from(value)),
        FieldValue::Int(value) => CellValue::Int(i64::from(value)),
        FieldValue::Long(value) => CellValue::Int(value),
        FieldValue::Char(value) => std::char::from_u32(u32::from(value))
            .map(|ch| CellValue::Str(ch.to_string()))
            .unwrap_or(CellValue::Null),
        FieldValue::Float(value) => CellValue::Str(value.to_string()),
        FieldValue::Double(value) => CellValue::Str(value.to_string()),
        FieldValue::ObjectRef(Some(reference)) => extract_string_value(graph, reference)
            .map(CellValue::Str)
            .unwrap_or(CellValue::Id(reference)),
        FieldValue::ObjectRef(None) => CellValue::Null,
    }
}

fn evaluate_null_condition(
    field: &FieldRef,
    op: ComparisonOp,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> Result<bool, QueryError> {
    let matches_null = match field {
        FieldRef::BuiltIn(_) => matches!(
            resolve_field_value(field, graph, dominator, object_id),
            CellValue::Null
        ),
        FieldRef::InstanceField(field_name) => {
            let Some(object) = graph.get_object(object_id) else {
                return Ok(false);
            };

            let class_name = graph
                .class_name(object.class_id)
                .unwrap_or("<unknown>")
                .replace('/', ".");
            let operator = if op == ComparisonOp::IsNotNull {
                "IS NOT NULL"
            } else {
                "IS NULL"
            };

            let Some(field_type) = lookup_instance_field_type(graph, object.class_id, field_name)
            else {
                return Err(QueryError::Unsupported(format!(
                    "{operator} field '{field_name}' does not exist on class '{class_name}'"
                )));
            };

            if field_type != field_types::OBJECT {
                return Err(QueryError::Unsupported(format!(
                    "{operator} not applicable to primitive field '{field_name}' on class '{class_name}'"
                )));
            }

            match read_field(object, &graph.classes, field_name, graph.identifier_size) {
                Some(FieldValue::ObjectRef(None)) => true,
                Some(FieldValue::ObjectRef(Some(_))) => false,
                Some(_) => {
                    return Err(QueryError::Unsupported(format!(
                        "{operator} not applicable to primitive field '{field_name}' on class '{class_name}'"
                    )));
                }
                None => {
                    return Err(QueryError::Unsupported(format!(
                        "{operator} field '{field_name}' could not be read from class '{class_name}'"
                    )));
                }
            }
        }
    };

    Ok(if op == ComparisonOp::IsNotNull {
        !matches_null
    } else {
        matches_null
    })
}

fn matches_instanceof_condition(
    field: &FieldRef,
    value: &Value,
    graph: &ObjectGraph,
    object_id: ObjectId,
) -> bool {
    let Value::Str(expected_class) = value else {
        return false;
    };

    let Some(target_id) = resolve_reference_target(field, graph, object_id) else {
        return false;
    };

    let Some(target_object) = graph.get_object(target_id) else {
        return false;
    };

    let pattern = if expected_class.contains('*') {
        ClassPattern::Glob(expected_class.clone())
    } else {
        ClassPattern::Exact(expected_class.clone())
    };

    matches_class_pattern(graph, target_object.class_id, &pattern, true)
}

fn resolve_reference_target(
    field: &FieldRef,
    graph: &ObjectGraph,
    object_id: ObjectId,
) -> Option<ObjectId> {
    let object = graph.get_object(object_id)?;
    match field {
        FieldRef::BuiltIn(_) => None,
        FieldRef::InstanceField(name) => {
            match read_field(object, &graph.classes, name, graph.identifier_size)? {
                FieldValue::ObjectRef(Some(reference)) => Some(reference),
                _ => None,
            }
        }
    }
}

fn compare_values(left: CellValue, op: ComparisonOp, right: &Value, regex: Option<&Regex>) -> bool {
    match (left, right) {
        (CellValue::Int(left), Value::Int(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            ComparisonOp::Gt => left > *right,
            ComparisonOp::Lt => left < *right,
            ComparisonOp::Ge => left >= *right,
            ComparisonOp::Le => left <= *right,
            _ => false,
        },
        (CellValue::Str(left), Value::Str(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            ComparisonOp::Like => like_match(right, &left),
            ComparisonOp::Contains => left.contains(right),
            ComparisonOp::RegexMatch => regex.is_some_and(|regex| regex.is_match(&left)),
            _ => false,
        },
        (CellValue::Bool(left), Value::Bool(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            _ => false,
        },
        (CellValue::Null, Value::Null) => matches!(op, ComparisonOp::Eq),
        (CellValue::Id(_), Value::Null) | (CellValue::Str(_), Value::Null) => {
            matches!(op, ComparisonOp::Ne)
        }
        (CellValue::Id(left), Value::Int(right)) => match op {
            ComparisonOp::Eq => left == *right as u64,
            ComparisonOp::Ne => left != *right as u64,
            ComparisonOp::Gt => left > *right as u64,
            ComparisonOp::Lt => left < *right as u64,
            ComparisonOp::Ge => left >= *right as u64,
            ComparisonOp::Le => left <= *right as u64,
            _ => false,
        },
        _ => false,
    }
}

fn like_match(pattern: &str, value: &str) -> bool {
    let mut regex_pattern = String::from("(?s)^");
    for ch in pattern.chars() {
        match ch {
            '%' => regex_pattern.push_str(".*"),
            '_' => regex_pattern.push('.'),
            _ => regex_pattern.push_str(&regex::escape(&ch.to_string())),
        }
    }
    regex_pattern.push('$');

    Regex::new(&regex_pattern)
        .map(|regex| regex.is_match(value))
        .unwrap_or(false)
}

fn project_row(
    select: &SelectClause,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> Vec<CellValue> {
    let fields: Vec<FieldRef> = match select {
        SelectClause::All => vec![
            FieldRef::BuiltIn(BuiltInField::ObjectId),
            FieldRef::BuiltIn(BuiltInField::ClassName),
        ],
        SelectClause::Fields(fields) => fields.clone(),
        SelectClause::Objects(_) => vec![
            FieldRef::BuiltIn(BuiltInField::ObjectId),
            FieldRef::BuiltIn(BuiltInField::ClassName),
        ],
    };

    fields
        .iter()
        .map(|field| resolve_field_value(field, graph, dominator, object_id))
        .collect()
}
