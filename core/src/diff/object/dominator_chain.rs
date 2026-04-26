use crate::{
    graph::{DominatorTree, VIRTUAL_ROOT_ID},
    hprof::{ClassId, ObjectGraph, ObjectId},
};

use super::fingerprint::stable_class_name_id;

const DOMINATOR_CHAIN_DEPTH: usize = 4;
const GC_ROOT_SENTINEL: ClassId = u64::MAX;
const UNKNOWN_CLASS_SENTINEL: ClassId = u64::MAX - 1;

pub(crate) fn dominator_class_chain(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    obj_id: ObjectId,
) -> Vec<ClassId> {
    let mut chain = Vec::with_capacity(DOMINATOR_CHAIN_DEPTH);
    let mut current = obj_id;
    let mut reached_gc_root = false;

    while chain.len() < DOMINATOR_CHAIN_DEPTH {
        if reached_gc_root {
            chain.push(GC_ROOT_SENTINEL);
            continue;
        }

        let Some(parent_id) = dom.immediate_dominator(current) else {
            chain.resize(DOMINATOR_CHAIN_DEPTH, UNKNOWN_CLASS_SENTINEL);
            break;
        };

        if parent_id == VIRTUAL_ROOT_ID {
            chain.push(GC_ROOT_SENTINEL);
            reached_gc_root = true;
            continue;
        }

        chain.push(class_id_for_object(graph, parent_id));
        current = parent_id;
    }

    chain
}

pub(crate) fn hash_dominator_class_chain(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    obj_id: ObjectId,
) -> u64 {
    let chain = dominator_class_chain(graph, dom, obj_id);
    let mut bytes = Vec::with_capacity(chain.len() * std::mem::size_of::<ClassId>());

    for class_id in chain {
        bytes.extend_from_slice(&class_id.to_le_bytes());
    }

    xxhash_rust::xxh64::xxh64(&bytes, 0)
}

fn class_id_for_object(graph: &ObjectGraph, obj_id: ObjectId) -> ClassId {
    graph
        .get_object(obj_id)
        .and_then(|obj| graph.class_name(obj.class_id))
        .map(|class_name| u64::from(stable_class_name_id(class_name)))
        .unwrap_or(UNKNOWN_CLASS_SENTINEL)
}
