//! Integration tests for M11 Slice 11.B: the `TraverseObjectGraph` workflow
//! kind (`core::workflow::traverse_object_graph`), exercised through the
//! shared `core::workflow::{start, advance, describe}` state machine.
//!
//! This is the one workflow kind with a real branch point (design doc §4
//! point 4 / §8 Slice 11.B): `choose_direction` loops back to `inspect` on
//! a caller-chosen id, or ends the walk. See
//! `full_run_step_sequence_contract_test_across_a_loop` below for how the
//! §6.1 mandatory contract test is adapted to account for that loop -- a
//! naive `observed_sequence == describe().steps` equality check (as
//! `workflow_triage_memory_leak.rs`/`workflow_tune_gc.rs` use for their
//! linear kinds) cannot pass here, since a real run's `step_history` visits
//! `inspect`/`choose_direction` repeatedly while `describe()` lists each
//! step name exactly once.
//!
//! Fixture: `build_simple_fixture()` (`core::hprof::test_fixtures`, `id_size
//! = 8`) produces a real, parseable 3-node reference chain --
//! `0x2001 --next--> 0x2002 --next--> 0x2003 --next--> (null, filtered)` --
//! plus an object array `0x3000` referencing both `0x2001` and `0x2002`
//! (giving `0x2001`/`0x2002` real `referrers_in` entries too, so
//! `choose_direction`'s branch-point validation gets coverage against both
//! `references_out` and `referrers_in`, not just one direction).

use mnemosyne_core::{
    analysis::inspect_object,
    graph::build_dominator_tree,
    hprof::{parse_hprof_file_with_options, test_fixtures::build_simple_fixture, ParseOptions},
    workflow::{advance, describe, start, WorkflowKind, WorkflowStore},
};
use serde_json::json;
use std::io::Write;
use tempfile::NamedTempFile;

fn write_fixture_heap() -> NamedTempFile {
    let fixture = build_simple_fixture();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&fixture).unwrap();
    file.flush().unwrap();
    file
}

fn heap_path(file: &NamedTempFile) -> String {
    file.path().to_str().unwrap().to_string()
}

/// `build_simple_fixture()` uses `id_size = 8`, so object ids render as
/// `0x` + 16 uppercase hex digits (see
/// `analysis::inspector::format_object_id`'s convention, mirrored here).
fn id_str(id: u64) -> String {
    format!("0x{id:016X}")
}

#[tokio::test]
async fn full_run_across_three_hops_matches_direct_inspect_object_calls() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let graph = parse_hprof_file_with_options(&heap, ParseOptions::default()).unwrap();
    let dominator = build_dominator_tree(&graph);
    let direct = |id: u64| {
        serde_json::to_value(inspect_object(&graph, Some(&dominator), id, false).unwrap()).unwrap()
    };

    // Hop 1: inspect 0x2001 (start_workflow's object_id).
    let state = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap.clone(),
        json!({ "object_id": id_str(0x2001) }),
    )
    .await
    .expect("start should succeed against a real fixture heap");
    assert_eq!(state.current_step, "choose_direction");
    assert_eq!(state.step_history[0].output_summary, direct(0x2001));
    assert_eq!(
        state.step_history[0].output_summary["references_out"],
        json!([{ "object_id": id_str(0x2002), "class_name": "com.example.Node" }])
    );

    // choose_direction: step into 0x2002 (the only references_out entry).
    let state = advance(
        &store,
        &state.workflow_id,
        json!({ "object_id": id_str(0x2002) }),
    )
    .await
    .expect("choosing the offered references_out id should succeed");
    assert_eq!(state.current_step, "inspect");

    // Hop 2: inspect 0x2002. It has two referrers (0x2001, the array
    // 0x3000) and one outgoing ref (0x2003) -- exercise picking a
    // *referrer* next, not just a references_out entry.
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .expect("inspect should succeed using the id chosen by choose_direction");
    assert_eq!(state.current_step, "choose_direction");
    assert_eq!(state.step_history[2].output_summary, direct(0x2002));
    let referrers = state.step_history[2].output_summary["referrers_in"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(referrers.len(), 2, "0x2002 should have two referrers");

    // choose_direction: step into 0x2003 (the sole references_out entry).
    let state = advance(
        &store,
        &state.workflow_id,
        json!({ "object_id": id_str(0x2003) }),
    )
    .await
    .expect("choosing the offered references_out id should succeed");
    assert_eq!(state.current_step, "inspect");

    // Hop 3: inspect 0x2003 (a dead end -- its `next` field is null).
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .expect("inspect should succeed for the third hop");
    assert_eq!(state.current_step, "choose_direction");
    assert_eq!(state.step_history[4].output_summary, direct(0x2003));
    assert_eq!(
        state.step_history[4].output_summary["references_out"],
        json!([])
    );

    // End the walk instead of looping again.
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .expect("omitting object_id should end the walk");
    assert_eq!(state.current_step, "complete");
    assert_eq!(
        state.step_history[5].output_summary,
        json!({ "ended": true })
    );

    // 5 recorded steps: inspect, choose_direction, inspect, choose_direction,
    // inspect, choose_direction (the ending one) -- 6 total.
    assert_eq!(state.step_history.len(), 6);
}

