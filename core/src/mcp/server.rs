use crate::{
    analysis::{
        analyze_heap, detect_leaks, focus_leaks, generate_ai_chat_turn_async,
        generate_ai_insights_async, validate_leak_id, AiChatTurn, AnalyzeRequest,
        LeakDetectionOptions, LeakKind, LeakSeverity,
    },
    config::AppConfig,
    errors::{CoreError, CoreResult},
    fix::{propose_fix_for_leaks_with_config, propose_fix_with_config, FixRequest, FixStyle},
    graph::{find_gc_path, GcPathRequest},
    hprof::{parse_heap, HeapParseJob},
    mapper::{map_to_code, MapToCodeRequest},
    mcp::session::{
        new_session_id, timestamp_now, top_leak_ids, McpSessionStore, PersistedAiSession,
        SessionAnalysisSnapshot, SessionConversationSnapshot, MCP_SESSION_VERSION,
    },
    query::{execute_query, parse_query},
    HistogramGroupBy,
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

#[derive(Debug, Deserialize)]
struct ParseHeapParams {
    path: String,
    #[serde(default)]
    include_strings: bool,
    #[serde(default)]
    max_objects: Option<u64>,
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
    object_id: String,
    #[serde(default)]
    max_depth: Option<u32>,
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
    top_n: Option<usize>,
    #[serde(default)]
    min_collection_capacity: Option<usize>,
    #[serde(default)]
    min_duplicate_count: Option<usize>,
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

#[derive(Debug, Deserialize)]
struct QueryHeapParams {
    heap_path: String,
    query: String,
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
                "description": "Parse an HPROF file and return a lightweight heap summary.",
                "params": [
                    { "name": "path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "include_strings", "type": "boolean", "required": false, "description": "Accept string extraction in the request, although the summary remains lightweight." },
                    { "name": "max_objects", "type": "number", "required": false, "description": "Optional object cap that falls back to parser.max_objects." }
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
                "description": "Run the full analysis pipeline and return the serialized analysis response.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
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
                    { "name": "top_n", "type": "number", "required": false, "description": "Result count used by top-N analysis sections." },
                    { "name": "min_collection_capacity", "type": "number", "required": false, "description": "Minimum collection capacity to report." },
                    { "name": "min_duplicate_count", "type": "number", "required": false, "description": "Minimum duplicate string count to report." }
                ]
            },
            {
                "name": "query_heap",
                "description": "Execute a built-in-field OQL-style query against the heap graph.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "query", "type": "string", "required": true, "description": "Query text." }
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
                "description": "Find a path from an object to a GC root.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "object_id", "type": "string", "required": true, "description": "Target object identifier, for example 0x1000." },
                    { "name": "max_depth", "type": "number", "required": false, "description": "Optional traversal depth cap." }
                ]
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
            }
        ]
    })
}

async fn handle_request(packet: RpcRequest, config: &AppConfig) -> CoreResult<Value> {
    match packet.method.as_str() {
        "list_tools" => Ok(tool_catalog()),
        "parse_heap" => {
            let params: ParseHeapParams = serde_json::from_value(packet.params)?;
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

            let analysis = analyze_heap(AnalyzeRequest {
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
                top_n: params.top_n.unwrap_or(AnalyzeRequest::default().top_n),
                min_collection_capacity: params
                    .min_collection_capacity
                    .unwrap_or(AnalyzeRequest::default().min_collection_capacity),
                min_duplicate_count: params
                    .min_duplicate_count
                    .unwrap_or(AnalyzeRequest::default().min_duplicate_count),
            })
            .await?;

            Ok(serde_json::to_value(analysis)?)
        }
        "query_heap" => {
            let params: QueryHeapParams = serde_json::from_value(packet.params)?;
            let graph = crate::hprof::parse_hprof_file(&params.heap_path)?;
            let dominator = crate::graph::build_dominator_tree(&graph);
            let query = parse_query(&params.query)
                .map_err(|err| CoreError::InvalidInput(err.to_string()))?;
            let result = execute_query(&query, &graph, Some(&dominator))?;
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
            let response = find_gc_path(&GcPathRequest {
                heap_path: params.heap_path,
                object_id: params.object_id,
                max_depth: params.max_depth,
            })?;
            Ok(serde_json::to_value(response)?)
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn handle_request_analyze_heap_includes_classloader_report_when_enabled() {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

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
}
