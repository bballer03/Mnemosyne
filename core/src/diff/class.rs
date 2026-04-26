use crate::{
    graph::DominatorTree,
    hprof::{ClassLevelDelta, ObjectGraph},
};
use std::collections::HashMap;

pub fn compute_class_level_diff(
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
