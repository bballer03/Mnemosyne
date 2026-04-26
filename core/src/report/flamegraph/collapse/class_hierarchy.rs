use std::collections::HashMap;

use crate::{
    hprof::{ClassId, ObjectGraph},
    report::flamegraph::{apply_budget, CollapseOptions, FlameRoot, FoldedStack, FoldedStacks},
};

const JAVA_LANG_OBJECT: &str = "java.lang.Object";

pub fn collapse_class_hierarchy(graph: &ObjectGraph, opts: &CollapseOptions) -> FoldedStacks {
    let mut bytes_per_class: HashMap<ClassId, u64> = HashMap::new();
    for object in graph.objects.values() {
        *bytes_per_class.entry(object.class_id).or_default() += u64::from(object.shallow_size);
    }

    let mut stacks = bytes_per_class
        .into_iter()
        .map(|(class_id, weight)| FoldedStack {
            frames: build_super_chain(graph, class_id),
            weight,
        })
        .collect::<Vec<_>>();

    stacks.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.frames.join(";").cmp(&right.frames.join(";")))
    });

    let collapsed = FoldedStacks::new(FlameRoot::ClassHierarchy, stacks);
    apply_budget(collapsed, opts.min_fraction, opts.max_frames)
}

fn build_super_chain(graph: &ObjectGraph, class_id: ClassId) -> Vec<String> {
    let mut frames = vec![class_name_for_class(graph, class_id)];
    let mut visited = std::collections::HashSet::from([class_id]);
    let mut current = graph
        .classes
        .get(&class_id)
        .map(|class_info| class_info.super_class_id);

    while let Some(super_class_id) = current {
        if super_class_id == 0 || !visited.insert(super_class_id) {
            break;
        }

        let Some(class_info) = graph.classes.get(&super_class_id) else {
            break;
        };

        frames.push(class_name_for_class(graph, super_class_id));
        current = Some(class_info.super_class_id);
    }

    frames.reverse();
    if frames.first().map(String::as_str) != Some(JAVA_LANG_OBJECT) {
        frames.insert(0, String::from(JAVA_LANG_OBJECT));
    }
    frames
}

fn class_name_for_class(graph: &ObjectGraph, class_id: ClassId) -> String {
    graph
        .class_name(class_id)
        .map(str::to_string)
        .unwrap_or_else(|| format!("<unknown class id={class_id}>"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hprof::{ClassInfo, HeapObject, ObjectGraph, ObjectKind},
        report::flamegraph::FoldedStack,
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

    fn add_object(graph: &mut ObjectGraph, object_id: u64, class_id: u64, shallow_size: u32) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
    }

    #[test]
    fn class_hierarchy_collapse_simple_inheritance_chain() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "java.lang.Throwable", 16);
        add_class(&mut graph, 0x300, 0x200, "java.lang.Exception", 24);
        add_class(&mut graph, 0x400, 0x300, "java.io.IOException", 32);

        add_object(&mut graph, 1, 0x200, 16);
        add_object(&mut graph, 2, 0x300, 24);
        add_object(&mut graph, 3, 0x400, 32);
        add_object(&mut graph, 4, 0x400, 32);

        let collapsed = collapse_class_hierarchy(&graph, &CollapseOptions::default());

        assert_eq!(collapsed.strategy, FlameRoot::ClassHierarchy);
        assert!(collapsed.stacks.contains(&FoldedStack {
            frames: vec![
                "java.lang.Object".into(),
                "java.lang.Throwable".into(),
                "java.lang.Exception".into(),
                "java.io.IOException".into(),
            ],
            weight: 64,
        }));
    }

    #[test]
    fn class_hierarchy_collapse_unrelated_chains_emit_separate_stacks() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "java.lang.Exception", 24);
        add_class(&mut graph, 0x300, 0x200, "java.io.IOException", 32);
        add_class(&mut graph, 0x400, 0x100, "java.util.AbstractList", 24);
        add_class(&mut graph, 0x500, 0x400, "java.util.ArrayList", 32);

        add_object(&mut graph, 1, 0x300, 32);
        add_object(&mut graph, 2, 0x500, 48);

        let collapsed = collapse_class_hierarchy(&graph, &CollapseOptions::default());

        assert!(collapsed.stacks.iter().any(|stack| {
            stack.frames
                == vec![
                    "java.lang.Object",
                    "java.lang.Exception",
                    "java.io.IOException",
                ]
        }));
        assert!(collapsed.stacks.iter().any(|stack| {
            stack.frames
                == vec![
                    "java.lang.Object",
                    "java.util.AbstractList",
                    "java.util.ArrayList",
                ]
        }));
    }

    #[test]
    fn class_hierarchy_collapse_unresolved_super_falls_back_to_object() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x500, 0xDEAD_BEEF, "com.example.Orphan", 24);
        add_object(&mut graph, 1, 0x500, 24);

        let collapsed = collapse_class_hierarchy(&graph, &CollapseOptions::default());

        assert!(collapsed.stacks.contains(&FoldedStack {
            frames: vec!["java.lang.Object".into(), "com.example.Orphan".into()],
            weight: 24,
        }));
    }

    #[test]
    fn class_hierarchy_total_weight_matches_sum_of_instance_bytes() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Foo", 16);
        add_class(&mut graph, 0x300, 0x100, "com.example.Bar", 24);

        add_object(&mut graph, 1, 0x200, 16);
        add_object(&mut graph, 2, 0x200, 16);
        add_object(&mut graph, 3, 0x300, 24);

        let collapsed = collapse_class_hierarchy(&graph, &CollapseOptions::default());

        assert_eq!(collapsed.total_weight, 56);
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
    fn class_hierarchy_truncates_to_other_when_under_min_fraction() {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Big", 32);
        add_class(&mut graph, 0x300, 0x100, "com.example.Small", 8);

        add_object(&mut graph, 1, 0x200, 1_000);
        add_object(&mut graph, 2, 0x300, 5);

        let collapsed = collapse_class_hierarchy(
            &graph,
            &CollapseOptions {
                min_fraction: 0.01,
                max_frames: usize::MAX,
            },
        );

        assert_eq!(collapsed.total_weight, 1_005);
        assert_eq!(collapsed.truncated_to_other, 5);
        assert!(collapsed
            .stacks
            .iter()
            .any(|stack| stack.frames.last() == Some(&"com.example.Big".into())));
    }
}
