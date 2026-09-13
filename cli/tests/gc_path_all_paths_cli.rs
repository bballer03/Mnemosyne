//! CLI integration tests for M8 Slice 8.A: `gc-path --all-paths` /
//! `--by-class` / `--max-paths`, plus the mandatory regression gate proving
//! today's `gc-path` (no new flags) is untouched by the additive changes.

use std::{io::Write, path::Path};

use assert_cmd::Command;
use mnemosyne_core::hprof::test_fixtures::build_graph_fixture;
use tempfile::{tempdir, NamedTempFile, TempDir};

const TAG_STRING_IN_UTF8: u8 = 0x01;
const TAG_LOAD_CLASS: u8 = 0x02;
const TAG_HEAP_DUMP: u8 = 0x0C;

const SUB_ROOT_STICKY_CLASS: u8 = 0x05;
const SUB_CLASS_DUMP: u8 = 0x20;
const SUB_INSTANCE_DUMP: u8 = 0x21;

const TYPE_OBJECT: u8 = 2;

#[allow(deprecated)]
fn cli_command() -> (Command, TempDir) {
    let sandbox = tempdir().unwrap();
    let mut command = Command::cargo_bin("mnemosyne-cli").unwrap();
    command.current_dir(sandbox.path());
    command.env("HOME", sandbox.path());
    command.env("XDG_CONFIG_HOME", sandbox.path());
    command.env_remove("MNEMOSYNE_CONFIG");
    command.env_remove("MNEMOSYNE_OUTPUT_FORMAT");
    command.env_remove("MNEMOSYNE_USE_MMAP");
    command.env_remove("MNEMOSYNE_THREADS");
    command.env_remove("MNEMOSYNE_MAX_OBJECTS");
    command.env_remove("MNEMOSYNE_AI_ENABLED");
    command.env_remove("MNEMOSYNE_AI_PROVIDER");
    command.env_remove("MNEMOSYNE_AI_MODEL");
    command.env_remove("MNEMOSYNE_AI_TEMPERATURE");
    command.env_remove("MNEMOSYNE_AI_AUDIT_LOG");
    command.env_remove("MNEMOSYNE_MIN_SEVERITY");
    command.env_remove("MNEMOSYNE_PACKAGES");
    command.env_remove("MNEMOSYNE_LEAK_TYPES");
    (command, sandbox)
}

fn write_fixture(data: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(data).unwrap();
    file.flush().unwrap();
    file
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn stdout_string(output: &[u8]) -> String {
    strip_ansi(&String::from_utf8_lossy(output))
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }

        output.push(ch);
    }

    output
}

fn push_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

struct HprofBuilder {
    id_size: u8,
    buf: Vec<u8>,
    records: Vec<Vec<u8>>,
}

impl HprofBuilder {
    fn new(id_size: u8) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
        push_u32(&mut buf, u32::from(id_size));
        push_u64(&mut buf, 0);

        Self {
            id_size,
            buf,
            records: Vec::new(),
        }
    }

    fn write_id(buf: &mut Vec<u8>, id: u64, id_size: u8) {
        match id_size {
            4 => push_u32(buf, id as u32),
            8 => push_u64(buf, id),
            _ => panic!("unsupported id_size: {id_size}"),
        }
    }

    fn push_record(&mut self, tag: u8, body: Vec<u8>) {
        let mut record = Vec::with_capacity(9 + body.len());
        record.push(tag);
        push_u32(&mut record, 0);
        push_u32(&mut record, body.len() as u32);
        record.extend_from_slice(&body);
        self.records.push(record);
    }

    fn add_string(&mut self, id: u64, value: &str) {
        let mut body = Vec::new();
        Self::write_id(&mut body, id, self.id_size);
        body.extend_from_slice(value.as_bytes());
        self.push_record(TAG_STRING_IN_UTF8, body);
    }

    fn add_load_class(&mut self, serial: u32, class_obj_id: u64, name_string_id: u64) {
        let mut body = Vec::new();
        push_u32(&mut body, serial);
        Self::write_id(&mut body, class_obj_id, self.id_size);
        push_u32(&mut body, 0);
        Self::write_id(&mut body, name_string_id, self.id_size);
        self.push_record(TAG_LOAD_CLASS, body);
    }

    fn add_heap_dump(&mut self, sub_records: Vec<u8>) {
        self.push_record(TAG_HEAP_DUMP, sub_records);
    }

    fn build(self) -> Vec<u8> {
        let mut buf = self.buf;
        for record in self.records {
            buf.extend_from_slice(&record);
        }
        buf
    }
}

struct HeapDumpBuilder {
    id_size: u8,
    buf: Vec<u8>,
}

