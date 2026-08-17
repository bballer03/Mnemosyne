use crate::analysis::ObjectInspection;

pub fn render_json(inspection: &ObjectInspection) -> String {
    serde_json::to_string_pretty(inspection)
        .expect("ObjectInspection serialization should not fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FieldValueEntry;

    #[test]
    fn round_trips_via_serde() {
        let inspection = ObjectInspection {
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
            referrers_in: Vec::new(),
            dominator_parent: None,
            dominator_children: Vec::new(),
        };

        let rendered = render_json(&inspection);
        let round_tripped: ObjectInspection = serde_json::from_str(&rendered).unwrap();

        assert_eq!(round_tripped, inspection);
    }

    #[test]
    fn omits_fields_key_when_none() {
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

        let rendered = render_json(&inspection);
        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert!(value.get("fields").is_none());
    }
}
