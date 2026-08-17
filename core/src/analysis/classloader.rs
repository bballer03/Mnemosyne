use crate::graph::DominatorTree;
use crate::hprof::{read_field, FieldValue, ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

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
    pub unique_class_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderLeakCandidate {
    pub object_id: ObjectId,
    pub class_name: String,
    pub retained_bytes: u64,
    pub loaded_class_count: usize,
    pub reason: String,
}

/// A class name loaded by two or more distinct classloaders -- MAT's
/// "Duplicate Classes" signal, and the actual defining pattern of the
/// classic Tomcat/Jetty/Spring hot-redeploy classloader leak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateClassGroup {
    pub class_name: String,
    /// Sorted ascending for determinism.
    pub loader_object_ids: Vec<ObjectId>,
    pub loader_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClassLoaderReport {
    pub loaders: Vec<ClassLoaderInfo>,
    pub potential_leaks: Vec<ClassLoaderLeakCandidate>,
    pub duplicate_classes: Vec<DuplicateClassGroup>,
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

    // Single pass over graph.classes, grouped by normalized class name to
    // its distinct set of class_loader_id values. Both duplicate_classes and
    // every loader's unique_class_count are derived from this one grouping
    // rather than scanning graph.classes twice for two different purposes.
    let name_to_loaders = group_classes_by_normalized_name(graph);
    let unique_class_counts = compute_unique_class_counts(&name_to_loaders);
    let duplicate_classes = build_duplicate_groups(name_to_loaders);

    let mut loaders: Vec<ClassLoaderInfo> = by_loader
        .into_iter()
        .map(|(loader_id, aggregate)| {
            let unique_class_count = unique_class_counts.get(&loader_id).copied().unwrap_or(0);
            build_loader_info(graph, dominator, loader_id, aggregate, unique_class_count)
        })
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
        duplicate_classes,
    }
}

/// Cross-loader duplicate-class detection, standalone. Groups `graph.classes`
/// by normalized class name and keeps only groups where the set of distinct
/// `class_loader_id` values has size >= 2 -- a class loaded twice by the
/// exact same loader id is not a duplicate in this sense.
pub fn detect_duplicate_classes(graph: &ObjectGraph) -> Vec<DuplicateClassGroup> {
    build_duplicate_groups(group_classes_by_normalized_name(graph))
}

/// Groups `graph.classes` by normalized class name, collecting the distinct
/// `class_loader_id` values (sorted ascending) that declare a class object
/// under that name.
fn group_classes_by_normalized_name(graph: &ObjectGraph) -> HashMap<String, Vec<ObjectId>> {
    let mut grouping: HashMap<String, BTreeSet<ObjectId>> = HashMap::new();

    for class_info in graph.classes.values() {
        let Some(name) = class_info.name.as_deref() else {
            continue;
        };
        let normalized = normalize_class_name(name);
        grouping
            .entry(normalized)
            .or_default()
            .insert(class_info.class_loader_id);
    }

    grouping
        .into_iter()
        .map(|(name, loader_ids)| (name, loader_ids.into_iter().collect()))
        .collect()
}

/// For every class name owned by exactly one distinct loader, credit that
/// loader with one unique class.
fn compute_unique_class_counts(
    name_to_loaders: &HashMap<String, Vec<ObjectId>>,
) -> HashMap<ObjectId, usize> {
    let mut counts: HashMap<ObjectId, usize> = HashMap::new();

    for loader_ids in name_to_loaders.values() {
        if let [only_loader] = loader_ids.as_slice() {
            *counts.entry(*only_loader).or_insert(0) += 1;
        }
    }

    counts
}

fn build_duplicate_groups(
    name_to_loaders: HashMap<String, Vec<ObjectId>>,
) -> Vec<DuplicateClassGroup> {
    let mut groups: Vec<DuplicateClassGroup> = name_to_loaders
        .into_iter()
        .filter(|(_, loader_ids)| loader_ids.len() >= 2)
        .map(|(class_name, loader_object_ids)| DuplicateClassGroup {
            class_name,
            loader_count: loader_object_ids.len(),
            loader_object_ids,
        })
        .collect();

    groups.sort_by(|left, right| {
        right
            .loader_count
            .cmp(&left.loader_count)
            .then_with(|| left.class_name.cmp(&right.class_name))
    });

    groups
}

