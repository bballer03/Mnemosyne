//! CLI integration tests for M13 Slice 13.B: `analyze --classloaders` gains
//! a "Duplicate classes across loaders" section, printed only when
//! `ClassLoaderReport.duplicate_classes` is non-empty -- same
//! empty-is-silent convention as the existing "Potential classloader leaks"
//! section (see `test_analyze_with_classloaders_flag` in
//! `cli/tests/integration.rs`, which this file's fixture-building approach
//! mirrors: `core::hprof::test_fixtures`'s public builders do not support
//! setting a class's `class_loader_id` to anything other than the bootstrap
//! loader (0), so -- following the same local, self-contained
//! hand-rolled-HPROF-bytes convention already used by
//! `cli/tests/gc_path_all_paths_cli.rs` -- this file defines its own small
//! HPROF builder rather than extending the shared production fixture code
//! for a single CLI-level test's sake.

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

#[allow(deprecated)] // Command::cargo_bin is deprecated in assert_cmd >=2.1
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

    /// Unlike the production `HeapDumpBuilder::add_class_dump` (and this
    /// file's own earlier CLI-test siblings), this variant takes an explicit
    /// `class_loader_id` rather than hardcoding the bootstrap loader (0) --
    /// required to construct a cross-loader duplicate-class shape at all.
    fn add_class_dump(
        &mut self,
        class_obj_id: u64,
        super_class_id: u64,
        class_loader_id: u64,
        instance_size: u32,
    ) {
        self.buf.push(SUB_CLASS_DUMP);
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        push_u32(&mut self.buf, 0);
        HprofBuilder::write_id(&mut self.buf, super_class_id, self.id_size);
        HprofBuilder::write_id(&mut self.buf, class_loader_id, self.id_size);
        for _ in 0..4 {
            HprofBuilder::write_id(&mut self.buf, 0, self.id_size);
        }
        push_u32(&mut self.buf, instance_size);
        push_u16(&mut self.buf, 0); // constant pool count
        push_u16(&mut self.buf, 0); // static field count
        push_u16(&mut self.buf, 0); // instance field count
    }

    fn add_instance_dump(&mut self, obj_id: u64, class_obj_id: u64) {
        self.buf.push(SUB_INSTANCE_DUMP);
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        push_u32(&mut self.buf, 0);
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        push_u32(&mut self.buf, 0); // no field data
    }

    fn build(self) -> Vec<u8> {
        self.buf
    }
}

/// Two distinct loader objects (0x1000, 0x2000), each declaring their own
/// class object for the same class name
/// ("com/example/webapp/RequestHandler") -- the classic "redeployed webapp"
/// shape: same class loaded twice, once per generation's classloader.
fn build_classloader_duplicate_fixture() -> Vec<u8> {
    const ID_SIZE: u8 = 4;
    const OBJECT_NAME: u64 = 1;
    const LOADER_NAME: u64 = 2;
    const HANDLER_NAME: u64 = 3;

    const OBJECT_CLASS: u64 = 0x100;
    const LOADER_CLASS: u64 = 0x200;
    const HANDLER_CLASS_GEN1: u64 = 0x300;
    const HANDLER_CLASS_GEN2: u64 = 0x301;

    const LOADER_ONE: u64 = 0x1000;
    const LOADER_TWO: u64 = 0x2000;
    const HANDLER_INSTANCE_ONE: u64 = 0x3000;
    const HANDLER_INSTANCE_TWO: u64 = 0x3001;

    let mut builder = HprofBuilder::new(ID_SIZE);
    builder.add_string(OBJECT_NAME, "java/lang/Object");
    builder.add_string(LOADER_NAME, "com/example/webapp/WebappLoader");
    builder.add_string(HANDLER_NAME, "com/example/webapp/RequestHandler");
    builder.add_load_class(1, OBJECT_CLASS, OBJECT_NAME);
    builder.add_load_class(2, LOADER_CLASS, LOADER_NAME);
    builder.add_load_class(3, HANDLER_CLASS_GEN1, HANDLER_NAME);
    builder.add_load_class(4, HANDLER_CLASS_GEN2, HANDLER_NAME);

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    heap.add_class_dump(OBJECT_CLASS, 0, 0, 0);
    heap.add_class_dump(LOADER_CLASS, OBJECT_CLASS, 0, 16);
    heap.add_class_dump(HANDLER_CLASS_GEN1, OBJECT_CLASS, LOADER_ONE, 8);
    heap.add_class_dump(HANDLER_CLASS_GEN2, OBJECT_CLASS, LOADER_TWO, 8);

    heap.add_instance_dump(LOADER_ONE, LOADER_CLASS);
    heap.add_instance_dump(LOADER_TWO, LOADER_CLASS);
    heap.add_instance_dump(HANDLER_INSTANCE_ONE, HANDLER_CLASS_GEN1);
    heap.add_instance_dump(HANDLER_INSTANCE_TWO, HANDLER_CLASS_GEN2);

    heap.add_gc_root_sticky_class(LOADER_ONE);
    heap.add_gc_root_sticky_class(LOADER_TWO);
    heap.add_gc_root_sticky_class(HANDLER_INSTANCE_ONE);
    heap.add_gc_root_sticky_class(HANDLER_INSTANCE_TWO);

    builder.add_heap_dump(heap.build());
    builder.build()
}

