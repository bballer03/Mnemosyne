use crate::{
    analysis::{
        analyze_heap, detect_leaks, focus_leaks, generate_ai_insights, validate_leak_id,
        AnalyzeRequest, LeakDetectionOptions, LeakKind, LeakSeverity,
    },
    config::AppConfig,
    errors::{CoreError, CoreResult},
    fix::{propose_fix, FixRequest, FixStyle},
    graph::{find_gc_path, GcPathRequest},
    hprof::{parse_heap, HeapParseJob},
    mapper::{map_to_code, MapToCodeRequest},
    query::{execute_query, parse_query},
    HistogramGroupBy,
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

async fn handle_request(packet: RpcRequest, config: &AppConfig) -> CoreResult<Value> {
    match packet.method.as_str() {
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
}
