//! Integration tests for M9 Slice 9.A: serializable `DominatorTree` +
//! `core::snapshot::{SnapshotManifest, SnapshotPayload, SnapshotStore}`.
//!
//! Fixture-construction pattern reused from `core/tests/gc_path_all_paths.rs`
//! / `core/tests/analyze_by_referrer.rs` (both on `main` already): a small
//! programmatic `ObjectGraph` built via direct field insertion rather than
//! parsing real HPROF bytes.
//!
//! Validation gates (per milestone-9-snapshot-persistence.md §11):
//! - save -> load round-trip produces identical results to the pre-save
//!   graph/dominator-tree for every query method: `get_object`,
//!   `get_references`, `get_referrers`, `immediate_dominator`,
//!   `dominated_by`, `retained_size`, `top_retained`.
//! - Corrupt/truncated snapshot file -> structured error, not a panic.
//! - `save` followed by `load` using the manifest's own `heap_sha256` as the
//!   key works (the actual intended usage pattern).

use mnemosyne_core::{
    graph::build_dominator_tree,
    hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
    snapshot::{SnapshotStore, SNAPSHOT_SCHEMA_VERSION},
};
use std::io::Write;
use tempfile::NamedTempFile;

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

/// Diamond-shaped graph with a linear tail, exercising every query method
/// under test:
///
/// ```text
/// Root(1) -> A(2), B(3)
/// A(2) -> Target(4)
/// B(3) -> Target(4)
/// Target(4) -> Leaf(5)
/// ```
///
/// `Target(4)` has two referrers (A and B) and is immediately dominated by
/// `Root(1)` (not A or B, since both paths converge there).
fn build_fixture_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.BranchA");
    add_class(&mut graph, 102, "com.example.BranchB");
    add_class(&mut graph, 103, "com.example.Target");
    add_class(&mut graph, 104, "com.example.Leaf");

    add_object(&mut graph, 1, 100, &[2, 3]);
    add_object(&mut graph, 2, 101, &[4]);
    add_object(&mut graph, 3, 102, &[4]);
    add_object(&mut graph, 4, 103, &[5]);
    add_object(&mut graph, 5, 104, &[]);
    add_root(&mut graph, 1);

    graph
}

fn write_temp_heap(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file
}

/// Assert every query method under test returns identical results between
/// the original (pre-save) and loaded (post-load) graph/dominator-tree
/// pairs, across every object ID in the fixture (plus a miss case).
fn assert_identical_query_results(
    original_graph: &ObjectGraph,
    original_dom: &mnemosyne_core::graph::DominatorTree,
    loaded_graph: &ObjectGraph,
    loaded_dom: &mnemosyne_core::graph::DominatorTree,
) {
    for id in [1u64, 2, 3, 4, 5, 9999 /* miss */] {
        assert_eq!(
            original_graph.get_object(id).map(|o| o.id),
            loaded_graph.get_object(id).map(|o| o.id),
            "get_object mismatch for id {id}"
        );
        assert_eq!(
            original_graph.get_references(id),
            loaded_graph.get_references(id),
            "get_references mismatch for id {id}"
        );
        // `get_referrers` is recomputed live by scanning `ObjectGraph.objects`
        // (a `HashMap`), whose iteration order is randomized per-instance and
        // therefore differs between the original graph and the deserialized
        // copy even though their contents are identical. Compare as sorted
        // sets rather than ordered sequences -- order was never part of the
        // method's contract.
        let mut original_referrers = original_graph.get_referrers(id);
        let mut loaded_referrers = loaded_graph.get_referrers(id);
        original_referrers.sort_unstable();
        loaded_referrers.sort_unstable();
        assert_eq!(
            original_referrers, loaded_referrers,
            "get_referrers mismatch for id {id}"
        );
        assert_eq!(
            original_dom.immediate_dominator(id),
            loaded_dom.immediate_dominator(id),
            "immediate_dominator mismatch for id {id}"
        );
        assert_eq!(
            original_dom.dominated_by(id),
            loaded_dom.dominated_by(id),
            "dominated_by mismatch for id {id}"
        );
        assert_eq!(
            original_dom.retained_size(id),
            loaded_dom.retained_size(id),
            "retained_size mismatch for id {id}"
        );
    }

    assert_eq!(
        original_dom.top_retained(10),
        loaded_dom.top_retained(10),
        "top_retained mismatch"
    );
}

#[test]
fn save_load_round_trip_preserves_all_query_results() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let heap_file = write_temp_heap(b"synthetic heap bytes for M9 slice 9.A");

    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);

    let manifest = store
        .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    assert_eq!(manifest.schema_version, SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(manifest.object_count, graph.objects.len());
    assert!(!manifest.heap_sha256.is_empty());

    // The intended usage pattern: load by the manifest's own heap_sha256.
    let loaded = store.load(&manifest.heap_sha256).unwrap();

    assert_eq!(loaded.manifest.heap_sha256, manifest.heap_sha256);
    assert_eq!(loaded.manifest.object_count, manifest.object_count);

    assert_identical_query_results(
        &graph,
        &dominator,
        &loaded.object_graph,
        &loaded.dominator_tree,
    );
}

#[test]
fn save_is_idempotent_for_the_same_heap_bytes() {
    // Same heap bytes -> same sha256 -> same target file. On Windows a
    // naive `fs::rename` would fail here because the target already
    // exists; SnapshotStore must handle this (mirrors McpSessionStore's
    // atomic-write-with-backup shape).
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let heap_file = write_temp_heap(b"stable heap bytes");
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);

    let first = store
        .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();
    let second = store
        .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    assert_eq!(first.heap_sha256, second.heap_sha256);
    let loaded = store.load(&second.heap_sha256).unwrap();
    assert_eq!(loaded.object_graph.objects.len(), graph.objects.len());
}

#[test]
fn load_of_corrupt_file_returns_structured_error_not_panic() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    store.ensure_root().unwrap();

    // Truncated / invalid JSON, simulating a crash mid-write or hand-edit.
    std::fs::write(
        store_dir.path().join("corrupt-key.json"),
        b"{ \"manifest\": { \"schema_version\": 1, truncated",
    )
    .unwrap();

    let result = store.load("corrupt-key");

    let err = result.expect_err("corrupt snapshot must error, not panic");
    assert!(
        err.to_string().contains("snapshot_corrupt"),
        "expected snapshot_corrupt error, got: {err}"
    );
}

#[test]
fn load_of_missing_key_returns_snapshot_not_found() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());

    let err = store.load("never-saved").unwrap_err();

    assert!(err.to_string().contains("snapshot_not_found"));
}

#[test]
fn empty_graph_round_trips_through_snapshot_store() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let heap_file = write_temp_heap(b"empty graph heap");

    let graph = ObjectGraph::new(8);
    let dominator = build_dominator_tree(&graph);

    let manifest = store
        .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();
    assert_eq!(manifest.object_count, 0);

    let loaded = store.load(&manifest.heap_sha256).unwrap();
    assert_eq!(loaded.object_graph.objects.len(), 0);
    assert_eq!(loaded.dominator_tree.node_count(), 0);
    assert!(loaded.dominator_tree.top_retained(10).is_empty());
}
