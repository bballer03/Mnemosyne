use super::ai::{generate_ai_insights, AiInsights};
use super::{
    analyze_strings, find_top_instances, inspect_collections, inspect_threads, CollectionReport,
    StringReport, ThreadReport, TopInstancesReport,
};
use crate::{
    config::{AnalysisConfig, AppConfig},
    errors::{CoreError, CoreResult},
    graph::{
        build_dominator_tree, build_graph_metrics_from_dominator, build_histogram,
        find_unreachable_objects, summarize_graph, DominatorTree, GraphMetrics, HistogramGroupBy,
        HistogramResult, UnreachableSet, VIRTUAL_ROOT_ID,
    },
    hprof::{
        parse_heap, parse_hprof_file_with_options, ClassDelta, ClassLevelDelta, ClassStat,
        HeapDiff, HeapParseJob, HeapSummary, ObjectGraph, ObjectId, ParseOptions,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    str::FromStr,
    time::{Duration, Instant},
};
use tracing::info;

/// Options that drive leak detection heuristics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LeakDetectionOptions {
    /// Lowest severity that should be reported (candidates below are filtered out).
    pub min_severity: LeakSeverity,
    /// Optional package hints used when synthesizing class names.
    pub package_filters: Vec<String>,
    /// Explicit leak kinds to emit. Empty = auto-detect a single kind.
    pub leak_types: Vec<LeakKind>,
    /// Minimum retained/shallow ratio to flag accumulation points.
    pub accumulation_threshold: f64,
}

/// High-level request for the multi-stage `analyze` workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub heap_path: String,
    pub config: AppConfig,
    pub leak_options: LeakDetectionOptions,
    pub enable_ai: bool,
    pub histogram_group_by: HistogramGroupBy,
    pub enable_threads: bool,
    pub enable_strings: bool,
    pub enable_collections: bool,
    pub enable_top_instances: bool,
    pub top_n: usize,
    pub min_collection_capacity: usize,
    pub min_duplicate_count: usize,
}

impl Default for AnalyzeRequest {
    fn default() -> Self {
        Self {
            heap_path: String::new(),
            config: AppConfig::default(),
            leak_options: LeakDetectionOptions::default(),
            enable_ai: false,
            histogram_group_by: HistogramGroupBy::Class,
            enable_threads: false,
            enable_strings: false,
            enable_collections: false,
            enable_top_instances: false,
            top_n: 10,
            min_collection_capacity: 16,
            min_duplicate_count: 2,
        }
    }
}

/// Explicit provenance metadata for synthetic, partial, fallback, or placeholder output.
///
/// Attach to any response surface so consumers can distinguish real analysis
/// results from heuristic / preview / stub data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProvenanceKind {
    /// Data was synthesized from aggregate summaries, not from class-level analysis.
    Synthetic,
    /// The result is a partial / preview snapshot and may change once full analysis completes.
    Partial,
    /// A fallback path was taken because the preferred data source was unavailable.
    Fallback,
    /// The output is a placeholder stub; real implementation is not wired yet.
    Placeholder,
}

/// A single provenance marker attached to a response surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceMarker {
    pub kind: ProvenanceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ProvenanceMarker {
    /// Marker with an explanatory detail string.
    pub fn new(kind: ProvenanceKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: Some(detail.into()),
        }
    }

    /// Marker without extra detail.
    pub fn bare(kind: ProvenanceKind) -> Self {
        Self { kind, detail: None }
    }
}

/// Response payload returned to callers (CLI, MCP, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeResponse {
    pub summary: HeapSummary,
    pub leaks: Vec<LeakInsight>,
    pub recommendations: Vec<String>,
    pub elapsed: Duration,
    pub graph: GraphMetrics,
    pub ai: Option<AiInsights>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub histogram: Option<HistogramResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreachable: Option<UnreachableSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_report: Option<ThreadReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_report: Option<CollectionReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_report: Option<StringReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_instances: Option<TopInstancesReport>,
    /// Provenance markers for the response as a whole (e.g. partial / preview).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProvenanceMarker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakSuspect {
    pub object_id: ObjectId,
    pub class_name: String,
    pub shallow_size: u64,
    pub retained_size: u64,
    pub ratio: f64,
    pub is_accumulation_point: bool,
    pub dominated_count: u64,
    pub reference_chain: Vec<String>,
    pub score: f64,
}

/// Machine- and human-readable leak record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakInsight {
    pub id: String,
    pub class_name: String,
    pub leak_kind: LeakKind,
    pub severity: LeakSeverity,
    pub retained_size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shallow_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspect_score: Option<f64>,
    pub instances: u64,
    pub description: String,
    /// Provenance markers for this individual leak (e.g. synthetic / fallback).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProvenanceMarker>,
}

/// Enumeration of leak flavors supported by the orchestrator.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, Hash, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LeakKind {
    #[default]
    Unknown,
    Cache,
    Coroutine,
    Thread,
    HttpResponse,
    ClassLoader,
    Collection,
    Listener,
}

impl FromStr for LeakKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "unknown" => Ok(LeakKind::Unknown),
            "cache" => Ok(LeakKind::Cache),
            "coroutine" => Ok(LeakKind::Coroutine),
            "thread" => Ok(LeakKind::Thread),
            "httpresponse" | "http_response" | "http-response" => Ok(LeakKind::HttpResponse),
            "classloader" | "class_loader" | "class-loader" => Ok(LeakKind::ClassLoader),
            "collection" => Ok(LeakKind::Collection),
            "listener" => Ok(LeakKind::Listener),
            other => Err(format!("unsupported leak kind '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LeakSeverity {
    Low,
    Medium,
    #[default]
    High,
    Critical,
}

impl FromStr for LeakSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(LeakSeverity::Low),
            "medium" | "med" => Ok(LeakSeverity::Medium),
            "high" => Ok(LeakSeverity::High),
            "critical" | "crit" => Ok(LeakSeverity::Critical),
            other => Err(format!("unsupported leak severity '{other}'")),
        }
    }
}

