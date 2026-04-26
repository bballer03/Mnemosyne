use byteorder::{BigEndian, WriteBytesExt};
use mnemosyne_core::{
    build_dominator_tree,
    hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectId, ObjectKind,
    },
    parse_hprof_with_options,
    query::{execute_query, parse_query, CellValue, QueryError},
    ParseOptions,
};

const TAG_STRING_IN_UTF8: u8 = 0x01;
const TAG_LOAD_CLASS: u8 = 0x02;
const TAG_HEAP_DUMP: u8 = 0x0C;
const SUB_ROOT_JAVA_FRAME: u8 = 0x03;
const SUB_CLASS_DUMP: u8 = 0x20;
const SUB_INSTANCE_DUMP: u8 = 0x21;
const TYPE_OBJECT: u8 = 2;

fn write_record(buf: &mut Vec<u8>, tag: u8, body: &[u8]) {
    buf.write_u8(tag).unwrap();
    buf.write_u32::<BigEndian>(0).unwrap();
    buf.write_u32::<BigEndian>(body.len() as u32).unwrap();
    buf.extend_from_slice(body);
}

fn object_ref_bytes(id: u64) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

fn int_field_bytes(value: i32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

fn string_field_bytes(value_array_id: u64, coder: i8) -> Vec<u8> {
    let mut bytes = object_ref_bytes(value_array_id);
    bytes.push(coder as u8);
    bytes
}

fn add_class(
    graph: &mut ObjectGraph,
    class_id: ObjectId,
    super_class_id: ObjectId,
    name: &str,
    fields: Vec<FieldDescriptor>,
) {
    graph.classes.insert(
        class_id,
        ClassInfo {
            class_obj_id: class_id,
            super_class_id,
            class_loader_id: 0,
            instance_size: 0,
            name: Some(name.into()),
            instance_fields: fields,
            static_references: Vec::new(),
        },
    );
}

fn add_string_object(
    graph: &mut ObjectGraph,
    string_id: ObjectId,
    array_id: ObjectId,
    value: &str,
) {
    graph.objects.insert(
        array_id,
        HeapObject {
            id: array_id,
            class_id: 0,
            shallow_size: value.len() as u32,
            references: Vec::new(),
            field_data: value.as_bytes().to_vec(),
            kind: ObjectKind::PrimitiveArray {
                element_type: field_types::BYTE,
                length: value.len() as u32,
            },
        },
    );

    graph.objects.insert(
        string_id,
        HeapObject {
            id: string_id,
            class_id: 2,
            shallow_size: 24,
            references: vec![array_id],
            field_data: string_field_bytes(array_id, 0),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: string_id,
        root_type: GcRootType::StickyClass,
    });
}

fn add_user_object(graph: &mut ObjectGraph, user_id: ObjectId, name_id: ObjectId, kind: i32) {
    let mut field_data = object_ref_bytes(name_id);
    field_data.extend_from_slice(&int_field_bytes(kind));
    let references = if name_id == 0 {
        Vec::new()
    } else {
        vec![name_id]
    };

    graph.objects.insert(
        user_id,
        HeapObject {
            id: user_id,
            class_id: 3,
            shallow_size: 32,
            references,
            field_data,
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: user_id,
        root_type: GcRootType::StickyClass,
    });
}

fn build_string_query_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(
        &mut graph,
        2,
        1,
        "java.lang.String",
        vec![
            FieldDescriptor {
                name: Some("value".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("coder".into()),
                field_type: field_types::BYTE,
            },
        ],
    );
    add_class(
        &mut graph,
        3,
        1,
        "com.example.User",
        vec![
            FieldDescriptor {
                name: Some("name".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("kind".into()),
                field_type: field_types::INT,
            },
        ],
    );

    add_string_object(&mut graph, 10, 100, "admin");
    add_string_object(&mut graph, 11, 101, "guest@1");
    add_string_object(&mut graph, 12, 102, ".foo");
    add_string_object(&mut graph, 13, 103, "exact");
    add_string_object(&mut graph, 14, 104, "adminRoot");
    add_string_object(&mut graph, 15, 105, "hello world");
    add_string_object(&mut graph, 16, 106, "aXbYc");

    add_user_object(&mut graph, 0x2000, 10, 1);
    add_user_object(&mut graph, 0x2001, 11, 2);
    add_user_object(&mut graph, 0x2002, 12, 3);
    add_user_object(&mut graph, 0x2003, 13, 4);
    add_user_object(&mut graph, 0x2004, 14, 7);
    add_user_object(&mut graph, 0x2005, 0, 8);
    add_user_object(&mut graph, 0x2006, 16, 9);

    graph
}

fn node_projection_field_bytes(
    parent_id: ObjectId,
    depth: i32,
    count: i32,
    payload_id: ObjectId,
) -> Vec<u8> {
    let mut field_data = object_ref_bytes(parent_id);
    field_data.extend_from_slice(&int_field_bytes(depth));
    field_data.extend_from_slice(&int_field_bytes(count));
    field_data.extend_from_slice(&object_ref_bytes(payload_id));
    field_data
}

fn add_projection_parent(graph: &mut ObjectGraph, object_id: ObjectId, class_id: ObjectId) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 16,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id,
        root_type: GcRootType::StickyClass,
    });
}

