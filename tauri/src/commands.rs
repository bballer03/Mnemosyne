use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use mnemosyne_core::graph::build_dominator_tree_controlled;
use mnemosyne_core::snapshot::SnapshotManifest;
use mnemosyne_core::workflow::WorkflowDescription;
use mnemosyne_core::{
    analysis::{
        analyze_heap, analyze_heap_capturing_graph_controlled, analyze_heap_with_graph_controlled,
        analyze_snapshot_from_graph_controlled, validate_leak_id, AnalyzeRequest, AnalyzeResponse,
        ObjectInspection,
    },
    diff::ObjectDiffReport,
    evaluate, focus_leaks, generate_ai_insights_async, parse_hprof_file_controlled,
    parse_hprof_file_with_options_controlled, parse_hprof_overview_file, propose_fix_with_config,
    query::{execute_query, parse_query, CellValue},
    render_report,
    report::flamegraph::{collapse, render, CollapseOptions, FlameFormat, FlameRoot},
    AllPathsRequest, AnalysisMode, FixRequest, FixResponse, FixStyle, GcPathRequest, GcPathResult,
    HistogramGroupBy, HistogramResult, LeakDetectionOptions, MapToCodeRequest,
    NoopOperationObserver, OperationObserver, OperationPhase, OperationProgressSnapshot,
    OutputFormat, OverviewOptions, ParseOptions, Policy, PolicyInput, Predicate, ProvenanceMarker,
    ReportRequest, Severity, SourceMapResult,
};
use mnemosyne_desktop_session::{
    ai_session_store_for_config, build_snapshot_workspace_hydrate, chat_session_for_session,
    close_ai_session_for_session, close_workflow_for_session, create_ai_session_for_session,
    default_snapshot_store, default_workflow_store, describe_workflow_for_session,
    diff_objects_for_session, dominator_children_for_session, find_all_gc_paths_for_session,
    get_ai_session_for_session, get_workflow_for_session, graph_has_field_data,
    inspect_object_for_session_controlled, install_field_data_cache_if_still_current,
    list_class_instances_for_session, list_snapshots_for_session, next_step_for_session,
    open_snapshot_for_session, parse_identity_strategy, parse_object_id,
    regroup_histogram_for_session, remove_snapshot_for_session, replace_session_analysis,
    resume_ai_session_for_session, start_workflow_for_session,
    structured_operation_cancelled_error, CancelOperationResult, CreateAiSessionInput,
    DiffObjectsSessionInput, FieldDataCacheCapture, OperationContext, OperationEnvelope,
    OperationProgress, OperationProgressCoalescer, OperationRegistration, OperationRegistry,
    SnapshotWorkspaceHydrate, StartWorkflowSessionInput, DEFAULT_CLASS_INSTANCES_LIMIT,
    DEFAULT_DOMINATOR_CHILDREN_LIMIT,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::state::HeapSession;

const NO_HEAP_LOADED: &str = "No heap loaded";
const LOCK_ERROR: &str = "Heap session lock poisoned";
const UNKNOWN_SOURCE: &str = "Unknown heap source";
const INVALID_HEAP_EXTENSION: &str = "Selected file must use a .hprof or .bin extension";

type SharedOperationObserver = Option<Arc<TauriOperationObserver>>;

fn require_operation_context(
    context: Option<OperationContext>,
) -> Result<OperationContext, String> {
    context.ok_or_else(|| "Missing operation context".to_string())
}

fn registered_operation_observer<'a>(
    app: &AppHandle,
    context: Option<OperationContext>,
    kind: &str,
    registry: &'a OperationRegistry,
) -> Result<(SharedOperationObserver, Option<OperationRegistration<'a>>), String> {
    let Some(context) = context else {
        return Ok((None, None));
    };
    let registration = registry
        .register(context.clone())
        .map_err(|error| error.to_string())?;
    let observer = Arc::new(TauriOperationObserver::new(
        app.clone(),
        context,
        kind,
        registration.cancellation_token(),
    ));
    Ok((Some(observer), Some(registration)))
}

fn core_observer(observer: &SharedOperationObserver) -> &dyn OperationObserver {
    observer
        .as_deref()
        .map(|observer| observer as &dyn OperationObserver)
        .unwrap_or(&NoopOperationObserver)
}

fn ensure_observer_not_cancelled(observer: &SharedOperationObserver) -> Result<(), String> {
    if core_observer(observer).is_cancelled() {
        Err(structured_operation_cancelled_error())
    } else {
        Ok(())
    }
}

