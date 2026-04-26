use std::collections::BTreeMap;

use crate::{
    errors::CoreResult,
    graph::DominatorTree,
    hprof::{ObjectGraph, ObjectId},
};

use super::{
    fingerprint::ObjectFingerprint,
    types::{IdentityStrategy, ObjectDelta, ObjectDiffReport},
};

#[derive(Debug, Clone)]
struct FingerprintAggregate {
    class_name: String,
    example_object_id: ObjectId,
    count: u64,
    retained_bytes: u64,
}

pub fn diff_object_graphs(
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> CoreResult<ObjectDiffReport> {
    let before = collect_fingerprints(
        before_graph,
        before_dom,
        strategy,
        bucket_bits,
        min_retained_bytes,
    )?;
    let after = collect_fingerprints(
        after_graph,
        after_dom,
        strategy,
        bucket_bits,
        min_retained_bytes,
    )?;

    let mut report = ObjectDiffReport::new(strategy, bucket_bits, min_retained_bytes);

    for (fingerprint, after_stats) in &after {
        match before.get(fingerprint) {
            Some(before_stats) if after_stats.count > before_stats.count => report.added.push(
                make_delta(*fingerprint, Some(before_stats), Some(after_stats)),
            ),
            None => report
                .added
                .push(make_delta(*fingerprint, None, Some(after_stats))),
            _ => {}
        }
    }

    for (fingerprint, before_stats) in &before {
        match after.get(fingerprint) {
            Some(after_stats) if before_stats.count > after_stats.count => report.removed.push(
                make_delta(*fingerprint, Some(before_stats), Some(after_stats)),
            ),
            None => report
                .removed
                .push(make_delta(*fingerprint, Some(before_stats), None)),
            _ => {}
        }
    }

    sort_deltas(&mut report.added);
    sort_deltas(&mut report.removed);

    Ok(report)
}

fn collect_fingerprints(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> CoreResult<BTreeMap<ObjectFingerprint, FingerprintAggregate>> {
    let mut fingerprints = BTreeMap::new();

    for (&obj_id, obj) in &graph.objects {
        let retained_bytes = dom.retained_size(obj_id);
        if retained_bytes < min_retained_bytes {
            continue;
        }

        let fingerprint = ObjectFingerprint::build(graph, dom, obj_id, strategy, bucket_bits)?;
        let class_name = graph
            .class_name(obj.class_id)
            .unwrap_or("<unknown>")
            .to_string();
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
    }
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
    delta
        .after_retained_bytes
        .abs_diff(delta.before_retained_bytes)
}
