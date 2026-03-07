use std::collections::{HashMap, HashSet};

use petgraph::algo::dominators::simple_fast;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::hprof::{ObjectGraph, ObjectId};

/// Virtual super-root ID that doesn't collide with real HPROF object IDs.
pub const VIRTUAL_ROOT_ID: ObjectId = u64::MAX;

/// The result of dominator tree computation over an `ObjectGraph`.
///
/// Built by [`build_dominator_tree`], which creates a virtual super-root
/// connected to every GC root present in the objects map, runs
/// Lengauer–Tarjan, and computes retained sizes in a single post-order pass.
pub struct DominatorTree {
    /// Map from each object ID to its immediate dominator's object ID.
    /// The virtual super-root is represented as [`VIRTUAL_ROOT_ID`].
    immediate_dominators: HashMap<ObjectId, ObjectId>,

    /// Map from each object ID to the list of objects it immediately dominates.
    dominated_children: HashMap<ObjectId, Vec<ObjectId>>,

    /// Retained size per object (accumulated shallow sizes of dominated subtree).
    retained_sizes: HashMap<ObjectId, u64>,
}

/// Build a dominator tree from the given object graph.
///
/// 1. Creates a virtual super-root connected to all GC-root object IDs that
///    exist in `graph.objects`.
/// 2. Materialises a `petgraph::DiGraph` of all objects and their references.
/// 3. Runs Lengauer–Tarjan (`simple_fast`) from the virtual root.
/// 4. Computes retained sizes via post-order traversal.
pub fn build_dominator_tree(graph: &ObjectGraph) -> DominatorTree {
    if graph.objects.is_empty() {
        return DominatorTree {
            immediate_dominators: HashMap::new(),
            dominated_children: HashMap::new(),
            retained_sizes: HashMap::new(),
        };
    }

    // -- 1. Build petgraph -------------------------------------------------
    let mut digraph: DiGraph<ObjectId, ()> = DiGraph::new();
    let mut id_to_node: HashMap<ObjectId, NodeIndex> = HashMap::new();

    // Virtual super-root
    let virtual_root_node = digraph.add_node(VIRTUAL_ROOT_ID);
    id_to_node.insert(VIRTUAL_ROOT_ID, virtual_root_node);

    // Add a node for every real object
    for &obj_id in graph.objects.keys() {
        let node = digraph.add_node(obj_id);
        id_to_node.insert(obj_id, node);
    }

    // -- 2. GC-root edges (deduplicated) -----------------------------------
    let root_ids: HashSet<ObjectId> = graph
        .gc_roots
        .iter()
        .map(|r| r.object_id)
        .filter(|id| graph.objects.contains_key(id))
        .collect();

    for &root_id in &root_ids {
        digraph.add_edge(virtual_root_node, id_to_node[&root_id], ());
    }

    // -- 3. Reference edges ------------------------------------------------
    for obj in graph.objects.values() {
        let from = id_to_node[&obj.id];
        for &ref_id in &obj.references {
            if let Some(&to) = id_to_node.get(&ref_id) {
                digraph.add_edge(from, to, ());
            }
        }
    }

    // -- 4. Run dominators -------------------------------------------------
    let dom_result = simple_fast(&digraph, virtual_root_node);

    // -- 5. Build immediate_dominators map ---------------------------------
    let mut immediate_dominators: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut dominated_children: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();

    for (&obj_id, &node) in &id_to_node {
        if obj_id == VIRTUAL_ROOT_ID {
            continue;
        }
        if let Some(idom_node) = dom_result.immediate_dominator(node) {
            let idom_id = digraph[idom_node];
            immediate_dominators.insert(obj_id, idom_id);
            dominated_children.entry(idom_id).or_default().push(obj_id);
        }
    }

    // -- 7. Compute retained sizes (post-order) ----------------------------
    let retained_sizes = compute_retained_sizes(graph, &dominated_children);

    DominatorTree {
        immediate_dominators,
        dominated_children,
        retained_sizes,
    }
}

