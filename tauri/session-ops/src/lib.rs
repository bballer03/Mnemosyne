//! Session-scoped heap operations shared by Tauri commands.
//!
//! Kept free of `tauri` dependencies so unit tests can run on headless CI
//! hosts without WebKit/GTK installed.

use std::path::{Path, PathBuf};

use mnemosyne_core::{
    analysis::{inspect_object, ObjectInspection},
    build_dominator_tree, build_histogram,
    diff::{
        object::types::{
            DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES, DEFAULT_OBJECT_DIFF_TOP_N,
            DEFAULT_RETAINED_CHANGE_THRESHOLD,
        },
        run_diff, DiffMode, DiffRequest, DiffResult, IdentityStrategy, ObjectDiffReport,
    },
    graph::find_all_gc_paths_in_graph,
    snapshot::{SnapshotManifest, SnapshotStore},
    workflow::{self, WorkflowDescription, WorkflowKind, WorkflowState, WorkflowStore},
    AllPathsRequest, GcPathResult, HistogramGroupBy, HistogramResult,
};
use serde_json::{json, Value};

/// Input for the M17 comparison bridge's `diffObjects` host method.
#[derive(Debug, Clone)]
pub struct DiffObjectsSessionInput {
    pub before_key: String,
    pub after_key: String,
    pub strategy: Option<IdentityStrategy>,
    pub top_n: Option<usize>,
    /// Leak-progression cross-reference (M10-B): default `false` unless the
    /// UI explicitly opts in.
    pub cross_reference_leaks: Option<bool>,
}

pub fn graph_has_field_data(graph: &mnemosyne_core::hprof::ObjectGraph) -> bool {
    graph
        .objects
        .values()
        .any(|object| !object.field_data.is_empty())
}

/// Session coordinates captured before an out-of-lock field-data reparse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDataCacheCapture {
    pub epoch: u64,
    pub heap_path: String,
}

/// Returns true when an in-flight field-data reparse may still be cached for
/// the session that requested it (epoch unchanged and heap path still loaded).
pub fn should_install_field_data_cache(
    capture_epoch: u64,
    current_epoch: u64,
    capture_heap_path: &str,
    current_heap_path: Option<&str>,
) -> bool {
    current_epoch == capture_epoch && current_heap_path == Some(capture_heap_path)
}

/// Install a field-data graph only when the live session still matches
/// `capture`. Callers must hold the same session-mutation lock used by
/// `load_heap` / `unload_heap` so epoch/path cannot change between the
/// re-verify and the cache write.
pub fn install_field_data_cache_if_still_current(
    capture: &FieldDataCacheCapture,
    current_epoch: u64,
    current_heap_path: Option<&str>,
    field_data_graph: &mut Option<mnemosyne_core::hprof::ObjectGraph>,
    parsed_graph: mnemosyne_core::hprof::ObjectGraph,
) -> bool {
    if should_install_field_data_cache(
        capture.epoch,
        current_epoch,
        &capture.heap_path,
        current_heap_path,
    ) {
        *field_data_graph = Some(parsed_graph);
        true
    } else {
        false
    }
}

pub fn inspect_object_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    heap_path: &str,
    object_id: &str,
    retain_field_data: bool,
) -> Result<ObjectInspection, String> {
    let target_id = parse_inspect_object_id(object_id)
        .filter(|id| graph.objects.contains_key(id))
        .ok_or_else(|| inspect_object_id_not_found(object_id, heap_path))?;

    let dominator = build_dominator_tree(graph);
    inspect_object(graph, Some(&dominator), target_id, retain_field_data)
        .ok_or_else(|| inspect_object_id_not_found(object_id, heap_path))
}

pub fn find_all_gc_paths_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    heap_path: &str,
    object_id: &str,
    max_paths: usize,
) -> Result<GcPathResult, String> {
    find_all_gc_paths_in_graph(
        graph,
        &AllPathsRequest {
            heap_path: heap_path.to_string(),
            object_id: Some(object_id.to_string()),
            by_class: None,
            max_paths,
            max_depth: None,
        },
    )
    .map_err(|error| error.to_string())
}

