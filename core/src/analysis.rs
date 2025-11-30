use crate::{
    ai::{generate_ai_insights, AiInsights},
    config::AppConfig,
    errors::{CoreError, CoreResult},
    graph::{summarize_graph, GraphMetrics},
    heap::{parse_heap, HeapDiff, HeapParseJob, HeapSummary},
};
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};
use tracing::info;

/// Options that drive leak detection heuristics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LeakDetectionOptions {
    pub min_severity: LeakSeverity,
    pub package_filter: Option<String>,
    pub leak_types: Vec<LeakKind>,
}

/// High-level request for the multi-stage `analyze` workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub heap_path: String,
    pub config: AppConfig,
    pub leak_options: LeakDetectionOptions,
    pub enable_ai: bool,
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
}

/// Machine- and human-readable leak record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakInsight {
    pub id: String,
    pub class_name: String,
    pub leak_kind: LeakKind,
    pub severity: LeakSeverity,
    pub retained_size_bytes: u64,
    pub instances: u64,
    pub description: String,
}

/// Enumeration of leak flavors supported by the orchestrator.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, Hash)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LeakSeverity {
    Low,
    Medium,
    #[default]
    High,
    Critical,
}

/// Execute the core analysis workflow. Currently a stub that wires together
/// the major phases and returns placeholder data.
pub async fn analyze_heap(request: AnalyzeRequest) -> CoreResult<AnalyzeResponse> {
    info!(heap = %request.heap_path, "starting analysis pipeline");
    let start = Instant::now();

    let parse_job = HeapParseJob {
        path: request.heap_path.clone(),
        include_strings: false,
        max_objects: request.config.parser.max_objects,
    };
    let summary = parse_heap(&parse_job)?;

    let graph = summarize_graph(&summary);
    let leaks = synthesize_leaks(&summary, &request.leak_options);
    let ai = if request.enable_ai || request.config.ai.enabled {
        info!(model = %request.config.ai.model, "generating synthetic AI insights");
        Some(generate_ai_insights(&summary, &leaks, &request.config.ai))
    } else {
        None
    };

    Ok(AnalyzeResponse {
        summary,
        leaks,
        recommendations: vec![
            "Integrate dominator-based retained size computation".into(),
            "Add AI insights pipeline".into(),
        ],
        elapsed: start.elapsed(),
        graph,
        ai,
    })
}

/// Compare two heap snapshots and emit a structured diff.
pub async fn diff_heaps(before: &str, after: &str) -> CoreResult<HeapDiff> {
    info!(%before, %after, "computing heap diff (stub)");
    Ok(HeapDiff::placeholder(before, after))
}

/// Kick off leak detection without the rest of the analysis pipeline.
pub async fn detect_leaks(
    heap_path: &str,
    options: LeakDetectionOptions,
) -> CoreResult<Vec<LeakInsight>> {
    info!(%heap_path, ?options, "detecting leaks via heuristic engine");
    let parse_job = HeapParseJob {
        path: heap_path.into(),
        include_strings: false,
        max_objects: None,
    };
    let summary = parse_heap(&parse_job)?;
    Ok(synthesize_leaks(&summary, &options))
}

impl LeakDetectionOptions {
    pub fn new(min_severity: LeakSeverity) -> Self {
        Self {
            min_severity,
            package_filter: None,
            leak_types: Vec::new(),
        }
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

fn synthesize_leaks(summary: &HeapSummary, options: &LeakDetectionOptions) -> Vec<LeakInsight> {
    if summary.total_size_bytes == 0 {
        return Vec::new();
    }

    let leak_kind = options
        .leak_types
        .first()
        .copied()
        .unwrap_or_else(|| infer_leak_kind(summary));

    let severity = cmp::max(
        cmp::max(
            severity_from_size(summary.total_size_bytes),
            severity_from_records(summary),
        ),
        options.min_severity,
    );
    let retained_size_bytes = summary.total_size_bytes / 2;
    let instances = cmp::max(summary.total_objects, 1);
    let description = build_description(summary);
    let class_name = options
        .package_filter
        .as_ref()
        .map(|pkg| format!("{pkg}.LeakCandidate"))
        .unwrap_or_else(|| "com.example.MemoryKeeper".into());

    let leak_id = make_leak_id(&class_name, leak_kind);

    vec![LeakInsight {
        id: leak_id,
        class_name,
        leak_kind,
        severity,
        retained_size_bytes,
        instances,
        description,
    }]
}

fn make_leak_id(class_name: &str, kind: LeakKind) -> String {
    let mut hasher = DefaultHasher::new();
    class_name.hash(&mut hasher);
    kind.hash(&mut hasher);
    format!("{}::{:08x}", class_name, hasher.finish())
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

// Additional helper methods live closer to their call sites for clarity.
