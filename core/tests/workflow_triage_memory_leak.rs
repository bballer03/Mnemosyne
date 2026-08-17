//! Integration tests for M11 Slice 11.A: the `core::workflow` state machine
//! scaffolding plus the `TriageMemoryLeak` workflow kind
//! (`core::workflow::{start, advance, describe, WorkflowStore}` /
//! `core::workflow::triage_memory_leak`).
//!
//! Per the design doc's §3.3 ("workflows are orchestration, not new
//! analysis") and §6.1 ("contract-test discipline", roadmap-mandated), this
//! file exercises three things end to end against a real, on-disk HPROF
//! fixture:
//!
//! 1. A full `start` → `advance` × N → `complete` run reaches completion and
//!    its observed step sequence matches `describe()`'s static schema
//!    exactly (the mandatory contract test).
//! 2. Each step's data is *equivalent* to calling the same underlying
//!    primitive directly (`detect_leaks`, `find_all_gc_paths`,
//!    `analyze_by_referrer`) -- proving the workflow introduces no new
//!    analysis logic, only orchestration.
//! 3. Negative paths never panic: malformed step input, an out-of-band
//!    `leak_id`, advancing a completed workflow, and starting an
//!    unimplemented workflow kind all return clear, structured errors.

use mnemosyne_core::{
    analysis::{
        analyze_by_referrer, detect_leaks, LeakDetectionOptions, LeakInsight, LeakSeverity,
    },
    graph::{build_dominator_tree, find_all_gc_paths, AllPathsRequest},
    hprof::{parse_hprof_file_with_options, ParseOptions},
    test_fixtures::build_graph_fixture,
    workflow::{advance, describe, start, WorkflowKind, WorkflowStore},
};
use serde_json::json;
use std::io::Write;
use tempfile::NamedTempFile;

/// `build_graph_fixture()` (shared HPROF-backed test fixture, also used by
/// `core/tests/analyze_by_referrer.rs` and `core::analysis::engine`'s own
/// leak-detection tests) is a GC-root-reachable `com/example/BigCache`
/// instance with one outgoing field reference to a plain `java/lang/Object`
/// instance -- small, but a real parsed graph with a real GC root, so
/// `detect_leaks`/`find_all_gc_paths`/`analyze_by_referrer` all have real
/// (not synthetic-fallback) data to work with.
fn write_fixture_heap() -> NamedTempFile {
    let fixture = build_graph_fixture();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&fixture).unwrap();
    file.flush().unwrap();
    file
}

fn heap_path(file: &NamedTempFile) -> String {
    file.path().to_str().unwrap().to_string()
}

/// Run a full `TriageMemoryLeak` workflow to completion, skipping the
/// optional `propose_fix` step. Returns the final state plus the `leak_id`
/// that was chosen at the `investigate_suspect` step.
async fn run_full_workflow(
    store: &WorkflowStore,
    heap: &str,
) -> (mnemosyne_core::workflow::WorkflowState, String) {
    let state = start(
        store,
        WorkflowKind::TriageMemoryLeak,
        heap.to_string(),
        json!({}),
    )
    .await
    .expect("start should succeed against a real fixture heap");
    assert_eq!(state.current_step, "investigate_suspect");

    let leak_id = state.step_history[0]
        .output_summary
        .get("top_leak_ids")
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .expect("detect step should surface at least one leak id")
        .to_string();

    let state = advance(store, &state.workflow_id, json!({ "leak_id": leak_id }))
        .await
        .expect("investigate_suspect should succeed for a leak id the detect step returned");
    assert_eq!(state.current_step, "explain");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("explain should succeed");
    assert_eq!(state.current_step, "propose_fix");

    let state = advance(store, &state.workflow_id, json!({ "skip": true }))
        .await
        .expect("propose_fix should succeed even when skipped");
    assert_eq!(state.current_step, "complete");

    (state, leak_id)
}

#[tokio::test]
async fn full_run_step_sequence_matches_static_description_exactly() {
    // The mandatory contract test named in design doc §6.1 / roadmap.md's
    // own M11 risk register entry (R1): the sequence of steps a live run
    // actually executes must exactly match `describe()`'s static schema, in
    // order. Each `StepRecord::step_name` in `step_history` records the
    // value `current_step` held while that step was executing (see
    // `core::workflow::triage_memory_leak::run_step`: it reads
    // `state.current_step` *before* advancing it, then pushes the record
    // with that same name) -- so `step_history`'s recorded names ARE the
    // sequence of `current_step` values observed while running the
    // workflow, one per call, in call order.
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let (state, _leak_id) = run_full_workflow(&store, &heap_path(&heap_file)).await;

    let observed: Vec<String> = state
        .step_history
        .iter()
        .map(|record| record.step_name.clone())
        .collect();

    let description = describe(WorkflowKind::TriageMemoryLeak).unwrap();
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
}

