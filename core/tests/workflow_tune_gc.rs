//! Integration tests for M11 Slice 11.B: the `TuneGc` workflow kind
//! (`core::workflow::tune_gc`), exercised through the shared
//! `core::workflow::{start, advance, describe}` state machine.
//!
//! Mirrors `core/tests/workflow_triage_memory_leak.rs`'s structure (design
//! doc §10 test strategy): a full run to completion, the §6.1 mandatory
//! contract test, "matches calling the underlying primitive directly"
//! equivalence checks, and negative-path coverage.

use std::collections::BTreeMap;
use std::io::Write;

use mnemosyne_core::{
    analysis::inspect_threads,
    graph::build_dominator_tree,
    hprof::{
        parse_hprof_file_with_options, test_fixtures::build_tune_gc_fixture, GcRootKind,
        GcRootType, ParseOptions,
    },
    workflow::{advance, describe, start, WorkflowKind, WorkflowStore},
};
use serde_json::{json, Value};
use tempfile::NamedTempFile;

/// Independent re-implementation of `core::graph::gc_root_path::gc_root_kind`
/// (the classification the workflow's own `root_kind_breakdown` step reuses
/// internally, `pub(crate)`-only). Deliberately a *separate* copy here
/// rather than reaching into the crate-internal helper: this test's whole
/// point is to be an independently-computed reference, not merely a
/// wrapper around the same code path the workflow calls (design doc §10 /
/// Slice 11.B validation gate: "matches an independently-computed
/// reference grouping").
fn classify_root_kind(root_type: &GcRootType) -> GcRootKind {
    match root_type {
        GcRootType::JniGlobal => GcRootKind::JniGlobal,
        GcRootType::JniLocal { .. } => GcRootKind::JniLocal,
        GcRootType::JavaFrame { .. } => GcRootKind::JavaFrame,
        GcRootType::NativeStack { .. } => GcRootKind::NativeStack,
        GcRootType::StickyClass => GcRootKind::StickyClass,
        GcRootType::ThreadBlock { .. } => GcRootKind::ThreadBlock,
        GcRootType::MonitorUsed => GcRootKind::MonitorUsed,
        GcRootType::ThreadObject { .. } => GcRootKind::ThreadObject,
        GcRootType::Unknown(_) => GcRootKind::Unknown,
    }
}

fn write_fixture_heap() -> NamedTempFile {
    let fixture = build_tune_gc_fixture();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&fixture).unwrap();
    file.flush().unwrap();
    file
}

fn heap_path(file: &NamedTempFile) -> String {
    file.path().to_str().unwrap().to_string()
}

/// Independently recompute the root-kind breakdown directly from the
/// primitives `root_kind_breakdown` composes (`ObjectGraph::gc_roots` +
/// `gc_root_kind` + `DominatorTree::retained_size`), *without* going
/// through the workflow at all -- this is the "reference grouping" the
/// design doc's Slice 11.B validation gate calls for.
fn independent_root_kind_breakdown(heap: &str) -> BTreeMap<GcRootKind, (usize, u64)> {
    let graph = parse_hprof_file_with_options(heap, ParseOptions::default()).unwrap();
    let dominator = build_dominator_tree(&graph);

    let mut totals: BTreeMap<GcRootKind, (usize, u64)> = BTreeMap::new();
    for root in &graph.gc_roots {
        let kind = classify_root_kind(&root.root_type);
        let entry = totals.entry(kind).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += dominator.retained_size(root.object_id);
    }
    totals
}

async fn run_full_workflow(
    store: &WorkflowStore,
    heap: &str,
) -> mnemosyne_core::workflow::WorkflowState {
    let state = start(store, WorkflowKind::TuneGc, heap.to_string(), json!({}))
        .await
        .expect("start should succeed against a real fixture heap with 2+ GC-root kinds");
    assert_eq!(state.current_step, "thread_local_review");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("thread_local_review should succeed");
    assert_eq!(state.current_step, "top_retainers");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("top_retainers should succeed");
    assert_eq!(state.current_step, "complete");

    state
}

