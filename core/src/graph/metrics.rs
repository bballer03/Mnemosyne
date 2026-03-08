use super::dominator::{DominatorTree, VIRTUAL_ROOT_ID};
use crate::hprof::{ClassStat, HeapSummary, ObjectGraph, ObjectId, RecordStat};
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
    pub dominates: usize,
    pub immediate_dominator: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retained_size: u64,
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
        .unwrap_or_else(|| format!("<loader:{}>", class_loader_id))
}

/// Build `GraphMetrics` from a real dominator tree and object graph.
pub fn build_graph_metrics_from_dominator(
    dom: &DominatorTree,
    graph: &ObjectGraph,
) -> GraphMetrics {
    let top = dom.top_retained(12);
    let mut dominators = Vec::with_capacity(top.len());

    for &(obj_id, retained) in &top {
        let name = graph
            .objects
            .get(&obj_id)
            .and_then(|obj| graph.class_name(obj.class_id))
            .unwrap_or("<unknown>")
            .to_string();

        let dominates = dom.dominated_by(obj_id).len();

        let immediate = dom
            .immediate_dominator(obj_id)
            .filter(|&idom| idom != VIRTUAL_ROOT_ID)
            .and_then(|idom| graph.objects.get(&idom))
            .and_then(|obj| graph.class_name(obj.class_id))
            .map(String::from);

        dominators.push(DominatorNode {
            name,
            dominates,
            immediate_dominator: immediate,
            retained_size: retained,
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
            dominates,
            immediate_dominator: immediate,
            retained_size: 0,
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
