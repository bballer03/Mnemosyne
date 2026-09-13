use byteorder::{BigEndian, WriteBytesExt};
use mnemosyne_core::{
    build_dominator_tree,
    hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectId, ObjectKind,
    },
    parse_hprof_with_options,
    query::{
        execute_query, execute_query_statement, parse_query, parse_query_statement, CellValue,
        QueryError, QueryStatement,
    },
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

fn add_owner_object(graph: &mut ObjectGraph, object_id: ObjectId, class_id: ObjectId) {
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
}

fn add_gc_root_target(
    graph: &mut ObjectGraph,
    object_id: ObjectId,
    class_id: ObjectId,
    owner_id: ObjectId,
) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 32,
            references: vec![owner_id],
            field_data: object_ref_bytes(owner_id),
            kind: ObjectKind::Instance,
        },
    );
}

fn build_gc_root_path_query_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(
        &mut graph,
        2,
        1,
        "com.example.ThreadLocalHolder",
        Vec::new(),
    );
    add_class(&mut graph, 3, 1, "com.example.PathNode", Vec::new());
    add_class(
        &mut graph,
        4,
        1,
        "com.example.Target",
        vec![FieldDescriptor {
            name: Some("owner".into()),
            field_type: field_types::OBJECT,
        }],
    );
    add_class(&mut graph, 5, 1, "com.example.Owner", Vec::new());
    add_class(&mut graph, 6, 1, "com.example.OtherOwner", Vec::new());

    graph.objects.insert(
        0x7100,
        HeapObject {
            id: 0x7100,
            class_id: 2,
            shallow_size: 24,
            references: vec![0x7200],
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.objects.insert(
        0x7200,
        HeapObject {
            id: 0x7200,
            class_id: 3,
            shallow_size: 24,
            references: vec![0x7300],
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    add_owner_object(&mut graph, 0x7400, 5);
    add_owner_object(&mut graph, 0x7401, 6);
    add_gc_root_target(&mut graph, 0x7300, 4, 0x7400);
    add_gc_root_target(&mut graph, 0x7301, 4, 0x7401);

    graph.gc_roots.push(GcRoot {
        object_id: 0x7100,
        root_type: GcRootType::JniGlobal,
    });

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

// M15 Slice 15.D: regex `=~` operator.

#[test]
fn regex_matches_prefix_pattern_on_instance_field() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2004)]]
    );
}

#[test]
fn regex_excludes_non_matching_instance_field() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    let matched_ids: Vec<_> = result.rows.into_iter().flatten().collect();
    assert!(!matched_ids.contains(&CellValue::Id(0x2001))); // "guest@1"
    assert!(!matched_ids.contains(&CellValue::Id(0x2002))); // ".foo"
    assert!(!matched_ids.contains(&CellValue::Id(0x2003))); // "exact"
}

#[test]
fn regex_no_match_returns_empty_result() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^zzz.*""#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn regex_in_overview_mode_returns_unavailable_error() {
    let graph = build_string_query_graph();
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*""#)
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
fn regex_combined_with_instanceof_works() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*" AND name INSTANCEOF "java.lang.String""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2004)]]
    );
}

#[test]
fn regex_combined_with_contains_via_or() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin$" OR name CONTAINS "exact""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2000)], vec![CellValue::Id(0x2003)]]
    );
}

#[test]
fn regex_combined_with_like_via_and() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*" AND name LIKE "%Root""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x2004)]]);
}

#[test]
fn malformed_regex_pattern_built_directly_returns_structured_error_not_panic() {
    // Bypasses the parser's own eager validation (constructs the `Query`
    // directly, as an MCP/library caller might) to prove the executor's
    // independent compile-once-per-query step also fails structurally
    // rather than panicking on a pattern the parser never got to see.
    use mnemosyne_core::query::{
        BuiltInField, ComparisonOp, Condition, FieldRef, FromClause, Query, SelectClause, Value,
        WhereClause,
    };

    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = Query {
        select: SelectClause::Fields(vec![FieldRef::BuiltIn(BuiltInField::ObjectId)]),
        from: FromClause {
            class_pattern: mnemosyne_core::query::ClassPattern::Exact("com.example.User".into()),
            instanceof: false,
        },
        filter: Some(WhereClause {
            conditions: vec![Condition {
                field: FieldRef::InstanceField("name".into()),
                op: ComparisonOp::RegexMatch,
                value: Value::Str("(unclosed".into()),
            }],
            operators: Vec::new(),
        }),
        limit: None,
    };

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("malformed regex pattern should fail structurally, not panic");

    assert!(matches!(error, QueryError::Unsupported(_)));
}

