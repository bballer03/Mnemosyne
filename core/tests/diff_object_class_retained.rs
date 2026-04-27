use mnemosyne_core::{
    build_dominator_tree,
    diff::object::engine::diff_object_graphs,
    hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
    IdentityStrategy,
};

fn make_graph() -> ObjectGraph {
    ObjectGraph::new(8)
}

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

fn add_root(graph: &mut ObjectGraph, object_id: u64) {
    graph.gc_roots.push(GcRoot {
        object_id,
        root_type: GcRootType::StickyClass,
    });
}

fn run_class_retained_diff(
    before: ObjectGraph,
    after: ObjectGraph,
    bucket_bits: u8,
    min_retained: u64,
) -> mnemosyne_core::ObjectDiffReport {
    let before_dom = build_dominator_tree(&before);
    let after_dom = build_dominator_tree(&after);

    diff_object_graphs(
        &before,
        &before_dom,
        &after,
        &after_dom,
        IdentityStrategy::ClassRetained,
        bucket_bits,
        min_retained,
    )
    .expect("class+retained diff should succeed")
}

#[test]
fn class_retained_added_objects_appear_in_added() {
    let mut before = make_graph();
    add_class(&mut before, 100, "com.example.Session");
    add_object(&mut before, 1, 100, 10, &[]);
    add_root(&mut before, 1);

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Session");
    add_class(&mut after, 901, "com.example.Cache");
    add_object(&mut after, 11, 900, 10, &[]);
    add_object(&mut after, 12, 901, 20, &[]);
    add_root(&mut after, 11);
    add_root(&mut after, 12);

    let report = run_class_retained_diff(before, after, 0, 0);

    assert_eq!(report.added.len(), 1);
    assert!(report.removed.is_empty());
    assert_eq!(report.added[0].class_name, "com.example.Cache");
    assert_eq!(report.added[0].after_count, 1);
    assert_eq!(report.added[0].after_retained_bytes, 20);
}

#[test]
fn class_retained_removed_objects_appear_in_removed() {
    let mut before = make_graph();
    add_class(&mut before, 100, "com.example.Session");
    add_class(&mut before, 101, "com.example.Cache");
    add_object(&mut before, 1, 100, 10, &[]);
    add_object(&mut before, 2, 101, 20, &[]);
    add_root(&mut before, 1);
    add_root(&mut before, 2);

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Session");
    add_object(&mut after, 11, 900, 10, &[]);
    add_root(&mut after, 11);

    let report = run_class_retained_diff(before, after, 0, 0);

    assert!(report.added.is_empty());
    assert_eq!(report.removed.len(), 1);
    assert_eq!(report.removed[0].class_name, "com.example.Cache");
    assert_eq!(report.removed[0].before_count, 1);
    assert_eq!(report.removed[0].before_retained_bytes, 20);
}

#[test]
fn class_retained_unchanged_objects_omitted() {
    let mut before = make_graph();
    add_class(&mut before, 100, "com.example.Session");
    add_object(&mut before, 1, 100, 10, &[]);
    add_root(&mut before, 1);

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Session");
    add_object(&mut after, 11, 900, 10, &[]);
    add_root(&mut after, 11);

    let report = run_class_retained_diff(before, after, 0, 0);

    assert!(report.added.is_empty());
    assert!(report.removed.is_empty());
}

#[test]
fn bucket_bits_zero_treats_each_byte_as_distinct() {
    let before = make_graph();

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Buffer");
    add_object(&mut after, 11, 900, 17, &[]);
    add_object(&mut after, 12, 900, 18, &[]);
    add_root(&mut after, 11);
    add_root(&mut after, 12);

    let report = run_class_retained_diff(before, after, 0, 0);

    assert_eq!(report.added.len(), 2);
    assert!(report
        .added
        .iter()
        .any(|delta| delta.after_retained_bytes == 17));
    assert!(report
        .added
        .iter()
        .any(|delta| delta.after_retained_bytes == 18));
}

#[test]
fn bucket_bits_too_large_collapses_all_into_one_bucket() {
    let before = make_graph();

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Buffer");
    add_object(&mut after, 11, 900, 17, &[]);
    add_object(&mut after, 12, 900, 18, &[]);
    add_root(&mut after, 11);
    add_root(&mut after, 12);

    let report = run_class_retained_diff(before, after, 64, 0);

    assert_eq!(report.added.len(), 1);
    assert_eq!(report.added[0].after_count, 2);
    assert_eq!(report.added[0].after_retained_bytes, 35);
}

#[test]
fn min_retained_floor_filters_small_objects() {
    let before = make_graph();

    let mut after = make_graph();
    add_class(&mut after, 900, "com.example.Buffer");
    add_object(&mut after, 11, 900, 5, &[]);
    add_object(&mut after, 12, 900, 12, &[]);
    add_root(&mut after, 11);
    add_root(&mut after, 12);

    let report = run_class_retained_diff(before, after, 0, 10);

    assert_eq!(report.added.len(), 1);
    assert_eq!(report.added[0].after_count, 1);
    assert_eq!(report.added[0].after_retained_bytes, 12);
}