#[tokio::test]
async fn detect_step_output_matches_direct_detect_leaks_call() {
    // Proves §3.3's "zero new analysis logic" claim for the `detect` step:
    // the workflow's leak list must be byte-for-byte what calling
    // `detect_leaks` directly, with the same options, would return.
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TriageMemoryLeak,
        heap.clone(),
        json!({}),
    )
    .await
    .unwrap();

    let workflow_leaks: Vec<LeakInsight> =
        serde_json::from_value(state.step_history[0].output_summary["leaks"].clone()).unwrap();

    let direct_leaks = detect_leaks(&heap, LeakDetectionOptions::new(LeakSeverity::Low))
        .await
        .unwrap();

    assert!(!direct_leaks.is_empty());
    assert_eq!(workflow_leaks.len(), direct_leaks.len());
    for (workflow_leak, direct_leak) in workflow_leaks.iter().zip(direct_leaks.iter()) {
        assert_eq!(workflow_leak.id, direct_leak.id);
        assert_eq!(workflow_leak.class_name, direct_leak.class_name);
        assert_eq!(
            workflow_leak.retained_size_bytes,
            direct_leak.retained_size_bytes
        );
    }
}

#[tokio::test]
async fn investigate_suspect_output_matches_direct_gc_path_and_referrer_calls() {
    // Proves §3.3's "zero new analysis logic" claim for the
    // `investigate_suspect` step: the GC path and referrer profile it
    // returns must match what calling `find_all_gc_paths` /
    // `analyze_by_referrer` directly, with equivalent parameters, returns.
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TriageMemoryLeak,
        heap.clone(),
        json!({}),
    )
    .await
    .unwrap();
    let leaks: Vec<LeakInsight> =
        serde_json::from_value(state.step_history[0].output_summary["leaks"].clone()).unwrap();
    let chosen = leaks
        .iter()
        .find(|leak| leak.class_name.contains("BigCache"))
        .expect("fixture's BigCache instance should be a detected leak");

    let state = advance(&store, &state.workflow_id, json!({ "leak_id": chosen.id }))
        .await
        .unwrap();

    let step_output = &state.step_history[1].output_summary;

    let dotted_class_name = chosen.class_name.replace('/', ".");
    let direct_gc_path = find_all_gc_paths(&AllPathsRequest {
        heap_path: heap.clone(),
        object_id: None,
        by_class: Some(dotted_class_name),
        max_paths: AllPathsRequest::DEFAULT_MAX_PATHS,
        max_depth: None,
    })
    .unwrap();

    assert_eq!(
        step_output["gc_path_length"].as_u64().unwrap() as usize,
        direct_gc_path.path_length
    );
    assert_eq!(
        step_output["gc_path_truncated"].as_bool().unwrap(),
        direct_gc_path.truncated
    );

    let graph = parse_hprof_file_with_options(&heap, ParseOptions::default()).unwrap();
    let dominator = build_dominator_tree(&graph);
    let direct_referrer_report =
        analyze_by_referrer(&graph, Some(&dominator), graph.objects.len().max(1));
    let direct_matching_entries: Vec<_> = direct_referrer_report
        .entries
        .into_iter()
        .filter(|entry| entry.class_name == chosen.class_name)
        .collect();

    assert_eq!(
        step_output["referrer_entry_count"].as_u64().unwrap() as usize,
        direct_matching_entries.len()
    );
    assert!(
        !direct_matching_entries.is_empty(),
        "the chosen suspect's class should appear in its own referrer ranking"
    );
}

#[tokio::test]
async fn propose_fix_step_can_be_skipped() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let (state, _leak_id) = run_full_workflow(&store, &heap_path(&heap_file)).await;

    let fix_record = state.step_history.last().unwrap();
    assert_eq!(fix_record.step_name, "propose_fix");
    assert_eq!(fix_record.output_summary, json!({ "skipped": true }));
    assert_eq!(state.context["fix"], json!({ "skipped": true }));
}

#[tokio::test]
async fn advance_with_leak_id_not_returned_by_detect_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TriageMemoryLeak,
        heap_path(&heap_file),
        json!({}),
    )
    .await
    .unwrap();

    let err = advance(
        &store,
        &state.workflow_id,
        json!({ "leak_id": "does-not-exist::00000000" }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_with_malformed_step_input_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TriageMemoryLeak,
        heap_path(&heap_file),
        json!({}),
    )
    .await
    .unwrap();

    // `investigate_suspect` requires an object with a `leak_id` string;
    // a bare JSON string is not that shape.
    let err = advance(&store, &state.workflow_id, json!("not-an-object"))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_after_complete_returns_workflow_already_complete() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let (state, _leak_id) = run_full_workflow(&store, &heap_path(&heap_file)).await;
    assert_eq!(state.current_step, "complete");

    let err = advance(&store, &state.workflow_id, json!({}))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_already_complete"));
}

#[tokio::test]
async fn advance_with_unknown_workflow_id_returns_workflow_not_found() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let err = advance(&store, "does-not-exist", json!({}))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_not_found"));
}

#[tokio::test]
async fn start_with_unimplemented_kind_returns_clear_error_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    for kind in [
        WorkflowKind::TuneGc,
        WorkflowKind::TraverseObjectGraph,
        WorkflowKind::CompareSnapshots,
    ] {
        let err = start(&store, kind, heap_path(&heap_file), json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("workflow_kind_not_implemented"));
    }
}