pub fn inspect_object_id_not_found(object_id: &str, heap_path: &str) -> String {
    format!(
        "inspect_object_id_not_found: object id '{object_id}' was not found in heap dump '{heap_path}'"
    )
}

/// Parse an inspect-supplied object id (`0x...` hex or bare decimal), mirroring
/// MCP/CLI semantics: malformed ids are treated as not-found by callers.
pub fn parse_inspect_object_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }

    if trimmed.chars().any(|character| matches!(character, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed, 16).ok();
    }

    trimmed.parse::<u64>().ok()
}

pub fn parse_object_id(input: &str) -> Result<u64, String> {
    parse_inspect_object_id(input).ok_or_else(|| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            "Object id must not be empty".to_string()
        } else {
            format!("Invalid object id '{trimmed}'")
        }
    })
}

/// Parse the UI's PascalCase identity strategy labels into core enums.
pub fn parse_identity_strategy(raw: &str) -> Result<IdentityStrategy, String> {
    match raw {
        "ClassRetained" => Ok(IdentityStrategy::ClassRetained),
        "ClassDominator" => Ok(IdentityStrategy::ClassDominator),
        "FullFingerprint" => Ok(IdentityStrategy::FullFingerprint),
        other => Err(format!("Invalid identity strategy '{other}'")),
    }
}

/// Parse MCP/CLI-style `histogram_group_by` strings (`class_loader` preferred;
/// `classloader` accepted as a CLI alias).
pub fn parse_histogram_group_by(raw: &str) -> Result<HistogramGroupBy, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "class" => Ok(HistogramGroupBy::Class),
        "package" => Ok(HistogramGroupBy::Package),
        "class_loader" | "classloader" => Ok(HistogramGroupBy::ClassLoader),
        "superclass" => Ok(HistogramGroupBy::Superclass),
        other => Err(format!(
            "Invalid histogram group_by '{other}'. Expected class, package, class_loader, or superclass."
        )),
    }
}

/// Rebuild a flat grouped histogram for the loaded session graph.
///
/// Uses the same `build_histogram` path as MCP `analyze_heap.histogram_group_by`
/// without re-running the full analyze pipeline.
pub fn regroup_histogram_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    group_by: &str,
) -> Result<HistogramResult, String> {
    let group_by = parse_histogram_group_by(group_by)?;
    let dominator = build_dominator_tree(graph);
    Ok(build_histogram(graph, &dominator, group_by))
}

/// Resolve a snapshot key (SHA-256 hash) or direct heap file path to the
/// on-disk heap path `run_diff` expects.
pub fn resolve_heap_path_from_key(store: &SnapshotStore, key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("snapshot key must not be empty".to_string());
    }

    if let Ok(payload) = store.load(trimmed) {
        return Ok(payload.manifest.heap_path);
    }

    if Path::new(trimmed).is_file() {
        return Ok(trimmed.to_string());
    }

    Err(format!(
        "snapshot_not_found: no snapshot or heap file found for key '{trimmed}'"
    ))
}

