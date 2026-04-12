use mnemosyne_core::{
    analysis::{analyze_classloaders, analyze_heap, AnalyzeRequest, LeakDetectionOptions},
    config::AppConfig,
    graph::build_dominator_tree,
    hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectId, ObjectKind,
    },
};
use tempfile::NamedTempFile;

fn ref_bytes(id: u64) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

fn add_class(
    graph: &mut ObjectGraph,
    class_id: ObjectId,
    super_class_id: ObjectId,
    name: &str,
    class_loader_id: ObjectId,
    fields: Vec<FieldDescriptor>,
) {
    graph.classes.insert(
        class_id,
        ClassInfo {
            class_obj_id: class_id,
            super_class_id,
            class_loader_id,
            instance_size: 0,
            name: Some(name.into()),
            instance_fields: fields,
            static_references: Vec::new(),
        },
    );
}

fn build_classloader_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", 0, Vec::new());
    add_class(
        &mut graph,
        2,
        1,
        "java.lang.ClassLoader",
        0,
        vec![FieldDescriptor {
            name: Some("parent".into()),
            field_type: field_types::OBJECT,
        }],
    );
    add_class(
        &mut graph,
        3,
        2,
        "com.example.PluginClassLoader",
        0,
        Vec::new(),
    );
    add_class(&mut graph, 100, 1, "com.example.PluginA", 5000, Vec::new());
    add_class(&mut graph, 101, 1, "com.example.PluginB", 5000, Vec::new());

    graph.objects.insert(
        5000,
        HeapObject {
            id: 5000,
            class_id: 3,
            shallow_size: 64,
            references: vec![10, 11, 12],
            field_data: ref_bytes(0),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 5000,
        root_type: GcRootType::StickyClass,
    });

    graph.objects.insert(
        10,
        HeapObject {
            id: 10,
            class_id: 100,
            shallow_size: 128,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.objects.insert(
        11,
        HeapObject {
            id: 11,
            class_id: 100,
            shallow_size: 64,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.objects.insert(
        12,
        HeapObject {
            id: 12,
            class_id: 101,
            shallow_size: 256,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );

    graph
}

fn build_leaky_classloader_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);

    add_class(&mut graph, 1, 0, "java.lang.Object", 0, Vec::new());
    add_class(
        &mut graph,
        2,
        1,
        "java.lang.ClassLoader",
        0,
        vec![FieldDescriptor {
            name: Some("parent".into()),
            field_type: field_types::OBJECT,
        }],
    );
    add_class(
        &mut graph,
        3,
        2,
        "com.example.LeakyPluginClassLoader",
        0,
        Vec::new(),
    );
    add_class(
        &mut graph,
        200,
        1,
        "com.example.LeakyPlugin",
        7000,
        Vec::new(),
    );

    graph.objects.insert(
        7000,
        HeapObject {
            id: 7000,
            class_id: 3,
            shallow_size: 64,
            references: vec![20, 21],
            field_data: ref_bytes(0),
            kind: ObjectKind::Instance,
        },
    );
    graph.gc_roots.push(GcRoot {
        object_id: 7000,
        root_type: GcRootType::StickyClass,
    });

    graph.objects.insert(
        20,
        HeapObject {
            id: 20,
            class_id: 200,
            shallow_size: 8 * 1024 * 1024,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );
    graph.objects.insert(
        21,
        HeapObject {
            id: 21,
            class_id: 200,
            shallow_size: 2 * 1024 * 1024,
            references: Vec::new(),
            field_data: Vec::new(),
            kind: ObjectKind::Instance,
        },
    );

    graph
}

fn build_classloader_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    bytes.extend_from_slice(&4u32.to_be_bytes());
    bytes.extend_from_slice(&0u64.to_be_bytes());

    fn write_record(buf: &mut Vec<u8>, tag: u8, body: &[u8]) {
        buf.push(tag);
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
        buf.extend_from_slice(body);
    }

    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_be_bytes());
    }

    write_record(
        &mut bytes,
        0x01,
        &[
            0, 0, 0, 1, b'j', b'a', b'v', b'a', b'/', b'l', b'a', b'n', b'g', b'/', b'O', b'b',
            b'j', b'e', b'c', b't',
        ],
    );
    write_record(
        &mut bytes,
        0x01,
        &[
            0, 0, 0, 2, b'j', b'a', b'v', b'a', b'/', b'l', b'a', b'n', b'g', b'/', b'C', b'l',
            b'a', b's', b's', b'L', b'o', b'a', b'd', b'e', b'r',
        ],
    );
    write_record(
        &mut bytes,
        0x01,
        &[
            0, 0, 0, 3, b'c', b'o', b'm', b'/', b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'/',
            b'P', b'l', b'u', b'g', b'i', b'n', b'C', b'l', b'a', b's', b's', b'L', b'o', b'a',
            b'd', b'e', b'r',
        ],
    );
    write_record(
        &mut bytes,
        0x01,
        &[
            0, 0, 0, 4, b'c', b'o', b'm', b'/', b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'/',
            b'P', b'l', b'u', b'g', b'i', b'n', b'A',
        ],
    );
    write_record(
        &mut bytes,
        0x01,
        &[
            0, 0, 0, 5, b'c', b'o', b'm', b'/', b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'/',
            b'P', b'l', b'u', b'g', b'i', b'n', b'B',
        ],
    );
    write_record(
        &mut bytes,
        0x01,
        &[0, 0, 0, 6, b'p', b'a', b'r', b'e', b'n', b't'],
    );

    let mut load_class = Vec::new();
    push_u32(&mut load_class, 1);
    push_u32(&mut load_class, 0x100);
    push_u32(&mut load_class, 0);
    push_u32(&mut load_class, 1);
    write_record(&mut bytes, 0x02, &load_class);

    let mut load_class = Vec::new();
    push_u32(&mut load_class, 2);
    push_u32(&mut load_class, 0x200);
    push_u32(&mut load_class, 0);
    push_u32(&mut load_class, 2);
    write_record(&mut bytes, 0x02, &load_class);

    let mut load_class = Vec::new();
    push_u32(&mut load_class, 3);
    push_u32(&mut load_class, 0x300);
    push_u32(&mut load_class, 0);
    push_u32(&mut load_class, 3);
    write_record(&mut bytes, 0x02, &load_class);

    let mut load_class = Vec::new();
    push_u32(&mut load_class, 4);
    push_u32(&mut load_class, 0x400);
    push_u32(&mut load_class, 0);
    push_u32(&mut load_class, 4);
    write_record(&mut bytes, 0x02, &load_class);

    let mut load_class = Vec::new();
    push_u32(&mut load_class, 5);
    push_u32(&mut load_class, 0x500);
    push_u32(&mut load_class, 0);
    push_u32(&mut load_class, 5);
    write_record(&mut bytes, 0x02, &load_class);

    let mut heap = Vec::new();
    heap.push(0x08);
    push_u32(&mut heap, 5000);
    push_u32(&mut heap, 1);
    push_u32(&mut heap, 0);

    heap.push(0x20);
    push_u32(&mut heap, 0x100);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0);
    for _ in 0..5 {
        push_u32(&mut heap, 0);
    }
    push_u32(&mut heap, 0);
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());

    heap.push(0x20);
    push_u32(&mut heap, 0x200);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x100);
    push_u32(&mut heap, 0);
    for _ in 0..4 {
        push_u32(&mut heap, 0);
    }
    push_u32(&mut heap, 4);
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&1u16.to_be_bytes());
    push_u32(&mut heap, 6);
    heap.push(2);

    heap.push(0x20);
    push_u32(&mut heap, 0x300);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x200);
    push_u32(&mut heap, 0);
    for _ in 0..4 {
        push_u32(&mut heap, 0);
    }
    push_u32(&mut heap, 4);
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());

    heap.push(0x20);
    push_u32(&mut heap, 0x400);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x100);
    push_u32(&mut heap, 5000);
    for _ in 0..4 {
        push_u32(&mut heap, 0);
    }
    push_u32(&mut heap, 32);
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());

    heap.push(0x20);
    push_u32(&mut heap, 0x500);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x100);
    push_u32(&mut heap, 5000);
    for _ in 0..4 {
        push_u32(&mut heap, 0);
    }
    push_u32(&mut heap, 48);
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());
    heap.extend_from_slice(&0u16.to_be_bytes());

    heap.push(0x21);
    push_u32(&mut heap, 5000);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x300);
    push_u32(&mut heap, 4);
    push_u32(&mut heap, 0);

    heap.push(0x21);
    push_u32(&mut heap, 6000);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x400);
    push_u32(&mut heap, 0);

    heap.push(0x21);
    push_u32(&mut heap, 7000);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x400);
    push_u32(&mut heap, 0);

    heap.push(0x21);
    push_u32(&mut heap, 8000);
    push_u32(&mut heap, 0);
    push_u32(&mut heap, 0x500);
    push_u32(&mut heap, 0);

    write_record(&mut bytes, 0x0C, &heap);
    bytes
}

