//! Session-scoped heap operations shared by Tauri commands.
//!
//! Kept free of `tauri` dependencies so unit tests can run on headless CI
//! hosts without WebKit/GTK installed.

use std::path::{Path, PathBuf};

use mnemosyne_core::{
    analysis::{
        analyze_heap, focus_leaks, generate_ai_chat_turn_async, inspect_object, validate_leak_id,
        AiChatTurn, AnalyzeRequest, LeakDetectionOptions, ObjectInspection,
    },
    build_dominator_tree, build_histogram,
    diff::{
        object::types::{
            DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES, DEFAULT_OBJECT_DIFF_TOP_N,
            DEFAULT_RETAINED_CHANGE_THRESHOLD,
        },
        run_diff, DiffMode, DiffRequest, DiffResult, IdentityStrategy, ObjectDiffReport,
    },
    graph::find_all_gc_paths_in_graph,
    hprof::{parse_hprof_file_with_options, ObjectGraph, ParseOptions},
    mcp::session::{
        effective_history_limit, new_session_id, timestamp_now, top_leak_ids, trim_history_to,
        McpSessionStore, PersistedAiSession, SessionAnalysisSnapshot, SessionConversationSnapshot,
        DEFAULT_SESSION_HISTORY, HARD_MAX_SESSION_HISTORY, MCP_SESSION_VERSION,
    },
    snapshot::{SnapshotManifest, SnapshotStore},
    workflow::{self, WorkflowDescription, WorkflowKind, WorkflowState, WorkflowStore},
    resolve_live_instances_by_class, AllPathsRequest, AppConfig, DominatorTree, GcPathResult,
    HistogramGroupBy, HistogramResult,
};
use serde_json::{json, Value};

pub const DEFAULT_CLASS_INSTANCES_LIMIT: usize = 100;
pub const MAX_CLASS_INSTANCES_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInstanceEntry {
    pub object_id: String,
    pub class_name: String,
    pub shallow_size: u32,
    pub retained_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassInstancesPage {
    pub class_key: String,
    pub total: usize,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub truncated: bool,
    pub instances: Vec<ClassInstanceEntry>,
}

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

pub fn list_class_instances_for_session(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
    class_key: &str,
    offset: usize,
    limit: usize,
) -> Result<ClassInstancesPage, String> {
    let limit = limit.min(MAX_CLASS_INSTANCES_LIMIT);
    let mut matching_ids = resolve_live_instances_by_class(graph, class_key);
    matching_ids.sort_unstable_by(|left_id, right_id| {
        let left_shallow = graph
            .get_object(*left_id)
            .map(|object| object.shallow_size)
            .unwrap_or(0);
        let right_shallow = graph
            .get_object(*right_id)
            .map(|object| object.shallow_size)
            .unwrap_or(0);

        dominator
            .retained_size(*right_id)
            .cmp(&dominator.retained_size(*left_id))
            .then_with(|| right_shallow.cmp(&left_shallow))
            .then_with(|| left_id.cmp(right_id))
    });

    let total = matching_ids.len();
    let instances = matching_ids
        .into_iter()
        .skip(offset)
        .take(limit)
        .filter_map(|object_id| {
            graph.get_object(object_id).map(|object| ClassInstanceEntry {
                object_id: format!(
                    "0x{object_id:0width$X}",
                    width = usize::from(graph.identifier_size) * 2
                ),
                class_name: graph
                    .class_name(object.class_id)
                    .map(|name| name.replace('/', "."))
                    .unwrap_or_else(|| "<unknown>".to_string()),
                shallow_size: object.shallow_size,
                retained_size: dominator.retained_size(object_id),
            })
        })
        .collect::<Vec<_>>();
    let returned = instances.len();

    Ok(ClassInstancesPage {
        class_key: class_key.to_string(),
        total,
        returned,
        offset,
        limit,
        truncated: offset.saturating_add(returned) < total,
        instances,
    })
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

    if trimmed
        .chars()
        .any(|character| matches!(character, 'A'..='F' | 'a'..='f'))
    {
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
        Ok(DiffResult::Class(_)) => {
            Err("internal error: object diff returned class result".to_string())
        }
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

/// Read-only dump of a persisted workflow instance — mirrors MCP `get_workflow`,
/// but projects `heap_path` to a basename for desktop IPC (Terra path opacity).
pub fn get_workflow_for_session(store: &WorkflowStore, workflow_id: &str) -> Result<Value, String> {
    let state = store.load(workflow_id).map_err(|error| error.to_string())?;
    let mut value = serde_json::to_value(state).map_err(|error| error.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        if let Some(path) = obj.get("heap_path").and_then(|v| v.as_str()) {
            let basename = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("heap.dump")
                .to_string();
            obj.insert("heap_path".to_string(), Value::String(basename));
        }
    }
    Ok(value)
}

/// Delete a persisted workflow instance — mirrors MCP `close_workflow`.
pub fn close_workflow_for_session(
    store: &WorkflowStore,
    workflow_id: &str,
) -> Result<Value, String> {
    store
        .remove(workflow_id)
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "workflow_id": workflow_id,
        "closed": true,
    }))
}

