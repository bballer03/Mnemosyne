use crate::{AnalysisMode, Comparison, Severity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyResult {
    pub mode_used: AnalysisMode,
    pub mode_requested: AnalysisMode,
    pub violations: Vec<Violation>,
    pub evaluations: Vec<Evaluation>,
    pub skipped: Vec<SkippedRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    pub rule_id: String,
    pub predicate: String,
    pub severity: Severity,
    pub message: String,
    pub actual: serde_json::Value,
    pub expected: serde_json::Value,
    pub comparison: Comparison,
    pub remediation_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evaluation {
    pub rule_id: String,
    pub passed: bool,
    pub actual: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkippedRule {
    pub rule_id: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    DeepOnlyInOverviewMode,
    UnsupportedInThisMode,
}
