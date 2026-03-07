use crate::heap::{ClassStat, HeapSummary, RecordStat};
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
        });
    }

    GraphMetrics {
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        dominators,
    }
}
