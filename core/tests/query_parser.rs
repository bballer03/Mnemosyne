use mnemosyne_core::query::{
    parse_query, parse_query_statement, BuiltInField, ClassPattern, ComparisonOp, Condition,
    FieldRef, FromClause, Query, QueryStatement, SelectClause, Value, WhereClause,
    MAX_MULTI_CLASS_FROM_LIST_SIZE, MAX_OBJECTS_FIELD_HOPS,
};

#[test]
fn parse_query_supports_exact_class_match() {
    let query = parse_query(
        r#"SELECT @objectId, @retainedSize FROM "java.util.HashMap" WHERE @retainedSize > 1048576"#,
    )
    .expect("query should parse");

    assert_eq!(
        query,
        Query {
            select: SelectClause::Fields(vec![
                FieldRef::BuiltIn(BuiltInField::ObjectId),
                FieldRef::BuiltIn(BuiltInField::RetainedSize),
            ]),
            from: FromClause {
                class_pattern: ClassPattern::Exact("java.util.HashMap".into()),
                instanceof: false,
            },
            filter: Some(WhereClause {
                conditions: vec![Condition {
                    field: FieldRef::BuiltIn(BuiltInField::RetainedSize),
                    op: ComparisonOp::Gt,
                    value: Value::Int(1_048_576),
                }],
                operators: Vec::new(),
            }),
            limit: None,
        }
    );
}

#[test]
fn parse_query_supports_glob_class_match_and_limit() {
    let query = parse_query(
        r#"SELECT @objectId, @className FROM "com.example.*" WHERE @className LIKE "%Cache%" LIMIT 25"#,
    )
    .expect("query should parse");

    assert_eq!(
        query,
        Query {
            select: SelectClause::Fields(vec![
                FieldRef::BuiltIn(BuiltInField::ObjectId),
                FieldRef::BuiltIn(BuiltInField::ClassName),
            ]),
            from: FromClause {
                class_pattern: ClassPattern::Glob("com.example.*".into()),
                instanceof: false,
            },
            filter: Some(WhereClause {
                conditions: vec![Condition {
                    field: FieldRef::BuiltIn(BuiltInField::ClassName),
                    op: ComparisonOp::Like,
                    value: Value::Str("%Cache%".into()),
                }],
                operators: Vec::new(),
            }),
            limit: Some(25),
        }
    );
}

#[test]
fn parse_query_rejects_invalid_syntax() {
    let error = parse_query("SELECT FROM").expect_err("query should fail");
    assert!(
        error.to_string().contains("expected") || error.to_string().contains("invalid"),
        "unexpected parse error: {error}"
    );
}

#[test]
fn parse_query_supports_single_quoted_string_literals() {
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE @className LIKE 'com.example.%'"#,
    )
    .expect("query should parse");

    assert_eq!(
        query.filter,
        Some(WhereClause {
            conditions: vec![Condition {
                field: FieldRef::BuiltIn(BuiltInField::ClassName),
                op: ComparisonOp::Like,
                value: Value::Str("com.example.%".into()),
            }],
            operators: Vec::new(),
        })
    );
}

// M15 Slice 15.D: regex `=~` operator.

#[test]
fn parse_query_supports_regex_operator() {
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User" WHERE @className =~ "^com\.example\..*""#,
    )
    .expect("query should parse");

    assert_eq!(
        query.filter,
        Some(WhereClause {
            conditions: vec![Condition {
                field: FieldRef::BuiltIn(BuiltInField::ClassName),
                op: ComparisonOp::RegexMatch,
                value: Value::Str(r"^com\.example\..*".into()),
            }],
            operators: Vec::new(),
        })
    );
}

#[test]
fn parse_query_rejects_malformed_regex_pattern() {
    let error =
        parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE name =~ "(unclosed""#)
            .expect_err("malformed regex should fail to parse, not panic");

    assert!(
        error.to_string().to_lowercase().contains("regex"),
        "unexpected parse error: {error}"
    );
}

