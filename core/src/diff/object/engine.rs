use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    errors::CoreError,
    graph::DominatorTree,
    hprof::{ClassId, ObjectGraph, ObjectId},
};

use super::{
    dominator_chain,
    fingerprint::{stable_class_name_id, ObjectFingerprint},
    match_quality::compute_match_quality,
    reference_chain::build_reference_chain,
    types::{
        IdentityStrategy, ObjectDelta, ObjectDeltaKind, ObjectDiffReport,
        DEFAULT_RETAINED_CHANGE_THRESHOLD,
    },
};

pub(crate) const MAX_OBJECT_DIFF_FINGERPRINTS: usize = 8_000_000;

const GC_ROOT_SENTINEL: ClassId = u64::MAX;
const UNKNOWN_CLASS_SENTINEL: ClassId = u64::MAX - 1;

#[derive(Debug, Clone, Copy)]
pub struct ObjectDiffEngineConfig {
    pub strategy: IdentityStrategy,
    pub bucket_bits: u8,
    pub min_retained_bytes: u64,
    pub retained_change_threshold: u64,
    pub top_n: usize,
    pub max_fingerprints: usize,
}

#[derive(Debug, Clone)]
struct FingerprintAggregate {
    class_name: String,
    example_object_id: ObjectId,
    count: u64,
    retained_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDiffErrorEnvelope {
    pub code: String,
    pub message: String,
    pub details: ObjectDiffErrorDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDiffErrorDetails {
    pub side: String,
    pub actual_fingerprints: usize,
    pub max_fingerprints: usize,
}

impl ObjectDiffErrorEnvelope {
    fn object_diff_too_large(
        side: &str,
        actual_fingerprints: usize,
        max_fingerprints: usize,
    ) -> Self {
        Self {
            code: "feature_unavailable_object_diff_too_large".into(),
            message: "object diff fingerprint budget exceeded; raise --object-diff-min-retained or diff a smaller heap".into(),
            details: ObjectDiffErrorDetails {
                side: side.into(),
                actual_fingerprints,
                max_fingerprints,
            },
        }
    }

    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported".into(),
            message: message.into(),
            details: ObjectDiffErrorDetails {
                side: "either".into(),
                actual_fingerprints: 0,
                max_fingerprints: 0,
            },
        }
    }

    pub(crate) fn to_core_error(&self) -> CoreError {
        CoreError::Unsupported(format!(
            "{}: {} (side={}, actual_fingerprints={}, max_fingerprints={})",
            self.code,
            self.message,
            self.details.side,
            self.details.actual_fingerprints,
            self.details.max_fingerprints,
        ))
    }
}

