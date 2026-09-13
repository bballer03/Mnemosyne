//! Integration tests for M11 Slice 11.C: the `CompareSnapshots` workflow
//! kind (`core::workflow::compare_snapshots`), exercised through the shared
//! `core::workflow::{start, advance, describe}` state machine.
//!
//! Mirrors `core/tests/workflow_triage_memory_leak.rs` /
//! `workflow_tune_gc.rs`'s structure (design doc §10 test strategy): a full
//! run to completion, the §6.1 mandatory contract test (this kind is
//! linear/non-looping, so it follows `triage_memory_leak`'s/`tune_gc`'s
//! exact-sequence-equality pattern, not `traverse_object_graph`'s
//! distinct-names pattern), "matches calling the underlying primitive
//! directly" equivalence checks (§3.3's "zero new analysis logic" claim),
//! and negative-path coverage.

use std::io::Write;

use mnemosyne_core::{
    diff::{run_diff, DiffMode, DiffRequest, DiffResult, IdentityStrategy},
    graph::build_dominator_tree,
    hprof::{
        parse_hprof_file_with_options,
        test_fixtures::{build_graph_fixture, build_tune_gc_fixture},
        ParseOptions,
    },
    snapshot::SnapshotStore,
    workflow::{advance, describe, start, WorkflowKind, WorkflowState, WorkflowStore},
};
use serde_json::{json, Value};
use tempfile::NamedTempFile;

fn write_fixture_heap(bytes: Vec<u8>) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&bytes).unwrap();
    file.flush().unwrap();
    file
}

fn heap_path(file: &NamedTempFile) -> String {
    file.path().to_str().unwrap().to_string()
}

/// Builds the same `DiffRequest` shape `core::workflow::compare_snapshots`'s
/// `diff` step uses internally (and that the `diff_heaps` MCP tool / CLI
/// `diff` command use for object-mode diffing by default) -- an
/// independent copy here, not a call into the crate-private helper, so this
/// test is a genuine "matches calling the primitive directly" equivalence
/// check per design doc §3.3/§10, not a tautology.
fn default_object_diff_request(before_path: String, after_path: String) -> DiffRequest {
    DiffRequest {
        before_path,
        after_path,
        mode: DiffMode::Object,
        identity_strategy: IdentityStrategy::default(),
        retained_bucket_bits: 10,
        min_retained_bytes:
            mnemosyne_core::diff::object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
        retained_change_threshold:
            mnemosyne_core::diff::object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
        top_n: mnemosyne_core::diff::object::types::DEFAULT_OBJECT_DIFF_TOP_N,
        retain_field_data: false,
        cross_reference_leaks: false,
    }
}

/// Run a full `CompareSnapshots` workflow to completion given `initial_params`
/// (either heap-path or snapshot-key form). Returns the final state.
async fn run_full_workflow(store: &WorkflowStore, initial_params: Value) -> WorkflowState {
    let state = start(
        store,
        WorkflowKind::CompareSnapshots,
        String::new(),
        initial_params,
    )
    .await
    .expect("start should succeed");
    assert_eq!(state.current_step, "diff");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("diff step should succeed");
    assert_eq!(state.current_step, "complete");

    state
}

