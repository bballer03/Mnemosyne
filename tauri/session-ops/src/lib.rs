//! Session-scoped heap operations shared by Tauri commands.
//!
//! Kept free of `tauri` dependencies so unit tests can run on headless CI
//! hosts without WebKit/GTK installed.

use mnemosyne_core::{
    analysis::{inspect_object, ObjectInspection},
    build_dominator_tree, graph::find_all_gc_paths_in_graph, AllPathsRequest, GcPathResult,
};

pub fn inspect_object_for_session(
    graph: &mnemosyne_core::hprof::ObjectGraph,
    heap_path: &str,
    object_id: &str,
    retain_field_data: bool,
) -> Result<ObjectInspection, String> {
    let target_id = parse_object_id(object_id)?;
    if !graph.objects.contains_key(&target_id) {
        return Err(inspect_object_id_not_found(object_id, heap_path));
    }

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

pub fn parse_object_id(input: &str) -> Result<u64, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Object id must not be empty".to_string());
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid object id '{trimmed}'"));
    }

    if trimmed.chars().any(|character| matches!(character, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed.trim_start_matches("0x"), 16)
            .map_err(|_| format!("Invalid object id '{trimmed}'"));
    }

    trimmed
        .parse::<u64>()
        .map_err(|_| format!("Invalid object id '{trimmed}'"))
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
}
