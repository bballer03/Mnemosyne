use mnemosyne_core::query::{
    parse_query, BuiltInField, ClassPattern, ComparisonOp, Condition, FieldRef, FromClause, Query,
    SelectClause, Value, WhereClause,
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