fn add_projection_payload(graph: &mut ObjectGraph, object_id: ObjectId, shallow_size: u32) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id: 6,
            shallow_size,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
}

fn add_projection_node(
    graph: &mut ObjectGraph,
    object_id: ObjectId,
    class_id: ObjectId,
    parent_id: ObjectId,
    depth: i32,
    count: i32,
    payload_id: ObjectId,
) {
    let mut references = Vec::new();
    if parent_id != 0 {
        references.push(parent_id);
    }
    if payload_id != 0 {
        references.push(payload_id);
    }

    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 32,
            references,
            field_data: node_projection_field_bytes(parent_id, depth, count, payload_id),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id,
        root_type: GcRootType::StickyClass,
    });
}

fn build_objects_projection_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(&mut graph, 2, 1, "com.example.ParentNode", Vec::new());
    add_class(&mut graph, 3, 1, "com.example.OtherParent", Vec::new());
    add_class(
        &mut graph,
        4,
        1,
        "com.example.Node",
        vec![
            FieldDescriptor {
                name: Some("parent".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("depth".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("count".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("payload".into()),
                field_type: field_types::OBJECT,
            },
        ],
    );
    add_class(&mut graph, 5, 4, "com.example.DeepNode", Vec::new());
    add_class(&mut graph, 6, 1, "com.example.Payload", Vec::new());

    add_projection_parent(&mut graph, 0x2100, 2);
    add_projection_parent(&mut graph, 0x2200, 2);
    add_projection_parent(&mut graph, 0x2300, 2);
    add_projection_parent(&mut graph, 0x2400, 3);

    add_projection_payload(&mut graph, 0x6100, 64);
    add_projection_payload(&mut graph, 0x6200, 8);
    add_projection_payload(&mut graph, 0x6400, 32);

    add_projection_node(&mut graph, 0x4100, 4, 0x2100, 4, 1, 0x6100);
    add_projection_node(&mut graph, 0x4200, 5, 0x2200, 9, 2, 0x6200);
    add_projection_node(&mut graph, 0x4300, 4, 0x2300, 8, 3, 0);
    add_projection_node(&mut graph, 0x4400, 4, 0x2300, 7, 4, 0x6400);
    add_projection_node(&mut graph, 0x4500, 4, 0, 10, 0, 0);
    add_projection_node(&mut graph, 0x4600, 4, 0x9999, 11, 0, 0);
    add_projection_node(&mut graph, 0x4700, 4, 0x2400, 6, 5, 0);

    graph
}

fn build_graph_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    bytes.write_u32::<BigEndian>(4).unwrap();
    bytes.write_u64::<BigEndian>(0).unwrap();

    let mut string_body = Vec::new();
    string_body.write_u32::<BigEndian>(1).unwrap();
    string_body.extend_from_slice(b"java/lang/Object");
    write_record(&mut bytes, TAG_STRING_IN_UTF8, &string_body);

    let mut string_body = Vec::new();
    string_body.write_u32::<BigEndian>(2).unwrap();
    string_body.extend_from_slice(b"com/example/BigCache");
    write_record(&mut bytes, TAG_STRING_IN_UTF8, &string_body);

    let mut string_body = Vec::new();
    string_body.write_u32::<BigEndian>(3).unwrap();
    string_body.extend_from_slice(b"entries");
    write_record(&mut bytes, TAG_STRING_IN_UTF8, &string_body);

    let mut load_class = Vec::new();
    load_class.write_u32::<BigEndian>(1).unwrap();
    load_class.write_u32::<BigEndian>(0x100).unwrap();
    load_class.write_u32::<BigEndian>(0).unwrap();
    load_class.write_u32::<BigEndian>(1).unwrap();
    write_record(&mut bytes, TAG_LOAD_CLASS, &load_class);

    let mut load_class = Vec::new();
    load_class.write_u32::<BigEndian>(2).unwrap();
    load_class.write_u32::<BigEndian>(0x200).unwrap();
    load_class.write_u32::<BigEndian>(0).unwrap();
    load_class.write_u32::<BigEndian>(2).unwrap();
    write_record(&mut bytes, TAG_LOAD_CLASS, &load_class);

    let mut heap = Vec::new();
    heap.write_u8(SUB_CLASS_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x100).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    for _ in 0..5 {
        heap.write_u32::<BigEndian>(0).unwrap();
    }
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();

    heap.write_u8(SUB_CLASS_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x200).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0x100).unwrap();
    for _ in 0..5 {
        heap.write_u32::<BigEndian>(0).unwrap();
    }
    heap.write_u32::<BigEndian>(4).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(1).unwrap();
    heap.write_u32::<BigEndian>(3).unwrap();
    heap.write_u8(TYPE_OBJECT).unwrap();

    heap.write_u8(SUB_ROOT_JAVA_FRAME).unwrap();
    heap.write_u32::<BigEndian>(0x1000).unwrap();
    heap.write_u32::<BigEndian>(1).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();

    heap.write_u8(SUB_INSTANCE_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x1000).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0x200).unwrap();
    heap.write_u32::<BigEndian>(4).unwrap();
    heap.write_u32::<BigEndian>(0x2000).unwrap();

    heap.write_u8(SUB_INSTANCE_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x2000).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0x100).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();

    write_record(&mut bytes, TAG_HEAP_DUMP, &heap);
    bytes
}

