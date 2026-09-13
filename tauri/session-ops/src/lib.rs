//! Session-scoped heap operations shared by Tauri commands.
//!
//! Kept free of `tauri` dependencies so unit tests can run on headless CI
//! hosts without WebKit/GTK installed.

use std::path::Path;

use mnemosyne_core::{
    analysis::{inspect_object, ObjectInspection},
    build_dominator_tree,
    diff::{
        object::types::{
            DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES, DEFAULT_OBJECT_DIFF_TOP_N,
            DEFAULT_RETAINED_CHANGE_THRESHOLD,
        },
        run_diff, DiffMode, DiffRequest, DiffResult, IdentityStrategy, ObjectDiffReport,
    },
    graph::find_all_gc_paths_in_graph,
    snapshot::SnapshotStore,
    AllPathsRequest, GcPathResult,
};

/// Input for the M17 comparison bridge's `diffObjects` host method.
#[derive(Debug, Clone)]
pub struct DiffObjectsSessionInput {
    pub before_key: String,
    pub after_key: String,
    pub strategy: Option<IdentityStrategy>,
    pub top_n: Option<usize>,
    /// Leak-progression cross-reference (M10-B): default `false` unless the
    /// UI explicitly opts in.
    pub cross_reference_leaks: Option<bool>,
}

pub fn graph_has_field_data(graph: &mnemosyne_core::hprof::ObjectGraph) -> bool {
    graph
        .objects
        .values()
        .any(|object| !object.field_data.is_empty())
}

pub fn inspect_object_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    heap_path: &str,
    object_id: &str,
    retain_field_data: bool,
) -> Result<ObjectInspection, String> {
    let target_id = parse_inspect_object_id(object_id)
        .filter(|id| graph.objects.contains_key(id))
        .ok_or_else(|| inspect_object_id_not_found(object_id, heap_path))?;

    let dominator = build_dominator_tree(graph);
    inspect_object(graph, Some(&dominator), target_id, retain_field_data)
        .ok_or_else(|| inspect_object_id_not_found(object_id, heap_path))
}

pub fn find_all_gc_paths_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    heap_path: &str,
    object_id: &str,
    max_paths: usize,
) -> Result<GcPathResult, String> {
    find_all_gc_paths_in_graph(
        graph,
        &AllPathsRequest {
            heap_path: heap_path.to_string(),
            object_id: Some(object_id.to_string()),
            by_class: None,
            max_paths,
            max_depth: None,
        },
    )
    .map_err(|error| error.to_string())
}

pub fn inspect_object_id_not_found(object_id: &str, heap_path: &str) -> String {
    format!(
        "inspect_object_id_not_found: object id '{object_id}' was not found in heap dump '{heap_path}'"
    )
}

/// Parse an inspect-supplied object id (`0x...` hex or bare decimal), mirroring
/// MCP/CLI semantics: malformed ids are treated as not-found by callers.
pub fn parse_inspect_object_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }

    if trimmed.chars().any(|character| matches!(character, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed, 16).ok();
    }

    trimmed.parse::<u64>().ok()
}

pub fn parse_object_id(input: &str) -> Result<u64, String> {
    parse_inspect_object_id(input).ok_or_else(|| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            "Object id must not be empty".to_string()
        } else {
            format!("Invalid object id '{trimmed}'")
        }
    })
}

/// Parse the UI's PascalCase identity strategy labels into core enums.
pub fn parse_identity_strategy(raw: &str) -> Result<IdentityStrategy, String> {
    match raw {
        "ClassRetained" => Ok(IdentityStrategy::ClassRetained),
        "ClassDominator" => Ok(IdentityStrategy::ClassDominator),
        "FullFingerprint" => Ok(IdentityStrategy::FullFingerprint),
        other => Err(format!("Invalid identity strategy '{other}'")),
    }
}

