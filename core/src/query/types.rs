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
    Objects(FieldRef),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassPattern {
    Exact(String),
    Glob(String),
    /// A traversal function producing an explicit object-id set instead of
    /// matching by class name. Slots into the same `FromClause.class_pattern`
    /// extension point as `Exact`/`Glob` rather than introducing a parallel
    /// `Query.from` shape -- see M15 Slice 15.C commit body for rationale.
    Traversal(TraversalFunction),
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
