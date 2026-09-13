//! Integration tests for M8 Slice 8.D: thread frame-local variable
//! resolution (`core::analysis::thread::{FrameLocal, FrameLocalRootKind,
//! inspect_threads}`).
//!
//! `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` GC roots are already parsed into
//! `ObjectGraph::gc_roots` as `GcRootType::JavaFrame { thread_serial, frame
//! }` / `GcRootType::JniLocal { thread_serial, frame }` by
//! `core::hprof::binary_parser` (confirmed during this slice — no parser
//! changes were required). This suite exercises `inspect_threads()`'s
//! correlation of those roots against a thread's already-parsed
//! `STACK_TRACE` / `STACK_FRAME` records by thread-serial + frame-number.

use mnemosyne_core::analysis::{inspect_threads, FrameLocalRootKind};
use mnemosyne_core::hprof::{
    ClassInfo, GcRoot, GcRootType, HeapObject, ObjectGraph, ObjectKind, StackFrame, StackTrace,
};

fn add_class(graph: &mut ObjectGraph, class_id: u64, super_class_id: u64, name: &str) {
    graph.classes.insert(
        class_id,
        ClassInfo {
            class_obj_id: class_id,
            super_class_id,
            class_loader_id: 0,
            instance_size: 16,
            name: Some(name.into()),
            instance_fields: Vec::new(),
            static_references: Vec::new(),
        },
    );
}

fn add_object(graph: &mut ObjectGraph, object_id: u64, class_id: u64) {
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
}

/// Builds a graph with a single thread (serial 7, object id 10, class
/// `com.example.Worker` extending `java.lang.Thread`) whose stack trace has
/// two frames:
///   - frame index 0: `com.example.Worker.run` (Worker.java:10)
///   - frame index 1: `com.example.Worker.helper` (no source info)
///
/// Local objects 30 and 31 (class `com.example.Local`) are inserted into
/// the graph but not yet attached to any GC root — callers push
/// `JavaFrame`/`JniLocal` roots as needed per test.
fn build_thread_graph() -> ObjectGraph {
    let mut graph = ObjectGraph::new(8);
    add_class(&mut graph, 1, 0, "java.lang.Object");
    add_class(&mut graph, 2, 1, "java.lang.Thread");
    add_class(&mut graph, 3, 2, "com.example.Worker");
    add_class(&mut graph, 4, 1, "com.example.Local");

    add_object(&mut graph, 10, 3);
    add_object(&mut graph, 30, 4);
    add_object(&mut graph, 31, 4);

    graph.gc_roots.push(GcRoot {
        object_id: 10,
        root_type: GcRootType::ThreadObject {
            thread_serial: 7,
            stack_trace_serial: 42,
        },
    });
    graph.stack_traces.insert(
        42,
        StackTrace {
            serial: 42,
            thread_serial: 7,
            frame_ids: vec![1000, 1001],
        },
    );
    graph.stack_frames.insert(
        1000,
        StackFrame {
            frame_id: 1000,
            method_name: "run".into(),
            class_name: "com.example.Worker".into(),
            source_file: Some("Worker.java".into()),
            line_number: 10,
        },
    );
    graph.stack_frames.insert(
        1001,
        StackFrame {
            frame_id: 1001,
            method_name: "helper".into(),
            class_name: "com.example.Worker".into(),
            source_file: None,
            line_number: -1,
        },
    );

    graph
}

#[test]
fn java_frame_root_resolves_to_correct_frame_local() {
    let mut graph = build_thread_graph();
    graph.gc_roots.push(GcRoot {
        object_id: 30,
        root_type: GcRootType::JavaFrame {
            thread_serial: 7,
            frame: 0,
        },
    });

    let report = inspect_threads(&graph, None, 5);
    assert_eq!(report.threads.len(), 1);
    let stack_trace = report.threads[0]
        .stack_trace
        .as_ref()
        .expect("stack trace must be present");

    let frame0 = &stack_trace[0].locals;
    assert_eq!(frame0.len(), 1);
    assert_eq!(frame0[0].variable_slot, 0);
    assert_eq!(frame0[0].object_id, "0x000000000000001E"); // 30 in hex
    assert_eq!(frame0[0].class_name, "com.example.Local");
    assert_eq!(frame0[0].root_kind, FrameLocalRootKind::JavaFrame);

    // Frame 1 has no attached root.
    assert!(stack_trace[1].locals.is_empty());
}

