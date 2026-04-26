pub mod class_hierarchy;
pub mod dominator;
pub mod gc_root_path;

pub use class_hierarchy::collapse_class_hierarchy;
pub use dominator::collapse_dominator;
pub use gc_root_path::collapse_gc_root_path;

use crate::{graph::DominatorTree, hprof::ObjectGraph, report::flamegraph::FlameRoot};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollapseOptions {
    pub min_fraction: f64,
    pub max_frames: usize,
}

impl Default for CollapseOptions {
    fn default() -> Self {
        Self {
            min_fraction: 0.001,
            max_frames: 5_000,
        }
    }
}

pub fn collapse(
    strategy: FlameRoot,
    graph: &ObjectGraph,
    dom: &DominatorTree,
    opts: &CollapseOptions,
) -> crate::report::flamegraph::FoldedStacks {
    match strategy {
        FlameRoot::Dominator => collapse_dominator(graph, dom, opts),
        FlameRoot::ClassHierarchy => collapse_class_hierarchy(graph, opts),
        FlameRoot::GcRootPath => collapse_gc_root_path(graph, dom, opts),
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

    fn dispatcher_graph() -> (ObjectGraph, DominatorTree) {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 0x100, 0, "java.lang.Object", 8);
        add_class(&mut graph, 0x200, 0x100, "com.example.Root", 16);
        add_class(&mut graph, 0x300, 0x200, "com.example.Leaf", 24);

        add_object(&mut graph, 1, 0x200, 16, &[2]);
        add_object(&mut graph, 2, 0x300, 48, &[]);

        graph.gc_roots.push(GcRoot {
            object_id: 1,
            root_type: GcRootType::StickyClass,
        });

        let dom = build_dominator_tree(&graph);
        (graph, dom)
    }

    #[test]
    fn collapse_dispatcher_routes_to_class_hierarchy_when_strategy_class_hierarchy() {
        let (graph, dom) = dispatcher_graph();

        let collapsed = collapse(
            FlameRoot::ClassHierarchy,
            &graph,
            &dom,
            &CollapseOptions::default(),
        );

        assert_eq!(collapsed.strategy, FlameRoot::ClassHierarchy);
        assert!(collapsed
            .stacks
            .iter()
            .any(|stack| stack.frames == vec!["java.lang.Object", "com.example.Root"]));
    }

    #[test]
    fn collapse_dispatcher_routes_to_gc_root_path_when_strategy_gc_root_path() {
        let (graph, dom) = dispatcher_graph();

        let collapsed = collapse(
            FlameRoot::GcRootPath,
            &graph,
            &dom,
            &CollapseOptions::default(),
        );

        assert_eq!(collapsed.strategy, FlameRoot::GcRootPath);
        assert!(collapsed.stacks.iter().any(|stack| {
            stack
                .frames
                .first()
                .is_some_and(|frame| frame.starts_with("<gc-root:"))
        }));
    }

    #[test]
    fn collapse_dispatcher_routes_to_dominator_when_strategy_dominator() {
        let (graph, dom) = dispatcher_graph();

        let collapsed = collapse(
            FlameRoot::Dominator,
            &graph,
            &dom,
            &CollapseOptions::default(),
        );

        assert_eq!(
            collapsed,
            collapse_dominator(&graph, &dom, &CollapseOptions::default())
        );
    }
}