impl HeapDumpBuilder {
    fn new(id_size: u8) -> Self {
        Self {
            id_size,
            buf: Vec::new(),
        }
    }

    fn add_gc_root_sticky_class(&mut self, obj_id: u64) {
        self.buf.push(SUB_ROOT_STICKY_CLASS);
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
    }

    fn add_class_dump(
        &mut self,
        class_obj_id: u64,
        super_class_id: u64,
        instance_size: u32,
        instance_fields: &[(u64, u8)],
    ) {
        self.buf.push(SUB_CLASS_DUMP);
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        push_u32(&mut self.buf, 0);
        HprofBuilder::write_id(&mut self.buf, super_class_id, self.id_size);
        for _ in 0..5 {
            HprofBuilder::write_id(&mut self.buf, 0, self.id_size);
        }
        push_u32(&mut self.buf, instance_size);
        push_u16(&mut self.buf, 0);
        push_u16(&mut self.buf, 0);
        push_u16(&mut self.buf, instance_fields.len() as u16);
        for &(name_string_id, field_type) in instance_fields {
            HprofBuilder::write_id(&mut self.buf, name_string_id, self.id_size);
            self.buf.push(field_type);
        }
    }

    fn add_instance_dump(&mut self, obj_id: u64, class_obj_id: u64, field_bytes: &[u8]) {
        self.buf.push(SUB_INSTANCE_DUMP);
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        push_u32(&mut self.buf, 0);
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        push_u32(&mut self.buf, field_bytes.len() as u32);
        self.buf.extend_from_slice(field_bytes);
    }

    fn build(self) -> Vec<u8> {
        self.buf
    }
}

fn encode_object_fields(id_size: u8, ids: &[u64]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &id in ids {
        HprofBuilder::write_id(&mut bytes, id, id_size);
    }
    bytes
}

/// Diamond-shaped graph: Root(1) -> BranchA(2), BranchB(3); both branches
/// point at the same Target(4). Two distinct shortest paths reach Target.
fn build_diamond_fixture() -> Vec<u8> {
    const ID_SIZE: u8 = 4;
    const ROOT_NAME: u64 = 1;
    const BRANCH_A_NAME: u64 = 2;
    const BRANCH_B_NAME: u64 = 3;
    const TARGET_NAME: u64 = 4;
    const LEFT_FIELD: u64 = 5;
    const RIGHT_FIELD: u64 = 6;
    const NEXT_FIELD: u64 = 7;

    let mut builder = HprofBuilder::new(ID_SIZE);
    for (id, value) in [
        (ROOT_NAME, "com.example.Root"),
        (BRANCH_A_NAME, "com.example.BranchA"),
        (BRANCH_B_NAME, "com.example.BranchB"),
        (TARGET_NAME, "com.example.Target"),
        (LEFT_FIELD, "left"),
        (RIGHT_FIELD, "right"),
        (NEXT_FIELD, "next"),
    ] {
        builder.add_string(id, value);
    }
    builder.add_load_class(1, 0x100, ROOT_NAME);
    builder.add_load_class(2, 0x101, BRANCH_A_NAME);
    builder.add_load_class(3, 0x102, BRANCH_B_NAME);
    builder.add_load_class(4, 0x103, TARGET_NAME);

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    heap.add_gc_root_sticky_class(1);
    heap.add_class_dump(
        0x100,
        0,
        8,
        &[(LEFT_FIELD, TYPE_OBJECT), (RIGHT_FIELD, TYPE_OBJECT)],
    );
    heap.add_class_dump(0x101, 0x100, 4, &[(NEXT_FIELD, TYPE_OBJECT)]);
    heap.add_class_dump(0x102, 0x100, 4, &[(NEXT_FIELD, TYPE_OBJECT)]);
    heap.add_class_dump(0x103, 0x100, 0, &[]);

    heap.add_instance_dump(1, 0x100, &encode_object_fields(ID_SIZE, &[2, 3]));
    heap.add_instance_dump(2, 0x101, &encode_object_fields(ID_SIZE, &[4]));
    heap.add_instance_dump(3, 0x102, &encode_object_fields(ID_SIZE, &[4]));
    heap.add_instance_dump(4, 0x103, &[]);

    builder.add_heap_dump(heap.build());
    builder.build()
}