fn build_loader_info(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    loader_id: ObjectId,
    aggregate: LoaderAggregate,
    unique_class_count: usize,
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
        unique_class_count,
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

    /// Two distinct loader objects (1 and 2), each with their own class
    /// object for "com.example.webapp.RequestHandler" -- the classic
    /// "redeployed webapp" shape: same class name, two different
    /// classloaders, neither aware of the other's existence.
    fn build_redeployed_webapp_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        // Loader classes (declared by the bootstrap loader, id 0).
        add_class(&mut graph, 100, "com.example.webapp.WebappLoader", 0);
        add_class(&mut graph, 101, "com.example.webapp.WebappLoader", 0);

        // First generation: loader 1 loads RequestHandler (class obj 200).
        add_class(&mut graph, 200, "com/example/webapp/RequestHandler", 1);
        // Second generation: loader 2 loads its own RequestHandler (class obj 201).
        add_class(&mut graph, 201, "com/example/webapp/RequestHandler", 2);

        add_object(&mut graph, 1, 100, 16, &[10]);
        add_object(&mut graph, 2, 101, 16, &[11]);
        add_object(&mut graph, 10, 200, 64, &[]);
        add_object(&mut graph, 11, 201, 64, &[]);
        add_root(&mut graph, 1);
        add_root(&mut graph, 2);

        graph
    }

    #[test]
    fn class_loaded_by_two_distinct_loaders_is_one_duplicate_group() {
        let graph = build_redeployed_webapp_graph();

        let duplicates = detect_duplicate_classes(&graph);

        assert_eq!(duplicates.len(), 1);
        let group = &duplicates[0];
        assert_eq!(group.class_name, "com.example.webapp.RequestHandler");
        assert_eq!(group.loader_count, 2);
        assert_eq!(group.loader_object_ids, vec![1, 2]);
    }

    #[test]
    fn class_loaded_twice_by_same_loader_is_not_a_duplicate_group() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.SoloLoader", 0);
        // Two class objects, same normalized name, but declared by the SAME
        // loader (id 1) -- must not be treated as a cross-loader duplicate.
        add_class(&mut graph, 200, "com/example/Widget", 1);
        add_class(&mut graph, 201, "com/example/Widget", 1);

        add_object(&mut graph, 1, 100, 16, &[10, 11]);
        add_object(&mut graph, 10, 200, 32, &[]);
        add_object(&mut graph, 11, 201, 32, &[]);
        add_root(&mut graph, 1);

        let duplicates = detect_duplicate_classes(&graph);

        assert!(duplicates.is_empty());
    }

    #[test]
    fn build_leaky_loader_graph_fixture_has_no_duplicate_classes() {
        let graph = build_leaky_loader_graph();
        let dominator = build_dominator_tree(&graph);

        let report = analyze_classloaders(&graph, Some(&dominator));

        assert_eq!(report.potential_leaks.len(), 1);
        assert!(report.duplicate_classes.is_empty());
    }

    #[test]
    fn class_loaded_by_exactly_one_loader_never_appears_in_duplicate_classes() {
        let graph = build_redeployed_webapp_graph();

        let duplicates = detect_duplicate_classes(&graph);

        // WebappLoader itself (loaded once by the bootstrap loader, id 0)
        // must not appear anywhere in the duplicate groups.
        assert!(!duplicates
            .iter()
            .any(|group| group.class_name == "com.example.webapp.WebappLoader"));
    }

    /// Three loaders: loader 1 and loader 2 both load `Shared`, while loader
    /// 1 also loads `OnlyInOne` and loader 2 also loads `OnlyInTwo`. Loader 3
    /// loads only `OnlyInThree`, shared with nobody.
    fn build_three_loader_mixed_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.LoaderOne", 0);
        add_class(&mut graph, 101, "com.example.LoaderTwo", 0);
        add_class(&mut graph, 102, "com.example.LoaderThree", 0);

        add_class(&mut graph, 200, "com/example/Shared", 1);
        add_class(&mut graph, 201, "com/example/OnlyInOne", 1);
        add_class(&mut graph, 202, "com/example/Shared", 2);
        add_class(&mut graph, 203, "com/example/OnlyInTwo", 2);
        add_class(&mut graph, 204, "com/example/OnlyInThree", 3);

        add_object(&mut graph, 1, 100, 16, &[20, 21]);
        add_object(&mut graph, 2, 101, 16, &[22, 23]);
        add_object(&mut graph, 3, 102, 16, &[24]);
        add_object(&mut graph, 20, 200, 32, &[]);
        add_object(&mut graph, 21, 201, 32, &[]);
        add_object(&mut graph, 22, 202, 32, &[]);
        add_object(&mut graph, 23, 203, 32, &[]);
        add_object(&mut graph, 24, 204, 32, &[]);
        add_root(&mut graph, 1);
        add_root(&mut graph, 2);
        add_root(&mut graph, 3);

        graph
    }

    #[test]
    fn unique_class_count_excludes_shared_classes_on_three_loader_fixture() {
        let graph = build_three_loader_mixed_graph();

        let report = analyze_classloaders(&graph, None);

        let loader_one = report
            .loaders
            .iter()
            .find(|loader| loader.object_id == 1)
            .expect("loader 1 present");
        let loader_two = report
            .loaders
            .iter()
            .find(|loader| loader.object_id == 2)
            .expect("loader 2 present");
        let loader_three = report
            .loaders
            .iter()
            .find(|loader| loader.object_id == 3)
            .expect("loader 3 present");

        assert_eq!(loader_one.loaded_class_count, 2);
        assert_eq!(loader_one.unique_class_count, 1); // OnlyInOne, not Shared

        assert_eq!(loader_two.loaded_class_count, 2);
        assert_eq!(loader_two.unique_class_count, 1); // OnlyInTwo, not Shared

        assert_eq!(loader_three.loaded_class_count, 1);
        assert_eq!(loader_three.unique_class_count, 1); // OnlyInThree

        let duplicates = report.duplicate_classes;
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].class_name, "com.example.Shared");
        assert_eq!(duplicates[0].loader_count, 2);
        assert_eq!(duplicates[0].loader_object_ids, vec![1, 2]);
    }
}
