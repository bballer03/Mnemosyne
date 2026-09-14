//! Integration tests for M19 Slice 19.D: `WorkflowKind::ClassloaderLeak`.

use std::io::Write;

use mnemosyne_core::workflow::{advance, describe, start, WorkflowKind, WorkflowStore};
use serde_json::json;
use tempfile::NamedTempFile;

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn write_id(buf: &mut Vec<u8>, id: u64) {
    push_u32(buf, id as u32);
}

fn push_record(records: &mut Vec<Vec<u8>>, tag: u8, body: Vec<u8>) {
    let mut record = Vec::with_capacity(9 + body.len());
    record.push(tag);
    push_u32(&mut record, 0);
    push_u32(&mut record, body.len() as u32);
    record.extend_from_slice(&body);
    records.push(record);
}

/// Same cross-loader duplicate shape used by MCP/CLI classloader fixtures.
fn build_classloader_duplicate_fixture() -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    push_u32(&mut header, 4);
    push_u64(&mut header, 0);

    let mut records = Vec::new();

    let mut add_string = |id: u64, value: &str| {
        let mut body = Vec::new();
        write_id(&mut body, id);
        body.extend_from_slice(value.as_bytes());
        push_record(&mut records, 0x01, body);
    };
    add_string(1, "java/lang/Object");
    add_string(2, "com/example/webapp/WebappLoader");
    add_string(3, "com/example/webapp/RequestHandler");

    let mut add_load_class = |serial: u32, class_obj_id: u64, name_id: u64| {
        let mut body = Vec::new();
        push_u32(&mut body, serial);
        write_id(&mut body, class_obj_id);
        push_u32(&mut body, 0);
        write_id(&mut body, name_id);
        push_record(&mut records, 0x02, body);
    };
    add_load_class(1, 0x100, 1);
    add_load_class(2, 0x200, 2);
    add_load_class(3, 0x300, 3);
    add_load_class(4, 0x301, 3);

    let mut heap = Vec::new();
    for (class_obj_id, super_id, loader_id, size) in [
        (0x100u64, 0u64, 0u64, 0u32),
        (0x200, 0x100, 0, 16),
        (0x300, 0x100, 0x1000, 8),
        (0x301, 0x100, 0x2000, 8),
    ] {
        heap.push(0x20);
        write_id(&mut heap, class_obj_id);
        push_u32(&mut heap, 0);
        write_id(&mut heap, super_id);
        write_id(&mut heap, loader_id);
        for _ in 0..4 {
            write_id(&mut heap, 0);
        }
        push_u32(&mut heap, size);
        heap.extend_from_slice(&0u16.to_be_bytes());
        heap.extend_from_slice(&0u16.to_be_bytes());
        heap.extend_from_slice(&0u16.to_be_bytes());
    }
    for (obj_id, class_obj_id) in [
        (0x1000u64, 0x200u64),
        (0x2000, 0x200),
        (0x3000, 0x300),
        (0x3001, 0x301),
    ] {
        heap.push(0x21);
        write_id(&mut heap, obj_id);
        push_u32(&mut heap, 0);
        write_id(&mut heap, class_obj_id);
        push_u32(&mut heap, 0);
    }
    for obj_id in [0x1000u64, 0x2000, 0x3000, 0x3001] {
        heap.push(0x05);
        write_id(&mut heap, obj_id);
    }
    push_record(&mut records, 0x0C, heap);

    let mut bytes = header;
    for record in records {
        bytes.extend_from_slice(&record);
    }
    bytes
}

fn write_fixture_heap() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&build_classloader_duplicate_fixture())
        .unwrap();
    file.flush().unwrap();
    file
}

async fn run_full_workflow(
    store: &WorkflowStore,
    heap: &str,
) -> mnemosyne_core::workflow::WorkflowState {
    let state = start(
        store,
        WorkflowKind::ClassloaderLeak,
        heap.to_string(),
        json!(null),
    )
    .await
    .expect("detect should succeed");
    assert_eq!(state.current_step, "select");

    let class_name = state.step_history[0]
        .output_summary
        .get("duplicate_class_names")
        .and_then(|names| names.get(0))
        .and_then(|name| name.as_str())
        .expect("detect should surface a duplicate class")
        .to_string();

    let state = advance(
        store,
        &state.workflow_id,
        json!({ "class_name": class_name }),
    )
    .await
    .expect("select should accept a detect-returned class");
    assert_eq!(state.current_step, "inspect_retention");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("inspect_retention should succeed");
    assert_eq!(state.current_step, "explain");

    let state = advance(store, &state.workflow_id, json!(null))
        .await
        .expect("explain should succeed");
    assert_eq!(state.current_step, "complete");
    state
}

#[tokio::test]
async fn full_run_step_sequence_matches_static_description_exactly() {
    let temp = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(temp.path().to_path_buf());
    let heap = write_fixture_heap();
    let heap_path = heap.path().to_str().unwrap();

    let state = run_full_workflow(&store, heap_path).await;
    let observed: Vec<&str> = state
        .step_history
        .iter()
        .map(|step| step.step_name.as_str())
        .collect();
    let description = describe(WorkflowKind::ClassloaderLeak).unwrap();
    let expected: Vec<&str> = description
        .steps
        .iter()
        .map(|step| step.name.as_str())
        .collect();
    assert_eq!(observed, expected);
}

#[tokio::test]
async fn invalid_selection_returns_structured_error_and_keeps_workflow_resumable() {
    let temp = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(temp.path().to_path_buf());
    let heap = write_fixture_heap();
    let heap_path = heap.path().to_str().unwrap().to_string();

    let state = start(
        &store,
        WorkflowKind::ClassloaderLeak,
        heap_path,
        json!(null),
    )
    .await
    .unwrap();
    assert_eq!(state.current_step, "select");

    let err = advance(
        &store,
        &state.workflow_id,
        json!({ "class_name": "com/example/Missing" }),
    )
    .await
    .expect_err("unknown class must fail");
    let message = err.to_string();
    assert!(
        message.contains("class_name") && message.contains("detect"),
        "unexpected error: {message}"
    );

    let resumed = store.load(&state.workflow_id).unwrap();
    assert_eq!(resumed.current_step, "select");
}

#[tokio::test]
async fn closing_completed_workflow_rejects_further_advance() {
    let temp = tempfile::tempdir().unwrap();
    let store = WorkflowStore::new(temp.path().to_path_buf());
    let heap = write_fixture_heap();
    let state = run_full_workflow(&store, heap.path().to_str().unwrap()).await;

    let err = advance(&store, &state.workflow_id, json!(null))
        .await
        .expect_err("completed workflow must reject advance");
    assert!(err.to_string().contains("complete") || err.to_string().contains("already"));
}
