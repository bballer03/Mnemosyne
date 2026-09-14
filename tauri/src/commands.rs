use std::{path::PathBuf, sync::atomic::Ordering};

use mnemosyne_core::snapshot::SnapshotManifest;
use mnemosyne_core::workflow::WorkflowDescription;
use mnemosyne_core::{
    analysis::{
        analyze_heap, analyze_heap_capturing_graph, validate_leak_id, AnalyzeRequest,
        ObjectInspection,
    },
    diff::ObjectDiffReport,
    evaluate, focus_leaks, generate_ai_insights_async, parse_hprof_file,
    parse_hprof_file_with_options, parse_hprof_overview_file, propose_fix_with_config,
    query::{execute_query, parse_query, CellValue},
    report::flamegraph::{collapse, render, CollapseOptions, FlameFormat, FlameRoot},
    AllPathsRequest, AnalysisMode, FixRequest, FixResponse, FixStyle, GcPathRequest, GcPathResult,
    HistogramGroupBy, HistogramResult, LeakDetectionOptions, MapToCodeRequest, OverviewOptions,
    ParseOptions, Policy, PolicyInput, Predicate, ProvenanceMarker, Severity, SourceMapResult,
};
use mnemosyne_desktop_session::{
    ai_session_store_for_config, chat_session_for_session, close_ai_session_for_session,
    close_workflow_for_session, create_ai_session_for_session, default_snapshot_store,
    default_workflow_store, describe_workflow_for_session, diff_objects_for_session,
    find_all_gc_paths_for_session, get_ai_session_for_session, get_workflow_for_session,
    graph_has_field_data, inspect_object_for_session, install_field_data_cache_if_still_current,
    list_snapshots_for_session, next_step_for_session, open_snapshot_for_session,
    parse_identity_strategy, parse_object_id, regroup_histogram_for_session,
    remove_snapshot_for_session, resume_ai_session_for_session, save_snapshot_for_session,
    start_workflow_for_session, CreateAiSessionInput, DiffObjectsSessionInput,
    FieldDataCacheCapture, StartWorkflowSessionInput,
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
const INVALID_HEAP_EXTENSION: &str = "Selected file must use a .hprof or .bin extension";

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
    /// Field names must be camelCase for the React bridge (`sourceId` / `displayName`).
    #[serde(rename_all = "camelCase")]
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

fn map_native_error(error: impl ToString) -> String {
    let raw = error.to_string();
    if raw.contains('/') || raw.contains('\\') {
        return "Heap open or analysis failed. Check that the file is a valid .hprof/.bin dump."
            .to_string();
    }
    raw
}

fn sanitize_analyze_response_value(mut value: Value, display_name: &str) -> Value {
    if let Some(summary) = value
        .get_mut("summary")
        .and_then(|summary| summary.as_object_mut())
    {
        summary.insert(
            "heap_path".to_string(),
            Value::String(display_name.to_string()),
        );
    }
    value
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAnalysisInput {
    source_id: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    enable_classloaders: Option<bool>,
    #[serde(default)]
    enable_threads: Option<bool>,
    #[serde(default)]
    enable_strings: Option<bool>,
    #[serde(default)]
    enable_collections: Option<bool>,
    #[serde(default)]
    enable_top_instances: Option<bool>,
    #[serde(default)]
    enable_by_referrer: Option<bool>,
    #[serde(default)]
    enable_duplicate_arrays: Option<bool>,
    #[serde(default)]
    top_n: Option<usize>,
    #[serde(default)]
    min_collection_capacity: Option<usize>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn run_desktop_analysis(
    input: DesktopAnalysisInput,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let path = {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources
            .get(&input.source_id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?
    };

    let display_name = display_name_for_path(&path);
    let mode = input.mode.as_deref().unwrap_or("incident");
    if mode.eq_ignore_ascii_case("overview") {
        return Err(
            "Overview-only desktop analysis is not wired yet; use incident or custom deep analysis."
                .to_string(),
        );
    }

    // Home Open-heap / incident defaults: dashboard-useful, field-data light.
    // Strings/collections/threads/duplicate_arrays force retain_field_data and
    // multi-GB RSS on large dumps — opt in via custom flags from other surfaces.
    let enable_classloaders = input.enable_classloaders.unwrap_or(true);
    let enable_threads = input.enable_threads.unwrap_or(false);
    let enable_strings = input.enable_strings.unwrap_or(false);
    let enable_collections = input.enable_collections.unwrap_or(false);
    let enable_top_instances = input.enable_top_instances.unwrap_or(true);
    let enable_by_referrer = input.enable_by_referrer.unwrap_or(false);
    let enable_duplicate_arrays = input.enable_duplicate_arrays.unwrap_or(false);
    let top_n = input.top_n.unwrap_or(25);
    let min_collection_capacity = input.min_collection_capacity.unwrap_or(16);

    let config = state
        .config
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone();

    // Clear any previously loaded graph before analysis so we never retain two
    // object graphs for the same desktop session (Terra 20.C lifecycle gate).
    {
        let _session = state
            .session_mutation
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        state.bump_session_epoch();
        *state.graph.write().map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
    }

    let request = AnalyzeRequest {
        heap_path: path.clone(),
        config,
        leak_options: LeakDetectionOptions::default(),
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        enable_classloaders,
        enable_threads,
        enable_strings,
        enable_collections,
        enable_top_instances,
        enable_by_referrer,
        enable_duplicate_arrays,
        top_n,
        min_collection_capacity,
        min_duplicate_count: 2,
    };

    let file_bytes = std::fs::metadata(&path).ok().map(|meta| meta.len());
    let started = std::time::Instant::now();
    tracing::info!(
        %display_name,
        source_id = %input.source_id,
        mode,
        file_bytes,
        enable_classloaders,
        enable_threads,
        enable_strings,
        enable_collections,
        enable_top_instances,
        enable_by_referrer,
        enable_duplicate_arrays,
        "run_desktop_analysis: starting (lean Home defaults skip field-data reports unless explicitly enabled)"
    );

    let (response, object_graph, _dominator) = match analyze_heap_capturing_graph(request).await {
        Ok(result) => result,
        Err(error) => {
            let mapped = map_native_error(error);
            tracing::error!(
                %display_name,
                elapsed_ms = started.elapsed().as_millis() as u64,
                error = %mapped,
                "run_desktop_analysis: failed"
            );
            return Err(mapped);
        }
    };

    let object_count = object_graph.as_ref().map(|graph| graph.object_count());
    tracing::info!(
        %display_name,
        elapsed_ms = started.elapsed().as_millis() as u64,
        object_count,
        leak_count = response.leaks.len(),
        "run_desktop_analysis: completed"
    );

    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    state.bump_session_epoch();
    *state.graph.write().map_err(|_| LOCK_ERROR.to_string())? = object_graph;
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = Some(path);

    let raw = serde_json::to_value(response).map_err(|error| error.to_string())?;
    Ok(sanitize_analyze_response_value(raw, &display_name))
}

fn desktop_ci_check_exit_code(result: &mnemosyne_core::PolicyResult, fail_on: Severity) -> i32 {
    if result.violations.iter().any(|violation| {
        violation.severity == Severity::Critical
            && violation.actual.is_null()
            && violation.expected.is_null()
            && violation
                .message
                .contains("cannot run in explicit overview mode")
    }) {
        4
    } else if result
        .violations
        .iter()
        .any(|violation| violation.severity >= fail_on)
    {
        1
    } else {
        0
    }
}

fn parse_fail_on(value: Option<&str>) -> Result<Severity, String> {
    match value.unwrap_or("error").to_ascii_lowercase().as_str() {
        "info" => Ok(Severity::Info),
        "warning" => Ok(Severity::Warning),
        "error" => Ok(Severity::Error),
        "critical" => Ok(Severity::Critical),
        other => Err(format!("unsupported failOn severity: {other}")),
    }
}

fn parse_analysis_mode(value: Option<&str>) -> Result<AnalysisMode, String> {
    match value.unwrap_or("deep").to_ascii_lowercase().as_str() {
        "auto" => Ok(AnalysisMode::Auto),
        "deep" => Ok(AnalysisMode::Deep),
        "overview" => Ok(AnalysisMode::Overview),
        other => Err(format!("unsupported analysis mode: {other}")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCiCheckInput {
    source_id: String,
    policy_toml: String,
    #[serde(default)]
    fail_on: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    baseline_source_id: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn run_ci_check(
    input: DesktopCiCheckInput,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let path = {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources
            .get(&input.source_id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?
    };

    let baseline_path = if let Some(baseline_id) = input.baseline_source_id.as_ref() {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        Some(
            sources
                .get(baseline_id)
                .cloned()
                .ok_or_else(|| "Unknown baseline heap source".to_string())?,
        )
    } else {
        None
    };

    let policy = Policy::from_toml_str(&input.policy_toml).map_err(map_native_error)?;
    let fail_on = parse_fail_on(input.fail_on.as_deref())?;
    let requested_mode = parse_analysis_mode(input.mode.as_deref())?;

    let needs_baseline = policy
        .rules
        .iter()
        .any(|rule| matches!(rule.predicate, Predicate::ObjectGrowthThreshold));
    if needs_baseline && baseline_path.is_none() {
        return Err(
            "object_growth_threshold_requires_baseline: pass a baseline heap source for growth rules."
                .to_string(),
        );
    }

    let resolved_mode = match requested_mode {
        AnalysisMode::Auto => {
            let size = std::fs::metadata(&path).map_err(map_native_error)?.len();
            AnalysisMode::Auto.resolve(size)
        }
        mode => mode,
    };

    let object_diff = if let Some(baseline) = baseline_path.as_deref() {
        let result = mnemosyne_core::diff::run_diff(mnemosyne_core::DiffRequest {
            before_path: baseline.into(),
            after_path: path.clone().into(),
            mode: mnemosyne_core::DiffMode::Object,
            identity_strategy: mnemosyne_core::IdentityStrategy::default(),
            retained_bucket_bits: 10,
            min_retained_bytes:
                mnemosyne_core::diff::object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
            retained_change_threshold:
                mnemosyne_core::diff::object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
            top_n: mnemosyne_core::diff::object::types::DEFAULT_OBJECT_DIFF_TOP_N,
            retain_field_data: false,
            cross_reference_leaks: false,
        })
        .await
        .map_err(map_native_error)?;
        match result {
            mnemosyne_core::diff::DiffResult::Object(diff) => diff.object_diff,
            mnemosyne_core::diff::DiffResult::Class(_) => {
                return Err("expected object diff for baseline comparison".to_string());
            }
        }
    } else {
        None
    };

    let result = match resolved_mode {
        AnalysisMode::Overview => {
            let summary = parse_hprof_overview_file(&path, &OverviewOptions::default())
                .map_err(map_native_error)?;
            evaluate(
                &policy,
                &PolicyInput::Overview(&summary),
                requested_mode,
                object_diff.as_ref(),
            )
        }
        AnalysisMode::Deep => {
            let config = state
                .config
                .read()
                .map_err(|_| LOCK_ERROR.to_string())?
                .clone();
            let enable_classloaders = policy
                .rules
                .iter()
                .any(|rule| matches!(rule.predicate, Predicate::ClassloaderLeakCount));
            let analysis = analyze_heap(AnalyzeRequest {
                heap_path: path,
                config,
                leak_options: LeakDetectionOptions::default(),
                enable_ai: false,
                histogram_group_by: HistogramGroupBy::Class,
                enable_classloaders,
                enable_threads: false,
                enable_strings: false,
                enable_collections: false,
                enable_top_instances: false,
                enable_by_referrer: false,
                enable_duplicate_arrays: false,
                top_n: 10,
                min_collection_capacity: 16,
                min_duplicate_count: 2,
            })
            .await
            .map_err(map_native_error)?;
            evaluate(
                &policy,
                &PolicyInput::Deep(&analysis),
                requested_mode,
                object_diff.as_ref(),
            )
        }
        AnalysisMode::Auto => unreachable!("resolved mode must not remain auto"),
    };

    Ok(serde_json::json!({
        "result": result,
        "exit_code": desktop_ci_check_exit_code(&result, fail_on),
        "fail_on": fail_on,
        // Desktop UI must not treat exit_code 0 as a full green pass when
        // deep-only rules were skipped (e.g. auto → overview). CLI/MCP keep
        // the historical exit-code contract; this flag is additive for UI.
        "evaluation_complete": result.skipped.is_empty(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFlamegraphInput {
    source_id: String,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generate_desktop_flamegraph(
    input: DesktopFlamegraphInput,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let path = {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources
            .get(&input.source_id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?
    };

    let root = match input.root.as_deref().unwrap_or("dominator") {
        "dominator" => FlameRoot::Dominator,
        "class-hierarchy" | "class_hierarchy" => FlameRoot::ClassHierarchy,
        "gc-root-path" | "gc_root_path" => FlameRoot::GcRootPath,
        other => return Err(format!("unsupported flamegraph root: {other}")),
    };
    let format = match input.format.as_deref().unwrap_or("svg") {
        "svg" => FlameFormat::Svg,
        "folded" | "folded-stack" | "folded_stack" => FlameFormat::FoldedStack,
        "json" => FlameFormat::Json,
        other => return Err(format!("unsupported flamegraph format: {other}")),
    };

    let config = state
        .config
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone();

    let (graph, dominator) = {
        let (_, graph, dominator) =
            mnemosyne_core::analysis::analyze_heap_with_graph(AnalyzeRequest {
                heap_path: path.clone(),
                config,
                leak_options: LeakDetectionOptions::default(),
                enable_ai: false,
                histogram_group_by: HistogramGroupBy::Class,
                ..AnalyzeRequest::default()
            })
            .await
            .map_err(map_native_error)?;
        (graph, dominator)
    };

    {
        let _session = state
            .session_mutation
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        state.bump_session_epoch();
        *state.graph.write().map_err(|_| LOCK_ERROR.to_string())? = Some(graph.clone());
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = Some(path);
    }

    let stacks = collapse(root, &graph, &dominator, &CollapseOptions::default());
    let mut buffer = Vec::new();
    render(&stacks, format, Some("Mnemosyne"), &mut buffer).map_err(map_native_error)?;
    let rendered = String::from_utf8(buffer).map_err(|error| error.to_string())?;

    match format {
        FlameFormat::Svg => Ok(serde_json::json!({
            "format": "svg",
            "content": rendered,
            "byteLength": rendered.len(),
        })),
        FlameFormat::FoldedStack => Ok(serde_json::json!({
            "format": "folded",
            "content": rendered,
            "byteLength": rendered.len(),
        })),
        FlameFormat::Json => {
            let value: Value =
                serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "format": "json",
                "content": value,
                "byteLength": rendered.len(),
            }))
        }
    }
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
    tracing::info!("pick_heap_file: opening native file dialog");
    let picked = app
        .dialog()
        .file()
        .add_filter("Heap dumps", &["hprof", "bin"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        tracing::info!("pick_heap_file: cancelled");
        return Ok(PickHeapFileResult::Cancelled);
    };

    let path = match file_path.into_path() {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(error = %error, "pick_heap_file: path conversion failed");
            return Ok(PickHeapFileResult::Unavailable);
        }
    };

    let path_string = path.to_string_lossy().into_owned();
    if !is_supported_heap_path(&path_string) {
        tracing::warn!(ext_ok = false, "pick_heap_file: unsupported extension");
        return Err(INVALID_HEAP_EXTENSION.to_string());
    }

    let file_bytes = std::fs::metadata(&path).ok().map(|meta| meta.len());
    let source_id = Uuid::new_v4().to_string();
    let display_name = display_name_for_path(&path_string);
    let mut sources = state
        .selected_sources
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    sources.insert(source_id.clone(), path_string);

    tracing::info!(
        %display_name,
        %source_id,
        file_bytes,
        "pick_heap_file: selected"
    );

    Ok(PickHeapFileResult::Selected {
        source_id,
        display_name,
    })
}

/// Absolute path to the desktop host log file (for Support / Validation Console).
/// Path is the log location itself — not a heap path.
#[tauri::command(rename_all = "camelCase")]
pub fn get_desktop_log_path() -> String {
    crate::logging::log_file_path()
        .to_string_lossy()
        .into_owned()
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
pub async fn load_heap(
    path: String,
    state: State<'_, HeapSession>,
) -> Result<HeapLoadSummary, String> {
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
        move || parse_hprof_file(&path).map_err(map_native_error)
    })
    .await
    .map_err(map_native_error)??;

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
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = Some(path);

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
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;

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
        let result =
            execute_query(&query, &graph, Some(&dominator)).map_err(|error| error.to_string())?;

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
        let (inspect_graph, refreshed_field_graph) = if retain_field_data
            && !graph_has_field_data(&graph)
        {
            if let Some(cached) = cached_field_graph.filter(|cached| graph_has_field_data(cached)) {
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

        inspect_object_for_session(&inspect_graph, &heap_path, &object_id, retain_field_data)
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

    spawn_blocking(move || find_all_gc_paths_for_session(&graph, &heap_path, &object_id, max_paths))
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

#[tauri::command(rename_all = "camelCase")]
pub async fn get_workflow(workflow_id: String) -> Result<Value, String> {
    get_workflow_for_session(&default_workflow_store(), &workflow_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn close_workflow(workflow_id: String) -> Result<Value, String> {
    close_workflow_for_session(&default_workflow_store(), &workflow_id)
}

/// M23.C — create a persisted AI session over the currently loaded heap.
#[tauri::command(rename_all = "camelCase")]
pub async fn create_ai_session(
    source_id: Option<String>,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let heap_path = if let Some(id) = source_id.as_ref() {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        let resolved = sources
            .get(id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?;
        // When a heap is already loaded, the source must address that same file.
        if let Ok(loaded) = require_loaded_heap_path(&state) {
            if loaded != resolved {
                return Err(format!(
                    "Loaded heap does not match requested source '{id}'"
                ));
            }
        }
        resolved
    } else {
        require_loaded_heap_path(&state)?
    };

    let config = read_config(&state)?;
    let store = ai_session_store_for_config(&config);
    create_ai_session_for_session(&store, &config, CreateAiSessionInput { heap_path }).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn resume_ai_session(
    session_id: String,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let config = read_config(&state)?;
    let store = ai_session_store_for_config(&config);
    resume_ai_session_for_session(&store, &session_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_ai_session(
    session_id: String,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let config = read_config(&state)?;
    let store = ai_session_store_for_config(&config);
    get_ai_session_for_session(&store, &session_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn close_ai_session(
    session_id: String,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let config = read_config(&state)?;
    let store = ai_session_store_for_config(&config);
    close_ai_session_for_session(&store, &session_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn chat_session(
    session_id: String,
    question: String,
    focus_leak_id: Option<String>,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let config = read_config(&state)?;
    let store = ai_session_store_for_config(&config);
    chat_session_for_session(
        &store,
        &config,
        &session_id,
        &question,
        focus_leak_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_snapshots() -> Result<Vec<SnapshotManifest>, String> {
    list_snapshots_for_session(&default_snapshot_store())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSnapshotInput {
    source_id: String,
    #[serde(default)]
    retain_field_data: Option<bool>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn save_snapshot(
    input: SaveSnapshotInput,
    state: State<'_, HeapSession>,
) -> Result<SnapshotManifest, String> {
    let path = {
        let sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources
            .get(&input.source_id)
            .cloned()
            .ok_or_else(|| UNKNOWN_SOURCE.to_string())?
    };

    let retain_field_data = input.retain_field_data.unwrap_or(false);
    let heap_path = path.clone();
    spawn_blocking(move || {
        save_snapshot_for_session(&default_snapshot_store(), &heap_path, retain_field_data)
            .map_err(map_native_error)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn remove_snapshot(key: String) -> Result<Value, String> {
    remove_snapshot_for_session(&default_snapshot_store(), &key).map_err(map_native_error)
}

/// Load a cached snapshot by SHA-256 store key into the live desktop session.
///
/// Registers an opaque `sourceId` → `manifest.heap_path` for later save/ci_check
/// and returns a display-safe summary (basename + counts only — no absolute paths).
#[tauri::command(rename_all = "camelCase")]
pub async fn open_snapshot(
    key: String,
    state: State<'_, HeapSession>,
) -> Result<HeapLoadSummary, String> {
    let (manifest, graph, _dominator) = spawn_blocking(move || {
        open_snapshot_for_session(&default_snapshot_store(), &key).map_err(map_native_error)
    })
    .await
    .map_err(|error| error.to_string())??;

    let heap_path = manifest.heap_path;
    let display_name = display_name_for_path(&heap_path);
    let source_id = Uuid::new_v4().to_string();

    {
        let mut sources = state
            .selected_sources
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        sources.insert(source_id.clone(), heap_path.clone());
    }

    let summary = HeapLoadSummary {
        display_name,
        source_id: Some(source_id),
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
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = Some(heap_path);

    Ok(summary)
}

fn require_loaded_heap_path(state: &State<'_, HeapSession>) -> Result<String, String> {
    state
        .heap_path
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())
}

fn require_loaded_graph(
    state: &State<'_, HeapSession>,
) -> Result<mnemosyne_core::hprof::ObjectGraph, String> {
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

#[cfg(test)]
mod pick_heap_file_result_tests {
    use super::PickHeapFileResult;

    #[test]
    fn selected_serializes_camel_case_fields_for_ui_bridge() {
        let value = serde_json::to_value(PickHeapFileResult::Selected {
            source_id: "src-1".to_string(),
            display_name: "fixture.hprof".to_string(),
        })
        .expect("serialize");

        assert_eq!(value["status"], "selected");
        assert_eq!(value["sourceId"], "src-1");
        assert_eq!(value["displayName"], "fixture.hprof");
        assert!(value.get("source_id").is_none());
        assert!(value.get("display_name").is_none());
    }
}