/// `MNEMOSYNE_AI_SESSION_DIR` overrides the default AI session store root —
/// mirrors MCP `default_session_directory` / `[ai.sessions].directory`.
const AI_SESSION_DIR_ENV: &str = "MNEMOSYNE_AI_SESSION_DIR";

/// Default AI session store root for desktop — same layout as MCP sessions.
pub fn default_ai_session_store_root() -> PathBuf {
    if let Ok(dir) = std::env::var(AI_SESSION_DIR_ENV) {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(mut dir) = dirs::data_local_dir() {
        dir.push("mnemosyne");
        dir.push("ai-sessions");
        return dir;
    }

    let mut fallback = std::env::temp_dir();
    fallback.push("mnemosyne");
    fallback.push("ai-sessions");
    fallback
}

pub fn default_ai_session_store() -> McpSessionStore {
    McpSessionStore::new(default_ai_session_store_root())
}

/// Prefer `[ai.sessions].directory` when set; otherwise the desktop default root.
pub fn ai_session_store_for_config(config: &AppConfig) -> McpSessionStore {
    if let Some(dir) = config.ai.sessions.directory.as_ref() {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return McpSessionStore::new(PathBuf::from(trimmed));
        }
    }
    default_ai_session_store()
}

/// Input for the M23.C assistant bridge's `createAiSession` host method.
#[derive(Debug, Clone, Default)]
pub struct CreateAiSessionInput {
    pub heap_path: String,
}

fn display_name_for_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("heap.dump")
        .to_string()
}

/// Desktop-facing create/resume payload: basename only (path opacity).
fn opaque_ai_session_payload(session: &PersistedAiSession, include_history: bool) -> Value {
    let mut payload = json!({
        "session_id": session.session_id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "display_name": display_name_for_path(&session.heap_path),
        "leak_count": session.analysis.leaks.len(),
        "top_leaks": session.analysis.top_leaks,
        "focus_leak_id": session.conversation.focus_leak_id,
        "history_length": session.conversation.history.len(),
        "history_max_turns": DEFAULT_SESSION_HISTORY,
        "history_hard_max_turns": HARD_MAX_SESSION_HISTORY,
        // Explicit outbound metadata contract for the UI redaction notice.
        "outbound_metadata": {
            "sends": [
                "heap_summary_stats",
                "focused_leak_id_class_severity_description",
                "bounded_chat_history"
            ],
            "never_sends": ["api_keys", "absolute_heap_paths", "raw_heap_field_values"]
        },
    });
    if include_history {
        payload["history"] = json!(session.conversation.history);
    }
    payload
}

/// Compact get payload — mirrors MCP `get_ai_session` with path opacity.
fn opaque_compact_ai_session_payload(session: &PersistedAiSession) -> Value {
    json!({
        "session_id": session.session_id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "display_name": display_name_for_path(&session.heap_path),
        "leak_count": session.analysis.leaks.len(),
        "focus_leak_id": session.conversation.focus_leak_id,
        "history_length": session.conversation.history.len(),
        "history_max_turns": DEFAULT_SESSION_HISTORY,
        "history_hard_max_turns": HARD_MAX_SESSION_HISTORY,
    })
}

/// Chat response without `wire` (prompt/response bodies stay off the UI surface).
fn opaque_chat_response(ai: &mnemosyne_core::AiInsights) -> Value {
    json!({
        "summary": ai.summary,
        "model": ai.model,
        "recommendations": ai.recommendations,
        "confidence": ai.confidence,
    })
}

/// Create a persisted AI session — mirrors MCP `create_ai_session`.
pub async fn create_ai_session_for_session(
    store: &McpSessionStore,
    config: &AppConfig,
    input: CreateAiSessionInput,
) -> Result<Value, String> {
    if input.heap_path.trim().is_empty() {
        return Err("heap_path is required for create_ai_session".to_string());
    }

    let mut request_config = config.clone();
    request_config.ai.enabled = false;
    let leak_options = LeakDetectionOptions::from(&request_config.analysis);

    let analysis = analyze_heap(AnalyzeRequest {
        heap_path: input.heap_path.clone(),
        config: request_config,
        leak_options: leak_options.clone(),
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await
    .map_err(|error| error.to_string())?;

    let now = timestamp_now();
    let session = PersistedAiSession {
        session_version: MCP_SESSION_VERSION,
        session_id: new_session_id(),
        created_at: now.clone(),
        updated_at: now,
        heap_path: input.heap_path,
        analysis: SessionAnalysisSnapshot {
            min_severity: leak_options.min_severity,
            packages: leak_options.package_filters,
            leak_types: leak_options.leak_types,
            top_leaks: top_leak_ids(&analysis.leaks),
            summary: analysis.summary,
            leaks: analysis.leaks,
        },
        conversation: SessionConversationSnapshot {
            focus_leak_id: None,
            history: Vec::new(),
        },
    };

    store
        .save(&session)
        .map_err(|error| format!("session persist failed: {error}"))?;
    Ok(opaque_ai_session_payload(&session, false))
}

/// Touch + return a session — mirrors MCP `resume_ai_session`.
pub fn resume_ai_session_for_session(
    store: &McpSessionStore,
    session_id: &str,
) -> Result<Value, String> {
    let mut session = store.load(session_id).map_err(|error| error.to_string())?;
    session.updated_at = timestamp_now();
    store
        .save(&session)
        .map_err(|error| format!("session persist failed: {error}"))?;
    Ok(opaque_ai_session_payload(&session, true))
}

/// Read-only compact dump — mirrors MCP `get_ai_session`.
pub fn get_ai_session_for_session(
    store: &McpSessionStore,
    session_id: &str,
) -> Result<Value, String> {
    let session = store.load(session_id).map_err(|error| error.to_string())?;
    Ok(opaque_compact_ai_session_payload(&session))
}

/// Delete a persisted AI session — mirrors MCP `close_ai_session`.
pub fn close_ai_session_for_session(
    store: &McpSessionStore,
    session_id: &str,
) -> Result<Value, String> {
    store
        .delete(session_id)
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "session_id": session_id,
        "closed": true,
    }))
}

