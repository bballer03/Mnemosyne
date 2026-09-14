use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub select: SelectClause,
    pub from: FromClause,
    pub filter: Option<WhereClause>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectClause {
    All,
    Fields(Vec<FieldRef>),
    /// `SELECT OBJECTS <field>` — project referenced objects; duplicate
    /// targets are retained (MAT SELECT Clause without DISTINCT).
    Objects(FieldRef),
    /// `SELECT DISTINCT OBJECTS <field>` — same projection as [`Objects`],
    /// but collapse duplicate target object ids while preserving first-seen
    /// order (MAT SELECT Clause "Select unique objects" / DISTINCT OBJECTS).
    /// Bounded: DISTINCT is only accepted with OBJECTS, not `SELECT DISTINCT *`
    /// or field lists — matching the hop/multi-class style of explicit bounds.
    DistinctObjects(FieldRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldRef {
    BuiltIn(BuiltInField),
    InstanceField(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltInField {
    ObjectId,
    ClassName,
    ShallowSize,
    RetainedSize,
    ObjectAddress,
    ToString,
    GcRootPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FromClause {
    pub class_pattern: ClassPattern,
    pub instanceof: bool,
}

/// Maximum class patterns in a comma-separated `FROM` list (M22 Slice 22.B).
pub const MAX_MULTI_CLASS_FROM_LIST_SIZE: usize = 8;

/// Maximum object-reference field hops in `SELECT OBJECTS` (M22 Slice 22.C).
/// A path such as `n.parent.link.target` traverses three hops after a
/// non-field alias prefix; a fourth hop is rejected with a structured limit
/// error. Unprefixed paths count every segment as a hop (so
/// `parent.link.target.extra` is four hops and is rejected).
pub const MAX_OBJECTS_FIELD_HOPS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassPattern {
    Exact(String),
    Glob(String),
    /// Comma-separated class patterns in `FROM "a", "b", ...` (M22 Slice 22.B).
    /// Single-class queries keep `Exact`/`Glob` for backward-compatible serialization.
    /// Each entry reuses the same exact/glob resolution as a standalone `FROM`; the
    /// executor unions matched object ids and deduplicates before ordering/limit.
    Multi(Vec<ClassPattern>),
    /// A traversal function producing an explicit object-id set instead of
    /// matching by class name. Slots into the same `FromClause.class_pattern`
    /// extension point as `Exact`/`Glob` rather than introducing a parallel
    /// `Query.from` shape -- see M15 Slice 15.C commit body for rationale.
    Traversal(TraversalFunction),
    /// `FROM OBJECTS (<subquery>)` (M15 Slice 15.E). The boxed `Query` is
    /// evaluated first via its own FROM+WHERE+LIMIT pipeline (its `SELECT`
    /// clause is not consulted -- see `executor::resolve_subquery_candidates`),
    /// and the resulting object-id set becomes this `FromClause`'s candidate
    /// set, same extension-point shape as `Traversal` above. Bounded to one
    /// level of nesting: a `Query` reachable through this variant must not
    /// itself contain another `ClassPattern::Subquery` in its `from` --
    /// enforced by the parser at parse time (`parser::MAX_SUBQUERY_NESTING_DEPTH`)
    /// and, defense-in-depth, by the executor for a `Query` assembled
    /// directly (e.g. via the MCP surface, bypassing the parser).
    Subquery(Box<Query>),
}

/// `outbounds(id)` / `inbounds(id)` / `dominators(id)` OQL traversal
/// functions (M15 Slice 15.C). Each wraps a literal object id; nested-query
/// arguments (`outbounds(SELECT ...)`) are out of scope for this slice and
/// land with subqueries in M15 Slice 15.E.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalFunction {
    /// Objects the given object id directly references (outgoing edges).
    Outbounds(u64),
    /// Objects that directly reference the given object id (incoming edges).
    Inbounds(u64),
    /// The chain of immediate dominators of the given object id, nearest
    /// first, walking towards the virtual super-root (exclusive).
    Dominators(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhereClause {
    pub conditions: Vec<Condition>,
    pub operators: Vec<LogicalOp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: FieldRef,
    pub op: ComparisonOp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    /// SQL-style string pattern match.
    ///
    /// `%` matches any sequence, `_` matches any single character, and all
    /// other regex metacharacters are treated literally.
    Like,
    /// Case-sensitive substring match on string values.
    Contains,
    /// Regex pattern match (`=~`) on string-capable fields (M15 Slice 15.D).
    ///
    /// Powered by the `regex` crate's linear-time (non-backtracking) engine
    /// so a user-supplied pattern cannot become a ReDoS footgun against
    /// adversarial input -- see `docs/design/milestone-15-mat-backend-parity.md`
    /// §6 R3. The pattern itself always travels as a plain `Value::Str` (this
    /// enum stays `Copy`/`Eq`/`Serialize`, so it cannot hold a compiled
    /// `regex::Regex`); the parser eagerly validates the pattern compiles so
    /// a malformed regex fails fast before any graph work, and the executor
    /// independently (re-)compiles it once per query evaluation -- not once
    /// per candidate object -- for both defense-in-depth (a `Query` built
    /// programmatically, e.g. via the MCP surface, bypasses the parser) and
    /// performance.
    RegexMatch,
    IsNull,
    IsNotNull,
    InstanceOf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Str(String),
    Null,
    Bool(bool),
}

/// A top-level OQL statement: either a single `Query`, or two `Query`s
/// joined by `UNION` (M15 Slice 15.E §4.1 item 5). Deliberately a separate
/// type from `Query` itself rather than a new field/variant on `Query` --
/// `Query` keeps its exact M7-4 shape (`select`/`from`/`filter`/`limit`),
/// so every existing caller and test that constructs or matches a `Query`
/// literal (the CLI `query` command, the MCP `query_heap` handler, the
/// Tauri `query_heap` command, and every OQL test predating this slice)
/// needs zero changes. `UNION` is intentionally binary and non-recursive
/// (no `Query3` chained on): the design doc bounds this slice to "two
/// `Query` results concatenated, deduplicated by object id", not an
/// open-ended `UNION` chain. A `Query` reached through `Union` may still
/// itself use a one-level `ClassPattern::Subquery` FROM source -- the two
/// features compose independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryStatement {
    Single(Query),
    Union(Query, Query),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub total_matched: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellValue {
    Id(u64),
    Str(String),
    Int(i64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    FeatureUnavailableInOverviewMode { feature: String, hint: String },
    NotImplemented(String),
    Unsupported(String),
}

impl QueryError {
    pub fn feature_unavailable_in_overview_mode(
        feature: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self::FeatureUnavailableInOverviewMode {
            feature: feature.into(),
            hint: hint.into(),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::FeatureUnavailableInOverviewMode { feature, .. } => {
                write!(f, "'{feature}' is a deep-mode-only OQL feature.")
            }
            QueryError::NotImplemented(detail) => {
                write!(f, "Operation not yet implemented: {detail}")
            }
            QueryError::Unsupported(detail) => {
                write!(f, "Unsupported operation: {detail}")
            }
        }
    }
}

impl std::error::Error for QueryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParseError {
    message: String,
}

impl QueryParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryParseError {}
