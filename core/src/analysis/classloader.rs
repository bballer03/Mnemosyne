use crate::graph::DominatorTree;
use crate::hprof::{read_field, FieldValue, ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const LEAK_RETAINED_THRESHOLD_BYTES: u64 = 8 * 1024 * 1024;
const LEAK_MAX_CLASS_COUNT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderInfo {
    pub object_id: ObjectId,
    pub class_name: String,
    pub loaded_class_count: usize,
    pub instance_count: usize,
    pub total_shallow_bytes: u64,
    pub retained_bytes: Option<u64>,
    pub parent_loader: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderLeakCandidate {
    pub object_id: ObjectId,
    pub class_name: String,
    pub retained_bytes: u64,
    pub loaded_class_count: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClassLoaderReport {
    pub loaders: Vec<ClassLoaderInfo>,
    pub potential_leaks: Vec<ClassLoaderLeakCandidate>,
}

#[derive(Default)]
struct LoaderAggregate {
    loaded_class_count: usize,
    instance_count: usize,
    total_shallow_bytes: u64,
}

pub fn analyze_classloaders(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> ClassLoaderReport {
    let mut by_loader: HashMap<ObjectId, LoaderAggregate> = HashMap::new();

    for class_info in graph.classes.values() {
        if class_info.class_loader_id == 0 {
            continue;
        }

        by_loader
            .entry(class_info.class_loader_id)
            .or_default()
            .loaded_class_count += 1;
    }

    for object in graph.objects.values() {
        let Some(class_info) = graph.classes.get(&object.class_id) else {
            continue;
        };
        if class_info.class_loader_id == 0 {
            continue;
        }

        let aggregate = by_loader.entry(class_info.class_loader_id).or_default();
        aggregate.instance_count += 1;
        aggregate.total_shallow_bytes += u64::from(object.shallow_size);
    }

    let mut loaders: Vec<ClassLoaderInfo> = by_loader
        .into_iter()
        .map(|(loader_id, aggregate)| build_loader_info(graph, dominator, loader_id, aggregate))
        .collect();

    loaders.sort_by(|left, right| {
        right
            .retained_bytes
            .unwrap_or(0)
            .cmp(&left.retained_bytes.unwrap_or(0))
            .then_with(|| right.total_shallow_bytes.cmp(&left.total_shallow_bytes))
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let potential_leaks = loaders.iter().filter_map(build_leak_candidate).collect();

    ClassLoaderReport {
        loaders,
        potential_leaks,
    }
}

fn build_loader_info(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    loader_id: ObjectId,
    aggregate: LoaderAggregate,
) -> ClassLoaderInfo {
    let loader_object = graph.objects.get(&loader_id);
    let class_name = loader_object
        .and_then(|loader| graph.class_name(loader.class_id))
        .map(normalize_class_name)
        .unwrap_or_else(|| format!("<loader:{loader_id}>"));
    let parent_loader = loader_object.and_then(|loader| {
        match read_field(loader, &graph.classes, "parent", graph.identifier_size) {
            Some(FieldValue::ObjectRef(Some(parent_id))) => Some(parent_id),
            _ => None,
        }
    });

    ClassLoaderInfo {
        object_id: loader_id,
        class_name,
        loaded_class_count: aggregate.loaded_class_count,
        instance_count: aggregate.instance_count,
        total_shallow_bytes: aggregate.total_shallow_bytes,
        retained_bytes: dominator.map(|dom| dom.retained_size(loader_id)),
        parent_loader,
    }
}

fn build_leak_candidate(loader: &ClassLoaderInfo) -> Option<ClassLoaderLeakCandidate> {
    let retained_bytes = loader.retained_bytes?;
    if retained_bytes < LEAK_RETAINED_THRESHOLD_BYTES {
        return None;
    }
    if loader.loaded_class_count > LEAK_MAX_CLASS_COUNT {
        return None;
    }

    Some(ClassLoaderLeakCandidate {
        object_id: loader.object_id,
        class_name: loader.class_name.clone(),
        retained_bytes,
        loaded_class_count: loader.loaded_class_count,
        reason: format!(
            "Retains {:.2} MB but loads only {} classes",
            retained_bytes as f64 / (1024.0 * 1024.0),
            loader.loaded_class_count
        ),
    })
}

fn normalize_class_name(name: &str) -> String {
    name.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectKind};

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str, class_loader_id: u64) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
    }

    fn add_object(
        graph: &mut ObjectGraph,
        object_id: u64,
        class_id: u64,
        shallow_size: u32,
        references: &[u64],
    ) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size,
                references: references.to_vec(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
    }

    fn add_root(graph: &mut ObjectGraph, object_id: u64) {
        graph.gc_roots.push(GcRoot {
            object_id,
            root_type: GcRootType::StickyClass,
        });
    }

    /// GC root -> Loader(1, com.example.Loader, bootstrap) -> BigLeaked(2,
    /// com.example.BigLeaked, loaded by loader 1, ~9 MB shallow). Loader 1
    /// declares exactly one class (BigLeaked) -- above the retained-bytes
    /// threshold and at/under the loaded-class-count ceiling, so it must be
    /// flagged as a potential leak.
    fn build_leaky_loader_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.Loader", 0);
        add_class(&mut graph, 200, "com.example.BigLeaked", 1);

        add_object(&mut graph, 1, 100, 16, &[2]);
        add_object(&mut graph, 2, 200, 9_000_000, &[]);
        add_root(&mut graph, 1);

        graph
    }

    #[test]
    fn loader_with_few_classes_and_high_retained_size_is_flagged() {
        let graph = build_leaky_loader_graph();
        let dominator = build_dominator_tree(&graph);

        let report = analyze_classloaders(&graph, Some(&dominator));

        assert_eq!(report.potential_leaks.len(), 1);
        let leak = &report.potential_leaks[0];
        assert_eq!(leak.object_id, 1);
        assert_eq!(leak.class_name, "com.example.Loader");
        assert_eq!(leak.loaded_class_count, 1);
        assert!(leak.retained_bytes >= LEAK_RETAINED_THRESHOLD_BYTES);
        assert!(leak.reason.contains("Retains"));
    }

    #[test]
    fn loader_below_retained_threshold_is_not_flagged() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.SmallLoader", 0);
        add_class(&mut graph, 200, "com.example.Small", 1);
        add_object(&mut graph, 1, 100, 16, &[2]);
        add_object(&mut graph, 2, 200, 1_024, &[]);
        add_root(&mut graph, 1);

        let dominator = build_dominator_tree(&graph);
        let report = analyze_classloaders(&graph, Some(&dominator));

        assert!(report.potential_leaks.is_empty());
    }

    #[test]
    fn loader_with_many_classes_above_ceiling_is_not_flagged_even_if_large() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.BusyLoader", 0);
        let mut references = Vec::new();
        for i in 0..5u64 {
            let class_id = 200 + i;
            add_class(&mut graph, class_id, &format!("com.example.Class{i}"), 1);
            let object_id = 10 + i;
            add_object(&mut graph, object_id, class_id, 2_000_000, &[]);
            references.push(object_id);
        }
        add_object(&mut graph, 1, 100, 16, &references);
        add_root(&mut graph, 1);

        let dominator = build_dominator_tree(&graph);
        let report = analyze_classloaders(&graph, Some(&dominator));

        assert!(report.potential_leaks.is_empty());
    }

    #[test]
    fn no_dominator_tree_yields_no_leak_candidates() {
        let graph = build_leaky_loader_graph();

        let report = analyze_classloaders(&graph, None);

        assert!(report.potential_leaks.is_empty());
    }
}
