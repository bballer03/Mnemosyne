use crate::graph::DominatorTree;
use crate::hprof::{ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LargestInstance {
    pub object_id: ObjectId,
    pub class_name: String,
    pub shallow_size: u64,
    pub retained_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopInstancesReport {
    pub instances: Vec<LargestInstance>,
    pub total_count: usize,
}

pub fn find_top_instances(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    top_n: usize,
) -> TopInstancesReport {
    let mut instances: Vec<LargestInstance> = graph
        .objects
        .values()
        .map(|object| LargestInstance {
            object_id: object.id,
            class_name: graph
                .class_name(object.class_id)
                .unwrap_or("<unknown>")
                .to_string(),
            shallow_size: u64::from(object.shallow_size),
            retained_size: dominator.map(|dom| dom.retained_size(object.id)),
        })
        .collect();

    instances.sort_by(|left, right| {
        let left_primary = dominator
            .map(|_| left.retained_size.unwrap_or(0))
            .unwrap_or(left.shallow_size);
        let right_primary = dominator
            .map(|_| right.retained_size.unwrap_or(0))
            .unwrap_or(right.shallow_size);

        right_primary
            .cmp(&left_primary)
            .then_with(|| right.shallow_size.cmp(&left.shallow_size))
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let total_count = instances.len();
    instances.truncate(top_n);

    TopInstancesReport {
        instances,
        total_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{
        ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind,
    };

    fn make_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        graph.classes.insert(
            100,
            ClassInfo {
                class_obj_id: 100,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 0,
                name: Some("com.example.Root".into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
        graph.classes.insert(
            200,
            ClassInfo {
                class_obj_id: 200,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 0,
                name: Some("com.example.Payload".into()),
                instance_fields: Vec::<FieldDescriptor>::new(),
                static_references: Vec::new(),
            },
        );

        graph.objects.insert(
            1,
            HeapObject {
                id: 1,
                class_id: 100,
                shallow_size: 10,
                references: vec![2, 3],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            2,
            HeapObject {
                id: 2,
                class_id: 200,
                shallow_size: 40,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            3,
            HeapObject {
                id: 3,
                class_id: 200,
                shallow_size: 30,
                references: vec![4],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            4,
            HeapObject {
                id: 4,
                class_id: 200,
                shallow_size: 50,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );

        graph.gc_roots.push(GcRoot {
            object_id: 1,
            root_type: GcRootType::StickyClass,
        });

        graph
    }

    #[test]
    fn top_instances_uses_shallow_size_without_dominator() {
        let graph = make_graph();
        let report = find_top_instances(&graph, None, 2);

        assert_eq!(report.total_count, 4);
        assert_eq!(report.instances.len(), 2);
        assert_eq!(report.instances[0].object_id, 4);
        assert_eq!(report.instances[0].shallow_size, 50);
        assert_eq!(report.instances[0].retained_size, None);
        assert_eq!(report.instances[1].object_id, 2);
    }

    #[test]
    fn top_instances_uses_retained_size_with_dominator() {
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);
        let report = find_top_instances(&graph, Some(&dominator), 3);

        assert_eq!(report.total_count, 4);
        assert_eq!(report.instances.len(), 3);
        assert_eq!(report.instances[0].object_id, 1);
        assert_eq!(report.instances[0].retained_size, Some(130));
        assert_eq!(report.instances[1].object_id, 3);
        assert_eq!(report.instances[1].retained_size, Some(80));
        assert_eq!(report.instances[2].object_id, 4);
        assert_eq!(report.instances[2].retained_size, Some(50));
    }
}
