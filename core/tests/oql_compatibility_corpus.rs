//! M22 Slice 22.A — MAT compatibility corpus runner (thin).
//!
//! Loads `fixtures/oql/mat-compatibility.json` and checks parse/execute
//! outcomes for the recorded synthetic cases. Prefer behavioral depth in
//! `query_parser.rs` / `query_executor.rs`; this file only freezes the
//! equivalency catalog and the honesty labels.
//!
//! Honesty: only `equivalency == "mat-referenced"` cases may close a MAT
//! compatibility-matrix row. `non-equivalency` cases are Mnemosyne bounds
//! or syntax deltas and must not support equivalency claims.

use mnemosyne_core::{
    build_dominator_tree,
    hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectId, ObjectKind,
    },
    query::{execute_query, parse_query},
};
use serde::Deserialize;
use std::collections::HashSet;

const CORPUS_JSON: &str = include_str!("fixtures/oql/mat-compatibility.json");

#[derive(Debug, Deserialize)]
struct CorpusFile {
    version: u32,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    id: String,
    category: String,
    status: String,
    equivalency: String,
    #[serde(default)]
    non_equivalency_reason: Option<String>,
    #[serde(default)]
    mat_refs: Vec<MatRef>,
    query: String,
    expect: Expect,
}

#[derive(Debug, Deserialize)]
struct MatRef {
    section: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Expect {
    parse: String,
    #[serde(default)]
    execute: Option<String>,
    #[serde(default)]
    fixture: Option<String>,
    #[serde(default)]
    min_matched: Option<usize>,
    #[serde(default)]
    exact_matched: Option<usize>,
    #[serde(default)]
    truncated: Option<bool>,
    #[serde(default)]
    error_contains: Option<String>,
    #[serde(default)]
    result_budget: Option<String>,
}

fn load_corpus() -> CorpusFile {
    serde_json::from_str(CORPUS_JSON).expect("mat-compatibility.json should deserialize")
}

fn object_ref_bytes(id: u64) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

fn int_field_bytes(value: i32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

fn add_class(
    graph: &mut ObjectGraph,
    class_id: ObjectId,
    super_class_id: ObjectId,
    name: &str,
    fields: Vec<FieldDescriptor>,
) {
    graph.classes.insert(
        class_id,
        ClassInfo {
            class_obj_id: class_id,
            super_class_id,
            class_loader_id: 0,
            instance_size: 0,
            name: Some(name.into()),
            instance_fields: fields,
            static_references: Vec::new(),
        },
    );
}

fn add_user_object(graph: &mut ObjectGraph, user_id: ObjectId, name_id: ObjectId, kind: i32) {
    let mut field_data = object_ref_bytes(name_id);
    field_data.extend_from_slice(&int_field_bytes(kind));
    let references = if name_id == 0 {
        Vec::new()
    } else {
        vec![name_id]
    };

    graph.objects.insert(
        user_id,
        HeapObject {
            id: user_id,
            class_id: 3,
            shallow_size: 32,
            references,
            field_data,
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: user_id,
        root_type: GcRootType::StickyClass,
    });
}

fn node_projection_field_bytes(
    parent_id: ObjectId,
    depth: i32,
    count: i32,
    payload_id: ObjectId,
) -> Vec<u8> {
    let mut field_data = object_ref_bytes(parent_id);
    field_data.extend_from_slice(&int_field_bytes(depth));
    field_data.extend_from_slice(&int_field_bytes(count));
    field_data.extend_from_slice(&object_ref_bytes(payload_id));
    field_data
}

fn add_projection_parent(graph: &mut ObjectGraph, object_id: ObjectId, class_id: ObjectId) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 16,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id,
        root_type: GcRootType::StickyClass,
    });
}

fn add_projection_payload(graph: &mut ObjectGraph, object_id: ObjectId, shallow_size: u32) {
    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id: 6,
            shallow_size,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
}

