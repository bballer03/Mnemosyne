use std::{fs, io::Write, path::Path};

use assert_cmd::Command;
use mnemosyne_core::test_fixtures::{build_graph_fixture, build_simple_fixture};
use predicates::prelude::*;
use serde_json::{Deserializer, Value};
use tempfile::{tempdir, NamedTempFile, TempDir};

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

fn stdout_string(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

fn parse_first_json_value(input: &str) -> Value {
    let mut stream = Deserializer::from_str(input).into_iter::<Value>();
    stream.next().unwrap().unwrap()
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn test_parse_prints_summary() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["parse", fixture_path.as_str()]);
    cmd.assert().success().stdout(
        predicate::str::contains("Estimated objects:")
            .and(predicate::str::contains("File size:"))
            .and(predicate::str::contains("Total HPROF records:")),
    );
}

#[test]
fn test_parse_nonexistent_file() {
    let missing = "/tmp/mnemosyne-cli-does-not-exist.hprof";
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["parse", missing]);
    cmd.assert().failure();
}

#[test]
fn test_leaks_succeeds() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["leaks", fixture_path.as_str()]);
    cmd.assert().success();
}

#[test]
fn test_leaks_with_package_filter() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["leaks", fixture_path.as_str(), "--package", "com.example"]);
    cmd.assert().success();
}

#[test]
fn test_leaks_with_severity_filter() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["leaks", fixture_path.as_str(), "--min-severity", "critical"]);
    cmd.assert().success();
}

#[test]
fn test_analyze_text_format() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["analyze", fixture_path.as_str()]);
    cmd.assert().success().stdout(
        predicate::str::contains("Mnemosyne Analysis")
            .and(predicate::str::contains("Total Objects:"))
            .and(predicate::str::contains("Detected Leaks:"))
            .and(predicate::str::contains("Graph Nodes:")),
    );
}

#[test]
fn test_analyze_json_format() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    assert!(json.is_object());
}

#[test]
fn test_analyze_to_output_file() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let output_dir = tempdir().unwrap();
    let output_path = output_dir.path().join("analysis.txt");
    let output_arg = path_arg(&output_path);
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["analyze", fixture_path.as_str(), "-o", output_arg.as_str()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Report (text/plain) written to"));

    let written = fs::read_to_string(&output_path).unwrap();
    assert!(written.contains("Mnemosyne Analysis"));
}

#[test]
fn test_analyze_with_graph_fixture() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["analyze", fixture_path.as_str()]);
    cmd.assert().success().stdout(
        predicate::str::contains("Dominators").and(predicate::str::contains("dominated by")),
    );
}

#[test]
fn test_gc_path_finds_path() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["gc-path", fixture_path.as_str(), "--object-id", "0x1000"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("GC path for 0x00001000:"));
}

#[test]
fn test_gc_path_with_max_depth() {
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

#[test]
fn test_diff_same_file() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["diff", fixture_path.as_str(), fixture_path.as_str()]);
    cmd.assert().success().stdout(
        predicate::str::contains("Heap diff:")
            .and(predicate::str::contains("Delta size: +0.00 MB"))
            .and(predicate::str::contains("Delta objects: +0")),
    );
}

#[test]
fn test_fix_succeeds() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["fix", fixture_path.as_str()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Fix for ").and(predicate::str::contains("Patch:")));
}

#[test]
fn test_config_prints_json() {
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd.arg("config").output().unwrap();
    assert!(output.status.success());

    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert!(json.is_object());
    assert!(json.get("parser").is_some());
    assert!(json.get("analysis").is_some());
}

#[test]
fn test_no_args_shows_help() {
    let (mut cmd, _sandbox) = cli_command();

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage:").or(predicate::str::contains("Commands:")));
}

#[test]
fn test_unknown_subcommand() {
    let (mut cmd, _sandbox) = cli_command();

    cmd.arg("definitely-not-a-command");
    cmd.assert().failure().stderr(
        predicate::str::contains("unrecognized subcommand").or(predicate::str::contains("Usage:")),
    );
}
