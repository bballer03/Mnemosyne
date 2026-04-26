use crate::{CoreError, CoreResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

mod eval;
mod input;
mod result;

pub use eval::evaluate;
pub use input::PolicyInput;
pub use result::{Evaluation, PolicyResult, SkipReason, SkippedRule, Violation};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Policy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<PolicyMeta>,
    #[serde(default, skip_serializing_if = "PolicyDefaults::is_default")]
    pub defaults: PolicyDefaults,
    #[serde(rename = "rule", default)]
    pub rules: Vec<PolicyRule>,
}

impl Policy {
    pub fn from_toml_str(s: &str) -> CoreResult<Self> {
        let raw: RawPolicy = toml::from_str(s).map_err(|err| CoreError::ConfigError {
            detail: format!("invalid policy TOML: {err}"),
            suggestion: Some(
                "Check the policy syntax, comparison operators, and required rule fields.".into(),
            ),
        })?;

        validate_raw_policy(&raw)?;
        Ok(raw.into_policy())
    }

    pub fn from_toml_file(path: impl AsRef<Path>) -> CoreResult<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|err| CoreError::ConfigError {
            detail: format!("failed to read policy file '{}': {err}", path.display()),
            suggestion: Some("Ensure the policy file exists and is readable.".into()),
        })?;

        Self::from_toml_str(&contents)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PolicyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyDefaults {
    #[serde(default)]
    pub severity: Severity,
}

