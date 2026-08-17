//! Integration tests for M8 Slice 8.C: the object inspector
//! (`core::analysis::inspector::{inspect_object, ObjectInspection}`).
//!
//! Complements the module's own unit tests (`core/src/analysis/inspector.rs`)
//! with parse-then-inspect coverage against real HPROF binaries, mirroring
//! the fixture-construction pattern used by `core/tests/gc_path_all_paths.rs`
//! / `core/tests/analyze_by_referrer.rs`.

use mnemosyne_core::{
    analysis::{inspect_object, ObjectRef},
    build_dominator_tree,
    hprof::test_fixtures::build_graph_fixture,
    parse_hprof_file_with_options,
    report::inspect::{render, Format},
    ParseOptions,
};

fn obj_ref(object_id: &str, class_name: &str) -> ObjectRef {
    ObjectRef {
        object_id: object_id.into(),
        class_name: class_name.into(),
    }
}

/// `build_graph_fixture()` is: GC root 0x1000 (`com.example.BigCache`, one
/// field `entries` -> 0x2000) -> 0x2000 (`java.lang.Object`, no fields).
const ROOT_OBJECT_ID: u64 = 0x1000;
const CHILD_OBJECT_ID: u64 = 0x2000;

#[test]
fn inspects_known_object_with_correct_size_refs_and_dominator_context() {
    let bytes = build_graph_fixture();
    let graph =
        parse_hprof_file_with_options_from_bytes(&bytes, false).expect("fixture must parse");
    let dominator = build_dominator_tree(&graph);

    let inspection = inspect_object(&graph, Some(&dominator), ROOT_OBJECT_ID, false)
        .expect("root object must be found");

    assert_eq!(inspection.object_id, "0x00001000");
    assert_eq!(inspection.class_name, "com.example.BigCache");
    assert_eq!(
        inspection.references_out,
        vec![obj_ref("0x00002000", "java.lang.Object")]
    );
    assert_eq!(inspection.referrers_in, Vec::<ObjectRef>::new());
    // The GC root's immediate dominator is the virtual super-root, which is
    // not a real object.
    assert_eq!(inspection.dominator_parent, None);
    assert_eq!(
        inspection.dominator_children,
        vec![obj_ref("0x00002000", "java.lang.Object")]
    );
    assert!(inspection.retained_size.unwrap() > 0);
}

#[test]
fn child_object_reports_parent_as_dominator_and_root_as_referrer() {
    let bytes = build_graph_fixture();
    let graph =
        parse_hprof_file_with_options_from_bytes(&bytes, false).expect("fixture must parse");
    let dominator = build_dominator_tree(&graph);

    let inspection = inspect_object(&graph, Some(&dominator), CHILD_OBJECT_ID, false)
        .expect("child object must be found");

    assert_eq!(inspection.class_name, "java.lang.Object");
    assert_eq!(
        inspection.referrers_in,
        vec![obj_ref("0x00001000", "com.example.BigCache")]
    );
    assert_eq!(inspection.references_out, Vec::<ObjectRef>::new());
    assert_eq!(
        inspection.dominator_parent,
        Some(obj_ref("0x00001000", "com.example.BigCache"))
    );
    assert!(inspection.dominator_children.is_empty());
}

#[test]
fn retain_field_data_flag_gates_the_fields_section() {
    let bytes = build_graph_fixture();

    // Parsed WITHOUT retain_field_data: fields must stay None even when
    // the caller passes retain_field_data=true to inspect_object, because
    // the graph itself never retained field bytes.
    let graph_without = parse_hprof_file_with_options_from_bytes(&bytes, false).unwrap();
    let inspection_without = inspect_object(&graph_without, None, ROOT_OBJECT_ID, true).unwrap();
    assert_eq!(inspection_without.fields, None);

    // Parsed WITH retain_field_data, but the caller does not request
    // fields: still None.
    let graph_with = parse_hprof_file_with_options_from_bytes(&bytes, true).unwrap();
    let inspection_flag_off = inspect_object(&graph_with, None, ROOT_OBJECT_ID, false).unwrap();
    assert_eq!(inspection_flag_off.fields, None);

    // Parsed WITH retain_field_data AND the caller requests fields: the
    // BigCache root has one declared field ("entries" -> 0x2000).
    let inspection_flag_on = inspect_object(&graph_with, None, ROOT_OBJECT_ID, true).unwrap();
    let fields = inspection_flag_on.fields.expect("fields must be Some");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "entries");
    assert_eq!(fields[0].type_name, "java.lang.Object");
    assert_eq!(fields[0].value, "0x00002000");
}

#[test]
fn unknown_object_id_returns_none() {
    let bytes = build_graph_fixture();
    let graph = parse_hprof_file_with_options_from_bytes(&bytes, false).unwrap();
    let dominator = build_dominator_tree(&graph);

    assert!(inspect_object(&graph, Some(&dominator), 0xDEAD_BEEF, false).is_none());
}

#[test]
fn text_renderer_shows_class_resolved_refs_and_dominator_context() {
    let bytes = build_graph_fixture();
    let graph = parse_hprof_file_with_options_from_bytes(&bytes, false).unwrap();
    let dominator = build_dominator_tree(&graph);
    let inspection = inspect_object(&graph, Some(&dominator), CHILD_OBJECT_ID, false).unwrap();

    let text = render(&inspection, Format::Text).unwrap();

    assert!(text.contains("Object 0x00002000  (java.lang.Object)"));
    assert!(text.contains("Dominator parent: 0x00001000 (com.example.BigCache)"));
    assert!(text.contains("Referrers in (1): 0x00001000 (com.example.BigCache)"));
    assert!(text.contains("References out (0): (none)"));
}

#[test]
fn json_renderer_round_trips_object_inspection() {
    let bytes = build_graph_fixture();
    let graph = parse_hprof_file_with_options_from_bytes(&bytes, true).unwrap();
    let dominator = build_dominator_tree(&graph);
    let inspection = inspect_object(&graph, Some(&dominator), ROOT_OBJECT_ID, true).unwrap();

    let rendered = render(&inspection, Format::Json).unwrap();
    let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(value["object_id"], "0x00001000");
    assert!(value["fields"].as_array().unwrap().len() == 1);
}

fn parse_hprof_file_with_options_from_bytes(
    bytes: &[u8],
    retain_field_data: bool,
) -> mnemosyne_core::CoreResult<mnemosyne_core::hprof::ObjectGraph> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    parse_hprof_file_with_options(
        file.path().to_string_lossy().as_ref(),
        ParseOptions { retain_field_data },
    )
}