fn build_retained_size_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    bytes.write_u32::<BigEndian>(4).unwrap();
    bytes.write_u64::<BigEndian>(0).unwrap();

    for (id, value) in [
        (1, "java/lang/Object"),
        (2, "com/example/BigCache"),
        (3, "entries"),
        (4, "com/example/SmallEntry"),
        (5, "com/example/MediumEntry"),
        (6, "com/example/LargeEntry"),
    ] {
        let mut string_body = Vec::new();
        string_body.write_u32::<BigEndian>(id).unwrap();
        string_body.extend_from_slice(value.as_bytes());
        write_record(&mut bytes, TAG_STRING_IN_UTF8, &string_body);
    }

    for (serial, class_obj_id, name_string_id) in [
        (1, 0x100, 1),
        (2, 0x200, 2),
        (3, 0x300, 4),
        (4, 0x400, 5),
        (5, 0x500, 6),
    ] {
        let mut load_class = Vec::new();
        load_class.write_u32::<BigEndian>(serial).unwrap();
        load_class.write_u32::<BigEndian>(class_obj_id).unwrap();
        load_class.write_u32::<BigEndian>(0).unwrap();
        load_class.write_u32::<BigEndian>(name_string_id).unwrap();
        write_record(&mut bytes, TAG_LOAD_CLASS, &load_class);
    }

    let mut heap = Vec::new();

    heap.write_u8(SUB_CLASS_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x100).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    for _ in 0..5 {
        heap.write_u32::<BigEndian>(0).unwrap();
    }
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();

    heap.write_u8(SUB_CLASS_DUMP).unwrap();
    heap.write_u32::<BigEndian>(0x200).unwrap();
    heap.write_u32::<BigEndian>(0).unwrap();
    heap.write_u32::<BigEndian>(0x100).unwrap();
    for _ in 0..5 {
        heap.write_u32::<BigEndian>(0).unwrap();
    }
    heap.write_u32::<BigEndian>(4).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(0).unwrap();
    heap.write_u16::<BigEndian>(1).unwrap();
    heap.write_u32::<BigEndian>(3).unwrap();
    heap.write_u8(TYPE_OBJECT).unwrap();

    for (class_id, instance_size) in [(0x300, 500), (0x400, 1000), (0x500, 2000)] {
        heap.write_u8(SUB_CLASS_DUMP).unwrap();
        heap.write_u32::<BigEndian>(class_id).unwrap();
        heap.write_u32::<BigEndian>(0).unwrap();
        heap.write_u32::<BigEndian>(0x100).unwrap();
        for _ in 0..5 {
            heap.write_u32::<BigEndian>(0).unwrap();
        }
        heap.write_u32::<BigEndian>(instance_size).unwrap();
        heap.write_u16::<BigEndian>(0).unwrap();
        heap.write_u16::<BigEndian>(0).unwrap();
        heap.write_u16::<BigEndian>(0).unwrap();
    }

    for object_id in [0x1000_u32, 0x1100, 0x1200, 0x1300] {
        heap.write_u8(SUB_ROOT_JAVA_FRAME).unwrap();
        heap.write_u32::<BigEndian>(object_id).unwrap();
        heap.write_u32::<BigEndian>(1).unwrap();
        heap.write_u32::<BigEndian>(0).unwrap();
    }

    for (object_id, entry_id) in [
        (0x1000_u32, 0x3000_u32),
        (0x1100, 0x4000),
        (0x1200, 0x5000),
        (0x1300, 0),
    ] {
        heap.write_u8(SUB_INSTANCE_DUMP).unwrap();
        heap.write_u32::<BigEndian>(object_id).unwrap();
        heap.write_u32::<BigEndian>(0).unwrap();
        heap.write_u32::<BigEndian>(0x200).unwrap();
        heap.write_u32::<BigEndian>(4).unwrap();
        heap.write_u32::<BigEndian>(entry_id).unwrap();
    }

    for (object_id, class_id) in [(0x3000_u32, 0x300_u32), (0x4000, 0x400), (0x5000, 0x500)] {
        heap.write_u8(SUB_INSTANCE_DUMP).unwrap();
        heap.write_u32::<BigEndian>(object_id).unwrap();
        heap.write_u32::<BigEndian>(0).unwrap();
        heap.write_u32::<BigEndian>(class_id).unwrap();
        heap.write_u32::<BigEndian>(0).unwrap();
    }

    write_record(&mut bytes, TAG_HEAP_DUMP, &heap);
    bytes
}