pub fn diff_object_graphs(
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> Result<ObjectDiffReport, ObjectDiffErrorEnvelope> {
    diff_object_graphs_with_limit(
        before_graph,
        before_dom,
        after_graph,
        after_dom,
        ObjectDiffEngineConfig {
            strategy,
            bucket_bits,
            min_retained_bytes,
            retained_change_threshold: DEFAULT_RETAINED_CHANGE_THRESHOLD,
            top_n: usize::MAX,
            max_fingerprints: MAX_OBJECT_DIFF_FINGERPRINTS,
        },
    )
}

pub fn diff_object_graphs_with_limit(
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
    config: ObjectDiffEngineConfig,
) -> Result<ObjectDiffReport, ObjectDiffErrorEnvelope> {
    let before = collect_fingerprints(
        before_graph,
        before_dom,
        config.strategy,
        config.bucket_bits,
        config.min_retained_bytes,
        "before",
        config.max_fingerprints,
    )?;
    let after = collect_fingerprints(
        after_graph,
        after_dom,
        config.strategy,
        config.bucket_bits,
        config.min_retained_bytes,
        "after",
        config.max_fingerprints,
    )?;

    let mut report = ObjectDiffReport::new(
        config.strategy,
        config.bucket_bits,
        config.retained_change_threshold,
    );

    for (fingerprint, after_stats) in &after {
        match before.get(fingerprint) {
            Some(before_stats) if after_stats.count > before_stats.count => {
                report.added.push(make_delta(
                    *fingerprint,
                    Some(before_stats),
                    Some(after_stats),
                    ObjectDeltaKind::Added,
                ))
            }
            None => report.added.push(make_delta(
                *fingerprint,
                None,
                Some(after_stats),
                ObjectDeltaKind::Added,
            )),
            _ => {}
        }

        if let Some(before_stats) = before.get(fingerprint) {
            let retained_delta =
                retained_delta_values(before_stats.retained_bytes, after_stats.retained_bytes);

            if retained_delta > 0 && retained_delta >= config.retained_change_threshold {
                report.retained_changed.push(make_delta(
                    *fingerprint,
                    Some(before_stats),
                    Some(after_stats),
                    ObjectDeltaKind::RetainedChanged,
                ));
            }
        }
    }

    for (fingerprint, before_stats) in &before {
        match after.get(fingerprint) {
            Some(after_stats) if before_stats.count > after_stats.count => {
                report.removed.push(make_delta(
                    *fingerprint,
                    Some(before_stats),
                    Some(after_stats),
                    ObjectDeltaKind::Removed,
                ))
            }
            None => report.removed.push(make_delta(
                *fingerprint,
                Some(before_stats),
                None,
                ObjectDeltaKind::Removed,
            )),
            _ => {}
        }
    }

    sort_and_truncate(&mut report.added, config.top_n);
    sort_and_truncate(&mut report.removed, config.top_n);
    sort_and_truncate(&mut report.retained_changed, config.top_n);

    enrich_deltas(
        &mut report.added,
        before_graph,
        before_dom,
        after_graph,
        after_dom,
    );
    enrich_deltas(
        &mut report.removed,
        before_graph,
        before_dom,
        after_graph,
        after_dom,
    );
    enrich_deltas(
        &mut report.retained_changed,
        before_graph,
        before_dom,
        after_graph,
        after_dom,
    );

    let (match_quality, mut totals) = compute_match_quality(
        before_graph,
        before_dom,
        after_graph,
        after_dom,
        config.strategy,
        config.bucket_bits,
        config.min_retained_bytes,
    )
    .map_err(|error| ObjectDiffErrorEnvelope::unsupported(error.to_string()))?;
    totals.matched_pairs = matched_pairs(&before, &after);
    report.match_quality = match_quality;
    report.totals = totals;

    Ok(report)
}

fn collect_fingerprints(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
    side: &str,
    max_fingerprints: usize,
) -> Result<BTreeMap<ObjectFingerprint, FingerprintAggregate>, ObjectDiffErrorEnvelope> {
    let mut fingerprints = BTreeMap::new();

    for (&obj_id, obj) in &graph.objects {
        let retained_bytes = dom.retained_size(obj_id);
        if retained_bytes < min_retained_bytes {
            continue;
        }

        let fingerprint = ObjectFingerprint::build(graph, dom, obj_id, strategy, bucket_bits)
            .map_err(|error| ObjectDiffErrorEnvelope::unsupported(error.to_string()))?;
        let class_name = graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string();
        let needs_new_entry = !fingerprints.contains_key(&fingerprint);

        if needs_new_entry && fingerprints.len() >= max_fingerprints {
            return Err(ObjectDiffErrorEnvelope::object_diff_too_large(
                side,
                fingerprints.len() + 1,
                max_fingerprints,
            ));
        }

        let entry = fingerprints
            .entry(fingerprint)
            .or_insert_with(|| FingerprintAggregate {
                class_name,
                example_object_id: obj_id,
                count: 0,
                retained_bytes: 0,
            });

        entry.count += 1;
        entry.retained_bytes += retained_bytes;
    }

    Ok(fingerprints)
}

fn make_delta(
    fingerprint: ObjectFingerprint,
    before: Option<&FingerprintAggregate>,
    after: Option<&FingerprintAggregate>,
    kind: ObjectDeltaKind,
) -> ObjectDelta {
    let baseline = after
        .or(before)
        .expect("object diff delta requires data on one side");

    ObjectDelta {
        class_name: baseline.class_name.clone(),
        fingerprint,
        example_object_id: after
            .map(|stats| stats.example_object_id)
            .or_else(|| before.map(|stats| stats.example_object_id))
            .unwrap_or(0),
        before_count: before.map_or(0, |stats| stats.count),
        after_count: after.map_or(0, |stats| stats.count),
        before_retained_bytes: before.map_or(0, |stats| stats.retained_bytes),
        after_retained_bytes: after.map_or(0, |stats| stats.retained_bytes),
        dominator_chain: Vec::new(),
        reference_chain: Vec::new(),
        kind,
    }
}

fn sort_and_truncate(deltas: &mut Vec<ObjectDelta>, top_n: usize) {
    sort_deltas(deltas);
    deltas.truncate(top_n);
}

fn sort_deltas(deltas: &mut [ObjectDelta]) {
    deltas.sort_by(|left, right| {
        retained_delta(right)
            .cmp(&retained_delta(left))
            .then_with(|| left.class_name.cmp(&right.class_name))
            .then_with(|| left.fingerprint.cmp(&right.fingerprint))
    });
}

fn retained_delta(delta: &ObjectDelta) -> u64 {
    retained_delta_values(delta.before_retained_bytes, delta.after_retained_bytes)
}

fn retained_delta_values(before_retained_bytes: u64, after_retained_bytes: u64) -> u64 {
    after_retained_bytes.abs_diff(before_retained_bytes)
}

fn enrich_deltas(
    deltas: &mut [ObjectDelta],
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
) {
    for delta in deltas {
        enrich_delta(delta, before_graph, before_dom, after_graph, after_dom);
    }
}

fn enrich_delta(
    delta: &mut ObjectDelta,
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
) {
    let (graph, dom) = match delta.kind {
        ObjectDeltaKind::Added | ObjectDeltaKind::RetainedChanged => (after_graph, after_dom),
        ObjectDeltaKind::Removed => (before_graph, before_dom),
    };

    delta.dominator_chain = build_dominator_chain(graph, dom, delta.example_object_id);
    delta.reference_chain = build_reference_chain(graph, delta.example_object_id);
}

fn build_dominator_chain(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    obj_id: ObjectId,
) -> Vec<String> {
    let mut labels = Vec::new();

    for class_id in dominator_chain::dominator_class_chain(graph, dom, obj_id) {
        match class_id {
            GC_ROOT_SENTINEL => {
                labels.push(String::from("GC Root"));
                break;
            }
            UNKNOWN_CLASS_SENTINEL => {
                labels.push(String::from("<unknown>"));
                break;
            }
            _ => labels.push(resolve_stable_class_name(graph, class_id)),
        }
    }

    labels
}

fn resolve_stable_class_name(graph: &ObjectGraph, stable_class_id: ClassId) -> String {
    let mut matches: Vec<&str> = graph
        .classes
        .values()
        .filter_map(|class_info| class_info.name.as_deref())
        .filter(|class_name| u64::from(stable_class_name_id(class_name)) == stable_class_id)
        .collect();

    matches.sort_unstable();
    matches.dedup();

    matches.first().copied().unwrap_or("<unknown>").to_string()
}

fn matched_pairs(
    before: &BTreeMap<ObjectFingerprint, FingerprintAggregate>,
    after: &BTreeMap<ObjectFingerprint, FingerprintAggregate>,
) -> u64 {
    before
        .iter()
        .filter_map(|(fingerprint, before_stats)| {
            after
                .get(fingerprint)
                .map(|after_stats| before_stats.count.min(after_stats.count))
        })
        .sum()
}