fn ensure_operation_can_commit(
    registration: Option<&OperationRegistration<'_>>,
) -> Result<(), String> {
    if registration.is_some_and(|registration| !registration.can_commit()) {
        Err(structured_operation_cancelled_error())
    } else {
        Ok(())
    }
}

fn emit_indeterminate(
    observer: &SharedOperationObserver,
    phase: OperationPhase,
    started: std::time::Instant,
) {
    if let Some(observer) = observer {
        observer.progress(OperationProgressSnapshot {
            phase,
            completed: None,
            total: None,
            unit: None,
            indeterminate: true,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }
}

fn emit_completed(
    observer: &SharedOperationObserver,
    phase: OperationPhase,
    started: std::time::Instant,
) {
    if let Some(observer) = observer {
        observer.progress(OperationProgressSnapshot {
            phase,
            completed: Some(1),
            total: Some(1),
            unit: Some("stage".to_string()),
            indeterminate: false,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }
}

fn emit_operation_error(
    observer: &SharedOperationObserver,
    error: &str,
    started: std::time::Instant,
) {
    emit_indeterminate(
        observer,
        if error == "Operation cancelled" || error.starts_with("operation_cancelled:") {
            OperationPhase::Cancelled
        } else {
            OperationPhase::Failed
        },
        started,
    );
}

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
    if raw == "Operation cancelled" {
        return structured_operation_cancelled_error();
    }
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
    context: Option<OperationContext>,
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
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<Value>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "analyze", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Opening, started);

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
        let mut graph = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
        let mut dominator = state
            .dominator
            .write()
            .map_err(|_| LOCK_ERROR.to_string())?;
        replace_session_analysis(&mut graph, &mut dominator, None);
        *state.analysis.write().map_err(|_| LOCK_ERROR.to_string())? = None;
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

    let (response, object_graph, dominator) =
        match analyze_heap_capturing_graph_controlled(request, core_observer(&observer)).await {
            Ok(result) => result,
            Err(error) => {
                let cancelled = matches!(&error, mnemosyne_core::CoreError::OperationCancelled);
                emit_indeterminate(
                    &observer,
                    if cancelled {
                        OperationPhase::Cancelled
                    } else {
                        OperationPhase::Failed
                    },
                    started,
                );
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
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }

    let object_count = object_graph.as_ref().map(|graph| graph.object_count());
    tracing::info!(
        %display_name,
        elapsed_ms = started.elapsed().as_millis() as u64,
        object_count,
        leak_count = response.leaks.len(),
        "run_desktop_analysis: completed"
    );

    emit_indeterminate(&observer, OperationPhase::Committing, started);
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    state.bump_session_epoch();
    let replacement = match (object_graph, dominator) {
        (Some(graph), Some(dominator)) => Some((graph, dominator)),
        (None, None) => None,
        _ => return Err("Analysis returned an incomplete graph/dominator pair".to_string()),
    };
    let mut graph = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    let mut dominator = state
        .dominator
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut analysis_slot = state.analysis.write().map_err(|_| LOCK_ERROR.to_string())?;
    replace_session_analysis(&mut graph, &mut dominator, replacement);
    *analysis_slot = Some(response.clone());
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = Some(path);

    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        replace_session_analysis(&mut graph, &mut dominator, None);
        *analysis_slot = None;
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    let raw = serde_json::to_value(&response).map_err(|error| error.to_string())?;
    let result = sanitize_analyze_response_value(raw, &display_name);
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        replace_session_analysis(&mut graph, &mut dominator, None);
        *analysis_slot = None;
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    emit_completed(&observer, OperationPhase::Complete, started);
    Ok(OperationEnvelope::new(context, result))
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
    context: Option<OperationContext>,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generate_desktop_flamegraph(
    input: DesktopFlamegraphInput,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<Value>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) = registered_operation_observer(
        &app,
        Some(context.clone()),
        "flamegraph",
        &state.operations,
    )?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Opening, started);

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

    let (mut analysis, graph, dominator) = {
        analyze_heap_with_graph_controlled(
            AnalyzeRequest {
                heap_path: path.clone(),
                config,
                leak_options: LeakDetectionOptions::default(),
                enable_ai: false,
                histogram_group_by: HistogramGroupBy::Class,
                ..AnalyzeRequest::default()
            },
            core_observer(&observer),
        )
        .await
        .map_err(map_native_error)?
    };
    analysis.summary.heap_path = display_name;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }

    emit_indeterminate(&observer, OperationPhase::Rendering, started);
    let stacks = collapse(root, &graph, &dominator, &CollapseOptions::default());
    let mut buffer = Vec::new();
    render(&stacks, format, Some("Mnemosyne"), &mut buffer).map_err(map_native_error)?;
    let rendered = String::from_utf8(buffer).map_err(|error| error.to_string())?;
    let mode = serde_json::to_value(analysis.mode).map_err(|error| error.to_string())?;
    let provenance =
        serde_json::to_value(&analysis.provenance).map_err(|error| error.to_string())?;

    let result: Result<Value, String> = match format {
        FlameFormat::Svg => Ok(serde_json::json!({
            "format": "svg",
            "content": rendered,
            "byteLength": rendered.len(),
            "mode": mode,
            "provenance": provenance,
        })),
        FlameFormat::FoldedStack => Ok(serde_json::json!({
            "format": "folded-stack",
            "content": rendered,
            "byteLength": rendered.len(),
            "mode": mode,
            "provenance": provenance,
        })),
        FlameFormat::Json => {
            let value: Value =
                serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "format": "json",
                "content": value,
                "byteLength": rendered.len(),
                "mode": mode,
                "provenance": provenance,
            }))
        }
    };
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_indeterminate(&observer, OperationPhase::Committing, started);
        let _session = state
            .session_mutation
            .lock()
            .map_err(|_| LOCK_ERROR.to_string())?;
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        state.bump_session_epoch();
        let mut graph_slot = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
        let mut dominator_slot = state
            .dominator
            .write()
            .map_err(|_| LOCK_ERROR.to_string())?;
        let mut analysis_slot = state.analysis.write().map_err(|_| LOCK_ERROR.to_string())?;
        replace_session_analysis(
            &mut graph_slot,
            &mut dominator_slot,
            Some((graph, dominator)),
        );
        *analysis_slot = Some(analysis);
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = Some(path);
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            replace_session_analysis(&mut graph_slot, &mut dominator_slot, None);
            *analysis_slot = None;
            *state
                .field_data_graph
                .write()
                .map_err(|_| LOCK_ERROR.to_string())? = None;
            *state
                .heap_path
                .write()
                .map_err(|_| LOCK_ERROR.to_string())? = None;
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopReportExportInput {
    source_id: String,
    format: String,
    #[serde(default)]
    context: Option<OperationContext>,
}

