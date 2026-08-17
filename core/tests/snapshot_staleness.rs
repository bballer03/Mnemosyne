//! Integration tests for M9 Slice 9.B: `SnapshotStore::{list, remove,
//! find_fresh_for_heap}` and staleness-aware auto-discovery.
//!
//! Fixture-construction pattern reused verbatim from
//! `core/tests/snapshot_round_trip.rs` (Slice 9.A): a small programmatic
//! `ObjectGraph` built via direct field insertion, plus `write_temp_heap`
//! for a real on-disk heap file to hash.
//!
//! Validation gates (per milestone-9-snapshot-persistence.md §9 Slice 9.B,
//! §11):
//! - `list()` returns manifests for multiple saved snapshots and skips a
//!   corrupt entry without erroring the whole listing.
//! - `remove()` deletes an entry; a second `remove()` on the same key
//!   returns `snapshot_not_found`.
//! - `find_fresh_for_heap()` returns `None` (not an error) on a first-run
//!   cache miss.
//! - `find_fresh_for_heap()` returns `Some(payload)` after a prior `save()`
//!   for the same heap bytes.
//! - `find_fresh_for_heap()` returns `None` (not an error) when the cached
//!   entry's `schema_version` doesn't match `SNAPSHOT_SCHEMA_VERSION` --
//!   auto-discovery misses (including staleness) are silent per §7.

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

fn build_fixture_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 100, "com.example.Root");
    add_class(&mut graph, 101, "com.example.Target");
    add_object(&mut graph, 1, 100, &[2]);
    add_object(&mut graph, 2, 101, &[]);
    add_root(&mut graph, 1);
    graph
}

fn write_temp_heap(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn list_returns_manifests_for_multiple_saved_snapshots() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);

    let heap_a = write_temp_heap(b"heap A bytes");
    let heap_b = write_temp_heap(b"heap B bytes, different");

    let manifest_a = store
        .save(heap_a.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();
    let manifest_b = store
        .save(heap_b.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    let mut listed = store.list().unwrap();
    listed.sort_by(|a, b| a.heap_sha256.cmp(&b.heap_sha256));

    let mut expected_hashes = vec![manifest_a.heap_sha256, manifest_b.heap_sha256];
    expected_hashes.sort();

    assert_eq!(listed.len(), 2);
    let listed_hashes: Vec<String> = listed.iter().map(|m| m.heap_sha256.clone()).collect();
    assert_eq!(listed_hashes, expected_hashes);
}

#[test]
fn list_skips_corrupt_entries_without_erroring() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);

    let heap = write_temp_heap(b"valid heap bytes");
    let manifest = store
        .save(heap.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    // A corrupt sibling entry must not break the whole listing.
    store.ensure_root().unwrap();
    std::fs::write(
        store_dir.path().join("corrupt-entry.json"),
        b"{ not valid json at all",
    )
    .unwrap();

    let listed = store.list().unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].heap_sha256, manifest.heap_sha256);
}

#[test]
fn list_on_empty_store_returns_empty_vec() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());

    let listed = store.list().unwrap();

    assert!(listed.is_empty());
}

#[test]
fn remove_deletes_entry_and_second_remove_returns_not_found() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);
    let heap = write_temp_heap(b"heap to remove");

    let manifest = store
        .save(heap.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    store.remove(&manifest.heap_sha256).unwrap();

    // First remove worked: load now fails with snapshot_not_found.
    let load_err = store.load(&manifest.heap_sha256).unwrap_err();
    assert!(load_err.to_string().contains("snapshot_not_found"));

    // Second remove on the same key is a loud not-found, not a silent no-op.
    let remove_err = store.remove(&manifest.heap_sha256).unwrap_err();
    assert!(remove_err.to_string().contains("snapshot_not_found"));
}

#[test]
fn remove_of_never_saved_key_returns_snapshot_not_found() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());

    let err = store.remove("never-saved-key").unwrap_err();

    assert!(err.to_string().contains("snapshot_not_found"));
}

#[test]
fn find_fresh_for_heap_returns_none_on_first_run_cache_miss() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let heap = write_temp_heap(b"never snapshotted heap bytes");

    let result = store
        .find_fresh_for_heap(heap.path().to_str().unwrap())
        .unwrap();

    assert!(result.is_none());
}

#[test]
fn find_fresh_for_heap_returns_some_after_prior_save() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);
    let heap = write_temp_heap(b"heap bytes with a fresh cache entry");

    let manifest = store
        .save(heap.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    let found = store
        .find_fresh_for_heap(heap.path().to_str().unwrap())
        .unwrap();

    let payload = found.expect("expected a fresh cache hit");
    assert_eq!(payload.manifest.heap_sha256, manifest.heap_sha256);
    assert_eq!(payload.object_graph.objects.len(), graph.objects.len());
}

#[test]
fn find_fresh_for_heap_returns_none_when_schema_version_mismatches() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);
    let heap = write_temp_heap(b"heap bytes behind a stale schema version");

    let manifest = store
        .save(heap.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    // Simulate a snapshot written by an older/newer binary: hand-edit the
    // saved file's schema_version so it no longer matches
    // SNAPSHOT_SCHEMA_VERSION, without changing the file name (the cache
    // key), mirroring a real stale-schema scenario.
    let path = store_dir
        .path()
        .join(format!("{}.json", manifest.heap_sha256));
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    value["manifest"]["schema_version"] = serde_json::json!(SNAPSHOT_SCHEMA_VERSION + 1);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let result = store
        .find_fresh_for_heap(heap.path().to_str().unwrap())
        .unwrap();

    assert!(
        result.is_none(),
        "stale schema_version must be a silent auto-discovery miss, not an error or a stale hit"
    );
}

#[test]
fn find_fresh_for_heap_returns_none_when_heap_bytes_changed_since_save() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = SnapshotStore::new(store_dir.path().to_path_buf());
    let graph = build_fixture_graph();
    let dominator = build_dominator_tree(&graph);
    let mut heap = write_temp_heap(b"original heap bytes");

    store
        .save(heap.path().to_str().unwrap(), &graph, &dominator)
        .unwrap();

    // Mutate the heap file on disk after the snapshot was taken -- its
    // sha256 no longer matches any cache entry, so this is a plain cache
    // miss for the new hash (not the stale one).
    heap.as_file_mut().set_len(0).unwrap();
    use std::io::Seek;
    heap.as_file_mut()
        .seek(std::io::SeekFrom::Start(0))
        .unwrap();
    heap.write_all(b"mutated heap bytes, totally different content")
        .unwrap();
    heap.flush().unwrap();

    let result = store
        .find_fresh_for_heap(heap.path().to_str().unwrap())
        .unwrap();

    assert!(result.is_none());
}
