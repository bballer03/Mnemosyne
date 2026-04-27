use std::collections::HashMap;

use mnemosyne_core::{
    build_dominator_tree,
    diff::{
        object::{
            engine::{diff_object_graphs_with_limit, ObjectDiffEngineConfig},
            match_quality::compute_collision_rate,
        },
        DiffRequest,
    },
    hprof::{
        ClassDelta, ClassInfo, GcRoot, GcRootType, HeapDiff, HeapObject, ObjectGraph, ObjectKind,
    },
    IdentityStrategy,
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
    before: &ObjectGraph,
    after: &ObjectGraph,
    request: DiffRequest,
) -> mnemosyne_core::ObjectDiffReport {
    let before_dom = build_dominator_tree(before);
    let after_dom = build_dominator_tree(after);

    diff_object_graphs_with_limit(
        before,
        &before_dom,
        after,
        &after_dom,
        ObjectDiffEngineConfig {
            strategy: request.identity_strategy,
            bucket_bits: request.retained_bucket_bits,
            min_retained_bytes: request.min_retained_bytes,
            retained_change_threshold: request.retained_change_threshold,
            top_n: request.top_n,
            max_fingerprints: usize::MAX,
        },
    )
    .expect("object diff should succeed")
}

fn build_engine_before_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.ParentAdded");
    add_class(&mut graph, 102, "com.example.ParentRemoved");
    add_class(&mut graph, 103, "com.example.ParentRetained");
    add_class(&mut graph, 104, "com.example.ParentStable");
    add_class(&mut graph, 105, "com.example.AddedOnly");
    add_class(&mut graph, 106, "com.example.RemovedOnly");
    add_class(&mut graph, 107, "com.example.RetainedChange");
    add_class(&mut graph, 108, "com.example.Stable");

    add_object(&mut graph, 1, 100, 10, &[10, 20, 30, 40]);
    add_object(&mut graph, 10, 101, 10, &[]);
    add_object(&mut graph, 20, 102, 10, &[21]);
    add_object(&mut graph, 21, 106, 30, &[]);
    add_object(&mut graph, 30, 103, 10, &[31]);
    add_object(&mut graph, 31, 107, 10, &[]);
    add_object(&mut graph, 40, 104, 10, &[41]);
    add_object(&mut graph, 41, 108, 20, &[]);
    add_root(&mut graph, 1);

    graph
}

fn build_engine_after_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 900, "com.example.Root");
    add_class(&mut graph, 901, "com.example.ParentAdded");
    add_class(&mut graph, 902, "com.example.ParentRemoved");
    add_class(&mut graph, 903, "com.example.ParentRetained");
    add_class(&mut graph, 904, "com.example.ParentStable");
    add_class(&mut graph, 905, "com.example.AddedOnly");
    add_class(&mut graph, 906, "com.example.RemovedOnly");
    add_class(&mut graph, 907, "com.example.RetainedChange");
    add_class(&mut graph, 908, "com.example.Stable");
    add_class(&mut graph, 909, "com.example.AddedLeaf");

    add_object(&mut graph, 11, 900, 10, &[12, 13, 14, 15]);
    add_object(&mut graph, 12, 901, 10, &[16]);
    add_object(&mut graph, 13, 902, 10, &[]);
    add_object(&mut graph, 14, 903, 10, &[17, 18]);
    add_object(&mut graph, 15, 904, 10, &[19]);
    add_object(&mut graph, 16, 905, 15, &[160]);
    add_object(&mut graph, 160, 909, 25, &[]);
    add_object(&mut graph, 17, 907, 40, &[]);
    add_object(&mut graph, 18, 909, 40, &[]);
    add_object(&mut graph, 19, 908, 20, &[]);
    add_root(&mut graph, 11);

    graph
}