impl PolicyDefaults {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

impl Default for PolicyDefaults {
    fn default() -> Self {
        Self {
            severity: Severity::Error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyRule {
    pub id: String,
    pub predicate: Predicate,
    #[serde(rename = "op", alias = "comparison")]
    pub comparison: Comparison,
    #[serde(rename = "value", alias = "threshold")]
    pub threshold: u64,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation_hint: Option<String>,
    #[serde(default, skip_serializing_if = "ModeRequirement::is_any")]
    pub mode_requirement: ModeRequirement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    TotalBytes,
    TotalInstances,
    ClassInstances,
    ClassBytes,
    LoadedClassCount,
    GcRootCount,
    ProvenanceMustNotContain,
    LeakCount,
    RetainedSize,
    DominatorRootCount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Comparison {
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Lte,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    #[default]
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModeRequirement {
    #[default]
    Any,
    DeepOnly,
}

impl ModeRequirement {
    fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    #[serde(default)]
    meta: Option<PolicyMeta>,
    #[serde(default)]
    defaults: PolicyDefaults,
    #[serde(rename = "rule", default)]
    rules: Vec<RawPolicyRule>,
}

impl RawPolicy {
    fn into_policy(self) -> Policy {
        let default_severity = self.defaults.severity;
        let rules = self
            .rules
            .into_iter()
            .map(|rule| rule.into_policy_rule(default_severity))
            .collect();

        Policy {
            meta: self.meta,
            defaults: self.defaults,
            rules,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicyRule {
    id: String,
    predicate: Predicate,
    #[serde(rename = "op", alias = "comparison")]
    comparison: Comparison,
    #[serde(rename = "value", alias = "threshold")]
    threshold: u64,
    #[serde(default)]
    severity: Option<Severity>,
    #[serde(default)]
    remediation_hint: Option<String>,
    #[serde(default)]
    mode_requirement: Option<ModeRequirement>,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    class_pattern: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    severity_filter: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

impl RawPolicyRule {
    fn into_policy_rule(self, default_severity: Severity) -> PolicyRule {
        PolicyRule {
            id: self.id,
            predicate: self.predicate,
            comparison: self.comparison,
            threshold: self.threshold,
            severity: self.severity.unwrap_or(default_severity),
            remediation_hint: self.remediation_hint,
            mode_requirement: self.mode_requirement.unwrap_or_default(),
            class: self.class,
            class_pattern: self.class_pattern,
            kind: self.kind,
            severity_filter: self.severity_filter,
            scope: self.scope,
        }
    }
}

fn validate_raw_policy(policy: &RawPolicy) -> CoreResult<()> {
    let mut seen_rule_ids = HashSet::new();

    for rule in &policy.rules {
        if !seen_rule_ids.insert(rule.id.as_str()) {
            return Err(CoreError::ConfigError {
                detail: format!("duplicate policy rule id '{}'", rule.id),
                suggestion: Some("Ensure every [[rule]].id is unique.".into()),
            });
        }

        if rule.class.is_some() && rule.class_pattern.is_some() {
            return Err(CoreError::ConfigError {
                detail: format!(
                    "rule '{}' cannot set both 'class' and 'class_pattern'",
                    rule.id
                ),
                suggestion: Some("Pick either an exact class match or a regex pattern.".into()),
            });
        }

        if let Some(pattern) = &rule.class_pattern {
            Regex::new(pattern).map_err(|err| CoreError::ConfigError {
                detail: format!("invalid class_pattern regex for rule '{}': {err}", rule.id),
                suggestion: Some("Use Rust regex syntax or remove class_pattern.".into()),
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_rule_toml(mode_requirement: &str, extra: &str) -> String {
        format!(
            concat!(
                "[meta]\n",
                "name = \"slice-a\"\n",
                "description = \"policy parser smoke test\"\n",
                "version = \"1.0\"\n\n",
                "[[rule]]\n",
                "id = \"heap-budget\"\n",
                "predicate = \"total_bytes\"\n",
                "op = \"<=\"\n",
                "value = 1024\n",
                "severity = \"warning\"\n",
                "remediation_hint = \"investigate retained roots\"\n",
                "mode_requirement = \"{mode_requirement}\"\n",
                "{extra}"
            ),
            mode_requirement = mode_requirement,
            extra = extra,
        )
    }

    #[test]
    fn policy_parses_minimal_valid_toml() {
        let policy = Policy::from_toml_str(&minimal_rule_toml("deep_only", ""))
            .expect("minimal policy TOML should parse");

        assert_eq!(
            policy.meta.as_ref().and_then(|meta| meta.name.as_deref()),
            Some("slice-a")
        );
        assert_eq!(policy.rules.len(), 1);

        let rule = &policy.rules[0];
        assert_eq!(rule.id, "heap-budget");
        assert_eq!(rule.predicate, Predicate::TotalBytes);
        assert_eq!(rule.comparison, Comparison::Lte);
        assert_eq!(rule.threshold, 1024);
        assert_eq!(rule.severity, Severity::Warning);
        assert_eq!(
            rule.remediation_hint.as_deref(),
            Some("investigate retained roots")
        );
        assert_eq!(rule.mode_requirement, ModeRequirement::DeepOnly);

        let roundtrip = toml::to_string(&policy).expect("policy should serialize back to TOML");
        let reparsed = Policy::from_toml_str(&roundtrip).expect("round-tripped TOML should parse");
        assert_eq!(reparsed, policy);
    }

    #[test]
    fn policy_parses_all_predicate_variants() {
        let predicates = [
            "total_bytes",
            "total_instances",
            "class_instances",
            "class_bytes",
            "loaded_class_count",
            "gc_root_count",
            "provenance_must_not_contain",
            "leak_count",
            "retained_size",
            "dominator_root_count",
        ];

        let mut toml = String::new();
        for (index, predicate) in predicates.iter().enumerate() {
            toml.push_str(&format!(
                concat!(
                    "[[rule]]\n",
                    "id = \"rule-{index}\"\n",
                    "predicate = \"{predicate}\"\n",
                    "op = \"<=\"\n",
                    "value = {value}\n",
                    "severity = \"error\"\n\n"
                ),
                index = index,
                predicate = predicate,
                value = index + 1,
            ));
        }

        let policy = Policy::from_toml_str(&toml).expect("all predicate variants should parse");
        let parsed = policy
            .rules
            .iter()
            .map(|rule| rule.predicate)
            .collect::<Vec<_>>();

        assert_eq!(
            parsed,
            vec![
                Predicate::TotalBytes,
                Predicate::TotalInstances,
                Predicate::ClassInstances,
                Predicate::ClassBytes,
                Predicate::LoadedClassCount,
                Predicate::GcRootCount,
                Predicate::ProvenanceMustNotContain,
                Predicate::LeakCount,
                Predicate::RetainedSize,
                Predicate::DominatorRootCount,
            ]
        );
    }

    #[test]
    fn policy_rejects_unknown_predicate() {
        let err = Policy::from_toml_str(
            r#"[[rule]]
id = "bad-predicate"
predicate = "totl_bytes"
op = "<="
value = 5
severity = "error"
"#,
        )
        .expect_err("unknown predicate should fail");

        let message = err.to_string();
        assert!(message.contains("predicate") || message.contains("unknown variant"));
    }

    #[test]
    fn policy_rejects_invalid_comparison() {
        let err = Policy::from_toml_str(
            r#"[[rule]]
id = "bad-op"
predicate = "total_bytes"
op = "<>"
value = 5
severity = "error"
"#,
        )
        .expect_err("invalid comparison should fail");

        let message = err.to_string();
        assert!(
            message.contains("op")
                || message.contains("comparison")
                || message.contains("unknown variant")
        );
    }

    #[test]
    fn policy_rejects_missing_required_field() {
        let err = Policy::from_toml_str(
            r#"[[rule]]
id = "missing-threshold"
predicate = "total_bytes"
op = "<="
severity = "error"
"#,
        )
        .expect_err("missing threshold should fail");

        let message = err.to_string();
        assert!(message.contains("value") || message.contains("threshold"));
    }

    #[test]
    fn policy_severity_serde_roundtrip() {
        let cases = [
            (Severity::Info, "\"info\""),
            (Severity::Warning, "\"warning\""),
            (Severity::Error, "\"error\""),
            (Severity::Critical, "\"critical\""),
        ];

        for (severity, expected) in cases {
            let serialized = serde_json::to_string(&severity).expect("severity should serialize");
            assert_eq!(serialized, expected);

            let parsed: Severity =
                serde_json::from_str(&serialized).expect("severity should deserialize");
            assert_eq!(parsed, severity);
        }
    }

    #[test]
    fn policy_mode_requirement_defaults_to_any_when_omitted() {
        let policy = Policy::from_toml_str(
            r#"[[rule]]
id = "default-mode"
predicate = "total_instances"
op = "<="
value = 25
severity = "error"
"#,
        )
        .expect("policy without mode_requirement should parse");

        assert_eq!(policy.rules[0].mode_requirement, ModeRequirement::Any);
    }

    #[test]
    fn policy_from_toml_file_reads_disk() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let path = tempdir.path().join("policy.toml");
        std::fs::write(&path, minimal_rule_toml("any", "")).expect("policy file should be written");

        let policy = Policy::from_toml_file(&path).expect("policy TOML file should parse");
        assert_eq!(policy.rules.len(), 1);
        assert_eq!(policy.rules[0].mode_requirement, ModeRequirement::Any);
    }
}
