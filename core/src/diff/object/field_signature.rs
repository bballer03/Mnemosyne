use std::collections::{BTreeMap, HashMap};

use crate::hprof::{
    field_types, field_value_size, ClassId, ClassInfo, FieldDescriptor, HeapObject, ObjectGraph,
    ObjectId,
};

const OTHER_CLASS_BUCKET: &str = "<other>";
const UNKNOWN_CLASS_BUCKET: &str = "<unknown>";
const MAX_DISTINCT_OUTBOUND_CLASSES: usize = 64;

pub(crate) fn field_shape_signature(graph: &ObjectGraph, obj_id: ObjectId) -> u64 {
    let Some(object) = graph.get_object(obj_id) else {
        return 0;
    };

    let mut fields = field_shape_entries(object, graph);
    fields.sort_unstable();

    let mut bytes = Vec::new();
    for (field_name, field_type, null_bit) in fields {
        bytes.extend_from_slice(field_name.as_bytes());
        bytes.push(0);
        bytes.push(field_type);
        bytes.push(null_bit);
    }

    xxhash_rust::xxh64::xxh64(&bytes, 0)
}

pub(crate) fn outbound_class_set_signature(graph: &ObjectGraph, obj_id: ObjectId) -> u64 {
    let Some(object) = graph.get_object(obj_id) else {
        return 0;
    };

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for &reference_id in &object.references {
        let class_name = graph
            .get_object(reference_id)
            .and_then(|reference| graph.class_name(reference.class_id))
            .unwrap_or(UNKNOWN_CLASS_BUCKET)
            .to_string();
        *counts.entry(class_name).or_insert(0) += 1;
    }

    if counts.len() > MAX_DISTINCT_OUTBOUND_CLASSES {
        let mut compacted = BTreeMap::new();
        let mut overflow = 0_u64;

        for (index, (class_name, count)) in counts.into_iter().enumerate() {
            if index < MAX_DISTINCT_OUTBOUND_CLASSES - 1 {
                compacted.insert(class_name, count);
            } else {
                overflow += count;
            }
        }

        if overflow > 0 {
            compacted.insert(OTHER_CLASS_BUCKET.to_string(), overflow);
        }

        counts = compacted;
    }

    let mut bytes = Vec::new();
    for (class_name, count) in counts {
        bytes.extend_from_slice(class_name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&count.to_le_bytes());
    }

    xxhash_rust::xxh64::xxh64(&bytes, 0)
}

fn field_shape_entries(object: &HeapObject, graph: &ObjectGraph) -> Vec<(String, u8, u8)> {
    let layout = class_field_layout(&graph.classes, object.class_id);
    let mut offset = 0_usize;
    let mut entries = Vec::with_capacity(layout.len());

    for (index, descriptor) in layout.into_iter().enumerate() {
        let name = descriptor
            .name
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("<unnamed_field_{index}>"));
        let null_bit =
            object_reference_null_bit(object, graph.identifier_size, offset, &descriptor);

        entries.push((name, descriptor.field_type, null_bit));

        if let Some(width) = field_value_size(descriptor.field_type, graph.identifier_size) {
            offset += usize::from(width);
        }
    }

    entries
}

fn object_reference_null_bit(
    object: &HeapObject,
    identifier_size: u8,
    offset: usize,
    descriptor: &FieldDescriptor,
) -> u8 {
    if descriptor.field_type != field_types::OBJECT {
        return u8::MAX;
    }

    let Some(width) = field_value_size(descriptor.field_type, identifier_size).map(usize::from)
    else {
        return u8::MAX;
    };
    if offset + width > object.field_data.len() {
        return u8::MAX;
    }

    let reference = match identifier_size {
        4 => u32::from_le_bytes([0, 0, 0, 0]),
        8 => 0,
        _ => 0,
    };
    let _ = reference;

    let is_null = match identifier_size {
        4 => {
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&object.field_data[offset..offset + width]);
            u32::from_be_bytes(bytes) == 0
        }
        8 => {
            let mut bytes = [0_u8; 8];
            bytes.copy_from_slice(&object.field_data[offset..offset + width]);
            u64::from_be_bytes(bytes) == 0
        }
        _ => true,
    };

    if is_null {
        0
    } else {
        1
    }
}

fn class_field_layout(
    classes: &HashMap<ClassId, ClassInfo>,
    class_id: ClassId,
) -> Vec<FieldDescriptor> {
    fn collect(
        classes: &HashMap<ClassId, ClassInfo>,
        class_id: ClassId,
        fields: &mut Vec<FieldDescriptor>,
    ) {
        if class_id == 0 {
            return;
        }

        let Some(class_info) = classes.get(&class_id) else {
            return;
        };

        collect(classes, class_info.super_class_id, fields);
        fields.extend(class_info.instance_fields.iter().cloned());
    }

    let mut fields = Vec::new();
    collect(classes, class_id, &mut fields);
    fields
}

#[cfg(test)]
mod tests {
    use super::{field_shape_signature, outbound_class_set_signature};
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, HeapObject, ObjectGraph, ObjectKind,
    };

    fn make_graph() -> ObjectGraph {
        ObjectGraph::new(4)
    }

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str, fields: Vec<FieldDescriptor>) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: fields,
                static_references: Vec::new(),
            },
        );
    }

    fn add_object(
        graph: &mut ObjectGraph,
        object_id: u64,
        class_id: u64,
        references: Vec<u64>,
        field_data: Vec<u8>,
    ) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size: 16,
                references,
                field_data,
                kind: ObjectKind::Instance,
            },
        );
    }

    #[test]
    fn field_shape_signature_changes_when_reference_nullness_changes() {
        let mut graph = make_graph();
        add_class(
            &mut graph,
            10,
            "com.example.CacheEntry",
            vec![
                FieldDescriptor {
                    name: Some("next".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("count".into()),
                    field_type: field_types::INT,
                },
            ],
        );

        add_object(&mut graph, 1, 10, vec![99], vec![0, 0, 0, 99, 0, 0, 0, 7]);
        add_object(&mut graph, 2, 10, Vec::new(), vec![0, 0, 0, 0, 0, 0, 0, 7]);

        let left_populated = field_shape_signature(&graph, 1);
        let left_null = field_shape_signature(&graph, 2);

        assert_ne!(left_populated, left_null);
    }

    #[test]
    fn outbound_class_set_signature_changes_when_outbound_classes_change() {
        let mut graph = make_graph();
        add_class(&mut graph, 10, "com.example.Root", Vec::new());
        add_class(&mut graph, 11, "com.example.CacheA", Vec::new());
        add_class(&mut graph, 12, "com.example.CacheB", Vec::new());
        add_class(&mut graph, 13, "com.example.CacheC", Vec::new());

        add_object(&mut graph, 1, 10, vec![11, 12], Vec::new());
        add_object(&mut graph, 2, 10, vec![11, 13], Vec::new());
        add_object(&mut graph, 11, 11, Vec::new(), Vec::new());
        add_object(&mut graph, 12, 12, Vec::new(), Vec::new());
        add_object(&mut graph, 13, 13, Vec::new(), Vec::new());

        let left = outbound_class_set_signature(&graph, 1);
        let right = outbound_class_set_signature(&graph, 2);

        assert_ne!(left, right);
    }
}