#[tokio::test]
async fn full_run_step_sequence_matches_static_description_exactly() {
    // The mandatory contract test named in design doc §6.1 / roadmap.md's
    // own M11 risk register entry (R1), same exact-sequence-equality
    // pattern `triage_memory_leak`/`tune_gc` already use (this kind has no
    // branch point, unlike `traverse_object_graph`).
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let state = run_full_workflow(
        &workflow_store,
        json!({
            "before_heap_path": heap_path(&before_file),
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await;

    let observed: Vec<String> = state
        .step_history
        .iter()
        .map(|record| record.step_name.clone())
        .collect();

    let description = describe(WorkflowKind::CompareSnapshots).unwrap();
    let expected: Vec<String> = description
        .steps
        .iter()
        .map(|step| step.name.clone())
        .collect();

    assert_eq!(
        observed, expected,
        "runtime step sequence must exactly match describe()'s static schema"
    );
    assert_eq!(state.current_step, "complete");
    // The placeholder `heap_path` `start` was called with is overwritten by
    // `resolve_snapshots` with a synthetic before/after description (see
    // `compare_snapshots`'s module doc comment).
    assert!(state.heap_path.contains(" -> "));
    assert!(state.heap_path.contains(&heap_path(&before_file)));
    assert!(state.heap_path.contains(&heap_path(&after_file)));
}

#[tokio::test]
async fn resolve_snapshots_saves_new_snapshots_for_heap_paths_with_no_existing_cache() {
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let state = start(
        &workflow_store,
        WorkflowKind::CompareSnapshots,
        String::new(),
        json!({
            "before_heap_path": heap_path(&before_file),
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await
    .expect("start should succeed against two real fixture heaps with no pre-existing snapshots");

    let resolve_output = &state.step_history[0].output_summary;
    assert!(resolve_output["before"]["snapshotted_now"]
        .as_bool()
        .unwrap());
    assert!(resolve_output["after"]["snapshotted_now"]
        .as_bool()
        .unwrap());

    let snapshot_store = SnapshotStore::new(snapshot_dir.path().to_path_buf());
    let manifests = snapshot_store.list().unwrap();
    assert_eq!(
        manifests.len(),
        2,
        "resolve_snapshots should have saved exactly one new snapshot per side"
    );
}

#[tokio::test]
async fn resolve_snapshots_skips_save_for_already_cached_snapshot_keys() {
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let snapshot_dir = tempfile::tempdir().unwrap();
    let snapshot_store = SnapshotStore::new(snapshot_dir.path().to_path_buf());

    // Pre-seed both snapshots directly via `SnapshotStore::save`, exactly
    // what a prior `resolve_snapshots` (or CLI `--snapshot`, or a previous
    // workflow run) would have done.
    let before_graph =
        parse_hprof_file_with_options(&heap_path(&before_file), ParseOptions::default()).unwrap();
    let before_dom = build_dominator_tree(&before_graph);
    let before_manifest = snapshot_store
        .save(&heap_path(&before_file), &before_graph, &before_dom)
        .unwrap();

    let after_graph =
        parse_hprof_file_with_options(&heap_path(&after_file), ParseOptions::default()).unwrap();
    let after_dom = build_dominator_tree(&after_graph);
    let after_manifest = snapshot_store
        .save(&heap_path(&after_file), &after_graph, &after_dom)
        .unwrap();

    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());

    let state = start(
        &workflow_store,
        WorkflowKind::CompareSnapshots,
        String::new(),
        json!({
            "before_snapshot_key": before_manifest.heap_sha256,
            "after_snapshot_key": after_manifest.heap_sha256,
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await
    .expect("start should succeed given two already-cached snapshot keys");

    let resolve_output = &state.step_history[0].output_summary;
    assert!(!resolve_output["before"]["snapshotted_now"]
        .as_bool()
        .unwrap());
    assert!(!resolve_output["after"]["snapshotted_now"]
        .as_bool()
        .unwrap());
    assert_eq!(
        resolve_output["before"]["heap_path"].as_str().unwrap(),
        heap_path(&before_file)
    );
    assert_eq!(
        resolve_output["after"]["heap_path"].as_str().unwrap(),
        heap_path(&after_file)
    );

    // Still exactly the two pre-seeded snapshots -- resolve_snapshots must
    // not have written any new ones.
    let manifests = snapshot_store.list().unwrap();
    assert_eq!(manifests.len(), 2);
}

#[tokio::test]
async fn diff_step_output_matches_direct_run_diff_call() {
    // Proves §3.3's "zero new analysis logic" claim for the `diff` step:
    // the object-diff report it returns must be exactly what calling
    // `run_diff` directly, with the equivalent default `DiffRequest`, would
    // return for the same two heap paths.
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let state = run_full_workflow(
        &workflow_store,
        json!({
            "before_heap_path": heap_path(&before_file),
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await;

    let workflow_diff = state.step_history[1].output_summary.clone();

    let expected = match run_diff(default_object_diff_request(
        heap_path(&before_file),
        heap_path(&after_file),
    ))
    .await
    .expect("direct object diff should succeed")
    {
        DiffResult::Object(diff) => serde_json::to_value(diff).unwrap(),
        DiffResult::Class(_) => panic!("expected object diff"),
    };

    assert_eq!(workflow_diff, expected);
    assert!(workflow_diff.get("object_diff").is_some());
}

#[tokio::test]
async fn resolve_snapshots_rejects_both_heap_path_and_snapshot_key_for_same_side() {
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let err = start(
        &workflow_store,
        WorkflowKind::CompareSnapshots,
        String::new(),
        json!({
            "before_heap_path": heap_path(&before_file),
            "before_snapshot_key": "some-key",
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn resolve_snapshots_rejects_missing_before_identity_not_panic() {
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let err = start(
        &workflow_store,
        WorkflowKind::CompareSnapshots,
        String::new(),
        json!({
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_after_complete_returns_workflow_already_complete() {
    let before_file = write_fixture_heap(build_graph_fixture());
    let after_file = write_fixture_heap(build_tune_gc_fixture());
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());
    let snapshot_dir = tempfile::tempdir().unwrap();

    let state = run_full_workflow(
        &workflow_store,
        json!({
            "before_heap_path": heap_path(&before_file),
            "after_heap_path": heap_path(&after_file),
            "snapshot_dir": snapshot_dir.path().to_str().unwrap(),
        }),
    )
    .await;
    assert_eq!(state.current_step, "complete");

    let err = advance(&workflow_store, &state.workflow_id, json!(null))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_already_complete"));
}

#[tokio::test]
async fn advance_with_unknown_workflow_id_returns_workflow_not_found() {
    let workflow_store_dir = tempfile::tempdir().unwrap();
    let workflow_store = WorkflowStore::new(workflow_store_dir.path().to_path_buf());

    let err = advance(&workflow_store, "does-not-exist", json!(null))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_not_found"));
}
