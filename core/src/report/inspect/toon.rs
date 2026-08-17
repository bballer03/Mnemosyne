use std::fmt::Write as _;

use crate::analysis::{FieldValueEntry, ObjectInspection};

pub fn render_toon(inspection: &ObjectInspection) -> String {
    let mut doc = String::from("TOON v1\n");

    doc.push_str("section object\n");
    push_kv(&mut doc, 2, "object_id", &inspection.object_id);
    push_kv(&mut doc, 2, "class_name", &inspection.class_name);
    push_kv(&mut doc, 2, "shallow_size", inspection.shallow_size);
    match inspection.retained_size {
        Some(size) => push_kv(&mut doc, 2, "retained_size", size),
        None => push_kv(&mut doc, 2, "retained_size", "absent"),
    }
    match &inspection.dominator_parent {
        Some(parent) => push_kv(&mut doc, 2, "dominator_parent", parent),
        None => push_kv(&mut doc, 2, "dominator_parent", "absent"),
    }
    push_kv(
        &mut doc,
        2,
        "dominator_children_count",
        inspection.dominator_children.len(),
    );
    for (idx, child) in inspection.dominator_children.iter().enumerate() {
        push_kv(&mut doc, 2, &format!("dominator_child#{idx}"), child);
    }

    doc.push_str("section references_out\n");
    push_kv(&mut doc, 2, "count", inspection.references_out.len());
    for (idx, reference) in inspection.references_out.iter().enumerate() {
        push_kv(&mut doc, 2, &format!("reference#{idx}"), reference);
    }

    doc.push_str("section referrers_in\n");
    push_kv(&mut doc, 2, "count", inspection.referrers_in.len());
    for (idx, referrer) in inspection.referrers_in.iter().enumerate() {
        push_kv(&mut doc, 2, &format!("referrer#{idx}"), referrer);
    }

    doc.push_str("section fields\n");
    match &inspection.fields {
        Some(fields) if fields.is_empty() => push_kv(&mut doc, 2, "status", "empty"),
        Some(fields) => render_fields_section(&mut doc, fields),
        None => push_kv(&mut doc, 2, "status", "absent"),
    }

    doc.trim_end_matches('\n').to_string()
}

fn render_fields_section(doc: &mut String, fields: &[FieldValueEntry]) {
    push_kv(doc, 2, "count", fields.len());
    for (idx, field) in fields.iter().enumerate() {
        doc.push_str(&format!("  field#{idx}\n"));
        push_kv(doc, 4, "name", &field.name);
        push_kv(doc, 4, "type_name", &field.type_name);
        push_kv(doc, 4, "value", &field.value);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_inspection() -> ObjectInspection {
        ObjectInspection {
            object_id: "0x00001000".into(),
            class_name: "com.example.CacheEntry".into(),
            shallow_size: 48,
            retained_size: Some(1_258_291),
            fields: Some(vec![FieldValueEntry {
                name: "key".into(),
                type_name: "com.example.Key".into(),
                value: "0x00002000".into(),
            }]),
            references_out: vec!["0x00003000 (java.lang.String)".into()],
            referrers_in: vec!["0x00004000 (com.example.Cache)".into()],
            dominator_parent: Some("0x00004000 (com.example.Cache)".into()),
            dominator_children: vec!["0x00005000".into()],
        }
    }

    #[test]
    fn emits_one_section_per_top_level_field() {
        let doc = render_toon(&sample_inspection());

        assert!(doc.starts_with("TOON v1\n"));
        assert!(doc.contains("section object\n"));
        assert!(doc.contains("section references_out\n"));
        assert!(doc.contains("section referrers_in\n"));
        assert!(doc.contains("section fields\n"));
        assert!(doc.contains("object_id=0x00001000"));
        assert!(doc.contains("field#0"));
        assert!(doc.contains("name=key"));
    }

    #[test]
    fn deterministic_across_repeated_renders() {
        let inspection = sample_inspection();
        assert_eq!(render_toon(&inspection), render_toon(&inspection));
    }

    #[test]
    fn absent_fields_render_status_absent() {
        let mut inspection = sample_inspection();
        inspection.fields = None;

        let doc = render_toon(&inspection);
        assert!(doc.contains("section fields\n  status=absent"));
    }
}
