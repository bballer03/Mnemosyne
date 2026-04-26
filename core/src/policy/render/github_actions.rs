use std::fmt::Write;

use crate::{Severity, SkipReason, Violation};

use super::PolicyRenderContext;

/// Renders GitHub Actions workflow commands for `mnemosyne ci-check`.
///
/// Known limitation: policy rules do not carry source spans yet, so annotations
/// include `file=` but intentionally omit `line=`.
/// TODO: thread TOML spans through `PolicyRule` and add `line=` when available.
pub fn render_github_actions_report(context: &PolicyRenderContext<'_>) -> String {
    let mut output = String::new();
    let policy_path = context.policy_path.to_string_lossy();

    for rule in &context.policy.rules {
        if let Some(violation) = context
            .result
            .violations
            .iter()
            .find(|item| item.rule_id == rule.id)
        {
            writeln!(
                &mut output,
                "::{} file={},title={}::{}",
                annotation_level(violation),
                escape_property(&policy_path),
                escape_property(&rule.id),
                escape_message(&violation.message)
            )
            .expect("write to string should succeed");
            continue;
        }

        if let Some(skipped) = context
            .result
            .skipped
            .iter()
            .find(|item| item.rule_id == rule.id)
        {
            writeln!(
                &mut output,
                "::notice file={},title={}::{}",
                escape_property(&policy_path),
                escape_property(&rule.id),
                escape_message(&format!("Skipped: {}", format_skip_reason(&skipped.reason)))
            )
            .expect("write to string should succeed");
        }
    }

    writeln!(
        &mut output,
        "Mnemosyne ci-check: {} violations ({} critical, {} error, {} warning, {} info), {} skipped",
        context.result.violations.len(),
        count_severity(context, Severity::Critical),
        count_severity(context, Severity::Error),
        count_severity(context, Severity::Warning),
        count_severity(context, Severity::Info),
        context.result.skipped.len()
    )
    .expect("write to string should succeed");

    output
}

fn annotation_level(violation: &Violation) -> &'static str {
    match violation.severity {
        Severity::Critical | Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "notice",
    }
}

fn count_severity(context: &PolicyRenderContext<'_>, severity: Severity) -> usize {
    context
        .result
        .violations
        .iter()
        .filter(|violation| violation.severity == severity)
        .count()
}

fn format_skip_reason(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::DeepOnlyInOverviewMode => {
            "deep-only predicate skipped because auto mode resolved to overview"
        }
        SkipReason::UnsupportedInThisMode => "predicate is unsupported in this analysis mode",
    }
}

fn escape_message(input: &str) -> String {
    input
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(input: &str) -> String {
    escape_message(input)
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::render_github_actions_report;
    use crate::policy::PolicyDefaults;
    use crate::{
        Comparison, ModeRequirement, Policy, PolicyResult, PolicyRule, Predicate, Severity,
        SkipReason, SkippedRule, Violation,
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

    fn result(violations: Vec<Violation>, skipped: Vec<SkippedRule>) -> PolicyResult {
        PolicyResult {
            mode_used: crate::AnalysisMode::Overview,
            mode_requested: crate::AnalysisMode::Overview,
            violations,
            evaluations: Vec::new(),
            skipped,
        }
    }

    fn violation(rule_id: &str, severity: Severity, message: &str) -> Violation {
        Violation {
            rule_id: rule_id.to_string(),
            predicate: "total_bytes".to_string(),
            severity,
            message: message.to_string(),
            actual: json!(42),
            expected: json!(10),
            comparison: Comparison::Lte,
            remediation_hint: None,
        }
    }

    fn skipped(rule_id: &str, reason: SkipReason) -> SkippedRule {
        SkippedRule {
            rule_id: rule_id.to_string(),
            reason,
        }
    }

    fn render(policy: &Policy, result: &PolicyResult) -> String {
        render_github_actions_report(&PolicyRenderContext {
            heap_path: Path::new("heap.hprof"),
            policy_path: Path::new("policy.toml"),
            policy,
            result,
            fail_on: Severity::Error,
        })
    }

    #[test]
    fn gh_actions_violation_emits_error_command() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![violation(
                "heap-budget",
                Severity::Critical,
                "heap budget exceeded",
            )],
            Vec::new(),
        );

        let rendered = render(&policy, &result);
        let first_line = rendered.lines().next().unwrap();

        assert_eq!(
            first_line,
            "::error file=policy.toml,title=heap-budget::heap budget exceeded"
        );
    }

    #[test]
    fn gh_actions_warning_severity_uses_warning_command() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![violation(
                "heap-budget",
                Severity::Warning,
                "warning threshold crossed",
            )],
            Vec::new(),
        );

        let rendered = render(&policy, &result);

        assert!(rendered.starts_with(
            "::warning file=policy.toml,title=heap-budget::warning threshold crossed"
        ));
    }

    #[test]
    fn gh_actions_info_severity_uses_notice_command() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![violation(
                "heap-budget",
                Severity::Info,
                "info threshold crossed",
            )],
            Vec::new(),
        );

        let rendered = render(&policy, &result);

        assert!(rendered
            .starts_with("::notice file=policy.toml,title=heap-budget::info threshold crossed"));
    }

    #[test]
    fn gh_actions_skipped_emits_notice_with_skipped_prefix() {
        let policy = policy(vec![rule("deep-rule", Predicate::LeakCount)]);
        let result = result(
            Vec::new(),
            vec![skipped("deep-rule", SkipReason::DeepOnlyInOverviewMode)],
        );

        let rendered = render(&policy, &result);

        assert!(rendered.starts_with(
            "::notice file=policy.toml,title=deep-rule::Skipped: deep-only predicate skipped because auto mode resolved to overview"
        ));
    }

    #[test]
    fn gh_actions_summary_line_counts_by_severity() {
        let policy = policy(vec![
            rule("critical-rule", Predicate::TotalBytes),
            rule("error-rule", Predicate::TotalBytes),
            rule("warning-rule", Predicate::TotalBytes),
            rule("info-rule", Predicate::TotalBytes),
            rule("skip-rule", Predicate::LeakCount),
        ]);
        let result = result(
            vec![
                violation("critical-rule", Severity::Critical, "critical"),
                violation("error-rule", Severity::Error, "error"),
                violation("warning-rule", Severity::Warning, "warning"),
                violation("info-rule", Severity::Info, "info"),
            ],
            vec![skipped("skip-rule", SkipReason::DeepOnlyInOverviewMode)],
        );

        let rendered = render(&policy, &result);
        let summary = rendered.lines().last().unwrap();

        assert_eq!(
            summary,
            "Mnemosyne ci-check: 4 violations (1 critical, 1 error, 1 warning, 1 info), 1 skipped"
        );
    }

    #[test]
    fn gh_actions_no_line_field_present() {
        let policy = policy(vec![rule("heap-budget", Predicate::TotalBytes)]);
        let result = result(
            vec![violation(
                "heap-budget",
                Severity::Error,
                "heap budget exceeded",
            )],
            Vec::new(),
        );

        let rendered = render(&policy, &result);

        assert!(
            rendered.contains("file=policy.toml,title=heap-budget"),
            "{rendered}"
        );
        assert!(!rendered.contains("line="), "{rendered}");
    }
}
