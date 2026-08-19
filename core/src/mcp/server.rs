use crate::{
    analysis::{
        analyze_heap, analyze_heap_from_graph, detect_duplicate_classes, detect_leaks, focus_leaks,
        generate_ai_chat_turn_async, generate_ai_insights_async, validate_leak_id, AiChatTurn,
        AnalysisMode, AnalyzeRequest, LeakDetectionOptions, LeakKind, LeakSeverity,
    },
    config::AppConfig,
    diff::{DiffRequest, DiffResult},
    errors::{CoreError, CoreResult},
    fix::{propose_fix_for_leaks_with_config, propose_fix_with_config, FixRequest, FixStyle},
    graph::{
        find_all_gc_paths, find_all_gc_paths_in_graph, find_gc_path, find_gc_path_in_graph,
        AllPathsRequest, GcPathRequest,
    },
    hprof::{parse_heap, parse_hprof_overview_file, HeapParseJob, OverviewOptions},
    mapper::{map_to_code, MapToCodeRequest},
    mcp::session::{
        new_session_id, timestamp_now, top_leak_ids, McpSessionStore, PersistedAiSession,
        SessionAnalysisSnapshot, SessionConversationSnapshot, MCP_SESSION_VERSION,
    },
    query::{execute_query, parse_query},
    snapshot::{SnapshotPayload, SnapshotStore},
    workflow::{WorkflowKind, WorkflowState, WorkflowStore},
    HistogramGroupBy, ParseOptions,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

/// Configuration for the MCP server (currently informational).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerOptions {
    pub host: String,
    pub port: u16,
}

/// Start the MCP server loop, reading JSON lines from stdin and emitting
/// responses on stdout.
pub async fn serve(options: McpServerOptions, config: AppConfig) -> CoreResult<()> {
    info!(host = %options.host, port = options.port, "starting MCP server over stdio");

    let stdin = io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(packet) => {
                let id = packet.id.clone();
                match handle_request(packet, &config).await {
                    Ok(value) => RpcResponse::success(id, value),
                    Err(err) => RpcResponse::from_core_error(id, &err),
                }
            }
            Err(err) => RpcResponse::invalid_json(Value::Null, format!("invalid JSON: {err}")),
        };

        let serialized = serde_json::to_string(&response)?;
        stdout.write_all(serialized.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    info!("stdin closed; shutting down MCP server");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    id: Value,
    success: bool,
    result: Value,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_details: Option<RpcErrorDetails>,
}

#[derive(Debug, Serialize)]
struct RpcErrorDetails {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl RpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            id,
            success: true,
            result,
            error: None,
            error_details: None,
        }
    }

    fn invalid_json(id: Value, message: String) -> Self {
        Self::from_error_details(
            id,
            RpcErrorDetails {
                code: "invalid_json",
                message,
                details: None,
            },
        )
    }

    fn from_core_error(id: Value, error: &CoreError) -> Self {
        Self::from_error_details(id, RpcErrorDetails::from_core_error(error))
    }

    fn from_error_details(id: Value, error_details: RpcErrorDetails) -> Self {
        error!(code = error_details.code, message = %error_details.message, "MCP request failed");
        Self {
            id,
            success: false,
            result: Value::Null,
            error: Some(error_details.message.clone()),
            error_details: Some(error_details),
        }
    }
}

