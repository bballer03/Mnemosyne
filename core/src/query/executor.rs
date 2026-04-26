use super::synth::synth_to_string;
use super::types::{
    BuiltInField, CellValue, ClassPattern, ComparisonOp, FieldRef, Query, QueryError, QueryResult,
    SelectClause, Value,
};
use crate::{
    analysis::string_analysis::extract_string_value,
    graph::DominatorTree,
    hprof::{field_types, read_field, FieldValue, ObjectGraph, ObjectId},
};
use regex::Regex;

const OBJECTS_OVERVIEW_HINT: &str =
    "re-run with --mode deep; OBJECTS requires deep-mode object field traversal.";
const RETAINED_SIZE_OVERVIEW_HINT: &str =
    "re-run with --mode deep or use @shallowSize for a per-object approximation.";
const STRING_PREDICATE_OVERVIEW_HINT: &str =
    "re-run with --mode deep; LIKE and CONTAINS on instance fields require deep-mode field data.";
const TO_STRING_OVERVIEW_HINT: &str =
    "re-run with --mode deep; @toString requires deep-mode field data.";

pub fn execute_query(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> Result<QueryResult, QueryError> {
    validate_supported_query(query, dominator)?;

    let columns = projected_columns(&query.select);
    let mut matched_ids = Vec::new();

    for (&object_id, object) in &graph.objects {
        if !matches_class_pattern(
            graph,
            object.class_id,
            &query.from.class_pattern,
            query.from.instanceof,
        ) {
            continue;
        }
        if !matches_filter(query, graph, dominator, object_id) {
            continue;
        }
        matched_ids.push(object_id);
    }

    matched_ids.sort_unstable();

    if let SelectClause::Objects(field) = &query.select {
        return execute_objects_projection(query, graph, field, matched_ids, columns);
    }

    let total_before_limit = matched_ids.len();
    if let Some(limit) = query.limit {
        matched_ids.truncate(limit);
    }

    let mut rows = Vec::with_capacity(matched_ids.len());
    for object_id in matched_ids {
        rows.push(project_row(&query.select, graph, dominator, object_id));
    }

    Ok(QueryResult {
        columns,
        rows,
        total_matched: total_before_limit.min(query.limit.unwrap_or(total_before_limit)),
        truncated: query.limit.is_some_and(|limit| total_before_limit > limit),
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
    if query_references_gc_root_path(query) {
        return Err(slice_a_not_implemented("@gcRootPath query evaluation"));
    }

    if query_uses_deferred_null_predicates(query) {
        return Err(slice_a_not_implemented(
            "IS NULL / IS NOT NULL query evaluation",
        ));
    }

    if dominator.is_none() {
        if matches!(query.select, SelectClause::Objects(_)) {
            return Err(QueryError::feature_unavailable_in_overview_mode(
                "OBJECTS",
                OBJECTS_OVERVIEW_HINT,
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

fn query_uses_deferred_null_predicates(query: &Query) -> bool {
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
            if !matches!(condition.op, ComparisonOp::Like | ComparisonOp::Contains) {
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

fn slice_a_not_implemented(feature: &str) -> QueryError {
    QueryError::NotImplemented(format!("{feature} is not yet implemented in slice A"))
}

fn execute_objects_projection(
    query: &Query,
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
    if let Some(limit) = query.limit {
        rows.truncate(limit);
    }

    Ok(QueryResult {
        columns,
        rows,
        total_matched: total_before_limit.min(query.limit.unwrap_or(total_before_limit)),
        truncated: query.limit.is_some_and(|limit| total_before_limit > limit),
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
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        value.starts_with(prefix) && value.ends_with(suffix)
    } else {
        pattern == value
    }
}

fn matches_filter(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> bool {
    let Some(filter) = &query.filter else {
        return true;
    };

    let mut result = evaluate_condition(&filter.conditions[0], graph, dominator, object_id);
    for (idx, op) in filter.operators.iter().enumerate() {
        let next = evaluate_condition(&filter.conditions[idx + 1], graph, dominator, object_id);
        result = match op {
            super::types::LogicalOp::And => result && next,
            super::types::LogicalOp::Or => result || next,
        };
    }
    result
}

fn evaluate_condition(
    condition: &super::types::Condition,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> bool {
    if condition.op == ComparisonOp::InstanceOf {
        return matches_instanceof_condition(&condition.field, &condition.value, graph, object_id);
    }

    let left = resolve_field_value(&condition.field, graph, dominator, object_id);
    compare_values(left, condition.op, &condition.value)
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
        FieldRef::BuiltIn(BuiltInField::GcRootPath) => CellValue::Null,
        FieldRef::InstanceField(name) => resolve_instance_field_value(object, name, graph),
    }
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

fn compare_values(left: CellValue, op: ComparisonOp, right: &Value) -> bool {
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
