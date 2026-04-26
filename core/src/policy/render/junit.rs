use std::fmt::Write;

use crate::{Comparison, Predicate, Severity, SkipReason, Violation};

use super::PolicyRenderContext;

pub fn render_junit_report(context: &PolicyRenderContext<'_>) -> String {
    let mut output = String::new();
    let tests = context.policy.rules.len();
    let failures = context
        .result
        .violations
        .iter()
        .filter(|violation| !is_explicit_overview_mode_mismatch(violation))
        .count();
    let errors = context
        .result
        .violations
        .iter()
        .filter(|violation| is_explicit_overview_mode_mismatch(violation))
        .count();
    let skipped = context.result.skipped.len();

    writeln!(&mut output, "<testsuites>").expect("write to string should succeed");
    writeln!(
        &mut output,
        "  <testsuite name=\"mnemosyne-ci-check\" tests=\"{tests}\" failures=\"{failures}\" errors=\"{errors}\" skipped=\"{skipped}\">"
    )
    .expect("write to string should succeed");

    for rule in &context.policy.rules {
        let classname = format!("mnemosyne.policy.{}", predicate_name(rule.predicate));
        let testcase_prefix = format!(
            "    <testcase name=\"{}\" classname=\"{}\"",
            escape_xml_attr(&rule.id),
            escape_xml_attr(&classname)
        );

        if let Some(skipped_rule) = context
            .result
            .skipped
            .iter()
            .find(|item| item.rule_id == rule.id)
        {
            writeln!(&mut output, "{testcase_prefix}>").expect("write to string should succeed");
            writeln!(
                &mut output,
                "      <skipped message=\"{}\"/>",
                escape_xml_attr(format_skip_reason(&skipped_rule.reason))
            )
            .expect("write to string should succeed");
            writeln!(&mut output, "    </testcase>").expect("write to string should succeed");
            continue;
        }

        if let Some(violation) = context
            .result
            .violations
            .iter()
            .find(|item| item.rule_id == rule.id)
        {
            let element_name = if is_explicit_overview_mode_mismatch(violation) {
                "error"
            } else {
                "failure"
            };

            writeln!(&mut output, "{testcase_prefix}>").expect("write to string should succeed");
            write!(
                &mut output,
                "      <{element_name} type=\"{}\" message=\"{}\">",
                escape_xml_attr(severity_name(violation.severity)),
                escape_xml_attr(&violation.message)
            )
            .expect("write to string should succeed");
            write_cdata(&mut output, &detail_text(violation));
            writeln!(&mut output, "</{element_name}>").expect("write to string should succeed");
            writeln!(&mut output, "    </testcase>").expect("write to string should succeed");
            continue;
        }

        writeln!(&mut output, "{testcase_prefix} />").expect("write to string should succeed");
    }

    writeln!(&mut output, "  </testsuite>").expect("write to string should succeed");
    writeln!(&mut output, "</testsuites>").expect("write to string should succeed");
    output
}

fn detail_text(violation: &Violation) -> String {
    let mut detail = format!(
        "actual: {}\nexpected: {} {}",
        format_value(&violation.actual),
        comparison_symbol(violation.comparison),
        format_value(&violation.expected)
    );

    if let Some(remediation_hint) = violation.remediation_hint.as_deref() {
        detail.push_str("\nremediation: ");
        detail.push_str(remediation_hint);
    }

    detail
}

fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
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

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Critical => "critical",
    }
}

fn format_skip_reason(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::DeepOnlyInOverviewMode => {
            "deep-only predicate skipped because auto mode resolved to overview"
        }
        SkipReason::UnsupportedInThisMode => "predicate is unsupported in this analysis mode",
    }
}

fn is_explicit_overview_mode_mismatch(violation: &Violation) -> bool {
    violation.severity == Severity::Critical
        && violation.actual.is_null()
        && violation.expected.is_null()
        && violation
            .message
            .contains("cannot run in explicit overview mode")
}

