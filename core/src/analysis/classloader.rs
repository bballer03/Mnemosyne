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
