//! Integration tests for M8 Slice 8.A: all-paths + by-class GC root path
//! enumeration (`core::graph::gc_path::{AllPathsRequest, find_all_gc_paths,
//! enumerate_gc_paths, resolve_live_instances_by_class}`).

use mnemosyne_core::{
    graph::{enumerate_gc_paths, resolve_live_instances_by_class, AllPathsRequest},
    hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
};

fn make_graph() -> ObjectGraph {
    ObjectGraph::new(8)
}

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

fn add_object(graph: &mut ObjectGraph, object_id: u64, class_id: u64, references: &[u64]) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 16,
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

/// Diamond-shaped graph:
///
/// ```text
/// Root(1) -> A(2), B(3)
/// A(2) -> Target(4)
/// B(3) -> Target(4)
/// ```
///
/// Two distinct shortest paths reach Target(4): 1->2->4 and 1->3->4.
fn build_diamond_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.BranchA");
    add_class(&mut graph, 102, "com.example.BranchB");
    add_class(&mut graph, 103, "com.example.Target");

    add_object(&mut graph, 1, 100, &[2, 3]);
    add_object(&mut graph, 2, 101, &[4]);
    add_object(&mut graph, 3, 102, &[4]);
    add_object(&mut graph, 4, 103, &[]);
    add_root(&mut graph, 1);

    graph
}

#[test]
fn diamond_fixture_returns_both_paths_to_shared_target() {
    let graph = build_diamond_graph();

    let (paths, truncated) = enumerate_gc_paths(&graph, 4, 6, 20);

    assert!(!truncated, "budget was not exhausted, should not truncate");
    assert_eq!(
        paths.len(),
        2,
        "diamond fixture should yield two distinct paths"
    );

    for path in &paths {
        assert_eq!(
            path.len(),
            3,
            "each path should be root -> branch -> target"
        );
        assert!(path[0].is_root);
        assert_eq!(path[0].class_name, "com.example.Root");
        assert_eq!(path.last().unwrap().class_name, "com.example.Target");
    }

    let branch_classes: std::collections::HashSet<_> =
        paths.iter().map(|p| p[1].class_name.clone()).collect();
    assert!(branch_classes.contains("com.example.BranchA"));
    assert!(branch_classes.contains("com.example.BranchB"));
}

#[test]
fn single_path_target_returns_one_path() {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.Leaf");
    add_object(&mut graph, 1, 100, &[2]);
    add_object(&mut graph, 2, 101, &[]);
    add_root(&mut graph, 1);

    let (paths, truncated) = enumerate_gc_paths(&graph, 2, 6, 20);

    assert!(!truncated);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].len(), 2);
}

#[test]
fn unreachable_target_returns_no_paths_without_panicking() {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.Orphan");
    add_object(&mut graph, 1, 100, &[]);
    add_object(&mut graph, 99, 101, &[]);
    add_root(&mut graph, 1);

    let (paths, truncated) = enumerate_gc_paths(&graph, 99, 6, 20);

    assert!(paths.is_empty());
    assert!(!truncated);
}

#[test]
fn max_paths_truncation_is_honest_and_does_not_panic() {
    // Fan-in graph: N branches from the root all converge on the same
    // target, giving N distinct shortest paths. Cap max_paths below N.
    // N is kept within the multi-parent BFS's per-node alternate-parent
    // cap so this test exercises the `max_paths` budget specifically,
    // not the (separately bounded) fan-in recording limit.
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.Branch");
    add_class(&mut graph, 102, "com.example.Target");

    let branch_ids: Vec<u64> = (2..7).collect(); // 5 branches
    add_object(&mut graph, 1, 100, &branch_ids);
    for &branch_id in &branch_ids {
        add_object(&mut graph, branch_id, 101, &[999]);
    }
    add_object(&mut graph, 999, 102, &[]);
    add_root(&mut graph, 1);

    let (paths, truncated) = enumerate_gc_paths(&graph, 999, 6, 3);

    assert!(!paths.is_empty(), "should still return partial results");
    assert!(paths.len() <= 3, "must respect the max_paths budget");
    assert!(
        truncated,
        "budget was exhausted before enumeration finished"
    );

    // A larger budget than the number of available paths should not
    // truncate.
    let (all_paths, truncated_full) = enumerate_gc_paths(&graph, 999, 6, 100);
    assert_eq!(all_paths.len(), 5);
    assert!(!truncated_full);
}