#[test]
fn where_at_to_string_regex_matches_string_content() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "java.lang.String" WHERE @toString =~ "^hello.*""#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(15)]]);
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

#[test]
fn where_at_gc_root_path_contains_classname_matches() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.Target" WHERE @gcRootPath CONTAINS "ThreadLocal""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x7300)]]);
}

#[test]
fn where_at_gc_root_path_contains_no_match_returns_empty() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.Target" WHERE @gcRootPath CONTAINS "Nope""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 0);
    assert!(result.rows.is_empty());
}

#[test]
fn where_at_gc_root_path_like_with_wildcard_matches() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.Target" WHERE @gcRootPath LIKE "GcRoot/JniGlobal -> %PathNode -> com.example.Target""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x7300)]]);
}

#[test]
fn where_at_gc_root_path_for_unreachable_object_evaluates_false() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.Target" WHERE owner INSTANCEOF "com.example.OtherOwner" AND @gcRootPath CONTAINS "ThreadLocal""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 0);
    assert!(result.rows.is_empty());
}

#[test]
fn select_at_gc_root_path_projects_joined_string() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @gcRootPath FROM "com.example.Target" WHERE owner INSTANCEOF "com.example.Owner""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@gcRootPath"]);
    assert_eq!(
        result.rows,
        vec![vec![CellValue::Str(
            "GcRoot/JniGlobal -> com.example.ThreadLocalHolder -> com.example.PathNode -> com.example.Target"
                .into(),
        )]],
    );
}

#[test]
fn select_at_gc_root_path_for_unreachable_object_returns_null() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @gcRootPath FROM "com.example.Target" WHERE owner INSTANCEOF "com.example.OtherOwner""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Null]]);
}

#[test]
fn at_gc_root_path_in_overview_mode_returns_unavailable_error() {
    let graph = build_gc_root_path_query_graph();
    let query =
        parse_query(r#"SELECT @gcRootPath FROM "com.example.Target""#).expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode @gcRootPath should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "@gcRootPath" && hint.contains("--mode deep")
    ));
}

#[test]
fn at_gc_root_path_path_format_uses_arrow_separator() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @gcRootPath FROM "com.example.Target" WHERE owner INSTANCEOF "com.example.Owner""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    let CellValue::Str(path) = &result.rows[0][0] else {
        panic!("expected gc-root path string");
    };

    assert!(path.contains(" -> "));
    assert!(!path.contains(';'));
}

#[test]
fn at_gc_root_path_includes_root_kind_as_first_frame() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @gcRootPath FROM "com.example.Target" WHERE owner INSTANCEOF "com.example.Owner""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    let CellValue::Str(path) = &result.rows[0][0] else {
        panic!("expected gc-root path string");
    };

    assert!(path.starts_with("GcRoot/JniGlobal -> "));
}

#[test]
fn where_field_is_null_matches_objects_with_null_ref() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.Node" WHERE payload IS NULL"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![
            vec![CellValue::Id(0x4300)],
            vec![CellValue::Id(0x4500)],
            vec![CellValue::Id(0x4600)],
            vec![CellValue::Id(0x4700)],
        ]
    );
}

#[test]
fn where_field_is_not_null_matches_objects_with_resolved_ref() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT @objectId FROM "com.example.Node" WHERE payload IS NOT NULL"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x4100)], vec![CellValue::Id(0x4400)]],
    );
}

#[test]
fn is_null_on_primitive_field_returns_clear_error() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.Node" WHERE count IS NULL"#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("primitive IS NULL should fail clearly");

    assert!(error.to_string().contains("count"));
    assert!(error.to_string().contains("primitive"));
}

