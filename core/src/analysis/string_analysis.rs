use crate::graph::DominatorTree;
use crate::hprof::{
    field_types, read_field, FieldValue, HeapObject, ObjectGraph, ObjectId, ObjectKind,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringInfo {
    pub object_id: ObjectId,
    pub value: String,
    pub byte_length: u64,
    pub retained_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateStringGroup {
    pub value: String,
    pub count: usize,
    pub total_wasted_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringReport {
    pub total_strings: usize,
    pub total_string_bytes: u64,
    pub unique_strings: usize,
    pub duplicate_groups: Vec<DuplicateStringGroup>,
    pub total_duplicate_waste: u64,
    pub top_strings_by_size: Vec<StringInfo>,
}

pub fn analyze_strings(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    top_n: usize,
    min_duplicate_count: usize,
) -> StringReport {
    let mut strings = Vec::new();

    for object in graph.objects.values() {
        let Some(class_name) = graph.class_name(object.class_id) else {
            continue;
        };
        if !matches_class_name(class_name, "java.lang.String") {
            continue;
        }

        let Some((value, byte_length)) = decode_string_object(graph, object) else {
            continue;
        };

        strings.push(StringInfo {
            object_id: object.id,
            value,
            byte_length,
            retained_bytes: dominator.map(|dom| dom.retained_size(object.id)),
        });
    }

    let total_strings = strings.len();
    let total_string_bytes = strings.iter().map(|info| info.byte_length).sum();

    let mut grouped: HashMap<String, (usize, u64)> = HashMap::new();
    for info in &strings {
        let entry = grouped
            .entry(info.value.clone())
            .or_insert((0, info.byte_length));
        entry.0 += 1;
        entry.1 = entry.1.max(info.byte_length);
    }

    let unique_strings = grouped.len();
    let mut duplicate_groups: Vec<DuplicateStringGroup> = grouped
        .into_iter()
        .filter_map(|(value, (count, byte_length))| {
            if count < min_duplicate_count {
                return None;
            }

            Some(DuplicateStringGroup {
                value,
                count,
                total_wasted_bytes: (count.saturating_sub(1) as u64) * byte_length,
            })
        })
        .collect();

    duplicate_groups.sort_by(|left, right| {
        right
            .total_wasted_bytes
            .cmp(&left.total_wasted_bytes)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.value.cmp(&right.value))
    });

    let total_duplicate_waste = duplicate_groups
        .iter()
        .map(|group| group.total_wasted_bytes)
        .sum();

    strings.sort_by(|left, right| {
        right
            .byte_length
            .cmp(&left.byte_length)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });
    strings.truncate(top_n);

    StringReport {
        total_strings,
        total_string_bytes,
        unique_strings,
        duplicate_groups,
        total_duplicate_waste,
        top_strings_by_size: strings,
    }
}

pub(crate) fn extract_string_value(graph: &ObjectGraph, string_id: ObjectId) -> Option<String> {
    let object = graph.objects.get(&string_id)?;
    let (value, _) = decode_string_object(graph, object)?;
    Some(value)
}

fn decode_string_object(graph: &ObjectGraph, object: &HeapObject) -> Option<(String, u64)> {
    let value_ref = match read_field(object, &graph.classes, "value", graph.identifier_size)? {
        FieldValue::ObjectRef(Some(array_id)) => array_id,
        _ => return None,
    };
    let array_object = graph.objects.get(&value_ref)?;
    if array_object.field_data.is_empty() {
        return None;
    }

    let coder = match read_field(object, &graph.classes, "coder", graph.identifier_size) {
        Some(FieldValue::Byte(value)) => Some(value),
        Some(FieldValue::Int(value)) => i8::try_from(value).ok(),
        _ => None,
    };

    decode_backing_array(array_object, coder)
}

fn decode_backing_array(array_object: &HeapObject, coder: Option<i8>) -> Option<(String, u64)> {
    match array_object.kind {
        ObjectKind::PrimitiveArray {
            element_type: field_types::CHAR,
            ..
        } => Some((
            decode_utf16_bytes(&array_object.field_data),
            (array_object.field_data.len() / 2) as u64,
        )),
        ObjectKind::PrimitiveArray {
            element_type: field_types::BYTE,
            ..
        } => {
            let value = if coder == Some(1) {
                decode_utf16_bytes(&array_object.field_data)
            } else {
                decode_latin1_bytes(&array_object.field_data)
            };
            let byte_length = if coder == Some(1) {
                (array_object.field_data.len() / 2) as u64
            } else {
                array_object.field_data.len() as u64
            };
            Some((value, byte_length))
        }
        _ => None,
    }
}

fn decode_utf16_bytes(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode_latin1_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

fn matches_class_name(actual: &str, expected: &str) -> bool {
    actual == expected || actual.replace('/', ".") == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, HeapObject, ObjectGraph, ObjectKind,
    };

    fn add_class(
        graph: &mut ObjectGraph,
        class_id: ObjectId,
        super_class_id: ObjectId,
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

        graph.objects.insert(
            10,
            HeapObject {
                id: 10,
                class_id: 2,
                shallow_size: 24,
                references: vec![100],
                field_data: string_field_bytes(100, 1),
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
                field_data: string_field_bytes(101, 1),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            12,
            HeapObject {
                id: 12,
                class_id: 2,
                shallow_size: 24,
                references: vec![102],
                field_data: string_field_bytes(102, 0),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            13,
            HeapObject {
                id: 13,
                class_id: 2,
                shallow_size: 24,
                references: vec![103],
                field_data: string_field_bytes(103, 1),
                kind: ObjectKind::Instance,
            },
        );

        graph.objects.insert(
            100,
            HeapObject {
                id: 100,
                class_id: 0,
                shallow_size: 10,
                references: Vec::new(),
                field_data: vec![0x00, 0x68, 0x00, 0x69],
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::CHAR,
                    length: 2,
                },
            },
        );
        graph.objects.insert(
            101,
            HeapObject {
                id: 101,
                class_id: 0,
                shallow_size: 10,
                references: Vec::new(),
                field_data: vec![0x00, 0x68, 0x00, 0x69],
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::CHAR,
                    length: 2,
                },
            },
        );
        graph.objects.insert(
            102,
            HeapObject {
                id: 102,
                class_id: 0,
                shallow_size: 8,
                references: Vec::new(),
                field_data: b"hola".to_vec(),
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::BYTE,
                    length: 4,
                },
            },
        );
        graph.objects.insert(
            103,
            HeapObject {
                id: 103,
                class_id: 0,
                shallow_size: 8,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::BYTE,
                    length: 4,
                },
            },
        );

        graph
    }

    #[test]
    fn analyze_strings_extracts_values_and_duplicate_waste() {
        let graph = make_graph();
        let report = analyze_strings(&graph, None, 2, 2);

        assert_eq!(report.total_strings, 3);
        assert_eq!(report.total_string_bytes, 8);
        assert_eq!(report.unique_strings, 2);
        assert_eq!(report.duplicate_groups.len(), 1);
        assert_eq!(report.duplicate_groups[0].value, "hi");
        assert_eq!(report.duplicate_groups[0].count, 2);
        assert_eq!(report.duplicate_groups[0].total_wasted_bytes, 2);
        assert_eq!(report.total_duplicate_waste, 2);
        assert_eq!(report.top_strings_by_size.len(), 2);
        assert_eq!(report.top_strings_by_size[0].value, "hola");
        assert_eq!(report.top_strings_by_size[0].byte_length, 4);
        assert_eq!(report.top_strings_by_size[1].value, "hi");
    }

    #[test]
    fn extract_string_value_returns_none_for_missing_backing_data() {
        let graph = make_graph();
        assert_eq!(extract_string_value(&graph, 10).as_deref(), Some("hi"));
        assert_eq!(extract_string_value(&graph, 13), None);
    }
}