#[test]
fn analyze_classloaders_reports_non_bootstrap_loaders() {
    let graph = build_classloader_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_classloaders(&graph, Some(&dominator));

    assert_eq!(report.loaders.len(), 1);
    assert_eq!(report.loaders[0].object_id, 5000);
    assert_eq!(
        report.loaders[0].class_name,
        "com.example.PluginClassLoader"
    );
    assert_eq!(report.loaders[0].loaded_class_count, 2);
    assert_eq!(report.loaders[0].instance_count, 3);
    assert_eq!(report.loaders[0].total_shallow_bytes, 448);
    assert_eq!(report.loaders[0].retained_bytes, Some(512));
    assert!(report.potential_leaks.is_empty());
}

#[test]
fn analyze_classloaders_flags_loader_with_large_retained_graph_and_few_classes() {
    let graph = build_leaky_classloader_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_classloaders(&graph, Some(&dominator));

    assert_eq!(report.loaders.len(), 1);
    assert_eq!(report.potential_leaks.len(), 1);
    assert_eq!(report.potential_leaks[0].object_id, 7000);
    assert!(report.potential_leaks[0]
        .reason
        .contains("loads only 1 classes"));
}

#[tokio::test]
async fn analyze_heap_emits_classloader_report_when_enabled() {
    let fixture = build_classloader_fixture();
    let mut file = NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut file, &fixture).unwrap();

    let response = analyze_heap(AnalyzeRequest {
        heap_path: file.path().to_string_lossy().into_owned(),
        config: AppConfig::default(),
        leak_options: LeakDetectionOptions::default(),
        enable_classloaders: true,
        ..AnalyzeRequest::default()
    })
    .await
    .unwrap();

    let report = response.classloader_report.expect("classloader report");
    assert_eq!(report.loaders.len(), 1);
    assert_eq!(report.loaders[0].loaded_class_count, 2);
    assert_eq!(report.loaders[0].instance_count, 3);
}
