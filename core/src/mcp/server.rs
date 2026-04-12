use crate::{
    analysis::{
        analyze_heap, detect_leaks, focus_leaks, generate_ai_insights_async, validate_leak_id,
        AnalyzeRequest, LeakDetectionOptions, LeakKind, LeakSeverity,
    },
    config::AppConfig,
    errors::{CoreError, CoreResult},
    fix::{propose_fix_with_config, FixRequest, FixStyle},
    graph::{find_gc_path, GcPathRequest},
    hprof::{parse_heap, HeapParseJob},
    mapper::{map_to_code, MapToCodeRequest},
    query::{execute_query, parse_query},
    HistogramGroupBy,
};
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
            CoreError::NotImplemented(detail) => Self {
                code: "not_implemented",
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
    heap_path: String,
    #[serde(default)]
    leak_id: Option<String>,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
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

#[derive(Debug, Deserialize)]
struct QueryHeapParams {
    heap_path: String,
    query: String,
}

#[derive(Debug, Deserialize)]
struct ProposeFixParams {
    heap_path: String,
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
                "name": "explain_leak",
                "description": "Generate AI-backed leak explanations for a heap or a specific leak candidate.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
                    { "name": "leak_id", "type": "string", "required": false, "description": "Optional leak identifier to focus the explanation." },
                    { "name": "min_severity", "type": "string", "required": false, "description": "Optional minimum leak severity override." }
                ]
            },
            {
                "name": "propose_fix",
                "description": "Generate AI-backed fix suggestions when provider mode and source context are available, otherwise fall back to heuristic guidance.",
                "params": [
                    { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
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
            let mut config = config.clone();
            config.ai.enabled = true;
            let mut leak_options = LeakDetectionOptions::from(&config.analysis);
            if let Some(sev) = params.min_severity {
                leak_options.min_severity = sev;
            }
            let analysis = analyze_heap(AnalyzeRequest {
                heap_path: params.heap_path,
                config: config.clone(),
                leak_options,
                enable_ai: true,
                histogram_group_by: HistogramGroupBy::Class,
                ..AnalyzeRequest::default()
            })
            .await?;
            if let Some(ref target) = params.leak_id {
                validate_leak_id(&analysis.leaks, target)?;
            }
            let focused = focus_leaks(&analysis.leaks, params.leak_id.as_deref());
            let ai = generate_ai_insights_async(&analysis.summary, &focused, &config.ai).await?;
            Ok(serde_json::to_value(ai)?)
        }
        "propose_fix" => {
            let params: ProposeFixParams = serde_json::from_value(packet.params)?;
            let response = propose_fix_with_config(
                FixRequest {
                    heap_path: params.heap_path,
                    leak_id: params.leak_id,
                    style: params.style,
                    project_root: params.project_root,
                },
                config,
            )
            .await?;
            Ok(serde_json::to_value(response)?)
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
}
