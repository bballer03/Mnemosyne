//! Object inspector (M8 Slice 8.C).
//!
//! Answers "give me one focused view of a single object: fields, refs
//! in/out, dominator context" — the CLI/MCP equivalent of the UI's Object
//! Inspector pane. Thin composition over existing `ObjectGraph` /
//! `DominatorTree` accessors (`get_object` / `get_references` /
//! `get_referrers` / `immediate_dominator` / `dominated_by` /
//! `retained_size`) plus the existing typed field reader
//! (`read_all_fields`), gated by `retain_field_data` — the same opt-in
//! discipline already used by the string/collection analyzers. No new
//! graph-walking algorithms are introduced here. See
//! `docs/design/milestone-8-reachability-references.md` §6/§10 (Slice 8.C).

use crate::analysis::string_analysis::extract_string_value;
use crate::graph::{DominatorTree, VIRTUAL_ROOT_ID};
use crate::hprof::{read_all_fields, FieldValue, ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};

/// A single typed instance field, rendered to a display string.
///
/// `value` reuses the same rendering path the query engine's `@toString`
/// support already relies on: primitives via their `to_string()`, and
/// object references either resolved to their (quoted) `java.lang.String`
/// content via [`extract_string_value`] or, failing that, formatted as a
/// hex object id — no new field-decoding or formatting logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldValueEntry {
    pub name: String,
    pub type_name: String,
    pub value: String,
}

/// A class-name-resolved object reference. Kept structured (rather than a
/// baked `"{id} ({class})"` string) so JSON/TOON/MCP consumers — chiefly
/// AI agents chaining `inspect_object` calls — get a clean `object_id`
/// they can pass straight back into `inspect`/`gc-path`/`query`, matching
/// this crate's MCP-first structured-output commitment. The text renderer
/// is the only place that formats `"{object_id} ({class_name})"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRef {
    pub object_id: String,
    pub class_name: String,
}

/// A focused, single-object view: identity, size, dominator context, and
/// refs in/out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectInspection {
    pub object_id: String,
    pub class_name: String,
    pub shallow_size: u64,
    pub retained_size: Option<u64>,
    /// `Some(...)` only when `retain_field_data` was requested AND the
    /// graph actually retained field data for this object (i.e. the heap
    /// was parsed with `ParseOptions { retain_field_data: true }` and this
    /// object has non-empty `field_data`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<FieldValueEntry>>,
    pub references_out: Vec<ObjectRef>,
    pub referrers_in: Vec<ObjectRef>,
    pub dominator_parent: Option<ObjectRef>,
    pub dominator_children: Vec<ObjectRef>,
}

/// Inspect a single object: identity/size, refs in/out, dominator context,
/// and (opt-in) typed field values.
///
/// Returns `None` when `object_id` does not exist in `graph` — callers map
/// that to the "object id not found" error, reusing the same exit
/// code/semantic `gc-path --object-id` already established (CLI exit code
/// 8 / MCP `object_id_not_found`).
///
/// `dominator` is optional: when `None`, `retained_size` is `None` and
/// `dominator_parent` / `dominator_children` are empty/`None`, matching the
/// no-dominator-tree convention already used by
/// `analysis::referrers::analyze_by_referrer`.
pub fn inspect_object(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
    retain_field_data: bool,
) -> Option<ObjectInspection> {
    let object = graph.get_object(object_id)?;
    let id_size = graph.identifier_size as usize;

    let class_name = graph
        .class_name(object.class_id)
        .map(prettify_class_name)
        .unwrap_or_else(|| "<unknown>".to_string());

    let mut references_out_ids = graph.get_references(object_id);
    references_out_ids.sort_unstable();
    let references_out = references_out_ids
        .into_iter()
        .map(|id| object_ref(graph, id_size, id))
        .collect();

    let mut referrers_in_ids = graph.get_referrers(object_id);
    referrers_in_ids.sort_unstable();
    let referrers_in = referrers_in_ids
        .into_iter()
        .map(|id| object_ref(graph, id_size, id))
        .collect();

    let (dominator_parent, dominator_children, retained_size) = match dominator {
        Some(dom) => {
            let parent = dom
                .immediate_dominator(object_id)
                .filter(|&parent_id| parent_id != VIRTUAL_ROOT_ID)
                .map(|parent_id| object_ref(graph, id_size, parent_id));

            let mut children_ids = dom.dominated_by(object_id).to_vec();
            children_ids.sort_unstable();
            let children = children_ids
                .into_iter()
                .map(|id| object_ref(graph, id_size, id))
                .collect();

            (parent, children, Some(dom.retained_size(object_id)))
        }
        None => (None, Vec::new(), None),
    };

    let fields = (retain_field_data && !object.field_data.is_empty()).then(|| {
        read_all_fields(object, &graph.classes, graph.identifier_size)
            .into_iter()
            .map(|(name, value)| {
                let (type_name, rendered_value) = render_field_value(graph, id_size, &value);
                FieldValueEntry {
                    name,
                    type_name,
                    value: rendered_value,
                }
            })
            .collect()
    });

    Some(ObjectInspection {
        object_id: format_object_id(object_id, id_size),
        class_name,
        shallow_size: u64::from(object.shallow_size),
        retained_size,
        fields,
        references_out,
        referrers_in,
        dominator_parent,
        dominator_children,
    })
}

