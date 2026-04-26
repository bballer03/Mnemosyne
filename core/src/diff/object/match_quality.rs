use std::collections::HashMap;

use crate::{errors::CoreResult, graph::DominatorTree, hprof::ObjectGraph};

use super::{
    fingerprint::ObjectFingerprint,
    types::{IdentityStrategy, MatchQuality, ObjectDiffTotals, Risk},
};

#[derive(Debug, Clone, Copy, Default)]
struct CollisionStats {
    object_count: u64,
    collision_count: u64,
}

impl CollisionStats {
    fn collision_rate(self) -> f64 {
        if self.object_count == 0 {
            0.0
        } else {
            self.collision_count as f64 / self.object_count as f64
        }
    }
}

pub fn compute_collision_rate(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> CoreResult<f64> {
    Ok(
        compute_collision_stats(graph, dom, strategy, bucket_bits, min_retained_bytes)?
            .collision_rate(),
    )
}

pub(crate) fn compute_match_quality(
    before_graph: &ObjectGraph,
    before_dom: &DominatorTree,
    after_graph: &ObjectGraph,
    after_dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> CoreResult<(MatchQuality, ObjectDiffTotals)> {
    let before_stats = compute_collision_stats(
        before_graph,
        before_dom,
        strategy,
        bucket_bits,
        min_retained_bytes,
    )?;
    let after_stats = compute_collision_stats(
        after_graph,
        after_dom,
        strategy,
        bucket_bits,
        min_retained_bytes,
    )?;

    let total_objects = before_stats.object_count + after_stats.object_count;
    let collision_rate = if total_objects == 0 {
        0.0
    } else {
        (before_stats.collision_count + after_stats.collision_count) as f64 / total_objects as f64
    };

    let match_quality = MatchQuality {
        strategy,
        collision_rate,
        estimated_false_match_risk: false_match_risk(strategy, collision_rate),
        estimated_false_split_risk: false_split_risk(strategy),
        notes: notes_for_strategy(strategy, collision_rate),
    };

    let totals = ObjectDiffTotals {
        before_object_count: before_stats.object_count,
        after_object_count: after_stats.object_count,
        fingerprint_collisions_before: before_stats.collision_count,
        fingerprint_collisions_after: after_stats.collision_count,
        matched_pairs: 0,
    };

    Ok((match_quality, totals))
}

fn compute_collision_stats(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    strategy: IdentityStrategy,
    bucket_bits: u8,
    min_retained_bytes: u64,
) -> CoreResult<CollisionStats> {
    let mut counts = HashMap::new();
    let mut object_count = 0_u64;

    for &obj_id in graph.objects.keys() {
        if dom.retained_size(obj_id) < min_retained_bytes {
            continue;
        }

        let fingerprint = ObjectFingerprint::build(graph, dom, obj_id, strategy, bucket_bits)?;
        *counts.entry(fingerprint).or_insert(0_u64) += 1;
        object_count += 1;
    }

    let collision_count = counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1)
        .sum();

    Ok(CollisionStats {
        object_count,
        collision_count,
    })
}

fn false_match_risk(strategy: IdentityStrategy, collision_rate: f64) -> Risk {
    match strategy {
        IdentityStrategy::ClassRetained => {
            if collision_rate >= 0.01 {
                Risk::High
            } else {
                Risk::Medium
            }
        }
        IdentityStrategy::ClassDominator => {
            if collision_rate >= 0.05 {
                Risk::High
            } else if collision_rate > 0.0 {
                Risk::Medium
            } else {
                Risk::Low
            }
        }
        IdentityStrategy::FullFingerprint => {
            if collision_rate >= 0.05 {
                Risk::High
            } else if collision_rate > 0.0 {
                Risk::Medium
            } else {
                Risk::Low
            }
        }
    }
}

fn false_split_risk(strategy: IdentityStrategy) -> Risk {
    match strategy {
        IdentityStrategy::ClassRetained => Risk::Low,
        IdentityStrategy::ClassDominator => Risk::Low,
        IdentityStrategy::FullFingerprint => Risk::Medium,
    }
}

fn notes_for_strategy(strategy: IdentityStrategy, collision_rate: f64) -> Vec<String> {
    let mut notes = Vec::new();

    match strategy {
        IdentityStrategy::ClassRetained => notes.push(
            "class+retained can collapse sibling instances that share a retained-size bucket"
                .into(),
        ),
        IdentityStrategy::ClassDominator => notes.push(
            "class+dominator adds four hops of dominator context to reduce sibling collisions"
                .into(),
        ),
        IdentityStrategy::FullFingerprint => notes.push(
            "full-fingerprint is wired through Slice 8-1.C with field and outbound signatures stubbed to 0"
                .into(),
        ),
    }

    if collision_rate > 0.0 {
        notes.push(format!(
            "collision rate {:.2}% indicates fingerprint collisions remain in the eligible object set",
            collision_rate * 100.0
        ));
    }

    notes
}