#[tokio::test]
async fn full_run_step_sequence_matches_static_description_exactly() {
    // §6.1 mandatory contract test: `TuneGc` is linear (no branch point --
    // that's `TraverseObjectGraph`'s distinction), so this is the same
    // direct sequence-equality check `workflow_triage_memory_leak.rs` uses.
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = run_full_workflow(&store, &heap_path(&heap_file)).await;

    let observed: Vec<String> = state
        .step_history
        .iter()
        .map(|record| record.step_name.clone())
        .collect();
    let description = describe(WorkflowKind::TuneGc).unwrap();
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
async fn root_kind_breakdown_matches_independently_computed_reference_grouping() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(&store, WorkflowKind::TuneGc, heap.clone(), json!({}))
        .await
        .unwrap();

    let output = &state.step_history[0].output_summary;
    let root_kinds = output["root_kinds"].as_array().unwrap();

    let reference = independent_root_kind_breakdown(&heap);
    // The fixture is built with exactly 2 distinct GC-root kinds
    // (ThreadObject, StickyClass) -- assert that up front so this test
    // actually exercises multi-kind grouping, not a degenerate single-group
    // case.
    assert_eq!(
        reference.len(),
        2,
        "fixture should have exactly 2 distinct GC-root kinds"
    );
    assert_eq!(root_kinds.len(), reference.len());

    for entry in root_kinds {
        let kind_name = entry["kind"].as_str().unwrap();
        let matching_reference_kind = reference
            .keys()
            .find(|kind| format!("{kind:?}") == kind_name)
            .unwrap_or_else(|| panic!("workflow reported unexpected root kind '{kind_name}'"));
        let (expected_count, expected_bytes) = reference[matching_reference_kind];

        assert_eq!(
            entry["root_count"].as_u64().unwrap() as usize,
            expected_count
        );
        assert_eq!(entry["retained_bytes"].as_u64().unwrap(), expected_bytes);
    }

    let expected_total_roots: usize = reference.values().map(|(count, _)| count).sum();
    assert_eq!(
        output["total_roots"].as_u64().unwrap() as usize,
        expected_total_roots
    );
}

#[tokio::test]
async fn thread_local_review_output_matches_direct_inspect_threads_call() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(&store, WorkflowKind::TuneGc, heap.clone(), json!({}))
        .await
        .unwrap();
    let state = advance(&store, &state.workflow_id, json!(null))
        .await
        .unwrap();

    let workflow_output: Value = state.step_history[1].output_summary.clone();

    let graph = parse_hprof_file_with_options(&heap, ParseOptions::default()).unwrap();
    let dominator = build_dominator_tree(&graph);
    let direct_report = inspect_threads(&graph, Some(&dominator), 10);
    let direct_output = serde_json::to_value(&direct_report).unwrap();

    assert_eq!(workflow_output, direct_output);
    assert_eq!(
        direct_report.total_thread_count, 1,
        "fixture should contain exactly one Thread instance"
    );
}

#[tokio::test]
async fn top_retainers_output_matches_direct_dominator_top_retained_call() {
    let heap_file = write_fixture_heap();
    let heap = heap_path(&heap_file);
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = run_full_workflow(&store, &heap).await;
    let workflow_top_retainers = state.step_history[2].output_summary["top_retainers"]
        .as_array()
        .unwrap()
        .clone();

    let graph = parse_hprof_file_with_options(&heap, ParseOptions::default()).unwrap();
    let dominator = build_dominator_tree(&graph);
    let direct_top = dominator.top_retained(10);

    assert_eq!(workflow_top_retainers.len(), direct_top.len());
    for (entry, (object_id, retained_bytes)) in workflow_top_retainers.iter().zip(direct_top.iter())
    {
        let width = graph.identifier_size as usize * 2;
        assert_eq!(
            entry["object_id"].as_str().unwrap(),
            format!("0x{object_id:0width$X}")
        );
        assert_eq!(entry["retained_bytes"].as_u64().unwrap(), *retained_bytes);
    }
}

#[tokio::test]
async fn root_kind_breakdown_step_rejects_non_empty_input() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let err = start(
        &store,
        WorkflowKind::TuneGc,
        heap_path(&heap_file),
        json!({ "unexpected": true }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_with_malformed_top_n_is_step_input_mismatch_not_panic() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = start(
        &store,
        WorkflowKind::TuneGc,
        heap_path(&heap_file),
        json!({}),
    )
    .await
    .unwrap();

    let err = advance(
        &store,
        &state.workflow_id,
        json!({ "top_n": "not-a-number" }),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("workflow_step_input_mismatch"));
}

#[tokio::test]
async fn advance_after_complete_returns_workflow_already_complete() {
    let heap_file = write_fixture_heap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(store_dir.path().to_path_buf());

    let state = run_full_workflow(&store, &heap_path(&heap_file)).await;
    assert_eq!(state.current_step, "complete");

    let err = advance(&store, &state.workflow_id, json!({}))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("workflow_already_complete"));
}

#[tokio::test]
async fn describe_tune_gc_states_no_live_jvm_constraint_explicitly() {
    // Design doc §2 point 2 / §9 R2: the honesty constraint ("Mnemosyne
    // never touches a live JVM or applies a GC flag") must be present in
    // describe_workflow's own returned text, not just design docs/comments.
    let description = describe(WorkflowKind::TuneGc).unwrap();
    let combined_text: String = description
        .steps
        .iter()
        .map(|step| step.description.clone())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(combined_text.contains("never touches a live JVM"));
    assert!(combined_text.to_lowercase().contains("gc flag"));
}
