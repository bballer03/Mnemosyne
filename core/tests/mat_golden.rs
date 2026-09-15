#![cfg(feature = "test-fixtures")]

use mnemosyne_core::{
    analysis::{analyze_heap, AnalyzeRequest},
    build_dominator_tree,
    config::AppConfig,
    parse_hprof,
    query::{execute_query, parse_query},
    test_fixtures::{build_graph_fixture, build_simple_fixture},
    LeakDetectionOptions,
};
use serde_json::{json, Value};
use std::{fs, io::Write, path::PathBuf};
use tempfile::NamedTempFile;

const GOLDEN_SCHEMA_VERSION: u32 = 1;
const UPDATE_ENV: &str = "UPDATE_MNEMOSYNE_GOLDENS";

fn expected_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("expected")
}

fn normalize_analysis(mut value: Value) -> Value {
    let object = value
        .as_object_mut()
        .expect("AnalyzeResponse should serialize as an object");
    object.remove("elapsed");
    let summary = object
        .get_mut("summary")
        .and_then(Value::as_object_mut)
        .expect("analysis summary should be an object");
    summary.insert(
        "heap_path".into(),
        Value::String("<synthetic-fixture>".into()),
    );
    summary.remove("generated_at");
    value
}

async fn analysis_case() -> Value {
    let mut fixture = NamedTempFile::new().expect("create fixture file");
    fixture
        .write_all(&build_graph_fixture())
        .expect("write graph fixture");
    let response = analyze_heap(AnalyzeRequest {
        heap_path: fixture.path().to_string_lossy().into_owned(),
        config: AppConfig::default(),
        leak_options: LeakDetectionOptions::default(),
        ..AnalyzeRequest::default()
    })
    .await
    .expect("analyze graph fixture");
    normalize_analysis(serde_json::to_value(response).expect("serialize analysis"))
}

fn oql_case(fixture_bytes: &[u8], query_source: &str) -> Value {
    let graph = parse_hprof(fixture_bytes).expect("parse synthetic fixture");
    let dominator = build_dominator_tree(&graph);
    let query = parse_query(query_source).expect("parse golden OQL");
    serde_json::to_value(
        execute_query(&query, &graph, Some(&dominator)).expect("execute golden OQL"),
    )
    .expect("serialize query result")
}

fn envelope(case_id: &str, fixture: &str, command: &str, output: Value) -> Value {
    json!({
        "schema_version": GOLDEN_SCHEMA_VERSION,
        "case_id": case_id,
        "equivalence": "mnemosyne-golden",
        "fixture": fixture,
        "command": command,
        "eclipse_mat_reference": null,
        "output": output,
    })
}

fn compare_or_update(case_id: &str, actual: &Value) {
    let path = expected_dir().join(format!("{case_id}.json"));
    if std::env::var(UPDATE_ENV).as_deref() == Ok("1") {
        fs::create_dir_all(expected_dir()).expect("create golden expected directory");
        let rendered = serde_json::to_string_pretty(actual).expect("render golden JSON");
        fs::write(&path, format!("{rendered}\n")).expect("write golden expected file");
        return;
    }

    let expected_text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing golden {} ({error}); review output then run {UPDATE_ENV}=1 \
             cargo test -p mnemosyne-core --features test-fixtures --test mat_golden",
            path.display()
        )
    });
    let expected: Value = serde_json::from_str(&expected_text)
        .unwrap_or_else(|error| panic!("invalid golden {}: {error}", path.display()));
    assert_eq!(
        actual, &expected,
        "golden mismatch for `{case_id}`; review the semantic change before updating"
    );
}

#[tokio::test]
async fn selected_analysis_and_oql_outputs_match_versioned_goldens() {
    let cases = [
        envelope(
            "analysis-graph-default",
            "build_graph_fixture",
            "analyze --mode deep --format json",
            analysis_case().await,
        ),
        envelope(
            "oql-node-instances",
            "build_simple_fixture",
            "query SELECT @objectId, @className, @shallowSize FROM \"com.example.Node\"",
            oql_case(
                &build_simple_fixture(),
                r#"SELECT @objectId, @className, @shallowSize FROM "com.example.Node""#,
            ),
        ),
        envelope(
            "oql-cache-retained",
            "build_graph_fixture",
            "query SELECT @objectId, @retainedSize FROM \"com.example.BigCache\" WHERE @retainedSize >= 4",
            oql_case(
                &build_graph_fixture(),
                r#"SELECT @objectId, @retainedSize FROM "com.example.BigCache" WHERE @retainedSize >= 4"#,
            ),
        ),
    ];

    for case in cases {
        assert_eq!(case["equivalence"], "mnemosyne-golden");
        assert!(
            case["eclipse_mat_reference"].is_null(),
            "mnemosyne-golden cases must not imply a MAT reference"
        );
        compare_or_update(case["case_id"].as_str().expect("case id"), &case);
    }
}
