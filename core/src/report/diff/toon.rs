use std::fmt::Write as _;

use crate::{
    diff::{ObjectDelta, ObjectDiffReport},
    hprof::{ClassLevelDelta, HeapDiff},
};

use super::{dominator_chain_label, identity_strategy_label, risk_label};

pub fn render_toon(diff: &HeapDiff) -> String {
    let mut doc = String::from("TOON v1\n");

    doc.push_str("section heap_diff\n");
    push_kv(&mut doc, 2, "before", &diff.before);
    push_kv(&mut doc, 2, "after", &diff.after);
    push_kv(&mut doc, 2, "delta_bytes", diff.delta_bytes);
    push_kv(&mut doc, 2, "delta_objects", diff.delta_objects);
    push_kv(
        &mut doc,
        2,
        "changed_class_count",
        diff.changed_classes.len(),
    );
    for (idx, delta) in diff.changed_classes.iter().enumerate() {
        doc.push_str(&format!("  changed_class#{idx}\n"));
        push_kv(&mut doc, 4, "name", &delta.name);
        push_kv(&mut doc, 4, "before_bytes", delta.before_bytes);
        push_kv(&mut doc, 4, "after_bytes", delta.after_bytes);
    }

    doc.push_str("section class_diff\n");
    match &diff.class_diff {
        Some(entries) if entries.is_empty() => push_kv(&mut doc, 2, "status", "empty"),
        Some(entries) => render_class_diff_section(&mut doc, entries),
        None => push_kv(&mut doc, 2, "status", "absent"),
    }

    doc.push_str("section object_diff\n");
    match &diff.object_diff {
        Some(report) => render_object_diff_section(&mut doc, report),
        None => push_kv(&mut doc, 2, "status", "absent"),
    }

    doc.push_str("section match_quality\n");
    match &diff.object_diff {
        Some(report) => render_match_quality_section(&mut doc, report),
        None => push_kv(&mut doc, 2, "status", "absent"),
    }

    doc.trim_end_matches('\n').to_string()
}

fn render_class_diff_section(doc: &mut String, entries: &[ClassLevelDelta]) {
    push_kv(doc, 2, "count", entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        doc.push_str(&format!("  class#{idx}\n"));
        push_kv(doc, 4, "class_name", &entry.class_name);
        push_kv(doc, 4, "before_instances", entry.before_instances);
        push_kv(doc, 4, "after_instances", entry.after_instances);
        push_kv(doc, 4, "before_shallow_bytes", entry.before_shallow_bytes);
        push_kv(doc, 4, "after_shallow_bytes", entry.after_shallow_bytes);
        push_kv(doc, 4, "before_retained_bytes", entry.before_retained_bytes);
        push_kv(doc, 4, "after_retained_bytes", entry.after_retained_bytes);
    }
}

fn render_object_diff_section(doc: &mut String, report: &ObjectDiffReport) {
    push_kv(doc, 2, "strategy", identity_strategy_label(report.strategy));
    push_kv(doc, 2, "retained_bucket_bits", report.retained_bucket_bits);
    push_kv(
        doc,
        2,
        "retained_change_threshold",
        report.retained_change_threshold,
    );
    push_kv(doc, 2, "added_count", report.added.len());
    push_kv(doc, 2, "removed_count", report.removed.len());
    push_kv(
        doc,
        2,
        "retained_changed_count",
        report.retained_changed.len(),
    );
    push_kv(
        doc,
        2,
        "before_object_count",
        report.totals.before_object_count,
    );
    push_kv(
        doc,
        2,
        "after_object_count",
        report.totals.after_object_count,
    );
    push_kv(
        doc,
        2,
        "fingerprint_collisions_before",
        report.totals.fingerprint_collisions_before,
    );
    push_kv(
        doc,
        2,
        "fingerprint_collisions_after",
        report.totals.fingerprint_collisions_after,
    );
    push_kv(doc, 2, "matched_pairs", report.totals.matched_pairs);

    render_object_delta_group(doc, "added", &report.added);
    render_object_delta_group(doc, "removed", &report.removed);
    render_object_delta_group(doc, "retained_changed", &report.retained_changed);
}

fn render_match_quality_section(doc: &mut String, report: &ObjectDiffReport) {
    push_kv(
        doc,
        2,
        "strategy",
        identity_strategy_label(report.match_quality.strategy),
    );
    push_kv(
        doc,
        2,
        "collision_rate",
        format!("{:.3}", report.match_quality.collision_rate),
    );
    push_kv(
        doc,
        2,
        "false_match_risk",
        risk_label(report.match_quality.estimated_false_match_risk),
    );
    push_kv(
        doc,
        2,
        "false_split_risk",
        risk_label(report.match_quality.estimated_false_split_risk),
    );
    push_kv(doc, 2, "note_count", report.match_quality.notes.len());
    for (idx, note) in report.match_quality.notes.iter().enumerate() {
        push_kv(doc, 2, &format!("note#{idx}"), note);
    }
}

fn render_object_delta_group(doc: &mut String, prefix: &str, deltas: &[ObjectDelta]) {
    for (idx, delta) in deltas.iter().enumerate() {
        doc.push_str(&format!("  {prefix}#{idx}\n"));
        push_kv(doc, 4, "class_name", &delta.class_name);
        push_kv(doc, 4, "example_object_id", delta.example_object_id);
        push_kv(doc, 4, "before_count", delta.before_count);
        push_kv(doc, 4, "after_count", delta.after_count);
        push_kv(doc, 4, "before_retained_bytes", delta.before_retained_bytes);
        push_kv(doc, 4, "after_retained_bytes", delta.after_retained_bytes);
        push_kv(doc, 4, "fingerprint_class_id", delta.fingerprint.class_id);
        push_kv(
            doc,
            4,
            "fingerprint_retained_bucket",
            delta.fingerprint.retained_bucket,
        );
        push_kv(
            doc,
            4,
            "fingerprint_dominator_signature",
            delta.fingerprint.dominator_signature,
        );
        push_kv(
            doc,
            4,
            "fingerprint_field_signature",
            delta.fingerprint.field_signature,
        );
        push_kv(doc, 4, "kind", format!("{:?}", delta.kind));
        push_kv(
            doc,
            4,
            "dominator_chain",
            dominator_chain_label(&delta.dominator_chain),
        );
        push_kv(
            doc,
            4,
            "reference_chain",
            delta.reference_chain.join(" -> "),
        );
    }
}

fn push_kv<T: std::fmt::Display>(buf: &mut String, indent: usize, key: &str, value: T) {
    for _ in 0..indent {
        buf.push(' ');
    }
    let raw = value.to_string();
    let _ = writeln!(buf, "{}={}", key, escape_toon_value(&raw));
}

fn escape_toon_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