#[test]
fn where_retained_size_in_overview_mode_returns_unavailable_error() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 1000"#)
            .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode retained-size predicates should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "@retainedSize" && hint.contains("--mode deep")
    ));
}

#[test]
fn where_retained_size_gt_threshold_filters_correctly() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 1000"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x1100)], vec![CellValue::Id(0x1200)]]
    );
}

#[test]
fn where_retained_size_lt_threshold_filters_correctly() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize < 1000"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x1000)], vec![CellValue::Id(0x1300)]]
    );
}

#[test]
fn where_retained_size_eq_threshold_filters_correctly() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize = 1004"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.rows, vec![vec![CellValue::Id(0x1100)]]);
}

#[test]
fn where_retained_size_no_match_returns_empty_result() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 3000"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 0);
    assert!(result.rows.is_empty());
}

#[test]
fn select_retained_size_projects_into_result_column() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId, @retainedSize FROM "com.example.BigCache""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "@retainedSize"]);
    assert_eq!(
        result.rows,
        vec![
            vec![CellValue::Id(0x1000), CellValue::Int(504)],
            vec![CellValue::Id(0x1100), CellValue::Int(1004)],
            vec![CellValue::Id(0x1200), CellValue::Int(2004)],
            vec![CellValue::Id(0x1300), CellValue::Int(4)],
        ]
    );
}