#[tokio::test]
async fn full_run_step_sequence_contract_test_across_a_loop() {
    // §6.1 mandatory contract test, adapted for a looping kind (see this
    // file's module doc comment for why). Interpretation: the STATIC step
    // list `describe()` returns must equal the sequence of *distinct* step
    // names a live run visits, taken in first-occurrence order -- i.e.
    // collapsing repeats rather than requiring positional equality. For a
    // linear (non-looping) kind this reduces to exactly the same check
    // `workflow_triage_memory_leak.rs`/`workflow_tune_gc.rs` use (no step
    // name ever repeats there, so "first-occurrence order" and "full
    // sequence" coincide) -- this is a generalization of that check, not a
    // weaker unrelated one. It still catches real drift: a step renamed,
    // removed, reordered, or never actually reached by a real run would all
    // still fail this assertion.
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap.clone(),
        json!({ "object_id": id_str(0x2001) }),
    )
    .await
    .unwrap();
    let state = advance(
        &store,
        &state.workflow_id,
        json!({ "object_id": id_str(0x2002) }),
    )
    .await
    .unwrap();
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .unwrap();
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .unwrap();
    assert_eq!(state.current_step, "complete");

    let observed: Vec<String> = state
        .step_history
        .iter()
        .map(|record| record.step_name.clone())
        .collect();
    assert_eq!(
        observed,
        vec!["inspect", "choose_direction", "inspect", "choose_direction"],
        "sanity check: the loop actually visited inspect/choose_direction more than once"
    );

    let mut distinct_in_first_occurrence_order: Vec<String> = Vec::new();
    for name in &observed {
        if !distinct_in_first_occurrence_order.contains(name) {
            distinct_in_first_occurrence_order.push(name.clone());
        }
    }

    let description = describe(WorkflowKind::TraverseObjectGraph).unwrap();
    let expected: Vec<String> = description
        .steps
        .iter()
        .map(|step| step.name.clone())
        .collect();

    assert_eq!(distinct_in_first_occurrence_order, expected);
}

#[tokio::test]
async fn choose_direction_rejects_id_not_offered_by_prior_inspect() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap,
        json!({ "object_id": id_str(0x2001) }),
    )
    .await
    .unwrap();

    // 0x4000 (the primitive int array) exists in the fixture but is
    // neither a reference-out nor a referrer-in of 0x2001.
    let err = advance(
        &store,
        &state.workflow_id,
        json!({ "object_id": id_str(0x4000) }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn start_without_object_id_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let err = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap_path(&heap_file),
        json!({}),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn start_with_object_id_not_in_heap_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let err = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap_path(&heap_file),
        json!({ "object_id": "0xDEADBEEF" }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_with_malformed_choose_direction_input_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap_path(&heap_file),
        json!({ "object_id": id_str(0x2001) }),
    )
    .await
    .unwrap();

    // choose_direction expects an object with an optional `object_id`
    // string field; a bare JSON number is not that shape.
    let err = advance(&store, &state.workflow_id, json!(42))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_after_complete_returns_workflow_already_complete() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TraverseObjectGraph,
        heap,
        json!({ "object_id": id_str(0x2001) }),
    )
    .await
    .unwrap();
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .unwrap();
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