fn build_cap_before_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.AddParent");
    add_class(&mut graph, 102, "com.example.RemoveParent");
    add_class(&mut graph, 103, "com.example.ChangeParent");
    add_class(&mut graph, 104, "com.example.AddItemA");
    add_class(&mut graph, 105, "com.example.AddItemB");
    add_class(&mut graph, 106, "com.example.AddItemC");
    add_class(&mut graph, 107, "com.example.RemoveItemA");
    add_class(&mut graph, 108, "com.example.RemoveItemB");
    add_class(&mut graph, 109, "com.example.RemoveItemC");
    add_class(&mut graph, 110, "com.example.ChangeItemA");
    add_class(&mut graph, 111, "com.example.ChangeItemB");
    add_class(&mut graph, 112, "com.example.ChangeItemC");

    add_object(&mut graph, 1, 100, 10, &[10, 20, 30]);
    add_object(&mut graph, 10, 101, 10, &[]);
    add_object(&mut graph, 20, 102, 10, &[21, 22, 23]);
    add_object(&mut graph, 21, 107, 10, &[]);
    add_object(&mut graph, 22, 108, 20, &[]);
    add_object(&mut graph, 23, 109, 30, &[]);
    add_object(&mut graph, 30, 103, 10, &[31, 32, 33]);
    add_object(&mut graph, 31, 110, 10, &[]);
    add_object(&mut graph, 32, 111, 20, &[]);
    add_object(&mut graph, 33, 112, 30, &[]);
    add_root(&mut graph, 1);

    graph
}

fn build_cap_after_graph() -> ObjectGraph {
    let mut graph = make_graph();
    add_class(&mut graph, 900, "com.example.Root");
    add_class(&mut graph, 901, "com.example.AddParent");
    add_class(&mut graph, 902, "com.example.RemoveParent");
    add_class(&mut graph, 903, "com.example.ChangeParent");
    add_class(&mut graph, 904, "com.example.AddItemA");
    add_class(&mut graph, 905, "com.example.AddItemB");
    add_class(&mut graph, 906, "com.example.AddItemC");
    add_class(&mut graph, 907, "com.example.RemoveItemA");
    add_class(&mut graph, 908, "com.example.RemoveItemB");
    add_class(&mut graph, 909, "com.example.RemoveItemC");
    add_class(&mut graph, 910, "com.example.ChangeItemA");
    add_class(&mut graph, 911, "com.example.ChangeItemB");
    add_class(&mut graph, 912, "com.example.ChangeItemC");

    add_object(&mut graph, 11, 900, 10, &[12, 13, 14]);
    add_object(&mut graph, 12, 901, 10, &[15, 16, 17]);
    add_object(&mut graph, 13, 902, 10, &[]);
    add_object(&mut graph, 14, 903, 10, &[18, 19, 20]);
    add_object(&mut graph, 15, 904, 10, &[]);
    add_object(&mut graph, 16, 905, 20, &[]);
    add_object(&mut graph, 17, 906, 30, &[]);
    add_object(&mut graph, 18, 910, 40, &[180]);
    add_object(&mut graph, 180, 904, 10, &[]);
    add_object(&mut graph, 19, 911, 50, &[190]);
    add_object(&mut graph, 190, 905, 20, &[]);
    add_object(&mut graph, 20, 912, 60, &[200]);
    add_object(&mut graph, 200, 906, 30, &[]);
    add_root(&mut graph, 11);

    graph
}

fn default_request() -> DiffRequest {
    DiffRequest {
        before_path: String::from("before.hprof"),
        after_path: String::from("after.hprof"),
        mode: mnemosyne_core::DiffMode::Object,
        identity_strategy: IdentityStrategy::ClassDominator,
        retained_bucket_bits: 10,
        min_retained_bytes: 0,
        retained_change_threshold: 1,
        top_n: 50,
        retain_field_data: false,
    }
}

#[test]
fn engine_populates_all_three_sections() {
    let before = build_engine_before_graph();
    let after = build_engine_after_graph();

    let report = run_diff(&before, &after, default_request());

    assert!(report
        .added
        .iter()
        .any(|delta| delta.class_name == "com.example.AddedOnly"));
    assert!(report
        .removed
        .iter()
        .any(|delta| delta.class_name == "com.example.RemovedOnly"));
    assert!(report
        .retained_changed
        .iter()
        .any(|delta| delta.class_name == "com.example.RetainedChange"));
}

#[test]
fn engine_enforces_top_n_cap_per_section() {
    let before = build_cap_before_graph();
    let after = build_cap_after_graph();
    let mut request = default_request();
    request.top_n = 2;

    let report = run_diff(&before, &after, request);

    assert_eq!(report.added.len(), 2);
    assert_eq!(report.removed.len(), 2);
    assert_eq!(report.retained_changed.len(), 2);
}