/// Render a single typed field value to `(type_name, display_value)`.
///
/// Primitives use their existing `to_string()` `Display` path. Object
/// references reuse [`extract_string_value`] — the same helper the query
/// engine's `@toString` support (`query::synth::synth_to_string`) already
/// calls — to render `java.lang.String` contents as a quoted literal;
/// anything else renders as its hex object id, same convention as
/// [`format_object_id`].
fn render_field_value(graph: &ObjectGraph, id_size: usize, value: &FieldValue) -> (String, String) {
    match value {
        FieldValue::Boolean(v) => ("boolean".to_string(), v.to_string()),
        FieldValue::Byte(v) => ("byte".to_string(), v.to_string()),
        FieldValue::Char(v) => (
            "char".to_string(),
            char::from_u32(u32::from(*v))
                .map(|c| c.to_string())
                .unwrap_or_else(|| format!("\\u{v:04x}")),
        ),
        FieldValue::Short(v) => ("short".to_string(), v.to_string()),
        FieldValue::Int(v) => ("int".to_string(), v.to_string()),
        FieldValue::Long(v) => ("long".to_string(), v.to_string()),
        FieldValue::Float(v) => ("float".to_string(), v.to_string()),
        FieldValue::Double(v) => ("double".to_string(), v.to_string()),
        FieldValue::ObjectRef(Some(ref_id)) => {
            let type_name = graph
                .get_object(*ref_id)
                .and_then(|obj| graph.class_name(obj.class_id))
                .map(prettify_class_name)
                .unwrap_or_else(|| "<unknown>".to_string());
            let rendered = extract_string_value(graph, *ref_id)
                .map(|s| format!("\"{s}\""))
                .unwrap_or_else(|| format_object_id(*ref_id, id_size));
            (type_name, rendered)
        }
        FieldValue::ObjectRef(None) => ("<null>".to_string(), "null".to_string()),
    }
}

/// Build a structured [`ObjectRef`], resolving the class name by looking
/// the id up in `graph`.
fn object_ref(graph: &ObjectGraph, id_size: usize, id: ObjectId) -> ObjectRef {
    let class_name = graph
        .get_object(id)
        .and_then(|obj| graph.class_name(obj.class_id))
        .map(prettify_class_name)
        .unwrap_or_else(|| "<unknown>".to_string());
    ObjectRef {
        object_id: format_object_id(id, id_size),
        class_name,
    }
}

/// Render an object id as the same zero-padded hex string convention used
/// by `core::graph::gc_path` / `core::analysis::referrers`
/// (`0x` + `2 * identifier_size` hex digits).
fn format_object_id(object_id: ObjectId, id_size: usize) -> String {
    let width = id_size * 2;
    format!("0x{object_id:0width$X}")
}