/// Bounded chat turn — mirrors MCP `chat_session` (12 default / 32 hard max).
pub async fn chat_session_for_session(
    store: &McpSessionStore,
    config: &AppConfig,
    session_id: &str,
    question: &str,
    focus_leak_id: Option<&str>,
) -> Result<Value, String> {
    if question.trim().is_empty() {
        return Err("question must not be empty".to_string());
    }

    let mut session = store.load(session_id).map_err(|error| error.to_string())?;

    if let Some(target) = focus_leak_id {
        validate_leak_id(&session.analysis.leaks, target).map_err(|error| error.to_string())?;
    }

    let active_focus = focus_leak_id.or(session.conversation.focus_leak_id.as_deref());
    let focused = if let Some(target) = active_focus {
        focus_leaks(&session.analysis.leaks, Some(target))
    } else {
        let shortlist = session.analysis.top_leaks.clone();
        session
            .analysis
            .leaks
            .iter()
            .filter(|leak| shortlist.iter().any(|id| id == &leak.id))
            .cloned()
            .collect()
    };

    let mut ai_config = config.ai.clone();
    ai_config.enabled = true;
    let ai = generate_ai_chat_turn_async(
        &session.analysis.summary,
        &focused,
        question,
        &session.conversation.history,
        active_focus,
        &ai_config,
    )
    .await
    .map_err(|error| error.to_string())?;

    if let Some(target) = focus_leak_id {
        session.conversation.focus_leak_id = Some(target.to_string());
    }
    trim_history_to(
        &mut session.conversation.history,
        AiChatTurn {
            question: question.to_string(),
            answer_summary: ai.summary.clone(),
        },
        effective_history_limit(ai_config.sessions.history_max_turns),
    );
    session.updated_at = timestamp_now();
    store
        .save(&session)
        .map_err(|error| format!("session persist failed: {error}"))?;

    Ok(opaque_chat_response(&ai))
}

pub fn list_snapshots_for_session(store: &SnapshotStore) -> Result<Vec<SnapshotManifest>, String> {
    store.list().map_err(|error| error.to_string())
}

/// Validates that `key` is a snapshot-store SHA-256 hash, not an arbitrary
/// filesystem path — shared by `remove_snapshot` / `open_snapshot` (MCP store-key scope).
pub fn validated_store_snapshot_key(key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("snapshot key must not be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(
            "snapshot store accepts a store key (SHA-256 hash) only, not a file path".to_string(),
        );
    }
    if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("snapshot key must be a 64-character SHA-256 hex hash".to_string());
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub fn save_snapshot_for_session(
    store: &SnapshotStore,
    heap_path: &str,
    retain_field_data: bool,
) -> Result<SnapshotManifest, String> {
    let graph = parse_hprof_file_with_options(heap_path, ParseOptions { retain_field_data })
        .map_err(|error| error.to_string())?;
    let dominator = build_dominator_tree(&graph);
    store
        .save(heap_path, &graph, &dominator)
        .map_err(|error| error.to_string())
}

pub fn remove_snapshot_for_session(store: &SnapshotStore, key: &str) -> Result<Value, String> {
    let store_key = validated_store_snapshot_key(key)?;
    store
        .remove(&store_key)
        .map_err(|error| error.to_string())?;
    Ok(json!({ "removed": true, "key": store_key }))
}