#[test]
fn is_null_on_unknown_field_returns_clear_error() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "com.example.Node" WHERE missing IS NULL"#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, Some(&dominator))
        .expect_err("unknown IS NULL fields should fail clearly");

    assert!(error.to_string().contains("missing"));
    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn is_null_in_overview_mode_returns_unavailable_error() {
    let graph = build_objects_projection_graph();
    let query = parse_query(r#"SELECT @objectId FROM "com.example.Node" WHERE payload IS NULL"#)
        .expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode IS NULL should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { hint, .. } if hint.contains("--mode deep")
    ));
}

#[test]
fn at_gc_root_path_combined_with_at_retained_size_and_instanceof() {
    let graph = build_gc_root_path_query_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.Target" WHERE @gcRootPath CONTAINS "ThreadLocal" AND @retainedSize > 40 AND owner INSTANCEOF "com.example.Owner""#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(0x7300)]]);
}

// ---------------------------------------------------------------------------
// M15 Slice 15.C: outbounds/inbounds/dominators traversal functions
// ---------------------------------------------------------------------------

fn add_traversal_node(
    graph: &mut ObjectGraph,
    object_id: ObjectId,
    class_id: ObjectId,
    references: Vec<ObjectId>,
    is_gc_root: bool,
) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 16,
            references,
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    if is_gc_root {
        graph.gc_roots.push(GcRoot {
            object_id,
            root_type: GcRootType::StickyClass,
        });
    }
}

/// Fixture for `outbounds`/`inbounds` equivalence tests: object `1` (a GC
/// root) references `2` and `3`; object `4` (also a GC root) additionally
/// references `3`, giving `3` two referrers for a meaningful `inbounds` test.
fn build_outbounds_inbounds_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(&mut graph, 2, 1, "com.example.Node", Vec::new());

    add_traversal_node(&mut graph, 1, 2, vec![2, 3], true);
    add_traversal_node(&mut graph, 2, 2, Vec::new(), false);
    add_traversal_node(&mut graph, 3, 2, Vec::new(), false);
    add_traversal_node(&mut graph, 4, 2, vec![3], true);

    graph
}

/// Fixture for `dominators` chain tests: a clean, non-diverging chain
/// `10 -> 20 -> 30` (each intermediate node has exactly one referrer) so the
/// dominator tree's immediate-dominator chain is unambiguous and walkable.
fn build_dominator_chain_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(&mut graph, 2, 1, "com.example.Node", Vec::new());

    add_traversal_node(&mut graph, 10, 2, vec![20], true);
    add_traversal_node(&mut graph, 20, 2, vec![30], false);
    add_traversal_node(&mut graph, 30, 2, Vec::new(), false);

    graph
}

#[test]
fn outbounds_matches_direct_get_references_call() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM outbounds(1)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    let mut expected: Vec<CellValue> = graph
        .get_references(1)
        .into_iter()
        .map(CellValue::Id)
        .collect();
    expected.sort_by_key(|cell| match cell {
        CellValue::Id(id) => *id,
        _ => unreachable!(),
    });
    let actual: Vec<CellValue> = result.rows.into_iter().map(|row| row[0].clone()).collect();
    assert_eq!(actual, expected);
    assert_eq!(actual, vec![CellValue::Id(2), CellValue::Id(3)]);
}

#[test]
fn inbounds_matches_direct_get_referrers_call() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM inbounds(3)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    let mut expected: Vec<CellValue> = graph
        .get_referrers(3)
        .into_iter()
        .map(CellValue::Id)
        .collect();
    expected.sort_by_key(|cell| match cell {
        CellValue::Id(id) => *id,
        _ => unreachable!(),
    });
    let actual: Vec<CellValue> = result.rows.into_iter().map(|row| row[0].clone()).collect();
    assert_eq!(actual, expected);
    assert_eq!(actual, vec![CellValue::Id(1), CellValue::Id(4)]);
}

