use crate::heap::HeapSummary;
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

/// Build a synthetic object graph from the heap summary and compute dominator
/// relationships. The current implementation creates one node per record tag
/// and links them to a single heap root node. This keeps things lightweight
/// while we wire up the real parser.
pub fn summarize_graph(summary: &HeapSummary) -> GraphMetrics {
    let mut graph: Graph<String, ()> = Graph::new();
    let root = graph.add_node("<heap-root>".into());

    let mut tag_nodes = Vec::new();
    for record in &summary.record_stats {
        let node = graph.add_node(format!("{} ({} entries)", record.name, record.count));
        graph.add_edge(root, node, ());
        tag_nodes.push((node, record));
    }

    // Add a simple chain to produce more interesting dominator relationships.
    for idx in 1..tag_nodes.len() {
        let prev = tag_nodes[idx - 1].0;
        let current = tag_nodes[idx].0;
        graph.add_edge(prev, current, ());
    }

    let dom_result = simple_fast(&graph, root);
    let mut dominators = Vec::new();
    for (node, record) in tag_nodes {
        let immediate = dom_result.immediate_dominator(node).and_then(|idx| {
            if idx == node {
                None
            } else {
                graph.node_weight(idx).cloned()
            }
        });
        dominators.push(DominatorNode {
            name: record.name.clone(),
            dominates: record.count as usize,
            immediate_dominator: immediate,
        });
    }

    GraphMetrics {
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        dominators,
    }
}