impl RpcErrorDetails {
    fn from_core_error(error: &CoreError) -> Self {
        let message = error.to_string();
        match error {
            CoreError::Io(source) => Self {
                code: "io_error",
                message,
                details: Some(json!({ "detail": source.to_string() })),
            },
            CoreError::InvalidInput(detail) if detail.starts_with("session not found:") => Self {
                code: "session_not_found",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::InvalidInput(detail) if detail.starts_with("session load failed:") => Self {
                code: "session_load_failed",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::FileNotFound { path, suggestion } => Self {
                code: "file_not_found",
                message,
                details: Some(json!({
                    "path": path,
                    "suggestion": suggestion,
                })),
            },
            CoreError::NotAnHprof { path, detail } => Self {
                code: "not_hprof",
                message,
                details: Some(json!({
                    "path": path,
                    "detail": detail,
                })),
            },
            CoreError::HprofParseError { phase, detail } => Self {
                code: "hprof_parse_error",
                message,
                details: Some(json!({
                    "phase": phase,
                    "detail": detail,
                })),
            },
            CoreError::ConfigError { detail, suggestion } => Self {
                code: "config_error",
                message,
                details: Some(json!({
                    "detail": detail,
                    "suggestion": suggestion,
                })),
            },
            CoreError::InvalidInput(detail) => Self {
                code: "invalid_input",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::AiProviderError { detail, status } => Self {
                code: "provider_error",
                message,
                details: Some(json!({
                    "detail": detail,
                    "status": status,
                })),
            },
            CoreError::AiProviderTimeout { detail } => Self {
                code: "provider_timeout",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::NotImplemented(detail) => Self {
                code: "not_implemented",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::FeatureUnavailableInOverviewMode { feature, hint } => Self {
                code: "feature_unavailable_in_overview_mode",
                message,
                details: Some(json!({
                    "feature": feature,
                    "hint": hint,
                })),
            },
            CoreError::Unsupported(detail)
                if detail.starts_with("feature_unavailable_in_overview_mode:") =>
            {
                diff_feature_error_details("feature_unavailable_in_overview_mode", detail)
            }
            CoreError::Unsupported(detail)
                if detail.starts_with("feature_unavailable_object_diff_too_large:") =>
            {
                diff_feature_error_details("feature_unavailable_object_diff_too_large", detail)
            }
            CoreError::Unsupported(detail)
                if detail.starts_with("feature_unavailable_without_field_data:") =>
            {
                diff_feature_error_details("feature_unavailable_without_field_data", detail)
            }
            CoreError::Unsupported(detail)
                if detail.starts_with("inspect_object_id_not_found:") =>
            {
                Self {
                    code: "object_id_not_found",
                    message,
                    details: Some(json!({ "detail": detail })),
                }
            }
            CoreError::Unsupported(detail) if detail.starts_with("snapshot_not_found:") => {
                snapshot_error_details(
                    "snapshot_not_found",
                    message,
                    detail,
                    "run `mnemosyne snapshot save <heap>` first, or `mnemosyne snapshot list`",
                )
            }
            CoreError::Unsupported(detail) if detail.starts_with("snapshot_schema_mismatch:") => {
                snapshot_error_details(
                    "snapshot_schema_mismatch",
                    message,
                    detail,
                    "run with --refresh, or `mnemosyne snapshot rm <hash>`",
                )
            }
            CoreError::Unsupported(detail) if detail.starts_with("snapshot_stale_source:") => {
                snapshot_error_details(
                    "snapshot_stale_source",
                    message,
                    detail,
                    "run with --refresh to re-parse and update the cache",
                )
            }
            CoreError::Unsupported(detail) if detail.starts_with("snapshot_corrupt:") => {
                snapshot_error_details(
                    "snapshot_corrupt",
                    message,
                    detail,
                    "run `mnemosyne snapshot rm <hash>` and re-run without --snapshot",
                )
            }
            // M11 Slice 11.D: `core::workflow`'s four structured error codes
            // (`workflow_not_found`/`workflow_corrupt`/
            // `workflow_step_input_mismatch`/`workflow_already_complete`, all
            // shaped as `CoreError::Unsupported("<code>: <detail>")` per this
            // module's established convention -- see
            // `core::workflow::workflow_not_found` and neighbors). There is
            // no `workflow_kind_not_implemented` arm: that placeholder error
            // was removed from `core::workflow` as of Slice 11.C once all
            // four `WorkflowKind` variants shipped (see that module's own
            // comment), so it can no longer be produced -- were it ever
            // reintroduced, it would still fall through to the generic
            // `CoreError::Unsupported => "unsupported"` arm below rather than
            // panicking.
            CoreError::Unsupported(detail) if detail.starts_with("workflow_not_found:") => Self {
                code: "workflow_not_found",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::Unsupported(detail) if detail.starts_with("workflow_corrupt:") => Self {
                code: "workflow_corrupt",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::Unsupported(detail)
                if detail.starts_with("workflow_step_input_mismatch:") =>
            {
                Self {
                    code: "workflow_step_input_mismatch",
                    message,
                    details: Some(json!({ "detail": detail })),
                }
            }
            CoreError::Unsupported(detail) if detail.starts_with("workflow_already_complete:") => {
                Self {
                    code: "workflow_already_complete",
                    message,
                    details: Some(json!({ "detail": detail })),
                }
            }
            CoreError::Unsupported(detail) if detail.contains("session_version") => Self {
                code: "session_version_unsupported",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::Unsupported(detail) => Self {
                code: "unsupported",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::SerdeJson(source) => Self {
                code: "invalid_params",
                message,
                details: Some(json!({ "detail": source.to_string() })),
            },
            CoreError::Other(source)
                if source.to_string().starts_with("session persist failed:") =>
            {
                Self {
                    code: "session_persist_failed",
                    message,
                    details: Some(json!({ "detail": source.to_string() })),
                }
            }
            CoreError::Other(source) => Self {
                code: "internal_error",
                message,
                details: Some(json!({ "detail": source.to_string() })),
            },
        }
    }
}

/// M9 Slice 9.D: build the `RpcErrorDetails` envelope for one of
/// `core::snapshot`'s four structured error codes
/// (`snapshot_not_found`/`snapshot_schema_mismatch`/`snapshot_stale_source`/
/// `snapshot_corrupt`, all shaped as `CoreError::Unsupported("<code>:
/// <detail>")` per that module's established convention -- see
/// `core::snapshot::snapshot_error`).
///
/// The design doc's §6.1 error envelope shows three flat top-level keys
/// (`error`, `detail`, `hint`). This codebase's actual MCP envelope instead
/// nests everything but `code`/`message` under `details` (see
/// `CoreError::FileNotFound`'s `{"path":..,"suggestion":..}` shape above),
/// so `hint` is carried the same way here rather than introducing a new
/// top-level shape other error codes don't have.
fn snapshot_error_details(
    code: &'static str,
    message: String,
    detail: &str,
    hint: &'static str,
) -> RpcErrorDetails {
    RpcErrorDetails {
        code,
        message,
        details: Some(json!({ "detail": detail, "hint": hint })),
    }
}

/// M8 Slice 8.C: build the `inspect_object_id_not_found` error, reused for
/// both an unparseable `object_id` and an `object_id` that parses but is
/// not present in the graph — same "object id not found" semantic
/// `gc-path --object-id` already established, distinguished by a separate
/// MCP error code (`object_id_not_found`) per the design doc's §8 error
/// envelope table.
fn inspect_object_id_not_found(object_id: &str, heap_path: &str) -> CoreError {
    CoreError::Unsupported(format!(
        "inspect_object_id_not_found: object id '{object_id}' was not found in heap dump '{heap_path}'"
    ))
}

/// Parse an MCP-supplied object id (`0x...` hex or bare decimal), mirroring
/// the parsing convention `core::graph::gc_path` uses internally.
fn parse_inspect_object_id(input: &str) -> Option<u64> {
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
    if trimmed.chars().any(|c| matches!(c, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed, 16).ok();
    }
    trimmed.parse::<u64>().ok()
}

fn diff_feature_error_details(code: &'static str, raw_detail: &str) -> RpcErrorDetails {
    let message_and_details = raw_detail
        .strip_prefix(code)
        .and_then(|detail| detail.strip_prefix(':'))
        .map(str::trim)
        .unwrap_or(raw_detail);
    let (message, details) = split_message_and_kv_details(message_and_details);

    let details = if details.is_empty() {
        Some(json!({ "detail": message }))
    } else {
        let mut object = serde_json::Map::new();
        object.insert("detail".into(), json!(message));
        for (key, value) in details {
            object.insert(key, value);
        }
        Some(Value::Object(object))
    };

    RpcErrorDetails {
        code,
        message,
        details,
    }
}

fn split_message_and_kv_details(
    message_and_details: &str,
) -> (String, serde_json::Map<String, Value>) {
    if let Some(details_start) = message_and_details.rfind(" (") {
        if message_and_details.ends_with(')') {
            let message = message_and_details[..details_start].trim().to_owned();
            let details_raw =
                &message_and_details[details_start + 2..message_and_details.len() - 1];
            return (message, parse_kv_details(details_raw));
        }
    }

    (message_and_details.to_owned(), serde_json::Map::new())
}

fn parse_kv_details(details_raw: &str) -> serde_json::Map<String, Value> {
    let mut details = serde_json::Map::new();

    for pair in details_raw.split(", ") {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };

        let value = if let Ok(parsed) = value.parse::<u64>() {
            json!(parsed)
        } else {
            json!(value)
        };
        details.insert(key.to_owned(), value);
    }

    details
}

#[derive(Debug, Deserialize)]
struct ParseHeapParams {
    path: String,
    #[serde(default)]
    mode: AnalysisMode,
    #[serde(default)]
    include_strings: bool,
    #[serde(default)]
    max_objects: Option<u64>,
    /// M9 Slice 9.D: SHA-256 hash or direct snapshot file path. When
    /// present (and `mode` does not resolve to `overview`, which never
    /// builds an object graph -- see the module-level snapshot doc note),
    /// returns a manifest-derived summary instead of parsing `path`.
    #[serde(default)]
    snapshot: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DetectLeakParams {
    heap_path: String,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
    #[serde(default)]
    leak_types: Option<Vec<LeakKind>>,
}

#[derive(Debug, Deserialize)]
struct MapToCodeParams {
    leak_id: String,
    #[serde(default)]
    class: Option<String>,
    project_root: PathBuf,
    #[serde(default = "MapToCodeParams::default_include_git")]
    include_git_info: bool,
}

#[derive(Debug, Deserialize)]
struct FindGcPathParams {
    heap_path: String,
    #[serde(default)]
    object_id: Option<String>,
    #[serde(default)]
    max_depth: Option<u32>,
    /// M8 Slice 8.A: return every enumerated path instead of only the
    /// shortest. Additive param on the existing `find_gc_path` tool.
    #[serde(default)]
    all_paths: bool,
    /// M8 Slice 8.A: find all-paths for every live instance of this class
    /// instead of a single `object_id`. Mutually exclusive with
    /// `object_id`.
    #[serde(default)]
    by_class: Option<String>,
    /// M8 Slice 8.A: shared path-enumeration budget across the whole
    /// `all_paths`/`by_class` query. Defaults to
    /// `AllPathsRequest::DEFAULT_MAX_PATHS` (20) when omitted.
    #[serde(default)]
    max_paths: Option<usize>,
    /// M9 Slice 9.D: SHA-256 hash or direct snapshot file path. When
    /// present, traces against the cached graph instead of parsing
    /// `heap_path`.
    #[serde(default)]
    snapshot: Option<String>,
}

/// M8 Slice 8.C: params for the `inspect_object` tool.
#[derive(Debug, Deserialize)]
struct InspectObjectParams {
    heap_path: String,
    object_id: String,
    #[serde(default)]
    retain_field_data: bool,
    /// M9 Slice 9.D: SHA-256 hash or direct snapshot file path. When
    /// present, inspects against the cached graph instead of parsing
    /// `heap_path`.
    #[serde(default)]
    snapshot: Option<String>,
}

/// M13 Slice 13.C: standalone cross-loader duplicate-class detection --
/// mirrors why `diff_heaps` exists as its own tool rather than folding into
/// `analyze_heap`: a focused, cheaper single-purpose call for a caller that
/// already knows it wants exactly this signal.
#[derive(Debug, Deserialize)]
struct DetectClassloaderLeaksParams {
    heap_path: String,
}

/// M11 Slice 11.D: params for the `describe_workflow` tool.
#[derive(Debug, Deserialize)]
struct DescribeWorkflowParams {
    kind: String,
}

/// M11 Slice 11.D: params for the `start_workflow` tool. Every field beyond
/// `kind` is optional at the wire level -- which ones are actually required
/// depends on `kind` (design doc §7): `heap_path` for
/// triage_memory_leak/tune_gc/traverse_object_graph, `object_id`
/// additionally for traverse_object_graph, and the four before_*/after_*
/// fields exclusively for compare_snapshots (whose own `heap_path` argument
/// to `crate::workflow::start` is a caller-invisible placeholder -- see
/// `core::workflow::compare_snapshots`'s module doc comment).
#[derive(Debug, Deserialize)]
struct StartWorkflowParams {
    kind: String,
    #[serde(default)]
    heap_path: Option<String>,
    #[serde(default)]
    object_id: Option<String>,
    #[serde(default)]
    before_heap_path: Option<String>,
    #[serde(default)]
    after_heap_path: Option<String>,
    #[serde(default)]
    before_snapshot_key: Option<String>,
    #[serde(default)]
    after_snapshot_key: Option<String>,
}

/// M11 Slice 11.D: params for the `next_step` tool.
#[derive(Debug, Deserialize)]
struct NextStepParams {
    workflow_id: String,
    #[serde(default)]
    step_input: Value,
}

/// M11 Slice 11.D: params shared by `get_workflow` and `close_workflow`.
#[derive(Debug, Deserialize)]
struct WorkflowIdParams {
    workflow_id: String,
}

#[derive(Debug, Deserialize)]
struct ExplainLeakParams {
    #[serde(default)]
    heap_path: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    leak_id: Option<String>,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
}

#[derive(Debug, Deserialize)]
struct ChatSessionParams {
    session_id: String,
    question: String,
    #[serde(default)]
    focus_leak_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct AnalyzeHeapParams {
    heap_path: String,
    #[serde(default)]
    mode: AnalysisMode,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    leak_types: Vec<LeakKind>,
    #[serde(default)]
    histogram_group_by: Option<HistogramGroupBy>,
    #[serde(default)]
    enable_ai: bool,
    #[serde(default)]
    enable_classloaders: bool,
    #[serde(default)]
    enable_threads: bool,
    #[serde(default)]
    enable_strings: bool,
    #[serde(default)]
    enable_collections: bool,
    #[serde(default)]
    enable_top_instances: bool,
    #[serde(default)]
    by_referrer: bool,
    #[serde(default)]
    top_n: Option<usize>,
    #[serde(default)]
    min_collection_capacity: Option<usize>,
    #[serde(default)]
    min_duplicate_count: Option<usize>,
    /// M9 Slice 9.D: SHA-256 hash or direct snapshot file path. When
    /// present (and `mode` does not resolve to `overview`, which never
    /// builds an object graph -- there is nothing to snapshot in that mode,
    /// same as CLI's `analyze --snapshot` precedent), analyzes against the
    /// cached graph via `analyze_heap_from_graph` instead of parsing
    /// `heap_path`.
    #[serde(default)]
    snapshot: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct CreateAiSessionParams {
    heap_path: String,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    leak_types: Vec<LeakKind>,
}

#[derive(Debug, Deserialize)]
struct SessionIdParams {
    session_id: String,
}

/// M9 Slice 9.D: params for the `open_snapshot` tool.
#[derive(Debug, Deserialize)]
struct OpenSnapshotParams {
    key: String,
}

#[derive(Debug, Deserialize)]
struct QueryHeapParams {
    heap_path: String,
    query: String,
    /// M9 Slice 9.D: SHA-256 hash or direct snapshot file path. When
    /// present, queries against the cached graph instead of parsing
    /// `heap_path`.
    #[serde(default)]
    snapshot: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum McpDiffMode {
    #[default]
    Class,
    Object,
}

impl From<McpDiffMode> for crate::diff::DiffMode {
    fn from(value: McpDiffMode) -> Self {
        match value {
            McpDiffMode::Class => crate::diff::DiffMode::Class,
            McpDiffMode::Object => crate::diff::DiffMode::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
enum McpIdentityStrategy {
    #[serde(rename = "class+retained")]
    ClassRetained,
    #[default]
    #[serde(rename = "class+dominator")]
    ClassDominator,
    #[serde(rename = "full-fingerprint")]
    FullFingerprint,
}

impl From<McpIdentityStrategy> for crate::diff::IdentityStrategy {
    fn from(value: McpIdentityStrategy) -> Self {
        match value {
            McpIdentityStrategy::ClassRetained => crate::diff::IdentityStrategy::ClassRetained,
            McpIdentityStrategy::ClassDominator => crate::diff::IdentityStrategy::ClassDominator,
            McpIdentityStrategy::FullFingerprint => crate::diff::IdentityStrategy::FullFingerprint,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DiffHeapsParams {
    before: String,
    after: String,
    #[serde(default)]
    mode: McpDiffMode,
    #[serde(default)]
    identity_strategy: McpIdentityStrategy,
    #[serde(default = "default_diff_retained_bucket_bits")]
    retained_bucket_bits: u8,
    #[serde(default = "default_diff_retained_change_threshold")]
    retained_change_threshold: u64,
    #[serde(default = "default_diff_top_n", alias = "top")]
    top_n: usize,
    #[serde(default = "default_diff_object_min_retained")]
    object_diff_min_retained: u64,
    #[serde(default)]
    retain_field_data: bool,
}

#[derive(Debug, Deserialize)]
struct ProposeFixParams {
    #[serde(default)]
    heap_path: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    leak_id: Option<String>,
    #[serde(default)]
    project_root: Option<PathBuf>,
    #[serde(default = "default_fix_style")]
    style: FixStyle,
}

fn default_fix_style() -> FixStyle {
    FixStyle::Minimal
}

fn default_diff_retained_bucket_bits() -> u8 {
    10
}

fn default_diff_retained_change_threshold() -> u64 {
    crate::diff::object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD
}

fn default_diff_top_n() -> usize {
    crate::diff::object::types::DEFAULT_OBJECT_DIFF_TOP_N
}

fn default_diff_object_min_retained() -> u64 {
    crate::diff::object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES
}

fn session_store(config: &AppConfig) -> McpSessionStore {
    let root = config
        .ai
        .sessions
        .directory
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(default_session_directory);
    McpSessionStore::new(root)
}

fn default_session_directory() -> PathBuf {
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

/// `MNEMOSYNE_SNAPSHOT_DIR` overrides the default snapshot cache root (M9
/// Slice 9.D). Mirrors `cli/src/main.rs`'s Slice 9.C `SNAPSHOT_DIR_ENV`
/// constant of the same name exactly -- the CLI and MCP surfaces each own
/// their own directory-resolution glue already (compare
/// `default_session_directory` below, which has no CLI counterpart at all),
/// so this is a deliberate mirror, not a shared function, and MUST use the
/// same env var name/precedence as the CLI so `--snapshot`/`snapshot save`
/// and the MCP `snapshot` param address the same on-disk cache by default.
const SNAPSHOT_DIR_ENV: &str = "MNEMOSYNE_SNAPSHOT_DIR";

/// Default snapshot cache root: `dirs::cache_dir()/mnemosyne`, overridable
/// via `MNEMOSYNE_SNAPSHOT_DIR`. Falls back to a temp-dir-based path on
/// platforms where `dirs::cache_dir()` returns `None`. Mirrors
/// `cli/src/main.rs`'s `default_snapshot_dir()` (Slice 9.C) exactly --
/// there is no `[snapshot]` config-key override in this codebase today (the
/// design doc's §4 point 7 mentions one, but Slice 9.C shipped only the env
/// override, so this function follows that as the actual established
/// pattern rather than the doc's aspirational one).
fn default_snapshot_dir() -> std::path::PathBuf {
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

fn snapshot_store() -> SnapshotStore {
    SnapshotStore::new(default_snapshot_dir())
}

/// `MNEMOSYNE_WORKFLOW_DIR` overrides the default on-disk root for persisted
/// `WorkflowState` (M11 Slice 11.D). Mirrors `SNAPSHOT_DIR_ENV`/
/// `default_snapshot_dir()` above exactly -- same env-var-then-
/// `dirs::cache_dir()`-then-temp-dir fallback shape. The design doc's §4
/// point 3 mentions a speculative `[workflow].directory` config-key override,
/// but per that same doc's own note, Slice 9.C/9.D shipped only the env-var
/// pattern for snapshots (no config key ever landed), so this follows the
/// actually-established precedent rather than the doc's aspirational one. A
/// dedicated `workflows` subdirectory (on both the `cache_dir()` and
/// temp-dir fallback branches) keeps persisted workflow state out of the
/// snapshot cache's own root, which `default_snapshot_dir()` above claims
/// directly.
const WORKFLOW_DIR_ENV: &str = "MNEMOSYNE_WORKFLOW_DIR";

fn default_workflow_dir() -> PathBuf {
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

fn workflow_store() -> WorkflowStore {
    WorkflowStore::new(default_workflow_dir())
}

/// Parses the lowercase snake_case wire form of a workflow kind (design doc
/// §7 -- "One of: triage_memory_leak, tune_gc, traverse_object_graph,
/// compare_snapshots"), i.e. `WorkflowKind::as_str()`'s own convention.
/// Deliberately NOT `WorkflowKind`'s own `Deserialize` impl, which uses
/// `SCREAMING_SNAKE_CASE` for `WorkflowState`'s on-disk persistence format
/// (`#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`, `core/src/workflow/mod.rs`)
/// -- the two casings serve different contracts (wire input vs. persisted
/// state) and are not interchangeable.
fn parse_workflow_kind(kind: &str) -> CoreResult<WorkflowKind> {
    match kind {
        "triage_memory_leak" => Ok(WorkflowKind::TriageMemoryLeak),
        "tune_gc" => Ok(WorkflowKind::TuneGc),
        "traverse_object_graph" => Ok(WorkflowKind::TraverseObjectGraph),
        "compare_snapshots" => Ok(WorkflowKind::CompareSnapshots),
        other => Err(CoreError::InvalidInput(format!(
            "unknown workflow kind '{other}': expected one of triage_memory_leak, tune_gc, \
             traverse_object_graph, compare_snapshots"
        ))),
    }
}

/// Builds the `{ workflow_id, current_step, step_result, next_expected_input
/// }` envelope design doc §7 specifies for both `start_workflow` and
/// `next_step`'s responses. `step_result` is the step that was just executed
/// -- the most recently appended `WorkflowState::step_history` entry's own
/// output. `next_expected_input` looks up the *next* step's
/// `expected_input` schema via `core::workflow::describe`, or `[]` once
/// `current_step` has reached `"complete"`.
fn workflow_step_response(state: &WorkflowState) -> CoreResult<Value> {
    let step_result = state
        .step_history
        .last()
        .map(|record| record.output_summary.clone())
        .unwrap_or(Value::Null);

    let next_expected_input = if state.current_step == "complete" {
        json!([])
    } else {
        let description = crate::workflow::describe(state.kind)?;
        description
            .steps
            .iter()
            .find(|step| step.name == state.current_step)
            .map(|step| serde_json::to_value(&step.expected_input))
            .transpose()?
            .unwrap_or_else(|| json!([]))
    };

    Ok(json!({
        "workflow_id": state.workflow_id,
        "current_step": state.current_step,
        "step_result": step_result,
        "next_expected_input": next_expected_input,
    }))
}

fn persist_session(store: &McpSessionStore, session: &PersistedAiSession) -> CoreResult<()> {
    store
        .save(session)
        .map_err(|err| CoreError::Other(anyhow!("session persist failed: {err}")))
}

fn delete_session(store: &McpSessionStore, session_id: &str) -> CoreResult<()> {
    store.delete(session_id).map_err(|err| match err {
        CoreError::InvalidInput(_) | CoreError::Unsupported(_) => err,
        other => CoreError::Other(anyhow!("session persist failed: {other}")),
    })
}

fn session_payload(session: &PersistedAiSession) -> Value {
    json!({
        "session_id": session.session_id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "heap_path": session.heap_path,
        "summary": session.analysis.summary,
        "leak_count": session.analysis.leaks.len(),
        "top_leaks": session.analysis.top_leaks,
        "focus_leak_id": session.conversation.focus_leak_id,
    })
}

fn resumed_session_payload(session: &PersistedAiSession) -> Value {
    json!({
        "session_id": session.session_id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "heap_path": session.heap_path,
        "summary": session.analysis.summary,
        "leak_count": session.analysis.leaks.len(),
        "top_leaks": session.analysis.top_leaks,
        "focus_leak_id": session.conversation.focus_leak_id,
        "history": session.conversation.history,
    })
}

fn compact_session_payload(session: &PersistedAiSession) -> Value {
    json!({
        "session_id": session.session_id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "heap_path": session.heap_path,
        "leak_count": session.analysis.leaks.len(),
        "focus_leak_id": session.conversation.focus_leak_id,
        "history_length": session.conversation.history.len(),
    })
}

fn session_resolved_leaks<'a>(
    session: &'a PersistedAiSession,
    explicit_leak_id: Option<&'a str>,
) -> CoreResult<Vec<crate::analysis::LeakInsight>> {
    let resolved = explicit_leak_id.or(session.conversation.focus_leak_id.as_deref());
    if let Some(target) = resolved {
        validate_leak_id(&session.analysis.leaks, target)?;
    }
    Ok(focus_leaks(&session.analysis.leaks, resolved))
}

impl MapToCodeParams {
    fn default_include_git() -> bool {
        true
    }
}

const MCP_MODE_PARAM_DESCRIPTION: &str =
    "Analysis mode. `auto` resolves by file size, `deep` builds the full object graph, and `overview` streams the heap without building the object graph and reports approximate shallow sizes only.";

const MCP_OVERVIEW_TOOL_NOTE: &str =
    "Overview mode is streaming, builds no object graph, and reports approximate shallow sizes only.";

fn analysis_mode_param() -> Value {
    json!({
        "name": "mode",
        "type": "string",
        "required": false,
        "default": "auto",
        "enum": ["auto", "deep", "overview"],
        "description": MCP_MODE_PARAM_DESCRIPTION,
    })
}

/// M9 Slice 9.D: shared `snapshot` param description for every existing
/// tool that gained the additive param (`parse_heap`, `analyze_heap`,
/// `find_gc_path`, `inspect_object`, `query_heap`), mirroring how
/// `analysis_mode_param()` shares one description across tools.
const MCP_SNAPSHOT_PARAM_DESCRIPTION: &str = "SHA-256 hash or direct snapshot file path (see `open_snapshot`/`list_snapshots` and `mnemosyne snapshot save`). When set, uses the cached object graph instead of re-parsing heap_path/path -- an invalid, stale, or schema-mismatched key returns a structured snapshot_not_found/snapshot_stale_source/snapshot_schema_mismatch/snapshot_corrupt error rather than silently falling back to a fresh parse.";

fn snapshot_param() -> Value {
    json!({
        "name": "snapshot",
        "type": "string",
        "required": false,
        "description": MCP_SNAPSHOT_PARAM_DESCRIPTION,
    })
}

fn resolve_heap_mode(heap_path: &str, requested_mode: AnalysisMode) -> CoreResult<AnalysisMode> {
    match requested_mode {
        AnalysisMode::Auto => {
            let input_size_bytes = std::fs::metadata(heap_path)?.len();
            Ok(AnalysisMode::Auto.resolve(input_size_bytes))
        }
        mode => Ok(mode),
    }
}

fn overview_options(top_n: Option<usize>) -> OverviewOptions {
    if let Some(top_n) = top_n {
        OverviewOptions {
            top_n_classes: top_n,
            top_n_instances: top_n,
            ..OverviewOptions::default()
        }
    } else {
        OverviewOptions::default()
    }
}

fn serialize_overview_summary(summary: crate::hprof::OverviewSummary) -> CoreResult<Value> {
    let mut value = serde_json::to_value(summary)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("mode".into(), json!("overview"));
    }
    Ok(value)
}

/// M9 Slice 9.D: `parse_heap`'s `snapshot` param cannot return a real
/// `HeapSummary` -- this builds the smaller, clearly-distinguished shape
/// the design doc's step 4 guidance calls for instead.
///
/// `HeapSummary::header`/`total_records`/`record_stats`/`classes` are all
/// derived from a streaming byte-level scan of the HPROF file's *raw
/// records* (`core::hprof::parser::scan_hprof_records` +
/// `summarize_class_stats`) -- a completely different computation path from
/// the `ObjectGraph`/`DominatorTree` a snapshot caches (§6.2 of the design
/// doc: only those two are cached, deliberately). Reconstructing the real
/// `HeapSummary` shape from a snapshot would require either (a) re-reading
/// the source HPROF file anyway, defeating the entire point of
/// `--snapshot`, or (b) reimplementing class-stat aggregation against
/// `ObjectGraph.objects`/`classes` on a different sizing basis (shallow
/// object size vs. raw record byte length) -- which would silently drift
/// from `parse_heap`'s real numbers while *looking* like the same trusted
/// shape. Per this codebase's existing honesty-first pattern
/// (`ProvenanceMarker`/`ProvenanceKind`, already used to flag synthetic/
/// partial data elsewhere), the snapshot response is instead a distinctly
/// shaped object: manifest fields plus a `total_shallow_size_bytes` total
/// that *is* honestly computed from the loaded `ObjectGraph`, carrying a
/// `ProvenanceKind::Partial` marker so a caller cannot mistake this for the
/// real by-file `HeapSummary`.
fn serialize_snapshot_summary(payload: SnapshotPayload) -> CoreResult<Value> {
    let total_shallow_size_bytes: u64 = payload
        .object_graph
        .objects
        .values()
        .map(|obj| u64::from(obj.shallow_size))
        .sum();

    let provenance = vec![crate::analysis::ProvenanceMarker::new(
        crate::analysis::ProvenanceKind::Partial,
        "derived from a cached snapshot's manifest + object graph, not a fresh HPROF \
         record-tag scan; header/total_records/record_stats/classes are unavailable \
         without re-parsing the source file",
    )];

    Ok(json!({
        "source": "snapshot",
        "heap_path": payload.manifest.heap_path,
        "heap_sha256": payload.manifest.heap_sha256,
        "object_count": payload.manifest.object_count,
        "total_shallow_size_bytes": total_shallow_size_bytes,
        "has_field_data": payload.manifest.has_field_data,
        "schema_version": payload.manifest.schema_version,
        "created_at": payload.manifest.created_at,
        "mnemosyne_version": payload.manifest.mnemosyne_version,
        "provenance": provenance,
    }))
}

fn tool_catalog() -> Value {
    json!({
        "tools": [
            {
                "name": "list_tools",
                "description": "List the live MCP tools and their parameter shapes.",
                "params": []
            },
            {
                "name": "parse_heap",
                "description": format!(
                    "Parse an HPROF file and return a lightweight heap summary. {MCP_OVERVIEW_TOOL_NOTE}"
                ),
                "params": [
                    { "name": "path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    analysis_mode_param(),
                    { "name": "include_strings", "type": "boolean", "required": false, "description": "Accept string extraction in the request, although the summary remains lightweight." },
                    { "name": "max_objects", "type": "number", "required": false, "description": "Optional object cap that falls back to parser.max_objects." },
                    snapshot_param()
                ]
            },
            {
                "name": "detect_leaks",
                "description": "Run leak detection with optional severity and package filters.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "package", "type": "string", "required": false, "description": "Single package prefix filter for this MCP method." },
                    { "name": "min_severity", "type": "string", "required": false, "description": "Minimum leak severity (LOW, MEDIUM, HIGH, CRITICAL)." },
                    { "name": "leak_types", "type": "array<string>", "required": false, "description": "Optional leak-kind filter list." }
                ]
            },
            {
                "name": "analyze_heap",
                "description": format!(
                    "Run the full analysis pipeline and return the serialized analysis response. {MCP_OVERVIEW_TOOL_NOTE}"
                ),
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    analysis_mode_param(),
                    { "name": "min_severity", "type": "string", "required": false, "description": "Optional minimum leak severity override." },
                    { "name": "packages", "type": "array<string>", "required": false, "description": "Optional package prefix filters." },
                    { "name": "leak_types", "type": "array<string>", "required": false, "description": "Optional leak-kind filter list." },
                    { "name": "histogram_group_by", "type": "string", "required": false, "description": "Histogram grouping: class, package, or class_loader." },
                    { "name": "enable_ai", "type": "boolean", "required": false, "description": "Enable AI insights for the analysis response." },
                    { "name": "enable_classloaders", "type": "boolean", "required": false, "description": "Attach classloader analysis." },
                    { "name": "enable_threads", "type": "boolean", "required": false, "description": "Attach thread analysis." },
                    { "name": "enable_strings", "type": "boolean", "required": false, "description": "Attach string analysis." },
                    { "name": "enable_collections", "type": "boolean", "required": false, "description": "Attach collection analysis." },
                    { "name": "enable_top_instances", "type": "boolean", "required": false, "description": "Attach the top-instances report." },
                    { "name": "by_referrer", "type": "boolean", "required": false, "description": "Attach the group-by-referrer report (objects ranked by incoming reference count)." },
                    { "name": "top_n", "type": "number", "required": false, "description": "Result count used by top-N analysis sections." },
                    { "name": "min_collection_capacity", "type": "number", "required": false, "description": "Minimum collection capacity to report." },
                    { "name": "min_duplicate_count", "type": "number", "required": false, "description": "Minimum duplicate string count to report." },
                    snapshot_param()
                ]
            },
            {
                "name": "diff_heaps",
                "description": "Compute class-level and (optionally) object-level diff between two HPROF heap dumps.",
                "params": [
                    { "name": "before", "type": "string", "required": true, "description": "Path to the BEFORE heap dump." },
                    { "name": "after", "type": "string", "required": true, "description": "Path to the AFTER heap dump." },
                    { "name": "mode", "type": "string", "required": false, "default": "class", "enum": ["class", "object"], "description": "'class' (default) or 'object'." },
                    { "name": "identity_strategy", "type": "string", "required": false, "default": "class+dominator", "enum": ["class+retained", "class+dominator", "full-fingerprint"], "description": "'class+retained' | 'class+dominator' (default) | 'full-fingerprint'. Ignored when mode='class'." },
                    { "name": "retained_bucket_bits", "type": "number", "required": false, "default": 10, "description": "Power-of-two bucket exponent for retained sizes; default 10 (1 KB)." },
                    { "name": "retained_change_threshold", "type": "number", "required": false, "default": 1048576, "description": "Minimum |retained delta| in bytes for inclusion in retained_changed; default 1048576." },
                    { "name": "top_n", "type": "number", "required": false, "default": 50, "description": "Per-section result cap; default 50." },
                    { "name": "object_diff_min_retained", "type": "number", "required": false, "default": 4096, "description": "Skip objects whose retained size is below this floor; default 4096." },
                    { "name": "retain_field_data", "type": "boolean", "required": false, "default": false, "description": "Required when identity_strategy='full-fingerprint'." }
                ],
                "output_schema": "HeapDiff (existing) extended with optional object_diff: ObjectDiffReport"
            },
            {
                "name": "query_heap",
                "description": "Execute an OQL-style query against the heap graph, including built-in fields plus retained instance fields when available.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "query", "type": "string", "required": true, "description": "Query text." },
                    snapshot_param()
                ]
            },
            {
                "name": "map_to_code",
                "description": "Map a leak candidate to likely source files under a project root.",
                "params": [
                    { "name": "leak_id", "type": "string", "required": true, "description": "Leak identifier from analyze or detect_leaks." },
                    { "name": "class", "type": "string", "required": false, "description": "Optional class-name override to bias source mapping." },
                    { "name": "project_root", "type": "string", "required": true, "description": "Project directory to scan for source files." },
                    { "name": "include_git_info", "type": "boolean", "required": false, "description": "Include git blame metadata when available." }
                ]
            },
            {
                "name": "find_gc_path",
                "description": "Find a path from an object to a GC root. Set all_paths or by_class to enumerate every path (bounded by max_paths) instead of only the shortest.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "object_id", "type": "string", "required": false, "description": "Target object identifier, for example 0x1000. Required unless by_class is set; mutually exclusive with by_class." },
                    { "name": "max_depth", "type": "number", "required": false, "description": "Optional traversal depth cap." },
                    { "name": "all_paths", "type": "boolean", "required": false, "description": "Return all paths (bounded by max_paths) instead of only the shortest." },
                    { "name": "by_class", "type": "string", "required": false, "description": "Find all-paths for every live instance of this class instead of a single object_id." },
                    { "name": "max_paths", "type": "number", "required": false, "description": "Shared path-enumeration budget across the whole all_paths/by_class query. Default 20." },
                    snapshot_param()
                ]
            },
            {
                "name": "inspect_object",
                "description": "Field-level inspection of a single heap object: values, refs in/out, dominator context.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "object_id", "type": "string", "required": true, "description": "Target object identifier, for example 0x1000." },
                    { "name": "retain_field_data", "type": "boolean", "required": false, "description": "Populate the fields section with typed instance field values." },
                    snapshot_param()
                ],
                "output_schema": "ObjectInspection"
            },
            {
                "name": "create_ai_session",
                "description": "Analyze a heap once and persist an AI follow-up session.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "min_severity", "type": "string", "required": false, "description": "Optional minimum leak severity override." },
                    { "name": "packages", "type": "array<string>", "required": false, "description": "Optional package prefix filters." },
                    { "name": "leak_types", "type": "array<string>", "required": false, "description": "Optional leak-kind filter list." }
                ]
            },
            {
                "name": "resume_ai_session",
                "description": "Resume a persisted AI follow-up session by session_id.",
                "params": [
                    { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
                ]
            },
            {
                "name": "get_ai_session",
                "description": "Inspect compact metadata for a persisted AI session.",
                "params": [
                    { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
                ]
            },
            {
                "name": "close_ai_session",
                "description": "Delete a persisted AI follow-up session.",
                "params": [
                    { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
                ]
            },
            {
                "name": "chat_session",
                "description": "Ask a follow-up AI question against a persisted session.",
                "params": [
                    { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." },
                    { "name": "question", "type": "string", "required": true, "description": "Follow-up question to ask." },
                    { "name": "focus_leak_id", "type": "string", "required": false, "description": "Optional leak identifier to focus the turn." }
                ]
            },
            {
                "name": "explain_leak",
                "description": "Generate AI-backed leak explanations from a heap path or a persisted AI session.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": false, "description": "Path to the heap dump. Exactly one of heap_path or session_id is required." },
                    { "name": "session_id", "type": "string", "required": false, "description": "Persisted AI session identifier. Exactly one of heap_path or session_id is required." },
                    { "name": "leak_id", "type": "string", "required": false, "description": "Optional leak identifier to focus the explanation." },
                    { "name": "min_severity", "type": "string", "required": false, "description": "Optional minimum leak severity override for the heap_path flow only." }
                ]
            },
            {
                "name": "propose_fix",
                "description": "Generate AI-backed fix suggestions from a heap path or a persisted AI session when provider mode and source context are available, otherwise fall back to heuristic guidance.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": false, "description": "Path to the heap dump. Exactly one of heap_path or session_id is required." },
                    { "name": "session_id", "type": "string", "required": false, "description": "Persisted AI session identifier. Exactly one of heap_path or session_id is required." },
                    { "name": "leak_id", "type": "string", "required": false, "description": "Optional leak identifier to narrow the suggestion set." },
                    { "name": "project_root", "type": "string", "required": false, "description": "Optional project directory used for file targeting." },
                    { "name": "style", "type": "string", "required": false, "description": "Patch style: Minimal, Defensive, or Comprehensive." }
                ]
            },
            {
                "name": "open_snapshot",
                "description": "Load a previously saved snapshot cache entry and return its manifest (does not itself run any analysis -- pairs with the snapshot param on analyze_heap/parse_heap/find_gc_path/inspect_object/query_heap).",
                "params": [
                    { "name": "key", "type": "string", "required": true, "description": "SHA-256 hash or direct snapshot file path." }
                ],
                "output_schema": "SnapshotManifest"
            },
            {
                "name": "list_snapshots",
                "description": "List the manifests of every snapshot currently in the cache.",
                "params": [],
                "output_schema": "Vec<SnapshotManifest>"
            },
            {
                "name": "detect_classloader_leaks",
                "description": "Cross-loader duplicate-class detection -- the classic Tomcat/Jetty/Spring hot-redeploy leak pattern.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." }
                ],
                "output_schema": "Vec<DuplicateClassGroup>"
            },
            {
                "name": "describe_workflow",
                "description": "Introspect a workflow kind's fixed step sequence and each step's expected input/underlying primitives, without creating any workflow state.",
                "params": [
                    { "name": "kind", "type": "string", "required": true, "description": "One of: triage_memory_leak, tune_gc, traverse_object_graph, compare_snapshots." }
                ],
                "output_schema": "WorkflowDescription"
            },
            {
                "name": "start_workflow",
                "description": "Create a new workflow instance of the given kind, run its first step, and persist the resulting state.",
                "params": [
                    { "name": "kind", "type": "string", "required": true, "description": "One of: triage_memory_leak, tune_gc, traverse_object_graph, compare_snapshots." },
                    { "name": "heap_path", "type": "string", "required": false, "description": "Required for triage_memory_leak/tune_gc/traverse_object_graph. Not used by compare_snapshots (see params below)." },
                    { "name": "object_id", "type": "string", "required": false, "description": "traverse_object_graph only: the starting object." },
                    { "name": "before_heap_path", "type": "string", "required": false, "description": "compare_snapshots only." },
                    { "name": "after_heap_path", "type": "string", "required": false, "description": "compare_snapshots only." },
                    { "name": "before_snapshot_key", "type": "string", "required": false, "description": "compare_snapshots only, alternative to before_heap_path." },
                    { "name": "after_snapshot_key", "type": "string", "required": false, "description": "compare_snapshots only, alternative to after_heap_path." }
                ],
                "output_schema": "{ workflow_id: string, current_step: string, step_result: object, next_expected_input: object }"
            },
            {
                "name": "next_step",
                "description": "Advance an in-flight workflow by executing whatever step current_step names, using step_input as that step's parameters.",
                "params": [
                    { "name": "workflow_id", "type": "string", "required": true, "description": "Workflow instance identifier returned by start_workflow." },
                    { "name": "step_input", "type": "object", "required": false, "description": "Shape depends on the current step -- see describe_workflow." }
                ],
                "output_schema": "same shape as start_workflow's output, or { current_step: \"complete\", ... } when done"
            },
            {
                "name": "get_workflow",
                "description": "Read-only dump of a workflow instance's full persisted state and step history.",
                "params": [
                    { "name": "workflow_id", "type": "string", "required": true, "description": "Workflow instance identifier returned by start_workflow." }
                ],
                "output_schema": "WorkflowState"
            },
            {
                "name": "close_workflow",
                "description": "Delete a persisted workflow instance's state.",
                "params": [
                    { "name": "workflow_id", "type": "string", "required": true, "description": "Workflow instance identifier returned by start_workflow." }
                ],
                "output_schema": "{ closed: true }"
            }
        ]
    })
}

async fn handle_request(packet: RpcRequest, config: &AppConfig) -> CoreResult<Value> {
    match packet.method.as_str() {
        "list_tools" => Ok(tool_catalog()),
        "open_snapshot" => {
            let params: OpenSnapshotParams = serde_json::from_value(packet.params)?;
            let store = snapshot_store();
            let payload = store.load(&params.key)?;
            Ok(serde_json::to_value(payload.manifest)?)
        }
        "list_snapshots" => {
            let store = snapshot_store();
            let manifests = store.list()?;
            Ok(serde_json::to_value(manifests)?)
        }
        "parse_heap" => {
            let params: ParseHeapParams = serde_json::from_value(packet.params)?;
            let resolved_mode = resolve_heap_mode(&params.path, params.mode)?;
            if resolved_mode == AnalysisMode::Overview {
                let summary = parse_hprof_overview_file(&params.path, &OverviewOptions::default())?;
                return serialize_overview_summary(summary);
            }

            if let Some(key) = &params.snapshot {
                let store = snapshot_store();
                let payload = store.load_checked(key, &params.path)?;
                return serialize_snapshot_summary(payload);
            }

            let job = HeapParseJob {
                path: params.path,
                include_strings: params.include_strings,
                max_objects: params.max_objects.or(config.parser.max_objects),
            };
            let summary = parse_heap(&job)?;
            Ok(serde_json::to_value(summary)?)
        }
        "detect_leaks" => {
            let params: DetectLeakParams = serde_json::from_value(packet.params)?;
            let mut options = LeakDetectionOptions::from(&config.analysis);
            if let Some(sev) = params.min_severity {
                options.min_severity = sev;
            }
            if let Some(package) = params.package {
                options.package_filters = vec![package];
            }
            if let Some(leak_types) = params.leak_types {
                options.leak_types = leak_types;
            }
            let leaks = detect_leaks(&params.heap_path, options).await?;
            Ok(serde_json::to_value(leaks)?)
        }
        "create_ai_session" => {
            let params: CreateAiSessionParams = serde_json::from_value(packet.params)?;
            let mut request_config = config.clone();
            if !params.packages.is_empty() {
                request_config.analysis.packages = params.packages.clone();
            }
            if !params.leak_types.is_empty() {
                request_config.analysis.leak_types = params.leak_types.clone();
            }
            request_config.ai.enabled = false;

            let mut leak_options = LeakDetectionOptions::from(&request_config.analysis);
            if let Some(sev) = params.min_severity {
                leak_options.min_severity = sev;
            }

            let analysis = analyze_heap(AnalyzeRequest {
                heap_path: params.heap_path.clone(),
                config: request_config,
                leak_options: leak_options.clone(),
                enable_ai: false,
                histogram_group_by: HistogramGroupBy::Class,
                ..AnalyzeRequest::default()
            })
            .await?;

            let now = timestamp_now();
            let session = PersistedAiSession {
                session_version: MCP_SESSION_VERSION,
                session_id: new_session_id(),
                created_at: now.clone(),
                updated_at: now,
                heap_path: params.heap_path,
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

            let store = session_store(config);
            persist_session(&store, &session)?;
            Ok(session_payload(&session))
        }
        "resume_ai_session" => {
            let params: SessionIdParams = serde_json::from_value(packet.params)?;
            let store = session_store(config);
            let mut session = store.load(&params.session_id)?;
            session.updated_at = timestamp_now();
            persist_session(&store, &session)?;
            Ok(resumed_session_payload(&session))
        }
        "get_ai_session" => {
            let params: SessionIdParams = serde_json::from_value(packet.params)?;
            let store = session_store(config);
            let session = store.load(&params.session_id)?;
            Ok(compact_session_payload(&session))
        }
        "close_ai_session" => {
            let params: SessionIdParams = serde_json::from_value(packet.params)?;
            let store = session_store(config);
            delete_session(&store, &params.session_id)?;
            Ok(json!({ "session_id": params.session_id, "closed": true }))
        }
        "chat_session" => {
            let params: ChatSessionParams = serde_json::from_value(packet.params)?;
            let store = session_store(config);
            let mut session = store.load(&params.session_id)?;

            if let Some(ref target) = params.focus_leak_id {
                validate_leak_id(&session.analysis.leaks, target)?;
            }

            let active_focus = params
                .focus_leak_id
                .as_deref()
                .or(session.conversation.focus_leak_id.as_deref());
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
                &params.question,
                &session.conversation.history,
                active_focus,
                &ai_config,
            )
            .await?;

            if let Some(target) = params.focus_leak_id {
                session.conversation.focus_leak_id = Some(target);
            }
            crate::mcp::session::trim_history(
                &mut session.conversation.history,
                AiChatTurn {
                    question: params.question,
                    answer_summary: ai.summary.clone(),
                },
            );
            session.updated_at = timestamp_now();
            persist_session(&store, &session)?;
            Ok(serde_json::to_value(ai)?)
        }
        "analyze_heap" => {
            let params: AnalyzeHeapParams = serde_json::from_value(packet.params)?;
            let resolved_mode = resolve_heap_mode(&params.heap_path, params.mode)?;
            if resolved_mode == AnalysisMode::Overview {
                let summary =
                    parse_hprof_overview_file(&params.heap_path, &overview_options(params.top_n))?;
                return serialize_overview_summary(summary);
            }

            let mut request_config = config.clone();
            if !params.packages.is_empty() {
                request_config.analysis.packages = params.packages.clone();
            }
            if !params.leak_types.is_empty() {
                request_config.analysis.leak_types = params.leak_types.clone();
            }
            request_config.ai.enabled = params.enable_ai;

            let mut leak_options = LeakDetectionOptions::from(&request_config.analysis);
            if let Some(sev) = params.min_severity {
                leak_options.min_severity = sev;
            }

            let heap_path = params.heap_path.clone();
            let request = AnalyzeRequest {
                heap_path: params.heap_path,
                config: request_config,
                leak_options,
                enable_ai: params.enable_ai,
                histogram_group_by: params.histogram_group_by.unwrap_or(HistogramGroupBy::Class),
                enable_classloaders: params.enable_classloaders,
                enable_threads: params.enable_threads,
                enable_strings: params.enable_strings,
                enable_collections: params.enable_collections,
                enable_top_instances: params.enable_top_instances,
                enable_by_referrer: params.by_referrer,
                top_n: params.top_n.unwrap_or(AnalyzeRequest::default().top_n),
                min_collection_capacity: params
                    .min_collection_capacity
                    .unwrap_or(AnalyzeRequest::default().min_collection_capacity),
                min_duplicate_count: params
                    .min_duplicate_count
                    .unwrap_or(AnalyzeRequest::default().min_duplicate_count),
            };

            // M9 Slice 9.D: mirrors `cli/src/main.rs`'s `resolve_analyze_response`
            // (Slice 9.C) -- an explicit `snapshot` key loads exactly that
            // cache entry via `load_checked` (loud on mismatch/staleness/
            // corruption/missing, never a silent parse fallback) and feeds it
            // into `analyze_heap_from_graph` instead of a fresh binary parse.
            let analysis = if let Some(key) = &params.snapshot {
                let store = snapshot_store();
                let payload = store.load_checked(key, &heap_path)?;
                analyze_heap_from_graph(request, &payload.object_graph, &payload.dominator_tree)
                    .await?
            } else {
                analyze_heap(request).await?
            };

            Ok(serde_json::to_value(analysis)?)
        }
        "diff_heaps" => {
            let params: DiffHeapsParams = serde_json::from_value(packet.params)?;
            let diff = match crate::diff::run_diff(DiffRequest {
                before_path: params.before,
                after_path: params.after,
                mode: params.mode.into(),
                identity_strategy: params.identity_strategy.into(),
                retained_bucket_bits: params.retained_bucket_bits,
                min_retained_bytes: params.object_diff_min_retained,
                retained_change_threshold: params.retained_change_threshold,
                top_n: params.top_n,
                retain_field_data: params.retain_field_data,
                // M10-B's --cross-reference-leaks / DiffRequest.cross_reference_leaks
                // is CLI-first (design doc §3 "Out"); MCP wiring is deferred, so this
                // handler always leaves it false pending a future slice.
                cross_reference_leaks: false,
            })
            .await?
            {
                DiffResult::Class(diff) | DiffResult::Object(diff) => diff,
            };

            Ok(serde_json::to_value(diff)?)
        }
        "query_heap" => {
            let params: QueryHeapParams = serde_json::from_value(packet.params)?;

            // M9 Slice 9.D: same snapshot-backed shape as `inspect_object`
            // above. Same field-data caveat: a cached snapshot saved without
            // `retain_field_data` will not retroactively gain field values.
            let (graph, dominator) = if let Some(key) = &params.snapshot {
                let store = snapshot_store();
                let payload = store.load_checked(key, &params.heap_path)?;
                (payload.object_graph, payload.dominator_tree)
            } else {
                let graph = crate::hprof::parse_hprof_file_with_options(
                    &params.heap_path,
                    ParseOptions {
                        retain_field_data: true,
                    },
                )?;
                let dominator = crate::graph::build_dominator_tree(&graph);
                (graph, dominator)
            };
            let query = parse_query(&params.query)
                .map_err(|err| CoreError::InvalidInput(err.to_string()))?;
            let result =
                execute_query(&query, &graph, Some(&dominator)).map_err(CoreError::from)?;
            Ok(serde_json::to_value(result)?)
        }
        "map_to_code" => {
            let params: MapToCodeParams = serde_json::from_value(packet.params)?;
            let response = map_to_code(&MapToCodeRequest {
                leak_id: params.leak_id,
                class_name: params.class,
                project_root: params.project_root,
                include_git_info: params.include_git_info,
            })?;
            Ok(serde_json::to_value(response)?)
        }
        "find_gc_path" => {
            let params: FindGcPathParams = serde_json::from_value(packet.params)?;

            // M9 Slice 9.D: mirrors the other snapshot-backed handlers --
            // an explicit `snapshot` key loads the cached graph via
            // `load_checked` and traces against it directly via the
            // graph-based `find_gc_path_in_graph`/`find_all_gc_paths_in_graph`
            // (core/src/graph/gc_path.rs) instead of `find_gc_path`/
            // `find_all_gc_paths`, which always parse `heap_path` from disk.
            if let Some(key) = &params.snapshot {
                let store = snapshot_store();
                let payload = store.load_checked(key, &params.heap_path)?;

                if params.all_paths || params.by_class.is_some() {
                    let request = AllPathsRequest {
                        heap_path: params.heap_path,
                        object_id: params.object_id,
                        by_class: params.by_class,
                        max_paths: params
                            .max_paths
                            .unwrap_or(AllPathsRequest::DEFAULT_MAX_PATHS),
                        max_depth: params.max_depth,
                    };
                    let response = find_all_gc_paths_in_graph(&payload.object_graph, &request)?;
                    return Ok(serde_json::to_value(response)?);
                }

                let object_id = params.object_id.ok_or_else(|| {
                    CoreError::InvalidInput(
                        "object_id is required unless all_paths or by_class is set".into(),
                    )
                })?;
                let response = find_gc_path_in_graph(
                    &payload.object_graph,
                    &params.heap_path,
                    &object_id,
                    params.max_depth,
                )?;
                return Ok(serde_json::to_value(response)?);
            }

            if params.all_paths || params.by_class.is_some() {
                let response = find_all_gc_paths(&AllPathsRequest {
                    heap_path: params.heap_path,
                    object_id: params.object_id,
                    by_class: params.by_class,
                    max_paths: params
                        .max_paths
                        .unwrap_or(AllPathsRequest::DEFAULT_MAX_PATHS),
                    max_depth: params.max_depth,
                })?;
                Ok(serde_json::to_value(response)?)
            } else {
                let object_id = params.object_id.ok_or_else(|| {
                    CoreError::InvalidInput(
                        "object_id is required unless all_paths or by_class is set".into(),
                    )
                })?;
                let response = find_gc_path(&GcPathRequest {
                    heap_path: params.heap_path,
                    object_id,
                    max_depth: params.max_depth,
                })?;
                Ok(serde_json::to_value(response)?)
            }
        }
        "inspect_object" => {
            let params: InspectObjectParams = serde_json::from_value(packet.params)?;

            // M9 Slice 9.D: an explicit `snapshot` key loads the cached
            // `(ObjectGraph, DominatorTree)` pair via `load_checked` instead
            // of parsing `heap_path`. Note: the cached graph's field data
            // reflects whatever `retain_field_data` was in effect when the
            // snapshot was *saved* -- an explicit snapshot key loads exactly
            // what's there (same "explicit is loud, not silently different"
            // precedent `SnapshotStore::load_checked`'s doc comment
            // establishes), it does not force a re-parse just because this
            // request's `retain_field_data` differs.
            let (graph, dominator) = if let Some(key) = &params.snapshot {
                let store = snapshot_store();
                let payload = store.load_checked(key, &params.heap_path)?;
                (payload.object_graph, payload.dominator_tree)
            } else {
                let graph = crate::hprof::parse_hprof_file_with_options(
                    &params.heap_path,
                    ParseOptions {
                        retain_field_data: params.retain_field_data,
                    },
                )?;
                let dominator = crate::graph::build_dominator_tree(&graph);
                (graph, dominator)
            };
            let target_id = parse_inspect_object_id(&params.object_id)
                .ok_or_else(|| inspect_object_id_not_found(&params.object_id, &params.heap_path))?;
            let inspection = crate::analysis::inspect_object(
                &graph,
                Some(&dominator),
                target_id,
                params.retain_field_data,
            )
            .ok_or_else(|| inspect_object_id_not_found(&params.object_id, &params.heap_path))?;
            Ok(serde_json::to_value(inspection)?)
        }
        "detect_classloader_leaks" => {
            let params: DetectClassloaderLeaksParams = serde_json::from_value(packet.params)?;

            // Thin handler: parse the heap into a graph and call the
            // existing M13 Slice 13.A analysis function directly -- no new
            // analysis logic here, same "focused, cheaper single-purpose
            // call" rationale as `diff_heaps`.
            let graph = crate::hprof::parse_hprof_file_with_options(
                &params.heap_path,
                ParseOptions {
                    retain_field_data: false,
                },
            )?;
            let duplicates = detect_duplicate_classes(&graph);
            Ok(serde_json::to_value(duplicates)?)
        }
        "explain_leak" => {
            let params: ExplainLeakParams = serde_json::from_value(packet.params)?;
            match (&params.heap_path, &params.session_id) {
                (Some(_), Some(_)) | (None, None) => Err(CoreError::InvalidInput(
                    "exactly one of heap_path or session_id is required".into(),
                )),
                (Some(heap_path), None) => {
                    let mut analyze_config = config.clone();
                    analyze_config.ai.enabled = false;
                    let mut leak_options = LeakDetectionOptions::from(&analyze_config.analysis);
                    if let Some(sev) = params.min_severity {
                        leak_options.min_severity = sev;
                    }
                    let analysis = analyze_heap(AnalyzeRequest {
                        heap_path: heap_path.clone(),
                        config: analyze_config.clone(),
                        leak_options,
                        enable_ai: false,
                        histogram_group_by: HistogramGroupBy::Class,
                        ..AnalyzeRequest::default()
                    })
                    .await?;
                    if let Some(ref target) = params.leak_id {
                        validate_leak_id(&analysis.leaks, target)?;
                    }
                    let focused = focus_leaks(&analysis.leaks, params.leak_id.as_deref());
                    let mut ai_config = analyze_config.ai.clone();
                    ai_config.enabled = true;
                    let ai =
                        generate_ai_insights_async(&analysis.summary, &focused, &ai_config).await?;
                    Ok(serde_json::to_value(ai)?)
                }
                (None, Some(session_id)) => {
                    if params.min_severity.is_some() {
                        return Err(CoreError::InvalidInput(
                            "min_severity is not supported for session-backed explain_leak".into(),
                        ));
                    }
                    let store = session_store(config);
                    let session = store.load(session_id)?;
                    let focused = session_resolved_leaks(&session, params.leak_id.as_deref())?;
                    let mut ai_config = config.ai.clone();
                    ai_config.enabled = true;
                    let ai =
                        generate_ai_insights_async(&session.analysis.summary, &focused, &ai_config)
                            .await?;
                    Ok(serde_json::to_value(ai)?)
                }
            }
        }
        "propose_fix" => {
            let params: ProposeFixParams = serde_json::from_value(packet.params)?;
            match (&params.heap_path, &params.session_id) {
                (Some(_), Some(_)) | (None, None) => Err(CoreError::InvalidInput(
                    "exactly one of heap_path or session_id is required".into(),
                )),
                (Some(heap_path), None) => {
                    let response = propose_fix_with_config(
                        FixRequest {
                            heap_path: heap_path.clone(),
                            leak_id: params.leak_id,
                            style: params.style,
                            project_root: params.project_root,
                        },
                        config,
                    )
                    .await?;
                    Ok(serde_json::to_value(response)?)
                }
                (None, Some(session_id)) => {
                    let store = session_store(config);
                    let session = store.load(session_id)?;
                    let leak_id = params
                        .leak_id
                        .clone()
                        .or(session.conversation.focus_leak_id.clone());
                    let response = propose_fix_for_leaks_with_config(
                        &session.analysis.leaks,
                        &FixRequest {
                            heap_path: session.heap_path,
                            leak_id,
                            style: params.style,
                            project_root: params.project_root,
                        },
                        config,
                    )
                    .await?;
                    Ok(serde_json::to_value(response)?)
                }
            }
        }
        "describe_workflow" => {
            let params: DescribeWorkflowParams = serde_json::from_value(packet.params)?;
            let kind = parse_workflow_kind(&params.kind)?;
            let description = crate::workflow::describe(kind)?;
            Ok(serde_json::to_value(description)?)
        }
        "start_workflow" => {
            let params: StartWorkflowParams = serde_json::from_value(packet.params)?;
            let kind = parse_workflow_kind(&params.kind)?;

            let (heap_path, initial_params) = match kind {
                WorkflowKind::TriageMemoryLeak
                | WorkflowKind::TuneGc
                | WorkflowKind::TraverseObjectGraph => {
                    let heap_path = params.heap_path.clone().ok_or_else(|| {
                        CoreError::InvalidInput(format!(
                            "heap_path is required for start_workflow(kind: \"{}\")",
                            kind.as_str()
                        ))
                    })?;
                    let initial_params = if kind == WorkflowKind::TraverseObjectGraph {
                        json!({ "object_id": params.object_id })
                    } else {
                        Value::Null
                    };
                    (heap_path, initial_params)
                }
                WorkflowKind::CompareSnapshots => (
                    // Caller-invisible placeholder -- see
                    // `core::workflow::compare_snapshots`'s module doc
                    // comment ("dual heap identity" section). Overwritten by
                    // the workflow's first step once both sides resolve.
                    String::new(),
                    json!({
                        "before_heap_path": params.before_heap_path,
                        "after_heap_path": params.after_heap_path,
                        "before_snapshot_key": params.before_snapshot_key,
                        "after_snapshot_key": params.after_snapshot_key,
                    }),
                ),
            };

            let store = workflow_store();
            let state = crate::workflow::start(&store, kind, heap_path, initial_params).await?;
            workflow_step_response(&state)
        }
        "next_step" => {
            let params: NextStepParams = serde_json::from_value(packet.params)?;
            let store = workflow_store();
            let state =
                crate::workflow::advance(&store, &params.workflow_id, params.step_input).await?;
            workflow_step_response(&state)
        }
        "get_workflow" => {
            let params: WorkflowIdParams = serde_json::from_value(packet.params)?;
            let store = workflow_store();
            let state = store.load(&params.workflow_id)?;
            Ok(serde_json::to_value(state)?)
        }
        "close_workflow" => {
            let params: WorkflowIdParams = serde_json::from_value(packet.params)?;
            let store = workflow_store();
            store.remove(&params.workflow_id)?;
            Ok(json!({ "workflow_id": params.workflow_id, "closed": true }))
        }
        other => Err(CoreError::InvalidInput(format!(
            "unsupported MCP method: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::build_graph_fixture;
    use serde_json::json;
    use std::ffi::OsString;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_fixture() -> NamedTempFile {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();
        file
    }

    async fn analyze_heap_result(heap_path: &str, extra_params: Value) -> Value {
        let mut params = serde_json::Map::new();
        params.insert("heap_path".into(), json!(heap_path));
        if let Some(extra) = extra_params.as_object() {
            params.extend(extra.clone());
        }

        handle_request(
            RpcRequest {
                id: json!(1),
                method: "analyze_heap".into(),
                params: Value::Object(params),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap()
    }

    async fn parse_heap_result(path: &str, extra_params: Value) -> Value {
        let mut params = serde_json::Map::new();
        params.insert("path".into(), json!(path));
        if let Some(extra) = extra_params.as_object() {
            params.extend(extra.clone());
        }

        handle_request(
            RpcRequest {
                id: json!(1),
                method: "parse_heap".into(),
                params: Value::Object(params),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap()
    }

    async fn mode_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
        crate::analysis::mode::test_mode_env_lock().lock().await
    }

    /// M9 Slice 9.D: serializes `MNEMOSYNE_SNAPSHOT_DIR` mutation across
    /// snapshot-backed tests. Delegates to `crate::snapshot::test_snapshot_env_lock`
    /// (crate-shared, not a private-to-this-file static) since this crate's
    /// unified `mnemosyne_core` lib test binary also runs
    /// `core::workflow::compare_snapshots`'s tests, which mutate the same
    /// env var -- a private per-file lock here would not serialize against
    /// those, which is exactly the race that used to slip through.
    async fn snapshot_env_lock() -> tokio::sync::MutexGuard<'static, ()> {
        crate::snapshot::test_snapshot_env_lock().lock().await
    }

    /// Pins `MNEMOSYNE_SNAPSHOT_DIR` to `dir` for the duration of the
    /// returned guard, restoring the previous value on drop. Callers must
    /// hold `snapshot_env_lock()` for the same duration to avoid cross-test
    /// races (same shape as the mode tests' `TempEnvVar` usage).
    fn pin_snapshot_dir(dir: &std::path::Path) -> TempEnvVar {
        TempEnvVar::set("MNEMOSYNE_SNAPSHOT_DIR", &dir.to_string_lossy())
    }

    /// Parses `heap_file`, builds its dominator tree, and saves it to a
    /// snapshot store rooted at `store_dir` -- returns the manifest
    /// (notably `heap_sha256`, the key every snapshot-backed call below
    /// uses). `retain_field_data` controls whether the cached graph carries
    /// instance field values (needed by field-based `query_heap` tests).
    fn seed_snapshot(
        store_dir: &std::path::Path,
        heap_file: &std::path::Path,
        retain_field_data: bool,
    ) -> crate::snapshot::SnapshotManifest {
        let graph = crate::hprof::parse_hprof_file_with_options(
            &heap_file.to_string_lossy(),
            ParseOptions { retain_field_data },
        )
        .unwrap();
        let dominator = crate::graph::build_dominator_tree(&graph);
        let store = SnapshotStore::new(store_dir.to_path_buf());
        store
            .save(&heap_file.to_string_lossy(), &graph, &dominator)
            .unwrap()
    }

    async fn list_tools_result() -> Value {
        handle_request(
            RpcRequest {
                id: json!(1),
                method: "list_tools".into(),
                params: Value::Null,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap()
    }

    async fn diff_heaps_result(
        before: &str,
        after: &str,
        extra_params: Value,
    ) -> CoreResult<Value> {
        let mut params = serde_json::Map::new();
        params.insert("before".into(), json!(before));
        params.insert("after".into(), json!(after));
        if let Some(extra) = extra_params.as_object() {
            params.extend(extra.clone());
        }

        handle_request(
            RpcRequest {
                id: json!(99),
                method: "diff_heaps".into(),
                params: Value::Object(params),
            },
            &AppConfig::default(),
        )
        .await
    }

    async fn diff_heaps_response(before: &str, after: &str, extra_params: Value) -> Value {
        let response = match diff_heaps_result(before, after, extra_params).await {
            Ok(value) => RpcResponse::success(json!(99), value),
            Err(err) => RpcResponse::from_core_error(json!(99), &err),
        };

        serde_json::to_value(response).expect("diff_heaps response should serialize")
    }

    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u64(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_be_bytes());
    }

    fn build_overview_only_fixture() -> Vec<u8> {
        const TAG_STRING_IN_UTF8: u8 = 0x01;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
        push_u32(&mut bytes, 8);
        push_u64(&mut bytes, 0);

        let body = b"synthetic-record";
        bytes.push(TAG_STRING_IN_UTF8);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, body.len() as u32 + 8);
        push_u64(&mut bytes, 1);
        bytes.extend_from_slice(body);

        bytes
    }

    struct TempEnvVar {
        key: String,
        previous: Option<OsString>,
    }

    impl TempEnvVar {
        fn set(key: &str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self {
                key: key.to_owned(),
                previous,
            }
        }
    }

    impl Drop for TempEnvVar {
        fn drop(&mut self) {
            match &self.previous {
                Some(previous) => std::env::set_var(&self.key, previous),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    fn normalize_deep_result(value: &mut Value) {
        if let Some(object) = value.as_object_mut() {
            object.remove("elapsed");
            if let Some(summary) = object.get_mut("summary").and_then(Value::as_object_mut) {
                summary.remove("generated_at");
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_advertises_mode_parameter() {
        let _guard = mode_test_guard().await;
        let result = list_tools_result().await;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let analyze_tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("analyze_heap")))
            .expect("analyze_heap tool");
        let mode_param = analyze_tool
            .get("params")
            .and_then(Value::as_array)
            .and_then(|params| {
                params
                    .iter()
                    .find(|param| param.get("name") == Some(&json!("mode")))
            })
            .expect("mode param");

        assert_eq!(
            mode_param.get("enum"),
            Some(&json!(["auto", "deep", "overview"]))
        );
        assert_eq!(mode_param.get("required"), Some(&json!(false)));
        assert!(
            analyze_tool
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| {
                    description.contains("streaming")
                        && description.contains("no object graph")
                        && description.contains("approximate")
                }),
            "analyze_heap description should disclose overview limitations"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_parse_heap_default_mode_keeps_existing_shape() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let result = parse_heap_result(&file.path().to_string_lossy(), json!({})).await;

        assert!(
            result.get("header").is_some(),
            "parse_heap should keep the deep summary shape"
        );
        assert!(
            result.get("mode").is_none(),
            "default parse_heap response should preserve the existing shape"
        );
        assert!(result.get("class_stats").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_parse_heap_mode_overview_returns_overview_summary() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let result = parse_heap_result(
            &file.path().to_string_lossy(),
            json!({ "mode": "overview" }),
        )
        .await;

        assert_eq!(result.get("mode"), Some(&json!("overview")));
        assert!(result.get("class_stats").is_some());
        assert!(result.get("header").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_default_mode_uses_deep_for_small_fixture() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let result = analyze_heap_result(&file.path().to_string_lossy(), json!({})).await;

        assert!(
            result.get("summary").is_some(),
            "deep response should include summary"
        );
        assert!(
            result.get("leaks").is_some(),
            "deep response should include leaks"
        );
        assert!(
            result.get("graph").is_some(),
            "deep response should include graph metrics"
        );
        assert!(
            result.get("class_stats").is_none(),
            "deep response must not impersonate overview payload"
        );
        assert!(
            result.get("mode").is_none(),
            "deep response should keep the existing serialized shape"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_mode_deep_explicit_matches_default() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let mut default_result =
            analyze_heap_result(&file.path().to_string_lossy(), json!({})).await;
        let mut explicit_result =
            analyze_heap_result(&file.path().to_string_lossy(), json!({ "mode": "deep" })).await;

        normalize_deep_result(&mut default_result);
        normalize_deep_result(&mut explicit_result);

        assert_eq!(explicit_result, default_result);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_mode_overview_returns_overview_summary() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let result = analyze_heap_result(
            &file.path().to_string_lossy(),
            json!({ "mode": "overview" }),
        )
        .await;

        assert_eq!(result.get("mode"), Some(&json!("overview")));
        assert!(
            result.get("summary").is_none(),
            "overview response must not reuse AnalyzeResponse shape"
        );

        let class_entries = result
            .get("class_stats")
            .and_then(|value| value.get("entries"))
            .and_then(Value::as_array)
            .expect("overview class_stats entries");
        assert!(
            !class_entries.is_empty(),
            "overview class stats should not be empty"
        );
        assert!(class_entries.iter().any(|entry| {
            entry
                .get("class_name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.contains("BigCache"))
        }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_mode_invalid_returns_error() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(17),
                method: "analyze_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "mode": "bogus"
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(17), &err);
        let serialized = serde_json::to_value(response).unwrap();
        assert_eq!(serialized.get("success"), Some(&json!(false)));
        assert_eq!(
            serialized
                .get("error_details")
                .and_then(|value| value.get("code")),
            Some(&json!("invalid_params"))
        );
        assert!(
            serialized
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|message| message.contains("bogus") && message.contains("overview")),
            "invalid mode should surface a clean parameter error"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_auto_threshold_env_forces_overview() {
        let _guard = mode_test_guard().await;
        let file = write_fixture();
        let heap_path = file.path().to_string_lossy().into_owned();
        let _guard = TempEnvVar::set("MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD", "1");

        let result = analyze_heap_result(&heap_path, json!({ "mode": "auto" })).await;

        assert_eq!(result.get("mode"), Some(&json!("overview")));
        assert!(result.get("class_stats").is_some());
        assert!(result.get("summary").is_none());
    }

    #[tokio::test]
    async fn handle_request_analyze_heap_includes_classloader_report_when_enabled() {
        let file = write_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "analyze_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "enable_classloaders": true,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let classloader_report = result
            .get("classloader_report")
            .and_then(Value::as_object)
            .expect("classloader_report object");
        assert!(classloader_report.contains_key("loaders"));
        assert!(classloader_report.contains_key("potential_leaks"));
    }

    #[tokio::test]
    async fn handle_request_query_heap_returns_rows() {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "query_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "query": r#"SELECT @objectId, @className FROM "com.example.BigCache""#,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let rows = result
            .get("rows")
            .and_then(Value::as_array)
            .expect("rows array");
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn handle_request_query_heap_projects_instance_fields() {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

        let result = handle_request(
            RpcRequest {
                id: json!(2),
                method: "query_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "query": r#"SELECT @objectId, entries FROM "com.example.BigCache" WHERE entries = 8192"#,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            result.get("columns"),
            Some(&json!(["@objectId", "entries"]))
        );
        assert_eq!(result.get("total_matched"), Some(&json!(1)));
        assert_eq!(
            result.get("rows"),
            Some(&json!([[{ "Id": 4096 }, { "Id": 8192 }]]))
        );
    }

    #[tokio::test]
    async fn handle_request_list_tools_returns_descriptions() {
        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "list_tools".into(),
                params: Value::Null,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        assert!(tools.iter().any(|tool| {
            tool.get("name") == Some(&json!("analyze_heap"))
                && tool
                    .get("description")
                    .and_then(Value::as_str)
                    .is_some_and(|desc| desc.contains("full analysis"))
        }));
        assert!(tools.iter().any(|tool| {
            tool.get("name") == Some(&json!("list_tools"))
                && tool
                    .get("params")
                    .and_then(Value::as_array)
                    .is_some_and(|params| params.is_empty())
        }));
        assert!(tools.iter().any(|tool| {
            tool.get("name") == Some(&json!("query_heap"))
                && tool
                    .get("description")
                    .and_then(Value::as_str)
                    .is_some_and(|desc| desc.contains("instance fields"))
        }));
    }

    #[test]
    fn rpc_response_error_includes_structured_details() {
        let response = RpcResponse::from_core_error(
            json!(7),
            &CoreError::ConfigError {
                detail: "missing API key".into(),
                suggestion: Some("Set MNEMOSYNE_TEST_API_KEY before retrying.".into()),
            },
        );

        let serialized = serde_json::to_value(&response).unwrap();
        assert_eq!(serialized.get("id"), Some(&json!(7)));
        assert_eq!(serialized.get("success"), Some(&json!(false)));
        assert_eq!(
            serialized.get("error"),
            Some(&json!("Configuration error: missing API key"))
        );

        let details = serialized
            .get("error_details")
            .and_then(Value::as_object)
            .expect("error_details object");
        assert_eq!(details.get("code"), Some(&json!("config_error")));
        assert_eq!(
            details.get("message"),
            Some(&json!("Configuration error: missing API key"))
        );
        assert_eq!(
            details
                .get("details")
                .and_then(Value::as_object)
                .and_then(|value| value.get("suggestion")),
            Some(&json!("Set MNEMOSYNE_TEST_API_KEY before retrying."))
        );
    }

    #[test]
    fn rpc_response_maps_session_error_codes() {
        for (error, expected_code) in [
            (
                CoreError::InvalidInput("session not found: session-404".into()),
                "session_not_found",
            ),
            (
                CoreError::InvalidInput(
                    "session load failed: session-123: embedded session_id session-actual does not match requested session_id session-123"
                        .into(),
                ),
                "session_load_failed",
            ),
            (
                CoreError::Unsupported("session_version 99 is unsupported".into()),
                "session_version_unsupported",
            ),
            (
                CoreError::Other(anyhow::anyhow!("session persist failed: disk full")),
                "session_persist_failed",
            ),
        ] {
            let response = RpcResponse::from_core_error(json!(17), &error);
            let serialized = serde_json::to_value(&response).unwrap();
            let details = serialized
                .get("error_details")
                .and_then(Value::as_object)
                .expect("error_details object");

            assert_eq!(details.get("code"), Some(&json!(expected_code)));
        }
    }

    #[tokio::test]
    async fn handle_request_propose_fix_falls_back_without_transport_error() {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let result = handle_request(
            RpcRequest {
                id: json!(8),
                method: "propose_fix".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "style": "Minimal"
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let suggestions = result
            .get("suggestions")
            .and_then(Value::as_array)
            .expect("suggestions array");
        assert_eq!(suggestions.len(), 1);

        let provenance = result
            .get("provenance")
            .and_then(Value::as_array)
            .expect("provenance array");
        assert!(provenance
            .iter()
            .any(|m| m.get("kind") == Some(&json!("FALLBACK"))));
    }

    #[tokio::test]
    async fn handle_request_propose_fix_returns_provider_backed_patch() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "TOON v1\nsection response\n  confidence_pct=84\n  description=Evict idle entries before they accumulate.\nsection patch\n  diff=--- a/src/main/java/com/example/BigCache.java\\n+++ b/src/main/java/com/example/BigCache.java\\n@@ ...\n"
                    }
                }
            ]
        })
        .to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 8192];
            let read = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..read]).into_owned();
            assert!(request.contains("intent=generate_fix"), "{request}");
            assert!(
                request.contains("target_file=src/main/java/com/example/BigCache.java"),
                "{request}"
            );

            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

        let project = tempfile::tempdir().unwrap();
        let source_dir = project
            .path()
            .join("src")
            .join("main")
            .join("java")
            .join("com")
            .join("example");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("BigCache.java"),
            "package com.example;\npublic class BigCache {\n  void retain() {}\n}\n",
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;
        config.ai.provider = crate::config::AiProvider::Local;
        config.ai.endpoint = Some(format!("http://{addr}/v1"));
        config.ai.api_key_env = Some("MNEMOSYNE_TEST_MCP_LOCAL_KEY".into());
        config.ai.timeout_secs = 2;
        std::env::set_var("MNEMOSYNE_TEST_MCP_LOCAL_KEY", "dummy-key");

        let result = handle_request(
            RpcRequest {
                id: json!(9),
                method: "propose_fix".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "project_root": project.path().to_string_lossy().into_owned(),
                    "style": "Minimal"
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let suggestions = result
            .get("suggestions")
            .and_then(Value::as_array)
            .expect("suggestions array");
        assert_eq!(suggestions.len(), 1);
        assert_eq!(
            suggestions[0].get("description"),
            Some(&json!("Evict idle entries before they accumulate."))
        );
        assert!(result.get("provenance").is_none());

        std::env::remove_var("MNEMOSYNE_TEST_MCP_LOCAL_KEY");
        server.join().unwrap();
    }

    #[tokio::test]
    async fn handle_request_create_ai_session_returns_session_metadata() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let result = handle_request(
            RpcRequest {
                id: json!(11),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        assert!(result.get("session_id").and_then(Value::as_str).is_some());
        assert_eq!(result.get("focus_leak_id"), Some(&Value::Null));
        assert!(result.get("top_leaks").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn handle_request_resume_ai_session_reads_persisted_state() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(12),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

        let resumed = handle_request(
            RpcRequest {
                id: json!(13),
                method: "resume_ai_session".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(resumed.get("session_id"), Some(&json!(session_id)));
        assert!(resumed.get("history").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn handle_request_get_ai_session_returns_compact_metadata() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(33),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

        let compact = handle_request(
            RpcRequest {
                id: json!(34),
                method: "get_ai_session".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(compact.get("session_id"), Some(&json!(session_id)));
        assert_eq!(compact.get("history_length"), Some(&json!(0)));
        assert_eq!(compact.get("focus_leak_id"), Some(&Value::Null));
        assert!(compact.get("summary").is_none());
    }

    #[tokio::test]
    async fn handle_request_resume_ai_session_rejects_invalid_session_id() {
        let sessions = tempfile::tempdir().unwrap();
        let mut config = AppConfig::default();
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let err = handle_request(
            RpcRequest {
                id: json!(101),
                method: "resume_ai_session".into(),
                params: json!({ "session_id": "../escaped" }),
            },
            &config,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("invalid session_id"));
    }

    #[tokio::test]
    async fn handle_request_close_ai_session_removes_persisted_state() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(14),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

        handle_request(
            RpcRequest {
                id: json!(15),
                method: "close_ai_session".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap();

        let err = handle_request(
            RpcRequest {
                id: json!(16),
                method: "resume_ai_session".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("session"));
    }

    #[tokio::test]
    async fn handle_request_chat_session_updates_history_and_focus() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Rules;
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(21),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned(),
                    "min_severity": "LOW"
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();
        let leak_id = created
            .get("top_leaks")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .unwrap();

        let ai = handle_request(
            RpcRequest {
                id: json!(22),
                method: "chat_session".into(),
                params: json!({
                    "session_id": session_id,
                    "question": "What should I fix first?",
                    "focus_leak_id": leak_id
                }),
            },
            &config,
        )
        .await
        .unwrap();

        assert!(ai.get("summary").and_then(Value::as_str).is_some());

        let resumed = handle_request(
            RpcRequest {
                id: json!(23),
                method: "resume_ai_session".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(resumed.get("focus_leak_id"), Some(&json!(leak_id)));
        assert_eq!(
            resumed
                .get("history")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[tokio::test]
    async fn handle_request_explain_leak_supports_session_id() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Rules;
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(24),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

        let explained = handle_request(
            RpcRequest {
                id: json!(25),
                method: "explain_leak".into(),
                params: json!({ "session_id": session_id }),
            },
            &config,
        )
        .await
        .unwrap();

        assert!(explained.get("summary").and_then(Value::as_str).is_some());
    }

    #[tokio::test]
    async fn handle_request_propose_fix_supports_session_id() {
        let fixture = build_graph_fixture();
        let mut heap = NamedTempFile::new().unwrap();
        heap.write_all(&fixture).unwrap();
        let sessions = tempfile::tempdir().unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = false;
        config.ai.sessions.directory = Some(sessions.path().display().to_string());

        let created = handle_request(
            RpcRequest {
                id: json!(26),
                method: "create_ai_session".into(),
                params: json!({
                    "heap_path": heap.path().to_string_lossy().into_owned()
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

        let fix = handle_request(
            RpcRequest {
                id: json!(27),
                method: "propose_fix".into(),
                params: json!({
                    "session_id": session_id,
                    "style": "Minimal"
                }),
            },
            &config,
        )
        .await
        .unwrap();

        assert!(fix.get("suggestions").and_then(Value::as_array).is_some());
    }

    #[tokio::test]
    async fn handle_request_explain_leak_rejects_conflicting_context_sources() {
        let err = handle_request(
            RpcRequest {
                id: json!(31),
                method: "explain_leak".into(),
                params: json!({
                    "heap_path": "heap.hprof",
                    "session_id": "session-123"
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("exactly one of heap_path or session_id"));
    }

    #[tokio::test]
    async fn handle_request_list_tools_includes_ai_session_methods() {
        let result = handle_request(
            RpcRequest {
                id: json!(32),
                method: "list_tools".into(),
                params: Value::Null,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");

        for name in [
            "create_ai_session",
            "resume_ai_session",
            "get_ai_session",
            "close_ai_session",
            "chat_session",
        ] {
            assert!(
                tools
                    .iter()
                    .any(|tool| tool.get("name") == Some(&json!(name))),
                "missing {name}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_tools_includes_diff_heaps() {
        let result = list_tools_result().await;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let diff_tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("diff_heaps")))
            .expect("diff_heaps tool");

        assert_eq!(
            diff_tool.get("description"),
            Some(&json!(
                "Compute class-level and (optionally) object-level diff between two HPROF heap dumps."
            ))
        );
        assert_eq!(
            diff_tool.get("output_schema"),
            Some(&json!(
                "HeapDiff (existing) extended with optional object_diff: ObjectDiffReport"
            ))
        );
        assert_eq!(
            diff_tool.get("params"),
            Some(&json!([
                {
                    "name": "before",
                    "type": "string",
                    "required": true,
                    "description": "Path to the BEFORE heap dump."
                },
                {
                    "name": "after",
                    "type": "string",
                    "required": true,
                    "description": "Path to the AFTER heap dump."
                },
                {
                    "name": "mode",
                    "type": "string",
                    "required": false,
                    "default": "class",
                    "enum": ["class", "object"],
                    "description": "'class' (default) or 'object'."
                },
                {
                    "name": "identity_strategy",
                    "type": "string",
                    "required": false,
                    "default": "class+dominator",
                    "enum": ["class+retained", "class+dominator", "full-fingerprint"],
                    "description": "'class+retained' | 'class+dominator' (default) | 'full-fingerprint'. Ignored when mode='class'."
                },
                {
                    "name": "retained_bucket_bits",
                    "type": "number",
                    "required": false,
                    "default": 10,
                    "description": "Power-of-two bucket exponent for retained sizes; default 10 (1 KB)."
                },
                {
                    "name": "retained_change_threshold",
                    "type": "number",
                    "required": false,
                    "default": 1048576,
                    "description": "Minimum |retained delta| in bytes for inclusion in retained_changed; default 1048576."
                },
                {
                    "name": "top_n",
                    "type": "number",
                    "required": false,
                    "default": 50,
                    "description": "Per-section result cap; default 50."
                },
                {
                    "name": "object_diff_min_retained",
                    "type": "number",
                    "required": false,
                    "default": 4096,
                    "description": "Skip objects whose retained size is below this floor; default 4096."
                },
                {
                    "name": "retain_field_data",
                    "type": "boolean",
                    "required": false,
                    "default": false,
                    "description": "Required when identity_strategy='full-fingerprint'."
                }
            ]))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn diff_heaps_class_mode_returns_heap_diff_shape() {
        let file = write_fixture();
        let heap_path = file.path().to_string_lossy().into_owned();

        let result = diff_heaps_result(&heap_path, &heap_path, json!({}))
            .await
            .expect("diff_heaps class mode should succeed");
        let expected =
            match crate::diff::run_diff(crate::diff::DiffRequest::class(&heap_path, &heap_path))
                .await
                .expect("direct class diff should succeed")
            {
                crate::diff::DiffResult::Class(diff) => serde_json::to_value(diff).unwrap(),
                crate::diff::DiffResult::Object(_) => panic!("expected class diff"),
            };

        assert_eq!(result, expected);
        assert!(result.get("object_diff").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn diff_heaps_object_mode_returns_populated_object_diff() {
        let file = write_fixture();
        let heap_path = file.path().to_string_lossy().into_owned();

        let result = diff_heaps_result(&heap_path, &heap_path, json!({ "mode": "object" }))
            .await
            .expect("diff_heaps object mode should succeed");
        let expected = match crate::diff::run_diff(crate::diff::DiffRequest {
            before_path: heap_path.clone(),
            after_path: heap_path.clone(),
            mode: crate::diff::DiffMode::Object,
            identity_strategy: crate::diff::IdentityStrategy::ClassDominator,
            retained_bucket_bits: 10,
            min_retained_bytes: crate::diff::object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
            retained_change_threshold:
                crate::diff::object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
            top_n: crate::diff::object::types::DEFAULT_OBJECT_DIFF_TOP_N,
            retain_field_data: false,
            cross_reference_leaks: false,
        })
        .await
        .expect("direct object diff should succeed")
        {
            crate::diff::DiffResult::Class(_) => panic!("expected object diff"),
            crate::diff::DiffResult::Object(diff) => serde_json::to_value(diff).unwrap(),
        };

        assert_eq!(result, expected);
        assert!(result.get("object_diff").is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn diff_heaps_full_fingerprint_without_retain_field_data_returns_error_7() {
        let file = write_fixture();
        let heap_path = file.path().to_string_lossy().into_owned();

        let response = diff_heaps_response(
            &heap_path,
            &heap_path,
            json!({
                "mode": "object",
                "identity_strategy": "full-fingerprint"
            }),
        )
        .await;

        assert_eq!(response.get("success"), Some(&json!(false)));
        assert_eq!(
            response
                .get("error_details")
                .and_then(|value| value.get("code")),
            Some(&json!("feature_unavailable_without_field_data"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn diff_heaps_budget_exceeded_returns_error_6() {
        let file = write_fixture();
        let heap_path = file.path().to_string_lossy().into_owned();
        let _guard = TempEnvVar::set("MNEMOSYNE_OBJECT_DIFF_MAX_FINGERPRINTS", "1");

        let response = diff_heaps_response(
            &heap_path,
            &heap_path,
            json!({
                "mode": "object",
                "object_diff_min_retained": 0
            }),
        )
        .await;

        assert_eq!(response.get("success"), Some(&json!(false)));
        assert_eq!(
            response
                .get("error_details")
                .and_then(|value| value.get("code")),
            Some(&json!("feature_unavailable_object_diff_too_large"))
        );
        assert_eq!(
            response
                .pointer("/error_details/details/max_fingerprints")
                .and_then(Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn diff_heaps_overview_only_dump_returns_error_5_shape() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), build_overview_only_fixture()).unwrap();
        let heap_path = file.path().to_string_lossy().into_owned();

        let response =
            diff_heaps_response(&heap_path, &heap_path, json!({ "mode": "object" })).await;

        assert_eq!(response.get("success"), Some(&json!(false)));
        assert_eq!(
            response
                .get("error_details")
                .and_then(|value| value.get("code")),
            Some(&json!("feature_unavailable_in_overview_mode"))
        );
    }

    // --- M8 Slice 8.A: `find_gc_path` all_paths / by_class / max_paths ---

    fn obj_ref_bytes(ids: &[u64]) -> Vec<u8> {
        let mut buf = Vec::new();
        for &id in ids {
            buf.extend_from_slice(&(id as u32).to_be_bytes());
        }
        buf
    }

    /// Diamond-shaped graph: Root(1) -> BranchA(2), BranchB(3), both of
    /// which point at the same Target(4). Two distinct shortest paths
    /// reach Target.
    fn build_diamond_fixture() -> Vec<u8> {
        let mut builder = crate::hprof::test_fixtures::HprofBuilder::new(4);
        builder
            .add_string(1, "com/example/Root")
            .add_string(2, "com/example/BranchA")
            .add_string(3, "com/example/BranchB")
            .add_string(4, "com/example/Target")
            .add_string(5, "left")
            .add_string(6, "right")
            .add_string(7, "next")
            .add_load_class(1, 0x100, 0, 1)
            .add_load_class(2, 0x200, 0, 2)
            .add_load_class(3, 0x300, 0, 3)
            .add_load_class(4, 0x400, 0, 4);

        let mut heap = crate::hprof::test_fixtures::HeapDumpBuilder::new(4);
        heap.add_gc_root_java_frame(1, 1, 0)
            .add_class_dump(0x100, 0, 8, &[(5, 2), (6, 2)])
            .add_class_dump(0x200, 0x100, 4, &[(7, 2)])
            .add_class_dump(0x300, 0x100, 4, &[(7, 2)])
            .add_class_dump(0x400, 0x100, 0, &[])
            .add_instance_dump(1, 0x100, &obj_ref_bytes(&[2, 3]))
            .add_instance_dump(2, 0x200, &obj_ref_bytes(&[4]))
            .add_instance_dump(3, 0x300, &obj_ref_bytes(&[4]))
            .add_instance_dump(4, 0x400, &[]);

        builder.add_heap_dump(heap.build());
        builder.build()
    }

    /// Root(1) -> Session(10), Session(11), Session(12): three live
    /// instances of the same class, each reachable via a distinct one-hop
    /// path from root.
    fn build_by_class_fixture() -> Vec<u8> {
        let mut builder = crate::hprof::test_fixtures::HprofBuilder::new(4);
        builder
            .add_string(1, "com/example/Root")
            .add_string(2, "com/example/Session")
            .add_string(3, "a")
            .add_string(4, "b")
            .add_string(5, "c")
            .add_load_class(1, 0x500, 0, 1)
            .add_load_class(2, 0x600, 0, 2);

        let mut heap = crate::hprof::test_fixtures::HeapDumpBuilder::new(4);
        heap.add_gc_root_java_frame(1, 1, 0)
            .add_class_dump(0x500, 0, 12, &[(3, 2), (4, 2), (5, 2)])
            .add_class_dump(0x600, 0x500, 0, &[])
            .add_instance_dump(1, 0x500, &obj_ref_bytes(&[10, 11, 12]))
            .add_instance_dump(10, 0x600, &[])
            .add_instance_dump(11, 0x600, &[])
            .add_instance_dump(12, 0x600, &[]);

        builder.add_heap_dump(heap.build());
        builder.build()
    }

    fn write_bytes_fixture(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_default_shape_is_unchanged() {
        let file = write_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert!(result.get("path").is_some());
        assert!(
            result.get("all_paths").is_none(),
            "all_paths must be omitted from the default (no new params) response shape"
        );
        assert!(
            result.get("truncated").is_none(),
            "truncated must be omitted from the default (no new params) response shape"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_all_paths_returns_diamond_paths() {
        let file = write_bytes_fixture(&build_diamond_fixture());

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x4",
                    "all_paths": true,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let all_paths = result
            .get("all_paths")
            .and_then(Value::as_array)
            .expect("all_paths array");
        assert_eq!(all_paths.len(), 2);
        assert_eq!(result.get("truncated"), None, "not truncated => omitted");
        assert!(
            result
                .get("path")
                .and_then(Value::as_array)
                .is_some_and(|path| !path.is_empty()),
            "legacy `path` field must still be populated with the shortest path"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_by_class_returns_paths_for_all_instances() {
        let file = write_bytes_fixture(&build_by_class_fixture());

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "by_class": "com.example.Session",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let all_paths = result
            .get("all_paths")
            .and_then(Value::as_array)
            .expect("all_paths array");
        assert_eq!(all_paths.len(), 3);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_max_paths_truncates_honestly() {
        let file = write_bytes_fixture(&build_diamond_fixture());

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x4",
                    "all_paths": true,
                    "max_paths": 1,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let all_paths = result
            .get("all_paths")
            .and_then(Value::as_array)
            .expect("all_paths array");
        assert_eq!(all_paths.len(), 1);
        assert_eq!(result.get("truncated"), Some(&json!(true)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_object_id_not_found_returns_error() {
        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0xdeadbeef",
                    "all_paths": true,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        match err {
            CoreError::Unsupported(detail) => {
                assert!(
                    detail.starts_with("gc_path_object_id_not_found:"),
                    "{detail}"
                );
            }
            other => panic!("expected CoreError::Unsupported, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_by_class_no_instances_returns_error() {
        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "by_class": "com.example.DoesNotExist",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        match err {
            CoreError::Unsupported(detail) => {
                assert!(
                    detail.starts_with("gc_path_class_has_no_live_instances:"),
                    "{detail}"
                );
            }
            other => panic!("expected CoreError::Unsupported, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_advertises_find_gc_path_new_params() {
        let result = list_tools_result().await;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let gc_path_tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("find_gc_path")))
            .expect("find_gc_path tool");
        let params = gc_path_tool
            .get("params")
            .and_then(Value::as_array)
            .expect("params array");

        for expected in ["all_paths", "by_class", "max_paths"] {
            assert!(
                params
                    .iter()
                    .any(|param| param.get("name") == Some(&json!(expected))),
                "find_gc_path tool should advertise the new '{expected}' param"
            );
        }
    }

    // --- M8 Slice 8.C: `inspect_object` ---

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_inspect_object_returns_correct_shape_without_fields() {
        let file = write_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(result.get("object_id"), Some(&json!("0x00001000")));
        assert_eq!(
            result.get("class_name"),
            Some(&json!("com.example.BigCache"))
        );
        assert_eq!(
            result.get("references_out"),
            Some(&json!([{"object_id": "0x00002000", "class_name": "java.lang.Object"}]))
        );
        assert!(
            result.get("fields").is_none(),
            "fields must be omitted when retain_field_data is not set"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_inspect_object_retain_field_data_populates_fields() {
        let file = write_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                    "retain_field_data": true,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let fields = result
            .get("fields")
            .and_then(Value::as_array)
            .expect("fields array");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].get("name"), Some(&json!("entries")));
        assert_eq!(fields[0].get("type_name"), Some(&json!("java.lang.Object")));
        assert_eq!(fields[0].get("value"), Some(&json!("0x00002000")));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_inspect_object_id_not_found_returns_error() {
        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0xdeadbeef",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        match err {
            CoreError::Unsupported(detail) => {
                assert!(
                    detail.starts_with("inspect_object_id_not_found:"),
                    "{detail}"
                );
            }
            other => panic!("expected CoreError::Unsupported, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_inspect_object_id_not_found_error_details_code() {
        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0xdeadbeef",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("object_id_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_advertises_inspect_object() {
        let result = list_tools_result().await;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let inspect_tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("inspect_object")))
            .expect("inspect_object tool");
        let params = inspect_tool
            .get("params")
            .and_then(Value::as_array)
            .expect("params array");

        for expected in ["heap_path", "object_id", "retain_field_data"] {
            assert!(
                params
                    .iter()
                    .any(|param| param.get("name") == Some(&json!(expected))),
                "inspect_object tool should advertise the '{expected}' param"
            );
        }
    }

    // --- M9 Slice 9.D: `open_snapshot` / `list_snapshots` + additive
    // `snapshot` param on analyze_heap/parse_heap/find_gc_path/
    // inspect_object/query_heap ---

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_includes_open_snapshot_and_list_snapshots() {
        let result = list_tools_result().await;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");

        let open_snapshot = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("open_snapshot")))
            .expect("open_snapshot tool");
        assert_eq!(
            open_snapshot.get("params"),
            Some(&json!([
                {
                    "name": "key",
                    "type": "string",
                    "required": true,
                    "description": "SHA-256 hash or direct snapshot file path."
                }
            ]))
        );
        assert_eq!(
            open_snapshot.get("output_schema"),
            Some(&json!("SnapshotManifest"))
        );

        let list_snapshots = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("list_snapshots")))
            .expect("list_snapshots tool");
        assert_eq!(list_snapshots.get("params"), Some(&json!([])));
        assert_eq!(
            list_snapshots.get("output_schema"),
            Some(&json!("Vec<SnapshotManifest>"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_advertises_snapshot_param_on_existing_tools() {
        let result = list_tools_result().await;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");

        for tool_name in [
            "analyze_heap",
            "parse_heap",
            "find_gc_path",
            "inspect_object",
            "query_heap",
        ] {
            let tool = tools
                .iter()
                .find(|tool| tool.get("name") == Some(&json!(tool_name)))
                .unwrap_or_else(|| panic!("{tool_name} tool"));
            let params = tool
                .get("params")
                .and_then(Value::as_array)
                .expect("params array");
            assert!(
                params
                    .iter()
                    .any(|param| param.get("name") == Some(&json!("snapshot"))),
                "{tool_name} should advertise the additive 'snapshot' param"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_open_snapshot_round_trip_returns_matching_manifest() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "open_snapshot".into(),
                params: json!({ "key": manifest.heap_sha256 }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            result.get("heap_sha256"),
            Some(&json!(manifest.heap_sha256))
        );
        assert_eq!(
            result.get("object_count"),
            Some(&json!(manifest.object_count))
        );
        assert_eq!(
            result.get("schema_version"),
            Some(&json!(manifest.schema_version))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_open_snapshot_missing_key_returns_snapshot_not_found_error_details() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "open_snapshot".into(),
                params: json!({ "key": "does-not-exist" }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_not_found"))
        );
        assert_eq!(
            value.pointer("/error_details/details/hint"),
            Some(&json!(
                "run `mnemosyne snapshot save <heap>` first, or `mnemosyne snapshot list`"
            ))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_snapshots_returns_all_saved_manifests() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file_a = write_fixture();
        let file_b = write_bytes_fixture(&build_diamond_fixture());
        let manifest_a = seed_snapshot(store_dir.path(), file_a.path(), false);
        let manifest_b = seed_snapshot(store_dir.path(), file_b.path(), false);

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "list_snapshots".into(),
                params: Value::Null,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let manifests = result.as_array().expect("list_snapshots returns an array");
        assert_eq!(manifests.len(), 2);
        let hashes: Vec<String> = manifests
            .iter()
            .map(|m| {
                m.get("heap_sha256")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert!(hashes.contains(&manifest_a.heap_sha256));
        assert!(hashes.contains(&manifest_b.heap_sha256));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_heap_with_valid_snapshot_matches_normal_parse() {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        let mut baseline = analyze_heap_result(&file.path().to_string_lossy(), json!({})).await;
        let mut via_snapshot = analyze_heap_result(
            &file.path().to_string_lossy(),
            json!({ "snapshot": manifest.heap_sha256 }),
        )
        .await;

        normalize_deep_result(&mut baseline);
        normalize_deep_result(&mut via_snapshot);

        assert_eq!(via_snapshot, baseline);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_heap_with_invalid_snapshot_key_returns_snapshot_not_found_error_details() {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "analyze_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "snapshot": "does-not-exist",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_heap_with_stale_snapshot_source_returns_snapshot_stale_source_error_details(
    ) {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let mut file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        // Mutate the heap file's bytes after the snapshot was saved (mirrors
        // core::snapshot::tests::load_checked_detects_stale_source).
        file.as_file_mut().set_len(0).unwrap();
        file.write_all(b"different bytes now, definitely not the original hprof fixture")
            .unwrap();
        file.flush().unwrap();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "analyze_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "snapshot": manifest.heap_sha256,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_stale_source"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_analyze_heap_with_schema_mismatched_snapshot_returns_snapshot_schema_mismatch_error_details(
    ) {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        // Hand-edit the persisted manifest's schema_version to simulate a
        // snapshot saved by an older/newer binary (mirrors
        // core::snapshot::tests::load_checked_detects_schema_mismatch).
        let payload_path = store_dir
            .path()
            .join(format!("{}.json", manifest.heap_sha256));
        let mut payload: SnapshotPayload =
            serde_json::from_slice(&std::fs::read(&payload_path).unwrap()).unwrap();
        payload.manifest.schema_version = crate::snapshot::SNAPSHOT_SCHEMA_VERSION + 1;
        std::fs::write(&payload_path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "analyze_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "snapshot": manifest.heap_sha256,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_schema_mismatch"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_with_snapshot_matches_normal_parse() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        let baseline = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let via_snapshot = handle_request(
            RpcRequest {
                id: json!(2),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                    "snapshot": manifest.heap_sha256,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(via_snapshot, baseline);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_find_gc_path_with_invalid_snapshot_returns_snapshot_not_found_error_details() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "find_gc_path".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                    "snapshot": "does-not-exist",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_inspect_object_with_snapshot_matches_normal_parse() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        let baseline = handle_request(
            RpcRequest {
                id: json!(1),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let via_snapshot = handle_request(
            RpcRequest {
                id: json!(2),
                method: "inspect_object".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "object_id": "0x1000",
                    "snapshot": manifest.heap_sha256,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(via_snapshot, baseline);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_query_heap_with_snapshot_matches_normal_parse() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        // query_heap always parses with retain_field_data: true, so the
        // seeded snapshot must carry field data too for the field-based
        // WHERE clause below to match on both paths.
        let manifest = seed_snapshot(store_dir.path(), file.path(), true);

        let query = r#"SELECT @objectId, entries FROM "com.example.BigCache" WHERE entries = 8192"#;

        let baseline = handle_request(
            RpcRequest {
                id: json!(1),
                method: "query_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "query": query,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let via_snapshot = handle_request(
            RpcRequest {
                id: json!(2),
                method: "query_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "query": query,
                    "snapshot": manifest.heap_sha256,
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(via_snapshot, baseline);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_query_heap_with_invalid_snapshot_returns_snapshot_not_found_error_details() {
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "query_heap".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "query": r#"SELECT @objectId FROM "com.example.BigCache""#,
                    "snapshot": "does-not-exist",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_parse_heap_with_snapshot_returns_manifest_derived_summary_not_full_heap_summary() {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();
        let manifest = seed_snapshot(store_dir.path(), file.path(), false);

        let result = parse_heap_result(
            &file.path().to_string_lossy(),
            json!({ "snapshot": manifest.heap_sha256 }),
        )
        .await;

        assert_eq!(result.get("source"), Some(&json!("snapshot")));
        assert_eq!(
            result.get("object_count"),
            Some(&json!(manifest.object_count))
        );
        assert!(
            result.get("header").is_none(),
            "snapshot summary must not impersonate a full HeapSummary"
        );
        assert!(result.get("record_stats").is_none());
        assert!(result.get("classes").is_none());
        assert!(result.get("total_shallow_size_bytes").is_some());
        let provenance = result
            .get("provenance")
            .and_then(Value::as_array)
            .expect("provenance array");
        assert!(!provenance.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_parse_heap_with_invalid_snapshot_returns_snapshot_not_found_error_details() {
        let _guard = mode_test_guard().await;
        let _env_lock = snapshot_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_snapshot_dir(store_dir.path());

        let file = write_fixture();

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "parse_heap".into(),
                params: json!({
                    "path": file.path().to_string_lossy().into_owned(),
                    "snapshot": "does-not-exist",
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("snapshot_not_found"))
        );
    }

    // --- M13 Slice 13.C: `detect_classloader_leaks` ---
    //
    // `core::hprof::test_fixtures`'s public builders hardcode every class's
    // `class_loader_id` to the bootstrap loader (0), so a cross-loader
    // duplicate-class shape can't be built through them. Same "local,
    // self-contained hand-rolled-HPROF-bytes" convention already used by
    // `cli/tests/classloader_cli.rs` for the identical problem: a small
    // builder scoped to this test module rather than extending the shared
    // production fixture code for one test's sake.

    const M13_TAG_STRING_IN_UTF8: u8 = 0x01;
    const M13_TAG_LOAD_CLASS: u8 = 0x02;
    const M13_TAG_HEAP_DUMP: u8 = 0x0C;
    const M13_SUB_ROOT_STICKY_CLASS: u8 = 0x05;
    const M13_SUB_CLASS_DUMP: u8 = 0x20;
    const M13_SUB_INSTANCE_DUMP: u8 = 0x21;

    struct M13HprofBuilder {
        id_size: u8,
        buf: Vec<u8>,
        records: Vec<Vec<u8>>,
    }

    impl M13HprofBuilder {
        fn new(id_size: u8) -> Self {
            let mut buf = Vec::new();
            buf.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
            push_u32(&mut buf, u32::from(id_size));
            push_u64(&mut buf, 0);
            Self {
                id_size,
                buf,
                records: Vec::new(),
            }
        }

        fn write_id(buf: &mut Vec<u8>, id: u64, id_size: u8) {
            match id_size {
                4 => push_u32(buf, id as u32),
                8 => push_u64(buf, id),
                _ => panic!("unsupported id_size: {id_size}"),
            }
        }

        fn push_record(&mut self, tag: u8, body: Vec<u8>) {
            let mut record = Vec::with_capacity(9 + body.len());
            record.push(tag);
            push_u32(&mut record, 0);
            push_u32(&mut record, body.len() as u32);
            record.extend_from_slice(&body);
            self.records.push(record);
        }

        fn add_string(&mut self, id: u64, value: &str) {
            let mut body = Vec::new();
            Self::write_id(&mut body, id, self.id_size);
            body.extend_from_slice(value.as_bytes());
            self.push_record(M13_TAG_STRING_IN_UTF8, body);
        }

        fn add_load_class(&mut self, serial: u32, class_obj_id: u64, name_string_id: u64) {
            let mut body = Vec::new();
            push_u32(&mut body, serial);
            Self::write_id(&mut body, class_obj_id, self.id_size);
            push_u32(&mut body, 0);
            Self::write_id(&mut body, name_string_id, self.id_size);
            self.push_record(M13_TAG_LOAD_CLASS, body);
        }

        fn add_heap_dump(&mut self, sub_records: Vec<u8>) {
            self.push_record(M13_TAG_HEAP_DUMP, sub_records);
        }

        fn build(self) -> Vec<u8> {
            let mut buf = self.buf;
            for record in self.records {
                buf.extend_from_slice(&record);
            }
            buf
        }
    }

    struct M13HeapDumpBuilder {
        id_size: u8,
        buf: Vec<u8>,
    }

    impl M13HeapDumpBuilder {
        fn new(id_size: u8) -> Self {
            Self {
                id_size,
                buf: Vec::new(),
            }
        }

        fn add_gc_root_sticky_class(&mut self, obj_id: u64) {
            self.buf.push(M13_SUB_ROOT_STICKY_CLASS);
            M13HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        }

        /// Unlike the production `HeapDumpBuilder::add_class_dump`, this
        /// variant takes an explicit `class_loader_id` rather than
        /// hardcoding the bootstrap loader (0) -- required to construct a
        /// cross-loader duplicate-class shape at all.
        fn add_class_dump(
            &mut self,
            class_obj_id: u64,
            super_class_id: u64,
            class_loader_id: u64,
            instance_size: u32,
        ) {
            self.buf.push(M13_SUB_CLASS_DUMP);
            M13HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
            push_u32(&mut self.buf, 0);
            M13HprofBuilder::write_id(&mut self.buf, super_class_id, self.id_size);
            M13HprofBuilder::write_id(&mut self.buf, class_loader_id, self.id_size);
            for _ in 0..4 {
                M13HprofBuilder::write_id(&mut self.buf, 0, self.id_size);
            }
            push_u32(&mut self.buf, instance_size);
            self.buf.extend_from_slice(&0u16.to_be_bytes()); // constant pool count
            self.buf.extend_from_slice(&0u16.to_be_bytes()); // static field count
            self.buf.extend_from_slice(&0u16.to_be_bytes()); // instance field count
        }

        fn add_instance_dump(&mut self, obj_id: u64, class_obj_id: u64) {
            self.buf.push(M13_SUB_INSTANCE_DUMP);
            M13HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
            push_u32(&mut self.buf, 0);
            M13HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
            push_u32(&mut self.buf, 0); // no field data
        }

        fn build(self) -> Vec<u8> {
            self.buf
        }
    }

    /// Two distinct loader objects (0x1000, 0x2000), each declaring their
    /// own class object for the same class name
    /// ("com/example/webapp/RequestHandler") -- the classic "redeployed
    /// webapp" shape: same class loaded twice, once per generation's
    /// classloader. Mirrors `cli/tests/classloader_cli.rs`'s
    /// `build_classloader_duplicate_fixture`.
    fn build_classloader_duplicate_fixture() -> Vec<u8> {
        const ID_SIZE: u8 = 4;
        const OBJECT_NAME: u64 = 1;
        const LOADER_NAME: u64 = 2;
        const HANDLER_NAME: u64 = 3;

        const OBJECT_CLASS: u64 = 0x100;
        const LOADER_CLASS: u64 = 0x200;
        const HANDLER_CLASS_GEN1: u64 = 0x300;
        const HANDLER_CLASS_GEN2: u64 = 0x301;

        const LOADER_ONE: u64 = 0x1000;
        const LOADER_TWO: u64 = 0x2000;
        const HANDLER_INSTANCE_ONE: u64 = 0x3000;
        const HANDLER_INSTANCE_TWO: u64 = 0x3001;

        let mut builder = M13HprofBuilder::new(ID_SIZE);
        builder.add_string(OBJECT_NAME, "java/lang/Object");
        builder.add_string(LOADER_NAME, "com/example/webapp/WebappLoader");
        builder.add_string(HANDLER_NAME, "com/example/webapp/RequestHandler");
        builder.add_load_class(1, OBJECT_CLASS, OBJECT_NAME);
        builder.add_load_class(2, LOADER_CLASS, LOADER_NAME);
        builder.add_load_class(3, HANDLER_CLASS_GEN1, HANDLER_NAME);
        builder.add_load_class(4, HANDLER_CLASS_GEN2, HANDLER_NAME);

        let mut heap = M13HeapDumpBuilder::new(ID_SIZE);
        heap.add_class_dump(OBJECT_CLASS, 0, 0, 0);
        heap.add_class_dump(LOADER_CLASS, OBJECT_CLASS, 0, 16);
        heap.add_class_dump(HANDLER_CLASS_GEN1, OBJECT_CLASS, LOADER_ONE, 8);
        heap.add_class_dump(HANDLER_CLASS_GEN2, OBJECT_CLASS, LOADER_TWO, 8);

        heap.add_instance_dump(LOADER_ONE, LOADER_CLASS);
        heap.add_instance_dump(LOADER_TWO, LOADER_CLASS);
        heap.add_instance_dump(HANDLER_INSTANCE_ONE, HANDLER_CLASS_GEN1);
        heap.add_instance_dump(HANDLER_INSTANCE_TWO, HANDLER_CLASS_GEN2);

        heap.add_gc_root_sticky_class(LOADER_ONE);
        heap.add_gc_root_sticky_class(LOADER_TWO);
        heap.add_gc_root_sticky_class(HANDLER_INSTANCE_ONE);
        heap.add_gc_root_sticky_class(HANDLER_INSTANCE_TWO);

        builder.add_heap_dump(heap.build());
        builder.build()
    }

    fn write_classloader_duplicate_fixture() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&build_classloader_duplicate_fixture())
            .unwrap();
        file
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_includes_detect_classloader_leaks() {
        let result = list_tools_result().await;

        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("detect_classloader_leaks")))
            .expect("detect_classloader_leaks tool");

        assert_eq!(
            tool.get("description"),
            Some(&json!(
                "Cross-loader duplicate-class detection -- the classic Tomcat/Jetty/Spring hot-redeploy leak pattern."
            ))
        );
        assert_eq!(
            tool.get("params"),
            Some(&json!([
                { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." }
            ]))
        );
        assert_eq!(
            tool.get("output_schema"),
            Some(&json!("Vec<DuplicateClassGroup>"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_detect_classloader_leaks_returns_duplicate_groups() {
        let file = write_classloader_duplicate_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "detect_classloader_leaks".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let groups = result.as_array().expect("array of duplicate groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].get("class_name"),
            Some(&json!("com.example.webapp.RequestHandler"))
        );
        assert_eq!(groups[0].get("loader_count"), Some(&json!(2)));
        assert_eq!(
            groups[0].get("loader_object_ids"),
            Some(&json!([0x1000, 0x2000]))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_detect_classloader_leaks_returns_empty_on_non_duplicated_fixture() {
        let file = write_fixture();

        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "detect_classloader_leaks".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(result, json!([]));
    }

    // --- M11 Slice 11.D: workflow tool registration ---

    /// Serializes `MNEMOSYNE_WORKFLOW_DIR` mutation across workflow-backed
    /// tests within this (multi-threaded by default) test binary, mirroring
    /// `snapshot_env_lock`'s existing pattern.
    async fn workflow_env_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    /// Pins `MNEMOSYNE_WORKFLOW_DIR` to `dir` for the duration of the
    /// returned guard, restoring the previous value on drop. Callers must
    /// hold `workflow_env_lock()` for the same duration, mirroring
    /// `pin_snapshot_dir`'s existing contract.
    fn pin_workflow_dir(dir: &std::path::Path) -> TempEnvVar {
        TempEnvVar::set("MNEMOSYNE_WORKFLOW_DIR", &dir.to_string_lossy())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_list_tools_includes_workflow_tools() {
        let result = list_tools_result().await;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect();
        for expected in [
            "describe_workflow",
            "start_workflow",
            "next_step",
            "get_workflow",
            "close_workflow",
        ] {
            assert!(names.contains(&expected), "list_tools missing {expected}");
        }

        let start_workflow = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("start_workflow")))
            .expect("start_workflow tool");
        let params = start_workflow
            .get("params")
            .and_then(Value::as_array)
            .expect("start_workflow params array");
        let param_names: Vec<&str> = params
            .iter()
            .filter_map(|param| param.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(
            param_names,
            vec![
                "kind",
                "heap_path",
                "object_id",
                "before_heap_path",
                "after_heap_path",
                "before_snapshot_key",
                "after_snapshot_key",
            ]
        );

        let next_step = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("next_step")))
            .expect("next_step tool");
        assert_eq!(
            next_step.pointer("/params/0/name"),
            Some(&json!("workflow_id"))
        );
        assert_eq!(
            next_step.pointer("/params/1/name"),
            Some(&json!("step_input"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_describe_workflow_returns_step_sequence_for_triage_memory_leak() {
        let result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "describe_workflow".into(),
                params: json!({ "kind": "triage_memory_leak" }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let steps = result
            .get("steps")
            .and_then(Value::as_array)
            .expect("steps array");
        let names: Vec<&str> = steps
            .iter()
            .filter_map(|step| step.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(
            names,
            vec!["detect", "investigate_suspect", "explain", "propose_fix"]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_describe_workflow_rejects_unknown_kind() {
        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "describe_workflow".into(),
                params: json!({ "kind": "not_a_real_kind" }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown workflow kind"));
    }

    /// Full `start_workflow` -> `next_step`* -> `complete` round trip for
    /// `triage_memory_leak` via the MCP layer. The `detect` and
    /// `investigate_suspect` steps' `step_result`s are cross-checked against
    /// calling `core::workflow::start`/`advance` directly against the same
    /// fixture heap with the same inputs -- proving the MCP handlers are a
    /// thin, faithful wrapper rather than reshaping the underlying data.
    #[tokio::test(flavor = "current_thread")]
    async fn mcp_workflow_round_trip_matches_direct_core_workflow_for_triage_memory_leak() {
        let _env_lock = workflow_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_workflow_dir(store_dir.path());

        let heap_file = write_fixture();
        let heap_path = heap_file.path().to_string_lossy().into_owned();

        let start_result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "start_workflow".into(),
                params: json!({ "kind": "triage_memory_leak", "heap_path": heap_path }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();

        let workflow_id = start_result
            .get("workflow_id")
            .and_then(Value::as_str)
            .expect("workflow_id")
            .to_string();
        assert_eq!(
            start_result.get("current_step"),
            Some(&json!("investigate_suspect"))
        );
        let leaks = start_result
            .pointer("/step_result/leaks")
            .and_then(Value::as_array)
            .cloned()
            .expect("detect step should return leaks");
        assert!(
            !leaks.is_empty(),
            "fixture heap should surface at least one leak"
        );
        let leak_id = leaks[0]
            .get("id")
            .and_then(Value::as_str)
            .expect("leak id")
            .to_string();

        // Cross-check against a direct core::workflow::start call against
        // the same heap, sharing the same on-disk store root.
        let direct_store = WorkflowStore::new(store_dir.path().to_path_buf());
        let direct_start = crate::workflow::start(
            &direct_store,
            WorkflowKind::TriageMemoryLeak,
            heap_path.clone(),
            Value::Null,
        )
        .await
        .unwrap();
        let direct_detect_output = direct_start
            .step_history
            .last()
            .unwrap()
            .output_summary
            .clone();
        assert_eq!(start_result.get("step_result"), Some(&direct_detect_output));

        let step2 = handle_request(
            RpcRequest {
                id: json!(2),
                method: "next_step".into(),
                params: json!({
                    "workflow_id": workflow_id,
                    "step_input": { "leak_id": leak_id },
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(step2.get("current_step"), Some(&json!("explain")));

        let direct_step2 = crate::workflow::advance(
            &direct_store,
            &direct_start.workflow_id,
            json!({ "leak_id": leak_id }),
        )
        .await
        .unwrap();
        let direct_investigate_output = direct_step2
            .step_history
            .last()
            .unwrap()
            .output_summary
            .clone();
        assert_eq!(step2.get("step_result"), Some(&direct_investigate_output));

        let step3 = handle_request(
            RpcRequest {
                id: json!(3),
                method: "next_step".into(),
                params: json!({ "workflow_id": workflow_id, "step_input": Value::Null }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(step3.get("current_step"), Some(&json!("propose_fix")));

        let step4 = handle_request(
            RpcRequest {
                id: json!(4),
                method: "next_step".into(),
                params: json!({
                    "workflow_id": workflow_id,
                    "step_input": { "skip": true },
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(step4.get("current_step"), Some(&json!("complete")));
        assert_eq!(step4.pointer("/step_result/skipped"), Some(&json!(true)));
        assert_eq!(step4.get("next_expected_input"), Some(&json!([])));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_get_workflow_and_close_workflow_round_trip() {
        let _env_lock = workflow_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_workflow_dir(store_dir.path());

        let heap_file = write_fixture();
        let heap_path = heap_file.path().to_string_lossy().into_owned();

        let start_result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "start_workflow".into(),
                params: json!({ "kind": "tune_gc", "heap_path": heap_path }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        let workflow_id = start_result
            .get("workflow_id")
            .and_then(Value::as_str)
            .expect("workflow_id")
            .to_string();

        let get_result = handle_request(
            RpcRequest {
                id: json!(2),
                method: "get_workflow".into(),
                params: json!({ "workflow_id": workflow_id }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(get_result.get("workflow_id"), Some(&json!(workflow_id)));
        assert_eq!(
            get_result.get("current_step"),
            Some(&json!("thread_local_review"))
        );

        let close_result = handle_request(
            RpcRequest {
                id: json!(3),
                method: "close_workflow".into(),
                params: json!({ "workflow_id": workflow_id }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            close_result,
            json!({ "workflow_id": workflow_id, "closed": true })
        );

        let err = handle_request(
            RpcRequest {
                id: json!(4),
                method: "get_workflow".into(),
                params: json!({ "workflow_id": workflow_id }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();
        let response = RpcResponse::from_core_error(json!(4), &err);
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("workflow_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_next_step_unknown_workflow_id_returns_workflow_not_found_error_details() {
        let _env_lock = workflow_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_workflow_dir(store_dir.path());

        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "next_step".into(),
                params: json!({ "workflow_id": "does-not-exist" }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(1), &err);
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("workflow_not_found"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_next_step_malformed_step_input_returns_workflow_step_input_mismatch_error_details()
    {
        let _env_lock = workflow_env_lock().await;
        let store_dir = tempfile::tempdir().unwrap();
        let _env_guard = pin_workflow_dir(store_dir.path());

        let heap_file = write_fixture();
        let heap_path = heap_file.path().to_string_lossy().into_owned();

        let start_result = handle_request(
            RpcRequest {
                id: json!(1),
                method: "start_workflow".into(),
                params: json!({ "kind": "triage_memory_leak", "heap_path": heap_path }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap();
        let workflow_id = start_result
            .get("workflow_id")
            .and_then(Value::as_str)
            .expect("workflow_id")
            .to_string();

        // investigate_suspect requires {"leak_id": <string>}; send an
        // unrelated field instead.
        let err = handle_request(
            RpcRequest {
                id: json!(2),
                method: "next_step".into(),
                params: json!({
                    "workflow_id": workflow_id,
                    "step_input": { "not_a_field": true },
                }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        let response = RpcResponse::from_core_error(json!(2), &err);
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(
            value.pointer("/error_details/code"),
            Some(&json!("workflow_step_input_mismatch"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mcp_start_workflow_missing_heap_path_returns_invalid_input() {
        let err = handle_request(
            RpcRequest {
                id: json!(1),
                method: "start_workflow".into(),
                params: json!({ "kind": "tune_gc" }),
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("heap_path is required"));
    }
}