#[test]
fn parse_query_rejects_regex_operator_with_non_string_value() {
    let error = parse_query(r#"SELECT @objectId FROM "com.example.User" WHERE kind =~ 123"#)
        .expect_err("=~ with a non-string literal should fail to parse, not panic");

    assert!(
        error.to_string().to_lowercase().contains("regex"),
        "unexpected parse error: {error}"
    );
}

// M15 Slice 15.E: one-level subqueries (`FROM OBJECTS (<subquery>)`) and `UNION`.

#[test]
fn parser_constructs_subquery_from_clause() {
    let query =
        parse_query(r#"SELECT * FROM OBJECTS (SELECT * FROM "com.example.User" WHERE kind > 5)"#)
            .expect("query with a one-level subquery should parse");

    let ClassPattern::Subquery(inner) = &query.from.class_pattern else {
        panic!("expected ClassPattern::Subquery, got {:?}", query.from);
    };
    assert_eq!(
        inner.from.class_pattern,
        ClassPattern::Exact("com.example.User".into())
    );
    assert!(inner.filter.is_some());
}

#[test]
fn parser_supports_subquery_combined_with_outer_where_and_limit() {
    let query = parse_query(
        r#"SELECT @objectId FROM OBJECTS (SELECT * FROM outbounds(1)) WHERE @objectId > 0 LIMIT 5"#,
    )
    .expect("query should parse");

    assert!(matches!(
        query.from.class_pattern,
        ClassPattern::Subquery(_)
    ));
    assert!(query.filter.is_some());
    assert_eq!(query.limit, Some(5));
}

#[test]
fn parser_rejects_subquery_missing_open_paren() {
    let error = parse_query(r#"SELECT * FROM OBJECTS SELECT * FROM "com.example.User""#)
        .expect_err("should fail");
    assert!(error.to_string().contains("expected '('"));
}

#[test]
fn parser_rejects_subquery_missing_close_paren() {
    let error = parse_query(r#"SELECT * FROM OBJECTS (SELECT * FROM "com.example.User""#)
        .expect_err("should fail");
    assert!(error.to_string().contains("expected ')'"));
}

#[test]
fn parser_rejects_second_level_nested_subquery() {
    // Bounded to one level of nesting (M15 Slice 15.E §4.1 item 4): a
    // subquery reachable through `OBJECTS(...)` must not itself contain
    // another `OBJECTS(...)` subquery. This must be a hard, specific parse
    // error naming the bound -- not silent truncation and not an
    // unbounded/looping parse.
    let error = parse_query(
        r#"SELECT * FROM OBJECTS (SELECT * FROM OBJECTS (SELECT * FROM "com.example.User"))"#,
    )
    .expect_err("doubly nested subquery should be rejected at parse time");

    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("nesting depth exceeded"),
        "unexpected parse error: {error}"
    );
}

#[test]
fn parse_query_statement_returns_single_for_ordinary_query() {
    let statement = parse_query_statement(r#"SELECT * FROM "com.example.User""#)
        .expect("ordinary query should parse as a statement");

    assert!(matches!(statement, QueryStatement::Single(_)));
}

#[test]
fn parse_query_statement_recognizes_union_of_two_queries() {
    let statement = parse_query_statement(
        r#"SELECT * FROM "com.example.User" WHERE kind = 1 UNION SELECT * FROM "com.example.Admin""#,
    )
    .expect("UNION of two queries should parse");

    let QueryStatement::Union(left, right) = statement else {
        panic!("expected QueryStatement::Union");
    };
    assert_eq!(
        left.from.class_pattern,
        ClassPattern::Exact("com.example.User".into())
    );
    assert_eq!(
        right.from.class_pattern,
        ClassPattern::Exact("com.example.Admin".into())
    );
}

#[test]
fn parse_query_still_rejects_trailing_union_keyword() {
    // `parse_query` (the pre-existing, unchanged entry point) intentionally
    // does not understand `UNION` -- only `parse_query_statement` does.
    // Every existing caller of `parse_query` must keep seeing exactly the
    // same "trailing input" rejection it always has.
    let error =
        parse_query(r#"SELECT * FROM "com.example.User" UNION SELECT * FROM "com.example.Admin""#)
            .expect_err("parse_query should reject trailing UNION, not silently accept it");

    assert!(error.to_string().contains("expected end of query"));
}

// M22 Slice 22.B: bounded multi-class `FROM`.

#[test]
fn parse_query_supports_two_literal_class_patterns() {
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User", "com.example.Admin""#,
    )
    .expect("two-class FROM should parse");

    assert_eq!(
        query.from,
        FromClause {
            class_pattern: ClassPattern::Multi(vec![
                ClassPattern::Exact("com.example.User".into()),
                ClassPattern::Exact("com.example.Admin".into()),
            ]),
            instanceof: false,
        }
    );
}

#[test]
fn parse_query_supports_mixed_exact_and_glob_class_patterns() {
    let query = parse_query(
        r#"SELECT @objectId FROM "com.example.User", "com.example.*""#,
    )
    .expect("mixed-pattern FROM should parse");

    assert_eq!(
        query.from.class_pattern,
        ClassPattern::Multi(vec![
            ClassPattern::Exact("com.example.User".into()),
            ClassPattern::Glob("com.example.*".into()),
        ])
    );
}

#[test]
fn parse_query_keeps_single_class_pattern_as_exact_not_multi() {
    let query = parse_query(r#"SELECT @objectId FROM "com.example.User""#)
        .expect("single-class FROM should parse");

    assert_eq!(
        query.from.class_pattern,
        ClassPattern::Exact("com.example.User".into())
    );
}

#[test]
fn parse_query_rejects_trailing_comma_in_multi_class_from() {
    let error = parse_query(r#"SELECT @objectId FROM "com.example.User","#)
        .expect_err("trailing comma should fail");

    assert!(
        error.to_string().contains("quoted"),
        "unexpected parse error: {error}"
    );
}

#[test]
fn parse_query_rejects_empty_class_pattern_entry() {
    let error = parse_query(r#"SELECT @objectId FROM "", "com.example.User""#)
        .expect_err("empty class pattern should fail");

    assert!(
        error.to_string().contains("non-empty"),
        "unexpected parse error: {error}"
    );
}

// M22 Slice 22.C: bounded multi-hop `SELECT OBJECTS`.

#[test]
fn parse_query_supports_two_hop_objects_field_path() {
    let query = parse_query(r#"SELECT OBJECTS n.parent.link FROM "com.example.Node""#)
        .expect("two-hop OBJECTS should parse");

    assert_eq!(
        query.select,
        SelectClause::Objects(FieldRef::InstanceField("n.parent.link".into()))
    );
}

#[test]
fn parse_query_supports_three_hop_objects_field_path() {
    let query =
        parse_query(r#"SELECT OBJECTS n.parent.link.target FROM "com.example.Node""#)
            .expect("three-hop OBJECTS should parse");

    assert_eq!(
        query.select,
        SelectClause::Objects(FieldRef::InstanceField("n.parent.link.target".into()))
    );
}

#[test]
fn parse_query_rejects_four_hop_objects_field_path() {
    let hop_chain = (1..=MAX_OBJECTS_FIELD_HOPS + 1)
        .map(|idx| format!("hop{idx}"))
        .collect::<Vec<_>>()
        .join(".");
    let query_text = format!(r#"SELECT OBJECTS n.{hop_chain} FROM "com.example.Node""#);
    let error = parse_query(&query_text).expect_err("four-hop OBJECTS should fail at parse time");

    assert!(
        error
            .to_string()
            .contains("multi-hop OBJECTS exceeds limit"),
        "unexpected parse error: {error}"
    );
}

#[test]
fn parse_query_rejects_multi_class_from_list_over_limit() {
    let patterns = (0..=MAX_MULTI_CLASS_FROM_LIST_SIZE)
        .map(|idx| format!(r#""com.example.Class{idx}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let query_text = format!("SELECT @objectId FROM {patterns}");
    let error = parse_query(&query_text).expect_err("over-limit list should fail");

    assert!(
        error
            .to_string()
            .contains("multi-class FROM list exceeds limit"),
        "unexpected parse error: {error}"
    );
}
