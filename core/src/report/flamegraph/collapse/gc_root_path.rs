use crate::{
    graph::{gc_root_path::GcRootPathIndex, DominatorTree},
    hprof::{GcRootKind, ObjectGraph, ObjectId},
    report::flamegraph::{apply_budget, CollapseOptions, FlameRoot, FoldedStack, FoldedStacks},
};

const MAX_TOTAL_PATH_FRAMES: usize = 32;
const MAX_OBJECT_PATH_FRAMES: usize = MAX_TOTAL_PATH_FRAMES - 1;

pub fn collapse_gc_root_path(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    opts: &CollapseOptions,
) -> FoldedStacks {
    let gc_root_paths = GcRootPathIndex::new(graph);

    // Slice M7-3.B keeps this collapser local to graph+dom inputs, so the
    // stable seed set is the top retained dominator leaves that are not GC roots.
    let mut stacks = select_seed_objects(graph, dom, &gc_root_paths, opts.max_frames)
        .into_iter()
        .filter_map(|seed_id| {
            gc_root_paths
                .shortest_gc_root_path(seed_id, usize::MAX)
                .map(|path| FoldedStack {
                    frames: build_frames(graph, path.root_kind, &path.frames),
                    weight: dom.retained_size(seed_id),
                })
        })
        .collect::<Vec<_>>();

    stacks.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.frames.join(";").cmp(&right.frames.join(";")))
    });

    let collapsed = FoldedStacks::new(FlameRoot::GcRootPath, stacks);
    apply_budget(collapsed, opts.min_fraction, opts.max_frames)
}

fn select_seed_objects(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    gc_root_paths: &GcRootPathIndex,
    max_frames: usize,
) -> Vec<ObjectId> {
    let seed_limit = std::cmp::max(50, max_frames.div_ceil(MAX_TOTAL_PATH_FRAMES));

    dom.top_retained(graph.object_count())
        .into_iter()
        .map(|(object_id, _)| object_id)
        .filter(|object_id| dom.dominated_by(*object_id).is_empty())
        .filter(|object_id| !gc_root_paths.is_gc_root(*object_id))
        .take(seed_limit)
        .collect()
}
fn build_frames(graph: &ObjectGraph, root_kind: GcRootKind, path: &[ObjectId]) -> Vec<String> {
    let mut frames = Vec::with_capacity(path.len() + 1);
    frames.push(format!("<gc-root:{}>", gc_root_kind_name(root_kind)));

    let object_frames = path
        .iter()
        .map(|&object_id| class_name_for_object(graph, object_id))
        .collect::<Vec<_>>();
    frames.extend(cap_object_frames(object_frames));
    frames
}

fn cap_object_frames(object_frames: Vec<String>) -> Vec<String> {
    if object_frames.len() <= MAX_OBJECT_PATH_FRAMES {
        return object_frames;
    }

    let head_len = 15;
    let tail_len = 15;
    let elided = object_frames.len().saturating_sub(head_len + tail_len);

    let mut capped = Vec::with_capacity(MAX_OBJECT_PATH_FRAMES);
    capped.extend(object_frames.iter().take(head_len).cloned());
    capped.push(format!("<...elided {elided}...>"));
    capped.extend(
        object_frames
            .iter()
            .skip(object_frames.len() - tail_len)
            .cloned(),
    );
    capped
}

fn class_name_for_object(graph: &ObjectGraph, object_id: ObjectId) -> String {
    let Some(object) = graph.objects.get(&object_id) else {
        return format!("<unknown object id={object_id}>");
    };

    graph
        .class_name(object.class_id)
        .map(str::to_string)
        .unwrap_or_else(|| format!("<unknown class id={}>", object.class_id))
}