#[test]
fn engine_emits_dominator_chain_in_delta() {
    let before = build_engine_before_graph();
    let after = build_engine_after_graph();

    let report = run_diff(&before, &after, default_request());
    let added_only = report
        .added
        .iter()
        .find(|delta| delta.class_name == "com.example.AddedOnly")
        .expect("added delta for AddedOnly");

    assert!(!added_only.dominator_chain.is_empty());
    assert!(added_only
        .dominator_chain
        .iter()
        .any(|item| item == "com.example.ParentAdded"));
}

#[test]
fn engine_emits_reference_chain_in_delta() {
    let before = build_engine_before_graph();
    let after = build_engine_after_graph();

    let report = run_diff(&before, &after, default_request());
    let added_only = report
        .added
        .iter()
        .find(|delta| delta.class_name == "com.example.AddedOnly")
        .expect("added delta for AddedOnly");

    assert!(!added_only.reference_chain.is_empty());
    assert_eq!(added_only.reference_chain.first().unwrap(), "GC Root");
}

#[test]
fn engine_returns_structured_error_when_fingerprint_budget_exceeded() {
    let before = build_cap_before_graph();
    let after = build_cap_after_graph();
    let before_dom = build_dominator_tree(&before);
    let after_dom = build_dominator_tree(&after);

    let error = diff_object_graphs_with_limit(
        &before,
        &before_dom,
        &after,
        &after_dom,
        ObjectDiffEngineConfig {
            strategy: IdentityStrategy::ClassDominator,
            bucket_bits: 0,
            min_retained_bytes: 0,
            retained_change_threshold: 1,
            top_n: 100,
            max_fingerprints: 1,
        },
    )
    .expect_err("budget overflow should error");

    let json = serde_json::to_value(&error).expect("serializable error envelope");
    assert_eq!(
        json.get("code").and_then(|v| v.as_str()),
        Some("feature_unavailable_object_diff_too_large")
    );
    assert_eq!(
        json.pointer("/details/max_fingerprints")
            .and_then(|v| v.as_u64()),
        Some(1)
    );
}

#[test]
fn match_quality_collision_rate_matches_reference() {
    let mut graph = make_graph();
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.ParentA");
    add_class(&mut graph, 102, "com.example.ParentB");
    add_class(&mut graph, 103, "com.example.Cache");

    add_object(&mut graph, 1, 100, 10, &[2, 3]);
    add_object(&mut graph, 2, 101, 10, &[4]);
    add_object(&mut graph, 3, 102, 10, &[5]);
    add_object(&mut graph, 4, 103, 10, &[]);
    add_object(&mut graph, 5, 103, 10, &[]);
    add_root(&mut graph, 1);

    let dom = build_dominator_tree(&graph);
    let collision_rate =
        compute_collision_rate(&graph, &dom, IdentityStrategy::ClassRetained, 10, 0)
            .expect("collision rate should compute");

    let mut counts = HashMap::new();
    for &obj_id in graph.objects.keys() {
        let fingerprint = mnemosyne_core::ObjectFingerprint::build(
            &graph,
            &dom,
            obj_id,
            IdentityStrategy::ClassRetained,
            10,
        )
        .expect("fingerprint should build");
        *counts.entry(fingerprint).or_insert(0_u64) += 1;
    }
    let collisions: u64 = counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1)
        .sum();
    let expected = collisions as f64 / graph.object_count() as f64;

    assert_eq!(collision_rate, expected);
}

#[test]
fn heap_diff_json_unchanged_when_object_diff_is_none() {
    let diff = HeapDiff {
        before: String::from("before.hprof"),
        after: String::from("after.hprof"),
        delta_bytes: 12,
        delta_objects: 3,
        changed_classes: vec![ClassDelta {
            name: String::from("com.example.Cache"),
            before_bytes: 10,
            after_bytes: 22,
        }],
        class_diff: None,
        object_diff: None,
    };

    let actual = serde_json::to_vec(&diff).expect("diff should serialize");
    let expected = br#"{"before":"before.hprof","after":"after.hprof","delta_bytes":12,"delta_objects":3,"changed_classes":[{"name":"com.example.Cache","before_bytes":10,"after_bytes":22}]}"#;

    assert_eq!(actual, expected);
}