#[test]
fn dominators_matches_direct_immediate_dominator_walk() {
    let graph = build_dominator_chain_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM dominators(30)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    // Manually walk immediate_dominator the same way the executor's
    // resolve_dominator_chain does, to prove equivalence per this project's
    // "zero new analysis logic, pure composition" discipline. The traversal
    // set then flows through the same `matched_ids.sort_unstable()` every
    // other FROM source does, so results come back in ascending object-id
    // order, not dominance-chain order.
    let first = dominator
        .immediate_dominator(30)
        .expect("30 should be dominated");
    let second = dominator
        .immediate_dominator(first)
        .expect("intermediate node should be dominated");
    let mut expected = vec![CellValue::Id(first), CellValue::Id(second)];
    expected.sort_by_key(|cell| match cell {
        CellValue::Id(id) => *id,
        _ => unreachable!(),
    });
    let actual: Vec<CellValue> = result.rows.into_iter().map(|row| row[0].clone()).collect();
    assert_eq!(actual, expected);
    assert_eq!(actual, vec![CellValue::Id(10), CellValue::Id(20)]);
}

#[test]
fn dominators_chain_stops_before_virtual_root() {
    let graph = build_dominator_chain_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM dominators(10)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    // 10 is a direct GC-root child of the virtual super-root; its only
    // "dominator" is the virtual root itself, which must never leak into
    // OQL results.
    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn outbounds_on_unknown_object_id_returns_empty_result() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM outbounds(999999)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
    assert!(!result.truncated);
}

#[test]
fn inbounds_on_unknown_object_id_returns_empty_result() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT @objectId FROM inbounds(999999)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn dominators_on_unknown_object_id_returns_empty_result() {
    let graph = build_dominator_chain_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query("SELECT @objectId FROM dominators(999999)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert!(result.rows.is_empty());
    assert_eq!(result.total_matched, 0);
}

#[test]
fn outbounds_combined_with_where_filter() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM outbounds(1) WHERE @objectId = 3"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows, vec![vec![CellValue::Id(3)]]);
    assert_eq!(result.total_matched, 1);
}

#[test]
fn outbounds_combined_with_limit() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query("SELECT @objectId FROM outbounds(1) LIMIT 1").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows.len(), 1);
    assert!(result.truncated);
    assert_eq!(result.total_matched, 1);
}

#[test]
fn inbounds_combined_with_where_and_limit() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM inbounds(3) WHERE @objectId > 0 LIMIT 1"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.rows.len(), 1);
    assert!(result.truncated);
}

#[test]
fn dominators_in_overview_mode_returns_unavailable_error() {
    let graph = build_dominator_chain_graph();
    let query = parse_query("SELECT @objectId FROM dominators(30)").expect("query should parse");

    let error = execute_query(&query, &graph, None)
        .expect_err("overview-mode dominators() should fail structurally");

    assert!(matches!(
        error,
        QueryError::FeatureUnavailableInOverviewMode { feature, hint }
            if feature == "dominators(...)" && hint.contains("--mode deep")
    ));
}

#[test]
fn outbounds_select_all_projects_object_id_and_class_name() {
    let graph = build_outbounds_inbounds_graph();
    let dominator = build_dominator_tree(&graph);
    let query = parse_query("SELECT * FROM outbounds(1)").expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "@className"]);
    assert_eq!(
        result.rows,
        vec![
            vec![CellValue::Id(2), CellValue::Str("com.example.Node".into())],
            vec![CellValue::Id(3), CellValue::Str("com.example.Node".into())],
        ]
    );
}