/// Execute the core analysis workflow.
///
/// Attempts graph-backed analysis via HPROF object graph + dominator tree.
/// Falls back to heuristic leak detection when HPROF parsing fails.
pub async fn analyze_heap(request: AnalyzeRequest) -> CoreResult<AnalyzeResponse> {
    info!(heap = %request.heap_path, "starting analysis pipeline");
    let start = Instant::now();

    let parse_job = HeapParseJob {
        path: request.heap_path.clone(),
        include_strings: false,
        max_objects: request.config.parser.max_objects,
    };
    let summary = parse_heap(&parse_job)?;
    let retain_field_data =
        request.enable_strings || request.enable_collections || request.enable_threads;

    // Attempt graph-backed analysis
    let dominator_result = try_build_dominator(&request.heap_path, retain_field_data);

    let (
        graph,
        leaks,
        histogram,
        unreachable,
        thread_report,
        collection_report,
        string_report,
        top_instances,
        provenance,
    ) = if let Some((ref obj_graph, ref dom)) = dominator_result {
        let graph_metrics = build_graph_metrics_from_dominator(dom, obj_graph);
        let graph_leaks = graph_backed_leaks(dom, obj_graph, &request.leak_options);
        let histogram = Some(build_histogram(obj_graph, dom, request.histogram_group_by));
        let unreachable = Some(find_unreachable_objects(obj_graph));
        let thread_report = request
            .enable_threads
            .then(|| inspect_threads(obj_graph, Some(dom), request.top_n));
        let collection_report = request
            .enable_collections
            .then(|| inspect_collections(obj_graph, Some(dom), request.min_collection_capacity));
        let string_report = request.enable_strings.then(|| {
            analyze_strings(
                obj_graph,
                Some(dom),
                request.top_n,
                request.min_duplicate_count,
            )
        });
        let top_instances = request
            .enable_top_instances
            .then(|| find_top_instances(obj_graph, Some(dom), request.top_n));
        // If graph-backed produced no leaks (e.g. all filtered), fall back
        if graph_leaks.is_empty() {
            let fallback_leaks = synthesize_leaks(&summary, &request.leak_options);
            (
                graph_metrics,
                fallback_leaks,
                histogram,
                unreachable,
                thread_report,
                collection_report,
                string_report,
                top_instances,
                fallback_provenance(),
            )
        } else {
            (
                graph_metrics,
                graph_leaks,
                histogram,
                unreachable,
                thread_report,
                collection_report,
                string_report,
                top_instances,
                Vec::new(),
            )
        }
    } else {
        let graph = summarize_graph(&summary);
        let leaks = synthesize_leaks(&summary, &request.leak_options);
        (
            graph,
            leaks,
            None,
            None,
            None,
            None,
            None,
            None,
            heuristic_provenance(),
        )
    };

    let ai = if request.enable_ai || request.config.ai.enabled {
        info!(model = %request.config.ai.model, "generating synthetic AI insights");
        Some(generate_ai_insights(&summary, &leaks, &request.config.ai))
    } else {
        None
    };

    Ok(AnalyzeResponse {
        summary,
        leaks,
        recommendations: if dominator_result.is_some() {
            vec![
                "Graph-backed analysis complete. Retained sizes are computed from dominator tree."
                    .into(),
            ]
        } else {
            vec![
                "Integrate dominator-based retained size computation".into(),
                "Add AI insights pipeline".into(),
            ]
        },
        elapsed: start.elapsed(),
        graph,
        ai,
        histogram,
        unreachable,
        thread_report,
        collection_report,
        string_report,
        top_instances,
        provenance,
    })
}

/// Compare two heap snapshots and produce a structured diff of their dominant classes.
pub async fn diff_heaps(before_path: &str, after_path: &str) -> CoreResult<HeapDiff> {
    info!(%before_path, %after_path, "computing heap diff");

    let before_job = HeapParseJob {
        path: before_path.into(),
        include_strings: false,
        max_objects: None,
    };
    let after_job = HeapParseJob {
        path: after_path.into(),
        include_strings: false,
        max_objects: None,
    };

    let before_summary = parse_heap(&before_job)?;
    let after_summary = parse_heap(&after_job)?;

    let mut diff = build_heap_diff(before_summary, after_summary);

    let before_graph = try_build_dominator(before_path, false);
    let after_graph = try_build_dominator(after_path, false);
    if let (Some((before_graph, before_dom)), Some((after_graph, after_dom))) =
        (before_graph, after_graph)
    {
        diff.class_diff = Some(compute_class_level_diff(
            &before_graph,
            &before_dom,
            &after_graph,
            &after_dom,
        ));
    }

    Ok(diff)
}

/// Kick off leak detection without the rest of the analysis pipeline.
///
/// Attempts graph-backed analysis via HPROF object graph + dominator tree first,
/// falling back to heuristic leak detection when HPROF parsing fails.
pub async fn detect_leaks(
    heap_path: &str,
    options: LeakDetectionOptions,
) -> CoreResult<Vec<LeakInsight>> {
    info!(%heap_path, ?options, "detecting leaks");

    // Try graph-backed path first
    if let Some((obj_graph, dom)) = try_build_dominator(heap_path, false) {
        info!(%heap_path, "graph-backed leak detection succeeded");
        let graph_leaks = graph_backed_leaks(&dom, &obj_graph, &options);
        if !graph_leaks.is_empty() {
            return Ok(graph_leaks);
        }
        // Graph produced no leaks (e.g. filters eliminated everything) — fall through to heuristic
        info!(%heap_path, "graph-backed path returned no leaks after filtering; falling back to heuristic");
    } else {
        info!(%heap_path, "graph-backed path unavailable; using heuristic leak detection");
    }

    // Heuristic fallback
    let parse_job = HeapParseJob {
        path: heap_path.into(),
        include_strings: false,
        max_objects: None,
    };
    let summary = parse_heap(&parse_job)?;
    Ok(synthesize_leaks(&summary, &options))
}

impl From<&AnalysisConfig> for LeakDetectionOptions {
    fn from(value: &AnalysisConfig) -> Self {
        let mut options = LeakDetectionOptions::new(value.min_severity);
        options.package_filters = value.packages.clone();
        options.leak_types = value.leak_types.clone();
        options.accumulation_threshold = value.accumulation_threshold;
        options
    }
}

impl AnalyzeResponse {
    pub fn is_successful(&self) -> bool {
        !self.leaks.is_empty()
    }
}

impl From<LeakSeverity> for CoreError {
    fn from(_: LeakSeverity) -> Self {
        CoreError::Unsupported("converting severity into error is not meaningful".into())
    }
}

