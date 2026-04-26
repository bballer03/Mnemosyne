use crate::{
    graph::{DominatorTree, VIRTUAL_ROOT_ID},
    hprof::ObjectGraph,
    report::flamegraph::{apply_budget, CollapseOptions, FlameRoot, FoldedStack, FoldedStacks},
};

pub fn collapse_dominator(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    opts: &CollapseOptions,
) -> FoldedStacks {
    let mut stacks = graph
        .objects
        .keys()
        .copied()
        .filter(|&object_id| dom.dominated_by(object_id).is_empty())
        .map(|leaf_id| FoldedStack {
            frames: build_frames(graph, dom, leaf_id),
            weight: dom.retained_size(leaf_id),
        })
        .collect::<Vec<_>>();

    stacks.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.frames.join(";").cmp(&right.frames.join(";")))
    });

    let collapsed = FoldedStacks::new(FlameRoot::Dominator, stacks);
    apply_budget(collapsed, opts.min_fraction, opts.max_frames)
}

fn build_frames(graph: &ObjectGraph, dom: &DominatorTree, leaf_id: u64) -> Vec<String> {
    let mut frames = Vec::new();
    let mut current = Some(leaf_id);

    while let Some(object_id) = current {
        frames.push(frame_name_for_object(graph, object_id));
        current = match dom.immediate_dominator(object_id) {
            Some(VIRTUAL_ROOT_ID) => {
                frames.push(String::from("<gc-root>"));
                None
            }
            other => other,
        };
    }

    frames.reverse();
    frames
}

fn frame_name_for_object(graph: &ObjectGraph, object_id: u64) -> String {
    let Some(object) = graph.objects.get(&object_id) else {
        return format!("<unknown object id={object_id}>");
    };

    graph
        .class_name(object.class_id)
        .map(str::to_string)
        .unwrap_or_else(|| format!("<unknown class id={}>", object.class_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        graph::build_dominator_tree,
        hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
        report::flamegraph::FoldedStack,
    };

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: Vec::new(),
                static_references: Vec::new(),
            },
        );
    }

    fn synthetic_graph() -> (ObjectGraph, DominatorTree) {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, "com.example.Root");
        add_class(&mut graph, 0x200, "com.example.Left");
        add_class(&mut graph, 0x300, "com.example.Right");

        graph.objects.insert(
            1,
            HeapObject {
                id: 1,
                class_id: 0x100,
                shallow_size: 5,
                references: vec![2, 3],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            2,
            HeapObject {
                id: 2,
                class_id: 0x200,
                shallow_size: 10,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            3,
            HeapObject {
                id: 3,
                class_id: 0x300,
                shallow_size: 20,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.gc_roots.push(GcRoot {
            object_id: 1,
            root_type: GcRootType::StickyClass,
        });

        let dom = build_dominator_tree(&graph);
        (graph, dom)
    }

    #[test]
    fn dominator_collapse_synthetic_graph_produces_expected_stacks() {
        let (graph, dom) = synthetic_graph();

        let collapsed = collapse_dominator(&graph, &dom, &CollapseOptions::default());

        assert_eq!(collapsed.strategy, FlameRoot::Dominator);
        assert_eq!(
            collapsed.stacks,
            vec![
                FoldedStack {
                    frames: vec![
                        "<gc-root>".into(),
                        "com.example.Root".into(),
                        "com.example.Right".into(),
                    ],
                    weight: 20,
                },
                FoldedStack {
                    frames: vec![
                        "<gc-root>".into(),
                        "com.example.Root".into(),
                        "com.example.Left".into(),
                    ],
                    weight: 10,
                },
            ]
        );
        assert_eq!(collapsed.total_weight, 30);
    }

    #[test]
    fn dominator_collapse_total_weight_invariant_holds() {
        let (graph, dom) = synthetic_graph();
        let collapsed = collapse_dominator(
            &graph,
            &dom,
            &CollapseOptions {
                min_fraction: 0.4,
                max_frames: usize::MAX,
            },
        );

        let stack_weight_sum: u64 = collapsed.stacks.iter().map(|stack| stack.weight).sum();
        assert_eq!(
            collapsed.total_weight,
            stack_weight_sum + collapsed.truncated_to_other
        );
        assert!(collapsed.truncated_to_other > 0);
    }
}
