use std::{io::Write, path::Path};

use assert_cmd::Command;
use mnemosyne_core::hprof::test_fixtures::{build_graph_fixture, build_simple_fixture};
use tempfile::{tempdir, Builder as TempFileBuilder, NamedTempFile, TempDir};

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
    command.env_remove("MNEMOSYNE_OBJECT_DIFF_MAX_FINGERPRINTS");
    (command, sandbox)
}

fn write_fixture(data: &[u8]) -> NamedTempFile {
    write_fixture_with_suffix(data, ".hprof")
}

fn write_fixture_with_suffix(data: &[u8], suffix: &str) -> NamedTempFile {
    let mut file = TempFileBuilder::new().suffix(suffix).tempfile().unwrap();
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

fn build_overview_only_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    push_u32(&mut bytes, 8);
    push_u64(&mut bytes, 0);

    let body = b"synthetic-record";
    bytes.push(TAG_STRING_IN_UTF8);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, body.len() as u32 + 8);
    push_u64(&mut bytes, 1);
    bytes.extend_from_slice(body);

    bytes
}

fn build_cap_before_fixture() -> Vec<u8> {
    const ID_SIZE: u8 = 4;
    const ROOT_NAME: u64 = 1;
    const ADD_PARENT_NAME: u64 = 2;
    const REMOVE_PARENT_NAME: u64 = 3;
    const CHANGE_PARENT_NAME: u64 = 4;
    const ADD_ITEM_A_NAME: u64 = 5;
    const ADD_ITEM_B_NAME: u64 = 6;
    const ADD_ITEM_C_NAME: u64 = 7;
    const REMOVE_ITEM_A_NAME: u64 = 8;
    const REMOVE_ITEM_B_NAME: u64 = 9;
    const REMOVE_ITEM_C_NAME: u64 = 10;
    const CHANGE_ITEM_A_NAME: u64 = 11;
    const CHANGE_ITEM_B_NAME: u64 = 12;
    const CHANGE_ITEM_C_NAME: u64 = 13;
    const ROOT_CHILD_A_FIELD: u64 = 101;
    const ROOT_CHILD_B_FIELD: u64 = 102;
    const ROOT_CHILD_C_FIELD: u64 = 103;
    const PARENT_FIELD_A: u64 = 104;
    const PARENT_FIELD_B: u64 = 105;
    const PARENT_FIELD_C: u64 = 106;

    let mut builder = HprofBuilder::new(ID_SIZE);
    for (id, value) in [
        (ROOT_NAME, "com.example.Root"),
        (ADD_PARENT_NAME, "com.example.AddParent"),
        (REMOVE_PARENT_NAME, "com.example.RemoveParent"),
        (CHANGE_PARENT_NAME, "com.example.ChangeParent"),
        (ADD_ITEM_A_NAME, "com.example.AddItemA"),
        (ADD_ITEM_B_NAME, "com.example.AddItemB"),
        (ADD_ITEM_C_NAME, "com.example.AddItemC"),
        (REMOVE_ITEM_A_NAME, "com.example.RemoveItemA"),
        (REMOVE_ITEM_B_NAME, "com.example.RemoveItemB"),
        (REMOVE_ITEM_C_NAME, "com.example.RemoveItemC"),
        (CHANGE_ITEM_A_NAME, "com.example.ChangeItemA"),
        (CHANGE_ITEM_B_NAME, "com.example.ChangeItemB"),
        (CHANGE_ITEM_C_NAME, "com.example.ChangeItemC"),
        (ROOT_CHILD_A_FIELD, "left"),
        (ROOT_CHILD_B_FIELD, "middle"),
        (ROOT_CHILD_C_FIELD, "right"),
        (PARENT_FIELD_A, "slotA"),
        (PARENT_FIELD_B, "slotB"),
        (PARENT_FIELD_C, "slotC"),
    ] {
        builder.add_string(id, value);
    }

    for (serial, class_obj_id, name_string_id) in [
        (1, 100, ROOT_NAME),
        (2, 101, ADD_PARENT_NAME),
        (3, 102, REMOVE_PARENT_NAME),
        (4, 103, CHANGE_PARENT_NAME),
        (5, 104, ADD_ITEM_A_NAME),
        (6, 105, ADD_ITEM_B_NAME),
        (7, 106, ADD_ITEM_C_NAME),
        (8, 107, REMOVE_ITEM_A_NAME),
        (9, 108, REMOVE_ITEM_B_NAME),
        (10, 109, REMOVE_ITEM_C_NAME),
        (11, 110, CHANGE_ITEM_A_NAME),
        (12, 111, CHANGE_ITEM_B_NAME),
        (13, 112, CHANGE_ITEM_C_NAME),
    ] {
        builder.add_load_class(serial, class_obj_id, name_string_id);
    }

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    heap.add_gc_root_sticky_class(1);
    heap.add_class_dump(
        100,
        0,
        10,
        &[
            (ROOT_CHILD_A_FIELD, TYPE_OBJECT),
            (ROOT_CHILD_B_FIELD, TYPE_OBJECT),
            (ROOT_CHILD_C_FIELD, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(101, 100, 10, &[]);
    heap.add_class_dump(
        102,
        100,
        10,
        &[
            (PARENT_FIELD_A, TYPE_OBJECT),
            (PARENT_FIELD_B, TYPE_OBJECT),
            (PARENT_FIELD_C, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(
        103,
        100,
        10,
        &[
            (PARENT_FIELD_A, TYPE_OBJECT),
            (PARENT_FIELD_B, TYPE_OBJECT),
            (PARENT_FIELD_C, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(104, 100, 10, &[]);
    heap.add_class_dump(105, 100, 20, &[]);
    heap.add_class_dump(106, 100, 30, &[]);
    heap.add_class_dump(107, 100, 10, &[]);
    heap.add_class_dump(108, 100, 20, &[]);
    heap.add_class_dump(109, 100, 30, &[]);
    heap.add_class_dump(110, 100, 10, &[]);
    heap.add_class_dump(111, 100, 20, &[]);
    heap.add_class_dump(112, 100, 30, &[]);

    heap.add_instance_dump(1, 100, &encode_object_fields(ID_SIZE, &[10, 20, 30]));
    heap.add_instance_dump(10, 101, &[]);
    heap.add_instance_dump(20, 102, &encode_object_fields(ID_SIZE, &[21, 22, 23]));
    heap.add_instance_dump(21, 107, &[]);
    heap.add_instance_dump(22, 108, &[]);
    heap.add_instance_dump(23, 109, &[]);
    heap.add_instance_dump(30, 103, &encode_object_fields(ID_SIZE, &[31, 32, 33]));
    heap.add_instance_dump(31, 110, &[]);
    heap.add_instance_dump(32, 111, &[]);
    heap.add_instance_dump(33, 112, &[]);

    builder.add_heap_dump(heap.build());
    builder.build()
}

fn build_cap_after_fixture() -> Vec<u8> {
    const ID_SIZE: u8 = 4;
    const ROOT_NAME: u64 = 1;
    const ADD_PARENT_NAME: u64 = 2;
    const REMOVE_PARENT_NAME: u64 = 3;
    const CHANGE_PARENT_NAME: u64 = 4;
    const ADD_ITEM_A_NAME: u64 = 5;
    const ADD_ITEM_B_NAME: u64 = 6;
    const ADD_ITEM_C_NAME: u64 = 7;
    const REMOVE_ITEM_A_NAME: u64 = 8;
    const REMOVE_ITEM_B_NAME: u64 = 9;
    const REMOVE_ITEM_C_NAME: u64 = 10;
    const CHANGE_ITEM_A_NAME: u64 = 11;
    const CHANGE_ITEM_B_NAME: u64 = 12;
    const CHANGE_ITEM_C_NAME: u64 = 13;
    const ROOT_CHILD_A_FIELD: u64 = 101;
    const ROOT_CHILD_B_FIELD: u64 = 102;
    const ROOT_CHILD_C_FIELD: u64 = 103;
    const PARENT_FIELD_A: u64 = 104;
    const PARENT_FIELD_B: u64 = 105;
    const PARENT_FIELD_C: u64 = 106;

    let mut builder = HprofBuilder::new(ID_SIZE);
    for (id, value) in [
        (ROOT_NAME, "com.example.Root"),
        (ADD_PARENT_NAME, "com.example.AddParent"),
        (REMOVE_PARENT_NAME, "com.example.RemoveParent"),
        (CHANGE_PARENT_NAME, "com.example.ChangeParent"),
        (ADD_ITEM_A_NAME, "com.example.AddItemA"),
        (ADD_ITEM_B_NAME, "com.example.AddItemB"),
        (ADD_ITEM_C_NAME, "com.example.AddItemC"),
        (REMOVE_ITEM_A_NAME, "com.example.RemoveItemA"),
        (REMOVE_ITEM_B_NAME, "com.example.RemoveItemB"),
        (REMOVE_ITEM_C_NAME, "com.example.RemoveItemC"),
        (CHANGE_ITEM_A_NAME, "com.example.ChangeItemA"),
        (CHANGE_ITEM_B_NAME, "com.example.ChangeItemB"),
        (CHANGE_ITEM_C_NAME, "com.example.ChangeItemC"),
        (ROOT_CHILD_A_FIELD, "left"),
        (ROOT_CHILD_B_FIELD, "middle"),
        (ROOT_CHILD_C_FIELD, "right"),
        (PARENT_FIELD_A, "slotA"),
        (PARENT_FIELD_B, "slotB"),
        (PARENT_FIELD_C, "slotC"),
    ] {
        builder.add_string(id, value);
    }

    for (serial, class_obj_id, name_string_id) in [
        (1, 900, ROOT_NAME),
        (2, 901, ADD_PARENT_NAME),
        (3, 902, REMOVE_PARENT_NAME),
        (4, 903, CHANGE_PARENT_NAME),
        (5, 904, ADD_ITEM_A_NAME),
        (6, 905, ADD_ITEM_B_NAME),
        (7, 906, ADD_ITEM_C_NAME),
        (8, 907, REMOVE_ITEM_A_NAME),
        (9, 908, REMOVE_ITEM_B_NAME),
        (10, 909, REMOVE_ITEM_C_NAME),
        (11, 910, CHANGE_ITEM_A_NAME),
        (12, 911, CHANGE_ITEM_B_NAME),
        (13, 912, CHANGE_ITEM_C_NAME),
    ] {
        builder.add_load_class(serial, class_obj_id, name_string_id);
    }

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    heap.add_gc_root_sticky_class(11);
    heap.add_class_dump(
        900,
        0,
        10,
        &[
            (ROOT_CHILD_A_FIELD, TYPE_OBJECT),
            (ROOT_CHILD_B_FIELD, TYPE_OBJECT),
            (ROOT_CHILD_C_FIELD, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(
        901,
        900,
        10,
        &[
            (PARENT_FIELD_A, TYPE_OBJECT),
            (PARENT_FIELD_B, TYPE_OBJECT),
            (PARENT_FIELD_C, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(902, 900, 10, &[]);
    heap.add_class_dump(
        903,
        900,
        10,
        &[
            (PARENT_FIELD_A, TYPE_OBJECT),
            (PARENT_FIELD_B, TYPE_OBJECT),
            (PARENT_FIELD_C, TYPE_OBJECT),
        ],
    );
    heap.add_class_dump(904, 900, 10, &[]);
    heap.add_class_dump(905, 900, 20, &[]);
    heap.add_class_dump(906, 900, 30, &[]);
    heap.add_class_dump(907, 900, 10, &[]);
    heap.add_class_dump(908, 900, 20, &[]);
    heap.add_class_dump(909, 900, 30, &[]);
    heap.add_class_dump(910, 900, 40, &[(PARENT_FIELD_A, TYPE_OBJECT)]);
    heap.add_class_dump(911, 900, 50, &[(PARENT_FIELD_A, TYPE_OBJECT)]);
    heap.add_class_dump(912, 900, 60, &[(PARENT_FIELD_A, TYPE_OBJECT)]);

    heap.add_instance_dump(11, 900, &encode_object_fields(ID_SIZE, &[12, 13, 14]));
    heap.add_instance_dump(12, 901, &encode_object_fields(ID_SIZE, &[15, 16, 17]));
    heap.add_instance_dump(13, 902, &[]);
    heap.add_instance_dump(14, 903, &encode_object_fields(ID_SIZE, &[18, 19, 20]));
    heap.add_instance_dump(15, 904, &[]);
    heap.add_instance_dump(16, 905, &[]);
    heap.add_instance_dump(17, 906, &[]);
    heap.add_instance_dump(18, 910, &encode_object_fields(ID_SIZE, &[180]));
    heap.add_instance_dump(180, 904, &[]);
    heap.add_instance_dump(19, 911, &encode_object_fields(ID_SIZE, &[190]));
    heap.add_instance_dump(190, 905, &[]);
    heap.add_instance_dump(20, 912, &encode_object_fields(ID_SIZE, &[200]));
    heap.add_instance_dump(200, 906, &[]);

    builder.add_heap_dump(heap.build());
    builder.build()
}

#[test]
fn default_mode_class_text_byte_identical_to_v030() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let expected = format!(
        "Heap diff: {fixture_path} -> {fixture_path}\n  Delta size: +0.00 MB\n  Delta objects: +0\n  No dominant class or record shifts detected.\n"
    );

    let (mut cmd, _sandbox) = cli_command();
    let assert = cmd
        .args(["diff", fixture_path.as_str(), fixture_path.as_str()])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn mode_object_emits_object_diff_section() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "diff",
            fixture_path.as_str(),
            fixture_path.as_str(),
            "--mode",
            "object",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("object diff (strategy="), "{stdout}");
    assert!(stdout.contains("match quality:"), "{stdout}");
}

#[test]
fn full_fingerprint_without_retain_field_data_returns_code_7() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "diff",
        fixture_path.as_str(),
        fixture_path.as_str(),
        "--mode",
        "object",
        "--identity-strategy",
        "full-fingerprint",
    ]);
    cmd.assert().code(7);
}

#[test]
fn mode_object_on_overview_dump_returns_code_5() {
    let fixture = write_fixture(&build_overview_only_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "diff",
        fixture_path.as_str(),
        fixture_path.as_str(),
        "--mode",
        "object",
    ]);
    cmd.assert().code(5);
}

#[test]
fn oversized_dump_returns_code_6() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.env("MNEMOSYNE_OBJECT_DIFF_MAX_FINGERPRINTS", "1");
    cmd.args([
        "diff",
        fixture_path.as_str(),
        fixture_path.as_str(),
        "--mode",
        "object",
        "--object-diff-min-retained",
        "0",
    ]);
    cmd.assert().code(6);
}

#[test]
fn top_flag_caps_each_section() {
    let before = write_fixture(&build_cap_before_fixture());
    let after = write_fixture(&build_cap_after_fixture());
    let before_path = path_arg(before.path());
    let after_path = path_arg(after.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "diff",
            before_path.as_str(),
            after_path.as_str(),
            "--mode",
            "object",
            "--top",
            "2",
            "--object-diff-min-retained",
            "0",
            "--retained-change-threshold",
            "1",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("added (2):"), "{stdout}");
    assert!(stdout.contains("removed (2):"), "{stdout}");
    assert!(stdout.contains("retained_changed (2):"), "{stdout}");
}