/// Load a cached snapshot by validated SHA-256 store key for desktop session install.
///
/// Returns the manifest plus the deserialized graph/dominator pair. Callers that
/// install into `HeapSession` typically keep the graph (and discard or rebuild
/// the dominator on demand), matching `load_heap` / `run_desktop_analysis`.
pub fn open_snapshot_for_session(
    store: &SnapshotStore,
    key: &str,
) -> Result<(SnapshotManifest, ObjectGraph, DominatorTree), String> {
    let store_key = validated_store_snapshot_key(key)?;
    let payload = store.load(&store_key).map_err(|error| error.to_string())?;
    Ok((
        payload.manifest,
        payload.object_graph,
        payload.dominator_tree,
    ))
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

    fn add_rooted_big_cache(
        graph: &mut mnemosyne_core::hprof::ObjectGraph,
        object_id: u64,
        shallow_size: u32,
        child: Option<(u64, u32)>,
    ) {
        let mut object = graph
            .get_object(0x1000)
            .expect("fixture BigCache object")
            .clone();
        object.id = object_id;
        object.shallow_size = shallow_size;
        object.references = child.iter().map(|(id, _)| *id).collect();
        graph.objects.insert(object_id, object);

        if let Some((child_id, child_size)) = child {
            let mut child_object = graph
                .get_object(0x2000)
                .expect("fixture Object child")
                .clone();
            child_object.id = child_id;
            child_object.shallow_size = child_size;
            child_object.references.clear();
            graph.objects.insert(child_id, child_object);
        }

        let mut root = graph.gc_roots[0].clone();
        root.object_id = object_id;
        graph.gc_roots.push(root);
    }

    #[test]
    fn list_class_instances_matches_dotted_names_and_orders_deterministically() {
        let mut graph = graph_fixture();
        add_rooted_big_cache(&mut graph, 0x3000, 16, None);
        add_rooted_big_cache(&mut graph, 0x4000, 8, Some((0x4100, 16)));
        add_rooted_big_cache(&mut graph, 0x5000, 8, Some((0x5100, 16)));
        add_rooted_big_cache(&mut graph, 0x6000, 12, Some((0x6100, 12)));
        let dominator = build_dominator_tree(&graph);

        let page = list_class_instances_for_session(
            &graph,
            &dominator,
            "com.example.BigCache",
            0,
            100,
        )
        .expect("dotted class name must resolve");

        assert_eq!(page.class_key, "com.example.BigCache");
        assert_eq!(page.total, 5);
        assert_eq!(page.returned, 5);
        assert_eq!(page.offset, 0);
        assert_eq!(page.limit, 100);
        assert!(!page.truncated);
        assert_eq!(
            page.instances
                .iter()
                .map(|instance| instance.object_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "0x00006000",
                "0x00004000",
                "0x00005000",
                "0x00003000",
                "0x00001000",
            ]
        );
        assert!(page
            .instances
            .iter()
            .all(|instance| instance.class_name == "com.example.BigCache"));
    }

    #[test]
    fn list_class_instances_caps_limit_and_reports_truncation_after_offset() {
        let mut graph = graph_fixture();
        for index in 0..205 {
            add_rooted_big_cache(&mut graph, 0x10000 + index, 1, None);
        }
        let dominator = build_dominator_tree(&graph);

        let page = list_class_instances_for_session(
            &graph,
            &dominator,
            "com.example.BigCache",
            3,
            usize::MAX,
        )
        .expect("bounded page must resolve");

        assert_eq!(page.total, 206);
        assert_eq!(page.offset, 3);
        assert_eq!(page.limit, 200);
        assert_eq!(page.returned, 200);
        assert_eq!(page.instances.len(), 200);
        assert!(page.truncated);
        assert!(page.offset + page.returned < page.total);
    }

    fn parse_hprof_file_with_options_from_bytes(
        bytes: &[u8],
        retain_field_data: bool,
    ) -> Result<mnemosyne_core::hprof::ObjectGraph, String> {
        let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, bytes).map_err(|error| error.to_string())?;
        parse_hprof_file_with_options(
            file.path().to_str().expect("temp path must be valid UTF-8"),
            ParseOptions { retain_field_data },
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
        assert!(
            stale_slot.is_some(),
            "epoch mismatch must not overwrite cache"
        );

        let mut path_slot = Some(graph_fixture());
        assert!(!install_field_data_cache_if_still_current(
            &capture,
            3,
            Some("/tmp/b.hprof"),
            &mut path_slot,
            graph,
        ));
        assert!(
            path_slot.is_some(),
            "heap path mismatch must not overwrite cache"
        );
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
        assert!(result
            .all_paths
            .as_ref()
            .is_some_and(|paths| !paths.is_empty()));
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
            SnapshotStore::new(tempfile::tempdir().expect("temp dir must exist").keep())
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
                Ok(DiffResult::Object(diff)) => {
                    diff.object_diff.expect("object diff must include a report")
                }
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
            assert_eq!(
                report.match_quality.strategy,
                IdentityStrategy::ClassRetained
            );
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

            for delta in report.added.iter().chain(report.retained_changed.iter()) {
                assert!(delta.leak_severity.is_none());
            }
        }
    }

    mod workflow_bridge {
        use super::*;
        use mnemosyne_core::{
            hprof::{
                parse_hprof_file_with_options, test_fixtures::build_graph_fixture, ParseOptions,
            },
            workflow::{WorkflowKind, WorkflowStore, WORKFLOW_SCHEMA_VERSION},
        };
        use std::io::Write;

        fn workflow_store() -> WorkflowStore {
            WorkflowStore::new(tempfile::tempdir().expect("temp dir must exist").keep())
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
            let names: Vec<&str> = description
                .steps
                .iter()
                .map(|step| step.name.as_str())
                .collect();
            assert_eq!(
                names,
                vec!["detect", "investigate_suspect", "explain", "propose_fix"]
            );
        }

        #[test]
        fn parse_workflow_kind_rejects_unknown_labels() {
            let error = parse_workflow_kind("not_a_real_kind").expect_err("invalid kind");
            assert!(error.contains("unknown workflow kind"));
        }

        #[test]
        fn list_snapshots_for_session_returns_saved_manifests() {
            let store =
                SnapshotStore::new(tempfile::tempdir().expect("temp dir must exist").keep());
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
        fn open_snapshot_for_session_loads_graph_by_sha256_key() {
            let store =
                SnapshotStore::new(tempfile::tempdir().expect("temp dir must exist").keep());
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let graph = parse_hprof_file_with_options(&heap_path, ParseOptions::default())
                .expect("fixture must parse");
            let dominator = build_dominator_tree(&graph);
            let saved = store
                .save(&heap_path, &graph, &dominator)
                .expect("snapshot must save");

            let (manifest, loaded_graph, _loaded_dominator) =
                open_snapshot_for_session(&store, &saved.heap_sha256).expect("open must succeed");

            assert_eq!(manifest.heap_sha256, saved.heap_sha256);
            assert_eq!(manifest.heap_path, heap_path);
            assert_eq!(loaded_graph.object_count(), graph.object_count());
            assert_eq!(loaded_graph.classes.len(), graph.classes.len());
            assert_eq!(manifest.object_count, graph.object_count());
        }

        #[test]
        fn open_snapshot_for_session_rejects_path_like_keys() {
            let store =
                SnapshotStore::new(tempfile::tempdir().expect("temp dir must exist").keep());
            let error = open_snapshot_for_session(&store, "/tmp/not-a-key.hprof")
                .expect_err("path key must fail");
            assert!(error.contains("SHA-256 hash") || error.contains("not a file path"));
        }

        #[test]
        fn open_snapshot_for_session_missing_key_returns_structured_error() {
            let store =
                SnapshotStore::new(tempfile::tempdir().expect("temp dir must exist").keep());
            let missing = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            let error = open_snapshot_for_session(&store, missing).expect_err("missing must fail");
            assert!(
                error.contains("snapshot_not_found") || error.contains("not found"),
                "unexpected error: {error}"
            );
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
            assert_eq!(
                response.get("current_step"),
                Some(&json!("investigate_suspect"))
            );
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
            assert!(
                !leaks.is_empty(),
                "fixture heap should surface at least one leak"
            );
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

            let step = next_step_for_session(&store, &workflow_id, json!({ "leak_id": leak_id }))
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

        /// Cross-loader duplicate shape (mirrors `core/tests/workflow_classloader_leak.rs`).
        fn build_classloader_duplicate_fixture() -> Vec<u8> {
            fn push_u32(buf: &mut Vec<u8>, value: u32) {
                buf.extend_from_slice(&value.to_be_bytes());
            }
            fn push_u64(buf: &mut Vec<u8>, value: u64) {
                buf.extend_from_slice(&value.to_be_bytes());
            }
            fn write_id(buf: &mut Vec<u8>, id: u64) {
                push_u32(buf, id as u32);
            }
            fn push_record(records: &mut Vec<Vec<u8>>, tag: u8, body: Vec<u8>) {
                let mut record = Vec::with_capacity(9 + body.len());
                record.push(tag);
                push_u32(&mut record, 0);
                push_u32(&mut record, body.len() as u32);
                record.extend_from_slice(&body);
                records.push(record);
            }

            let mut header = Vec::new();
            header.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
            push_u32(&mut header, 4);
            push_u64(&mut header, 0);

            let mut records = Vec::new();
            let mut add_string = |id: u64, value: &str| {
                let mut body = Vec::new();
                write_id(&mut body, id);
                body.extend_from_slice(value.as_bytes());
                push_record(&mut records, 0x01, body);
            };
            add_string(1, "java/lang/Object");
            add_string(2, "com/example/webapp/WebappLoader");
            add_string(3, "com/example/webapp/RequestHandler");

            let mut add_load_class = |serial: u32, class_obj_id: u64, name_id: u64| {
                let mut body = Vec::new();
                push_u32(&mut body, serial);
                write_id(&mut body, class_obj_id);
                push_u32(&mut body, 0);
                write_id(&mut body, name_id);
                push_record(&mut records, 0x02, body);
            };
            add_load_class(1, 0x100, 1);
            add_load_class(2, 0x200, 2);
            add_load_class(3, 0x300, 3);
            add_load_class(4, 0x301, 3);

            let mut heap = Vec::new();
            for (class_obj_id, super_id, loader_id, size) in [
                (0x100u64, 0u64, 0u64, 0u32),
                (0x200, 0x100, 0, 16),
                (0x300, 0x100, 0x1000, 8),
                (0x301, 0x100, 0x2000, 8),
            ] {
                heap.push(0x20);
                write_id(&mut heap, class_obj_id);
                push_u32(&mut heap, 0);
                write_id(&mut heap, super_id);
                write_id(&mut heap, loader_id);
                for _ in 0..4 {
                    write_id(&mut heap, 0);
                }
                push_u32(&mut heap, size);
                heap.extend_from_slice(&0u16.to_be_bytes());
                heap.extend_from_slice(&0u16.to_be_bytes());
                heap.extend_from_slice(&0u16.to_be_bytes());
            }
            for (obj_id, class_obj_id) in [
                (0x1000u64, 0x200u64),
                (0x2000, 0x200),
                (0x3000, 0x300),
                (0x3001, 0x301),
            ] {
                heap.push(0x21);
                write_id(&mut heap, obj_id);
                push_u32(&mut heap, 0);
                write_id(&mut heap, class_obj_id);
                push_u32(&mut heap, 0);
            }
            for obj_id in [0x1000u64, 0x2000, 0x3000, 0x3001] {
                heap.push(0x05);
                write_id(&mut heap, obj_id);
            }
            push_record(&mut records, 0x0C, heap);

            let mut bytes = header;
            for record in records {
                bytes.extend_from_slice(&record);
            }
            bytes
        }

        fn write_classloader_fixture_heap() -> Result<(tempfile::NamedTempFile, String), String> {
            let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
            file.write_all(&build_classloader_duplicate_fixture())
                .map_err(|error| error.to_string())?;
            let path = file
                .path()
                .to_str()
                .expect("temp path must be valid UTF-8")
                .to_string();
            Ok((file, path))
        }

        #[tokio::test]
        async fn get_and_close_workflow_round_trip_matches_mcp_semantics() {
            let store = workflow_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");

            let start = start_workflow_for_session(
                &store,
                StartWorkflowSessionInput {
                    kind: "tune_gc".into(),
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

            let loaded = get_workflow_for_session(&store, &workflow_id).expect("get must succeed");
            assert_eq!(loaded.get("workflow_id"), Some(&json!(workflow_id)));
            assert_eq!(
                loaded.get("current_step"),
                Some(&json!("thread_local_review"))
            );
            assert_eq!(loaded.get("kind"), Some(&json!("TUNE_GC")));
            let projected = loaded
                .get("heap_path")
                .and_then(Value::as_str)
                .expect("heap_path");
            assert!(
                !projected.contains('/') && !projected.contains('\\'),
                "desktop get_workflow must project basename only, got {projected}"
            );

            let closed =
                close_workflow_for_session(&store, &workflow_id).expect("close must succeed");
            assert_eq!(
                closed,
                json!({ "workflow_id": workflow_id, "closed": true })
            );

            let missing = get_workflow_for_session(&store, &workflow_id)
                .expect_err("closed workflow must be gone");
            assert!(
                missing.contains("workflow_not_found"),
                "unexpected error: {missing}"
            );
        }

        #[tokio::test]
        async fn classloader_leak_get_resume_advance_and_close() {
            let store = workflow_store();
            let (_heap_file, heap_path) = write_classloader_fixture_heap().expect("heap fixture");

            let start = start_workflow_for_session(
                &store,
                StartWorkflowSessionInput {
                    kind: "classloader_leak".into(),
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
            assert_eq!(start.get("current_step"), Some(&json!("select")));

            let resumed = get_workflow_for_session(&store, &workflow_id).expect("get must succeed");
            assert_eq!(resumed.get("current_step"), Some(&json!("select")));
            assert_eq!(resumed.get("kind"), Some(&json!("CLASSLOADER_LEAK")));

            let class_name = resumed
                .pointer("/step_history/0/output_summary/duplicate_class_names/0")
                .and_then(Value::as_str)
                .expect("detect should surface a duplicate class")
                .to_string();

            let advanced =
                next_step_for_session(&store, &workflow_id, json!({ "class_name": class_name }))
                    .await
                    .expect("resume advance must succeed");
            assert_eq!(
                advanced.get("current_step"),
                Some(&json!("inspect_retention"))
            );

            let after = get_workflow_for_session(&store, &workflow_id).expect("get after advance");
            assert_eq!(after.get("current_step"), Some(&json!("inspect_retention")));

            close_workflow_for_session(&store, &workflow_id).expect("close must succeed");
            let missing = get_workflow_for_session(&store, &workflow_id)
                .expect_err("closed workflow must be gone");
            assert!(missing.contains("workflow_not_found"));
        }

        #[tokio::test]
        async fn get_workflow_surfaces_already_complete_and_corrupt_states() {
            let store = workflow_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");

            let start = start_workflow_for_session(
                &store,
                StartWorkflowSessionInput {
                    kind: "tune_gc".into(),
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

            // Drive to complete via successive empty next_step calls.
            let mut current = start
                .get("current_step")
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            while current != "complete" {
                let step = next_step_for_session(&store, &workflow_id, Value::Null)
                    .await
                    .expect("advance toward complete");
                current = step
                    .get("current_step")
                    .and_then(Value::as_str)
                    .expect("current_step")
                    .to_string();
            }

            let completed = get_workflow_for_session(&store, &workflow_id).expect("get complete");
            assert_eq!(completed.get("current_step"), Some(&json!("complete")));

            let already = next_step_for_session(&store, &workflow_id, Value::Null)
                .await
                .expect_err("complete workflow must reject advance");
            assert!(
                already.contains("workflow_already_complete"),
                "unexpected error: {already}"
            );

            let corrupt_root = tempfile::tempdir().expect("temp dir must exist").keep();
            let corrupt_store = WorkflowStore::new(corrupt_root.clone());
            std::fs::create_dir_all(&corrupt_root).expect("create store root");
            std::fs::write(
                corrupt_root.join("wf-corrupt-test.json"),
                b"{ not valid json",
            )
            .expect("write corrupt payload");
            let corrupt = get_workflow_for_session(&corrupt_store, "wf-corrupt-test")
                .expect_err("corrupt payload must fail");
            assert!(
                corrupt.contains("workflow_corrupt"),
                "unexpected error: {corrupt}"
            );
        }
    }

    mod ai_session_bridge {
        use super::*;
        use mnemosyne_core::{
            config::AiMode,
            hprof::test_fixtures::build_graph_fixture,
            mcp::session::{McpSessionStore, DEFAULT_SESSION_HISTORY, HARD_MAX_SESSION_HISTORY},
            AppConfig,
        };
        use std::io::Write;

        fn session_store() -> McpSessionStore {
            McpSessionStore::new(tempfile::tempdir().expect("temp dir must exist").keep())
        }

        fn write_fixture_heap() -> Result<(tempfile::NamedTempFile, String), String> {
            let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
            file.write_all(&build_graph_fixture())
                .map_err(|error| error.to_string())?;
            let path = file.path().to_string_lossy().into_owned();
            Ok((file, path))
        }

        #[tokio::test]
        async fn create_chat_get_close_round_trip_keeps_path_opaque() {
            let store = session_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let absolute_marker = heap_path.clone();
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            config.ai.mode = AiMode::Rules;

            let created = create_ai_session_for_session(
                &store,
                &config,
                CreateAiSessionInput {
                    heap_path: heap_path.clone(),
                },
            )
            .await
            .expect("create must succeed");

            let session_id = created
                .get("session_id")
                .and_then(Value::as_str)
                .expect("session_id")
                .to_string();
            let payload = created.to_string();
            assert!(
                !payload.contains(&absolute_marker),
                "create payload must not leak absolute heap path"
            );
            assert!(created
                .get("display_name")
                .and_then(Value::as_str)
                .is_some());
            assert_eq!(
                created.get("history_max_turns"),
                Some(&json!(DEFAULT_SESSION_HISTORY))
            );
            assert_eq!(
                created.get("history_hard_max_turns"),
                Some(&json!(HARD_MAX_SESSION_HISTORY))
            );
            assert!(created.get("outbound_metadata").is_some());

            let chat = chat_session_for_session(
                &store,
                &config,
                &session_id,
                "What should I investigate first?",
                None,
            )
            .await
            .expect("chat must succeed");
            assert!(chat.get("summary").and_then(Value::as_str).is_some());
            assert!(chat.get("model").and_then(Value::as_str).is_some());
            assert!(chat.get("wire").is_none(), "wire body must stay off UI");

            let got = get_ai_session_for_session(&store, &session_id).expect("get must succeed");
            assert_eq!(got.get("history_length"), Some(&json!(1)));
            assert!(!got.to_string().contains(&absolute_marker));

            let closed =
                close_ai_session_for_session(&store, &session_id).expect("close must succeed");
            assert_eq!(closed, json!({ "session_id": session_id, "closed": true }));
            let missing = get_ai_session_for_session(&store, &session_id)
                .expect_err("closed session must be gone");
            assert!(
                missing.contains("session not found") || missing.contains("not found"),
                "unexpected error: {missing}"
            );
        }

        #[tokio::test]
        async fn chat_session_evicts_history_past_twelve_default() {
            let store = session_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            config.ai.mode = AiMode::Rules;

            let created =
                create_ai_session_for_session(&store, &config, CreateAiSessionInput { heap_path })
                    .await
                    .expect("create");
            let session_id = created
                .get("session_id")
                .and_then(Value::as_str)
                .expect("session_id")
                .to_string();

            for i in 0..(DEFAULT_SESSION_HISTORY + 1) {
                chat_session_for_session(&store, &config, &session_id, &format!("turn-{i}"), None)
                    .await
                    .expect("chat turn");
            }

            let resumed = resume_ai_session_for_session(&store, &session_id).expect("resume");
            let history = resumed
                .get("history")
                .and_then(Value::as_array)
                .expect("history");
            assert_eq!(history.len(), DEFAULT_SESSION_HISTORY);
            assert_eq!(
                history[0].get("question").and_then(Value::as_str),
                Some("turn-1")
            );
        }

        #[tokio::test]
        async fn chat_session_hard_caps_history_at_thirty_two() {
            let store = session_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            config.ai.mode = AiMode::Rules;
            config.ai.sessions.history_max_turns = Some(100);

            let created =
                create_ai_session_for_session(&store, &config, CreateAiSessionInput { heap_path })
                    .await
                    .expect("create");
            let session_id = created
                .get("session_id")
                .and_then(Value::as_str)
                .expect("session_id")
                .to_string();

            for i in 0..(HARD_MAX_SESSION_HISTORY + 3) {
                chat_session_for_session(&store, &config, &session_id, &format!("hard-{i}"), None)
                    .await
                    .expect("chat turn");
            }

            let resumed = resume_ai_session_for_session(&store, &session_id).expect("resume");
            let history = resumed
                .get("history")
                .and_then(Value::as_array)
                .expect("history");
            assert_eq!(history.len(), HARD_MAX_SESSION_HISTORY);
        }

        #[tokio::test]
        async fn chat_session_surfaces_provider_errors_for_recovery() {
            let store = session_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            config.ai.mode = AiMode::Provider;
            config.ai.api_key_env = Some("MNEMOSYNE_TEST_MISSING_AI_KEY".into());
            // Ensure the env var is unset so provider mode fails honestly.
            std::env::remove_var("MNEMOSYNE_TEST_MISSING_AI_KEY");

            let created =
                create_ai_session_for_session(&store, &config, CreateAiSessionInput { heap_path })
                    .await
                    .expect("create");
            let session_id = created
                .get("session_id")
                .and_then(Value::as_str)
                .expect("session_id")
                .to_string();

            let error = chat_session_for_session(
                &store,
                &config,
                &session_id,
                "Why is this retaining?",
                None,
            )
            .await
            .expect_err("missing API key must fail");
            assert!(
                error.to_lowercase().contains("api key")
                    || error.to_lowercase().contains("provider")
                    || error.to_lowercase().contains("missing"),
                "unexpected error: {error}"
            );
            assert!(
                !error.to_lowercase().contains("sk-"),
                "error must not print secret material"
            );
        }

        #[tokio::test]
        async fn chat_session_updates_focus_leak_id() {
            let store = session_store();
            let (_heap_file, heap_path) = write_fixture_heap().expect("heap fixture");
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            config.ai.mode = AiMode::Rules;

            let created =
                create_ai_session_for_session(&store, &config, CreateAiSessionInput { heap_path })
                    .await
                    .expect("create");
            let session_id = created
                .get("session_id")
                .and_then(Value::as_str)
                .expect("session_id")
                .to_string();
            let focus = created
                .get("top_leaks")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .map(str::to_string);

            // Fixture may have zero leaks; focus switch still must accept None.
            chat_session_for_session(
                &store,
                &config,
                &session_id,
                "Explain the focus",
                focus.as_deref(),
            )
            .await
            .expect("chat");

            let got = get_ai_session_for_session(&store, &session_id).expect("get");
            if let Some(id) = focus {
                assert_eq!(got.get("focus_leak_id"), Some(&json!(id)));
            }
        }

        #[test]
        fn ai_session_store_for_config_prefers_explicit_directory() {
            let dir = tempfile::tempdir().expect("temp");
            let mut config = AppConfig::default();
            config.ai.sessions.directory = Some(dir.path().display().to_string());
            let store = ai_session_store_for_config(&config);
            let session = PersistedAiSession {
                session_version: MCP_SESSION_VERSION,
                session_id: "mcp-test-store".into(),
                created_at: "1".into(),
                updated_at: "1".into(),
                heap_path: "fixture.hprof".into(),
                analysis: SessionAnalysisSnapshot {
                    min_severity: mnemosyne_core::analysis::LeakSeverity::Low,
                    packages: Vec::new(),
                    leak_types: Vec::new(),
                    top_leaks: Vec::new(),
                    summary: mnemosyne_core::HeapSummary {
                        heap_path: "fixture.hprof".into(),
                        total_objects: 0,
                        total_size_bytes: 0,
                        classes: Vec::new(),
                        generated_at: std::time::SystemTime::UNIX_EPOCH,
                        header: None,
                        total_records: 0,
                        record_stats: Vec::new(),
                    },
                    leaks: Vec::new(),
                },
                conversation: SessionConversationSnapshot {
                    focus_leak_id: None,
                    history: Vec::new(),
                },
            };
            store
                .save(&session)
                .expect("save into configured directory");
            assert!(dir.path().join("mcp-test-store.json").exists());
        }
    }
}
