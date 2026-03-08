use super::string_analysis::extract_string_value;
use crate::graph::DominatorTree;
use crate::hprof::{read_field, FieldValue, GcRootType, ObjectGraph, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub daemon: bool,
    pub stack_trace: Option<Vec<StackFrameInfo>>,
    pub retained_bytes: u64,
    pub thread_local_count: usize,
    pub thread_local_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackFrameInfo {
    pub method_name: String,
    pub class_name: String,
    pub source_file: Option<String>,
    pub line_number: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadReport {
    pub threads: Vec<ThreadInfo>,
    pub total_thread_count: usize,
    pub total_thread_retained: u64,
    pub top_retainers: Vec<ThreadInfo>,
}

pub fn inspect_threads(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    top_n: usize,
) -> ThreadReport {
    let thread_roots = map_thread_roots(graph);
    let mut threads = Vec::new();

    for object in graph.objects.values() {
        if !is_thread_object(graph, object.class_id) {
            continue;
        }

        let thread_root = thread_roots.get(&object.id).copied();
        let name = resolve_thread_name(graph, object.id).unwrap_or_else(|| {
            if let Some((thread_serial, _)) = thread_root {
                format!("Thread-{thread_serial}")
            } else {
                format!("Thread-{}", object.id)
            }
        });
        let daemon = matches!(
            read_field(object, &graph.classes, "daemon", graph.identifier_size),
            Some(FieldValue::Boolean(true))
        );
        let retained_bytes = dominator
            .map(|dom| dom.retained_size(object.id))
            .unwrap_or(u64::from(object.shallow_size));
        let (thread_local_count, thread_local_bytes) = dominator
            .map(|dom| dominated_descendants(dom, graph, object.id))
            .unwrap_or((0, 0));
        let stack_trace = thread_root
            .and_then(|(_, stack_trace_serial)| resolve_stack_trace(graph, stack_trace_serial));

        threads.push(ThreadInfo {
            object_id: object.id,
            name,
            daemon,
            stack_trace,
            retained_bytes,
            thread_local_count,
            thread_local_bytes,
        });
    }

    threads.sort_by(|left, right| {
        right
            .retained_bytes
            .cmp(&left.retained_bytes)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let total_thread_count = threads.len();
    let total_thread_retained = threads.iter().map(|thread| thread.retained_bytes).sum();
    let mut top_retainers = threads.clone();
    top_retainers.truncate(top_n);

    ThreadReport {
        threads,
        total_thread_count,
        total_thread_retained,
        top_retainers,
    }
}

fn map_thread_roots(graph: &ObjectGraph) -> HashMap<ObjectId, (u32, u32)> {
    graph
        .gc_roots
        .iter()
        .filter_map(|root| match root.root_type {
            GcRootType::ThreadObject {
                thread_serial,
                stack_trace_serial,
            } => Some((root.object_id, (thread_serial, stack_trace_serial))),
            _ => None,
        })
        .collect()
}

fn is_thread_object(graph: &ObjectGraph, mut class_id: ObjectId) -> bool {
    while class_id != 0 {
        let Some(class_info) = graph.classes.get(&class_id) else {
            return false;
        };
        if class_info
            .name
            .as_deref()
            .is_some_and(|name| matches_class_name(name, "java.lang.Thread"))
        {
            return true;
        }
        class_id = class_info.super_class_id;
    }

    false
}

fn resolve_thread_name(graph: &ObjectGraph, object_id: ObjectId) -> Option<String> {
    let object = graph.objects.get(&object_id)?;
    let name_id = match read_field(object, &graph.classes, "name", graph.identifier_size)? {
        FieldValue::ObjectRef(Some(name_id)) => name_id,
        _ => return None,
    };
    extract_string_value(graph, name_id)
}

fn resolve_stack_trace(
    graph: &ObjectGraph,
    stack_trace_serial: u32,
) -> Option<Vec<StackFrameInfo>> {
    let stack_trace = graph.stack_traces.get(&stack_trace_serial)?;
    let frames: Vec<StackFrameInfo> = stack_trace
        .frame_ids
        .iter()
        .filter_map(|frame_id| graph.stack_frames.get(frame_id))
        .map(|frame| StackFrameInfo {
            method_name: frame.method_name.clone(),
            class_name: frame.class_name.clone(),
            source_file: frame.source_file.clone(),
            line_number: frame.line_number,
        })
        .collect();

    if frames.is_empty() {
        None
    } else {
        Some(frames)
    }
}

fn dominated_descendants(
    dominator: &DominatorTree,
    graph: &ObjectGraph,
    root_id: ObjectId,
) -> (usize, u64) {
    let mut count = 0usize;
    let mut bytes = 0u64;
    let mut stack = dominator.dominated_by(root_id).to_vec();

    while let Some(object_id) = stack.pop() {
        count += 1;
        bytes += graph
            .objects
            .get(&object_id)
            .map(|object| u64::from(object.shallow_size))
            .unwrap_or(0);
        stack.extend_from_slice(dominator.dominated_by(object_id));
    }

    (count, bytes)
}

fn matches_class_name(actual: &str, expected: &str) -> bool {
    actual == expected || actual.replace('/', ".") == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectKind, StackFrame, StackTrace,
    };

    fn add_class(
        graph: &mut ObjectGraph,
        class_id: u64,
        super_class_id: u64,
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

    fn ref_bytes(id: u64) -> Vec<u8> {
        id.to_be_bytes().to_vec()
    }

    fn make_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        add_class(&mut graph, 1, 0, "java.lang.Object", Vec::new());
        add_class(
            &mut graph,
            2,
            1,
            "java.lang.String",
            vec![
                FieldDescriptor {
                    name: Some("value".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("coder".into()),
                    field_type: field_types::BYTE,
                },
            ],
        );
        add_class(
            &mut graph,
            3,
            1,
            "java.lang.Thread",
            vec![
                FieldDescriptor {
                    name: Some("name".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("daemon".into()),
                    field_type: field_types::BOOLEAN,
                },
            ],
        );
        add_class(&mut graph, 4, 3, "com.example.WorkerThread", Vec::new());
        add_class(&mut graph, 5, 1, "com.example.LocalPayload", Vec::new());

        graph.objects.insert(
            10,
            HeapObject {
                id: 10,
                class_id: 4,
                shallow_size: 24,
                references: vec![20, 30],
                field_data: [ref_bytes(20), vec![1]].concat(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            20,
            HeapObject {
                id: 20,
                class_id: 2,
                shallow_size: 16,
                references: vec![21],
                field_data: [ref_bytes(21), vec![0]].concat(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            21,
            HeapObject {
                id: 21,
                class_id: 0,
                shallow_size: 4,
                references: Vec::new(),
                field_data: b"main".to_vec(),
                kind: ObjectKind::PrimitiveArray {
                    element_type: field_types::BYTE,
                    length: 4,
                },
            },
        );
        graph.objects.insert(
            30,
            HeapObject {
                id: 30,
                class_id: 5,
                shallow_size: 32,
                references: vec![31],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            31,
            HeapObject {
                id: 31,
                class_id: 5,
                shallow_size: 8,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );

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
                method_name: String::from("run"),
                class_name: String::from("com.example.WorkerThread"),
                source_file: Some(String::from("WorkerThread.java")),
                line_number: 123,
            },
        );
        graph.stack_frames.insert(
            1001,
            StackFrame {
                frame_id: 1001,
                method_name: String::from("mainLoop"),
                class_name: String::from("com.example.WorkerThread"),
                source_file: None,
                line_number: -1,
            },
        );

        graph
    }

    #[test]
    fn inspect_threads_resolves_thread_metadata() {
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let report = inspect_threads(&graph, Some(&dominator), 1);

        assert_eq!(report.total_thread_count, 1);
        assert_eq!(report.top_retainers.len(), 1);
        let thread = &report.threads[0];
        assert_eq!(thread.object_id, 10);
        assert_eq!(thread.name, "main");
        assert!(thread.daemon);
        assert_eq!(thread.retained_bytes, 84);
        assert_eq!(thread.thread_local_count, 4);
        assert_eq!(thread.thread_local_bytes, 60);
        let stack_trace = thread.stack_trace.as_ref().expect("stack trace");
        assert_eq!(stack_trace.len(), 2);
        assert_eq!(stack_trace[0].method_name, "run");
        assert_eq!(stack_trace[1].line_number, -1);
        assert_eq!(report.total_thread_retained, 84);
    }

    #[test]
    fn inspect_threads_falls_back_to_serial_when_name_is_missing() {
        let mut graph = make_graph();
        graph
            .objects
            .get_mut(&20)
            .expect("string object")
            .field_data = Vec::new();

        let report = inspect_threads(&graph, None, 5);

        assert_eq!(report.threads[0].name, "Thread-7");
        assert_eq!(report.threads[0].thread_local_count, 0);
    }
}