/// Run an object-level heap diff between two snapshot keys, reusing the
/// existing `DiffMode::Object` path (same defaults as CLI/MCP/workflow).
pub async fn diff_objects_for_session(
    store: &SnapshotStore,
    input: DiffObjectsSessionInput,
) -> Result<ObjectDiffReport, String> {
    let before_path = resolve_heap_path_from_key(store, &input.before_key)?;
    let after_path = resolve_heap_path_from_key(store, &input.after_key)?;

    let identity_strategy = input.strategy.unwrap_or_default();
    let top_n = input.top_n.unwrap_or(DEFAULT_OBJECT_DIFF_TOP_N);
    let cross_reference_leaks = input.cross_reference_leaks.unwrap_or(false);
    let retain_field_data = identity_strategy == IdentityStrategy::FullFingerprint;

    let request = DiffRequest {
        before_path,
        after_path,
        mode: DiffMode::Object,
        identity_strategy,
        retained_bucket_bits: 10,
        min_retained_bytes: DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
        retained_change_threshold: DEFAULT_RETAINED_CHANGE_THRESHOLD,
        top_n,
        retain_field_data,
        cross_reference_leaks,
    };

    match run_diff(request).await {
        Ok(DiffResult::Object(diff)) => diff
            .object_diff
            .ok_or_else(|| "object diff produced no report".to_string()),
        Ok(DiffResult::Class(_)) => Err("internal error: object diff returned class result".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

const SNAPSHOT_DIR_ENV: &str = "MNEMOSYNE_SNAPSHOT_DIR";
const WORKFLOW_DIR_ENV: &str = "MNEMOSYNE_WORKFLOW_DIR";

/// Default snapshot cache root — mirrors `core::mcp::server::default_snapshot_dir`.
pub fn default_snapshot_store_root() -> PathBuf {
    if let Ok(dir) = std::env::var(SNAPSHOT_DIR_ENV) {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(mut dir) = dirs::cache_dir() {
        dir.push("mnemosyne");
        return dir;
    }

    let mut fallback = std::env::temp_dir();
    fallback.push("mnemosyne");
    fallback.push("snapshots");
    fallback
}

/// Default workflow store root — mirrors `core::mcp::server::default_workflow_dir`.
pub fn default_workflow_store_root() -> PathBuf {
    if let Ok(dir) = std::env::var(WORKFLOW_DIR_ENV) {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(mut dir) = dirs::cache_dir() {
        dir.push("mnemosyne");
        dir.push("workflows");
        return dir;
    }

    let mut fallback = std::env::temp_dir();
    fallback.push("mnemosyne");
    fallback.push("workflows");
    fallback
}

pub fn default_snapshot_store() -> SnapshotStore {
    SnapshotStore::new(default_snapshot_store_root())
}

pub fn default_workflow_store() -> WorkflowStore {
    WorkflowStore::new(default_workflow_store_root())
}

/// Input for the M17 workflow bridge's `startWorkflow` host method.
#[derive(Debug, Clone, Default)]
pub struct StartWorkflowSessionInput {
    pub kind: String,
    pub heap_path: Option<String>,
    pub object_id: Option<String>,
    pub before_heap_path: Option<String>,
    pub after_heap_path: Option<String>,
    pub before_snapshot_key: Option<String>,
    pub after_snapshot_key: Option<String>,
}

/// Parses the lowercase snake_case wire form of a workflow kind, matching
/// `core::mcp::server::parse_workflow_kind`.
pub fn parse_workflow_kind(kind: &str) -> Result<WorkflowKind, String> {
    match kind {
        "triage_memory_leak" => Ok(WorkflowKind::TriageMemoryLeak),
        "tune_gc" => Ok(WorkflowKind::TuneGc),
        "traverse_object_graph" => Ok(WorkflowKind::TraverseObjectGraph),
        "compare_snapshots" => Ok(WorkflowKind::CompareSnapshots),
        "classloader_leak" => Ok(WorkflowKind::ClassloaderLeak),
        other => Err(format!(
            "unknown workflow kind '{other}': expected one of triage_memory_leak, tune_gc, \
             traverse_object_graph, compare_snapshots, classloader_leak"
        )),
    }
}

/// Builds the `{ workflow_id, current_step, step_result, next_expected_input }`
/// envelope shared by MCP `start_workflow`/`next_step` and the desktop bridge.
pub fn workflow_step_response(state: &WorkflowState) -> Result<Value, String> {
    let step_result = state
        .step_history
        .last()
        .map(|record| record.output_summary.clone())
        .unwrap_or(Value::Null);

    let next_expected_input = if state.current_step == "complete" {
        json!([])
    } else {
        let description = workflow::describe(state.kind).map_err(|error| error.to_string())?;
        description
            .steps
            .iter()
            .find(|step| step.name == state.current_step)
            .map(|step| serde_json::to_value(&step.expected_input))
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| json!([]))
    };

    Ok(json!({
        "workflow_id": state.workflow_id,
        "current_step": state.current_step,
        "step_result": step_result,
        "next_expected_input": next_expected_input,
    }))
}

pub fn describe_workflow_for_session(kind: &str) -> Result<WorkflowDescription, String> {
    let kind = parse_workflow_kind(kind)?;
    workflow::describe(kind).map_err(|error| error.to_string())
}

pub async fn start_workflow_for_session(
    store: &WorkflowStore,
    input: StartWorkflowSessionInput,
) -> Result<Value, String> {
    let kind = parse_workflow_kind(&input.kind)?;

    let (heap_path, initial_params) = match kind {
        WorkflowKind::TriageMemoryLeak
        | WorkflowKind::TuneGc
        | WorkflowKind::TraverseObjectGraph
        | WorkflowKind::ClassloaderLeak => {
            let heap_path = input.heap_path.clone().ok_or_else(|| {
                format!(
                    "heap_path is required for start_workflow(kind: \"{}\")",
                    kind.as_str()
                )
            })?;
            let initial_params = if kind == WorkflowKind::TraverseObjectGraph {
                json!({ "object_id": input.object_id })
            } else {
                Value::Null
            };
            (heap_path, initial_params)
        }
        WorkflowKind::CompareSnapshots => (
            String::new(),
            json!({
                "before_heap_path": input.before_heap_path,
                "after_heap_path": input.after_heap_path,
                "before_snapshot_key": input.before_snapshot_key,
                "after_snapshot_key": input.after_snapshot_key,
            }),
        ),
    };

    let state = workflow::start(store, kind, heap_path, initial_params)
        .await
        .map_err(|error| error.to_string())?;
    workflow_step_response(&state)
}

pub async fn next_step_for_session(
    store: &WorkflowStore,
    workflow_id: &str,
    step_input: Value,
) -> Result<Value, String> {
    let state = workflow::advance(store, workflow_id, step_input)
        .await
        .map_err(|error| error.to_string())?;
    workflow_step_response(&state)
}

pub fn list_snapshots_for_session(store: &SnapshotStore) -> Result<Vec<SnapshotManifest>, String> {
    store.list().map_err(|error| error.to_string())
}

#[cfg(all(test, feature = "test-fixtures"))]
mod tests {
    use super::*;
    use mnemosyne_core::{
        analysis::inspect_object,
        build_dominator_tree,
        hprof::{parse_hprof_file_with_options, test_fixtures::build_graph_fixture, ParseOptions},
    };

    fn graph_fixture() -> mnemosyne_core::hprof::ObjectGraph {
        let bytes = build_graph_fixture();
        parse_hprof_file_with_options_from_bytes(&bytes, false).expect("fixture must parse")
    }

    fn parse_hprof_file_with_options_from_bytes(
        bytes: &[u8],
        retain_field_data: bool,
    ) -> Result<mnemosyne_core::hprof::ObjectGraph, String> {
        let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, bytes).map_err(|error| error.to_string())?;
        parse_hprof_file_with_options(
            file.path().to_str().expect("temp path must be valid UTF-8"),
            ParseOptions {
                retain_field_data,
            },
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn parse_histogram_group_by_accepts_mcp_and_cli_aliases() {
        assert_eq!(
            parse_histogram_group_by("superclass").unwrap(),
            HistogramGroupBy::Superclass
        );
        assert_eq!(
            parse_histogram_group_by("class_loader").unwrap(),
            HistogramGroupBy::ClassLoader
        );
        assert_eq!(
            parse_histogram_group_by("classloader").unwrap(),
            HistogramGroupBy::ClassLoader
        );
        let error = parse_histogram_group_by("not-a-mode").expect_err("invalid");
        assert!(error.contains("Invalid histogram group_by"));
    }

    #[test]
    fn regroup_histogram_for_session_returns_superclass_groups() {
        let graph = graph_fixture();
        let histogram = regroup_histogram_for_session(&graph, "superclass")
            .expect("superclass regroup must succeed");
        assert_eq!(histogram.group_by, HistogramGroupBy::Superclass);
        assert!(!histogram.entries.is_empty());
        assert!(histogram.total_instances > 0);
    }

    #[test]
    fn should_install_field_data_cache_requires_matching_epoch_and_heap_path() {
        assert!(should_install_field_data_cache(
            3,
            3,
            "/tmp/a.hprof",
            Some("/tmp/a.hprof"),
        ));
        assert!(!should_install_field_data_cache(
            3,
            4,
            "/tmp/a.hprof",
            Some("/tmp/a.hprof"),
        ));
        assert!(!should_install_field_data_cache(
            3,
            3,
            "/tmp/a.hprof",
            Some("/tmp/b.hprof"),
        ));
        assert!(!should_install_field_data_cache(3, 3, "/tmp/a.hprof", None));
    }

    #[test]
    fn install_field_data_cache_if_still_current_writes_only_on_match() {
        let capture = FieldDataCacheCapture {
            epoch: 3,
            heap_path: "/tmp/a.hprof".to_string(),
        };
        let graph = graph_fixture();
        let mut slot: Option<mnemosyne_core::hprof::ObjectGraph> = None;

        assert!(install_field_data_cache_if_still_current(
            &capture,
            3,
            Some("/tmp/a.hprof"),
            &mut slot,
            graph.clone(),
        ));
        assert!(slot.is_some());

        let mut stale_slot = Some(graph_fixture());
        assert!(!install_field_data_cache_if_still_current(
            &capture,
            4,
            Some("/tmp/a.hprof"),
            &mut stale_slot,
            graph.clone(),
        ));
        assert!(stale_slot.is_some(), "epoch mismatch must not overwrite cache");

        let mut path_slot = Some(graph_fixture());
        assert!(!install_field_data_cache_if_still_current(
            &capture,
            3,
            Some("/tmp/b.hprof"),
            &mut path_slot,
            graph,
        ));
        assert!(path_slot.is_some(), "heap path mismatch must not overwrite cache");
    }

    #[test]
    fn inspect_object_for_session_returns_structured_inspection() {
        let graph = graph_fixture();
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", false)
            .expect("known object must inspect");

        assert_eq!(inspection.object_id, "0x00001000");
        assert_eq!(inspection.class_name, "com.example.BigCache");
        assert!(inspection.retained_size.unwrap_or(0) > 0);
    }

    #[test]
    fn inspect_object_for_session_unknown_id_matches_core_error() {
        let graph = graph_fixture();
        let error = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0xdeadbeef", false)
            .expect_err("unknown object must fail");

        assert_eq!(
            error,
            "inspect_object_id_not_found: object id '0xdeadbeef' was not found in heap dump '/tmp/heap.hprof'"
        );
    }

    #[test]
    fn find_all_gc_paths_for_session_returns_all_paths_and_truncated_flag() {
        let graph = graph_fixture();
        let result = find_all_gc_paths_for_session(&graph, "/tmp/heap.hprof", "0x2000", 5)
            .expect("known object must enumerate paths");

        assert_eq!(result.object_id, "0x00002000");
        assert_eq!(result.path_length, result.path.len());
        assert!(result.all_paths.as_ref().is_some_and(|paths| !paths.is_empty()));
        assert!(!result.truncated);
    }

    #[test]
    fn find_all_gc_paths_for_session_unknown_id_matches_core_error() {
        let graph = graph_fixture();
        let error = find_all_gc_paths_for_session(&graph, "/tmp/heap.hprof", "0xdeadbeef", 5)
            .expect_err("unknown object must fail");

        assert!(
            error.contains("gc_path_object_id_not_found:"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn inspect_object_for_session_honors_retain_field_data_without_fields_when_unretained() {
        let graph = graph_fixture();
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", true)
            .expect("known object must inspect");

        assert!(inspection.fields.is_none());
        let dominator = build_dominator_tree(&graph);
        assert!(inspect_object(&graph, Some(&dominator), 0x1000, true)
            .expect("core inspect must succeed")
            .fields
            .is_none());
    }

    #[test]
    fn inspect_object_for_session_retain_field_data_populates_fields_when_graph_retained() {
        let bytes = build_graph_fixture();
        let graph =
            parse_hprof_file_with_options_from_bytes(&bytes, true).expect("fixture must parse");
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", true)
            .expect("known object must inspect");

        let fields = inspection.fields.expect("fields must be present");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "entries");
    }

    #[test]
    fn inspect_object_for_session_malformed_id_matches_core_not_found_error() {
        let graph = graph_fixture();
        let error = inspect_object_for_session(&graph, "/tmp/heap.hprof", "not-an-id", false)
            .expect_err("malformed object id must fail");

        assert_eq!(
            error,
            "inspect_object_id_not_found: object id 'not-an-id' was not found in heap dump '/tmp/heap.hprof'"
        );
    }

    mod diff_objects {
        use super::*;
        use mnemosyne_core::{
            diff::{run_diff, DiffMode, DiffRequest, DiffResult, ObjectDeltaKind},
            hprof::{
                parse_hprof_file_with_options,
                test_fixtures::{build_graph_fixture, build_tune_gc_fixture},
                ParseOptions,
            },
            snapshot::SnapshotStore,
        };
        use std::io::Write;

        fn write_fixture(bytes: &[u8]) -> Result<(tempfile::NamedTempFile, String), String> {
            let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
            file.write_all(bytes).map_err(|error| error.to_string())?;
            let path = file
                .path()
                .to_str()
                .expect("temp path must be valid UTF-8")
                .to_string();
            Ok((file, path))
        }

        fn save_fixture_snapshot(
            store: &SnapshotStore,
            bytes: &[u8],
        ) -> Result<(tempfile::NamedTempFile, String), String> {
            let (file, path) = write_fixture(bytes)?;
            let graph = parse_hprof_file_with_options(&path, ParseOptions::default())
                .map_err(|error| error.to_string())?;
            let dominator = build_dominator_tree(&graph);
            let manifest = store
                .save(&path, &graph, &dominator)
                .map_err(|error| error.to_string())?;
            Ok((file, manifest.heap_sha256))
        }

        fn diff_store() -> SnapshotStore {
            SnapshotStore::new(
                tempfile::tempdir()
                    .expect("temp dir must exist")
                    .keep(),
            )
        }

        #[tokio::test]
        async fn diff_objects_same_snapshot_produces_empty_deltas() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key: before_key.clone(),
                    after_key: before_key,
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("same snapshot diff must succeed");

            assert!(report.added.is_empty());
            assert!(report.removed.is_empty());
            assert!(report.retained_changed.is_empty());
            assert_eq!(report.strategy, IdentityStrategy::ClassDominator);
        }

        #[tokio::test]
        async fn diff_objects_resolves_snapshot_keys_and_direct_heap_paths() {
            let store = diff_store();
            let (_snap_file, snap_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("snapshot");
            let resolved_from_key =
                resolve_heap_path_from_key(&store, &snap_key).expect("snapshot key must resolve");
            assert!(Path::new(&resolved_from_key).is_file());

            let (_path_file, heap_path) = write_fixture(&build_tune_gc_fixture()).expect("heap");
            let resolved_from_path =
                resolve_heap_path_from_key(&store, &heap_path).expect("heap path must resolve");
            assert_eq!(resolved_from_path, heap_path);
        }

        #[tokio::test]
        async fn diff_objects_fixture_pair_surfaces_add_and_remove_deltas() {
            let (_before_file, before_path) =
                write_fixture(&build_graph_fixture()).expect("before heap");
            let (_after_file, after_path) =
                write_fixture(&build_tune_gc_fixture()).expect("after heap");

            // Synthetic fixtures are below the production min-retained threshold,
            // so use a zero floor here (same technique as core's diff_object_engine).
            let request = DiffRequest {
                before_path,
                after_path,
                mode: DiffMode::Object,
                identity_strategy: IdentityStrategy::ClassDominator,
                retained_bucket_bits: 10,
                min_retained_bytes: 0,
                retained_change_threshold: 1,
                top_n: 50,
                retain_field_data: false,
                cross_reference_leaks: false,
            };

            let report = match run_diff(request).await {
                Ok(DiffResult::Object(diff)) => diff
                    .object_diff
                    .expect("object diff must include a report"),
                Ok(DiffResult::Class(_)) => panic!("expected object diff result"),
                Err(error) => panic!("diff failed: {error}"),
            };

            assert!(
                report
                    .added
                    .iter()
                    .any(|delta| delta.kind == ObjectDeltaKind::Added),
                "expected at least one Added delta"
            );
            assert!(
                report
                    .removed
                    .iter()
                    .any(|delta| delta.kind == ObjectDeltaKind::Removed),
                "expected at least one Removed delta"
            );
        }

        #[tokio::test]
        async fn diff_objects_preserves_match_quality_and_strategy() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let (_after_file, after_key) =
                save_fixture_snapshot(&store, &build_tune_gc_fixture()).expect("after snapshot");

            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key,
                    after_key,
                    strategy: Some(IdentityStrategy::ClassRetained),
                    top_n: Some(10),
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("strategy diff must succeed");

            assert_eq!(report.strategy, IdentityStrategy::ClassRetained);
            assert_eq!(report.match_quality.strategy, IdentityStrategy::ClassRetained);
            assert!(report.match_quality.collision_rate >= 0.0);
            assert!(report.added.len() <= 10);
            assert!(report.removed.len() <= 10);
            assert!(report.retained_changed.len() <= 10);
        }

        #[tokio::test]
        async fn diff_objects_unknown_key_returns_structured_error() {
            let store = diff_store();
            let error = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key: "missing-key".to_string(),
                    after_key: "also-missing".to_string(),
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect_err("unknown keys must fail");

            assert!(error.contains("snapshot_not_found:"));
        }

        #[test]
        fn diff_objects_empty_key_rejected() {
            let store = diff_store();
            let error = resolve_heap_path_from_key(&store, "  ").expect_err("empty key must fail");
            assert!(error.contains("must not be empty"));
        }

        #[test]
        fn parse_identity_strategy_rejects_unknown_labels() {
            let error = parse_identity_strategy("NotARealStrategy").expect_err("invalid strategy");
            assert!(error.contains("Invalid identity strategy"));
        }

        #[tokio::test]
        async fn diff_objects_cross_reference_leaks_defaults_off() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let (_after_file, after_key) =
                save_fixture_snapshot(&store, &build_tune_gc_fixture()).expect("after snapshot");

            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key,
                    after_key,
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("diff must succeed");

            for delta in report
                .added
                .iter()
                .chain(report.retained_changed.iter())
            {
                assert!(delta.leak_severity.is_none());
            }
        }
    }

    mod workflow_bridge {
        use super::*;
        use mnemosyne_core::{
            hprof::{
                parse_hprof_file_with_options,
                test_fixtures::build_graph_fixture,
                ParseOptions,
            },
            workflow::{WorkflowKind, WorkflowStore, WORKFLOW_SCHEMA_VERSION},
        };
        use std::io::Write;

        fn workflow_store() -> WorkflowStore {
            WorkflowStore::new(
                tempfile::tempdir()
                    .expect("temp dir must exist")
                    .keep(),
            )
        }

        fn write_fixture_heap() -> Result<(tempfile::NamedTempFile, String), String> {
            let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
            file.write_all(&build_graph_fixture())
                .map_err(|error| error.to_string())?;
            let path = file
                .path()
                .to_str()
                .expect("temp path must be valid UTF-8")
                .to_string();
            Ok((file, path))
        }

        #[test]
        fn describe_workflow_returns_step_sequence_for_triage_memory_leak() {
            let description =
                describe_workflow_for_session("triage_memory_leak").expect("describe must succeed");
            assert_eq!(description.kind, WorkflowKind::TriageMemoryLeak);
            let names: Vec<&str> = description.steps.iter().map(|step| step.name.as_str()).collect();
            assert_eq!(
                names,
                vec![
                    "detect",
                    "investigate_suspect",
                    "explain",
                    "propose_fix"
                ]
            );
        }

        #[test]
        fn parse_workflow_kind_rejects_unknown_labels() {
            let error = parse_workflow_kind("not_a_real_kind").expect_err("invalid kind");
            assert!(error.contains("unknown workflow kind"));
        }

        #[test]
        fn list_snapshots_for_session_returns_saved_manifests() {
            let store = SnapshotStore::new(
                tempfile::tempdir()
                    .expect("temp dir must exist")
                    .keep(),
            );
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let graph = parse_hprof_file_with_options(&heap_path, ParseOptions::default())
                .map_err(|error| error.to_string())
                .expect("fixture must parse");
            let dominator = build_dominator_tree(&graph);
            store
                .save(&heap_path, &graph, &dominator)
                .expect("snapshot must save");

            let manifests = list_snapshots_for_session(&store).expect("list must succeed");
            assert_eq!(manifests.len(), 1);
            assert_eq!(manifests[0].heap_path, heap_path);
            assert_eq!(manifests[0].object_count, graph.object_count());
        }

        #[test]
        fn workflow_step_response_includes_next_expected_input_for_in_progress_state() {
            let state = WorkflowState {
                schema_version: WORKFLOW_SCHEMA_VERSION,
                workflow_id: "wf-test".into(),
                kind: WorkflowKind::TriageMemoryLeak,
                created_at: "1700000000".into(),
                updated_at: "1700000000".into(),
                heap_path: "heap.hprof".into(),
                current_step: "investigate_suspect".into(),
                step_history: vec![],
                context: json!({}),
            };

            let response = workflow_step_response(&state).expect("response must build");
            assert_eq!(response.get("workflow_id"), Some(&json!("wf-test")));
            assert_eq!(response.get("current_step"), Some(&json!("investigate_suspect")));
            assert!(response
                .get("next_expected_input")
                .and_then(Value::as_array)
                .is_some_and(|params| !params.is_empty()));
        }

        #[tokio::test]
        async fn start_workflow_for_session_runs_detect_step_on_fixture_heap() {
            let store = workflow_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");

            let response = start_workflow_for_session(
                &store,
                StartWorkflowSessionInput {
                    kind: "triage_memory_leak".into(),
                    heap_path: Some(heap_path),
                    ..StartWorkflowSessionInput::default()
                },
            )
            .await
            .expect("start must succeed");

            assert_eq!(
                response.get("current_step"),
                Some(&json!("investigate_suspect"))
            );
            let leaks = response
                .pointer("/step_result/leaks")
                .and_then(Value::as_array)
                .expect("detect step should return leaks");
            assert!(!leaks.is_empty(), "fixture heap should surface at least one leak");
        }

        #[tokio::test]
        async fn next_step_for_session_advances_investigate_suspect_step() {
            let store = workflow_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");

            let start = start_workflow_for_session(
                &store,
                StartWorkflowSessionInput {
                    kind: "triage_memory_leak".into(),
                    heap_path: Some(heap_path),
                    ..StartWorkflowSessionInput::default()
                },
            )
            .await
            .expect("start must succeed");

            let workflow_id = start
                .get("workflow_id")
                .and_then(Value::as_str)
                .expect("workflow_id")
                .to_string();
            let leak_id = start
                .pointer("/step_result/leaks/0/id")
                .and_then(Value::as_str)
                .expect("leak id")
                .to_string();

            let step = next_step_for_session(
                &store,
                &workflow_id,
                json!({ "leak_id": leak_id }),
            )
            .await
            .expect("next step must succeed");

            assert_eq!(step.get("current_step"), Some(&json!("explain")));
        }

        #[test]
        fn start_workflow_for_session_requires_heap_path_for_single_heap_kinds() {
            let store = workflow_store();
            let error = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(start_workflow_for_session(
                    &store,
                    StartWorkflowSessionInput {
                        kind: "tune_gc".into(),
                        ..StartWorkflowSessionInput::default()
                    },
                ))
                .expect_err("missing heap path must fail");

            assert!(error.contains("heap_path is required"));
        }
    }
}
