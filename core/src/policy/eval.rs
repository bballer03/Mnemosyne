use crate::{AnalysisMode, GcRootKind, ProvenanceKind};
use regex::Regex;
use serde_json::{json, Value};

use super::{
    Comparison, Evaluation, ModeRequirement, Policy, PolicyInput, PolicyResult, Predicate,
    SkipReason, SkippedRule, Violation,
};

pub fn evaluate(
    policy: &Policy,
    input: &PolicyInput<'_>,
    requested_mode: AnalysisMode,
) -> PolicyResult {
    let mut result = PolicyResult {
        mode_used: input.mode_used(),
        mode_requested: requested_mode,
        violations: Vec::new(),
        evaluations: Vec::new(),
        skipped: Vec::new(),
    };

    for rule in &policy.rules {
        if matches!(input, PolicyInput::Overview(_))
            && matches!(rule.mode_requirement, ModeRequirement::DeepOnly)
        {
            if requested_mode == AnalysisMode::Overview {
                result.violations.push(Violation {
                    rule_id: rule.id.clone(),
                    predicate: predicate_name(rule.predicate).to_string(),
                    severity: super::Severity::Critical,
                    message: format!(
                        "deep-only predicate `{}` cannot run in explicit overview mode",
                        rule.id
                    ),
                    actual: Value::Null,
                    expected: Value::Null,
                    comparison: rule.comparison,
                    remediation_hint: rule.remediation_hint.clone(),
                });
            } else {
                result.skipped.push(SkippedRule {
                    rule_id: rule.id.clone(),
                    reason: SkipReason::DeepOnlyInOverviewMode,
                });
            }
            continue;
        }

        let Some(outcome) = evaluate_rule(rule, input) else {
            result.skipped.push(SkippedRule {
                rule_id: rule.id.clone(),
                reason: SkipReason::UnsupportedInThisMode,
            });
            continue;
        };

        result.evaluations.push(Evaluation {
            rule_id: rule.id.clone(),
            passed: outcome.passed,
            actual: outcome.actual.clone(),
        });

        if !outcome.passed {
            result.violations.push(Violation {
                rule_id: rule.id.clone(),
                predicate: predicate_name(rule.predicate).to_string(),
                severity: rule.severity,
                message: outcome.message,
                actual: outcome.actual,
                expected: outcome.expected,
                comparison: rule.comparison,
                remediation_hint: rule.remediation_hint.clone(),
            });
        }
    }

    result
}

struct RuleOutcome {
    passed: bool,
    actual: Value,
    expected: Value,
    message: String,
}

