//! Integration tests for M8 Slice 8.B: group-by-referrer analyzer
//! (`core::analysis::referrers::{analyze_by_referrer, ReferrerReport,
//! ReferrerEntry}`) and its end-to-end wiring through
//! `AnalyzeRequest::enable_by_referrer` / `AnalyzeResponse::referrer_report`.
//!
//! Unit-level coverage of ranking/grouping correctness lives alongside the
//! implementation in `core/src/analysis/referrers.rs`; this file exercises
//! the analyzer via the public `analyze_heap` pipeline, plus the hard
//! regression gate: `analyze` with no `--by-referrer` (i.e.
//! `enable_by_referrer: false`) must leave `AnalyzeResponse` output
//! byte-identical to pre-M8-Slice-8.B — enforced here by asserting the
//! `referrer_report` key is entirely absent from the serialized JSON.

use mnemosyne_core::{
    analysis::{analyze_by_referrer, analyze_heap, AnalyzeRequest, LeakDetectionOptions},
    config::AppConfig,
    graph::build_dominator_tree,
    hprof::{ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind},
};
use std::io::Write;
use tempfile::NamedTempFile;

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

fn add_object(graph: &mut ObjectGraph, object_id: u64, class_id: u64, references: &[u64]) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 16,
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

/// One "hot" object with 10 referrers spread across 3 distinct classes
/// (4 + 3 + 3), reachable from a single GC root.
fn build_referrer_heavy_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 1, "com.example.Root");
    add_class(&mut graph, 2, "com.example.HotObject");
    add_class(&mut graph, 3, "com.example.HolderA");
    add_class(&mut graph, 4, "com.example.HolderB");
    add_class(&mut graph, 5, "com.example.HolderC");

    let holder_a: Vec<u64> = (200..204).collect(); // 4 referrers
    let holder_b: Vec<u64> = (300..303).collect(); // 3 referrers
    let holder_c: Vec<u64> = (400..403).collect(); // 3 referrers

    add_object(&mut graph, 100, 2, &[]); // the hot object
    for &id in &holder_a {
        add_object(&mut graph, id, 3, &[100]);
    }
    for &id in &holder_b {
        add_object(&mut graph, id, 4, &[100]);
    }
    for &id in &holder_c {
        add_object(&mut graph, id, 5, &[100]);
    }

    let mut root_refs = Vec::new();
    root_refs.extend(&holder_a);
    root_refs.extend(&holder_b);
    root_refs.extend(&holder_c);
    add_object(&mut graph, 1, 1, &root_refs);
    add_root(&mut graph, 1);

    graph
}

#[test]
fn hot_object_ranks_first_with_correct_referrer_class_grouping() {
    let graph = build_referrer_heavy_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_by_referrer(&graph, Some(&dominator), 10);

    assert_eq!(report.total_objects_considered, graph.objects.len());
    let hot = report
        .entries
        .first()
        .expect("report should have at least one entry");

    assert_eq!(hot.class_name, "com.example.HotObject");
    assert_eq!(hot.referrer_count, 10);
    assert_eq!(
        hot.top_referrer_classes,
        vec![
            ("com.example.HolderA".to_string(), 4),
            ("com.example.HolderB".to_string(), 3),
            ("com.example.HolderC".to_string(), 3),
        ],
        "top_referrer_classes must be sorted by count desc, alphabetical tiebreak"
    );
}

#[test]
fn top_n_zero_returns_empty_entries_but_reports_total_considered() {
    let graph = build_referrer_heavy_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_by_referrer(&graph, Some(&dominator), 0);

    assert!(report.entries.is_empty());
    assert_eq!(report.total_objects_considered, graph.objects.len());
}

/// Build a minimal but real HPROF-backed fixture via the shared
/// `test_fixtures` helper (same fixture `core::analysis::engine`'s own
/// byte-identical regression test uses), so we can exercise the full
/// `analyze_heap` pipeline rather than only the pure `analyze_by_referrer`
/// function.
fn write_graph_fixture_file() -> NamedTempFile {
    let fixture = mnemosyne_core::test_fixtures::build_graph_fixture();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&fixture).unwrap();
    file.flush().unwrap();
    file
}

#[tokio::test]
async fn analyze_heap_without_by_referrer_flag_omits_referrer_report_key() {
    let fixture = write_graph_fixture_file();
    let request = AnalyzeRequest {
        heap_path: fixture.path().to_string_lossy().into_owned(),
        config: AppConfig::default(),
        leak_options: LeakDetectionOptions::default(),
        ..AnalyzeRequest::default()
    };
    assert!(
        !request.enable_by_referrer,
        "default AnalyzeRequest must not enable by-referrer (byte-identical regression gate)"
    );

    let response = analyze_heap(request).await.unwrap();
    assert!(response.referrer_report.is_none());

    let json = serde_json::to_value(&response).unwrap();
    assert!(
        !json.as_object().unwrap().contains_key("referrer_report"),
        "referrer_report must be entirely absent from JSON when enable_by_referrer is false \
         (additive-field byte-identical regression gate, same discipline as Slice 8.A)"
    );
}

#[tokio::test]
async fn analyze_heap_with_by_referrer_flag_populates_referrer_report() {
    let fixture = write_graph_fixture_file();
    let response = analyze_heap(AnalyzeRequest {
        heap_path: fixture.path().to_string_lossy().into_owned(),
        config: AppConfig::default(),
        leak_options: LeakDetectionOptions::default(),
        enable_by_referrer: true,
        ..AnalyzeRequest::default()
    })
    .await
    .unwrap();

    let json = serde_json::to_value(&response).unwrap();
    assert!(json.as_object().unwrap().contains_key("referrer_report"));

    let report = response
        .referrer_report
        .expect("referrer_report must be populated when enable_by_referrer is true");
    assert!(report.total_objects_considered > 0);
}