/// Resolve a snapshot key (SHA-256 hash) or direct heap file path to the
/// on-disk heap path `run_diff` expects.
pub fn resolve_heap_path_from_key(store: &SnapshotStore, key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("snapshot key must not be empty".to_string());
    }

    if let Ok(payload) = store.load(trimmed) {
        return Ok(payload.manifest.heap_path);
    }

    if Path::new(trimmed).is_file() {
        return Ok(trimmed.to_string());
    }

    Err(format!(
        "snapshot_not_found: no snapshot or heap file found for key '{trimmed}'"
    ))
}

/// Run an object-level heap diff between two snapshot keys, reusing the
/// existing `DiffMode::Object` path (same defaults as CLI/MCP/workflow).
pub async fn diff_objects_for_session(
    store: &SnapshotStore,
    input: DiffObjectsSessionInput,
) -> Result<ObjectDiffReport, String> {
    let before_path = resolve_heap_path_from_key(store, &input.before_key)?;
    let after_path = resolve_heap_path_from_key(store, &input.after_key)?;

    let identity_strategy = input.strategy.unwrap_or_default();
    let top_n = input.top_n.unwrap_or(DEFAULT_OBJECT_DIFF_TOP_N);
    let cross_reference_leaks = input.cross_reference_leaks.unwrap_or(false);
    let retain_field_data = identity_strategy == IdentityStrategy::FullFingerprint;

    let request = DiffRequest {
        before_path,
        after_path,
        mode: DiffMode::Object,
        identity_strategy,
        retained_bucket_bits: 10,
        min_retained_bytes: DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
        retained_change_threshold: DEFAULT_RETAINED_CHANGE_THRESHOLD,
        top_n,
        retain_field_data,
        cross_reference_leaks,
    };

    match run_diff(request).await {
        Ok(DiffResult::Object(diff)) => diff
            .object_diff
            .ok_or_else(|| "object diff produced no report".to_string()),
        Ok(DiffResult::Class(_)) => Err("internal error: object diff returned class result".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(all(test, feature = "test-fixtures"))]
mod tests {
    use super::*;
    use mnemosyne_core::{
        analysis::inspect_object,
        build_dominator_tree,
        hprof::{parse_hprof_file_with_options, test_fixtures::build_graph_fixture, ParseOptions},
    };

    fn graph_fixture() -> mnemosyne_core::hprof::ObjectGraph {
        let bytes = build_graph_fixture();
        parse_hprof_file_with_options_from_bytes(&bytes, false).expect("fixture must parse")
    }

    fn parse_hprof_file_with_options_from_bytes(
        bytes: &[u8],
        retain_field_data: bool,
    ) -> Result<mnemosyne_core::hprof::ObjectGraph, String> {
        let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, bytes).map_err(|error| error.to_string())?;
        parse_hprof_file_with_options(
            file.path().to_str().expect("temp path must be valid UTF-8"),
            ParseOptions {
                retain_field_data,
            },
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn inspect_object_for_session_returns_structured_inspection() {
        let graph = graph_fixture();
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", false)
            .expect("known object must inspect");

        assert_eq!(inspection.object_id, "0x00001000");
        assert_eq!(inspection.class_name, "com.example.BigCache");
        assert!(inspection.retained_size.unwrap_or(0) > 0);
    }

    #[test]
    fn inspect_object_for_session_unknown_id_matches_core_error() {
        let graph = graph_fixture();
        let error = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0xdeadbeef", false)
            .expect_err("unknown object must fail");

        assert_eq!(
            error,
            "inspect_object_id_not_found: object id '0xdeadbeef' was not found in heap dump '/tmp/heap.hprof'"
        );
    }

    #[test]
    fn find_all_gc_paths_for_session_returns_all_paths_and_truncated_flag() {
        let graph = graph_fixture();
        let result = find_all_gc_paths_for_session(&graph, "/tmp/heap.hprof", "0x2000", 5)
            .expect("known object must enumerate paths");

        assert_eq!(result.object_id, "0x00002000");
        assert_eq!(result.path_length, result.path.len());
        assert!(result.all_paths.as_ref().is_some_and(|paths| !paths.is_empty()));
        assert!(!result.truncated);
    }

    #[test]
    fn find_all_gc_paths_for_session_unknown_id_matches_core_error() {
        let graph = graph_fixture();
        let error = find_all_gc_paths_for_session(&graph, "/tmp/heap.hprof", "0xdeadbeef", 5)
            .expect_err("unknown object must fail");

        assert!(
            error.contains("gc_path_object_id_not_found:"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn inspect_object_for_session_honors_retain_field_data_without_fields_when_unretained() {
        let graph = graph_fixture();
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", true)
            .expect("known object must inspect");

        assert!(inspection.fields.is_none());
        let dominator = build_dominator_tree(&graph);
        assert!(inspect_object(&graph, Some(&dominator), 0x1000, true)
            .expect("core inspect must succeed")
            .fields
            .is_none());
    }

    #[test]
    fn inspect_object_for_session_retain_field_data_populates_fields_when_graph_retained() {
        let bytes = build_graph_fixture();
        let graph =
            parse_hprof_file_with_options_from_bytes(&bytes, true).expect("fixture must parse");
        let inspection = inspect_object_for_session(&graph, "/tmp/heap.hprof", "0x1000", true)
            .expect("known object must inspect");

        let fields = inspection.fields.expect("fields must be present");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "entries");
    }

    #[test]
    fn inspect_object_for_session_malformed_id_matches_core_not_found_error() {
        let graph = graph_fixture();
        let error = inspect_object_for_session(&graph, "/tmp/heap.hprof", "not-an-id", false)
            .expect_err("malformed object id must fail");

        assert_eq!(
            error,
            "inspect_object_id_not_found: object id 'not-an-id' was not found in heap dump '/tmp/heap.hprof'"
        );
    }

    mod diff_objects {
        use super::*;
        use mnemosyne_core::{
            diff::{run_diff, DiffMode, DiffRequest, DiffResult, ObjectDeltaKind},
            hprof::{
                parse_hprof_file_with_options,
                test_fixtures::{build_graph_fixture, build_tune_gc_fixture},
                ParseOptions,
            },
            snapshot::SnapshotStore,
        };
        use std::io::Write;

        fn write_fixture(bytes: &[u8]) -> Result<(tempfile::NamedTempFile, String), String> {
            let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
            file.write_all(bytes).map_err(|error| error.to_string())?;
            let path = file
                .path()
                .to_str()
                .expect("temp path must be valid UTF-8")
                .to_string();
            Ok((file, path))
        }

        fn save_fixture_snapshot(
            store: &SnapshotStore,
            bytes: &[u8],
        ) -> Result<(tempfile::NamedTempFile, String), String> {
            let (file, path) = write_fixture(bytes)?;
            let graph = parse_hprof_file_with_options(&path, ParseOptions::default())
                .map_err(|error| error.to_string())?;
            let dominator = build_dominator_tree(&graph);
            let manifest = store
                .save(&path, &graph, &dominator)
                .map_err(|error| error.to_string())?;
            Ok((file, manifest.heap_sha256))
        }

        fn diff_store() -> SnapshotStore {
            SnapshotStore::new(
                tempfile::tempdir()
                    .expect("temp dir must exist")
                    .keep(),
            )
        }

        #[tokio::test]
        async fn diff_objects_same_snapshot_produces_empty_deltas() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key: before_key.clone(),
                    after_key: before_key,
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("same snapshot diff must succeed");

            assert!(report.added.is_empty());
            assert!(report.removed.is_empty());
            assert!(report.retained_changed.is_empty());
            assert_eq!(report.strategy, IdentityStrategy::ClassDominator);
        }

        #[tokio::test]
        async fn diff_objects_resolves_snapshot_keys_and_direct_heap_paths() {
            let store = diff_store();
            let (_snap_file, snap_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("snapshot");
            let resolved_from_key =
                resolve_heap_path_from_key(&store, &snap_key).expect("snapshot key must resolve");
            assert!(Path::new(&resolved_from_key).is_file());

            let (_path_file, heap_path) = write_fixture(&build_tune_gc_fixture()).expect("heap");
            let resolved_from_path =
                resolve_heap_path_from_key(&store, &heap_path).expect("heap path must resolve");
            assert_eq!(resolved_from_path, heap_path);
        }

        #[tokio::test]
        async fn diff_objects_fixture_pair_surfaces_add_and_remove_deltas() {
            let (_before_file, before_path) =
                write_fixture(&build_graph_fixture()).expect("before heap");
            let (_after_file, after_path) =
                write_fixture(&build_tune_gc_fixture()).expect("after heap");

            // Synthetic fixtures are below the production min-retained threshold,
            // so use a zero floor here (same technique as core's diff_object_engine).
            let request = DiffRequest {
                before_path,
                after_path,
                mode: DiffMode::Object,
                identity_strategy: IdentityStrategy::ClassDominator,
                retained_bucket_bits: 10,
                min_retained_bytes: 0,
                retained_change_threshold: 1,
                top_n: 50,
                retain_field_data: false,
                cross_reference_leaks: false,
            };

            let report = match run_diff(request).await {
                Ok(DiffResult::Object(diff)) => diff
                    .object_diff
                    .expect("object diff must include a report"),
                Ok(DiffResult::Class(_)) => panic!("expected object diff result"),
                Err(error) => panic!("diff failed: {error}"),
            };

            assert!(
                report
                    .added
                    .iter()
                    .any(|delta| delta.kind == ObjectDeltaKind::Added),
                "expected at least one Added delta"
            );
            assert!(
                report
                    .removed
                    .iter()
                    .any(|delta| delta.kind == ObjectDeltaKind::Removed),
                "expected at least one Removed delta"
            );
        }

        #[tokio::test]
        async fn diff_objects_preserves_match_quality_and_strategy() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let (_after_file, after_key) =
                save_fixture_snapshot(&store, &build_tune_gc_fixture()).expect("after snapshot");

            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key,
                    after_key,
                    strategy: Some(IdentityStrategy::ClassRetained),
                    top_n: Some(10),
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("strategy diff must succeed");

            assert_eq!(report.strategy, IdentityStrategy::ClassRetained);
            assert_eq!(report.match_quality.strategy, IdentityStrategy::ClassRetained);
            assert!(report.match_quality.collision_rate >= 0.0);
            assert!(report.added.len() <= 10);
            assert!(report.removed.len() <= 10);
            assert!(report.retained_changed.len() <= 10);
        }

        #[tokio::test]
        async fn diff_objects_unknown_key_returns_structured_error() {
            let store = diff_store();
            let error = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key: "missing-key".to_string(),
                    after_key: "also-missing".to_string(),
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect_err("unknown keys must fail");

            assert!(error.contains("snapshot_not_found:"));
        }

        #[test]
        fn diff_objects_empty_key_rejected() {
            let store = diff_store();
            let error = resolve_heap_path_from_key(&store, "  ").expect_err("empty key must fail");
            assert!(error.contains("must not be empty"));
        }

        #[test]
        fn parse_identity_strategy_rejects_unknown_labels() {
            let error = parse_identity_strategy("NotARealStrategy").expect_err("invalid strategy");
            assert!(error.contains("Invalid identity strategy"));
        }

        #[tokio::test]
        async fn diff_objects_cross_reference_leaks_defaults_off() {
            let store = diff_store();
            let (_before_file, before_key) =
                save_fixture_snapshot(&store, &build_graph_fixture()).expect("before snapshot");
            let (_after_file, after_key) =
                save_fixture_snapshot(&store, &build_tune_gc_fixture()).expect("after snapshot");

            let report = diff_objects_for_session(
                &store,
                DiffObjectsSessionInput {
                    before_key,
                    after_key,
                    strategy: None,
                    top_n: None,
                    cross_reference_leaks: None,
                },
            )
            .await
            .expect("diff must succeed");

            for delta in report
                .added
                .iter()
                .chain(report.retained_changed.iter())
            {
                assert!(delta.leak_severity.is_none());
            }
        }
    }
}
