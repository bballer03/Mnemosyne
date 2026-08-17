//! CLI integration tests for M8 Slice 8.C: `mnemosyne inspect`.

use std::{io::Write, path::Path};

use assert_cmd::Command;
use mnemosyne_core::hprof::test_fixtures::build_graph_fixture;
use tempfile::{tempdir, NamedTempFile, TempDir};

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

/// `build_graph_fixture()`: GC root 0x1000 (`com.example.BigCache`, field
/// `entries` -> 0x2000) -> 0x2000 (`java.lang.Object`, no fields).
const ROOT_OBJECT_ID: &str = "0x1000";
const CHILD_OBJECT_ID: &str = "0x2000";

#[test]
fn inspect_known_object_prints_text_report() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(
        stdout.contains("Object 0x00001000  (com.example.BigCache)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("References out (1): 0x00002000 (java.lang.Object)"),
        "{stdout}"
    );
    assert!(stdout.contains("Referrers in (0): (none)"), "{stdout}");
    assert!(!stdout.contains("Fields"), "{stdout}");
}

#[test]
fn inspect_child_object_reports_dominator_parent_and_referrer() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            CHILD_OBJECT_ID,
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(
        stdout.contains("Dominator parent: 0x00001000 (com.example.BigCache)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Referrers in (1): 0x00001000 (com.example.BigCache)"),
        "{stdout}"
    );
}

#[test]
fn inspect_with_retain_field_data_populates_fields_section() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
            "--retain-field-data",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(
        stdout.contains("Fields (--retain-field-data only):"),
        "{stdout}"
    );
    assert!(
        stdout.contains("entries: java.lang.Object = 0x00002000"),
        "{stdout}"
    );
}

#[test]
fn inspect_without_retain_field_data_omits_fields_section() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(!stdout.contains("Fields"), "{stdout}");
}

#[test]
fn inspect_json_format_round_trips_object_id() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON output");
    assert_eq!(value["object_id"], "0x00001000");
    assert_eq!(value["class_name"], "com.example.BigCache");
}

#[test]
fn inspect_toon_format_emits_sections() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
            "--format",
            "toon",
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.starts_with("TOON v1"), "{stdout}");
    assert!(stdout.contains("section object"), "{stdout}");
}

#[test]
fn inspect_unknown_object_id_returns_exit_code_8() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "inspect",
        fixture_path.as_str(),
        "--object-id",
        "0xdeadbeef",
    ]);
    cmd.assert().code(8);
}
