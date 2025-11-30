use crate::{
    analysis::{
        analyze_heap, detect_leaks, AnalyzeRequest, LeakDetectionOptions, LeakKind, LeakSeverity,
    },
    config::AppConfig,
    errors::{CoreError, CoreResult},
    fix::{propose_fix, FixRequest, FixStyle},
    focus_leaks,
    gc_path::{find_gc_path, GcPathRequest},
    generate_ai_insights,
    heap::{parse_heap, HeapParseJob},
    mapper::{map_to_code, MapToCodeRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
pub async fn serve(options: McpServerOptions) -> CoreResult<()> {
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
                match handle_request(packet).await {
                    Ok(value) => RpcResponse::success(id, value),
                    Err(err) => RpcResponse::error(id, err.to_string()),
                }
            }
            Err(err) => RpcResponse::error(Value::Null, format!("invalid JSON: {err}")),
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
}

impl RpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            id,
            success: true,
            result,
            error: None,
        }
    }

    fn error(id: Value, message: String) -> Self {
        error!(%message, "MCP request failed");
        Self {
            id,
            success: false,
            result: Value::Null,
            error: Some(message),
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

async fn handle_request(packet: RpcRequest) -> CoreResult<Value> {
    match packet.method.as_str() {
        "parse_heap" => {
            let params: ParseHeapParams = serde_json::from_value(packet.params)?;
            let job = HeapParseJob {
                path: params.path,
                include_strings: params.include_strings,
                max_objects: params.max_objects,
            };
            let summary = parse_heap(&job)?;
            Ok(serde_json::to_value(summary)?)
        }
        "detect_leaks" => {
            let params: DetectLeakParams = serde_json::from_value(packet.params)?;
            let mut options =
                LeakDetectionOptions::new(params.min_severity.unwrap_or(LeakSeverity::High));
            options.package_filter = params.package;
            if let Some(leak_types) = params.leak_types {
                options.leak_types = leak_types;
            }
            let leaks = detect_leaks(&params.heap_path, options).await?;
            Ok(serde_json::to_value(leaks)?)
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
            let mut config = AppConfig::default();
            config.ai.enabled = true;
            let analysis = analyze_heap(AnalyzeRequest {
                heap_path: params.heap_path,
                config: config.clone(),
                leak_options: LeakDetectionOptions::new(
                    params.min_severity.unwrap_or(LeakSeverity::Low),
                ),
                enable_ai: true,
            })
            .await?;
            let focused = focus_leaks(&analysis.leaks, params.leak_id.as_deref());
            let ai = generate_ai_insights(&analysis.summary, &focused, &config.ai);
            Ok(serde_json::to_value(ai)?)
        }
        "propose_fix" => {
            let params: ProposeFixParams = serde_json::from_value(packet.params)?;
            let response = propose_fix(FixRequest {
                heap_path: params.heap_path,
                leak_id: params.leak_id,
                style: params.style,
                project_root: params.project_root,
            })
            .await?;
            Ok(serde_json::to_value(response)?)
        }
        other => Err(CoreError::InvalidInput(format!(
            "unsupported MCP method: {other}"
        ))),
    }
}
