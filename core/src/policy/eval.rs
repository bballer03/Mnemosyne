use crate::{analysis::LeakSeverity, AnalysisMode, GcRootKind, ProvenanceKind};
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
        if matches!(input, PolicyInput::Overview(_)) && rule_requires_deep(rule) {
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
                PolicyInput::Deep(response) => aggregate_deep_class_value(
                    rule,
                    response,
                    |class| class.instances,
                    |entry| entry.instance_count,
                ),
                PolicyInput::Overview(summary) => {
                    aggregate_overview_class_value(rule, summary, |class| class.instance_count)
                }
            }?;
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::ClassBytes => {
            let actual = match input {
                PolicyInput::Deep(response) => aggregate_deep_class_value(
                    rule,
                    response,
                    |class| class.total_size_bytes,
                    |entry| entry.shallow_size,
                ),
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
        Predicate::LeakCount => {
            let PolicyInput::Deep(response) = input else {
                return None;
            };
            let filter = match parse_leak_severity_filter(rule.severity_filter.as_deref()) {
                Ok(filter) => filter,
                Err(message) => {
                    return Some(invalid_rule_outcome(
                        json!(rule.severity_filter.as_deref().unwrap_or_default()),
                        json!(severity_filter_documentation()),
                        message,
                    ));
                }
            };

            let actual = response
                .leaks
                .iter()
                .filter(|leak| filter.matches(leak.severity))
                .count() as u64;
            Some(evaluate_numeric(rule, actual))
        }
        Predicate::RetainedSize => {
            let PolicyInput::Deep(response) = input else {
                return None;
            };

            let actual = match parse_retained_size_scope(rule.scope.as_deref()) {
                Some(RetainedSizeScope::Class) => match retained_size_for_class(rule, response) {
                    Ok(actual) => actual,
                    Err(outcome) => return Some(outcome),
                },
                Some(RetainedSizeScope::LeakSuspect) => {
                    match retained_size_for_leak_suspect(rule, response) {
                        Ok(actual) => actual,
                        Err(outcome) => return Some(outcome),
                    }
                }
                None => {
                    return Some(invalid_rule_outcome(
                        json!(rule.scope.as_deref().unwrap_or_default()),
                        json!(["class", "leak_suspect"]),
                        format!(
                            "retained_size rule '{}' requires scope 'class' or 'leak_suspect'",
                            rule.id
                        ),
                    ));
                }
            };

            Some(evaluate_numeric(rule, actual))
        }
        Predicate::DominatorRootCount => {
            let PolicyInput::Deep(response) = input else {
                return None;
            };

            let actual = response
                .graph
                .dominators
                .iter()
                .filter(|node| node.immediate_dominator.is_none())
                .count() as u64;
            Some(evaluate_numeric(rule, actual))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeakSeverityFilter {
    All,
    Exact(LeakSeverity),
    AtLeast(LeakSeverity),
}

impl LeakSeverityFilter {
    fn matches(self, severity: LeakSeverity) -> bool {
        match self {
            Self::All => true,
            Self::Exact(expected) => severity == expected,
            Self::AtLeast(minimum) => severity >= minimum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetainedSizeScope {
    Class,
    LeakSuspect,
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

fn invalid_rule_outcome(actual: Value, expected: Value, message: String) -> RuleOutcome {
    RuleOutcome {
        passed: false,
        actual,
        expected,
        message,
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

fn rule_requires_deep(rule: &super::PolicyRule) -> bool {
    matches!(rule.mode_requirement, ModeRequirement::DeepOnly)
        || matches!(
            rule.predicate,
            Predicate::LeakCount | Predicate::RetainedSize | Predicate::DominatorRootCount
        )
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
    summary_value_of: impl Fn(&crate::hprof::ClassStat) -> u64 + Copy,
    histogram_value_of: impl Fn(&crate::HistogramEntry) -> u64 + Copy,
) -> Option<u64> {
    if let Some(histogram) = response
        .histogram
        .as_ref()
        .filter(|histogram| histogram.group_by == crate::HistogramGroupBy::Class)
    {
        return aggregate_histogram_class_value(rule, histogram, histogram_value_of);
    }

    aggregate_summary_class_value(rule, &response.summary.classes, summary_value_of)
}

fn aggregate_histogram_class_value(
    rule: &super::PolicyRule,
    histogram: &crate::HistogramResult,
    value_of: impl Fn(&crate::HistogramEntry) -> u64 + Copy,
) -> Option<u64> {
    if let Some(class_name) = &rule.class {
        return Some(
            histogram
                .entries
                .iter()
                .filter(|entry| entry.key == *class_name)
                .map(value_of)
                .sum(),
        );
    }

    let pattern = rule.class_pattern.as_deref()?;
    let regex = Regex::new(pattern).ok()?;
    Some(
        histogram
            .entries
            .iter()
            .filter(|entry| regex.is_match(&entry.key))
            .map(value_of)
            .sum(),
    )
}

fn aggregate_summary_class_value(
    rule: &super::PolicyRule,
    classes: &[crate::hprof::ClassStat],
    value_of: impl Fn(&crate::hprof::ClassStat) -> u64 + Copy,
) -> Option<u64> {
    if let Some(class_name) = &rule.class {
        return Some(
            classes
                .iter()
                .filter(|class| class.name == *class_name)
                .map(value_of)
                .sum(),
        );
    }

    let pattern = rule.class_pattern.as_deref()?;
    let regex = Regex::new(pattern).ok()?;
    Some(
        classes
            .iter()
            .filter(|class| regex.is_match(&class.name))
            .map(value_of)
            .sum(),
    )
}

fn parse_leak_severity_filter(value: Option<&str>) -> Result<LeakSeverityFilter, String> {
    let normalized = value
        .map(normalize_kind)
        .unwrap_or_else(|| String::from("all"));

    if normalized.is_empty() || normalized == "all" {
        return Ok(LeakSeverityFilter::All);
    }

    if let Some(exact) = normalized.strip_suffix("_only") {
        return parse_base_leak_severity(exact)
            .map(LeakSeverityFilter::Exact)
            .ok_or_else(|| invalid_severity_filter_message(value.unwrap_or_default()));
    }

    if let Some(threshold) = normalized.strip_suffix("_or_above") {
        return parse_base_leak_severity(threshold)
            .map(LeakSeverityFilter::AtLeast)
            .ok_or_else(|| invalid_severity_filter_message(value.unwrap_or_default()));
    }

    parse_base_leak_severity(&normalized)
        .map(LeakSeverityFilter::Exact)
        .ok_or_else(|| invalid_severity_filter_message(value.unwrap_or_default()))
}

fn parse_base_leak_severity(value: &str) -> Option<LeakSeverity> {
    match value {
        "low" => Some(LeakSeverity::Low),
        "medium" => Some(LeakSeverity::Medium),
        "high" => Some(LeakSeverity::High),
        "critical" => Some(LeakSeverity::Critical),
        _ => None,
    }
}

fn invalid_severity_filter_message(value: &str) -> String {
    format!(
        "invalid severity_filter '{value}'; expected one of {}",
        severity_filter_documentation().join(", ")
    )
}

fn severity_filter_documentation() -> [&'static str; 14] {
    [
        "all",
        "low",
        "medium",
        "high",
        "critical",
        "low_only",
        "medium_only",
        "high_only",
        "critical_only",
        "low_or_above",
        "medium_or_above",
        "high_or_above",
        "critical_or_above",
        "",
    ]
}

fn parse_retained_size_scope(scope: Option<&str>) -> Option<RetainedSizeScope> {
    match scope.map(normalize_kind).as_deref()? {
        "class" => Some(RetainedSizeScope::Class),
        "leak_suspect" => Some(RetainedSizeScope::LeakSuspect),
        _ => None,
    }
}

fn retained_size_for_class(
    rule: &super::PolicyRule,
    response: &crate::AnalyzeResponse,
) -> Result<u64, RuleOutcome> {
    if let Some(class_name) = &rule.class {
        return Ok(response
            .leaks
            .iter()
            .filter(|leak| leak.class_name == *class_name)
            .map(|leak| leak.retained_size_bytes)
            .sum());
    }

    if let Some(pattern) = rule.class_pattern.as_deref() {
        let regex = Regex::new(pattern).map_err(|err| {
            invalid_rule_outcome(
                json!(pattern),
                json!("valid class regex"),
                format!("invalid class_pattern regex for rule '{}': {err}", rule.id),
            )
        })?;
        return Ok(response
            .leaks
            .iter()
            .filter(|leak| regex.is_match(&leak.class_name))
            .map(|leak| leak.retained_size_bytes)
            .sum());
    }

    Err(invalid_rule_outcome(
        Value::Null,
        json!(["class", "class_pattern"]),
        format!(
            "retained_size rule '{}' with scope=class requires class or class_pattern",
            rule.id
        ),
    ))
}

fn retained_size_for_leak_suspect(
    rule: &super::PolicyRule,
    response: &crate::AnalyzeResponse,
) -> Result<u64, RuleOutcome> {
    if let Some(leak_id) = &rule.leak_id {
        return Ok(response
            .leaks
            .iter()
            .find(|leak| leak.id == *leak_id)
            .map(|leak| leak.retained_size_bytes)
            .unwrap_or_default());
    }

    if let Some(class_name) = &rule.class {
        return Ok(response
            .leaks
            .iter()
            .filter(|leak| leak.class_name == *class_name)
            .map(|leak| leak.retained_size_bytes)
            .max()
            .unwrap_or_default());
    }

    if let Some(pattern) = rule.class_pattern.as_deref() {
        let regex = Regex::new(pattern).map_err(|err| {
            invalid_rule_outcome(
                json!(pattern),
                json!("valid class regex"),
                format!("invalid class_pattern regex for rule '{}': {err}", rule.id),
            )
        })?;
        return Ok(response
            .leaks
            .iter()
            .filter(|leak| regex.is_match(&leak.class_name))
            .map(|leak| leak.retained_size_bytes)
            .max()
            .unwrap_or_default());
    }

    Err(invalid_rule_outcome(
        Value::Null,
        json!(["leak_id", "class", "class_pattern"]),
        format!(
            "retained_size rule '{}' with scope=leak_suspect requires leak_id, class, or class_pattern",
            rule.id
        ),
    ))
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
    use crate::analysis::{LeakInsight, LeakKind, LeakSeverity};
    use crate::hprof::{ClassStat, RecordStat};
    use crate::{
        AnalyzeResponse, Comparison, DominatorNode, GcRootKind, GraphMetrics, HeapSummary,
        HistogramEntry, HistogramGroupBy, HistogramResult, ModeRequirement, OverviewClassStat,
        OverviewClassStats, OverviewOptions, OverviewSummary, Policy, PolicyRule, Predicate,
        ProvenanceKind, ProvenanceMarker, Severity,
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
            leak_id: None,
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
        deep_response_with_details(
            total_size_bytes,
            total_objects,
            classes,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            provenance,
        )
    }

    fn deep_response_with_details(
        total_size_bytes: u64,
        total_objects: u64,
        classes: Vec<ClassStat>,
        histogram_entries: Vec<HistogramEntry>,
        leaks: Vec<LeakInsight>,
        dominators: Vec<DominatorNode>,
        provenance: Vec<ProvenanceMarker>,
    ) -> AnalyzeResponse {
        let histogram = (!histogram_entries.is_empty()).then(|| HistogramResult {
            group_by: HistogramGroupBy::Class,
            total_instances: histogram_entries
                .iter()
                .map(|entry| entry.instance_count)
                .sum(),
            total_shallow_size: histogram_entries
                .iter()
                .map(|entry| entry.shallow_size)
                .sum(),
            entries: histogram_entries,
        });

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
            leaks,
            recommendations: Vec::new(),
            elapsed: Duration::from_secs(0),
            graph: GraphMetrics {
                node_count: dominators.len(),
                edge_count: 0,
                dominators,
            },
            ai: None,
            histogram,
            unreachable: None,
            thread_report: None,
            classloader_report: None,
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance,
        }
    }

    fn histogram_entry(
        key: &str,
        instance_count: u64,
        shallow_size: u64,
        retained_size: u64,
    ) -> HistogramEntry {
        HistogramEntry {
            key: key.to_string(),
            instance_count,
            shallow_size,
            retained_size,
        }
    }

    fn deep_leak(
        id: &str,
        class_name: &str,
        severity: LeakSeverity,
        retained_size: u64,
    ) -> LeakInsight {
        LeakInsight {
            id: id.to_string(),
            class_name: class_name.to_string(),
            leak_kind: LeakKind::Unknown,
            severity,
            retained_size_bytes: retained_size,
            shallow_size_bytes: Some(retained_size / 2),
            suspect_score: None,
            instances: 1,
            description: format!("{class_name} retains {retained_size} bytes"),
            provenance: Vec::new(),
        }
    }

    fn dominator_node(object_id: &str, immediate_dominator: Option<&str>) -> DominatorNode {
        DominatorNode {
            name: object_id.to_string(),
            class_name: object_id.to_string(),
            object_id: object_id.to_string(),
            dominates: 0,
            immediate_dominator: immediate_dominator.map(str::to_string),
            retained_size: 0,
            shallow_size: 0,
        }
    }

    fn policy_from_toml(toml: &str) -> Policy {
        Policy::from_toml_str(toml).expect("policy TOML should parse")
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

    #[test]
    fn evaluate_leak_count_no_filter_pass() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak("leak-1", "com.example.Cache", LeakSeverity::High, 1024),
                deep_leak("leak-2", "com.example.Buffer", LeakSeverity::Medium, 2048),
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule("leak-count-budget", Predicate::LeakCount, Comparison::Lt, 5);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert!(result.skipped.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(2_u64));
    }

    #[test]
    fn evaluate_leak_count_no_filter_fail() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            (0..10)
                .map(|index| {
                    deep_leak(
                        &format!("leak-{index}"),
                        "com.example.Cache",
                        LeakSeverity::High,
                        1024,
                    )
                })
                .collect(),
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule("leak-count-budget", Predicate::LeakCount, Comparison::Lt, 5);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].actual, json!(10_u64));
    }

    #[test]
    fn evaluate_leak_count_severity_filter_critical_only() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak(
                    "critical-1",
                    "com.example.Cache",
                    LeakSeverity::Critical,
                    8192,
                ),
                deep_leak("high-1", "com.example.Cache", LeakSeverity::High, 4096),
                deep_leak("high-2", "com.example.Cache", LeakSeverity::High, 4096),
                deep_leak("high-3", "com.example.Cache", LeakSeverity::High, 4096),
                deep_leak("high-4", "com.example.Cache", LeakSeverity::High, 4096),
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule("no-critical-leaks", Predicate::LeakCount, Comparison::Eq, 0);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.severity_filter = Some("critical_only".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].actual, json!(1_u64));
    }

    #[test]
    fn evaluate_leak_count_severity_filter_high_or_above() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak(
                    "critical-1",
                    "com.example.Cache",
                    LeakSeverity::Critical,
                    8192,
                ),
                deep_leak("high-1", "com.example.Cache", LeakSeverity::High, 4096),
                deep_leak("high-2", "com.example.Cache", LeakSeverity::High, 4096),
                deep_leak("medium-1", "com.example.Cache", LeakSeverity::Medium, 2048),
                deep_leak("medium-2", "com.example.Cache", LeakSeverity::Medium, 2048),
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "high-or-above-leaks",
            Predicate::LeakCount,
            Comparison::Lte,
            2,
        );
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.severity_filter = Some("high_or_above".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].actual, json!(3_u64));
    }

    #[test]
    fn evaluate_leak_count_severity_filter_invalid_returns_violation_or_skip() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![deep_leak(
                "critical-1",
                "com.example.Cache",
                LeakSeverity::Critical,
                8192,
            )],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule("invalid-filter", Predicate::LeakCount, Comparison::Eq, 0);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.severity_filter = Some("bogus".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(
            !result.violations.is_empty() || !result.skipped.is_empty(),
            "invalid severity filters must not silently pass"
        );
    }

    #[test]
    fn evaluate_retained_size_per_class_exact_name() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak("leak-1", "com.example.Cache", LeakSeverity::High, 2048),
                deep_leak("leak-2", "com.example.Cache", LeakSeverity::High, 1024),
                deep_leak("leak-3", "com.example.Other", LeakSeverity::Medium, 512),
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "cache-retained-size",
            Predicate::RetainedSize,
            Comparison::Eq,
            3072,
        );
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.scope = Some("class".into());
        rule.class = Some("com.example.Cache".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(3072_u64));
    }

    #[test]
    fn evaluate_retained_size_per_class_pattern_aggregates() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak(
                    "leak-1",
                    "com.example.cache.Primary",
                    LeakSeverity::High,
                    2048,
                ),
                deep_leak(
                    "leak-2",
                    "com.example.cache.Secondary",
                    LeakSeverity::High,
                    1024,
                ),
                deep_leak("leak-3", "com.example.Other", LeakSeverity::Medium, 512),
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "cache-pattern-retained-size",
            Predicate::RetainedSize,
            Comparison::Eq,
            3072,
        );
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.scope = Some("class".into());
        rule.class_pattern = Some("^com\\.example\\.cache\\..*$".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(3072_u64));
    }

    #[test]
    fn evaluate_retained_size_per_leak_suspect_id() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![
                deep_leak("leak-1", "com.example.Cache", LeakSeverity::High, 2048),
                deep_leak("leak-2", "com.example.Cache", LeakSeverity::Critical, 8192),
            ],
            Vec::new(),
            Vec::new(),
        );
        let policy = policy_from_toml(
            r#"[[rule]]
id = "suspect-retained-size"
predicate = "retained_size"
op = "=="
value = 8192
severity = "error"
mode_requirement = "deep_only"
scope = "leak_suspect"
leak_id = "leak-2"
"#,
        );

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(8192_u64));
    }

    #[test]
    fn evaluate_retained_size_missing_class_returns_zero_actual_passes_when_threshold_positive() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![deep_leak(
                "leak-1",
                "com.example.Cache",
                LeakSeverity::High,
                2048,
            )],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "missing-class-retained-size",
            Predicate::RetainedSize,
            Comparison::Lt,
            100 * 1024 * 1024,
        );
        rule.mode_requirement = ModeRequirement::DeepOnly;
        rule.scope = Some("class".into());
        rule.class = Some("com.example.Missing".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(0_u64));
    }

    #[test]
    fn evaluate_dominator_root_count() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![
                dominator_node("0x1", None),
                dominator_node("0x2", Some("0x1")),
                dominator_node("0x3", None),
            ],
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "dominator-root-cap",
            Predicate::DominatorRootCount,
            Comparison::Eq,
            2,
        );
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(2_u64));
    }

    #[test]
    fn evaluate_class_instances_deep_path_uses_deep_histogram() {
        let response = deep_response_with_details(
            4096,
            128,
            vec![class_stat("byte[]", 4, 2048)],
            vec![histogram_entry("byte[]", 7, 700, 1700)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "byte-array-instance-cap",
            Predicate::ClassInstances,
            Comparison::Eq,
            7,
        );
        rule.class = Some("byte[]".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(7_u64));
    }

    #[test]
    fn evaluate_class_bytes_deep_path_uses_deep_histogram_shallow() {
        let response = deep_response_with_details(
            4096,
            128,
            vec![class_stat("byte[]", 4, 2048)],
            vec![histogram_entry("byte[]", 7, 300, 1700)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule(
            "byte-array-bytes",
            Predicate::ClassBytes,
            Comparison::Eq,
            300,
        );
        rule.class = Some("byte[]".into());
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert_eq!(result.evaluations[0].actual, json!(300_u64));
    }

    #[test]
    fn evaluate_deep_only_predicate_runs_on_deep_input() {
        let response = deep_response_with_details(
            4096,
            128,
            Vec::new(),
            Vec::new(),
            vec![deep_leak(
                "critical-1",
                "com.example.Cache",
                LeakSeverity::Critical,
                8192,
            )],
            Vec::new(),
            Vec::new(),
        );
        let mut rule = numeric_rule("no-critical-leaks", Predicate::LeakCount, Comparison::Eq, 1);
        rule.mode_requirement = ModeRequirement::DeepOnly;
        let policy = policy_with_rule(rule);

        let result = evaluate(&policy, &PolicyInput::Deep(&response), AnalysisMode::Deep);

        assert!(result.violations.is_empty());
        assert!(result.skipped.is_empty());
        assert_eq!(result.evaluations.len(), 1);
        assert_eq!(result.evaluations[0].actual, json!(1_u64));
    }

    #[test]
    fn severity_filter_parser_accepts_all_documented_forms() {
        let cases = [
            (None, LeakSeverityFilter::All),
            (Some(""), LeakSeverityFilter::All),
            (Some("all"), LeakSeverityFilter::All),
            (Some("low"), LeakSeverityFilter::Exact(LeakSeverity::Low)),
            (
                Some("medium"),
                LeakSeverityFilter::Exact(LeakSeverity::Medium),
            ),
            (Some("high"), LeakSeverityFilter::Exact(LeakSeverity::High)),
            (
                Some("critical"),
                LeakSeverityFilter::Exact(LeakSeverity::Critical),
            ),
            (
                Some("low_only"),
                LeakSeverityFilter::Exact(LeakSeverity::Low),
            ),
            (
                Some("medium_only"),
                LeakSeverityFilter::Exact(LeakSeverity::Medium),
            ),
            (
                Some("high_only"),
                LeakSeverityFilter::Exact(LeakSeverity::High),
            ),
            (
                Some("critical_only"),
                LeakSeverityFilter::Exact(LeakSeverity::Critical),
            ),
            (
                Some("low_or_above"),
                LeakSeverityFilter::AtLeast(LeakSeverity::Low),
            ),
            (
                Some("medium_or_above"),
                LeakSeverityFilter::AtLeast(LeakSeverity::Medium),
            ),
            (
                Some("high_or_above"),
                LeakSeverityFilter::AtLeast(LeakSeverity::High),
            ),
            (
                Some("critical_or_above"),
                LeakSeverityFilter::AtLeast(LeakSeverity::Critical),
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(parse_leak_severity_filter(input).unwrap(), expected);
        }
    }

    #[test]
    fn severity_filter_parser_rejects_unknown() {
        let error = parse_leak_severity_filter(Some("bogus")).unwrap_err();

        assert!(error.contains("invalid severity_filter 'bogus'"));
    }
}