impl LeakDetectionOptions {
    pub fn new(min_severity: LeakSeverity) -> Self {
        Self {
            min_severity,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        }
    }
}

/// Attempt to parse the HPROF file into an object graph and build a dominator tree.
/// Returns None if parsing fails (graceful fallback to heuristic path).
fn try_build_dominator(
    heap_path: &str,
    retain_field_data: bool,
) -> Option<(ObjectGraph, DominatorTree)> {
    match parse_hprof_file_with_options(heap_path, ParseOptions { retain_field_data }) {
        Ok(graph) => {
            if graph.objects.is_empty() {
                return None;
            }
            let dom = build_dominator_tree(&graph);
            if dom.node_count() == 0 {
                return None;
            }
            Some((graph, dom))
        }
        Err(e) => {
            info!(error = %e, "HPROF object graph parsing failed; falling back to heuristic analysis");
            None
        }
    }
}

/// Produce leak insights from the dominator tree's top retained objects.
fn graph_backed_leaks(
    dom: &DominatorTree,
    graph: &ObjectGraph,
    options: &LeakDetectionOptions,
) -> Vec<LeakInsight> {
    build_leak_suspects(dom, graph, options)
        .into_iter()
        .map(|suspect| leak_insight_from_suspect(&suspect))
        .collect()
}

fn build_leak_suspects(
    dom: &DominatorTree,
    graph: &ObjectGraph,
    options: &LeakDetectionOptions,
) -> Vec<LeakSuspect> {
    let mut suspects = Vec::new();

    for (&obj_id, obj) in &graph.objects {
        let class_name = graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string();
        if !matches_package_filters(&class_name, &options.package_filters) {
            continue;
        }

        let leak_kind = infer_kind_from_class_name(&class_name);
        if !options.leak_types.is_empty() && !options.leak_types.contains(&leak_kind) {
            continue;
        }

        let retained_size = dom.retained_size(obj_id);
        let shallow_size = u64::from(obj.shallow_size);
        let ratio = retained_size as f64 / shallow_size.max(1) as f64;
        let severity = severity_from_size(retained_size);
        if severity < options.min_severity {
            continue;
        }

        let dominated_count = dominated_subtree_count(dom, obj_id);
        let score = retained_size as f64 * (ratio + 1.0_f64).log2();

        suspects.push(LeakSuspect {
            object_id: obj_id,
            class_name,
            shallow_size,
            retained_size,
            ratio,
            is_accumulation_point: ratio > options.accumulation_threshold && dominated_count > 1,
            dominated_count,
            reference_chain: build_reference_chain(dom, graph, obj_id),
            score,
        });
    }

    suspects.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(cmp::Ordering::Equal)
            .then_with(|| b.retained_size.cmp(&a.retained_size))
            .then_with(|| a.class_name.cmp(&b.class_name))
    });
    suspects.truncate(10);
    suspects
}

fn leak_insight_from_suspect(suspect: &LeakSuspect) -> LeakInsight {
    let leak_kind = infer_kind_from_class_name(&suspect.class_name);
    let accumulation_note = if suspect.is_accumulation_point {
        " accumulation point"
    } else {
        ""
    };
    let reference_chain = if suspect.reference_chain.is_empty() {
        String::from("reference chain unavailable")
    } else {
        suspect.reference_chain.join(" -> ")
    };

    LeakInsight {
        id: make_leak_id(&suspect.class_name, leak_kind),
        class_name: suspect.class_name.clone(),
        leak_kind,
        severity: severity_from_size(suspect.retained_size),
        retained_size_bytes: suspect.retained_size,
        shallow_size_bytes: Some(suspect.shallow_size),
        suspect_score: Some(suspect.score),
        instances: suspect.dominated_count + 1,
        description: format!(
            "{} retains {} bytes with shallow size {} (ratio {:.2}, score {:.2}, {} dominated objects,{}). Chain: {}",
            suspect.class_name,
            suspect.retained_size,
            suspect.shallow_size,
            suspect.ratio,
            suspect.score,
            suspect.dominated_count,
            accumulation_note,
            reference_chain
        ),
        provenance: Vec::new(),
    }
}

fn matches_package_filters(class_name: &str, package_filters: &[String]) -> bool {
    if package_filters.is_empty() {
        return true;
    }

    if class_name.contains('.') || class_name.contains('/') {
        return package_filters
            .iter()
            .any(|pkg| class_name.starts_with(pkg));
    }

    true
}

fn dominated_subtree_count(dom: &DominatorTree, root_id: ObjectId) -> u64 {
    let mut count = 0_u64;
    let mut stack = dom.dominated_by(root_id).to_vec();

    while let Some(obj_id) = stack.pop() {
        count += 1;
        stack.extend_from_slice(dom.dominated_by(obj_id));
    }

    count
}

fn build_reference_chain(
    dom: &DominatorTree,
    graph: &ObjectGraph,
    obj_id: ObjectId,
) -> Vec<String> {
    let mut chain = vec![class_name_for_object(graph, obj_id)];
    let mut current = obj_id;
    let mut ancestors = 0_usize;

    while ancestors < 3 {
        let Some(parent_id) = dom.immediate_dominator(current) else {
            break;
        };
        if parent_id == VIRTUAL_ROOT_ID {
            chain.push(String::from("GC Root"));
            break;
        }
        chain.push(class_name_for_object(graph, parent_id));
        current = parent_id;
        ancestors += 1;
    }

    chain.reverse();
    chain
}

fn class_name_for_object(graph: &ObjectGraph, obj_id: ObjectId) -> String {
    graph
        .objects
        .get(&obj_id)
        .and_then(|obj| graph.class_name(obj.class_id))
        .unwrap_or("<unknown>")
        .to_string()
}

fn synthesize_leaks(summary: &HeapSummary, options: &LeakDetectionOptions) -> Vec<LeakInsight> {
    if summary.total_size_bytes == 0 {
        return Vec::new();
    }

    let mut leaks = synthesize_from_class_stats(summary, options);
    if leaks.is_empty() {
        leaks = synthesize_synthetic_leaks(summary, options);
    }
    leaks
}

