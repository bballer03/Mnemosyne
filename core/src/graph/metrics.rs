use super::dominator::{DominatorTree, VIRTUAL_ROOT_ID};
use crate::hprof::{ClassId, ClassStat, HeapSummary, ObjectGraph, ObjectId, RecordStat};
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::Graph;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Aggregated dominator tree information for reporting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub dominators: Vec<DominatorNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DominatorNode {
    pub name: String,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub object_id: String,
    pub dominates: usize,
    pub immediate_dominator: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retained_size: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shallow_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistogramEntry {
    pub key: String,
    pub instance_count: u64,
    pub shallow_size: u64,
    pub retained_size: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistogramGroupBy {
    Class,
    Package,
    ClassLoader,
    Superclass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistogramResult {
    pub group_by: HistogramGroupBy,
    pub entries: Vec<HistogramEntry>,
    pub total_instances: u64,
    pub total_shallow_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnreachableSet {
    pub total_count: u64,
    pub total_shallow_size: u64,
    pub by_class: Vec<UnreachableClassEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnreachableClassEntry {
    pub class_name: String,
    pub count: u64,
    pub shallow_size: u64,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

fn format_graph_object_id(object_id: ObjectId, id_size: u8) -> String {
    let width = usize::from(id_size) * 2;
    format!("0x{object_id:0width$X}")
}

pub fn build_histogram(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    group_by: HistogramGroupBy,
) -> HistogramResult {
    let mut entries_by_key: HashMap<String, HistogramEntry> = HashMap::new();
    let mut total_instances = 0_u64;
    let mut total_shallow_size = 0_u64;

    for (&obj_id, obj) in &graph.objects {
        let shallow_size = u64::from(obj.shallow_size);
        let retained_size = dom.retained_size(obj_id);
        let key = resolve_histogram_key(graph, obj_id, group_by);

        let entry = entries_by_key.entry(key.clone()).or_insert(HistogramEntry {
            key,
            instance_count: 0,
            shallow_size: 0,
            retained_size: 0,
        });
        entry.instance_count += 1;
        entry.shallow_size += shallow_size;
        entry.retained_size += retained_size;

        total_instances += 1;
        total_shallow_size += shallow_size;
    }

    let mut entries: Vec<HistogramEntry> = entries_by_key.into_values().collect();
    entries.sort_by(|a, b| {
        b.retained_size
            .cmp(&a.retained_size)
            .then_with(|| b.shallow_size.cmp(&a.shallow_size))
            .then_with(|| a.key.cmp(&b.key))
    });

    HistogramResult {
        group_by,
        entries,
        total_instances,
        total_shallow_size,
    }
}

pub fn find_unreachable_objects(graph: &ObjectGraph) -> UnreachableSet {
    let mut reachable: HashSet<ObjectId> = HashSet::new();
    let mut queue: VecDeque<ObjectId> = VecDeque::new();

    for root in &graph.gc_roots {
        if graph.objects.contains_key(&root.object_id) && reachable.insert(root.object_id) {
            queue.push_back(root.object_id);
        }
    }

    while let Some(obj_id) = queue.pop_front() {
        if let Some(object) = graph.objects.get(&obj_id) {
            for &reference in &object.references {
                if graph.objects.contains_key(&reference) && reachable.insert(reference) {
                    queue.push_back(reference);
                }
            }
        }
    }

    let mut grouped: HashMap<String, UnreachableClassEntry> = HashMap::new();
    let mut total_count = 0_u64;
    let mut total_shallow_size = 0_u64;

    for (&obj_id, obj) in &graph.objects {
        if reachable.contains(&obj_id) {
            continue;
        }

        let class_name = graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string();
        let shallow_size = u64::from(obj.shallow_size);
        let entry = grouped
            .entry(class_name.clone())
            .or_insert(UnreachableClassEntry {
                class_name,
                count: 0,
                shallow_size: 0,
            });
        entry.count += 1;
        entry.shallow_size += shallow_size;
        total_count += 1;
        total_shallow_size += shallow_size;
    }

    let mut by_class: Vec<UnreachableClassEntry> = grouped.into_values().collect();
    by_class.sort_by(|a, b| {
        b.shallow_size
            .cmp(&a.shallow_size)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.class_name.cmp(&b.class_name))
    });

    UnreachableSet {
        total_count,
        total_shallow_size,
        by_class,
    }
}

fn resolve_histogram_key(
    graph: &ObjectGraph,
    obj_id: ObjectId,
    group_by: HistogramGroupBy,
) -> String {
    let Some(obj) = graph.objects.get(&obj_id) else {
        return String::from("<unknown>");
    };

    match group_by {
        HistogramGroupBy::Class => graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string(),
        HistogramGroupBy::Package => {
            extract_package_name(graph.class_name(obj.class_id).unwrap_or("<unknown>"))
        }
        HistogramGroupBy::ClassLoader => graph
            .classes
            .get(&obj.class_id)
            .map(|class| resolve_class_loader_name(graph, class.class_loader_id))
            .unwrap_or_else(|| String::from("<unknown>")),
        HistogramGroupBy::Superclass => {
            if graph.classes.contains_key(&obj.class_id) {
                resolve_superclass_key(graph, obj.class_id)
            } else {
                String::from("<unknown>")
            }
        }
    }
}

/// Default bound for `resolve_superclass_chain`'s ancestor walk, mirroring
/// `classloader::resolve_loader_chain`'s (M13) termination guard. Real JVM
/// class hierarchies are shallow even in deep framework stacks, so this cap
/// is generous headroom rather than a tight limit -- it exists purely as a
/// guard against adversarial or malformed HPROF data (a `super_class_id`
/// cycle), not because deep legitimate hierarchies are expected.
const MAX_SUPERCLASS_CHAIN_DEPTH: usize = 32;

/// Walks `ClassInfo.super_class_id` from `class_id`'s own superclass upward,
/// collecting ancestor `ClassId`s in ascending generation order (index 0 is
/// the immediate superclass). Does not include `class_id` itself. Stops at
/// the root (`super_class_id == 0`, i.e. `java.lang.Object` or unresolved),
/// at `max_depth` entries, or the moment a previously-visited class id would
/// be revisited (cycle guard) -- same bounded-walk-with-cycle-guard shape as
/// `classloader::resolve_loader_chain` (M13), applied to `super_class_id`
/// instead of a classloader's `parent` field.
fn resolve_superclass_chain(
    graph: &ObjectGraph,
    class_id: ClassId,
    max_depth: usize,
) -> Vec<ClassId> {
    let mut chain = Vec::new();
    let mut visited: HashSet<ClassId> = HashSet::new();
    visited.insert(class_id);

    let mut current = class_id;
    while chain.len() < max_depth {
        let Some(super_id) = graph.classes.get(&current).map(|c| c.super_class_id) else {
            break;
        };
        if super_id == 0 {
            // Reached the root: java.lang.Object (or an unresolved chain).
            break;
        }
        if !visited.insert(super_id) {
            // Cycle detected: this class id has already been visited in
            // this walk. Terminate rather than looping until max_depth.
            break;
        }
        chain.push(super_id);
        current = super_id;
    }

    chain
}

/// Grouping key for `HistogramGroupBy::Superclass`: the *immediate*
/// superclass of `class_id`, i.e. the nearest named ancestor level -- not
/// the root of the hierarchy. This aggregates sibling leaf classes that
/// share a common direct parent (MAT's stated use case: "many leaf classes
/// share a common ancestor worth aggregating"), while avoiding the
/// degenerate case where walking all the way to the universal root would
/// collapse almost every class in a heap into a single `java.lang.Object`
/// bucket. See the M15 Slice 15.B commit body for the full rationale.
///
/// Internally this still goes through the bounded, cycle-guarded
/// `resolve_superclass_chain` walk (rather than reading
/// `ClassInfo.super_class_id` directly) so a self-referential or cyclic
/// `super_class_id` chain in adversarial/malformed HPROF data can never
/// hang this lookup, and so a future deeper grouping mode (e.g. root
/// superclass) can reuse the same walk.
fn resolve_superclass_key(graph: &ObjectGraph, class_id: ClassId) -> String {
    let chain = resolve_superclass_chain(graph, class_id, MAX_SUPERCLASS_CHAIN_DEPTH);
    match chain.first() {
        Some(&super_id) => graph
            .class_name(super_id)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("<class:{super_id}>")),
        None => String::from("<java.lang.Object>"),
    }
}

fn extract_package_name(class_name: &str) -> String {
    class_name
        .rmatch_indices(['.', '/'])
        .next()
        .map(|(idx, _)| class_name[..idx].to_string())
        .filter(|package| !package.is_empty())
        .unwrap_or_else(|| String::from("<default>"))
}

fn resolve_class_loader_name(graph: &ObjectGraph, class_loader_id: ObjectId) -> String {
    if class_loader_id == 0 {
        return String::from("<bootstrap>");
    }

    graph
        .objects
        .get(&class_loader_id)
        .and_then(|loader| graph.class_name(loader.class_id))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("<loader:{class_loader_id}>"))
}

/// Build `GraphMetrics` from a real dominator tree and object graph.
pub fn build_graph_metrics_from_dominator(
    dom: &DominatorTree,
    graph: &ObjectGraph,
) -> GraphMetrics {
    let top = dom.top_retained(12);
    let mut dominators = Vec::with_capacity(top.len());

    for &(obj_id, retained) in &top {
        let object = graph.objects.get(&obj_id);
        let class_name = object
            .and_then(|obj| graph.class_name(obj.class_id))
            .unwrap_or("<unknown>")
            .to_string();
        let shallow_size = object.map(|obj| u64::from(obj.shallow_size)).unwrap_or(0);

        let dominates = dom.dominated_by(obj_id).len();

        let immediate = dom
            .immediate_dominator(obj_id)
            .filter(|&idom| idom != VIRTUAL_ROOT_ID)
            .and_then(|idom| graph.objects.get(&idom))
            .and_then(|obj| graph.class_name(obj.class_id))
            .map(String::from);

        dominators.push(DominatorNode {
            name: class_name.clone(),
            class_name,
            object_id: format_graph_object_id(obj_id, graph.identifier_size),
            dominates,
            immediate_dominator: immediate,
            retained_size: retained,
            shallow_size,
        });
    }

    let edge_count: usize = graph.objects.values().map(|o| o.references.len()).sum();

    GraphMetrics {
        node_count: dom.node_count(),
        edge_count,
        dominators,
    }
}

/// Build a lightweight dominator view driven by either the parsed class
/// histogram or (as a fallback) the raw record tags. This keeps reporting fast
/// while still reflecting what the parser observed in the heap dump.
pub fn summarize_graph(summary: &HeapSummary) -> GraphMetrics {
    let mut graph: Graph<String, ()> = Graph::new();
    let root = graph.add_node("<heap-root>".into());

    enum Source<'a> {
        Class(&'a ClassStat),
        Record(&'a RecordStat),
    }

    let sources: Vec<Source<'_>> = if summary.classes.is_empty() {
        summary
            .record_stats
            .iter()
            .take(12)
            .map(Source::Record)
            .collect()
    } else {
        summary.classes.iter().take(12).map(Source::Class).collect()
    };

    if sources.is_empty() {
        return GraphMetrics::default();
    }

    let mut node_entries = Vec::new();
    for source in &sources {
        match source {
            Source::Class(class) => {
                let label = format!(
                    "{} ({:.1}% / {:.2} MB)",
                    class.name,
                    class.percentage,
                    class.total_size_bytes as f64 / (1024.0 * 1024.0)
                );
                let node = graph.add_node(label);
                graph.add_edge(root, node, ());
                node_entries.push((node, class.name.clone(), class.total_size_bytes as usize));
            }
            Source::Record(record) => {
                let label = format!("{} ({} entries)", record.name, record.count);
                let node = graph.add_node(label);
                graph.add_edge(root, node, ());
                node_entries.push((node, record.name.clone(), record.count as usize));
            }
        }
    }

    for idx in 1..node_entries.len() {
        let prev = node_entries[idx - 1].0;
        let current = node_entries[idx].0;
        graph.add_edge(prev, current, ());
    }

    let dom_result = simple_fast(&graph, root);
    let mut dominators = Vec::new();
    for (node, logical_name, dominates) in node_entries {
        let immediate = dom_result.immediate_dominator(node).and_then(|idx| {
            if idx == node {
                None
            } else {
                graph.node_weight(idx).cloned()
            }
        });
        dominators.push(DominatorNode {
            name: logical_name,
            class_name: String::new(),
            object_id: String::new(),
            dominates,
            immediate_dominator: immediate,
            retained_size: 0,
            shallow_size: 0,
        });
    }

    GraphMetrics {
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        dominators,
    }
}

#[cfg(test)]
mod tests {
    use super::super::dominator::build_dominator_tree;
    use super::*;
    use crate::hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectKind};

    fn make_test_graph(objects: &[(u64, u64, u32, &[u64])], gc_roots: &[u64]) -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        for &(id, class_id, size, refs) in objects {
            graph.objects.insert(
                id,
                HeapObject {
                    id,
                    class_id,
                    shallow_size: size,
                    references: refs.to_vec(),
                    field_data: Vec::new(),
                    kind: ObjectKind::Instance,
                },
            );
        }
        for &root_id in gc_roots {
            graph.gc_roots.push(GcRoot {
                object_id: root_id,
                root_type: GcRootType::StickyClass,
            });
        }
        graph
    }

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str, class_loader_id: u64) {
        add_class_with_super(graph, class_id, name, class_loader_id, 0);
    }

    fn add_class_with_super(
        graph: &mut ObjectGraph,
        class_id: u64,
        name: &str,
        class_loader_id: u64,
        super_class_id: u64,
    ) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id,
                class_loader_id,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
    }

    #[test]
    fn build_graph_metrics_from_dominator_populates_retained_size() {
        // Root(1) → A(2) → B(3); shallow: 10, 20, 30
        // Retained: 1=60, 2=50, 3=30
        let obj_graph = make_test_graph(
            &[
                (1, 0x100, 10, &[2]),
                (2, 0x100, 20, &[3]),
                (3, 0x100, 30, &[]),
            ],
            &[1],
        );
        let dom = build_dominator_tree(&obj_graph);
        let metrics = build_graph_metrics_from_dominator(&dom, &obj_graph);

        assert_eq!(metrics.node_count, 3);
        // 3 objects, 2 reference edges (1→2, 2→3)
        assert_eq!(metrics.edge_count, 2);
        assert!(!metrics.dominators.is_empty());
        // Top retained should be object 1 with 60 bytes
        assert_eq!(metrics.dominators[0].retained_size, 60);
        assert_eq!(metrics.dominators[1].retained_size, 50);
        assert_eq!(metrics.dominators[2].retained_size, 30);
    }

    #[test]
    fn build_graph_metrics_from_dominator_exposes_browser_navigation_fields() {
        let mut obj_graph = make_test_graph(
            &[
                (0x1000, 0x100, 10, &[0x2000]),
                (0x2000, 0x200, 20, &[0x3000]),
                (0x3000, 0x300, 30, &[]),
            ],
            &[0x1000],
        );
        add_class(&mut obj_graph, 0x100, "com.example.Root", 0);
        add_class(&mut obj_graph, 0x200, "com.example.Parent", 0);
        add_class(&mut obj_graph, 0x300, "com.example.Child", 0);

        let dom = build_dominator_tree(&obj_graph);
        let metrics = build_graph_metrics_from_dominator(&dom, &obj_graph);

        assert_eq!(metrics.dominators.len(), 3);
        assert_eq!(metrics.dominators[0].name, "com.example.Root");
        assert_eq!(metrics.dominators[0].class_name, "com.example.Root");
        assert_eq!(metrics.dominators[0].object_id, "0x0000000000001000");
        assert_eq!(metrics.dominators[0].shallow_size, 10);

        assert_eq!(metrics.dominators[1].name, "com.example.Parent");
        assert_eq!(metrics.dominators[1].class_name, "com.example.Parent");
        assert_eq!(metrics.dominators[1].object_id, "0x0000000000002000");
        assert_eq!(
            metrics.dominators[1].immediate_dominator.as_deref(),
            Some("com.example.Root")
        );
        assert_eq!(metrics.dominators[1].shallow_size, 20);

        assert_eq!(metrics.dominators[2].name, "com.example.Child");
        assert_eq!(metrics.dominators[2].class_name, "com.example.Child");
        assert_eq!(metrics.dominators[2].object_id, "0x0000000000003000");
        assert_eq!(
            metrics.dominators[2].immediate_dominator.as_deref(),
            Some("com.example.Parent")
        );
        assert_eq!(metrics.dominators[2].shallow_size, 30);
    }

    #[test]
    fn summarize_graph_sets_zero_retained_size() {
        let summary = HeapSummary {
            heap_path: "test.hprof".into(),
            total_objects: 1,
            total_size_bytes: 100,
            classes: vec![ClassStat {
                name: "com.example.Foo".into(),
                instances: 1,
                total_size_bytes: 100,
                percentage: 100.0,
            }],
            generated_at: std::time::SystemTime::UNIX_EPOCH,
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let metrics = summarize_graph(&summary);
        for node in &metrics.dominators {
            assert_eq!(
                node.retained_size, 0,
                "summarize_graph nodes must have zero retained_size"
            );
        }
    }

    #[test]
    fn histogram_groups_by_class() {
        let mut graph = make_test_graph(
            &[(1, 100, 10, &[2]), (2, 100, 20, &[]), (3, 200, 30, &[])],
            &[1, 3],
        );
        add_class(&mut graph, 100, "com.example.Cache", 0);
        add_class(&mut graph, 200, "com.example.Listener", 0);

        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Class);

        assert_eq!(histogram.total_instances, 3);
        assert_eq!(histogram.total_shallow_size, 60);
        assert_eq!(histogram.entries.len(), 2);
        assert_eq!(histogram.entries[0].key, "com.example.Cache");
        assert_eq!(histogram.entries[0].instance_count, 2);
        assert_eq!(histogram.entries[0].shallow_size, 30);
        assert_eq!(histogram.entries[0].retained_size, 50);
        assert_eq!(histogram.entries[1].key, "com.example.Listener");
    }

    #[test]
    fn histogram_groups_by_package_prefix() {
        let mut graph = make_test_graph(
            &[(1, 100, 10, &[]), (2, 200, 20, &[]), (3, 300, 30, &[])],
            &[1, 2, 3],
        );
        add_class(&mut graph, 100, "com.example.cache.CacheA", 0);
        add_class(&mut graph, 200, "com.example.cache.CacheB", 0);
        add_class(&mut graph, 300, "DefaultClass", 0);

        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Package);

        assert_eq!(histogram.entries.len(), 2);
        let cache_group = histogram
            .entries
            .iter()
            .find(|entry| entry.key == "com.example.cache")
            .unwrap();
        let default_group = histogram
            .entries
            .iter()
            .find(|entry| entry.key == "<default>")
            .unwrap();
        assert_eq!(cache_group.instance_count, 2);
        assert_eq!(default_group.instance_count, 1);
    }

    #[test]
    fn histogram_handles_empty_graph() {
        let graph = ObjectGraph::new(8);
        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Class);

        assert_eq!(histogram.total_instances, 0);
        assert_eq!(histogram.total_shallow_size, 0);
        assert!(histogram.entries.is_empty());
    }

    /// Three-level hierarchy: Root(100, super=0) <- Mid(200, super=100) <-
    /// Leaf(300, super=200), plus a second leaf OtherLeaf(350, super=200)
    /// sharing Mid as its immediate superclass. Proves both the per-level
    /// walk (Leaf instances resolve to "Mid", Mid's own instance resolves to
    /// "Root", Root's own instance resolves to the root sentinel) and the
    /// aggregation value MAT's "group by superclass" is meant to provide:
    /// Leaf and OtherLeaf instances are distinct classes but land in the
    /// same "com.example.Mid" bucket because they share an immediate
    /// superclass.
    #[test]
    fn histogram_groups_by_immediate_superclass_across_a_three_level_hierarchy() {
        let mut graph = make_test_graph(
            &[
                (1, 300, 10, &[]), // Leaf instance
                (2, 300, 20, &[]), // Leaf instance
                (3, 200, 30, &[]), // Mid instance
                (4, 100, 40, &[]), // Root instance
                (5, 350, 5, &[]),  // OtherLeaf instance
            ],
            &[1, 2, 3, 4, 5],
        );
        add_class_with_super(&mut graph, 100, "com.example.Root", 0, 0);
        add_class_with_super(&mut graph, 200, "com.example.Mid", 0, 100);
        add_class_with_super(&mut graph, 300, "com.example.Leaf", 0, 200);
        add_class_with_super(&mut graph, 350, "com.example.OtherLeaf", 0, 200);

        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Superclass);

        assert_eq!(histogram.total_instances, 5);
        assert_eq!(histogram.total_shallow_size, 105);

        let mid_group = histogram
            .entries
            .iter()
            .find(|entry| entry.key == "com.example.Mid")
            .expect("Leaf + OtherLeaf instances group under their shared immediate superclass Mid");
        assert_eq!(mid_group.instance_count, 3); // 2 Leaf + 1 OtherLeaf
        assert_eq!(mid_group.shallow_size, 35);

        let root_group = histogram
            .entries
            .iter()
            .find(|entry| entry.key == "com.example.Root")
            .expect("Mid's own instance groups under its immediate superclass Root");
        assert_eq!(root_group.instance_count, 1);
        assert_eq!(root_group.shallow_size, 30);

        let object_group = histogram
            .entries
            .iter()
            .find(|entry| entry.key == "<java.lang.Object>")
            .expect("Root's own instance has no named superclass (super_class_id == 0)");
        assert_eq!(object_group.instance_count, 1);
        assert_eq!(object_group.shallow_size, 40);
    }

    #[test]
    fn histogram_group_by_superclass_falls_back_to_unknown_for_unresolved_class() {
        // Object references a class_id with no corresponding ClassInfo entry.
        let graph = make_test_graph(&[(1, 999, 10, &[])], &[1]);
        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Superclass);

        assert_eq!(histogram.entries.len(), 1);
        assert_eq!(histogram.entries[0].key, "<unknown>");
    }

    /// A self-referential class (its own `super_class_id` points back at
    /// itself) -- the simplest possible cyclic/adversarial HPROF shape.
    /// Mirrors `classloader::resolve_loader_chain`'s own self-referential
    /// regression test (M13): this must terminate, not infinite-loop, and
    /// the histogram build as a whole must still complete.
    #[test]
    fn resolve_superclass_chain_self_referential_class_does_not_hang_and_returns_empty() {
        let mut graph = ObjectGraph::new(8);
        add_class_with_super(&mut graph, 100, "com.example.SelfLoopClass", 0, 100);

        let chain = resolve_superclass_chain(&graph, 100, MAX_SUPERCLASS_CHAIN_DEPTH);

        assert!(chain.is_empty());
    }

    #[test]
    fn histogram_group_by_superclass_on_self_referential_class_does_not_hang() {
        let mut graph = make_test_graph(&[(1, 100, 10, &[])], &[1]);
        add_class_with_super(&mut graph, 100, "com.example.SelfLoopClass", 0, 100);

        let dom = build_dominator_tree(&graph);
        let histogram = build_histogram(&graph, &dom, HistogramGroupBy::Superclass);

        assert_eq!(histogram.total_instances, 1);
        assert_eq!(histogram.entries.len(), 1);
        assert_eq!(histogram.entries[0].key, "<java.lang.Object>");
    }

    /// A longer cycle: X(100) -> Y(200) -> X(100) -> ... . Must stop after
    /// visiting Y once, not loop until max_depth.
    #[test]
    fn resolve_superclass_chain_two_node_cycle_terminates_after_first_repeat() {
        let mut graph = ObjectGraph::new(8);
        add_class_with_super(&mut graph, 100, "com.example.X", 0, 200); // X: super Y
        add_class_with_super(&mut graph, 200, "com.example.Y", 0, 100); // Y: super X

        let chain = resolve_superclass_chain(&graph, 100, MAX_SUPERCLASS_CHAIN_DEPTH);

        // X -> Y, then Y's super (X) is already visited: stop.
        assert_eq!(chain, vec![200]);
        assert!(
            chain.len() < MAX_SUPERCLASS_CHAIN_DEPTH,
            "cycle guard must terminate well before the depth bound"
        );
    }

    /// A chain longer than `max_depth` must be truncated, not fully walked
    /// -- proves max_depth is an independent, respected bound and not just a
    /// fallback for the cycle guard.
    #[test]
    fn resolve_superclass_chain_respects_max_depth_on_a_longer_acyclic_chain() {
        let mut graph = ObjectGraph::new(8);
        add_class_with_super(&mut graph, 1, "com.example.Gen0", 0, 0); // root
        add_class_with_super(&mut graph, 2, "com.example.Gen1", 0, 1);
        add_class_with_super(&mut graph, 3, "com.example.Gen2", 0, 2);
        add_class_with_super(&mut graph, 4, "com.example.Gen3", 0, 3);
        add_class_with_super(&mut graph, 5, "com.example.Gen4", 0, 4);

        let full_chain = resolve_superclass_chain(&graph, 5, MAX_SUPERCLASS_CHAIN_DEPTH);
        assert_eq!(full_chain, vec![4, 3, 2, 1]);

        let truncated = resolve_superclass_chain(&graph, 5, 3);
        assert_eq!(truncated, vec![4, 3, 2]);
        assert_eq!(truncated.len(), 3);
    }

    #[test]
    fn finds_unreachable_objects() {
        let mut graph = make_test_graph(
            &[
                (1, 100, 10, &[2]),
                (2, 200, 20, &[]),
                (3, 300, 30, &[4]),
                (4, 300, 40, &[]),
            ],
            &[1],
        );
        add_class(&mut graph, 100, "com.example.Root", 0);
        add_class(&mut graph, 200, "com.example.Live", 0);
        add_class(&mut graph, 300, "com.example.Dead", 0);

        let unreachable = find_unreachable_objects(&graph);

        assert_eq!(unreachable.total_count, 2);
        assert_eq!(unreachable.total_shallow_size, 70);
        assert_eq!(unreachable.by_class.len(), 1);
        assert_eq!(unreachable.by_class[0].class_name, "com.example.Dead");
        assert_eq!(unreachable.by_class[0].count, 2);
        assert_eq!(unreachable.by_class[0].shallow_size, 70);
    }

    #[test]
    fn fully_reachable_graph_has_no_unreachable_objects() {
        let mut graph = make_test_graph(&[(1, 100, 10, &[2]), (2, 200, 20, &[])], &[1]);
        add_class(&mut graph, 100, "com.example.Root", 0);
        add_class(&mut graph, 200, "com.example.Live", 0);

        let unreachable = find_unreachable_objects(&graph);

        assert_eq!(unreachable.total_count, 0);
        assert_eq!(unreachable.total_shallow_size, 0);
        assert!(unreachable.by_class.is_empty());
    }
}
