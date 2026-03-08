use crate::graph::DominatorTree;
use crate::hprof::{read_field, FieldValue, ObjectGraph, ObjectId, ObjectKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub object_id: ObjectId,
    pub collection_type: String,
    pub size: usize,
    pub capacity: Option<usize>,
    pub fill_ratio: Option<f64>,
    pub shallow_bytes: u64,
    pub retained_bytes: Option<u64>,
    pub waste_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionTypeSummary {
    pub count: usize,
    pub total_shallow: u64,
    pub total_retained: u64,
    pub total_waste: u64,
    pub avg_fill_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionReport {
    pub total_collections: usize,
    pub total_waste_bytes: u64,
    pub empty_collections: usize,
    pub oversized_collections: Vec<CollectionInfo>,
    pub summary_by_type: HashMap<String, CollectionTypeSummary>,
}

pub fn inspect_collections(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    min_capacity: usize,
) -> CollectionReport {
    let hashset_backing_maps = collect_hashset_backing_maps(graph);
    let mut collections = Vec::new();

    for object in graph.objects.values() {
        let Some(class_name) = graph.class_name(object.class_id) else {
            continue;
        };
        let normalized = normalize_class_name(class_name);

        let info = match normalized.as_str() {
            "java.util.HashMap" if !hashset_backing_maps.contains(&object.id) => {
                inspect_hash_map_like(
                    graph,
                    dominator,
                    object.id,
                    object.id,
                    normalized,
                    min_capacity,
                )
            }
            "java.util.ArrayList" => inspect_array_list(graph, dominator, object.id, min_capacity),
            "java.util.HashSet" => inspect_hash_set(graph, dominator, object.id, min_capacity),
            "java.util.concurrent.ConcurrentHashMap" => {
                inspect_concurrent_hash_map(graph, dominator, object.id, min_capacity)
            }
            _ => None,
        };

        if let Some(info) = info {
            collections.push(info);
        }
    }

    collections.sort_by(|left, right| {
        right
            .waste_bytes
            .cmp(&left.waste_bytes)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let total_collections = collections.len();
    let total_waste_bytes = collections.iter().map(|info| info.waste_bytes).sum();
    let empty_collections = collections.iter().filter(|info| info.size == 0).count();
    let oversized_collections = collections
        .iter()
            .filter(|info| info.fill_ratio.is_some_and(|ratio| ratio <= 0.25))
        .cloned()
        .collect();

    let mut summary_by_type: HashMap<String, CollectionTypeSummary> = HashMap::new();
    let mut fill_ratio_totals: HashMap<String, (f64, usize)> = HashMap::new();
    for info in &collections {
        let entry = summary_by_type
            .entry(info.collection_type.clone())
            .or_insert(CollectionTypeSummary {
                count: 0,
                total_shallow: 0,
                total_retained: 0,
                total_waste: 0,
                avg_fill_ratio: 0.0,
            });

        entry.count += 1;
        entry.total_shallow += info.shallow_bytes;
        entry.total_retained += info.retained_bytes.unwrap_or(0);
        entry.total_waste += info.waste_bytes;

        if let Some(fill_ratio) = info.fill_ratio {
            let ratio_entry = fill_ratio_totals
                .entry(info.collection_type.clone())
                .or_insert((0.0, 0));
            ratio_entry.0 += fill_ratio;
            ratio_entry.1 += 1;
        }
    }

    for (collection_type, summary) in &mut summary_by_type {
        if let Some((ratio_sum, ratio_count)) = fill_ratio_totals.get(collection_type) {
            if *ratio_count > 0 {
                summary.avg_fill_ratio = ratio_sum / *ratio_count as f64;
            }
        }
    }

    CollectionReport {
        total_collections,
        total_waste_bytes,
        empty_collections,
        oversized_collections,
        summary_by_type,
    }
}

fn collect_hashset_backing_maps(graph: &ObjectGraph) -> HashSet<ObjectId> {
    graph
        .objects
        .values()
        .filter_map(|object| {
            let class_name = graph.class_name(object.class_id)?;
            if normalize_class_name(class_name) != "java.util.HashSet" {
                return None;
            }

            match read_field(object, &graph.classes, "map", graph.identifier_size) {
                Some(FieldValue::ObjectRef(Some(map_id))) => Some(map_id),
                _ => None,
            }
        })
        .collect()
}

fn inspect_hash_map_like(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    collection_object_id: ObjectId,
    map_object_id: ObjectId,
    collection_type: String,
    min_capacity: usize,
) -> Option<CollectionInfo> {
    let map_object = graph.objects.get(&map_object_id)?;
    let size = int_like_to_usize(read_field(
        map_object,
        &graph.classes,
        "size",
        graph.identifier_size,
    )?)?;
    let capacity = capacity_from_named_array_field(graph, map_object, "table")?;
    build_collection_info(
        graph,
        dominator,
        collection_object_id,
        collection_type,
        size,
        capacity,
        min_capacity,
    )
}

fn inspect_array_list(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    min_capacity: usize,
) -> Option<CollectionInfo> {
    let object = graph.objects.get(&object_id)?;
    let size = int_like_to_usize(read_field(
        object,
        &graph.classes,
        "size",
        graph.identifier_size,
    )?)?;
    let capacity = capacity_from_named_array_field(graph, object, "elementData")?;
    build_collection_info(
        graph,
        dominator,
        object_id,
        String::from("java.util.ArrayList"),
        size,
        capacity,
        min_capacity,
    )
}

fn inspect_hash_set(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    min_capacity: usize,
) -> Option<CollectionInfo> {
    let object = graph.objects.get(&object_id)?;
    let map_id = match read_field(object, &graph.classes, "map", graph.identifier_size)? {
        FieldValue::ObjectRef(Some(map_id)) => map_id,
        _ => return None,
    };

    inspect_hash_map_like(
        graph,
        dominator,
        object_id,
        map_id,
        String::from("java.util.HashSet"),
        min_capacity,
    )
}

fn inspect_concurrent_hash_map(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    min_capacity: usize,
) -> Option<CollectionInfo> {
    let object = graph.objects.get(&object_id)?;
    let table_id = match read_field(object, &graph.classes, "table", graph.identifier_size)? {
        FieldValue::ObjectRef(Some(table_id)) => table_id,
        _ => return None,
    };
    let capacity = array_length(graph, table_id)?;
    let table_object = graph.objects.get(&table_id)?;
    let size = match read_field(object, &graph.classes, "baseCount", graph.identifier_size) {
        Some(FieldValue::Long(value)) => usize::try_from(value).ok()?,
        Some(FieldValue::Int(value)) => usize::try_from(value).ok()?,
        _ => table_object.references.len(),
    };

    build_collection_info(
        graph,
        dominator,
        object_id,
        String::from("java.util.concurrent.ConcurrentHashMap"),
        size,
        capacity,
        min_capacity,
    )
}

fn build_collection_info(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    collection_type: String,
    size: usize,
    capacity: usize,
    min_capacity: usize,
) -> Option<CollectionInfo> {
    if capacity < min_capacity {
        return None;
    }

    let shallow_bytes = u64::from(graph.objects.get(&object_id)?.shallow_size);
    let retained_bytes = dominator.map(|dom| dom.retained_size(object_id));
    let fill_ratio = Some(size as f64 / capacity.max(1) as f64);
    let waste_slots = capacity.saturating_sub(size);
    let waste_bytes = waste_slots as u64 * u64::from(graph.identifier_size);

    Some(CollectionInfo {
        object_id,
        collection_type,
        size,
        capacity: Some(capacity),
        fill_ratio,
        shallow_bytes,
        retained_bytes,
        waste_bytes,
    })
}

fn capacity_from_named_array_field(
    graph: &ObjectGraph,
    object: &crate::hprof::HeapObject,
    field_name: &str,
) -> Option<usize> {
    let array_id = match read_field(object, &graph.classes, field_name, graph.identifier_size)? {
        FieldValue::ObjectRef(Some(array_id)) => array_id,
        _ => return None,
    };
    array_length(graph, array_id)
}

fn array_length(graph: &ObjectGraph, array_id: ObjectId) -> Option<usize> {
    let array_object = graph.objects.get(&array_id)?;
    match array_object.kind {
        ObjectKind::ObjectArray { length } | ObjectKind::PrimitiveArray { length, .. } => {
            usize::try_from(length).ok()
        }
        ObjectKind::Instance => None,
    }
}

fn int_like_to_usize(value: FieldValue) -> Option<usize> {
    match value {
        FieldValue::Int(value) => usize::try_from(value).ok(),
        FieldValue::Long(value) => usize::try_from(value).ok(),
        _ => None,
    }
}

fn normalize_class_name(name: &str) -> String {
    name.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectKind,
    };

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str, fields: Vec<FieldDescriptor>) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 0,
                name: Some(name.into()),
                instance_fields: fields,
                static_references: Vec::new(),
            },
        );
    }

    fn ref_bytes(id: u64) -> Vec<u8> {
        id.to_be_bytes().to_vec()
    }

    fn int_bytes(value: i32) -> Vec<u8> {
        value.to_be_bytes().to_vec()
    }

    fn long_bytes(value: i64) -> Vec<u8> {
        value.to_be_bytes().to_vec()
    }

    fn make_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            1,
            "java.util.HashMap",
            vec![
                FieldDescriptor {
                    name: Some("size".into()),
                    field_type: field_types::INT,
                },
                FieldDescriptor {
                    name: Some("table".into()),
                    field_type: field_types::OBJECT,
                },
            ],
        );
        add_class(
            &mut graph,
            2,
            "java.util.ArrayList",
            vec![
                FieldDescriptor {
                    name: Some("size".into()),
                    field_type: field_types::INT,
                },
                FieldDescriptor {
                    name: Some("elementData".into()),
                    field_type: field_types::OBJECT,
                },
            ],
        );
        add_class(
            &mut graph,
            3,
            "java.util.HashSet",
            vec![FieldDescriptor {
                name: Some("map".into()),
                field_type: field_types::OBJECT,
            }],
        );
        add_class(
            &mut graph,
            4,
            "java.util.concurrent.ConcurrentHashMap",
            vec![
                FieldDescriptor {
                    name: Some("table".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("baseCount".into()),
                    field_type: field_types::LONG,
                },
            ],
        );

        graph.objects.insert(
            10,
            HeapObject {
                id: 10,
                class_id: 1,
                shallow_size: 48,
                references: vec![100],
                field_data: [int_bytes(2), ref_bytes(100)].concat(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            11,
            HeapObject {
                id: 11,
                class_id: 2,
                shallow_size: 32,
                references: vec![101],
                field_data: [int_bytes(0), ref_bytes(101)].concat(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            12,
            HeapObject {
                id: 12,
                class_id: 3,
                shallow_size: 24,
                references: vec![13],
                field_data: ref_bytes(13),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            13,
            HeapObject {
                id: 13,
                class_id: 1,
                shallow_size: 48,
                references: vec![102],
                field_data: [int_bytes(1), ref_bytes(102)].concat(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            14,
            HeapObject {
                id: 14,
                class_id: 4,
                shallow_size: 56,
                references: vec![103],
                field_data: [ref_bytes(103), long_bytes(3)].concat(),
                kind: ObjectKind::Instance,
            },
        );

        graph.objects.insert(
            100,
            HeapObject {
                id: 100,
                class_id: 0,
                shallow_size: 80,
                references: vec![200, 201],
                field_data: Vec::new(),
                kind: ObjectKind::ObjectArray { length: 16 },
            },
        );
        graph.objects.insert(
            101,
            HeapObject {
                id: 101,
                class_id: 0,
                shallow_size: 32,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::ObjectArray { length: 4 },
            },
        );
        graph.objects.insert(
            102,
            HeapObject {
                id: 102,
                class_id: 0,
                shallow_size: 48,
                references: vec![300],
                field_data: Vec::new(),
                kind: ObjectKind::ObjectArray { length: 8 },
            },
        );
        graph.objects.insert(
            103,
            HeapObject {
                id: 103,
                class_id: 0,
                shallow_size: 96,
                references: vec![400, 401, 402],
                field_data: Vec::new(),
                kind: ObjectKind::ObjectArray { length: 12 },
            },
        );

        graph.gc_roots.push(GcRoot {
            object_id: 10,
            root_type: GcRootType::StickyClass,
        });
        graph.gc_roots.push(GcRoot {
            object_id: 11,
            root_type: GcRootType::StickyClass,
        });
        graph.gc_roots.push(GcRoot {
            object_id: 12,
            root_type: GcRootType::StickyClass,
        });
        graph.gc_roots.push(GcRoot {
            object_id: 14,
            root_type: GcRootType::StickyClass,
        });

        graph
    }

    #[test]
    fn inspect_collections_reports_supported_types() {
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let report = inspect_collections(&graph, Some(&dominator), 4);

        assert_eq!(report.total_collections, 4);
        assert_eq!(report.empty_collections, 1);
        assert_eq!(report.total_waste_bytes, (14 + 4 + 7 + 9) * 8);
        assert_eq!(report.oversized_collections.len(), 4);

        let hashmap = report
            .summary_by_type
            .get("java.util.HashMap")
            .expect("hashmap summary");
        assert_eq!(hashmap.count, 1);
        assert_eq!(hashmap.total_waste, 14 * 8);

        let hashset = report
            .summary_by_type
            .get("java.util.HashSet")
            .expect("hashset summary");
        assert_eq!(hashset.count, 1);
        assert!((hashset.avg_fill_ratio - 0.125).abs() < f64::EPSILON);

        let chm = report
            .summary_by_type
            .get("java.util.concurrent.ConcurrentHashMap")
            .expect("concurrent hashmap summary");
        assert_eq!(chm.count, 1);
        assert!((chm.avg_fill_ratio - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn inspect_collections_skips_small_capacities() {
        let graph = make_graph();
        let report = inspect_collections(&graph, None, 10);

        assert_eq!(report.total_collections, 2);
        assert!(report.summary_by_type.contains_key("java.util.HashMap"));
        assert!(report
            .summary_by_type
            .contains_key("java.util.concurrent.ConcurrentHashMap"));
        assert!(!report.summary_by_type.contains_key("java.util.ArrayList"));
        assert!(!report.summary_by_type.contains_key("java.util.HashSet"));
    }
}