fn synthesize_from_class_stats(
    summary: &HeapSummary,
    options: &LeakDetectionOptions,
) -> Vec<LeakInsight> {
    let ranked = ranked_class_stats(summary);
    if ranked.is_empty() {
        return Vec::new();
    }

    let mut leaks = Vec::new();
    for class in &ranked {
        if class.total_size_bytes == 0 {
            continue;
        }

        if !options.package_filters.is_empty()
            && class.name.contains('.')
            && !options
                .package_filters
                .iter()
                .any(|pkg| class.name.starts_with(pkg))
        {
            continue;
        }

        let leak_kind = infer_kind_from_class_name(&class.name);
        if !options.leak_types.is_empty() && !options.leak_types.contains(&leak_kind) {
            continue;
        }

        let severity = severity_from_size(class.total_size_bytes);
        if severity < options.min_severity {
            continue;
        }

        leaks.push(build_class_leak(summary, class, leak_kind, severity));

        if options.leak_types.is_empty() && leaks.len() >= 3 {
            break;
        }
    }

    if leaks.is_empty() && !options.leak_types.is_empty() {
        for (idx, leak_kind) in options.leak_types.iter().enumerate() {
            if let Some(class) = ranked.get(idx % ranked.len()) {
                let severity = severity_from_size(class.total_size_bytes);
                if severity < options.min_severity {
                    continue;
                }
                leaks.push(build_class_leak(summary, class, *leak_kind, severity));
            }
        }
    }

    leaks
}

fn synthesize_synthetic_leaks(
    summary: &HeapSummary,
    options: &LeakDetectionOptions,
) -> Vec<LeakInsight> {
    let severity = cmp::max(
        severity_from_size(summary.total_size_bytes),
        severity_from_records(summary),
    );

    if severity < options.min_severity {
        return Vec::new();
    }

    let retained_size_bytes = summary.total_size_bytes / 2;
    let instances = cmp::max(summary.total_objects, 1);
    let description = build_description(summary);
    let leak_types = if options.leak_types.is_empty() {
        vec![infer_leak_kind(summary)]
    } else {
        options.leak_types.clone()
    };

    leak_types
        .into_iter()
        .enumerate()
        .map(|(idx, leak_kind)| {
            let package_hint = package_hint_for(&options.package_filters, idx);
            let class_name = build_class_name(package_hint, leak_kind, idx);
            let leak_id = make_leak_id(&class_name, leak_kind);
            LeakInsight {
                id: leak_id,
                class_name,
                leak_kind,
                severity,
                retained_size_bytes,
                shallow_size_bytes: None,
                suspect_score: None,
                instances,
                description: description.clone(),
                provenance: synthetic_leak_provenance(),
            }
        })
        .collect()
}

fn build_class_name(package_hint: Option<&str>, leak_kind: LeakKind, ordinal: usize) -> String {
    let suffix = format!("{leak_kind:?}");
    if let Some(pkg) = package_hint {
        if ordinal == 0 {
            format!("{pkg}.{suffix}LeakCandidate")
        } else {
            format!("{pkg}.{suffix}LeakCandidate{}", ordinal + 1)
        }
    } else if ordinal == 0 {
        format!("com.example.{suffix}Leak")
    } else {
        format!("com.example.{suffix}Leak{}", ordinal + 1)
    }
}

fn ranked_class_stats(summary: &HeapSummary) -> Vec<&ClassStat> {
    let mut classes: Vec<&ClassStat> = summary.classes.iter().collect();
    classes.sort_by(|a, b| b.total_size_bytes.cmp(&a.total_size_bytes));
    classes
}

fn build_class_leak(
    summary: &HeapSummary,
    class: &ClassStat,
    leak_kind: LeakKind,
    severity: LeakSeverity,
) -> LeakInsight {
    let class_name = class.name.clone();
    let leak_id = make_leak_id(&class_name, leak_kind);
    LeakInsight {
        id: leak_id,
        class_name,
        leak_kind,
        severity,
        retained_size_bytes: class.total_size_bytes,
        shallow_size_bytes: None,
        suspect_score: None,
        instances: cmp::max(class.instances, 1),
        description: describe_class_leak(summary, class),
        provenance: Vec::new(),
    }
}

/// Provenance for the analysis response when analysis is entirely heuristic.
fn heuristic_provenance() -> Vec<ProvenanceMarker> {
    vec![ProvenanceMarker::new(
        ProvenanceKind::Partial,
        "Graph metrics are summary-level preview data; full dominator-based heap analysis is not yet implemented.",
    )]
}

/// Provenance when graph was available but leak filters produced no results,
/// so heuristic leaks were used as fallback.
fn fallback_provenance() -> Vec<ProvenanceMarker> {
    vec![ProvenanceMarker::new(
        ProvenanceKind::Fallback,
        "Graph-backed dominator analysis was available but leak filters produced no results; heuristic fallback was used for leak detection.",
    )]
}

/// Provenance attached to every leak produced by the synthetic-fallback path.
fn synthetic_leak_provenance() -> Vec<ProvenanceMarker> {
    vec![
        ProvenanceMarker::new(
            ProvenanceKind::Synthetic,
            "Leak candidate was synthesized from aggregate heap summary data.",
        ),
        ProvenanceMarker::new(
            ProvenanceKind::Fallback,
            "No class-level leak candidates were available; summary-derived fallback was used.",
        ),
    ]
}

fn package_hint_for(packages: &[String], ordinal: usize) -> Option<&str> {
    if packages.is_empty() {
        None
    } else {
        let index = ordinal % packages.len();
        Some(packages[index].as_str())
    }
}

fn make_leak_id(class_name: &str, kind: LeakKind) -> String {
    let mut hasher = DefaultHasher::new();
    class_name.hash(&mut hasher);
    kind.hash(&mut hasher);
    format!("{}::{:08x}", class_name, hasher.finish())
}

fn infer_kind_from_class_name(name: &str) -> LeakKind {
    let lower = name.to_ascii_lowercase();
    if lower.contains("cache") {
        LeakKind::Cache
    } else if lower.contains("thread") || lower.contains("executor") {
        LeakKind::Thread
    } else if lower.contains("http") || lower.contains("response") {
        LeakKind::HttpResponse
    } else if lower.contains("loader") {
        LeakKind::ClassLoader
    } else if lower.contains("listener") || lower.contains("handler") {
        LeakKind::Listener
    } else if lower.contains("array")
        || lower.contains("collection")
        || lower.contains("map")
        || lower.contains("list")
    {
        LeakKind::Collection
    } else if lower.contains("coroutine") || lower.contains("continuation") {
        LeakKind::Coroutine
    } else {
        LeakKind::Unknown
    }
}