/// Convert an HPROF-internal class name (`com/example/Foo`) to its
/// dotted display form (`com.example.Foo`), matching the convention
/// already used by `core::graph::gc_path::prettify_class_name`.
fn prettify_class_name(raw: &str) -> String {
    raw.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectKind,
    };

    fn add_class(graph: &mut ObjectGraph, class_id: u64, name: &str, fields: Vec<(&str, u8)>) {
        graph.classes.insert(
            class_id,
            ClassInfo {
                class_obj_id: class_id,
                super_class_id: 0,
                class_loader_id: 0,
                instance_size: 16,
                name: Some(name.into()),
                instance_fields: fields
                    .into_iter()
                    .map(|(name, field_type)| FieldDescriptor {
                        name: Some(name.into()),
                        field_type,
                    })
                    .collect(),
                static_references: Vec::new(),
            },
        );
    }

    fn add_object(
        graph: &mut ObjectGraph,
        object_id: u64,
        class_id: u64,
        shallow_size: u32,
        references: &[u64],
    ) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size,
                references: references.to_vec(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
    }

    fn add_object_with_fields(
        graph: &mut ObjectGraph,
        object_id: u64,
        class_id: u64,
        shallow_size: u32,
        references: &[u64],
        field_data: Vec<u8>,
    ) {
        graph.objects.insert(
            object_id,
            HeapObject {
                id: object_id,
                class_id,
                shallow_size,
                references: references.to_vec(),
                field_data,
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

    /// Root(1) -> Middle(2) -> Leaf(3); Middle is directly dominated by
    /// Root, Leaf is directly dominated by Middle. Used across several
    /// tests below.
    fn build_chain_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 100, "com.example.Root", Vec::new());
        add_class(&mut graph, 200, "com.example.Middle", Vec::new());
        add_class(&mut graph, 300, "com.example.Leaf", Vec::new());

        add_object(&mut graph, 1, 100, 8, &[2]);
        add_object(&mut graph, 2, 200, 24, &[3]);
        add_object(&mut graph, 3, 300, 40, &[]);
        add_root(&mut graph, 1);

        graph
    }

    #[test]
    fn known_object_reports_correct_shallow_and_retained_size() {
        let graph = build_chain_graph();
        let dominator = build_dominator_tree(&graph);

        let inspection =
            inspect_object(&graph, Some(&dominator), 2, false).expect("object 2 must be found");

        assert_eq!(inspection.object_id, "0x0000000000000002");
        assert_eq!(inspection.class_name, "com.example.Middle");
        assert_eq!(inspection.shallow_size, 24);
        // Middle(24) + Leaf(40) = 64
        assert_eq!(inspection.retained_size, Some(64));
    }

    #[test]
    fn known_object_reports_refs_out_and_referrers_in_with_class_names() {
        let graph = build_chain_graph();
        let dominator = build_dominator_tree(&graph);

        let inspection = inspect_object(&graph, Some(&dominator), 2, false).unwrap();

        assert_eq!(
            inspection.references_out,
            vec![ObjectRef {
                object_id: "0x0000000000000003".to_string(),
                class_name: "com.example.Leaf".to_string(),
            }]
        );
        assert_eq!(
            inspection.referrers_in,
            vec![ObjectRef {
                object_id: "0x0000000000000001".to_string(),
                class_name: "com.example.Root".to_string(),
            }]
        );
    }

    #[test]
    fn known_object_reports_dominator_parent_and_children() {
        let graph = build_chain_graph();
        let dominator = build_dominator_tree(&graph);

        let inspection = inspect_object(&graph, Some(&dominator), 2, false).unwrap();

        assert_eq!(
            inspection.dominator_parent,
            Some(ObjectRef {
                object_id: "0x0000000000000001".to_string(),
                class_name: "com.example.Root".to_string(),
            })
        );
        assert_eq!(
            inspection.dominator_children,
            vec![ObjectRef {
                object_id: "0x0000000000000003".to_string(),
                class_name: "com.example.Leaf".to_string(),
            }]
        );
    }

    #[test]
    fn object_directly_under_gc_root_has_no_dominator_parent() {
        let graph = build_chain_graph();
        let dominator = build_dominator_tree(&graph);

        // Object 1 is itself a GC root: its immediate dominator is the
        // virtual super-root, which is not a real object.
        let inspection = inspect_object(&graph, Some(&dominator), 1, false).unwrap();

        assert_eq!(inspection.dominator_parent, None);
    }

    #[test]
    fn no_dominator_tree_yields_none_retained_size_and_empty_dominator_data() {
        let graph = build_chain_graph();

        let inspection = inspect_object(&graph, None, 2, false).unwrap();

        assert_eq!(inspection.retained_size, None);
        assert_eq!(inspection.dominator_parent, None);
        assert!(inspection.dominator_children.is_empty());
    }

    #[test]
    fn unknown_object_id_returns_none() {
        let graph = build_chain_graph();
        let dominator = build_dominator_tree(&graph);

        assert!(inspect_object(&graph, Some(&dominator), 0xDEAD_BEEF, false).is_none());
    }

    #[test]
    fn retain_field_data_false_leaves_fields_none_even_with_data_present() {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            100,
            "com.example.Holder",
            vec![("count", field_types::INT)],
        );
        let mut field_bytes = Vec::new();
        field_bytes.extend_from_slice(&42i32.to_be_bytes());
        add_object_with_fields(&mut graph, 1, 100, 16, &[], field_bytes);

        let inspection = inspect_object(&graph, None, 1, false).unwrap();

        assert_eq!(inspection.fields, None);
    }

    #[test]
    fn retain_field_data_true_but_no_data_retained_leaves_fields_none() {
        let graph = build_chain_graph();

        // Object 2's class has no declared fields and field_data is empty
        // (built via `add_object`, not `add_object_with_fields`) — this is
        // the "parsed without retain_field_data" case.
        let inspection = inspect_object(&graph, None, 2, true).unwrap();

        assert_eq!(inspection.fields, None);
    }

    #[test]
    fn retain_field_data_true_populates_primitive_and_object_ref_fields() {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            100,
            "com.example.Key",
            vec![("count", field_types::INT), ("next", field_types::OBJECT)],
        );
        add_class(&mut graph, 200, "com.example.Leaf", Vec::new());
        add_object(&mut graph, 2, 200, 8, &[]);

        let mut field_bytes = Vec::new();
        field_bytes.extend_from_slice(&7i32.to_be_bytes());
        field_bytes.extend_from_slice(&2u64.to_be_bytes());
        add_object_with_fields(&mut graph, 1, 100, 24, &[2], field_bytes);

        let inspection = inspect_object(&graph, None, 1, true).unwrap();

        let fields = inspection.fields.expect("fields must be populated");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "count");
        assert_eq!(fields[0].type_name, "int");
        assert_eq!(fields[0].value, "7");
        assert_eq!(fields[1].name, "next");
        assert_eq!(fields[1].type_name, "com.example.Leaf");
        assert_eq!(fields[1].value, "0x0000000000000002");
    }

    #[test]
    fn retain_field_data_true_resolves_string_field_content() {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            100,
            "com.example.Session",
            vec![("label", field_types::OBJECT)],
        );
        add_class(
            &mut graph,
            200,
            "java.lang.String",
            vec![("value", field_types::OBJECT)],
        );
        add_class(&mut graph, 300, "[C", Vec::new());

        // String backing char array "hi".
        graph.objects.insert(
            3,
            HeapObject {
                id: 3,
                class_id: 300,
                shallow_size: 16,
                references: Vec::new(),
                field_data: vec![0x00, b'h', 0x00, b'i'],
                kind: ObjectKind::PrimitiveArray {
                    element_type: 5,
                    length: 2,
                },
            },
        );
        let mut string_field_bytes = Vec::new();
        string_field_bytes.extend_from_slice(&3u64.to_be_bytes());
        add_object_with_fields(&mut graph, 2, 200, 16, &[3], string_field_bytes);

        let mut label_field_bytes = Vec::new();
        label_field_bytes.extend_from_slice(&2u64.to_be_bytes());
        add_object_with_fields(&mut graph, 1, 100, 8, &[2], label_field_bytes);

        let inspection = inspect_object(&graph, None, 1, true).unwrap();

        let fields = inspection.fields.expect("fields must be populated");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "label");
        assert_eq!(fields[0].type_name, "java.lang.String");
        assert_eq!(fields[0].value, "\"hi\"");
    }
}
