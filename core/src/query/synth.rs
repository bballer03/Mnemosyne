use super::types::CellValue;
use crate::analysis::string_analysis::extract_string_value;
use crate::hprof::ObjectGraph;

const MAX_SYNTH_TO_STRING_CHARS: usize = 256;

pub(crate) fn synth_to_string(graph: &ObjectGraph, object_id: u64) -> CellValue {
    let Some(object) = graph.get_object(object_id) else {
        return CellValue::Null;
    };
    let Some(class_name) = graph
        .class_name(object.class_id)
        .map(|name| name.replace('/', "."))
    else {
        return CellValue::Null;
    };

    if class_name == "java.lang.String" {
        return extract_string_value(graph, object_id)
            .map(truncate_string)
            .map(CellValue::Str)
            .unwrap_or(CellValue::Null);
    }

    CellValue::Str(format!("{class_name}@{object_id}"))
}

fn truncate_string(value: String) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(MAX_SYNTH_TO_STRING_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}\u{2026}")
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, HeapObject, ObjectGraph, ObjectKind,
    };

    fn add_class(
        graph: &mut ObjectGraph,
        class_id: u64,
        super_class_id: u64,
        name: &str,
        fields: Vec<FieldDescriptor>,
    ) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id,
                class_loader_id: 0,
                instance_size: 0,
                name: Some(name.into()),
                instance_fields: fields,
                static_references: Vec::new(),
            },
        );
    }

    fn object_ref_bytes(id: u64) -> Vec<u8> {
        id.to_be_bytes().to_vec()
    }

    fn string_field_bytes(value_array_id: u64, coder: i8) -> Vec<u8> {
        let mut bytes = object_ref_bytes(value_array_id);
        bytes.push(coder as u8);
        bytes
    }

    fn make_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
        add_class(
            &mut graph,
            2,
            1,
            "java.lang.String",
            vec![
                FieldDescriptor {
                    name: Some("value".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("coder".into()),
                    field_type: field_types::BYTE,
                },
            ],
        );
        add_class(&mut graph, 3, 1, "com.example.Cache", Vec::new());

        graph.objects.insert(
            10,
            HeapObject {
                id: 10,
                class_id: 2,
                shallow_size: 24,
                references: vec![100],
                field_data: string_field_bytes(100, 0),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            11,
            HeapObject {
                id: 11,
                class_id: 2,
                shallow_size: 24,
                references: vec![101],
                field_data: string_field_bytes(101, 0),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            20,
            HeapObject {
                id: 20,
                class_id: 3,
                shallow_size: 16,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );

        graph.objects.insert(
            100,
            HeapObject {
                id: 100,
                class_id: 0,
                shallow_size: 8,
                references: Vec::new(),
                field_data: b"hello".to_vec(),
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::BYTE,
                    length: 5,
                },
            },
        );
        graph.objects.insert(
            101,
            HeapObject {
                id: 101,
                class_id: 0,
                shallow_size: 260,
                references: Vec::new(),
                field_data: vec![b'a'; 300],
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::BYTE,
                    length: 300,
                },
            },
        );

        graph
    }

    #[test]
    fn synth_to_string_for_string_returns_actual_content() {
        let graph = make_graph();

        assert_eq!(synth_to_string(&graph, 10), CellValue::Str("hello".into()));
    }

    #[test]
    fn synth_to_string_truncates_long_strings() {
        let graph = make_graph();

        let CellValue::Str(value) = synth_to_string(&graph, 11) else {
            panic!("expected string cell value");
        };

        assert_eq!(value.chars().count(), 257);
        assert!(value.starts_with(&"a".repeat(256)));
        assert!(value.ends_with('\u{2026}'));
    }

    #[test]
    fn synth_to_string_for_other_class_returns_class_at_id_format() {
        let graph = make_graph();

        assert_eq!(
            synth_to_string(&graph, 20),
            CellValue::Str("com.example.Cache@20".into())
        );
    }

    #[test]
    fn synth_to_string_for_null_returns_null() {
        let graph = make_graph();

        assert_eq!(synth_to_string(&graph, 999), CellValue::Null);
    }
}
