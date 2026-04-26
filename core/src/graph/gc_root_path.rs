use std::collections::{HashMap, HashSet, VecDeque};

use crate::hprof::{GcRootKind, GcRootType, ObjectGraph, ObjectId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcRootPath {
    pub root_kind: GcRootKind,
    pub frames: Vec<ObjectId>,
}

pub fn shortest_gc_root_path(
    graph: &ObjectGraph,
    target: ObjectId,
    max_depth: usize,
) -> Option<GcRootPath> {
    GcRootPathIndex::new(graph).shortest_gc_root_path(target, max_depth)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GcRootPathIndex {
    reverse_edges: HashMap<ObjectId, Vec<ObjectId>>,
    gc_roots: HashMap<ObjectId, GcRootKind>,
}

impl GcRootPathIndex {
    pub(crate) fn new(graph: &ObjectGraph) -> Self {
        Self {
            reverse_edges: build_reverse_edges(graph),
            gc_roots: build_gc_root_lookup(graph),
        }
    }

    pub(crate) fn shortest_gc_root_path(
        &self,
        target: ObjectId,
        max_depth: usize,
    ) -> Option<GcRootPath> {
        if max_depth == 0 {
            return None;
        }

        let mut queue = VecDeque::from([(target, 1usize)]);
        let mut visited = HashSet::from([target]);
        let mut next_toward_target = HashMap::<ObjectId, ObjectId>::new();

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(&root_kind) = self.gc_roots.get(&current) {
                return Some(GcRootPath {
                    root_kind,
                    frames: reconstruct_path(current, target, &next_toward_target),
                });
            }

            if depth >= max_depth {
                continue;
            }

            if let Some(referrers) = self.reverse_edges.get(&current) {
                for &referrer in referrers {
                    if visited.insert(referrer) {
                        next_toward_target.insert(referrer, current);
                        queue.push_back((referrer, depth + 1));
                    }
                }
            }
        }

        None
    }

    pub(crate) fn is_gc_root(&self, object_id: ObjectId) -> bool {
        self.gc_roots.contains_key(&object_id)
    }
}

fn build_reverse_edges(graph: &ObjectGraph) -> HashMap<ObjectId, Vec<ObjectId>> {
    let mut reverse_edges: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();

    for object in graph.objects.values() {
        for &reference in &object.references {
            reverse_edges.entry(reference).or_default().push(object.id);
        }
    }

    for referrers in reverse_edges.values_mut() {
        referrers.sort_unstable();
        referrers.dedup();
    }

    reverse_edges
}

fn build_gc_root_lookup(graph: &ObjectGraph) -> HashMap<ObjectId, GcRootKind> {
    let mut lookup = HashMap::new();
    for root in &graph.gc_roots {
        lookup
            .entry(root.object_id)
            .or_insert_with(|| gc_root_kind(root.root_type.clone()));
    }
    lookup
}

fn gc_root_kind(root_type: GcRootType) -> GcRootKind {
    match root_type {
        GcRootType::JniGlobal => GcRootKind::JniGlobal,
        GcRootType::JniLocal { .. } => GcRootKind::JniLocal,
        GcRootType::JavaFrame { .. } => GcRootKind::JavaFrame,
        GcRootType::NativeStack { .. } => GcRootKind::NativeStack,
        GcRootType::StickyClass => GcRootKind::StickyClass,
        GcRootType::ThreadBlock { .. } => GcRootKind::ThreadBlock,
        GcRootType::MonitorUsed => GcRootKind::MonitorUsed,
        GcRootType::ThreadObject { .. } => GcRootKind::ThreadObject,
        GcRootType::Unknown(_) => GcRootKind::Unknown,
    }
}

fn reconstruct_path(
    root_id: ObjectId,
    target: ObjectId,
    next_toward_target: &HashMap<ObjectId, ObjectId>,
) -> Vec<ObjectId> {
    let mut path = vec![root_id];
    let mut current = root_id;

    while current != target {
        let Some(next) = next_toward_target.get(&current).copied() else {
            break;
        };
        current = next;
        path.push(current);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind};

    fn add_class(
        graph: &mut ObjectGraph,
        class_id: u64,
        super_class_id: u64,
        name: &str,
        instance_size: u32,
    ) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id,
                class_loader_id: 0,
                instance_size,
                name: Some(name.into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
    }

    fn add_object(
        graph: &mut ObjectGraph,
        object_id: u64,
        class_id: u64,
        shallow_size: u32,
        references: &[u64],
    ) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size,
                references: references.to_vec(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
    }

    fn add_root(graph: &mut ObjectGraph, object_id: u64, root_type: GcRootType) {
        graph.gc_roots.push(GcRoot {
            object_id,
            root_type,
        });
    }

    fn synthetic_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.Middle", 24);
        add_class(&mut graph, 0x400, 0x100, "com.example.Target", 24);
        add_class(&mut graph, 0x500, 0x100, "com.example.Unreachable", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2]);
        add_object(&mut graph, 2, 0x300, 24, &[3]);
        add_object(&mut graph, 3, 0x400, 32, &[]);
        add_object(&mut graph, 99, 0x500, 32, &[]);
        add_root(&mut graph, 1, GcRootType::StickyClass);
        graph
    }

    #[test]
    fn shortest_gc_root_path_finds_known_path_in_synthetic_graph() {
        let graph = synthetic_graph();

        let path = shortest_gc_root_path(&graph, 3, 8).expect("path should exist");

        assert_eq!(path.root_kind, GcRootKind::StickyClass);
        assert_eq!(path.frames, vec![1, 2, 3]);
    }

    #[test]
    fn shortest_gc_root_path_returns_none_for_unreachable() {
        let graph = synthetic_graph();

        assert_eq!(shortest_gc_root_path(&graph, 99, 8), None);
    }

    #[test]
    fn shortest_gc_root_path_respects_max_depth() {
        let graph = synthetic_graph();

        assert_eq!(shortest_gc_root_path(&graph, 3, 2), None);
        assert!(shortest_gc_root_path(&graph, 3, 3).is_some());
    }
}
