use std::fmt::Write as _;

use crate::{
    diff::{ObjectDelta, ObjectDeltaKind, ObjectDiffReport},
    hprof::{ClassLevelDelta, HeapDiff},
};

use super::{
    binary_unit_label, bucket_label, dominator_chain_label, identity_strategy_label, risk_label,
};

pub fn render_text(diff: &HeapDiff) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "Heap diff: {} -> {}", diff.before, diff.after);
    let _ = writeln!(
        out,
        "  Delta size: {}",
        format_signed_megabytes(diff.delta_bytes)
    );
    let _ = writeln!(
        out,
        "  Delta objects: {}",
        format_signed_count(diff.delta_objects)
    );

    if diff.changed_classes.is_empty() {
        let _ = writeln!(out, "  No dominant class or record shifts detected.");
    } else {
        let _ = writeln!(out, "  Top changes:");
        for entry in &diff.changed_classes {
            let delta = entry.after_bytes as i64 - entry.before_bytes as i64;
            let before_mb = entry.before_bytes as f64 / (1024.0 * 1024.0);
            let after_mb = entry.after_bytes as f64 / (1024.0 * 1024.0);
            let _ = writeln!(
                out,
                "    - {}: {} (before {:.2} MB -> after {:.2} MB)",
                entry.name,
                format_signed_megabytes(delta),
                before_mb,
                after_mb
            );
        }
    }

    if let Some(class_diff) = &diff.class_diff {
        if !class_diff.is_empty() {
            let _ = writeln!(out, "  Class-level retained deltas:");
            out.push_str(&render_class_diff_table(class_diff));
            out.push('\n');
        }
    }

    if let Some(object_diff) = &diff.object_diff {
        out.push_str(&render_object_diff_text(object_diff));
        out.push('\n');
    }

    out.trim_end_matches('\n').to_string()
}

fn render_class_diff_table(class_diff: &[ClassLevelDelta]) -> String {
    let rows: Vec<[String; 4]> = class_diff
        .iter()
        .take(10)
        .map(|entry| {
            let instance_delta = entry.after_instances as i64 - entry.before_instances as i64;
            let retained_delta =
                entry.after_retained_bytes as i64 - entry.before_retained_bytes as i64;
            [
                entry.class_name.clone(),
                format_signed_count(instance_delta),
                format!(
                    "{:.2} -> {:.2} MB",
                    entry.before_shallow_bytes as f64 / (1024.0 * 1024.0),
                    entry.after_shallow_bytes as f64 / (1024.0 * 1024.0)
                ),
                format_signed_megabytes(retained_delta),
            ]
        })
        .collect();

    let headers = ["Class", "Instances", "Shallow", "Retained Delta"];
    let class_width = std::iter::once(headers[0].len())
        .chain(rows.iter().map(|row| row[0].len()))
        .max()
        .unwrap_or(headers[0].len());
    let instances_width = std::iter::once(headers[1].len())
        .chain(rows.iter().map(|row| row[1].len()))
        .max()
        .unwrap_or(headers[1].len());
    let shallow_width = std::iter::once(headers[2].len())
        .chain(rows.iter().map(|row| row[2].len()))
        .max()
        .unwrap_or(headers[2].len());
    let retained_width = std::iter::once(headers[3].len())
        .chain(rows.iter().map(|row| row[3].len()))
        .max()
        .unwrap_or(headers[3].len());

    let mut table = String::new();
    let _ = writeln!(
        table,
        "{:<class_width$} | {:>instances_width$} | {:>shallow_width$} | {:>retained_width$}",
        headers[0], headers[1], headers[2], headers[3],
    );
    let _ = writeln!(
        table,
        "{}-+-{}-+-{}-+-{}",
        "-".repeat(class_width),
        "-".repeat(instances_width),
        "-".repeat(shallow_width),
        "-".repeat(retained_width),
    );

    for row in rows {
        let _ = writeln!(
            table,
            "{:<class_width$} | {:>instances_width$} | {:>shallow_width$} | {:>retained_width$}",
            row[0], row[1], row[2], row[3],
        );
    }

    table.trim_end_matches('\n').to_string()
}

fn render_object_diff_text(report: &ObjectDiffReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "object diff (strategy={}, bucket={}, threshold={}):",
        identity_strategy_label(report.strategy),
        bucket_label(report.retained_bucket_bits),
        binary_unit_label(report.retained_change_threshold),
    );

    let _ = writeln!(out, "  added ({}):", report.added.len());
    for delta in &report.added {
        let _ = writeln!(out, "{}", format_object_delta_line(delta));
    }

    let _ = writeln!(out, "  removed ({}):", report.removed.len());
    for delta in &report.removed {
        let _ = writeln!(out, "{}", format_object_delta_line(delta));
    }

    let _ = writeln!(
        out,
        "  retained_changed ({}):",
        report.retained_changed.len()
    );
    for delta in &report.retained_changed {
        let _ = writeln!(out, "{}", format_object_delta_line(delta));
    }

    let _ = writeln!(
        out,
        "  match quality: collision_rate={:.3}  false_match_risk={}  false_split_risk={}",
        report.match_quality.collision_rate,
        risk_label(report.match_quality.estimated_false_match_risk),
        risk_label(report.match_quality.estimated_false_split_risk),
    );
    let _ = write!(
        out,
        "    notes: \"{}\"",
        report.match_quality.notes.join("; ")
    );

    out.trim_end_matches('\n').to_string()
}

fn format_object_delta_line(delta: &ObjectDelta) -> String {
    match delta.kind {
        ObjectDeltaKind::Added => format!(
            "    {:<32} {:+} bytes  count={}  dom={}",
            delta.class_name,
            delta.after_retained_bytes as i64,
            delta.after_count,
            dominator_chain_label(&delta.dominator_chain),
        ),
        ObjectDeltaKind::Removed => format!(
            "    {:<32} {:+} bytes  count={}  dom={}",
            delta.class_name,
            -(delta.before_retained_bytes as i64),
            delta.before_count,
            dominator_chain_label(&delta.dominator_chain),
        ),
        ObjectDeltaKind::RetainedChanged => format!(
            "    {:<32} {:+} bytes  count={}->{}  dom={}",
            delta.class_name,
            delta.after_retained_bytes as i64 - delta.before_retained_bytes as i64,
            delta.before_count,
            delta.after_count,
            dominator_chain_label(&delta.dominator_chain),
        ),
    }
}

fn format_signed_megabytes(delta_bytes: i64) -> String {
    format!("{:+.2} MB", delta_bytes as f64 / (1024.0 * 1024.0))
}

fn format_signed_count(delta: i64) -> String {
    format!("{delta:+}")
}
