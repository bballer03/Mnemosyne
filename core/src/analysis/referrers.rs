//! Group-by-referrer analyzer (M8 Slice 8.B).
//!
//! Answers "which objects hold the most incoming references, and from
//! where?" by ranking every object in the graph by its referrer count,
//! reusing [`ObjectGraph::get_referrers`] — the existing O(1)-per-call
//! primitive backed by a precomputed reverse-reference index — instead of
//! building a second reverse index. See
//! `docs/design/milestone-8-reachability-references.md` §6/§10 (Slice
//! 8.B) for the design.

use crate::graph::DominatorTree;
use crate::hprof::{ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Up to this many distinct referrer classes are retained per entry,
/// sorted by count descending (ties broken alphabetically for determinism).
const MAX_TOP_REFERRER_CLASSES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferrerEntry {
    pub object_id: String,
    pub class_name: String,
    pub retained_size: Option<u64>,
    pub referrer_count: usize,
    /// Up to 5 distinct referrer classes, sorted by count desc.
    pub top_referrer_classes: Vec<(String, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferrerReport {
    /// Top-N entries by `referrer_count` (tiebreak: `retained_size` desc).
    pub entries: Vec<ReferrerEntry>,
    /// Total number of objects considered before truncation to `top_n`.
    pub total_objects_considered: usize,
}

/// Rank every object in `graph` by incoming reference count.
///
/// Reuses `graph.get_referrers(id)` (O(1) per call, backed by a
/// precomputed reverse-reference index) — no new reverse index is built
/// here. Ranking is `referrer_count` descending, tiebreak `retained_size`
/// descending (objects with no dominator tree available sort as if
/// `retained_size` were 0), final tiebreak `object_id` ascending for a
/// deterministic total order.
pub fn analyze_by_referrer(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    top_n: usize,
) -> ReferrerReport {
    let id_size = graph.identifier_size as usize;

    let mut entries: Vec<ReferrerEntry> = graph
        .objects
        .values()
        .map(|object| {
            let referrers = graph.get_referrers(object.id);
            let top_referrer_classes = top_referrer_classes(graph, &referrers);

            ReferrerEntry {
                object_id: format_object_id(object.id, id_size),
                class_name: graph
                    .class_name(object.class_id)
                    .unwrap_or("<unknown>")
                    .to_string(),
                retained_size: dominator.map(|dom| dom.retained_size(object.id)),
                referrer_count: referrers.len(),
                top_referrer_classes,
            }
        })
        .collect();

    let total_objects_considered = entries.len();

    entries.sort_by(|left, right| {
        right
            .referrer_count
            .cmp(&left.referrer_count)
            .then_with(|| {
                right
                    .retained_size
                    .unwrap_or(0)
                    .cmp(&left.retained_size.unwrap_or(0))
            })
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    entries.truncate(top_n);

    ReferrerReport {
        entries,
        total_objects_considered,
    }
}

/// Group `referrers` by class name, count occurrences, and keep the top
/// [`MAX_TOP_REFERRER_CLASSES`] sorted by count desc (alphabetical
/// tiebreak for determinism).
fn top_referrer_classes(graph: &ObjectGraph, referrers: &[ObjectId]) -> Vec<(String, usize)> {
    let mut class_counts: HashMap<String, usize> = HashMap::new();
    for &referrer_id in referrers {
        let class_name = graph
            .get_object(referrer_id)
            .and_then(|obj| graph.class_name(obj.class_id))
            .unwrap_or("<unknown>")
            .to_string();
        *class_counts.entry(class_name).or_insert(0) += 1;
    }

    let mut top: Vec<(String, usize)> = class_counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(MAX_TOP_REFERRER_CLASSES);
    top
}

/// Render an object id as the same zero-padded hex string convention used
/// by `core::graph::gc_path` (`0x` + `2 * identifier_size` hex digits).
fn format_object_id(object_id: ObjectId, id_size: usize) -> String {
    let width = id_size * 2;
    format!("0x{object_id:0width$X}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind};

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
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

    #[test]
    fn ranks_hot_object_first_and_groups_referrer_classes() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, "com.example.Root");
        add_class(&mut graph, 2, "com.example.Hot");
        add_class(&mut graph, 3, "com.example.ClassA");
        add_class(&mut graph, 4, "com.example.ClassB");
        add_class(&mut graph, 5, "com.example.ClassC");

        // Hot(100) has 10 referrers across 3 classes: 4xClassA, 3xClassB, 3xClassC.
        let ref_a: Vec<u64> = (200..204).collect(); // 4 refs
        let ref_b: Vec<u64> = (300..303).collect(); // 3 refs
        let ref_c: Vec<u64> = (400..403).collect(); // 3 refs

        add_object(&mut graph, 100, 2, 20, &[]);
        for &id in &ref_a {
            add_object(&mut graph, id, 3, 5, &[100]);
        }
        for &id in &ref_b {
            add_object(&mut graph, id, 4, 5, &[100]);
        }
        for &id in &ref_c {
            add_object(&mut graph, id, 5, 5, &[100]);
        }

        let mut root_refs = vec![];
        root_refs.extend(&ref_a);
        root_refs.extend(&ref_b);
        root_refs.extend(&ref_c);
        add_object(&mut graph, 1, 1, 8, &root_refs);
        add_root(&mut graph, 1);

        let dominator = build_dominator_tree(&graph);
        let report = analyze_by_referrer(&graph, Some(&dominator), 5);

        assert_eq!(report.total_objects_considered, graph.objects.len());
        assert!(!report.entries.is_empty());

        let hot = &report.entries[0];
        assert_eq!(hot.class_name, "com.example.Hot");
        assert_eq!(hot.referrer_count, 10);
        assert_eq!(
            hot.top_referrer_classes,
            vec![
                ("com.example.ClassA".to_string(), 4),
                ("com.example.ClassB".to_string(), 3),
                ("com.example.ClassC".to_string(), 3),
            ]
        );
    }

    #[test]
    fn ties_on_referrer_count_break_by_retained_size_desc() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, "com.example.Root");
        add_class(&mut graph, 2, "com.example.TieA");
        add_class(&mut graph, 3, "com.example.TieB");
        add_class(&mut graph, 4, "com.example.RefTieA");
        add_class(&mut graph, 5, "com.example.RefTieB");
        add_class(&mut graph, 6, "com.example.ExtraBig");

        // TieA(50) and TieB(60) both have exactly 2 referrers each, but
        // TieA exclusively dominates a large object (ExtraBig), so its
        // retained size is much larger and it must sort first.
        add_object(&mut graph, 50, 2, 10, &[99]);
        add_object(&mut graph, 60, 3, 10, &[]);
        add_object(&mut graph, 99, 6, 1000, &[]);
        add_object(&mut graph, 71, 4, 5, &[50]);
        add_object(&mut graph, 72, 4, 5, &[50]);
        add_object(&mut graph, 81, 5, 5, &[60]);
        add_object(&mut graph, 82, 5, 5, &[60]);
        add_object(&mut graph, 1, 1, 8, &[71, 72, 81, 82]);
        add_root(&mut graph, 1);

        let dominator = build_dominator_tree(&graph);
        let report = analyze_by_referrer(&graph, Some(&dominator), 10);

        let tie_a_idx = report
            .entries
            .iter()
            .position(|e| e.class_name == "com.example.TieA")
            .expect("TieA present");
        let tie_b_idx = report
            .entries
            .iter()
            .position(|e| e.class_name == "com.example.TieB")
            .expect("TieB present");

        assert_eq!(report.entries[tie_a_idx].referrer_count, 2);
        assert_eq!(report.entries[tie_b_idx].referrer_count, 2);
        assert_eq!(report.entries[tie_a_idx].retained_size, Some(1010));
        assert_eq!(report.entries[tie_b_idx].retained_size, Some(10));
        assert!(
            tie_a_idx < tie_b_idx,
            "higher retained size must sort first on a referrer_count tie"
        );
    }

    #[test]
    fn no_dominator_tree_yields_none_retained_size() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, "com.example.Leaf");
        add_object(&mut graph, 1, 1, 16, &[]);

        let report = analyze_by_referrer(&graph, None, 10);

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].retained_size, None);
        assert_eq!(report.entries[0].referrer_count, 0);
        assert!(report.entries[0].top_referrer_classes.is_empty());
    }

    #[test]
    fn top_n_truncates_entries_but_not_total_objects_considered() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, "com.example.Leaf");
        for id in 1..=5u64 {
            add_object(&mut graph, id, 1, 16, &[]);
        }

        let report = analyze_by_referrer(&graph, None, 2);

        assert_eq!(report.total_objects_considered, 5);
        assert_eq!(report.entries.len(), 2);
    }
}