fn add_projection_node(
    graph: &mut ObjectGraph,
    object_id: ObjectId,
    class_id: ObjectId,
    parent_id: ObjectId,
    depth: i32,
    count: i32,
    payload_id: ObjectId,
) {
    let mut references = Vec::new();
    if parent_id != 0 {
        references.push(parent_id);
    }
    if payload_id != 0 {
        references.push(payload_id);
    }

    graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size: 32,
            references,
            field_data: node_projection_field_bytes(parent_id, depth, count, payload_id),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id,
        root_type: GcRootType::StickyClass,
    });
}

fn build_multi_class_from_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(
        &mut graph,
        3,
        1,
        "com.example.User",
        vec![
            FieldDescriptor {
                name: Some("name".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("kind".into()),
                field_type: field_types::INT,
            },
        ],
    );
    add_class(
        &mut graph,
        4,
        1,
        "com.example.Admin",
        vec![FieldDescriptor {
            name: Some("level".into()),
            field_type: field_types::INT,
        }],
    );
    add_class(&mut graph, 5, 1, "com.example.Other", Vec::new());

    add_user_object(&mut graph, 0x3000, 0, 1);
    add_user_object(&mut graph, 0x3001, 0, 2);

    graph.objects.insert(
        0x3100,
        HeapObject {
            id: 0x3100,
            class_id: 4,
            shallow_size: 24,
            references: Vec::new(),
            field_data: int_field_bytes(9),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 0x3100,
        root_type: GcRootType::StickyClass,
    });

    graph.objects.insert(
        0x3200,
        HeapObject {
            id: 0x3200,
            class_id: 5,
            shallow_size: 16,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 0x3200,
        root_type: GcRootType::StickyClass,
    });

    graph
}