fn describe_class_leak(summary: &HeapSummary, class: &ClassStat) -> String {
    let heap_gb = summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let retained_mb = class.total_size_bytes as f64 / (1024.0 * 1024.0);
    let mut parts = vec![format!(
        "{} retains {:.2} MB (~{:.1}% of {:.2} GB heap) across {} instances",
        class.name,
        retained_mb,
        class.percentage,
        heap_gb,
        cmp::max(class.instances, 1)
    )];

    if let Some(record) = summary.record_stats.first() {
        parts.push(format!(
            "Dominant record: {} (tag 0x{:02X})",
            record.name, record.tag
        ));
    }

    parts.join(" | ")
}

fn severity_from_size(bytes: u64) -> LeakSeverity {
    const GB: u64 = 1024 * 1024 * 1024;
    match bytes {
        b if b >= 8 * GB => LeakSeverity::Critical,
        b if b >= 4 * GB => LeakSeverity::High,
        b if b >= 2 * GB => LeakSeverity::Medium,
        _ => LeakSeverity::Low,
    }
}

fn infer_leak_kind(summary: &HeapSummary) -> LeakKind {
    if let Some(header) = &summary.header {
        if header.format.contains("profile") {
            return LeakKind::Thread;
        }
    }
    if let Some(stat) = summary.record_stats.first() {
        return match stat.tag {
            0x21 | 0x22 => LeakKind::Collection,
            0x23 => LeakKind::HttpResponse,
            0x2C => LeakKind::ClassLoader,
            0x0D | 0x0E => LeakKind::Cache,
            _ => LeakKind::Cache,
        };
    }
    LeakKind::Cache
}

fn build_description(summary: &HeapSummary) -> String {
    let mut details = vec![format!(
        "Heap file size: {:.2} GB",
        summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    )];

    if let Some(header) = &summary.header {
        details.push(format!(
            "HPROF format `{}` (id size {} bytes)",
            header.format.trim(),
            header.identifier_size
        ));
    }

    if let Some(record) = summary.record_stats.first() {
        details.push(format!(
            "Dominant record: {} (tag 0x{:02X}, {} entries, {:.2} MB)",
            record.name,
            record.tag,
            record.count,
            record.bytes as f64 / (1024.0 * 1024.0)
        ));
    }

    details.join(" | ")
}

fn severity_from_records(summary: &HeapSummary) -> LeakSeverity {
    summary
        .record_stats
        .first()
        .map(|record| severity_from_size(record.bytes))
        .unwrap_or(LeakSeverity::Low)
}

fn build_heap_diff(before: HeapSummary, after: HeapSummary) -> HeapDiff {
    let changed_classes = collect_changed_classes(&before, &after);
    HeapDiff {
        before: before.heap_path,
        after: after.heap_path,
        delta_bytes: after.total_size_bytes as i64 - before.total_size_bytes as i64,
        delta_objects: after.total_objects as i64 - before.total_objects as i64,
        changed_classes,
        class_diff: None,
    }
}

fn compute_class_level_diff(
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
) -> Vec<ClassLevelDelta> {
    let before = collect_class_level_stats(before_graph, before_dom);
    let after = collect_class_level_stats(after_graph, after_dom);
    let mut merged: HashMap<String, ClassLevelDelta> = HashMap::new();

    for (class_name, stats) in before {
        merged.insert(
            class_name.clone(),
            ClassLevelDelta {
                class_name,
                before_instances: stats.instances,
                after_instances: 0,
                before_shallow_bytes: stats.shallow_bytes,
                after_shallow_bytes: 0,
                before_retained_bytes: stats.retained_bytes,
                after_retained_bytes: 0,
            },
        );
    }

    for (class_name, stats) in after {
        let entry = merged.entry(class_name.clone()).or_insert(ClassLevelDelta {
            class_name,
            before_instances: 0,
            after_instances: 0,
            before_shallow_bytes: 0,
            after_shallow_bytes: 0,
            before_retained_bytes: 0,
            after_retained_bytes: 0,
        });
        entry.after_instances = stats.instances;
        entry.after_shallow_bytes = stats.shallow_bytes;
        entry.after_retained_bytes = stats.retained_bytes;
    }

    let mut deltas: Vec<ClassLevelDelta> = merged
        .into_values()
        .filter(|entry| {
            entry.before_instances != entry.after_instances
                || entry.before_shallow_bytes != entry.after_shallow_bytes
                || entry.before_retained_bytes != entry.after_retained_bytes
        })
        .collect();

    deltas.sort_by(|a, b| {
        let delta_a = (a.after_retained_bytes as i128 - a.before_retained_bytes as i128).abs();
        let delta_b = (b.after_retained_bytes as i128 - b.before_retained_bytes as i128).abs();
        delta_b
            .cmp(&delta_a)
            .then_with(|| a.class_name.cmp(&b.class_name))
    });
    deltas.truncate(20);
    deltas
}

#[derive(Default)]
struct ClassLevelStats {
    instances: u64,
    shallow_bytes: u64,
    retained_bytes: u64,
}

fn collect_class_level_stats(
    graph: &ObjectGraph,
    dom: &DominatorTree,
) -> HashMap<String, ClassLevelStats> {
    let mut stats: HashMap<String, ClassLevelStats> = HashMap::new();

    for (&obj_id, obj) in &graph.objects {
        let class_name = graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string();
        let entry = stats.entry(class_name).or_default();
        entry.instances += 1;
        entry.shallow_bytes += u64::from(obj.shallow_size);
        entry.retained_bytes += dom.retained_size(obj_id);
    }

    stats
}

fn collect_changed_classes(before: &HeapSummary, after: &HeapSummary) -> Vec<ClassDelta> {
    if !before.classes.is_empty() || !after.classes.is_empty() {
        return diff_named_totals(
            before
                .classes
                .iter()
                .map(|class| (class.name.clone(), class.total_size_bytes)),
            after
                .classes
                .iter()
                .map(|class| (class.name.clone(), class.total_size_bytes)),
        );
    }

    diff_named_totals(
        before
            .record_stats
            .iter()
            .map(|record| (record.name.clone(), record.bytes)),
        after
            .record_stats
            .iter()
            .map(|record| (record.name.clone(), record.bytes)),
    )
}