#[test]
fn analyze_classloaders_prints_duplicate_classes_section_when_present() {
    let fixture = write_fixture(&build_classloader_duplicate_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--classloaders"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);

    assert!(stdout.contains("ClassLoader Report:"));
    assert!(
        stdout.contains("Duplicate classes across loaders (1):"),
        "stdout missing duplicate-classes section: {stdout}"
    );
    assert!(
        stdout.contains("com.example.webapp.RequestHandler"),
        "stdout missing duplicated class name: {stdout}"
    );
    assert!(
        stdout.contains("loaded by 2 loaders"),
        "stdout missing loader count: {stdout}"
    );
}

#[test]
fn analyze_classloaders_omits_duplicate_classes_section_when_absent() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--classloaders"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);

    assert!(stdout.contains("ClassLoader Report:"));
    assert!(!stdout.contains("Duplicate classes across loaders"));
}

/// End-to-end check that `ci-check` actually enables classloader analysis
/// when the policy declares a `classloader_leak_count` rule: without this,
/// `AnalyzeRequest.enable_classloaders` stayed hardcoded `false` in
/// `handle_ci_check`, so `ClassLoaderReport` (and therefore
/// `duplicate_classes`) was never populated and the rule silently skipped
/// on every real invocation, even though the predicate's own evaluator
/// logic (tested directly in `core::policy`) was correct.
#[test]
fn ci_check_classloader_leak_count_rule_fires_on_duplicate_fixture() {
    let fixture = write_fixture(&build_classloader_duplicate_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let policy_path = sandbox.path().join("policy.toml");
    std::fs::write(
        &policy_path,
        "[[rule]]\nid = \"no-classloader-duplicates\"\npredicate = \"classloader_leak_count\"\nop = \"<=\"\nvalue = 0\nseverity = \"error\"\n",
    )
    .unwrap();
    let policy_arg = path_arg(&policy_path);

    let output = cmd
        .args([
            "ci-check",
            fixture_path.as_str(),
            "--policy",
            policy_arg.as_str(),
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected a policy violation (exit 1); stdout: {}",
        stdout_string(&output.stdout)
    );
    let stdout = stdout_string(&output.stdout);
    assert!(stdout.contains("RESULT: FAIL"), "{stdout}");
}

/// Companion regression: a heap with no cross-loader duplicates must not
/// trip the same rule.
#[test]
fn ci_check_classloader_leak_count_rule_passes_on_clean_fixture() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let policy_path = sandbox.path().join("policy.toml");
    std::fs::write(
        &policy_path,
        "[[rule]]\nid = \"no-classloader-duplicates\"\npredicate = \"classloader_leak_count\"\nop = \"<=\"\nvalue = 0\nseverity = \"error\"\n",
    )
    .unwrap();
    let policy_arg = path_arg(&policy_path);

    let output = cmd
        .args([
            "ci-check",
            fixture_path.as_str(),
            "--policy",
            policy_arg.as_str(),
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected a clean pass (exit 0); stdout: {}",
        stdout_string(&output.stdout)
    );
}
