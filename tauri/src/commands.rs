use std::path::PathBuf;

use mnemosyne_core::{
    analysis::{analyze_heap, validate_leak_id},
    focus_leaks, generate_ai_insights_async, parse_hprof_file, propose_fix_with_config,
    query::{execute_query, parse_query, CellValue},
    FixRequest, FixResponse, FixStyle, GcPathRequest, HistogramGroupBy, LeakDetectionOptions,
    MapToCodeRequest, ProvenanceMarker, SourceMapResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tokio::task::spawn_blocking;

use crate::state::HeapSession;

const NO_HEAP_LOADED: &str = "No heap loaded";
const LOCK_ERROR: &str = "Heap session lock poisoned";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapLoadSummary {
    heap_path: String,
    object_count: usize,
    class_count: usize,
    gc_root_count: usize,
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

#[derive(Debug, Serialize)]
pub struct ExplainLeakResult {
    leak_id: String,
    summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    provenance: Vec<ProvenanceMarker>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_heap(path: String, state: State<'_, HeapSession>) -> Result<HeapLoadSummary, String> {
    let graph = spawn_blocking({
        let path = path.clone();
        move || parse_hprof_file(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    let summary = HeapLoadSummary {
        heap_path: path.clone(),
        object_count: graph.object_count(),
        class_count: graph.classes.len(),
        gc_root_count: graph.gc_roots.len(),
    };

    *state.graph.write().map_err(|_| LOCK_ERROR.to_string())? = Some(graph);
    *state.heap_path.write().map_err(|_| LOCK_ERROR.to_string())? = Some(path);

    Ok(summary)
}

#[tauri::command]
pub fn unload_heap(state: State<'_, HeapSession>) -> Result<(), String> {
    let mut graph = state.graph.write().map_err(|_| LOCK_ERROR.to_string())?;
    if graph.is_none() {
        return Err(NO_HEAP_LOADED.to_string());
    }

    *graph = None;
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

fn parse_object_id(input: &str) -> Result<u64, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Object id must not be empty".to_string());
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid object id '{trimmed}'"));
    }

    if trimmed.chars().any(|character| matches!(character, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed.trim_start_matches("0x"), 16)
            .map_err(|_| format!("Invalid object id '{trimmed}'"));
    }

    trimmed
        .parse::<u64>()
        .map_err(|_| format!("Invalid object id '{trimmed}'"))
}

fn format_object_id(object_id: u64, id_size: usize) -> String {
    let width = id_size * 2;
    format!("0x{object_id:0width$X}")
}

fn prettify_class_name(raw: &str) -> String {
    raw.replace('/', ".")
}
