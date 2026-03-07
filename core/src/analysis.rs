use crate::{
    ai::{generate_ai_insights, AiInsights},
    config::{AnalysisConfig, AppConfig},
    errors::{CoreError, CoreResult},
    graph::{summarize_graph, GraphMetrics},
    heap::{parse_heap, ClassDelta, ClassStat, HeapDiff, HeapParseJob, HeapSummary},
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
}

/// High-level request for the multi-stage `analyze` workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub heap_path: String,
    pub config: AppConfig,
    pub leak_options: LeakDetectionOptions,
    pub enable_ai: bool,
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
    /// Provenance markers for the response as a whole (e.g. partial / preview).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProvenanceMarker>,
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
        provenance: response_level_provenance(),
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

    Ok(build_heap_diff(before_summary, after_summary))
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

impl From<&AnalysisConfig> for LeakDetectionOptions {
    fn from(value: &AnalysisConfig) -> Self {
        let mut options = LeakDetectionOptions::new(value.min_severity);
        options.package_filters = value.packages.clone();
        options.leak_types = value.leak_types.clone();
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
        }
    }
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
        instances: cmp::max(class.instances, 1),
        description: describe_class_leak(summary, class),
        provenance: Vec::new(),
    }
}

/// Provenance for the analysis response as a whole.
fn response_level_provenance() -> Vec<ProvenanceMarker> {
    vec![ProvenanceMarker::new(
        ProvenanceKind::Partial,
        "Graph metrics are summary-level preview data; full dominator-based heap analysis is not yet implemented.",
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
    }
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
    use crate::heap::{ClassStat, RecordStat};
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
        };

        let leaks = synthesize_leaks(&summary, &options);
        assert!(!leaks.is_empty());
        assert!(
            leaks[0].provenance.is_empty(),
            "class-stat leaks must have empty provenance"
        );
    }

    #[test]
    fn response_provenance_is_partial_preview() {
        let markers = response_level_provenance();
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
}