/// Post-order traversal of the dominator tree to accumulate retained sizes.
fn compute_retained_sizes(
    graph: &ObjectGraph,
    children: &HashMap<ObjectId, Vec<ObjectId>>,
) -> HashMap<ObjectId, u64> {
    let mut sizes: HashMap<ObjectId, u64> = HashMap::new();

    // Iterative post-order using an explicit stack.
    // Start from VIRTUAL_ROOT_ID so we visit the whole tree.
    let mut stack: Vec<(ObjectId, bool)> = vec![(VIRTUAL_ROOT_ID, false)];

    while let Some((id, visited)) = stack.pop() {
        if visited {
            let shallow = graph
                .objects
                .get(&id)
                .map(|o| u64::from(o.shallow_size))
                .unwrap_or(0);
            let child_sum: u64 = children
                .get(&id)
                .map(|kids| {
                    kids.iter()
                        .map(|c| sizes.get(c).copied().unwrap_or(0))
                        .sum()
                })
                .unwrap_or(0);
            sizes.insert(id, shallow + child_sum);
        } else {
            stack.push((id, true));
            if let Some(kids) = children.get(&id) {
                for &child in kids {
                    stack.push((child, false));
                }
            }
        }
    }

    sizes
}

impl DominatorTree {
    /// Returns the immediate dominator of the given object, or `None` if
    /// the object is not in the tree or is the virtual root.
    pub fn immediate_dominator(&self, id: ObjectId) -> Option<ObjectId> {
        self.immediate_dominators.get(&id).copied()
    }

    /// Returns the list of objects immediately dominated by the given object.
    pub fn dominated_by(&self, id: ObjectId) -> &[ObjectId] {
        self.dominated_children
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns the retained size of the given object (sum of shallow sizes
    /// in its dominated subtree, including itself).
    pub fn retained_size(&self, id: ObjectId) -> u64 {
        self.retained_sizes.get(&id).copied().unwrap_or(0)
    }

    /// Returns the number of objects in the dominator tree
    /// (excluding the virtual root).
    pub fn node_count(&self) -> usize {
        self.immediate_dominators.len()
    }

    /// Returns the top N objects by retained size, sorted descending.
    pub fn top_retained(&self, n: usize) -> Vec<(ObjectId, u64)> {
        let mut entries: Vec<(ObjectId, u64)> = self
            .retained_sizes
            .iter()
            .filter(|(&id, _)| id != VIRTUAL_ROOT_ID)
            .map(|(&id, &size)| (id, size))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(n);
        entries
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{GcRoot, GcRootType, HeapObject, ObjectKind};

    /// Helper: build a programmatic ObjectGraph from a compact description.
    fn make_graph(
        objects: &[(ObjectId, u32, &[ObjectId])],
        gc_root_ids: &[ObjectId],
    ) -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        for &(id, shallow_size, refs) in objects {
            graph.objects.insert(
                id,
                HeapObject {
                    id,
                    class_id: 0x100,
                    shallow_size,
                    references: refs.to_vec(),
                    kind: ObjectKind::Instance,
                },
            );
        }
        for &id in gc_root_ids {
            graph.gc_roots.push(GcRoot {
                object_id: id,
                root_type: GcRootType::StickyClass,
            });
        }
        graph
    }

    // -- test_empty_graph ---------------------------------------------------
    #[test]
    fn test_empty_graph() {
        let graph = ObjectGraph::new(8);
        let tree = build_dominator_tree(&graph);
        assert_eq!(tree.node_count(), 0);
        assert!(tree.top_retained(10).is_empty());
    }

    // -- test_dominator_tree_linear_chain -----------------------------------
    #[test]
    fn test_dominator_tree_linear_chain() {
        // Root → A → B → C
        let graph = make_graph(&[(1, 10, &[2]), (2, 20, &[3]), (3, 30, &[])], &[1]);
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.node_count(), 3);
        assert_eq!(tree.immediate_dominator(1), Some(VIRTUAL_ROOT_ID));
        assert_eq!(tree.immediate_dominator(2), Some(1));
        assert_eq!(tree.immediate_dominator(3), Some(2));

        // Retained sizes: C=30, B=20+30=50, A=10+50=60
        assert_eq!(tree.retained_size(3), 30);
        assert_eq!(tree.retained_size(2), 50);
        assert_eq!(tree.retained_size(1), 60);
    }

    // -- test_retained_sizes ------------------------------------------------
    #[test]
    fn test_retained_sizes() {
        // Root → A; A → B, A → C; B → D
        let graph = make_graph(
            &[(1, 10, &[2, 3]), (2, 20, &[4]), (3, 30, &[]), (4, 40, &[])],
            &[1],
        );
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.retained_size(4), 40);
        assert_eq!(tree.retained_size(2), 60); // 20 + 40
        assert_eq!(tree.retained_size(3), 30);
        assert_eq!(tree.retained_size(1), 100); // 10 + 60 + 30
    }