#[test]
fn select_retained_size_in_overview_mode_returns_unavailable_error() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let query = parse_query(r#"SELECT @retainedSize FROM "com.example.BigCache""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode retained-size projection should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "@retainedSize" && hint.contains("--mode deep")
    ));
}

#[test]
fn where_retained_size_combined_with_instanceof_works() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 1000 AND entries INSTANCEOF "java.lang.Object""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x1100)], vec![CellValue::Id(0x1200)]]
    );
}

#[test]
fn where_retained_size_combined_with_class_field_eq_works() {
    let graph = parse_hprof_with_options(
        &build_retained_size_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 1000 AND entries = 16384"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.rows, vec![vec![CellValue::Id(0x1100)]]);
}

#[test]
fn execute_query_returns_builtin_fields_for_matching_objects() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId, @className, @shallowSize, @retainedSize FROM "com.example.BigCache" WHERE @retainedSize > 1"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.columns,
        vec!["@objectId", "@className", "@shallowSize", "@retainedSize"]
    );
    assert_eq!(result.total_matched, 1);
    assert!(!result.truncated);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0],
        vec![
            CellValue::Id(0x1000),
            CellValue::Str("com.example.BigCache".into()),
            CellValue::Int(4),
            CellValue::Int(4),
        ]
    );
}

#[test]
fn execute_query_applies_limit_after_matching() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "java.lang.Object" LIMIT 1"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.rows.len(), 1);
}

#[test]
fn execute_query_matches_from_instanceof_across_superclasses() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM INSTANCEOF "java.lang.Object""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId"]);
    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x1000)], vec![CellValue::Id(0x2000)]]
    );
}

#[test]
fn execute_query_projects_and_filters_instance_fields() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId, entries FROM "com.example.BigCache" WHERE entries = 8192"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "entries"]);
    assert_eq!(result.total_matched, 1);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x1000), CellValue::Id(0x2000)]]
    );
}

#[test]
fn execute_query_supports_instanceof_filters_on_instance_fields() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.BigCache" WHERE entries INSTANCEOF "java.lang.Object""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.rows, vec![vec![CellValue::Id(0x1000)]]);
}

#[test]
fn objects_projection_returns_referenced_target() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE count = 1"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "@className"]);
    assert_eq!(result.total_matched, 1);
    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Id(0x2100),
            CellValue::Str("com.example.ParentNode".into()),
        ]]
    );
}

#[test]
fn objects_projection_omits_rows_with_null_field() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE depth = 10"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 0);
    assert!(result.rows.is_empty());
}

#[test]
fn objects_projection_omits_rows_with_unresolved_target() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE depth = 11"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 0);
    assert!(result.rows.is_empty());
}

#[test]
fn objects_projection_keeps_duplicates_when_multiple_sources_share_target() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE count >= 3 AND count < 5"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
        ]
    );
}

#[test]
fn objects_projection_combined_with_where_filter() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE depth > 5"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 3);
    assert_eq!(
        result.rows,
        vec![
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2400),
                CellValue::Str("com.example.OtherParent".into()),
            ],
        ]
    );
}

#[test]
fn objects_projection_combined_with_instanceof() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT OBJECTS n.parent FROM INSTANCEOF "com.example.Node" WHERE parent INSTANCEOF "com.example.ParentNode""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 4);
    assert_eq!(
        result.rows,
        vec![
            vec![
                CellValue::Id(0x2100),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2200),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
        ]
    );
}

#[test]
fn objects_projection_in_overview_mode_returns_unavailable_error() {
    let graph = build_objects_projection_graph();
    let query = parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode OBJECTS should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "OBJECTS" && hint.contains("OBJECTS") && hint.contains("--mode deep")
    ));
}

#[test]
fn objects_projection_on_primitive_field_returns_clear_error() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.count FROM "com.example.Node""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("primitive OBJECTS fields should fail clearly");

    assert!(error.to_string().contains("count"));
    assert!(error.to_string().contains("object-reference"));
}

