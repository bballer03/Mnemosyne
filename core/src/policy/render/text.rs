use std::fmt::Write;

use crate::{Comparison, Severity, SkipReason, Violation};

use super::PolicyRenderContext;

pub fn render_text_report(context: &PolicyRenderContext<'_>) -> String {
    let mut output = String::new();
    let policy_name = context
        .policy
        .meta
        .as_ref()
        .and_then(|meta| meta.name.as_deref())
        .unwrap_or("unnamed-policy");
    let passed = context
        .result
        .evaluations
        .iter()
        .filter(|evaluation| evaluation.passed)
        .count();
    let failed = has_violations_at_or_above(context.result, context.fail_on);

    writeln!(&mut output, "Policy: {policy_name}").expect("write to string should succeed");
    writeln!(&mut output, "Heap: {}", context.heap_path.display())
        .expect("write to string should succeed");
    writeln!(
        &mut output,
        "Policy file: {}",
        context.policy_path.display()
    )
    .expect("write to string should succeed");
    writeln!(
        &mut output,
        "Mode: requested={} used={}",
        mode_label(context.result.mode_requested),
        mode_label(context.result.mode_used)
    )
    .expect("write to string should succeed");
    writeln!(&mut output, "Total rules: {}", context.policy.rules.len())
        .expect("write to string should succeed");
    writeln!(
        &mut output,
        "Total violations: {}",
        context.result.violations.len()
    )
    .expect("write to string should succeed");

    if let Some(description) = context
        .policy
        .meta
        .as_ref()
        .and_then(|meta| meta.description.as_deref())
    {
        writeln!(&mut output, "Description: {description}")
            .expect("write to string should succeed");
    }

    if !context.result.skipped.is_empty() {
        writeln!(&mut output).expect("write to string should succeed");
        writeln!(&mut output, "Skipped rules:").expect("write to string should succeed");
        for skipped in &context.result.skipped {
            writeln!(
                &mut output,
                "- {}: {}",
                skipped.rule_id,
                format_skip_reason(&skipped.reason)
            )
            .expect("write to string should succeed");
        }
    }

    for severity in [
        Severity::Critical,
        Severity::Error,
        Severity::Warning,
        Severity::Info,
    ] {
        let group = context
            .result
            .violations
            .iter()
            .filter(|violation| violation.severity == severity)
            .collect::<Vec<_>>();

        if group.is_empty() {
            continue;
        }

        writeln!(&mut output).expect("write to string should succeed");
        writeln!(&mut output, "[{}]", severity_label(severity))
            .expect("write to string should succeed");

        for violation in group {
            writeln!(&mut output, "{}", format_violation(violation))
                .expect("write to string should succeed");
            if let Some(remediation_hint) = violation.remediation_hint.as_deref() {
                writeln!(&mut output, "  remediation: {remediation_hint}")
                    .expect("write to string should succeed");
            }
        }
    }

    writeln!(&mut output).expect("write to string should succeed");
    writeln!(
        &mut output,
        "Summary: {passed} passed, {} violated, {} skipped",
        context.result.violations.len(),
        context.result.skipped.len()
    )
    .expect("write to string should succeed");
    writeln!(
        &mut output,
        "RESULT: {}{}",
        if failed { "FAIL" } else { "PASS" },
        if failed {
            format!(" (fail-on={})", severity_name(context.fail_on))
        } else {
            String::new()
        }
    )
    .expect("write to string should succeed");

    output
}

fn has_violations_at_or_above(result: &crate::PolicyResult, fail_on: Severity) -> bool {
    result
        .violations
        .iter()
        .any(|violation| violation.severity >= fail_on)
}

fn format_violation(violation: &Violation) -> String {
    format!(
        "[{}] {}: {} (actual={}, expected {} {})",
        severity_label(violation.severity),
        violation.rule_id,
        violation.message,
        format_value(&violation.actual),
        comparison_symbol(violation.comparison),
        format_value(&violation.expected)
    )
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

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "INFO",
        Severity::Warning => "WARNING",
        Severity::Error => "ERROR",
        Severity::Critical => "CRITICAL",
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

fn mode_label(mode: crate::AnalysisMode) -> &'static str {
    match mode {
        crate::AnalysisMode::Auto => "auto",
        crate::AnalysisMode::Deep => "deep",
        crate::AnalysisMode::Overview => "overview",
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