#[test]
fn is_null_combined_with_objects_projection() {
    let graph = build_objects_projection_graph();
    let dominator = build_dominator_tree(&graph);
    let query =
        parse_query(r#"SELECT OBJECTS n.parent FROM "com.example.Node" WHERE payload IS NULL"#)
            .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.columns, vec!["@objectId", "@className"]);
    assert_eq!(
        result.rows,
        vec![
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

// M15 Slice 15.E: one-level subqueries (`FROM OBJECTS (<subquery>)`) and `UNION`.

#[test]
fn subquery_restricts_outer_candidates_matches_direct_inner_query() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    // Equivalence-check discipline (per this session's established "zero
    // new analysis logic, prove it via direct comparison" convention):
    // the outer subquery-sourced query, with no WHERE/LIMIT of its own,
    // must produce exactly the same rows as running the inner query on
    // its own.
    let inner = parse_query(r#"SELECT * FROM "com.example.User" WHERE kind > 5"#)
        .expect("inner query should parse");
    let inner_result =
        execute_query(&inner, &graph, Some(&dominator)).expect("inner query should execute");

    let outer =
        parse_query(r#"SELECT * FROM OBJECTS (SELECT * FROM "com.example.User" WHERE kind > 5)"#)
            .expect("outer subquery should parse");
    let outer_result =
        execute_query(&outer, &graph, Some(&dominator)).expect("outer subquery should execute");

    assert_eq!(outer_result.rows, inner_result.rows);
    assert_eq!(
        outer_result.rows,
        vec![
            vec![
                CellValue::Id(0x2004),
                CellValue::Str("com.example.User".into())
            ],
            vec![
                CellValue::Id(0x2005),
                CellValue::Str("com.example.User".into())
            ],
            vec![
                CellValue::Id(0x2006),
                CellValue::Str("com.example.User".into())
            ],
        ]
    );
}

#[test]
fn subquery_composed_with_outbounds_and_regex_in_inner_where() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    // 0x2004 == 8196; its sole outbound reference is object 14, the
    // `java.lang.String` instance backing its `name` field. Proves a
    // subquery's inner FROM/WHERE can freely use 15.C's traversal
    // functions and 15.D's regex operator -- the subquery pipeline simply
    // re-invokes the same query executor recursively once.
    let query = parse_query(
        r#"SELECT * FROM OBJECTS (SELECT * FROM outbounds(8196) WHERE @className =~ "^java\.lang\..*")"#,
    )
    .expect("subquery composed with outbounds+regex should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Id(14),
            CellValue::Str("java.lang.String".into())
        ]]
    );
}

#[test]
fn subquery_inner_where_still_applies_when_outer_where_present() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    // Inner restricts to kind > 5 (0x2004, 0x2005, 0x2006); outer further
    // restricts to @objectId != 0x2005 (8197). Both WHERE clauses must
    // apply -- the outer query's own filter runs on top of the subquery's
    // candidate set, exactly like it runs on top of a traversal function's
    // candidate set.
    let query = parse_query(
        r#"SELECT @objectId FROM OBJECTS (SELECT * FROM "com.example.User" WHERE kind > 5) WHERE @objectId != 8197"#,
    )
    .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(
        result.rows,
        vec![vec![CellValue::Id(0x2004)], vec![CellValue::Id(0x2006)]]
    );
}

#[test]
fn doubly_nested_subquery_built_directly_returns_structured_error_not_infinite_recursion() {
    // Bypasses the parser's own nesting-depth rejection (constructs the
    // `Query` directly, as an MCP/library caller might) to prove the
    // executor's independent depth guard also fails structurally rather
    // than recursing without bound (M15 Slice 15.E defense-in-depth,
    // mirroring 15.D's `malformed_regex_pattern_built_directly...` test).
    use mnemosyne_core::query::{ClassPattern, FromClause, Query, SelectClause};

    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    let innermost = Query {
        select: SelectClause::All,
        from: FromClause {
            class_pattern: ClassPattern::Exact("com.example.User".into()),
            instanceof: false,
        },
        filter: None,
        limit: None,
    };
    let middle = Query {
        select: SelectClause::All,
        from: FromClause {
            class_pattern: ClassPattern::Subquery(Box::new(innermost)),
            instanceof: false,
        },
        filter: None,
        limit: None,
    };
    let outer = Query {
        select: SelectClause::All,
        from: FromClause {
            class_pattern: ClassPattern::Subquery(Box::new(middle)),
            instanceof: false,
        },
        filter: None,
        limit: None,
    };

    let error = execute_query(&outer, &graph, Some(&dominator))
        .expect_err("doubly nested subquery should fail structurally, not recurse unbounded");

    assert!(matches!(error, QueryError::Unsupported(_)));
    assert!(error.to_string().to_lowercase().contains("nesting"));
}