#[test]
fn objects_projection_multi_hop_returns_not_supported_error() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.parent.parent FROM "com.example.Node""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("multi-hop OBJECTS should be deferred cleanly");

    assert!(error.to_string().contains("multi-hop OBJECTS"));
    assert!(error.to_string().contains("not yet supported"));
}

#[test]
fn objects_projection_with_unknown_field_returns_clear_error() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT OBJECTS n.missing FROM "com.example.Node""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("missing OBJECTS fields should fail clearly");

    assert!(error.to_string().contains("missing"));
    assert!(error.to_string().contains("com.example.Node"));
}

#[test]
fn objects_projection_combined_with_at_retained_size_predicate_works() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE @retainedSize > 60"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 2);
    assert_eq!(
        result.rows,
        vec![
            vec![
                CellValue::Id(0x2100),
                CellValue::Str("com.example.ParentNode".into()),
            ],
            vec![
                CellValue::Id(0x2300),
                CellValue::Str("com.example.ParentNode".into()),
            ],
        ]
    );
}

#[test]
fn like_with_percent_wildcard_matches_prefix() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "admin%""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2004)]]
    );
}

#[test]
fn like_with_percent_wildcard_matches_suffix() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "%@1""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2001)]]);
}

#[test]
fn like_with_underscore_matches_single_char() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "admi_""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2000)]]);
}

#[test]
fn like_with_no_wildcards_matches_exact() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "exact""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2003)]]);
}

#[test]
fn like_with_special_regex_chars_in_pattern_treated_literally() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE ".foo""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2002)]]);
}

#[test]
fn like_with_multiple_percent_wildcards_matches_segments_in_order() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "a%b%c""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2006)]]);
}

#[test]
fn like_no_match_returns_empty_result() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "nomatch%""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn like_in_overview_mode_returns_unavailable_error() {
    let graph = build_string_query_graph();
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "admin%""#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode string predicates should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "name" && hint.contains("--mode deep")
    ));
}

#[test]
fn contains_with_substring_matches() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name CONTAINS "min""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2004)]]
    );
}

#[test]
fn contains_with_no_match_returns_empty() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name CONTAINS "zzz""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn contains_in_overview_mode_returns_unavailable_error() {
    let graph = build_string_query_graph();
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name CONTAINS "min""#)
            .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode string predicates should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "name" && hint.contains("--mode deep")
    ));
}

#[test]
fn where_at_to_string_like_matches_string_content() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "java.lang.String" WHERE @toString LIKE "hello%""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(15)]]);
}

#[test]
fn where_at_to_string_contains_substring_works() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "java.lang.String" WHERE @toString CONTAINS "world""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(15)]]);
}

#[test]
fn select_at_to_string_projects_into_result_column() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId, @toString FROM "java.lang.String" LIMIT 3"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "@toString"]);
    assert_eq!(
        result.rows,
        vec![
            vec![CellValue::Id(10), CellValue::Str("admin".into())],
            vec![CellValue::Id(11), CellValue::Str("guest@1".into())],
            vec![CellValue::Id(12), CellValue::Str(".foo".into())],
        ]
    );
}

#[test]
fn at_to_string_in_overview_mode_returns_unavailable_error() {
    let graph = build_string_query_graph();
    let query =
        parse_query(r#"SELECT @objectId FROM "java.lang.String" WHERE @toString LIKE "hello%""#)
            .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode @toString predicates should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "@toString" && hint.contains("--mode deep")
    ));
}

#[test]
fn like_combined_with_instanceof_works() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE name LIKE "admin%" AND name INSTANCEOF "java.lang.String""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2004)]]
    );
}

#[test]
fn contains_combined_with_field_equality_works() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE name CONTAINS "min" AND kind = 7"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2004)]]);
}

#[test]
fn field_ne_null_matches_non_null_object_ref() {
    let graph = parse_hprof_with_options(
        &build_graph_fixture(),
        ParseOptions {
            retain_field_data: true,
        },
    )
    .expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE entries != null"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x1000)]]);
}