fn gc_root_kind_name(root_kind: GcRootKind) -> &'static str {
    match root_kind {
        GcRootKind::JniGlobal => "jni_global",
        GcRootKind::JniLocal => "jni_local",
        GcRootKind::JavaFrame => "java_frame",
        GcRootKind::NativeStack => "native_stack",
        GcRootKind::StickyClass => "sticky_class",
        GcRootKind::ThreadBlock => "thread_block",
        GcRootKind::MonitorUsed => "monitor_used",
        GcRootKind::ThreadObject => "thread_object",
        GcRootKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        graph::build_dominator_tree,
        hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
    };

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

    #[test]
    fn gc_root_path_collapse_synthetic_top_n_includes_root_kind_frame() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.Target", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2]);
        add_object(&mut graph, 2, 0x300, 128, &[]);
        add_root(&mut graph, 1, GcRootType::StickyClass);

        let dom = build_dominator_tree(&graph);
        let collapsed = collapse_gc_root_path(&graph, &dom, &CollapseOptions::default());

        assert_eq!(collapsed.strategy, FlameRoot::GcRootPath);
        assert!(collapsed.stacks.iter().any(|stack| {
            stack
                .frames
                .first()
                .is_some_and(|frame| frame.starts_with("<gc-root:"))
        }));
    }

    #[test]
    fn gc_root_path_handles_cycles_via_visited_set() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.A", 24);
        add_class(&mut graph, 0x400, 0x100, "com.example.B", 24);
        add_class(&mut graph, 0x500, 0x100, "com.example.Target", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2]);
        add_object(&mut graph, 2, 0x300, 32, &[3, 4]);
        add_object(&mut graph, 3, 0x400, 32, &[2]);
        add_object(&mut graph, 4, 0x500, 96, &[]);
        add_root(&mut graph, 1, GcRootType::StickyClass);

        let dom = build_dominator_tree(&graph);
        let collapsed = collapse_gc_root_path(&graph, &dom, &CollapseOptions::default());
        let target_stack = collapsed
            .stacks
            .iter()
            .find(|stack| stack.frames.last() == Some(&"com.example.Target".into()))
            .expect("target stack should be emitted");

        let duplicates = target_stack
            .frames
            .iter()
            .filter(|frame| frame.as_str() == "com.example.A")
            .count();
        assert_eq!(duplicates, 1);
    }

    #[test]
    fn gc_root_path_handles_unreachable_nodes_emits_skipped_or_omits() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.Target", 24);
        add_class(&mut graph, 0x400, 0x100, "com.example.Unreachable", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2]);
        add_object(&mut graph, 2, 0x300, 64, &[]);
        add_object(&mut graph, 99, 0x400, 256, &[]);
        add_root(&mut graph, 1, GcRootType::StickyClass);

        let dom = build_dominator_tree(&graph);
        let collapsed = collapse_gc_root_path(&graph, &dom, &CollapseOptions::default());

        assert!(collapsed
            .stacks
            .iter()
            .all(|stack| stack.frames.last() != Some(&"com.example.Unreachable".into())));
    }

    #[test]
    fn gc_root_path_total_weight_matches_sum_of_target_retained_bytes() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.Left", 24);
        add_class(&mut graph, 0x400, 0x100, "com.example.Right", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2, 3]);
        add_object(&mut graph, 2, 0x300, 64, &[]);
        add_object(&mut graph, 3, 0x400, 96, &[]);
        add_root(&mut graph, 1, GcRootType::StickyClass);

        let dom = build_dominator_tree(&graph);
        let collapsed = collapse_gc_root_path(&graph, &dom, &CollapseOptions::default());

        assert_eq!(collapsed.total_weight, 160);
        assert_eq!(
            collapsed.total_weight,
            collapsed
                .stacks
                .iter()
                .map(|stack| stack.weight)
                .sum::<u64>()
                + collapsed.truncated_to_other
        );
    }

    #[test]
    fn gc_root_path_caps_depth_at_32() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);

        let chain_len = 40u64;
        for index in 0..=chain_len {
            let class_id = 0x200 + index;
            let name = format!("com.example.Node{index}");
            add_class(&mut graph, class_id, 0x100, &name, 16);
        }

        for index in 0..=chain_len {
            let object_id = index + 1;
            let class_id = 0x200 + index;
            let next = if index == chain_len {
                Vec::new()
            } else {
                vec![object_id + 1]
            };
            add_object(&mut graph, object_id, class_id, 16, &next);
        }
        add_root(&mut graph, 1, GcRootType::StickyClass);

        let dom = build_dominator_tree(&graph);
        let collapsed = collapse_gc_root_path(&graph, &dom, &CollapseOptions::default());
        let deep_stack = collapsed
            .stacks
            .iter()
            .find(|stack| stack.frames.last() == Some(&"com.example.Node40".into()))
            .expect("deep target stack should be emitted");

        assert!(deep_stack.frames.len() <= 32);
        assert!(deep_stack
            .frames
            .iter()
            .any(|frame| frame.starts_with("<...elided ")));
    }
}