    // -- test_top_retained --------------------------------------------------
    #[test]
    fn test_top_retained() {
        let graph = make_graph(
            &[(1, 10, &[2, 3]), (2, 20, &[4]), (3, 30, &[]), (4, 40, &[])],
            &[1],
        );
        let tree = build_dominator_tree(&graph);
        let top2 = tree.top_retained(2);
        assert_eq!(top2.len(), 2);
        // A(100), B(60) are the two largest
        assert_eq!(top2[0], (1, 100));
        assert_eq!(top2[1], (2, 60));
    }

    // -- test_diamond_graph -------------------------------------------------
    #[test]
    fn test_diamond_graph() {
        // Root → A; A → B, A → C; B → D, C → D
        // D's immediate dominator should be A (reachable through both B and C).
        let graph = make_graph(
            &[(1, 10, &[2, 3]), (2, 20, &[4]), (3, 30, &[4]), (4, 40, &[])],
            &[1],
        );
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.immediate_dominator(4), Some(1));
        // D is dominated by A, not B or C.
        assert!(tree.dominated_by(2).is_empty() || !tree.dominated_by(2).contains(&4));
        assert!(tree.dominated_by(3).is_empty() || !tree.dominated_by(3).contains(&4));
        assert!(tree.dominated_by(1).contains(&4));
    }

    // -- test_gc_root_not_in_objects ----------------------------------------
    #[test]
    fn test_gc_root_not_in_objects() {
        // GC root 0x9999 has no HeapObject — should be silently skipped.
        // Only object 1 is a real GC root.
        let graph = make_graph(&[(1, 10, &[2]), (2, 20, &[])], &[0x9999, 1]);
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.node_count(), 2);
        assert_eq!(tree.immediate_dominator(1), Some(VIRTUAL_ROOT_ID));
        assert_eq!(tree.immediate_dominator(2), Some(1));
    }

    // -- test_unreachable_objects -------------------------------------------
    #[test]
    fn test_unreachable_objects() {
        // Object 3 is in the objects map but unreachable — should not appear
        // in the dominator tree.
        let graph = make_graph(&[(1, 10, &[2]), (2, 20, &[]), (3, 30, &[])], &[1]);
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.node_count(), 2);
        assert_eq!(tree.immediate_dominator(3), None);
        assert_eq!(tree.retained_size(3), 0);
    }

    // -- test_duplicate_gc_roots --------------------------------------------
    #[test]
    fn test_duplicate_gc_roots() {
        // Same GC root listed twice — should not cause duplicate edges or double-counting.
        let graph = make_graph(&[(1, 10, &[])], &[1, 1]);
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.node_count(), 1);
        assert_eq!(tree.retained_size(1), 10);
    }

    // -- test_with_parsed_fixture -------------------------------------------
    #[test]
    fn test_with_parsed_fixture() {
        // The simple fixture's only GC root (0x1000) has no HeapObject, so
        // the dominator tree should be empty (no reachable objects).
        let data = crate::test_fixtures::build_simple_fixture();
        let obj_graph = crate::hprof::parse_hprof(&data).expect("parse should succeed");
        let tree = build_dominator_tree(&obj_graph);

        // 0x1000 is a GC root but NOT in objects → nothing is reachable
        assert_eq!(tree.node_count(), 0);
    }

    // -- test_multiple_gc_roots ---------------------------------------------
    #[test]
    fn test_multiple_gc_roots() {
        // Two independent GC roots each owning a sub-tree.
        let graph = make_graph(
            &[(1, 10, &[3]), (2, 20, &[4]), (3, 30, &[]), (4, 40, &[])],
            &[1, 2],
        );
        let tree = build_dominator_tree(&graph);

        assert_eq!(tree.node_count(), 4);
        assert_eq!(tree.immediate_dominator(1), Some(VIRTUAL_ROOT_ID));
        assert_eq!(tree.immediate_dominator(2), Some(VIRTUAL_ROOT_ID));
        assert_eq!(tree.immediate_dominator(3), Some(1));
        assert_eq!(tree.immediate_dominator(4), Some(2));

        assert_eq!(tree.retained_size(1), 40); // 10 + 30
        assert_eq!(tree.retained_size(2), 60); // 20 + 40
    }
}