#[test]
fn execute_query_statement_single_matches_execute_query_directly() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    let query = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE kind = 7"#)
        .expect("query should parse");
    let direct = execute_query(&query, &graph, Some(&dominator)).expect("direct execute");

    let statement =
        parse_query_statement(r#"SELECT @objectId FROM "com.example.User" WHERE kind = 7"#)
            .expect("statement should parse");
    assert!(matches!(statement, QueryStatement::Single(_)));
    let via_statement =
        execute_query_statement(&statement, &graph, Some(&dominator)).expect("statement execute");

    assert_eq!(direct, via_statement);
}

#[test]
fn union_of_disjoint_queries_returns_combined_set() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    let statement = parse_query_statement(
        r#"SELECT @objectId FROM "com.example.User" WHERE kind < 3 UNION SELECT @objectId FROM "com.example.User" WHERE kind > 7"#,
    )
    .expect("union query should parse");
    assert!(matches!(statement, QueryStatement::Union(_, _)));

    let result = execute_query_statement(&statement, &graph, Some(&dominator))
        .expect("union query should execute");

    let ids: Vec<u64> = result
        .rows
        .iter()
        .map(|row| match row[0] {
            CellValue::Id(id) => id,
            _ => unreachable!("expected @objectId column"),
        })
        .collect();
    assert_eq!(ids, vec![0x2000, 0x2001, 0x2005, 0x2006]);
    assert_eq!(result.total_matched, 4);
}

#[test]
fn union_of_overlapping_queries_dedupes_by_object_id() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    let statement = parse_query_statement(
        r#"SELECT @objectId FROM "com.example.User" WHERE kind <= 3 UNION SELECT @objectId FROM "com.example.User" WHERE kind >= 2 AND kind <= 4"#,
    )
    .expect("union query should parse");

    let result = execute_query_statement(&statement, &graph, Some(&dominator))
        .expect("union query should execute");

    let ids: Vec<u64> = result
        .rows
        .iter()
        .map(|row| match row[0] {
            CellValue::Id(id) => id,
            _ => unreachable!("expected @objectId column"),
        })
        .collect();

    // Left alone: {0x2000, 0x2001, 0x2002}. Right alone: {0x2001, 0x2002,
    // 0x2003}. 0x2001/0x2002 overlap and must not be double-counted.
    assert_eq!(ids, vec![0x2000, 0x2001, 0x2002, 0x2003]);
    assert_eq!(result.total_matched, 4);
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(
        deduped.len(),
        ids.len(),
        "object id appeared more than once"
    );
}

#[test]
fn union_right_side_limit_bounds_its_own_contribution_before_merge() {
    let graph = build_string_query_graph();
    let dominator = build_dominator_tree(&graph);

    let statement = parse_query_statement(
        r#"SELECT @objectId FROM "com.example.User" WHERE kind = 1 UNION SELECT @objectId FROM "com.example.User" WHERE kind > 0 LIMIT 3"#,
    )
    .expect("union query should parse");

    let result = execute_query_statement(&statement, &graph, Some(&dominator))
        .expect("union query should execute");

    let ids: Vec<u64> = result
        .rows
        .iter()
        .map(|row| match row[0] {
            CellValue::Id(id) => id,
            _ => unreachable!("expected @objectId column"),
        })
        .collect();

    // Left matches only 0x2000 (kind = 1). Right, unbounded, would match
    // all 7 users (every `kind` is > 0); its own `LIMIT 3` bounds its
    // contribution to the first 3 by ascending object id (0x2000, 0x2001,
    // 0x2002) *before* the union/dedup step -- each side of `UNION` is
    // evaluated as an independent `Query`, own `LIMIT` included, exactly as
    // if it had been run standalone. If `LIMIT` were silently dropped here,
    // this would instead return all 7 users.
    assert_eq!(ids, vec![0x2000, 0x2001, 0x2002]);
}