fn evaluate_rule(rule: &super::PolicyRule, input: &PolicyInput<'_>) -> Option<RuleOutcome> {
    match rule.predicate {
        Predicate::TotalBytes => {
            let actual = match input {
                PolicyInput::Deep(response) => response.summary.total_size_bytes,
                PolicyInput::Overview(summary) => summary.total_size_bytes,
            };
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::TotalInstances => {
            let actual = match input {
                PolicyInput::Deep(response) => response.summary.total_objects,
                PolicyInput::Overview(summary) => summary.total_instances,
            };
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::ClassInstances => {
            let actual = match input {
                PolicyInput::Deep(response) => {
                    aggregate_deep_class_value(rule, response, |class| class.instances)
                }
                PolicyInput::Overview(summary) => {
                    aggregate_overview_class_value(rule, summary, |class| class.instance_count)
                }
            }?;
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::ClassBytes => {
            let actual = match input {
                PolicyInput::Deep(response) => {
                    aggregate_deep_class_value(rule, response, |class| class.total_size_bytes)
                }
                PolicyInput::Overview(summary) => {
                    aggregate_overview_class_value(rule, summary, |class| {
                        class.approx_shallow_bytes
                    })
                }
            }?;
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::LoadedClassCount => {
            let actual = match input {
                PolicyInput::Deep(response) => response.summary.classes.len() as u64,
                PolicyInput::Overview(summary) => summary.loaded_class_count,
            };
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::GcRootCount => {
            let PolicyInput::Overview(summary) = input else {
                return None;
            };
            let kind = parse_gc_root_kind(rule.kind.as_deref()?)?;
            let actual = summary
                .gc_root_counts
                .get(&kind)
                .copied()
                .unwrap_or_default();
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::ProvenanceMustNotContain => {
            let kind = parse_provenance_kind(rule.kind.as_deref()?)?;
            let has_marker = match input {
                PolicyInput::Deep(response) => has_provenance_kind(&response.provenance, kind),
                PolicyInput::Overview(summary) => has_provenance_kind(&summary.provenance, kind),
            };

            Some(RuleOutcome {
                passed: !has_marker,
                actual: json!(has_marker),
                expected: json!(provenance_kind_name(kind)),
                message: format!(
                    "expected provenance_must_not_contain {} but marker was present",
                    provenance_kind_name(kind)
                ),
            })
        }
        // TODO(m7-2.c): implement deep-only evaluators for leak_count, retained_size,
        // and dominator_root_count once Slice M7-2.C lands.
        Predicate::LeakCount | Predicate::RetainedSize | Predicate::DominatorRootCount => None,
    }
}

fn evaluate_numeric(rule: &super::PolicyRule, actual: u64) -> RuleOutcome {
    RuleOutcome {
        passed: compare_u64(actual, rule.comparison, rule.threshold),
        actual: json!(actual),
        expected: json!(rule.threshold),
        message: format!(
            "expected {} {} {}, got {}",
            predicate_name(rule.predicate),
            comparison_symbol(rule.comparison),
            rule.threshold,
            actual
        ),
    }
}

fn compare_u64(actual: u64, comparison: Comparison, expected: u64) -> bool {
    match comparison {
        Comparison::Lt => actual < expected,
        Comparison::Lte => actual <= expected,
        Comparison::Gt => actual > expected,
        Comparison::Gte => actual >= expected,
        Comparison::Eq => actual == expected,
        Comparison::Ne => actual != expected,
    }
}

fn comparison_symbol(comparison: Comparison) -> &'static str {
    match comparison {
        Comparison::Lt => "<",
        Comparison::Lte => "<=",
        Comparison::Gt => ">",
        Comparison::Gte => ">=",
        Comparison::Eq => "==",
        Comparison::Ne => "!=",
    }
}

fn predicate_name(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::TotalBytes => "total_bytes",
        Predicate::TotalInstances => "total_instances",
        Predicate::ClassInstances => "class_instances",
        Predicate::ClassBytes => "class_bytes",
        Predicate::LoadedClassCount => "loaded_class_count",
        Predicate::GcRootCount => "gc_root_count",
        Predicate::ProvenanceMustNotContain => "provenance_must_not_contain",
        Predicate::LeakCount => "leak_count",
        Predicate::RetainedSize => "retained_size",
        Predicate::DominatorRootCount => "dominator_root_count",
    }
}

fn aggregate_overview_class_value(
    rule: &super::PolicyRule,
    summary: &crate::OverviewSummary,
    value_of: impl Fn(&crate::OverviewClassStat) -> u64,
) -> Option<u64> {
    // Slice M7-2.B aggregates all matching classes into a single numeric actual.
    if let Some(class_name) = &rule.class {
        return Some(
            summary
                .class_stats
                .entries
                .iter()
                .filter(|entry| entry.class_name == *class_name)
                .map(value_of)
                .sum(),
        );
    }

    let pattern = rule.class_pattern.as_deref()?;
    let regex = Regex::new(pattern).ok()?;
    Some(
        summary
            .class_stats
            .entries
            .iter()
            .filter(|entry| regex.is_match(&entry.class_name))
            .map(value_of)
            .sum(),
    )
}

fn aggregate_deep_class_value(
    rule: &super::PolicyRule,
    response: &crate::AnalyzeResponse,
    value_of: impl Fn(&crate::hprof::ClassStat) -> u64,
) -> Option<u64> {
    if let Some(class_name) = &rule.class {
        return Some(
            response
                .summary
                .classes
                .iter()
                .filter(|class| class.name == *class_name)
                .map(value_of)
                .sum(),
        );
    }

    let pattern = rule.class_pattern.as_deref()?;
    let regex = Regex::new(pattern).ok()?;
    Some(
        response
            .summary
            .classes
            .iter()
            .filter(|class| regex.is_match(&class.name))
            .map(value_of)
            .sum(),
    )
}

fn parse_gc_root_kind(kind: &str) -> Option<GcRootKind> {
    match normalize_kind(kind).as_str() {
        "jni_global" => Some(GcRootKind::JniGlobal),
        "jni_local" => Some(GcRootKind::JniLocal),
        "java_frame" => Some(GcRootKind::JavaFrame),
        "native_stack" => Some(GcRootKind::NativeStack),
        "sticky_class" => Some(GcRootKind::StickyClass),
        "thread_block" => Some(GcRootKind::ThreadBlock),
        "monitor_used" => Some(GcRootKind::MonitorUsed),
        "thread_object" => Some(GcRootKind::ThreadObject),
        "other" | "unknown" => Some(GcRootKind::Unknown),
        _ => None,
    }
}

fn parse_provenance_kind(kind: &str) -> Option<ProvenanceKind> {
    match normalize_kind(kind).as_str() {
        "synthetic" => Some(ProvenanceKind::Synthetic),
        "partial" => Some(ProvenanceKind::Partial),
        "fallback" => Some(ProvenanceKind::Fallback),
        "placeholder" => Some(ProvenanceKind::Placeholder),
        _ => None,
    }
}

fn normalize_kind(kind: &str) -> String {
    kind.trim().to_ascii_lowercase().replace('-', "_")
}

fn provenance_kind_name(kind: ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::Synthetic => "synthetic",
        ProvenanceKind::Partial => "partial",
        ProvenanceKind::Fallback => "fallback",
        ProvenanceKind::Placeholder => "placeholder",
    }
}

fn has_provenance_kind(markers: &[crate::ProvenanceMarker], kind: ProvenanceKind) -> bool {
    markers.iter().any(|marker| marker.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::super::PolicyDefaults;
    use super::*;
    use crate::hprof::{ClassStat, RecordStat};
    use crate::{
        AnalyzeResponse, Comparison, GcRootKind, GraphMetrics, HeapSummary, ModeRequirement,
        OverviewClassStat, OverviewClassStats, OverviewOptions, OverviewSummary, Policy,
        PolicyRule, Predicate, ProvenanceKind, ProvenanceMarker, Severity,
    };
    use serde_json::json;
    use std::{collections::HashMap, time::Duration, time::SystemTime};

    fn policy_with_rule(rule: PolicyRule) -> Policy {
        Policy {
            meta: None,
            defaults: PolicyDefaults::default(),
            rules: vec![rule],
        }
    }

    fn numeric_rule(
        id: &str,
        predicate: Predicate,
        comparison: Comparison,
        threshold: u64,
    ) -> PolicyRule {
        PolicyRule {
            id: id.to_string(),
            predicate,
            comparison,
            threshold,
            severity: Severity::Error,
            remediation_hint: None,
            mode_requirement: ModeRequirement::Any,
            class: None,
            class_pattern: None,
            kind: None,
            severity_filter: None,
            scope: None,
        }
    }

    fn overview_class_stat(name: &str, instances: u64, bytes: u64) -> OverviewClassStat {
        OverviewClassStat {
            class_id: hash_class_id(name),
            class_name: name.to_string(),
            instance_count: instances,
            approx_shallow_bytes: bytes,
        }
    }

    fn hash_class_id(name: &str) -> u64 {
        name.as_bytes().iter().fold(0_u64, |acc, byte| {
            acc.wrapping_mul(131).wrapping_add(u64::from(*byte))
        })
    }

    fn overview_summary(
        total_size_bytes: u64,
        total_instances: u64,
        loaded_class_count: u64,
        class_entries: Vec<OverviewClassStat>,
    ) -> OverviewSummary {
        OverviewSummary {
            heap_path: "heap.hprof".into(),
            total_bytes_processed: total_size_bytes,
            total_size_bytes,
            total_record_count: 1,
            total_instances,
            loaded_class_count,
            class_stats: OverviewClassStats {
                entries: class_entries,
                truncated: false,
            },
            top_instances: Vec::new(),
            gc_root_counts: HashMap::new(),
            thread_frames: Vec::new(),
            truncated: false,
            options: OverviewOptions::default(),
            provenance: Vec::new(),
        }
    }

    fn class_stat(name: &str, instances: u64, bytes: u64) -> ClassStat {
        ClassStat {
            name: name.to_string(),
            instances,
            total_size_bytes: bytes,
            percentage: 0.0,
        }
    }

    fn deep_response(
        total_size_bytes: u64,
        total_objects: u64,
        classes: Vec<ClassStat>,
        provenance: Vec<ProvenanceMarker>,
    ) -> AnalyzeResponse {
        AnalyzeResponse {
            mode: AnalysisMode::Deep,
            overview: None,
            summary: HeapSummary {
                heap_path: "heap.hprof".into(),
                total_objects,
                total_size_bytes,
                classes,
                generated_at: SystemTime::UNIX_EPOCH,
                header: None,
                total_records: 1,
                record_stats: vec![RecordStat {
                    tag: 0x21,
                    name: "INSTANCE_DUMP".into(),
                    count: 1,
                    bytes: total_size_bytes,
                }],
            },
            leaks: Vec::new(),
            recommendations: Vec::new(),
            elapsed: Duration::from_secs(0),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            classloader_report: None,
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance,
        }
    }

    #[test]
    fn evaluate_total_bytes_pass_when_below_threshold() {
        let overview = overview_summary(1024 * 1024, 10, 3, Vec::new());
        let policy = policy_with_rule(numeric_rule(
            "total-heap-budget",
            Predicate::TotalBytes,
            Comparison::Lt,
            2 * 1024 * 1024,
        ));

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations.len(), 1);
        assert!(result.evaluations[0].passed);
        assert_eq!(result.evaluations[0].actual, json!(1024 * 1024_u64));
    }

    #[test]
    fn evaluate_total_bytes_fail_when_above_threshold() {
        let overview = overview_summary(5 * 1024 * 1024, 10, 3, Vec::new());
        let policy = policy_with_rule(numeric_rule(
            "total-heap-budget",
            Predicate::TotalBytes,
            Comparison::Lt,
            2 * 1024 * 1024,
        ));

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert_eq!(result.evaluations.len(), 1);
        assert!(!result.evaluations[0].passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule_id, "total-heap-budget");
        assert_eq!(result.violations[0].predicate, "total_bytes");
        assert_eq!(result.violations[0].actual, json!(5 * 1024 * 1024_u64));
        assert_eq!(result.violations[0].expected, json!(2 * 1024 * 1024_u64));
        assert_eq!(result.violations[0].severity, Severity::Error);
    }

    #[test]
    fn evaluate_total_instances_overview() {
        let overview = overview_summary(1024, 42, 3, Vec::new());
        let policy = policy_with_rule(numeric_rule(
            "instance-count-budget",
            Predicate::TotalInstances,
            Comparison::Eq,
            42,
        ));

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(42_u64));
    }

    #[test]
    fn evaluate_class_instances_pattern_aggregates() {
        let overview = overview_summary(
            4096,
            16,
            4,
            vec![
                overview_class_stat("java.util.HashMap", 3, 300),
                overview_class_stat("java.util.HashMap$Node", 4, 120),
                overview_class_stat("java.lang.String", 9, 500),
            ],
        );
        let mut rule = numeric_rule(
            "hashmap-instance-cap",
            Predicate::ClassInstances,
            Comparison::Eq,
            7,
        );
        rule.class_pattern = Some("^java\\.util\\.HashMap.*".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(7_u64));
    }

    #[test]
    fn evaluate_class_bytes_exact_name() {
        let overview = overview_summary(
            4096,
            16,
            4,
            vec![
                overview_class_stat("java.lang.String", 8, 1024),
                overview_class_stat("byte[]", 4, 512),
            ],
        );
        let mut rule = numeric_rule(
            "string-bytes-cap",
            Predicate::ClassBytes,
            Comparison::Eq,
            1024,
        );
        rule.class = Some("java.lang.String".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(1024_u64));
    }

    #[test]
    fn evaluate_loaded_class_count() {
        let overview = overview_summary(4096, 16, 12, Vec::new());
        let policy = policy_with_rule(numeric_rule(
            "loaded-class-ceiling",
            Predicate::LoadedClassCount,
            Comparison::Eq,
            12,
        ));

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(12_u64));
    }

    #[test]
    fn evaluate_gc_root_count_jni_global() {
        let mut overview = overview_summary(4096, 16, 12, Vec::new());
        overview.gc_root_counts.insert(GcRootKind::JniGlobal, 3);
        let mut rule = numeric_rule(
            "jni-global-roots",
            Predicate::GcRootCount,
            Comparison::Eq,
            3,
        );
        rule.kind = Some("jni_global".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(3_u64));
    }

    #[test]
    fn evaluate_provenance_must_not_contain_synthetic_passes_when_absent() {
        let overview = overview_summary(4096, 16, 12, Vec::new());
        let mut rule = numeric_rule(
            "no-synthetic-provenance",
            Predicate::ProvenanceMustNotContain,
            Comparison::Eq,
            0,
        );
        rule.kind = Some("synthetic".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert!(result.violations.is_empty());
        assert!(result.evaluations[0].passed);
        assert_eq!(result.evaluations[0].actual, json!(false));
    }

    #[test]
    fn evaluate_provenance_must_not_contain_synthetic_fails_when_present() {
        let mut overview = overview_summary(4096, 16, 12, Vec::new());
        overview
            .provenance
            .push(ProvenanceMarker::bare(ProvenanceKind::Synthetic));
        let mut rule = numeric_rule(
            "no-synthetic-provenance",
            Predicate::ProvenanceMustNotContain,
            Comparison::Eq,
            0,
        );
        rule.kind = Some("synthetic".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert_eq!(result.violations.len(), 1);
        assert_eq!(
            result.violations[0].predicate,
            "provenance_must_not_contain"
        );
        assert_eq!(result.violations[0].actual, json!(true));
        assert_eq!(result.violations[0].expected, json!("synthetic"));
    }

    #[test]
    fn evaluate_deep_only_predicate_skipped_in_auto_resolved_overview() {
        let overview = overview_summary(4096, 16, 12, Vec::new());
        let mut rule = numeric_rule("no-critical-leaks", Predicate::LeakCount, Comparison::Eq, 0);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Auto,
        );

        assert!(result.violations.is_empty());
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].rule_id, "no-critical-leaks");
        assert_eq!(
            result.skipped[0].reason,
            super::super::SkipReason::DeepOnlyInOverviewMode
        );
    }

    #[test]
    fn evaluate_deep_only_predicate_violation_in_explicit_overview_mode() {
        let overview = overview_summary(4096, 16, 12, Vec::new());
        let mut rule = numeric_rule("no-critical-leaks", Predicate::LeakCount, Comparison::Eq, 0);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].severity, Severity::Critical);
        assert_eq!(
            result.violations[0].message,
            "deep-only predicate `no-critical-leaks` cannot run in explicit overview mode"
        );
    }

    #[test]
    fn evaluate_severity_levels_propagate_into_violations() {
        let overview = overview_summary(5 * 1024 * 1024, 16, 12, Vec::new());
        let mut rule = numeric_rule(
            "total-heap-budget",
            Predicate::TotalBytes,
            Comparison::Lt,
            2 * 1024 * 1024,
        );
        rule.severity = Severity::Warning;
        let policy = policy_with_rule(rule);

        let result = evaluate(
            &policy,
            &PolicyInput::Overview(&overview),
            AnalysisMode::Overview,
        );

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].severity, Severity::Warning);
    }

    #[test]
    fn evaluate_handles_deep_input_for_overview_compatible_predicates() {
        let response = deep_response(4096, 128, vec![class_stat("byte[]", 4, 2048)], Vec::new());
        let policy = policy_with_rule(numeric_rule(
            "total-heap-budget",
            Predicate::TotalBytes,
            Comparison::Eq,
            4096,
        ));

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.mode_used, AnalysisMode::Deep);
        assert_eq!(result.evaluations[0].actual, json!(4096_u64));
    }
}
