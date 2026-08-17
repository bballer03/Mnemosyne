use std::fmt::Write as _;

use crate::analysis::ObjectInspection;

pub fn render_text(inspection: &ObjectInspection) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "Object {}  ({})",
        inspection.object_id, inspection.class_name
    );

    let retained_label = inspection
        .retained_size
        .map(format_bytes_human)
        .unwrap_or_else(|| "n/a".to_string());
    let _ = writeln!(
        out,
        "  Shallow: {}   Retained: {}",
        format_bytes_human(inspection.shallow_size),
        retained_label
    );

    let dominator_parent_label = inspection
        .dominator_parent
        .as_deref()
        .unwrap_or("(none - direct GC root)");
    let _ = writeln!(out, "  Dominator parent: {dominator_parent_label}");
    let _ = writeln!(
        out,
        "  Dominator children: {}",
        inspection.dominator_children.len()
    );

    let _ = writeln!(
        out,
        "  References out ({}): {}",
        inspection.references_out.len(),
        join_or_none(&inspection.references_out)
    );
    let _ = writeln!(
        out,
        "  Referrers in ({}): {}",
        inspection.referrers_in.len(),
        join_or_none(&inspection.referrers_in)
    );

    if let Some(fields) = &inspection.fields {
        let _ = writeln!(out, "  Fields (--retain-field-data only):");
        for field in fields {
            let _ = writeln!(
                out,
                "    {}: {} = {}",
                field.name, field.type_name, field.value
            );
        }
    }

    out.trim_end_matches('\n').to_string()
}

fn join_or_none(entries: &[String]) -> String {
    if entries.is_empty() {
        "(none)".to_string()
    } else {
        entries.join(", ")
    }
}

/// Human-readable byte size, matching the `48 B` / `1.20 MB` style used by
/// the design doc's §9 example output.
fn format_bytes_human(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FieldValueEntry;

    fn sample_inspection() -> ObjectInspection {
        ObjectInspection {
            object_id: "0x00001000".into(),
            class_name: "com.example.CacheEntry".into(),
            shallow_size: 48,
            retained_size: Some(1_258_291), // ~1.20 MB
            fields: Some(vec![FieldValueEntry {
                name: "key".into(),
                type_name: "com.example.Key".into(),
                value: "0x00002000".into(),
            }]),
            references_out: vec!["0x00003000 (java.lang.String)".into()],
            referrers_in: vec!["0x00004000 (com.example.Cache)".into()],
            dominator_parent: Some("0x00004000 (com.example.Cache)".into()),
            dominator_children: vec!["0x00005000".into(), "0x00005001".into()],
        }
    }

    #[test]
    fn renders_all_sections_when_fields_present() {
        let text = render_text(&sample_inspection());

        assert!(text.contains("Object 0x00001000  (com.example.CacheEntry)"));
        assert!(text.contains("Shallow: 48 B"));
        assert!(text.contains("Retained: 1.20 MB"));
        assert!(text.contains("Dominator parent: 0x00004000 (com.example.Cache)"));
        assert!(text.contains("Dominator children: 2"));
        assert!(text.contains("References out (1): 0x00003000 (java.lang.String)"));
        assert!(text.contains("Referrers in (1): 0x00004000 (com.example.Cache)"));
        assert!(text.contains("Fields (--retain-field-data only):"));
        assert!(text.contains("key: com.example.Key = 0x00002000"));
    }

    #[test]
    fn omits_fields_section_when_absent() {
        let mut inspection = sample_inspection();
        inspection.fields = None;

        let text = render_text(&inspection);

        assert!(!text.contains("Fields"));
    }

    #[test]
    fn empty_refs_and_missing_dominator_parent_render_placeholders() {
        let inspection = ObjectInspection {
            object_id: "0x00001000".into(),
            class_name: "com.example.Root".into(),
            shallow_size: 8,
            retained_size: None,
            fields: None,
            references_out: Vec::new(),
            referrers_in: Vec::new(),
            dominator_parent: None,
            dominator_children: Vec::new(),
        };

        let text = render_text(&inspection);

        assert!(text.contains("Retained: n/a"));
        assert!(text.contains("Dominator parent: (none - direct GC root)"));
        assert!(text.contains("Dominator children: 0"));
        assert!(text.contains("References out (0): (none)"));
        assert!(text.contains("Referrers in (0): (none)"));
    }
}