#[test]
fn max_paths_zero_returns_no_paths_without_panicking() {
    let graph = build_diamond_graph();

    let (paths, truncated) = enumerate_gc_paths(&graph, 4, 6, 0);

    assert!(paths.is_empty());
    assert!(!truncated);
}

#[test]
fn resolve_live_instances_by_class_finds_all_matches_sorted() {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 200, "com.example.Session");
    add_class(&mut graph, 201, "com.example.Other");

    add_object(&mut graph, 1, 100, &[30, 10, 20]);
    add_object(&mut graph, 10, 200, &[]);
    add_object(&mut graph, 20, 200, &[]);
    add_object(&mut graph, 30, 200, &[]);
    add_object(&mut graph, 40, 201, &[]);
    add_root(&mut graph, 1);

    let matches = resolve_live_instances_by_class(&graph, "com.example.Session");

    assert_eq!(matches, vec![10, 20, 30]);
}

#[test]
fn resolve_live_instances_by_class_returns_empty_for_unknown_class() {
    let graph = build_diamond_graph();

    let matches = resolve_live_instances_by_class(&graph, "com.example.DoesNotExist");

    assert!(matches.is_empty());
}

#[test]
fn by_class_budget_is_shared_across_all_matching_instances() {
    // Three live instances of the same class, each reachable via its own
    // single-hop path from the root. A shared max_paths budget of 2 should
    // return paths for only two of the three instances, honestly flagged.
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 200, "com.example.Session");

    add_object(&mut graph, 1, 100, &[10, 20, 30]);
    add_object(&mut graph, 10, 200, &[]);
    add_object(&mut graph, 20, 200, &[]);
    add_object(&mut graph, 30, 200, &[]);
    add_root(&mut graph, 1);

    let instances = resolve_live_instances_by_class(&graph, "com.example.Session");
    assert_eq!(instances.len(), 3);

    let mut budget = 2usize;
    let mut total_paths = 0usize;
    let mut truncated = false;
    for (idx, &instance_id) in instances.iter().enumerate() {
        if budget == 0 {
            truncated = true;
            break;
        }
        let (paths, hit_cap) = enumerate_gc_paths(&graph, instance_id, 6, budget);
        if hit_cap {
            truncated = true;
        }
        budget = budget.saturating_sub(paths.len());
        total_paths += paths.len();
        if budget == 0 && idx + 1 < instances.len() {
            truncated = true;
        }
    }

    assert_eq!(
        total_paths, 2,
        "shared budget caps total paths across instances"
    );
    assert!(truncated);

    // With a budget covering all three instances, nothing is truncated and
    // all three instances contribute exactly one path each.
    let (paths_0, hit_0) = enumerate_gc_paths(&graph, instances[0], 6, 20);
    let (paths_1, hit_1) = enumerate_gc_paths(&graph, instances[1], 6, 20 - paths_0.len());
    let (paths_2, hit_2) =
        enumerate_gc_paths(&graph, instances[2], 6, 20 - paths_0.len() - paths_1.len());
    assert!(!hit_0 && !hit_1 && !hit_2);
    assert_eq!(paths_0.len() + paths_1.len() + paths_2.len(), 3);
}

#[test]
fn all_paths_request_default_max_paths_constant_is_twenty() {
    assert_eq!(AllPathsRequest::DEFAULT_MAX_PATHS, 20);
}