/// Root(1) -> Session(10), Session(11), Session(12): three live instances of
/// the same class, each reachable via a distinct one-hop path from root.
fn build_by_class_fixture() -> Vec<u8> {
    const ID_SIZE: u8 = 4;
    const ROOT_NAME: u64 = 1;
    const SESSION_NAME: u64 = 2;
    const FIELD_A: u64 = 3;
    const FIELD_B: u64 = 4;
    const FIELD_C: u64 = 5;

    let mut builder = HprofBuilder::new(ID_SIZE);
    for (id, value) in [
        (ROOT_NAME, "com.example.Root"),
        (SESSION_NAME, "com.example.Session"),
        (FIELD_A, "a"),
        (FIELD_B, "b"),
        (FIELD_C, "c"),
    ] {
        builder.add_string(id, value);
    }
    builder.add_load_class(1, 0x200, ROOT_NAME);
    builder.add_load_class(2, 0x201, SESSION_NAME);

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    heap.add_gc_root_sticky_class(1);
    heap.add_class_dump(
        0x200,
        0,
        12,
        &[
            (FIELD_A, TYPE_OBJECT),
            (FIELD_B, TYPE_OBJECT),
            (FIELD_C, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(0x201, 0x200, 0, &[]);

    heap.add_instance_dump(1, 0x200, &encode_object_fields(ID_SIZE, &[10, 11, 12]));
    heap.add_instance_dump(10, 0x201, &[]);
    heap.add_instance_dump(11, 0x201, &[]);
    heap.add_instance_dump(12, 0x201, &[]);

    builder.add_heap_dump(heap.build());
    builder.build()
}

#[test]
fn default_no_new_flags_output_is_byte_identical_to_pre_m8() {
    // Regression gate (design doc §12 / task requirement): today's
    // `gc-path` (no `--all-paths`/`--by-class`/`--max-paths`) must produce
    // exactly the same text output as before this change. `0x1000` is
    // itself the GC root in `build_graph_fixture()`.
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args(["gc-path", fixture_path.as_str(), "--object-id", "0x1000"])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert_eq!(
        stdout,
        "GC path for 0x00001000:\nROOT -> com.example.BigCache [0x00001000] via ROOT\n"
    );
}

#[test]
fn all_paths_diamond_fixture_returns_both_paths() {
    let fixture = write_fixture(&build_diamond_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "gc-path",
            fixture_path.as_str(),
            "--object-id",
            "0x4",
            "--all-paths",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("(2 found"), "{stdout}");
    assert!(!stdout.contains("truncated"), "{stdout}");
    assert!(stdout.contains("Path 1"), "{stdout}");
    assert!(stdout.contains("Path 2"), "{stdout}");
    assert!(stdout.contains("com.example.BranchA"), "{stdout}");
    assert!(stdout.contains("com.example.BranchB"), "{stdout}");
    assert!(stdout.contains("com.example.Target"), "{stdout}");
}

#[test]
fn by_class_with_three_live_instances_returns_paths_for_all_three() {
    let fixture = write_fixture(&build_by_class_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "gc-path",
            fixture_path.as_str(),
            "--by-class",
            "com.example.Session",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("(3 found"), "{stdout}");
    assert!(!stdout.contains("truncated"), "{stdout}");
    assert!(stdout.contains("Path 1"), "{stdout}");
    assert!(stdout.contains("Path 2"), "{stdout}");
    assert!(stdout.contains("Path 3"), "{stdout}");
}

#[test]
fn max_paths_truncation_is_surfaced_without_panicking() {
    let fixture = write_fixture(&build_diamond_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "gc-path",
            fixture_path.as_str(),
            "--object-id",
            "0x4",
            "--all-paths",
            "--max-paths",
            "1",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("(1 found, truncated)"), "{stdout}");
    assert!(stdout.contains("Path 1"), "{stdout}");
    assert!(!stdout.contains("Path 2"), "{stdout}");
}

#[test]
fn all_paths_object_id_not_found_returns_exit_code_8() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "gc-path",
        fixture_path.as_str(),
        "--object-id",
        "0xdeadbeef",
        "--all-paths",
    ]);
    cmd.assert().code(8);
}

#[test]
fn by_class_zero_live_instances_returns_exit_code_9() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "gc-path",
        fixture_path.as_str(),
        "--by-class",
        "com.example.DoesNotExist",
    ]);
    cmd.assert().code(9);
}

#[test]
fn object_id_and_by_class_are_mutually_exclusive() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "gc-path",
        fixture_path.as_str(),
        "--object-id",
        "0x1000",
        "--by-class",
        "com.example.BigCache",
        "--all-paths",
    ]);
    cmd.assert().failure();
}

#[test]
fn existing_max_depth_flag_still_works_with_no_new_flags() {
    // Second half of the regression boundary: today's other `gc-path` flag
    // combination (max-depth, no new flags) must keep succeeding exactly
    // as before.
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "gc-path",
        fixture_path.as_str(),
        "--object-id",
        "0x1000",
        "--max-depth",
        "2",
    ]);
    cmd.assert().success();
}
