use std::collections::HashMap;

use mnemosyne_core::{
    build_dominator_tree,
    diff::object::engine::diff_object_graphs,
    hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
    IdentityStrategy, ObjectDiffReport, ObjectFingerprint,
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

fn run_diff(
    before: ObjectGraph,
    after: ObjectGraph,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained: u64,
) -> ObjectDiffReport {
    let before_dom = build_dominator_tree(&before);
    let after_dom = build_dominator_tree(&after);

    diff_object_graphs(
        &before,
        &before_dom,
        &after,
        &after_dom,
        strategy,
        bucket_bits,
        min_retained,
    )
    .expect("object diff should succeed")
}

fn build_cache_move_before_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.ServiceA");
    add_class(&mut graph, 102, "com.example.ServiceB");
    add_class(&mut graph, 103, "com.example.Cache");

    add_object(&mut graph, 1, 100, 16, &[2, 3]);
    add_object(&mut graph, 2, 101, 16, &[4]);
    add_object(&mut graph, 3, 102, 16, &[]);
    add_object(&mut graph, 4, 103, 16, &[]);
    add_root(&mut graph, 1);

    graph
}

fn build_cache_move_after_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 900, "com.example.Root");
    add_class(&mut graph, 901, "com.example.ServiceA");
    add_class(&mut graph, 902, "com.example.ServiceB");
    add_class(&mut graph, 903, "com.example.Cache");

    add_object(&mut graph, 11, 900, 16, &[12, 13]);
    add_object(&mut graph, 12, 901, 16, &[]);
    add_object(&mut graph, 13, 902, 16, &[14]);
    add_object(&mut graph, 14, 903, 16, &[]);
    add_root(&mut graph, 11);

    graph
}

fn build_collision_fixture_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.ServiceA");
    add_class(&mut graph, 102, "com.example.ServiceB");
    add_class(&mut graph, 103, "com.example.Cache");

    add_object(&mut graph, 1, 100, 16, &[2, 3]);
    add_object(&mut graph, 2, 101, 16, &[4]);
    add_object(&mut graph, 3, 102, 16, &[5]);
    add_object(&mut graph, 4, 103, 16, &[]);
    add_object(&mut graph, 5, 103, 16, &[]);
    add_root(&mut graph, 1);

    graph
}

fn collision_rate(graph: &ObjectGraph, strategy: IdentityStrategy, bucket_bits: u8) -> f64 {
    let dom = build_dominator_tree(graph);
    let mut counts = HashMap::new();

    for &obj_id in graph.objects.keys() {
        let fingerprint = ObjectFingerprint::build(graph, &dom, obj_id, strategy, bucket_bits)
            .expect("fingerprint should build");
        *counts.entry(fingerprint).or_insert(0_u64) += 1;
    }

    let collisions: u64 = counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1)
        .sum();

    collisions as f64 / graph.object_count() as f64
}

fn stable_class_name_id(class_name: &str) -> u32 {
    let mut hash = 0x811c9dc5_u32;

    for byte in class_name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }

    hash
}

#[test]
fn class_dominator_separates_siblings_with_distinct_parents() {
    let before = build_cache_move_before_graph();
    let after = build_cache_move_after_graph();

    let class_retained = run_diff(
        before.clone(),
        after.clone(),
        IdentityStrategy::ClassRetained,
        10,
        0,
    );
    assert!(class_retained.added.is_empty());
    assert!(class_retained.removed.is_empty());

    let class_dominator = run_diff(before, after, IdentityStrategy::ClassDominator, 10, 0);
    assert_eq!(class_dominator.added.len(), 1);
    assert_eq!(class_dominator.removed.len(), 1);
    assert_eq!(class_dominator.added[0].class_name, "com.example.Cache");
    assert_eq!(class_dominator.removed[0].class_name, "com.example.Cache");
}

#[test]
fn class_dominator_collision_rate_lower_than_class_retained() {
    let graph = build_collision_fixture_graph();

    let class_retained_rate = collision_rate(&graph, IdentityStrategy::ClassRetained, 10);
    let class_dominator_rate = collision_rate(&graph, IdentityStrategy::ClassDominator, 10);

    assert_eq!(class_retained_rate, 0.2);
    assert_eq!(class_dominator_rate, 0.0);
    assert!(class_dominator_rate < class_retained_rate);
}

#[test]
fn class_retained_results_unchanged_from_slice_a() {
    let mut before = make_graph();
    add_class(&mut before, 100, "com.example.Session");
    add_object(&mut before, 1, 100, 10, &[]);
    add_root(&mut before, 1);

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Session");
    add_class(&mut after, 901, "com.example.Cache");
    add_object(&mut after, 11, 900, 10, &[]);
    add_object(&mut after, 12, 901, 20, &[]);
    add_root(&mut after, 11);
    add_root(&mut after, 12);

    let report = run_diff(before, after, IdentityStrategy::ClassRetained, 0, 0);

    assert_eq!(report.strategy, IdentityStrategy::ClassRetained);
    assert_eq!(report.retained_bucket_bits, 0);
    assert!(report.removed.is_empty());
    assert!(report.retained_changed.is_empty());
    assert_eq!(report.added.len(), 1);
    assert_eq!(report.match_quality.collision_rate, 0.0);
    assert_eq!(report.totals.matched_pairs, 1);

    let delta = &report.added[0];
    assert_eq!(delta.class_name, "com.example.Cache");
    assert_eq!(
        delta.fingerprint,
        ObjectFingerprint {
            class_id: stable_class_name_id("com.example.Cache"),
            retained_bucket: 20,
            dominator_signature: 0,
            field_signature: 0,
        }
    );
    assert_eq!(delta.example_object_id, 12);
    assert_eq!(delta.before_count, 0);
    assert_eq!(delta.after_count, 1);
    assert_eq!(delta.before_retained_bytes, 0);
    assert_eq!(delta.after_retained_bytes, 20);
}
