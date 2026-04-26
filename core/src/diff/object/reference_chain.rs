use std::collections::{HashSet, VecDeque};

use crate::hprof::{ObjectGraph, ObjectId};

const REFERENCE_CHAIN_DEPTH: usize = 4;

pub(crate) fn build_reference_chain(graph: &ObjectGraph, obj_id: ObjectId) -> Vec<String> {
    let gc_roots: HashSet<ObjectId> = graph
        .gc_roots
        .iter()
        .filter_map(|root| {
            graph
                .objects
                .contains_key(&root.object_id)
                .then_some(root.object_id)
        })
        .collect();
    let mut queue = VecDeque::from([vec![obj_id]]);
    let mut seen = HashSet::from([obj_id]);
    let mut best_path = vec![obj_id];

    while let Some(path) = queue.pop_front() {
        let Some(&current) = path.last() else {
            continue;
        };

        if gc_roots.contains(&current) {
            return materialize_chain(graph, &path, true);
        }

        if path.len() > best_path.len() {
            best_path = path.clone();
        }

        if path.len() >= REFERENCE_CHAIN_DEPTH {
            continue;
        }

        let mut referrers = graph.get_referrers(current);
        referrers.sort_unstable();

        for referrer in referrers {
            if seen.insert(referrer) {
                let mut next_path = path.clone();
                next_path.push(referrer);
                queue.push_back(next_path);
            }
        }
    }

    materialize_chain(
        graph,
        &best_path,
        gc_roots.contains(best_path.last().unwrap_or(&obj_id)),
    )
}

fn materialize_chain(graph: &ObjectGraph, path: &[ObjectId], include_gc_root: bool) -> Vec<String> {
    let mut chain: Vec<String> = path
        .iter()
        .rev()
        .map(|&path_obj_id| class_name_for_object(graph, path_obj_id))
        .collect();

    if include_gc_root {
        chain.insert(0, String::from("GC Root"));
    }

    chain
}

fn class_name_for_object(graph: &ObjectGraph, obj_id: ObjectId) -> String {
    graph
        .get_object(obj_id)
        .and_then(|obj| graph.class_name(obj.class_id))
        .unwrap_or("<unknown>")
        .to_string()
}
