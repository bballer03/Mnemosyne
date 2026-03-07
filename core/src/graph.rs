use crate::dominator::{DominatorTree, VIRTUAL_ROOT_ID};
use crate::heap::{ClassStat, HeapSummary, RecordStat};
use crate::object_graph::ObjectGraph;
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::Graph;
use serde::{Deserialize, Serialize};

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

fn is_zero(v: &u64) -> bool {
    *v == 0
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
    use super::*;
    use crate::dominator::build_dominator_tree;
    use crate::object_graph::{GcRoot, GcRootType, HeapObject, ObjectKind};

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
}