fn escape_xml_attr(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn write_cdata(output: &mut String, input: &str) {
    output.push_str("<![CDATA[");
    output.push_str(&input.replace("]]>", "]]]]><![CDATA[>"));
    output.push_str("]]>");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::render_junit_report;
    use crate::policy::PolicyDefaults;
    use crate::{
        Comparison, Evaluation, ModeRequirement, Policy, PolicyResult, PolicyRule, Predicate,
        Severity, SkipReason, SkippedRule, Violation,
    };

    use super::super::PolicyRenderContext;

    fn policy(rules: Vec<PolicyRule>) -> Policy {
        Policy {
            meta: None,
            defaults: PolicyDefaults::default(),
            rules,
        }
    }

    fn rule(id: &str, predicate: Predicate) -> PolicyRule {
        PolicyRule {
            id: id.to_string(),
            predicate,
            comparison: Comparison::Lte,
            threshold: 10,
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

    fn result(
        violations: Vec<Violation>,
        skipped: Vec<SkippedRule>,
        evaluations: Vec<Evaluation>,
    ) -> PolicyResult {
        PolicyResult {
            mode_used: crate::AnalysisMode::Overview,
            mode_requested: crate::AnalysisMode::Overview,
            violations,
            evaluations,
            skipped,
        }
    }

    fn evaluation(rule_id: &str, passed: bool, actual: serde_json::Value) -> Evaluation {
        Evaluation {
            rule_id: rule_id.to_string(),
            passed,
            actual,
        }
    }

    fn threshold_violation(
        rule_id: &str,
        predicate: &str,
        severity: Severity,
        message: &str,
        remediation_hint: Option<&str>,
    ) -> Violation {
        Violation {
            rule_id: rule_id.to_string(),
            predicate: predicate.to_string(),
            severity,
            message: message.to_string(),
            actual: json!(42),
            expected: json!(10),
            comparison: Comparison::Lte,
            remediation_hint: remediation_hint.map(str::to_string),
        }
    }

    fn mode_mismatch_violation(
        rule_id: &str,
        predicate: &str,
        remediation_hint: Option<&str>,
    ) -> Violation {
        Violation {
            rule_id: rule_id.to_string(),
            predicate: predicate.to_string(),
            severity: Severity::Critical,
            message: format!(
                "deep-only predicate `{rule_id}` cannot run in explicit overview mode"
            ),
            actual: serde_json::Value::Null,
            expected: serde_json::Value::Null,
            comparison: Comparison::Eq,
            remediation_hint: remediation_hint.map(str::to_string),
        }
    }

    fn skipped(rule_id: &str, reason: SkipReason) -> SkippedRule {
        SkippedRule {
            rule_id: rule_id.to_string(),
            reason,
        }
    }

    fn render(policy: &Policy, result: &PolicyResult) -> String {
        render_junit_report(&PolicyRenderContext {
            heap_path: Path::new("heap.hprof"),
            policy_path: Path::new("policy.toml"),
            policy,
            result,
            fail_on: Severity::Error,
        })
    }

    fn assert_well_formed_xml_like(input: &str) {
        let mut stack = Vec::new();
        let mut cursor = input;

        while let Some(start) = cursor.find('<') {
            cursor = &cursor[start..];

            if let Some(rest) = cursor.strip_prefix("<![CDATA[") {
                let end = rest.find("]]>").expect("CDATA must terminate");
                cursor = &rest[end + 3..];
                continue;
            }

            let end = cursor.find('>').expect("tag must terminate");
            let tag = &cursor[1..end];
            let trimmed = tag.trim();

            if let Some(rest) = trimmed.strip_prefix('/') {
                let expected = stack.pop().expect("closing tag must match an opener");
                assert_eq!(rest.trim(), expected, "closing tag order mismatch");
            } else if !trimmed.starts_with('!') {
                let self_closing = trimmed.ends_with('/');
                let name = trimmed
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .expect("tag name should exist");
                if !self_closing {
                    stack.push(name.to_string());
                }
            }

            cursor = &cursor[end + 1..];
        }

        assert!(stack.is_empty(), "all XML tags should be closed");
    }

    #[test]
    fn junit_empty_result_emits_well_formed_xml() {
        let policy = policy(Vec::new());
        let result = result(Vec::new(), Vec::new(), Vec::new());

        let rendered = render(&policy, &result);

        assert_eq!(
            rendered,
            "<testsuites>\n  <testsuite name=\"mnemosyne-ci-check\" tests=\"0\" failures=\"0\" errors=\"0\" skipped=\"0\">\n  </testsuite>\n</testsuites>\n"
        );
        assert_well_formed_xml_like(&rendered);
    }

    #[test]
    fn junit_violation_renders_failure_element() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![threshold_violation(
                "heap-budget",
                "total_bytes",
                Severity::Warning,
                "heap budget exceeded",
                None,
            )],
            Vec::new(),
            vec![evaluation("heap-budget", false, json!(42))],
        );

        let rendered = render(&policy, &result);

        assert!(rendered.contains("<failure type=\"warning\" message=\"heap budget exceeded\">"));
        assert!(!rendered.contains("<error type="));
    }

    #[test]
    fn junit_skipped_renders_skipped_element() {
        let policy = policy(vec![rule("deep-rule", Predicate::LeakCount)]);
        let result = result(
            Vec::new(),
            vec![skipped("deep-rule", SkipReason::DeepOnlyInOverviewMode)],
            Vec::new(),
        );

        let rendered = render(&policy, &result);

        assert!(rendered.contains(
            "<skipped message=\"deep-only predicate skipped because auto mode resolved to overview\"/>"
        ));
    }

    #[test]
    fn junit_mode_mismatch_renders_error_not_failure() {
        let policy = policy(vec![rule("mode-mismatch", Predicate::RetainedSize)]);
        let result = result(
            vec![mode_mismatch_violation(
                "mode-mismatch",
                "retained_size",
                Some("Use --mode deep"),
            )],
            Vec::new(),
            Vec::new(),
        );

        let rendered = render(&policy, &result);

        assert!(rendered.contains("<error type=\"critical\""), "{rendered}");
        assert!(
            !rendered.contains("<failure type=\"critical\""),
            "{rendered}"
        );
    }

    #[test]
    fn junit_xml_escaping_for_special_chars() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![threshold_violation(
                "heap-budget",
                "total_bytes",
                Severity::Error,
                "bad < heap & \"quote\" 'apos' > zero",
                None,
            )],
            Vec::new(),
            vec![evaluation("heap-budget", false, json!(42))],
        );

        let rendered = render(&policy, &result);

        assert!(rendered.contains(
            "message=\"bad &lt; heap &amp; &quot;quote&quot; &apos;apos&apos; &gt; zero\""
        ));
    }

    #[test]
    fn junit_snapshot_full_result() {
        let policy = policy(vec![
            rule("pass-rule", Predicate::TotalInstances),
            rule("fail-rule", Predicate::TotalBytes),
            rule("skip-rule", Predicate::LeakCount),
            rule("mode-mismatch", Predicate::RetainedSize),
        ]);
        let result = result(
            vec![
                threshold_violation(
                    "fail-rule",
                    "total_bytes",
                    Severity::Error,
                    "heap budget exceeded",
                    Some("Reduce retained roots"),
                ),
                mode_mismatch_violation("mode-mismatch", "retained_size", Some("Use --mode deep")),
            ],
            vec![skipped("skip-rule", SkipReason::DeepOnlyInOverviewMode)],
            vec![
                evaluation("pass-rule", true, json!(5)),
                evaluation("fail-rule", false, json!(42)),
            ],
        );

        let rendered = render(&policy, &result);

        assert_eq!(
            rendered,
            concat!(
                "<testsuites>\n",
                "  <testsuite name=\"mnemosyne-ci-check\" tests=\"4\" failures=\"1\" errors=\"1\" skipped=\"1\">\n",
                "    <testcase name=\"pass-rule\" classname=\"mnemosyne.policy.total_instances\" />\n",
                "    <testcase name=\"fail-rule\" classname=\"mnemosyne.policy.total_bytes\">\n",
                "      <failure type=\"error\" message=\"heap budget exceeded\"><![CDATA[actual: 42\nexpected: <= 10\nremediation: Reduce retained roots]]></failure>\n",
                "    </testcase>\n",
                "    <testcase name=\"skip-rule\" classname=\"mnemosyne.policy.leak_count\">\n",
                "      <skipped message=\"deep-only predicate skipped because auto mode resolved to overview\"/>\n",
                "    </testcase>\n",
                "    <testcase name=\"mode-mismatch\" classname=\"mnemosyne.policy.retained_size\">\n",
                "      <error type=\"critical\" message=\"deep-only predicate `mode-mismatch` cannot run in explicit overview mode\"><![CDATA[actual: null\nexpected: == null\nremediation: Use --mode deep]]></error>\n",
                "    </testcase>\n",
                "  </testsuite>\n",
                "</testsuites>\n"
            )
        );
    }
}
