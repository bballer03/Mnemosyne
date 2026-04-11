use mnemosyne_core::{
    build_dominator_tree, parse_hprof,
    query::{execute_query, parse_query, CellValue},
    test_fixtures::build_graph_fixture,
};

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
