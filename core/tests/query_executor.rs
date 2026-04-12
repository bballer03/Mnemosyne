use byteorder::{BigEndian, WriteBytesExt};
use mnemosyne_core::{
    build_dominator_tree, parse_hprof,
    query::{execute_query, parse_query, CellValue},
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

#[test]
fn execute_query_returns_builtin_fields_for_matching_objects() {
    let graph = parse_hprof(&build_graph_fixture()).expect("fixture should parse");
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
    let graph = parse_hprof(&build_graph_fixture()).expect("fixture should parse");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(r#"SELECT @objectId FROM "java.lang.Object" LIMIT 1"#)
        .expect("query should parse");

    let result = execute_query(&query, &graph, Some(&dominator)).expect("query should execute");

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.rows.len(), 1);
}