fn parse_report_export_format(value: &str) -> Result<(OutputFormat, &'static str), String> {
    match value.to_ascii_lowercase().as_str() {
        "text" => Ok((OutputFormat::Text, "text")),
        "markdown" => Ok((OutputFormat::Markdown, "markdown")),
        "html" => Ok((OutputFormat::Html, "html")),
        "toon" => Ok((OutputFormat::Toon, "toon")),
        "json" => Ok((OutputFormat::Json, "json")),
        other => Err(format!("unsupported report export format: {other}")),
    }
}

fn render_desktop_report_export(
    mut analysis: AnalyzeResponse,
    display_name: &str,
    requested_format: &str,
) -> Result<Value, String> {
    let (format, format_name) = parse_report_export_format(requested_format)?;
    analysis.summary.heap_path = display_name.to_string();
    let mode = serde_json::to_value(analysis.mode).map_err(|error| error.to_string())?;
    let provenance =
        serde_json::to_value(&analysis.provenance).map_err(|error| error.to_string())?;
    let report = render_report(&ReportRequest { analysis, format }).map_err(map_native_error)?;
    let byte_length = report.contents.as_bytes().len();

    Ok(serde_json::json!({
        "format": format_name,
        "content": report.contents,
        "mimeType": report.mime_type,
        "byteLength": byte_length,
        "mode": mode,
        "provenance": provenance,
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn export_desktop_report(
    input: DesktopReportExportInput,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<Value>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "analyze", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Opening, started);

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
    ensure_loaded_heap_matches(&state, Some(&path))?;
    let analysis = state
        .analysis
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| {
            "No committed analysis is available for the active workspace; run analysis first."
                .to_string()
        })?;

    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    emit_indeterminate(&observer, OperationPhase::Rendering, started);
    let result =
        render_desktop_report_export(analysis, &display_name_for_path(&path), &input.format)?;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    emit_completed(&observer, OperationPhase::Complete, started);
    Ok(OperationEnvelope::new(context, result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapQueryInput {
    heap_path: String,
    query: String,
    #[serde(default)]
    context: Option<OperationContext>,
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
    #[serde(default)]
    context: Option<OperationContext>,
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
pub fn cancel_operation(
    operation_id: String,
    state: State<'_, HeapSession>,
) -> CancelOperationResult {
    state.operations.cancel(&operation_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_heap_from_source(
    source_id: String,
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<HeapLoadSummary>, String> {
    let context = require_operation_context(context)?;
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

    load_heap_internal(path, Some(source_id), context, &app, &state).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_heap(
    path: String,
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<HeapLoadSummary>, String> {
    let context = require_operation_context(context)?;
    if !is_supported_heap_path(&path) {
        return Err(INVALID_HEAP_EXTENSION.to_string());
    }
    load_heap_internal(path, None, context, &app, &state).await
}

async fn load_heap_internal(
    path: String,
    source_id: Option<String>,
    context: OperationContext,
    app: &AppHandle,
    state: &State<'_, HeapSession>,
) -> Result<OperationEnvelope<HeapLoadSummary>, String> {
    let started = std::time::Instant::now();
    let (observer, registration) =
        registered_operation_observer(app, Some(context.clone()), "open", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Opening, started);
    let background_observer = observer.clone();
    let (graph, dominator) = spawn_blocking({
        let path = path.clone();
        move || {
            ensure_observer_not_cancelled(&background_observer)?;
            let graph = parse_hprof_file_controlled(&path, core_observer(&background_observer))
                .map_err(map_native_error)?;
            ensure_observer_not_cancelled(&background_observer)?;
            emit_indeterminate(&background_observer, OperationPhase::BuildingGraph, started);
            emit_indeterminate(
                &background_observer,
                OperationPhase::ComputingDominators,
                started,
            );
            let dominator =
                build_dominator_tree_controlled(&graph, core_observer(&background_observer))
                    .map_err(map_native_error)?;
            ensure_observer_not_cancelled(&background_observer)?;
            emit_indeterminate(&background_observer, OperationPhase::Analyzing, started);
            Ok::<_, String>((graph, dominator))
        }
    })
    .await
    .map_err(map_native_error)??;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }

    let summary = HeapLoadSummary {
        display_name: display_name_for_path(&path),
        source_id,
        object_count: graph.object_count(),
        class_count: graph.classes.len(),
        gc_root_count: graph.gc_roots.len(),
    };

    emit_indeterminate(&observer, OperationPhase::Committing, started);
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    state.bump_session_epoch();
    let mut graph_slot = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    let mut dominator_slot = state
        .dominator
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut analysis_slot = state.analysis.write().map_err(|_| LOCK_ERROR.to_string())?;
    replace_session_analysis(
        &mut graph_slot,
        &mut dominator_slot,
        Some((graph, dominator)),
    );
    *analysis_slot = None;
    *state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = None;
    *state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())? = Some(path);

    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        replace_session_analysis(&mut graph_slot, &mut dominator_slot, None);
        *analysis_slot = None;
        *state
            .field_data_graph
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        *state
            .heap_path
            .write()
            .map_err(|_| LOCK_ERROR.to_string())? = None;
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    emit_completed(&observer, OperationPhase::Complete, started);
    Ok(OperationEnvelope::new(context, summary))
}

#[tauri::command]
pub fn unload_heap(state: State<'_, HeapSession>) -> Result<(), String> {
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut graph = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    let mut dominator = state
        .dominator
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut analysis = state.analysis.write().map_err(|_| LOCK_ERROR.to_string())?;
    // Idempotent: Close from UI must succeed even if the graph was already cleared.
    if graph.is_none() && dominator.is_none() && analysis.is_none() {
        return Ok(());
    }

    state.bump_session_epoch();
    replace_session_analysis(&mut graph, &mut dominator, None);
    *analysis = None;
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
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<HeapQueryResult>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "query", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Analyzing, started);
    ensure_loaded_heap_matches(&state, Some(&input.heap_path))?;
    let (graph, dominator) = require_loaded_analysis(&state)?;
    let background_observer = observer.clone();

    let result = spawn_blocking(move || -> Result<HeapQueryResult, String> {
        ensure_observer_not_cancelled(&background_observer)?;
        let query = parse_query(&input.query).map_err(|error| error.to_string())?;
        let result =
            execute_query(&query, &graph, Some(&dominator)).map_err(|error| error.to_string())?;

        let response = HeapQueryResult {
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
        };
        ensure_observer_not_cancelled(&background_observer)?;
        Ok(response)
    })
    .await
    .map_err(|error| error.to_string())?;
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn regroup_histogram(
    group_by: String,
    state: State<'_, HeapSession>,
) -> Result<HistogramResult, String> {
    let (graph, dominator) = require_loaded_analysis(&state)?;
    spawn_blocking(move || regroup_histogram_for_session(&graph, &dominator, &group_by))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_class_instances(
    class_key: String,
    offset: Option<usize>,
    limit: Option<usize>,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let (graph, dominator) = require_loaded_analysis(&state)?;
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(DEFAULT_CLASS_INSTANCES_LIMIT);

    spawn_blocking(move || {
        let page = list_class_instances_for_session(&graph, &dominator, &class_key, offset, limit)?;
        let instances = page
            .instances
            .into_iter()
            .map(|instance| {
                serde_json::json!({
                    "object_id": instance.object_id,
                    "class_name": instance.class_name,
                    "shallow_size": instance.shallow_size,
                    "retained_size": instance.retained_size,
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "class_key": page.class_key,
            "total": page.total,
            "returned": page.returned,
            "offset": page.offset,
            "limit": page.limit,
            "truncated": page.truncated,
            "instances": instances,
        }))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_dominator_children(
    parent_object_id: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    min_retained_bytes: Option<u64>,
    state: State<'_, HeapSession>,
) -> Result<Value, String> {
    let (graph, dominator) = require_loaded_analysis(&state)?;
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(DEFAULT_DOMINATOR_CHILDREN_LIMIT);
    let min_retained_bytes = min_retained_bytes.unwrap_or(0);

    spawn_blocking(move || {
        let page = dominator_children_for_session(
            &graph,
            &dominator,
            parent_object_id.as_deref(),
            offset,
            limit,
            min_retained_bytes,
        );
        let children = page
            .children
            .into_iter()
            .map(|child| {
                serde_json::json!({
                    "object_id": child.object_id,
                    "class_name": child.class_name,
                    "shallow_size": child.shallow_size,
                    "retained_size": child.retained_size,
                    "dominated_count": child.dominated_count,
                    "has_children": child.has_children,
                })
            })
            .collect::<Vec<_>>();

        Ok::<Value, String>(serde_json::json!({
            "total": page.total,
            "returned": page.returned,
            "offset": page.offset,
            "limit": page.limit,
            "truncated": page.truncated,
            "children": children,
        }))
    })
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
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<ObjectInspection>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(context)?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "inspect", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
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

    let background_observer = observer.clone();
    let (inspection, refreshed_field_graph) = spawn_blocking(move || {
        let (inspect_graph, refreshed_field_graph) = if retain_field_data
            && !graph_has_field_data(&graph)
        {
            if let Some(cached) = cached_field_graph.filter(|cached| graph_has_field_data(cached)) {
                (cached, None)
            } else {
                let reloaded = parse_hprof_file_with_options_controlled(
                    &heap_path,
                    ParseOptions {
                        retain_field_data: true,
                    },
                    core_observer(&background_observer),
                )
                .map_err(map_native_error)?;
                (reloaded.clone(), Some(reloaded))
            }
        } else {
            (graph, None)
        };

        emit_indeterminate(&background_observer, OperationPhase::Analyzing, started);
        ensure_observer_not_cancelled(&background_observer)?;
        let inspection = inspect_object_for_session_controlled(
            &inspect_graph,
            &heap_path,
            &object_id,
            retain_field_data,
            core_observer(&background_observer),
        )
        .map_err(map_native_error)?;
        ensure_observer_not_cancelled(&background_observer)?;
        Ok::<_, String>((inspection, refreshed_field_graph))
    })
    .await
    .map_err(|error| error.to_string())??;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }

    let mut installed_field_data = false;
    let _session = if refreshed_field_graph.is_some() {
        Some(
            state
                .session_mutation
                .lock()
                .map_err(|_| LOCK_ERROR.to_string())?,
        )
    } else {
        None
    };
    if let Some(field_graph) = refreshed_field_graph {
        emit_indeterminate(&observer, OperationPhase::Committing, started);
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
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
        installed_field_data = install_field_data_cache_if_still_current(
            &cache_capture,
            current_epoch,
            current_heap_path.as_deref(),
            &mut field_data_graph,
            field_graph,
        );
    }

    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        if installed_field_data {
            *state
                .field_data_graph
                .write()
                .map_err(|_| LOCK_ERROR.to_string())? = None;
        }
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }
    emit_completed(&observer, OperationPhase::Complete, started);
    Ok(OperationEnvelope::new(context, inspection))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn find_all_gc_paths(
    object_id: String,
    max_paths: Option<usize>,
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<GcPathResult>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(context)?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "gc-path", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Analyzing, started);
    ensure_loaded_heap_matches(&state, None)?;
    let graph = require_loaded_graph(&state)?;
    let heap_path = require_loaded_heap_path(&state)?;
    let max_paths = max_paths.unwrap_or(AllPathsRequest::DEFAULT_MAX_PATHS);
    let background_observer = observer.clone();

    let result = spawn_blocking(move || {
        ensure_observer_not_cancelled(&background_observer)?;
        let result = find_all_gc_paths_for_session(&graph, &heap_path, &object_id, max_paths)?;
        ensure_observer_not_cancelled(&background_observer)?;
        Ok::<_, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?;
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn find_gc_path(
    object_id: String,
    heap_path: String,
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<mnemosyne_core::GcPathResult>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(context)?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "gc-path", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Analyzing, started);
    let active_heap_path = ensure_loaded_heap_matches(&state, Some(&heap_path))?;
    let background_observer = observer.clone();

    let result = spawn_blocking(move || {
        ensure_observer_not_cancelled(&background_observer)?;
        let result = mnemosyne_core::find_gc_path(&GcPathRequest {
            heap_path: active_heap_path,
            object_id,
            max_depth: None,
        })
        .map_err(map_native_error)?;
        ensure_observer_not_cancelled(&background_observer)?;
        Ok::<_, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?;
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
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
pub async fn diff_objects(
    input: DiffObjectsBridgeInput,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<ObjectDiffReport>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "diff", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Analyzing, started);
    let strategy = match input.strategy.as_deref() {
        None => None,
        Some(raw) => Some(parse_identity_strategy(raw)?),
    };

    let store = default_snapshot_store();
    let result = diff_objects_for_session(
        &store,
        DiffObjectsSessionInput {
            before_key: input.before_key,
            after_key: input.after_key,
            strategy,
            top_n: input.top_n,
            cross_reference_leaks: input.cross_reference_leaks,
        },
    )
    .await;
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
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
    context: Option<OperationContext>,
    #[serde(default)]
    retain_field_data: Option<bool>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn save_snapshot(
    input: SaveSnapshotInput,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<SnapshotManifest>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(input.context.clone())?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "snapshot", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
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
    let background_observer = observer.clone();
    let result = spawn_blocking(move || {
        ensure_observer_not_cancelled(&background_observer)?;
        let graph = parse_hprof_file_with_options_controlled(
            &heap_path,
            ParseOptions { retain_field_data },
            core_observer(&background_observer),
        )
        .map_err(map_native_error)?;
        ensure_observer_not_cancelled(&background_observer)?;
        emit_indeterminate(&background_observer, OperationPhase::BuildingGraph, started);
        emit_indeterminate(
            &background_observer,
            OperationPhase::ComputingDominators,
            started,
        );
        let dominator =
            build_dominator_tree_controlled(&graph, core_observer(&background_observer))
                .map_err(map_native_error)?;
        ensure_observer_not_cancelled(&background_observer)?;
        emit_indeterminate(&background_observer, OperationPhase::Committing, started);
        let manifest = default_snapshot_store()
            .save(&heap_path, &graph, &dominator)
            .map_err(map_native_error)?;
        Ok::<_, String>(manifest)
    })
    .await
    .map_err(|error| error.to_string())?;
    if result.is_ok() {
        if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
            if let Ok(manifest) = &result {
                let _ = default_snapshot_store().remove(&manifest.heap_sha256);
            }
            emit_indeterminate(&observer, OperationPhase::Cancelled, started);
            return Err(error);
        }
        emit_completed(&observer, OperationPhase::Complete, started);
    } else {
        emit_operation_error(
            &observer,
            result.as_ref().expect_err("checked error result"),
            started,
        );
    }
    result.map(|data| OperationEnvelope::new(context, data))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn remove_snapshot(key: String) -> Result<Value, String> {
    remove_snapshot_for_session(&default_snapshot_store(), &key).map_err(map_native_error)
}

/// Load a cached snapshot by SHA-256 store key into the live desktop session.
///
/// Registers an opaque `sourceId` → `manifest.heap_path` for later save/ci_check
/// and returns one display-safe graph/facts/mode/capability hydrate.
#[tauri::command(rename_all = "camelCase")]
pub async fn open_snapshot(
    key: String,
    context: Option<OperationContext>,
    app: AppHandle,
    state: State<'_, HeapSession>,
) -> Result<OperationEnvelope<SnapshotWorkspaceHydrate>, String> {
    let started = std::time::Instant::now();
    let context = require_operation_context(context)?;
    let (observer, registration) =
        registered_operation_observer(&app, Some(context.clone()), "snapshot", &state.operations)?;
    emit_completed(&observer, OperationPhase::Accepted, started);
    emit_indeterminate(&observer, OperationPhase::Opening, started);
    let background_observer = observer.clone();
    let (manifest, graph, dominator) = spawn_blocking(move || {
        ensure_observer_not_cancelled(&background_observer)?;
        let result =
            open_snapshot_for_session(&default_snapshot_store(), &key).map_err(map_native_error)?;
        ensure_observer_not_cancelled(&background_observer)?;
        Ok::<_, String>(result)
    })
    .await
    .map_err(|error| error.to_string())??;
    if let Err(error) = ensure_operation_can_commit(registration.as_ref()) {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(error);
    }

    let heap_path = manifest.heap_path.clone();
    let display_name = display_name_for_path(&heap_path);
    let source_id = Uuid::new_v4().to_string();
    let config = state
        .config
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone();
    let analysis = analyze_snapshot_from_graph_controlled(
        AnalyzeRequest {
            heap_path: display_name,
            config,
            leak_options: LeakDetectionOptions::default(),
            enable_ai: false,
            histogram_group_by: HistogramGroupBy::Class,
            enable_classloaders: true,
            enable_threads: false,
            enable_strings: false,
            enable_collections: false,
            enable_top_instances: true,
            enable_by_referrer: false,
            enable_duplicate_arrays: false,
            top_n: 25,
            min_collection_capacity: 16,
            min_duplicate_count: 2,
        },
        &graph,
        &dominator,
        core_observer(&observer),
    )
    .await
    .map_err(map_native_error)?;
    let hydrate = build_snapshot_workspace_hydrate(&manifest, &source_id, analysis.clone());

    emit_indeterminate(&observer, OperationPhase::Committing, started);
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut graph_slot = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    let mut dominator_slot = state
        .dominator
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut analysis_slot = state.analysis.write().map_err(|_| LOCK_ERROR.to_string())?;
    let mut field_data_slot = state
        .field_data_graph
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut heap_path_slot = state
        .heap_path
        .write()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let mut sources = state
        .selected_sources
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let registration = registration
        .as_ref()
        .ok_or_else(|| "Missing operation registration".to_string())?;
    let committed = registration.commit_if_current(|| {
        state.bump_session_epoch();
        replace_session_analysis(
            &mut graph_slot,
            &mut dominator_slot,
            Some((graph, dominator)),
        );
        *analysis_slot = Some(analysis);
        *field_data_slot = None;
        *heap_path_slot = Some(heap_path.clone());
        sources.insert(source_id, heap_path);
    });
    if committed.is_none() {
        emit_indeterminate(&observer, OperationPhase::Cancelled, started);
        return Err(structured_operation_cancelled_error());
    }
    emit_completed(&observer, OperationPhase::Complete, started);
    Ok(OperationEnvelope::new(context, hydrate))
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

fn require_loaded_analysis(
    state: &State<'_, HeapSession>,
) -> Result<
    (
        mnemosyne_core::hprof::ObjectGraph,
        mnemosyne_core::DominatorTree,
    ),
    String,
> {
    let _session = state
        .session_mutation
        .lock()
        .map_err(|_| LOCK_ERROR.to_string())?;
    let graph = state
        .graph
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())?;
    let dominator = state
        .dominator
        .read()
        .map_err(|_| LOCK_ERROR.to_string())?
        .clone()
        .ok_or_else(|| NO_HEAP_LOADED.to_string())?;
    Ok((graph, dominator))
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
mod report_export_tests {
    use super::render_desktop_report_export;
    use mnemosyne_core::{graph::GraphMetrics, hprof::HeapSummary};
    use mnemosyne_core::{AnalysisMode, AnalyzeResponse, ProvenanceKind, ProvenanceMarker};
    use std::time::{Duration, SystemTime};

    fn sample_response() -> AnalyzeResponse {
        AnalyzeResponse {
            mode: AnalysisMode::Deep,
            overview: None,
            summary: HeapSummary {
                heap_path: "private/source/path.hprof".into(),
                total_objects: 1,
                total_size_bytes: 16,
                classes: Vec::new(),
                generated_at: SystemTime::UNIX_EPOCH,
                header: None,
                total_records: 1,
                record_stats: Vec::new(),
            },
            leaks: Vec::new(),
            recommendations: Vec::new(),
            elapsed: Duration::from_millis(1),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            classloader_report: None,
            collection_report: None,
            string_report: None,
            array_report: None,
            top_instances: None,
            referrer_report: None,
            plugin_results: Vec::new(),
            provenance: vec![ProvenanceMarker::new(
                ProvenanceKind::Partial,
                "bounded analyzer output",
            )],
        }
    }

    #[test]
    fn report_export_uses_existing_html_escaping_and_preserves_labels() {
        let export =
            render_desktop_report_export(sample_response(), "evil<script>.hprof", "html").unwrap();
        let content = export["content"].as_str().unwrap();

        assert!(content.contains("evil&lt;script&gt;.hprof"));
        assert!(!content.contains("evil<script>.hprof"));
        assert_eq!(export["mode"], "deep");
        assert_eq!(export["provenance"][0]["kind"], "Partial");
        assert_eq!(export["provenance"][0]["detail"], "bounded analyzer output");
    }

    #[test]
    fn report_export_rejects_unknown_formats() {
        let error =
            render_desktop_report_export(sample_response(), "fixture.hprof", "custom:unsafe")
                .unwrap_err();
        assert!(error.contains("unsupported report export format"));
    }
}

#[cfg(test)]
mod pick_heap_file_result_tests {
    use super::{OperationEnvelope, PickHeapFileResult};
    use mnemosyne_desktop_session::OperationContext;

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

    #[test]
    fn operation_envelope_echoes_camel_case_identity_and_data() {
        let value = serde_json::to_value(OperationEnvelope::new(
            OperationContext {
                workspace_id: "workspace-1".to_string(),
                revision: 7,
                operation_id: "operation-9".to_string(),
            },
            serde_json::json!({ "ok": true }),
        ))
        .expect("serialize");

        assert_eq!(value["workspaceId"], "workspace-1");
        assert_eq!(value["revision"], 7);
        assert_eq!(value["operationId"], "operation-9");
        assert_eq!(value["data"]["ok"], true);
        assert!(value.get("context").is_none());
    }
}

pub const OPERATION_PROGRESS_EVENT: &str = "mnemosyne://operation-progress";

/// Bridges dependency-neutral core progress into correlated Tauri events.
pub struct TauriOperationObserver {
    app: AppHandle,
    context: OperationContext,
    kind: String,
    cancellation: Arc<AtomicBool>,
    coalescer: Mutex<OperationProgressCoalescer>,
}

impl TauriOperationObserver {
    pub fn new(
        app: AppHandle,
        context: OperationContext,
        kind: impl Into<String>,
        cancellation: Arc<AtomicBool>,
    ) -> Self {
        Self {
            app,
            context,
            kind: kind.into(),
            cancellation,
            coalescer: Mutex::new(OperationProgressCoalescer::default()),
        }
    }
}

impl OperationObserver for TauriOperationObserver {
    fn progress(&self, snapshot: OperationProgressSnapshot) {
        let event =
            OperationProgress::from_snapshot(self.context.clone(), self.kind.clone(), snapshot);
        let phase = event.phase;
        let elapsed_ms = event.elapsed_ms;
        let event = match self.coalescer.lock() {
            Ok(mut coalescer) => coalescer.coalesce(event),
            Err(_) => {
                tracing::warn!(
                    operation_id = %self.context.operation_id,
                    ?phase,
                    elapsed_ms,
                    error_code = "operation_progress_coalescer_unavailable",
                    "operation progress event dropped"
                );
                return;
            }
        };

        if let Some(event) = event {
            if self.app.emit(OPERATION_PROGRESS_EVENT, event).is_err() {
                tracing::warn!(
                    operation_id = %self.context.operation_id,
                    ?phase,
                    elapsed_ms,
                    error_code = "operation_progress_emit_failed",
                    "operation progress event dropped"
                );
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }
}
