pub mod class;
pub mod object;

use std::collections::HashMap;

use crate::{
    errors::{CoreError, CoreResult},
    graph::build_dominator_tree,
    hprof::{
        parse_heap, parse_hprof_file_with_options, ClassDelta, HeapDiff, HeapParseJob, HeapSummary,
        ParseOptions,
    },
};

pub use class::compute_class_level_diff;
pub use object::{
    DiffMode, IdentityStrategy, MatchQuality, ObjectDelta, ObjectDeltaKind, ObjectDiffReport,
    ObjectDiffTotals, ObjectFingerprint, Risk,
};

#[derive(Debug, Clone)]
pub struct DiffRequest {
    pub before_path: String,
    pub after_path: String,
    pub mode: DiffMode,
    pub identity_strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub min_retained_bytes: u64,
    pub retained_change_threshold: u64,
    pub top_n: usize,
    pub retain_field_data: bool,
}

impl DiffRequest {
    pub(crate) fn class(before_path: &str, after_path: &str) -> Self {
        Self {
            before_path: before_path.into(),
            after_path: after_path.into(),
            mode: DiffMode::Class,
            identity_strategy: IdentityStrategy::default(),
            retained_bucket_bits: 10,
            min_retained_bytes: object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
            retained_change_threshold: object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
            top_n: object::types::DEFAULT_OBJECT_DIFF_TOP_N,
            retain_field_data: false,
        }
    }
}

pub enum DiffResult {
    Class(HeapDiff),
    Object(HeapDiff),
}

pub async fn run_diff(request: DiffRequest) -> CoreResult<DiffResult> {
    match request.mode {
        DiffMode::Class => Ok(DiffResult::Class(
            crate::analysis::engine::diff_heaps_class(&request.before_path, &request.after_path)
                .await?,
        )),
        DiffMode::Object => Ok(DiffResult::Object(run_object_diff(request).await?)),
    }
}

async fn run_object_diff(request: DiffRequest) -> CoreResult<HeapDiff> {
    if request.identity_strategy == IdentityStrategy::FullFingerprint && !request.retain_field_data
    {
        return Err(diff_feature_unavailable(
            "feature_unavailable_without_field_data",
            "full-fingerprint requires --retain-field-data",
        ));
    }

    let mut diff = build_heap_diff_summary(&request.before_path, &request.after_path)?;

    let (before_graph, before_dom) =
        load_object_diff_graph(&request.before_path, request.retain_field_data)?;
    let (after_graph, after_dom) =
        load_object_diff_graph(&request.after_path, request.retain_field_data)?;

    diff.class_diff = Some(crate::diff::class::compute_class_level_diff(
        &before_graph,
        &before_dom,
        &after_graph,
        &after_dom,
    ));

    let report = object::engine::diff_object_graphs_with_limit(
        &before_graph,
        &before_dom,
        &after_graph,
        &after_dom,
        object::engine::ObjectDiffEngineConfig {
            strategy: request.identity_strategy,
            bucket_bits: request.retained_bucket_bits,
            min_retained_bytes: request.min_retained_bytes,
            retained_change_threshold: request.retained_change_threshold,
            top_n: request.top_n,
            max_fingerprints: max_object_diff_fingerprints(),
        },
    )
    .map_err(|error| error.to_core_error())?;

    diff.object_diff = Some(report);

    Ok(diff)
}

fn build_heap_diff_summary(before_path: &str, after_path: &str) -> CoreResult<HeapDiff> {
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

    Ok(HeapDiff {
        before: before_summary.heap_path.clone(),
        after: after_summary.heap_path.clone(),
        delta_bytes: after_summary.total_size_bytes as i64 - before_summary.total_size_bytes as i64,
        delta_objects: after_summary.total_objects as i64 - before_summary.total_objects as i64,
        changed_classes: collect_changed_classes(&before_summary, &after_summary),
        class_diff: None,
        object_diff: None,
    })
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

    changed.sort_by_key(|entry| std::cmp::Reverse(entry.after_bytes.abs_diff(entry.before_bytes)));
    changed.truncate(5);
    changed
}

fn load_object_diff_graph(
    heap_path: &str,
    retain_field_data: bool,
) -> CoreResult<(crate::hprof::ObjectGraph, crate::graph::DominatorTree)> {
    let graph = parse_hprof_file_with_options(heap_path, ParseOptions { retain_field_data })?;
    if graph.objects.is_empty() {
        return Err(diff_feature_unavailable(
            "feature_unavailable_in_overview_mode",
            format!(
                "object diff requires deep-mode object graph data for '{heap_path}'; the parsed graph was empty"
            ),
        ));
    }

    let dom = build_dominator_tree(&graph);
    if dom.node_count() == 0 {
        return Err(diff_feature_unavailable(
            "feature_unavailable_in_overview_mode",
            format!(
                "object diff requires dominator data for '{heap_path}'; no dominator tree could be built"
            ),
        ));
    }

    Ok((graph, dom))
}

fn diff_feature_unavailable(code: &str, message: impl Into<String>) -> CoreError {
    CoreError::Unsupported(format!("{code}: {}", message.into()))
}

fn max_object_diff_fingerprints() -> usize {
    std::env::var("MNEMOSYNE_OBJECT_DIFF_MAX_FINGERPRINTS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(object::engine::MAX_OBJECT_DIFF_FINGERPRINTS)
}
