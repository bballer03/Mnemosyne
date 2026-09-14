use std::{
    path::PathBuf,
    sync::atomic::Ordering,
};

use mnemosyne_core::{
    analysis::{analyze_heap, validate_leak_id, ObjectInspection},
    diff::ObjectDiffReport,
    focus_leaks, generate_ai_insights_async, parse_hprof_file, parse_hprof_file_with_options,
    propose_fix_with_config,
    query::{execute_query, parse_query, CellValue},
    AllPathsRequest, FixRequest, FixResponse, FixStyle, GcPathRequest, GcPathResult,
    HistogramGroupBy, LeakDetectionOptions, MapToCodeRequest, ParseOptions, ProvenanceMarker,
    SourceMapResult, HistogramResult,
};
use mnemosyne_core::snapshot::SnapshotManifest;
use mnemosyne_core::workflow::WorkflowDescription;
use mnemosyne_desktop_session::{
    default_snapshot_store, default_workflow_store, describe_workflow_for_session,
    diff_objects_for_session, find_all_gc_paths_for_session, graph_has_field_data,
    install_field_data_cache_if_still_current, inspect_object_for_session,
    list_snapshots_for_session, next_step_for_session, parse_identity_strategy,
    parse_object_id, regroup_histogram_for_session, start_workflow_for_session,
    DiffObjectsSessionInput, FieldDataCacheCapture, StartWorkflowSessionInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::state::HeapSession;

const NO_HEAP_LOADED: &str = "No heap loaded";
const LOCK_ERROR: &str = "Heap session lock poisoned";
const UNKNOWN_SOURCE: &str = "Unknown heap source";
const INVALID_HEAP_EXTENSION: &str =
    "Selected file must use a .hprof or .bin extension";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapLoadSummary {
    /// Filename only — never an absolute path (Terra 20.B/20.C path gate).
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_id: Option<String>,
    object_count: usize,
    class_count: usize,
    gc_root_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PickHeapFileResult {
    Selected {
        source_id: String,
        display_name: String,
    },
    Cancelled,
    Unavailable,
}

fn display_name_for_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("heap.dump")
        .to_string()
}

fn is_supported_heap_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".hprof") || lower.ends_with(".bin")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapQueryInput {
    heap_path: String,
    query: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapQueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReferenceEntry {
    object_id: String,
    class_name: String,
    shallow_size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReferencesResult {
    object_id: String,
    references: Vec<ObjectReferenceEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReferrersResult {
    object_id: String,
    referrers: Vec<ObjectReferenceEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffObjectsBridgeInput {
    before_key: String,
    after_key: String,
    strategy: Option<String>,
    top_n: Option<usize>,
    cross_reference_leaks: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ExplainLeakResult {
    leak_id: String,
    summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    provenance: Vec<ProvenanceMarker>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn pick_heap_file(
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<PickHeapFileResult, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Heap dumps", &["hprof", "bin"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(PickHeapFileResult::Cancelled);
    };

    let path = match file_path.into_path() {
        Ok(path) => path,
        Err(_) => return Ok(PickHeapFileResult::Unavailable),
    };

    let path_string = path.to_string_lossy().into_owned();
    if !is_supported_heap_path(&path_string) {
        return Err(INVALID_HEAP_EXTENSION.to_string());
    }

    let source_id = Uuid::new_v4().to_string();
    let display_name = display_name_for_path(&path_string);
    let mut sources = state
        .selected_sources
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    sources.insert(source_id.clone(), path_string);

    Ok(PickHeapFileResult::Selected {
        source_id,
        display_name,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_heap_from_source(
    source_id: String,
    state: State<'_, HeapSession>,
) -> Result<HeapLoadSummary, String> {
    let path = {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources
            .get(&source_id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?
    };

    load_heap_internal(path, Some(source_id), &state).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_heap(path: String, state: State<'_, HeapSession>) -> Result<HeapLoadSummary, String> {
    if !is_supported_heap_path(&path) {
        return Err(INVALID_HEAP_EXTENSION.to_string());
    }
    load_heap_internal(path, None, &state).await
}

async fn load_heap_internal(
    path: String,
    source_id: Option<String>,
    state: &State<'_, HeapSession>,
) -> Result<HeapLoadSummary, String> {
    let graph = spawn_blocking({
        let path = path.clone();
        move || parse_hprof_file(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    let summary = HeapLoadSummary {
        display_name: display_name_for_path(&path),
        source_id,
        object_count: graph.object_count(),
        class_count: graph.classes.len(),
        gc_root_count: graph.gc_roots.len(),
    };

    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    state.bump_session_epoch();
    *state.graph.write().map_err(|_| LOCK_ERROR.to_string())? = Some(graph);
    *state.field_data_graph.write().map_err(|_| LOCK_ERROR.to_string())? = None;
    *state.heap_path.write().map_err(|_| LOCK_ERROR.to_string())? = Some(path);

    Ok(summary)
}

#[tauri::command]
pub fn unload_heap(state: State<'_, HeapSession>) -> Result<(), String> {
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut graph = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    if graph.is_none() {
        return Err(NO_HEAP_LOADED.to_string());
    }

    state.bump_session_epoch();
    *graph = None;
    *state.field_data_graph.write().map_err(|_| LOCK_ERROR.to_string())? = None;
    *state.heap_path.write().map_err(|_| LOCK_ERROR.to_string())? = None;

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_references(
    object_id: String,
    state: State<'_, HeapSession>,
) -> Result<ObjectReferencesResult, String> {
    let graph = require_loaded_graph(&state)?;

    spawn_blocking(move || {
        let object_id_num = parse_object_id(&object_id)?;
        let references = graph
            .get_references(object_id_num)
            .into_iter()
            .map(|id| build_reference_entry(&graph, id))
            .collect();

        Ok(ObjectReferencesResult {
            object_id: format_object_id(object_id_num, graph.identifier_size as usize),
            references,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_referrers(
    object_id: String,
    state: State<'_, HeapSession>,
) -> Result<ObjectReferrersResult, String> {
    let graph = require_loaded_graph(&state)?;

    spawn_blocking(move || {
        let object_id_num = parse_object_id(&object_id)?;
        let referrers = graph
            .get_referrers(object_id_num)
            .into_iter()
            .map(|id| build_reference_entry(&graph, id))
            .collect();

        Ok(ObjectReferrersResult {
            object_id: format_object_id(object_id_num, graph.identifier_size as usize),
            referrers,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn query_heap(
    input: HeapQueryInput,
    state: State<'_, HeapSession>,
) -> Result<HeapQueryResult, String> {
    ensure_loaded_heap_matches(&state, Some(&input.heap_path))?;
    let graph = require_loaded_graph(&state)?;

    spawn_blocking(move || {
        let dominator = mnemosyne_core::build_dominator_tree(&graph);
        let query = parse_query(&input.query).map_err(|error| error.to_string())?;
        let result = execute_query(&query, &graph, Some(&dominator)).map_err(|error| error.to_string())?;

        Ok(HeapQueryResult {
            columns: result.columns,
            rows: result
                .rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| query_cell_to_value(cell, graph.identifier_size as usize))
                        .collect()
                })
                .collect(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn regroup_histogram(
    group_by: String,
    state: State<'_, HeapSession>,
) -> Result<HistogramResult, String> {
    let graph = require_loaded_graph(&state)?;
    spawn_blocking(move || regroup_histogram_for_session(&graph, &group_by))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn explain_leak(
    leak_id: String,
    heap_path: String,
    state: State<'_, HeapSession>,
) -> Result<ExplainLeakResult, String> {
    let active_heap_path = ensure_loaded_heap_matches(&state, Some(&heap_path))?;
    let mut analyze_config = read_config(&state)?;
    analyze_config.ai.enabled = false;
    let leak_options = LeakDetectionOptions::from(&analyze_config.analysis);

    let analysis = analyze_heap(mnemosyne_core::AnalyzeRequest {
        heap_path: active_heap_path,
        config: analyze_config.clone(),
        leak_options,
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..mnemosyne_core::AnalyzeRequest::default()
    })
    .await
    .map_err(|error| error.to_string())?;

    validate_leak_id(&analysis.leaks, &leak_id).map_err(|error| error.to_string())?;
    let focused = focus_leaks(&analysis.leaks, Some(&leak_id));
    let mut ai_config = analyze_config.ai.clone();
    ai_config.enabled = true;
    let ai = generate_ai_insights_async(&analysis.summary, &focused, &ai_config)
        .await
        .map_err(|error| error.to_string())?;

    let provenance = focused
        .first()
        .map(|leak| leak.provenance.clone())
        .filter(|markers| !markers.is_empty())
        .unwrap_or_else(|| analysis.provenance.clone());

    Ok(ExplainLeakResult {
        leak_id,
        summary: ai.summary,
        provenance,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn inspect_object(
    object_id: String,
    retain_field_data: Option<bool>,
    state: State<'_, HeapSession>,
) -> Result<ObjectInspection, String> {
    let graph = require_loaded_graph(&state)?;
    let heap_path = require_loaded_heap_path(&state)?;
    let retain_field_data = retain_field_data.unwrap_or(false);
    let cache_capture = FieldDataCacheCapture {
        epoch: state.session_epoch.load(Ordering::Acquire),
        heap_path: heap_path.clone(),
    };
    let cached_field_graph = if retain_field_data && !graph_has_field_data(&graph) {
        state
            .field_data_graph
            .read()
            .map_err(|_| LOCK_ERROR.to_string())?
            .clone()
    } else {
        None
    };

    let (inspection, refreshed_field_graph) = spawn_blocking(move || {
        let (inspect_graph, refreshed_field_graph) =
            if retain_field_data && !graph_has_field_data(&graph) {
                if let Some(cached) =
                    cached_field_graph.filter(|cached| graph_has_field_data(cached))
                {
                    (cached, None)
                } else {
                    let reloaded = parse_hprof_file_with_options(
                        &heap_path,
                        ParseOptions {
                            retain_field_data: true,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    (reloaded.clone(), Some(reloaded))
                }
            } else {
                (graph, None)
            };

        inspect_object_for_session(
            &inspect_graph,
            &heap_path,
            &object_id,
            retain_field_data,
        )
        .map(|inspection| (inspection, refreshed_field_graph))
    })
    .await
    .map_err(|error| error.to_string())??;

    if let Some(field_graph) = refreshed_field_graph {
        let _session = state
            .session_mutation
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        let current_epoch = state.session_epoch.load(Ordering::Acquire);
        let current_heap_path = state
            .heap_path
            .read()
            .map_err(|_| LOCK_ERROR.to_string())?
            .as_deref()
            .map(str::to_string);
        let mut field_data_graph = state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())?;
        install_field_data_cache_if_still_current(
            &cache_capture,
            current_epoch,
            current_heap_path.as_deref(),
            &mut field_data_graph,
            field_graph,
        );
    }

    Ok(inspection)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn find_all_gc_paths(
    object_id: String,
    max_paths: Option<usize>,
    state: State<'_, HeapSession>,
) -> Result<GcPathResult, String> {
    ensure_loaded_heap_matches(&state, None)?;
    let graph = require_loaded_graph(&state)?;
    let heap_path = require_loaded_heap_path(&state)?;
    let max_paths = max_paths.unwrap_or(AllPathsRequest::DEFAULT_MAX_PATHS);

    spawn_blocking(move || {
        find_all_gc_paths_for_session(&graph, &heap_path, &object_id, max_paths)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn find_gc_path(
    object_id: String,
    heap_path: String,
    state: State<'_, HeapSession>,
) -> Result<mnemosyne_core::GcPathResult, String> {
    let active_heap_path = ensure_loaded_heap_matches(&state, Some(&heap_path))?;

    spawn_blocking(move || {
        mnemosyne_core::find_gc_path(&GcPathRequest {
            heap_path: active_heap_path,
            object_id,
            max_depth: None,
        })
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn map_to_code(
    leak_id: Option<String>,
    class_name: String,
    project_root: String,
    state: State<'_, HeapSession>,
) -> Result<SourceMapResult, String> {
    ensure_loaded_heap_matches(&state, None)?;
    let leak_id = leak_id.unwrap_or_else(|| class_name.clone());

    spawn_blocking(move || {
        mnemosyne_core::mapper::map_to_code(&MapToCodeRequest {
            leak_id,
            class_name: Some(class_name),
            project_root: PathBuf::from(project_root),
            include_git_info: true,
        })
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn diff_objects(input: DiffObjectsBridgeInput) -> Result<ObjectDiffReport, String> {
    let strategy = match input.strategy.as_deref() {
        None => None,
        Some(raw) => Some(parse_identity_strategy(raw)?),
    };

    let store = default_snapshot_store();
    diff_objects_for_session(
        &store,
        DiffObjectsSessionInput {
            before_key: input.before_key,
            after_key: input.after_key,
            strategy,
            top_n: input.top_n,
            cross_reference_leaks: input.cross_reference_leaks,
        },
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn propose_fix(
    leak_id: String,
    heap_path: String,
    project_root: Option<String>,
    state: State<'_, HeapSession>,
) -> Result<FixResponse, String> {
    let active_heap_path = ensure_loaded_heap_matches(&state, Some(&heap_path))?;
    let config = read_config(&state)?;

    propose_fix_with_config(
        FixRequest {
            heap_path: active_heap_path,
            leak_id: Some(leak_id),
            style: FixStyle::Defensive,
            project_root: project_root.map(PathBuf::from),
        },
        &config,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn describe_workflow(kind: String) -> Result<WorkflowDescription, String> {
    describe_workflow_for_session(&kind)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_workflow(
    kind: String,
    heap_path: Option<String>,
    object_id: Option<String>,
    before_heap_path: Option<String>,
    after_heap_path: Option<String>,
    before_snapshot_key: Option<String>,
    after_snapshot_key: Option<String>,
) -> Result<Value, String> {
    start_workflow_for_session(
        &default_workflow_store(),
        StartWorkflowSessionInput {
            kind,
            heap_path,
            object_id,
            before_heap_path,
            after_heap_path,
            before_snapshot_key,
            after_snapshot_key,
        },
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn next_step(workflow_id: String, input: Option<Value>) -> Result<Value, String> {
    next_step_for_session(
        &default_workflow_store(),
        &workflow_id,
        input.unwrap_or(Value::Null),
    )
    .await
}

#[tauri::command]
pub async fn list_snapshots() -> Result<Vec<SnapshotManifest>, String> {
    list_snapshots_for_session(&default_snapshot_store())
}

fn require_loaded_heap_path(state: &State<'_, HeapSession>) -> Result<String, String> {
    state
        .heap_path
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())
}

fn require_loaded_graph(state: &State<'_, HeapSession>) -> Result<mnemosyne_core::hprof::ObjectGraph, String> {
    state
        .graph
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())
}

fn read_config(state: &State<'_, HeapSession>) -> Result<mnemosyne_core::AppConfig, String> {
    state
        .config
        .read()
        .map_err(|_| LOCK_ERROR.to_string())
        .map(|config| config.clone())
}

fn ensure_loaded_heap_matches(
    state: &State<'_, HeapSession>,
    expected_heap_path: Option<&str>,
) -> Result<String, String> {
    let has_graph = state
        .graph
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .is_some();
    if !has_graph {
        return Err(NO_HEAP_LOADED.to_string());
    }

    let loaded_heap_path = state
        .heap_path
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())?;

    if let Some(expected) = expected_heap_path {
        if !expected.is_empty() && expected != loaded_heap_path {
            return Err(format!(
                "Loaded heap path '{loaded_heap_path}' does not match requested heap '{expected}'"
            ));
        }
    }

    Ok(loaded_heap_path)
}

fn build_reference_entry(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    object_id: u64,
) -> ObjectReferenceEntry {
    let object = graph.get_object(object_id);
    let class_name = object
        .and_then(|object| graph.class_name(object.class_id))
        .map(prettify_class_name)
        .unwrap_or_else(|| "<unknown>".to_string());
    let shallow_size = object.map(|object| object.shallow_size).unwrap_or(0);

    ObjectReferenceEntry {
        object_id: format_object_id(object_id, graph.identifier_size as usize),
        class_name,
        shallow_size,
        display_name: None,
    }
}

fn query_cell_to_value(cell: CellValue, id_size: usize) -> Value {
    match cell {
        CellValue::Id(value) => Value::String(format_object_id(value, id_size)),
        CellValue::Str(value) => Value::String(value),
        CellValue::Int(value) => Value::Number(value.into()),
        CellValue::Bool(value) => Value::Bool(value),
        CellValue::Null => Value::Null,
    }
}

fn format_object_id(object_id: u64, id_size: usize) -> String {
    let width = id_size * 2;
    format!("0x{object_id:0width$X}")
}

fn prettify_class_name(raw: &str) -> String {
    raw.replace('/', ".")
}