#[test]
fn jni_local_root_resolves_to_correct_frame_local() {
    let mut graph = build_thread_graph();
    graph.gc_roots.push(GcRoot {
        object_id: 31,
        root_type: GcRootType::JniLocal {
            thread_serial: 7,
            frame: 1,
        },
    });

    let report = inspect_threads(&graph, None, 5);
    let stack_trace = report.threads[0]
        .stack_trace
        .as_ref()
        .expect("stack trace must be present");

    assert!(stack_trace[0].locals.is_empty());

    let frame1 = &stack_trace[1].locals;
    assert_eq!(frame1.len(), 1);
    assert_eq!(frame1[0].variable_slot, 1);
    assert_eq!(frame1[0].object_id, "0x000000000000001F"); // 31 in hex
    assert_eq!(frame1[0].class_name, "com.example.Local");
    assert_eq!(frame1[0].root_kind, FrameLocalRootKind::JniLocal);
}

#[test]
fn multiple_locals_can_attach_to_the_same_frame() {
    let mut graph = build_thread_graph();
    graph.gc_roots.push(GcRoot {
        object_id: 30,
        root_type: GcRootType::JavaFrame {
            thread_serial: 7,
            frame: 0,
        },
    });
    graph.gc_roots.push(GcRoot {
        object_id: 31,
        root_type: GcRootType::JniLocal {
            thread_serial: 7,
            frame: 0,
        },
    });

    let report = inspect_threads(&graph, None, 5);
    let stack_trace = report.threads[0].stack_trace.as_ref().unwrap();

    let frame0 = &stack_trace[0].locals;
    assert_eq!(frame0.len(), 2);
    // Deterministic ordering: sorted by object_id.
    assert_eq!(frame0[0].object_id, "0x000000000000001E");
    assert_eq!(frame0[1].object_id, "0x000000000000001F");
    // Both share the same frame-number-derived "slot" honestly.
    assert_eq!(frame0[0].variable_slot, 0);
    assert_eq!(frame0[1].variable_slot, 0);
}

#[test]
fn thread_with_no_frame_local_roots_has_empty_locals_not_error() {
    let graph = build_thread_graph();

    let report = inspect_threads(&graph, None, 5);
    assert_eq!(report.threads.len(), 1);
    let stack_trace = report.threads[0]
        .stack_trace
        .as_ref()
        .expect("stack trace must still resolve");

    assert_eq!(stack_trace.len(), 2);
    for frame in stack_trace {
        assert!(frame.locals.is_empty());
    }
}

#[test]
fn frame_local_root_for_a_different_thread_serial_does_not_leak_in() {
    let mut graph = build_thread_graph();
    // Same frame index, but a thread_serial that does not belong to any
    // known thread in this graph -- must never attach anywhere.
    graph.gc_roots.push(GcRoot {
        object_id: 30,
        root_type: GcRootType::JavaFrame {
            thread_serial: 999,
            frame: 0,
        },
    });

    let report = inspect_threads(&graph, None, 5);
    let stack_trace = report.threads[0].stack_trace.as_ref().unwrap();

    for frame in stack_trace {
        assert!(frame.locals.is_empty());
    }
}

#[test]
fn frame_local_root_with_out_of_range_frame_number_is_ignored_not_panicking() {
    let mut graph = build_thread_graph();
    // Frame number 5 does not correspond to any parsed frame (stack has
    // only indices 0 and 1) -- and -1 (encoded as u32::MAX per the HPROF
    // "-1 for empty stack" convention) must not be treated as a valid
    // index either.
    graph.gc_roots.push(GcRoot {
        object_id: 30,
        root_type: GcRootType::JavaFrame {
            thread_serial: 7,
            frame: 5,
        },
    });
    graph.gc_roots.push(GcRoot {
        object_id: 31,
        root_type: GcRootType::JniLocal {
            thread_serial: 7,
            frame: u32::MAX,
        },
    });

    let report = inspect_threads(&graph, None, 5);
    let stack_trace = report.threads[0].stack_trace.as_ref().unwrap();

    for frame in stack_trace {
        assert!(frame.locals.is_empty());
    }
}