fn build_objects_projection_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(&mut graph, 2, 1, "com.example.ParentNode", Vec::new());
    add_class(&mut graph, 3, 1, "com.example.OtherParent", Vec::new());
    add_class(
        &mut graph,
        4,
        1,
        "com.example.Node",
        vec![
            FieldDescriptor {
                name: Some("parent".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("depth".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("count".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("payload".into()),
                field_type: field_types::OBJECT,
            },
        ],
    );
    add_class(&mut graph, 5, 4, "com.example.DeepNode", Vec::new());
    add_class(&mut graph, 6, 1, "com.example.Payload", Vec::new());

    add_projection_parent(&mut graph, 0x2100, 2);
    add_projection_parent(&mut graph, 0x2200, 2);
    add_projection_parent(&mut graph, 0x2300, 2);
    add_projection_parent(&mut graph, 0x2400, 3);

    add_projection_payload(&mut graph, 0x6100, 64);
    add_projection_payload(&mut graph, 0x6200, 8);
    add_projection_payload(&mut graph, 0x6400, 32);

    add_projection_node(&mut graph, 0x4100, 4, 0x2100, 4, 1, 0x6100);
    add_projection_node(&mut graph, 0x4200, 5, 0x2200, 9, 2, 0x6200);
    add_projection_node(&mut graph, 0x4300, 4, 0x2300, 8, 3, 0);
    add_projection_node(&mut graph, 0x4400, 4, 0x2300, 7, 4, 0x6400);
    add_projection_node(&mut graph, 0x4500, 4, 0, 10, 0, 0);
    add_projection_node(&mut graph, 0x4600, 4, 0x9999, 11, 0, 0);
    add_projection_node(&mut graph, 0x4700, 4, 0x2400, 6, 5, 0);

    graph
}

fn build_multi_hop_objects_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
    add_class(
        &mut graph,
        2,
        1,
        "com.example.ParentNode",
        vec![FieldDescriptor {
            name: Some("link".into()),
            field_type: field_types::OBJECT,
        }],
    );
    add_class(
        &mut graph,
        7,
        1,
        "com.example.Link",
        vec![FieldDescriptor {
            name: Some("target".into()),
            field_type: field_types::OBJECT,
        }],
    );
    add_class(
        &mut graph,
        4,
        1,
        "com.example.Node",
        vec![
            FieldDescriptor {
                name: Some("parent".into()),
                field_type: field_types::OBJECT,
            },
            FieldDescriptor {
                name: Some("depth".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("count".into()),
                field_type: field_types::INT,
            },
            FieldDescriptor {
                name: Some("payload".into()),
                field_type: field_types::OBJECT,
            },
        ],
    );
    add_class(&mut graph, 6, 1, "com.example.Payload", Vec::new());

    graph.objects.insert(
        0x2100,
        HeapObject {
            id: 0x2100,
            class_id: 2,
            shallow_size: 16,
            references: vec![0x7000],
            field_data: object_ref_bytes(0x7000),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 0x2100,
        root_type: GcRootType::StickyClass,
    });

    graph.objects.insert(
        0x2300,
        HeapObject {
            id: 0x2300,
            class_id: 2,
            shallow_size: 16,
            references: Vec::new(),
            field_data: object_ref_bytes(0),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 0x2300,
        root_type: GcRootType::StickyClass,
    });

    add_projection_payload(&mut graph, 0x6100, 64);

    graph.objects.insert(
        0x7000,
        HeapObject {
            id: 0x7000,
            class_id: 7,
            shallow_size: 16,
            references: vec![0x6100],
            field_data: object_ref_bytes(0x6100),
            kind: ObjectKind::Instance,
        },
    );

    add_projection_node(&mut graph, 0x4100, 4, 0x2100, 4, 1, 0x6100);
    add_projection_node(&mut graph, 0x4300, 4, 0x2300, 8, 3, 0);

    graph
}

fn build_multi_hop_cycle_graph() -> ObjectGraph {
    let mut graph = build_multi_hop_objects_graph();
    graph
        .objects
        .get_mut(&0x2100)
        .expect("parent fixture")
        .field_data = object_ref_bytes(0x2100);
    graph
        .objects
        .get_mut(&0x2100)
        .expect("parent fixture")
        .references = vec![0x2100];
    graph
}

fn fixture_graph(name: &str) -> ObjectGraph {
    match name {
        "multi_class" => build_multi_class_from_graph(),
        "objects_projection" => build_objects_projection_graph(),
        "multi_hop" => build_multi_hop_objects_graph(),
        "multi_hop_cycle" => build_multi_hop_cycle_graph(),
        other => panic!("unknown corpus fixture `{other}`"),
    }
}

#[test]
fn corpus_file_is_versioned_and_non_empty() {
    let corpus = load_corpus();
    assert_eq!(corpus.version, 1);
    assert!(!corpus.cases.is_empty());
}

#[test]
fn corpus_covers_required_m22a_categories() {
    let corpus = load_corpus();
    let categories: HashSet<_> = corpus.cases.iter().map(|c| c.category.as_str()).collect();
    for required in [
        "multi-class-from",
        "objects-hops",
        "objects-hop-limit",
        "duplicates",
        "null-missing",
        "cycles",
        "budget-exhaustion",
    ] {
        assert!(
            categories.contains(required),
            "corpus missing required category `{required}`"
        );
    }
}

#[test]
fn mat_referenced_cases_have_handbook_links() {
    let corpus = load_corpus();
    for case in &corpus.cases {
        match case.equivalency.as_str() {
            "mat-referenced" => {
                assert!(
                    !case.mat_refs.is_empty(),
                    "mat-referenced case `{}` needs MAT handbook refs",
                    case.id
                );
                for mat_ref in &case.mat_refs {
                    assert!(
                        mat_ref.url.contains("help.eclipse.org")
                            || mat_ref.url.contains("eclipse.org"),
                        "case `{}` mat_ref url looks non-MAT: {}",
                        case.id,
                        mat_ref.url
                    );
                    assert!(
                        !mat_ref.section.is_empty(),
                        "case `{}` mat_ref section must be non-empty",
                        case.id
                    );
                }
            }
            "non-equivalency" => {
                assert!(
                    case
                        .non_equivalency_reason
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()),
                    "non-equivalency case `{}` needs non_equivalency_reason",
                    case.id
                );
            }
            other => panic!("case `{}` has unknown equivalency `{other}`", case.id),
        }
        assert!(
            case.expect.result_budget.is_some(),
            "case `{}` must declare result_budget",
            case.id
        );
    }
}

#[test]
fn corpus_cases_match_parse_and_execute_expectations() {
    let corpus = load_corpus();

    for case in &corpus.cases {
        let parse_result = parse_query(&case.query);
        match case.expect.parse.as_str() {
            "ok" => {
                let query = parse_result.unwrap_or_else(|err| {
                    panic!("case `{}` should parse, got: {err}", case.id)
                });

                match case.expect.execute.as_deref() {
                    None => {}
                    Some("ok") => {
                        let fixture = case
                            .expect
                            .fixture
                            .as_deref()
                            .unwrap_or_else(|| panic!("case `{}` execute=ok needs fixture", case.id));
                        let graph = fixture_graph(fixture);
                        let dominator = build_dominator_tree(&graph);
                        let result = execute_query(&query, &graph, Some(&dominator))
                            .unwrap_or_else(|err| {
                                panic!("case `{}` should execute, got: {err}", case.id)
                            });

                        if let Some(min) = case.expect.min_matched {
                            assert!(
                                result.total_matched >= min,
                                "case `{}`: expected >= {min} matched, got {}",
                                case.id,
                                result.total_matched
                            );
                        }
                        if let Some(exact) = case.expect.exact_matched {
                            assert_eq!(
                                result.total_matched, exact,
                                "case `{}`: exact matched mismatch",
                                case.id
                            );
                        }
                        if let Some(truncated) = case.expect.truncated {
                            assert_eq!(
                                result.truncated, truncated,
                                "case `{}`: truncated flag mismatch",
                                case.id
                            );
                        }
                    }
                    Some("error") => {
                        let fixture = case
                            .expect
                            .fixture
                            .as_deref()
                            .unwrap_or_else(|| {
                                panic!("case `{}` execute=error needs fixture", case.id)
                            });
                        let graph = fixture_graph(fixture);
                        let dominator = build_dominator_tree(&graph);
                        let error = execute_query(&query, &graph, Some(&dominator))
                            .expect_err(&format!("case `{}` should fail at execute", case.id));
                        if let Some(needle) = &case.expect.error_contains {
                            assert!(
                                error.to_string().contains(needle),
                                "case `{}`: error `{error}` missing `{needle}`",
                                case.id
                            );
                        }
                    }
                    Some(other) => panic!("case `{}` unknown execute expect `{other}`", case.id),
                }
            }
            "error" => {
                let error = parse_result.expect_err(&format!(
                    "case `{}` should fail at parse",
                    case.id
                ));
                if let Some(needle) = &case.expect.error_contains {
                    assert!(
                        error.to_string().contains(needle),
                        "case `{}`: parse error `{error}` missing `{needle}`",
                        case.id
                    );
                }
            }
            other => panic!("case `{}` unknown parse expect `{other}`", case.id),
        }
    }
}

#[test]
fn only_mat_referenced_shipped_cases_are_equivalency_eligible() {
    // Meta-guard: keep the honesty rule mechanical so later slices cannot
    // accidentally mark a Mnemosyne-only bound as mat-referenced without refs.
    let corpus = load_corpus();
    let equivalency_eligible: Vec<_> = corpus
        .cases
        .iter()
        .filter(|c| c.status == "shipped" && c.equivalency == "mat-referenced")
        .map(|c| c.id.as_str())
        .collect();

    assert!(
        equivalency_eligible.contains(&"multi-class-from-two-literals"),
        "multi-class FROM should remain equivalency-eligible once shipped"
    );
    assert!(
        equivalency_eligible.contains(&"objects-one-hop"),
        "1-hop OBJECTS should remain equivalency-eligible once shipped"
    );
    assert!(
        !equivalency_eligible
            .iter()
            .any(|id| id.contains("four-hop") || id.contains("budget") || id.contains("cycle")),
        "Mnemosyne-bound cases must not appear in equivalency-eligible set: {equivalency_eligible:?}"
    );
}