fn diff_named_totals<I, J>(before: I, after: J) -> Vec<ClassDelta>
where
    I: IntoIterator<Item = (String, u64)>,
    J: IntoIterator<Item = (String, u64)>,
{
    let mut deltas: HashMap<String, (u64, u64)> = HashMap::new();

    for (name, bytes) in before {
        let entry = deltas.entry(name).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(bytes);
    }

    for (name, bytes) in after {
        let entry = deltas.entry(name).or_insert((0, 0));
        entry.1 = entry.1.saturating_add(bytes);
    }

    let mut changed: Vec<ClassDelta> = deltas
        .into_iter()
        .filter(|(_, (before_bytes, after_bytes))| before_bytes != after_bytes)
        .map(|(name, (before_bytes, after_bytes))| ClassDelta {
            name,
            before_bytes,
            after_bytes,
        })
        .collect();

    changed.sort_by(|a, b| {
        let delta_a = (a.after_bytes as i64 - a.before_bytes as i64).abs();
        let delta_b = (b.after_bytes as i64 - b.before_bytes as i64).abs();
        delta_b.cmp(&delta_a).then_with(|| a.name.cmp(&b.name))
    });

    changed.truncate(10);
    changed
}

// Additional helper methods live closer to their call sites for clarity.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{ClassInfo, ClassStat, RecordStat};
    use std::time::SystemTime;

    fn summary_with_size(bytes: u64) -> HeapSummary {
        HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: bytes,
            classes: Vec::new(),
            generated_at: SystemTime::UNIX_EPOCH,
            header: None,
            total_records: 1,
            record_stats: vec![RecordStat {
                tag: 0x21,
                name: "INSTANCE_DUMP".into(),
                count: 1,
                bytes,
            }],
        }
    }

    fn summary_with_classes(
        path: &str,
        total_size_bytes: u64,
        total_objects: u64,
        classes: Vec<ClassStat>,
        record_stats: Vec<RecordStat>,
    ) -> HeapSummary {
        HeapSummary {
            heap_path: path.into(),
            total_objects,
            total_size_bytes,
            classes,
            generated_at: SystemTime::UNIX_EPOCH,
            header: None,
            total_records: record_stats.len() as u64,
            record_stats,
        }
    }

    fn class_stat(name: &str, instances: u64, bytes: u64, percentage: f32) -> ClassStat {
        ClassStat {
            name: name.into(),
            instances,
            total_size_bytes: bytes,
            percentage,
        }
    }

    #[test]
    fn filters_out_leaks_below_min_severity() {
        let summary = summary_with_size(512 * 1024 * 1024); // ~0.5 GB => Low severity.
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::High,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert!(leaks.is_empty());
    }

    #[test]
    fn emits_one_entry_per_requested_kind() {
        let summary = summary_with_size(9 * 1024 * 1024 * 1024); // > 8 GB => Critical severity.
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: vec!["com.example".into()],
            leak_types: vec![LeakKind::Cache, LeakKind::Thread],
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert_eq!(2, leaks.len());
        let kinds: Vec<LeakKind> = leaks.iter().map(|leak| leak.leak_kind).collect();
        assert_eq!(kinds, vec![LeakKind::Cache, LeakKind::Thread]);
        assert!(leaks.iter().all(|leak| leak.severity >= LeakSeverity::Low));
    }

    #[test]
    fn cycles_package_hints_across_multiple_entries() {
        let summary = summary_with_size(9 * 1024 * 1024 * 1024);
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: vec!["com.one".into(), "org.two".into()],
            leak_types: vec![LeakKind::Cache, LeakKind::Thread, LeakKind::Listener],
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert_eq!(3, leaks.len());
        assert!(leaks[0].class_name.starts_with("com.one."));
        assert!(leaks[1].class_name.starts_with("org.two."));
        assert!(leaks[2].class_name.starts_with("com.one."));
    }

    #[test]
    fn synthetic_leaks_carry_provenance_markers() {
        let summary = summary_with_size(9 * 1024 * 1024 * 1024);
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_synthetic_leaks(&summary, &options);
        assert!(!leaks.is_empty());
        assert!(
            leaks[0]
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Synthetic),
            "synthetic leak must be labeled Synthetic"
        );
        assert!(
            leaks[0]
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Fallback),
            "synthetic leak must be labeled Fallback"
        );
    }

    #[test]
    fn class_stat_leaks_have_empty_provenance() {
        let summary = summary_with_classes(
            "heap.hprof",
            4 * 1024 * 1024 * 1024,
            10_000,
            vec![class_stat("com.example.Cache", 5_000, 1_000_000_000, 25.0)],
            Vec::new(),
        );
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert!(!leaks.is_empty());
        assert!(
            leaks[0].provenance.is_empty(),
            "class-stat leaks must have empty provenance"
        );
    }

    #[test]
    fn heuristic_provenance_is_partial_preview() {
        let markers = heuristic_provenance();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].kind, ProvenanceKind::Partial);
        assert!(markers[0]
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("preview"));
    }

    #[test]
    fn class_stats_drive_leaks() {
        let summary = summary_with_classes(
            "heap.hprof",
            4 * 1024 * 1024 * 1024,
            10_000,
            vec![
                class_stat("com.example.UserSessionCache", 5_000, 1_000_000_000, 25.0),
                class_stat("java.lang.String[]", 2_000, 512_000_000, 12.0),
            ],
            Vec::new(),
        );
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert!(!leaks.is_empty());
        assert_eq!("com.example.UserSessionCache", leaks[0].class_name);
        assert_eq!(LeakKind::Cache, leaks[0].leak_kind);
        assert!(leaks[0].description.contains("retains"));
    }

    #[test]
    fn respects_package_filters_for_real_classes() {
        let summary = summary_with_classes(
            "heap.hprof",
            4 * 1024 * 1024 * 1024,
            10_000,
            vec![
                class_stat("com.example.UserSessionCache", 5_000, 1_000_000_000, 25.0),
                class_stat("org.demo.Listener", 3_000, 700_000_000, 18.0),
            ],
            Vec::new(),
        );
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: vec!["org.demo".into()],
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert_eq!(1, leaks.len());
        assert_eq!("org.demo.Listener", leaks[0].class_name);
    }

    #[test]
    fn heap_diff_prefers_class_stats() {
        let before = summary_with_classes(
            "before.hprof",
            1_000,
            100,
            vec![ClassStat {
                name: "com.example.Cache".into(),
                instances: 10,
                total_size_bytes: 400,
                percentage: 40.0,
            }],
            Vec::new(),
        );

        let after = summary_with_classes(
            "after.hprof",
            1_600,
            120,
            vec![
                ClassStat {
                    name: "com.example.Cache".into(),
                    instances: 12,
                    total_size_bytes: 800,
                    percentage: 50.0,
                },
                ClassStat {
                    name: "com.example.New".into(),
                    instances: 4,
                    total_size_bytes: 400,
                    percentage: 25.0,
                },
            ],
            Vec::new(),
        );

        let diff = build_heap_diff(before, after);
        assert_eq!(600, diff.delta_bytes);
        assert_eq!(20, diff.delta_objects);
        assert_eq!(2, diff.changed_classes.len());
        assert_eq!("com.example.Cache", diff.changed_classes[0].name);
        assert_eq!(800, diff.changed_classes[0].after_bytes);
    }

    #[test]
    fn heap_diff_falls_back_to_record_stats() {
        let before = summary_with_classes(
            "before.hprof",
            500,
            5,
            Vec::new(),
            vec![RecordStat {
                tag: 0x21,
                name: "INSTANCE_DUMP".into(),
                count: 1,
                bytes: 200,
            }],
        );

        let after = summary_with_classes(
            "after.hprof",
            250,
            15,
            Vec::new(),
            vec![RecordStat {
                tag: 0x21,
                name: "INSTANCE_DUMP".into(),
                count: 1,
                bytes: 50,
            }],
        );

        let diff = build_heap_diff(before, after);
        assert_eq!(-250, diff.delta_bytes);
        assert_eq!(10, diff.delta_objects);
        assert_eq!(1, diff.changed_classes.len());
        assert_eq!(50, diff.changed_classes[0].after_bytes);
    }

    // -- graph-backed leak tests -------------------------------------------

    use crate::graph::build_dominator_tree;
    use crate::hprof::{GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind};

    /// Helper: build a programmatic ObjectGraph from a compact description.
    fn make_test_graph(objects: &[(u64, u64, u32, &[u64])], gc_roots: &[u64]) -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        for &(id, class_id, size, refs) in objects {
            graph.objects.insert(
                id,
                HeapObject {
                    id,
                    class_id,
                    shallow_size: size,
                    references: refs.to_vec(),
                    field_data: Vec::new(),
                    kind: ObjectKind::Instance,
                },
            );
        }
        for &root_id in gc_roots {
            graph.gc_roots.push(GcRoot {
                object_id: root_id,
                root_type: GcRootType::StickyClass,
            });
        }
        graph
    }

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
    }

    #[test]
    fn graph_backed_leaks_uses_retained_sizes() {
        // Root(1) → A(2) → B(3); shallow sizes: 10, 20, 30
        // Retained: 1=60, 2=50, 3=30
        let obj_graph = make_test_graph(
            &[
                (1, 0x100, 10, &[2]),
                (2, 0x100, 20, &[3]),
                (3, 0x100, 30, &[]),
            ],
            &[1],
        );
        let mut obj_graph = obj_graph;
        add_class(&mut obj_graph, 0x100, "com.example.CacheNode");
        let dom = build_dominator_tree(&obj_graph);
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = graph_backed_leaks(&dom, &obj_graph, &options);
        assert!(!leaks.is_empty());
        // The top leak should have retained_size matching the dominator tree
        assert_eq!(leaks[0].retained_size_bytes, 60);
        assert_eq!(leaks[1].retained_size_bytes, 50);
        assert_eq!(leaks[2].retained_size_bytes, 30);
        assert!(leaks[0].suspect_score.is_some());
        assert_eq!(leaks[0].shallow_size_bytes, Some(10));
    }

    #[test]
    fn graph_backed_leaks_respects_package_filter() {
        // All objects use class_id 0x100 which resolves to None → "<unknown>".
        // "<unknown>" does not contain '.', so package filter should NOT exclude it.
        let mut obj_graph = make_test_graph(&[(1, 0x100, 10, &[2]), (2, 0x100, 20, &[])], &[1]);
        add_class(&mut obj_graph, 0x100, "<unknown>");
        let dom = build_dominator_tree(&obj_graph);
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: vec!["com.example".into()],
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = graph_backed_leaks(&dom, &obj_graph, &options);
        // "<unknown>" doesn't contain '.', so package filter doesn't apply
        assert_eq!(leaks.len(), 2);
    }

    #[test]
    fn graph_backed_leaks_have_empty_provenance() {
        let mut obj_graph = make_test_graph(&[(1, 0x100, 10, &[])], &[1]);
        add_class(&mut obj_graph, 0x100, "com.example.CacheRoot");
        let dom = build_dominator_tree(&obj_graph);
        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = graph_backed_leaks(&dom, &obj_graph, &options);
        assert!(!leaks.is_empty());
        assert!(
            leaks[0].provenance.is_empty(),
            "graph-backed leaks must have empty provenance (real data)"
        );
    }

    #[test]
    fn fallback_provenance_is_fallback_kind() {
        let markers = fallback_provenance();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].kind, ProvenanceKind::Fallback);
        assert!(markers[0]
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("heuristic fallback"));
    }

    #[tokio::test]
    async fn detect_leaks_tries_graph_path() {
        use crate::test_fixtures::{HeapDumpBuilder, HprofBuilder};
        use std::io::Write;

        // Build an HPROF where the GC root IS an object in the heap.
        let mut builder = HprofBuilder::new(4);
        builder
            .add_string(1, "java/lang/Object")
            .add_string(2, "com/example/BigCache")
            .add_load_class(1, 0x100, 0, 1)
            .add_load_class(2, 0x200, 0, 2);

        let mut heap = HeapDumpBuilder::new(4);
        // Class dumps
        heap.add_class_dump(0x100, 0, 0, &[])
            .add_class_dump(0x200, 0x100, 0, &[]);
        // Root object that is also an instance
        heap.add_gc_root_java_frame(0x1000, 1, 0)
            .add_instance_dump(0x1000, 0x200, &[])
            .add_instance_dump(0x2000, 0x200, &[]);

        builder.add_heap_dump(heap.build());

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&builder.build()).unwrap();
        file.flush().unwrap();

        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let leaks = detect_leaks(file.path().to_str().unwrap(), options)
            .await
            .unwrap();
        // The fixture produces a valid HPROF with objects and a GC root that
        // is also in the objects map, so graph-backed path should succeed.
        assert!(!leaks.is_empty(), "detect_leaks should return results");
        // Graph-backed leaks have empty provenance (real data)
        for leak in &leaks {
            assert!(
                !leak
                    .provenance
                    .iter()
                    .any(|m| m.kind == ProvenanceKind::Synthetic),
                "graph-backed leaks must not have Synthetic provenance"
            );
        }
    }

    #[tokio::test]
    async fn detect_leaks_falls_back_to_heuristic() {
        use std::io::Write;

        // Write invalid HPROF data — graph path will fail, heuristic should kick in
        let mut file = tempfile::NamedTempFile::new().unwrap();
        // Valid HPROF header but no heap dump records
        file.write_all(b"JAVA PROFILE 1.0.2\0").unwrap();
        file.write_all(&4u32.to_be_bytes()).unwrap();
        file.write_all(&0u64.to_be_bytes()).unwrap();
        file.flush().unwrap();

        let options = LeakDetectionOptions {
            min_severity: LeakSeverity::Low,
            package_filters: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        };

        let result = detect_leaks(file.path().to_str().unwrap(), options).await;
        // Should succeed via heuristic path (may produce empty results for tiny files)
        assert!(result.is_ok(), "heuristic fallback should not error");
    }

    #[test]
    fn suspect_scoring_orders_by_score() {
        let mut obj_graph = make_test_graph(
            &[
                (1, 0x100, 100, &[2]),
                (2, 0x200, 10, &[3, 4]),
                (3, 0x300, 200, &[]),
                (4, 0x300, 200, &[]),
            ],
            &[1],
        );
        add_class(&mut obj_graph, 0x100, "com.example.Root");
        add_class(&mut obj_graph, 0x200, "com.example.Accumulator");
        add_class(&mut obj_graph, 0x300, "com.example.Payload");
        let dom = build_dominator_tree(&obj_graph);
        let suspects = build_leak_suspects(
            &dom,
            &obj_graph,
            &LeakDetectionOptions::new(LeakSeverity::Low),
        );

        assert!(!suspects.is_empty());
        assert_eq!(suspects[0].class_name, "com.example.Accumulator");
        assert!(suspects[0].score > suspects[1].score);
    }

    #[test]
    fn suspect_marks_accumulation_points() {
        let mut obj_graph = make_test_graph(
            &[
                (1, 0x100, 64, &[2]),
                (2, 0x200, 4, &[3, 4]),
                (3, 0x300, 128, &[]),
                (4, 0x300, 128, &[]),
            ],
            &[1],
        );
        add_class(&mut obj_graph, 0x100, "com.example.Root");
        add_class(&mut obj_graph, 0x200, "com.example.BufferOwner");
        add_class(&mut obj_graph, 0x300, "com.example.Payload");
        let dom = build_dominator_tree(&obj_graph);
        let suspects = build_leak_suspects(
            &dom,
            &obj_graph,
            &LeakDetectionOptions::new(LeakSeverity::Low),
        );

        let accumulation = suspects
            .iter()
            .find(|suspect| suspect.class_name == "com.example.BufferOwner")
            .unwrap();
        assert!(accumulation.is_accumulation_point);
        assert!(accumulation.ratio > 10.0);
        assert!(accumulation.dominated_count > 1);
    }

    #[test]
    fn suspect_extracts_reference_chain() {
        let mut obj_graph = make_test_graph(
            &[
                (1, 0x100, 64, &[2]),
                (2, 0x200, 32, &[3]),
                (3, 0x300, 16, &[]),
            ],
            &[1],
        );
        add_class(&mut obj_graph, 0x100, "com.example.Root");
        add_class(&mut obj_graph, 0x200, "com.example.Manager");
        add_class(&mut obj_graph, 0x300, "com.example.Leak");
        let dom = build_dominator_tree(&obj_graph);
        let suspects = build_leak_suspects(
            &dom,
            &obj_graph,
            &LeakDetectionOptions::new(LeakSeverity::Low),
        );

        let leak = suspects
            .iter()
            .find(|suspect| suspect.class_name == "com.example.Leak")
            .unwrap();
        assert_eq!(
            leak.reference_chain,
            vec![
                "GC Root",
                "com.example.Root",
                "com.example.Manager",
                "com.example.Leak"
            ]
        );
    }

    #[test]
    fn class_level_diff_computes_graph_deltas() {
        let mut before_graph = make_test_graph(&[(1, 0x100, 10, &[2]), (2, 0x200, 20, &[])], &[1]);
        add_class(&mut before_graph, 0x100, "com.example.Root");
        add_class(&mut before_graph, 0x200, "com.example.Cache");
        let before_dom = build_dominator_tree(&before_graph);

        let mut after_graph = make_test_graph(
            &[
                (1, 0x100, 10, &[2, 3]),
                (2, 0x200, 25, &[]),
                (3, 0x200, 25, &[]),
            ],
            &[1],
        );
        add_class(&mut after_graph, 0x100, "com.example.Root");
        add_class(&mut after_graph, 0x200, "com.example.Cache");
        let after_dom = build_dominator_tree(&after_graph);

        let diff = compute_class_level_diff(&before_graph, &before_dom, &after_graph, &after_dom);

        assert_eq!(diff.len(), 2);
        let cache = diff
            .iter()
            .find(|entry| entry.class_name == "com.example.Cache")
            .unwrap();
        assert_eq!(cache.before_instances, 1);
        assert_eq!(cache.after_instances, 2);
        assert_eq!(cache.before_shallow_bytes, 20);
        assert_eq!(cache.after_shallow_bytes, 50);
        assert_eq!(cache.before_retained_bytes, 20);
        assert_eq!(cache.after_retained_bytes, 50);
    }
}
