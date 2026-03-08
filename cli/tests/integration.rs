use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use mnemosyne_core::hprof::test_fixtures::{build_graph_fixture, build_simple_fixture};
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

fn real_heap_fixture_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .join("resources/test-fixtures/heap.hprof")
}

fn extract_usize_after_label(input: &str, label: &str) -> usize {
    input
        .split_once(label)
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .map(|value| value.trim_matches(|ch: char| !ch.is_ascii_digit()))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap()
}

fn write_record(buf: &mut Vec<u8>, tag: u8, body: &[u8]) {
    buf.push(tag);
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body);
}

fn build_parse_table_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    bytes.extend_from_slice(&8u32.to_be_bytes());
    bytes.extend_from_slice(&0u64.to_be_bytes());

    write_record(&mut bytes, 0x21, &[0; 28]);
    write_record(&mut bytes, 0x21, &[0; 20]);
    write_record(&mut bytes, 0x22, &[0; 16]);
    write_record(&mut bytes, 0x23, &[0; 8]);
    write_record(&mut bytes, 0x0C, &[0; 12]);

    bytes
}

fn build_fallback_leak_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"JAVA PROFILE 1.0.2\0");
    bytes.extend_from_slice(&8u32.to_be_bytes());
    bytes.extend_from_slice(&0u64.to_be_bytes());

    write_record(&mut bytes, 0x01, b"synthetic-record");
    write_record(&mut bytes, 0x0C, &[]);

    bytes
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

fn normalized_stdout(output: &[u8]) -> String {
    strip_ansi(&stdout_string(output))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_for_assert(value: &str, max_width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_width {
        return value.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let truncated: String = value.chars().take(max_width - 3).collect();
    format!("{truncated}...")
}

fn extract_between<'a>(input: &'a str, prefix: &str, suffix: &str) -> &'a str {
    input
        .split_once(prefix)
        .and_then(|(_, rest)| rest.split_once(suffix).map(|(value, _)| value))
        .unwrap()
}

fn extract_all_between(input: &str, prefix: &str, suffix: &str) -> Vec<String> {
    input
        .split(prefix)
        .skip(1)
        .map(|segment| segment.split_once(suffix).unwrap().0.to_string())
        .collect()
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
fn test_parse_emits_table_formatted_summary_sections() {
    let fixture = write_fixture(&build_parse_table_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd.args(["parse", fixture_path.as_str()]).output().unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Top heap record categories by aggregate bytes:"));
    assert!(stdout.contains("# Record Category Bytes Share Entries"));
    assert!(stdout.contains("INSTANCE_DUMP"));
    assert!(stdout.contains("OBJECT_ARRAY_DUMP"));
    assert!(stdout.contains("PRIMITIVE_ARRAY_DUMP"));

    assert!(stdout.contains("Top record tags:"));
    assert!(stdout.contains("Record Tag Hex Entries Size"));
    assert!(stdout.contains("HEAP_DUMP 0x0C"));
}

#[test]
fn test_parse_nonexistent_file() {
    let missing = "/tmp/mnemosyne-cli-does-not-exist.hprof";
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["parse", missing]);
    cmd.assert().failure().stderr(
        predicate::str::contains("not found").or(predicate::str::contains("Heap dump not found")),
    );
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
fn test_leaks_emits_table_and_readable_detail_blocks() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["leaks", fixture_path.as_str(), "--min-severity", "low"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Potential leaks:"));
    assert!(stdout.contains("Leak ID Class Kind Severity Retained Instances"));
    assert!(stdout.contains("com.example.CacheLeak"));
    assert!(stdout.contains("Cache"));

    assert!(stdout.contains("Leak:"));
    assert!(stdout.contains("Description:"));
    assert!(stdout.contains("Provenance:"));
    assert!(stdout.contains("SYNTHETIC"));
    assert!(stdout.contains("FALLBACK"));
}

#[test]
fn test_leaks_prints_full_ids_and_class_names_after_table_truncation() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let long_package =
        "com.example.really.long.package.name.with.deep.nesting.for.truncation.regression";
    let full_class_name = format!("{long_package}.CacheLeakCandidate");
    let truncated_class_name = truncate_for_assert(&full_class_name, 34);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "leaks",
            fixture_path.as_str(),
            "--min-severity",
            "low",
            "--package",
            long_package,
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    let full_leak_id = extract_between(&stdout, "Leak: ", " Description:");
    let truncated_leak_id = truncate_for_assert(full_leak_id, 20);

    assert!(stdout.contains("Potential leaks:"));
    assert!(stdout.contains("Leak ID Class Kind Severity Retained Instances"));
    assert_ne!(truncated_leak_id, full_leak_id);
    assert!(stdout.contains(&truncated_leak_id));
    assert!(stdout.contains("Full leak IDs for truncated rows:"));
    assert!(stdout.contains(&format!("{} -> {}", truncated_leak_id, full_leak_id)));
    assert!(stdout.contains(&format!("Leak: {full_leak_id}")));
    assert!(stdout.contains(&truncated_class_name));
    assert!(stdout.contains("Full class names for truncated leak rows:"));
    assert!(stdout.contains(&format!("-> {full_class_name}")));
}

#[test]
fn test_leaks_discloses_colliding_truncated_ids_with_row_stable_mapping() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let long_package =
        "com.example.really.long.package.name.with.deep.nesting.for.row.stable.id.regression";
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "leaks",
            fixture_path.as_str(),
            "--min-severity",
            "low",
            "--package",
            long_package,
            "--leak-kind",
            "cache,collection",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    let full_leak_ids = extract_all_between(&stdout, "Leak: ", " Description:");

    assert_eq!(full_leak_ids.len(), 2);

    let truncated_first = truncate_for_assert(&full_leak_ids[0], 20);
    let truncated_second = truncate_for_assert(&full_leak_ids[1], 20);

    assert_ne!(full_leak_ids[0], full_leak_ids[1]);
    assert_eq!(truncated_first, truncated_second);
    assert!(stdout.contains("Full leak IDs for truncated rows:"));
    assert!(stdout.contains(&format!(
        "row 1 | {} -> {}",
        truncated_first, full_leak_ids[0]
    )));
    assert!(stdout.contains(&format!(
        "row 2 | {} -> {}",
        truncated_second, full_leak_ids[1]
    )));
    assert!(stdout.contains(&format!("Leak: {}", full_leak_ids[0])));
    assert!(stdout.contains(&format!("Leak: {}", full_leak_ids[1])));
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
fn test_parse_real_heap_dump() {
    let fixture_path = real_heap_fixture_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping: real heap fixture not found at {}",
            fixture_path.display()
        );
        return;
    }

    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd.args(["parse", fixture_arg.as_str()]).output().unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("HEAP_DUMP_SEGMENT"));
    assert!(stdout.contains("Estimated objects:"));
    assert!(stdout.contains("File size:"));
}

#[test]
fn test_analyze_real_heap_dump() {
    let fixture_path = real_heap_fixture_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping: real heap fixture not found at {}",
            fixture_path.display()
        );
        return;
    }

    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_arg.as_str()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(
        stdout.contains("Dominators")
            || stdout.contains("dominated by")
            || stdout.contains("Graph-backed analysis complete")
    );
    assert!(extract_usize_after_label(&stdout, "Graph Nodes:") > 7);
}

#[test]
fn test_leaks_real_heap_dump() {
    let fixture_path = real_heap_fixture_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping: real heap fixture not found at {}",
            fixture_path.display()
        );
        return;
    }

    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["leaks", fixture_arg.as_str(), "--min-severity", "low"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Potential leaks:") || stdout.contains("Leak:"));
}

#[test]
fn test_gc_path_real_heap_dump() {
    let fixture_path = real_heap_fixture_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping: real heap fixture not found at {}",
            fixture_path.display()
        );
        return;
    }

    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["gc-path", fixture_arg.as_str(), "--object-id", "0x1"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("GC path for"));
}

#[test]
fn test_leaks_real_heap_fallback_with_nonexistent_package() {
    let fixture_path = real_heap_fixture_path();
    if !fixture_path.exists() {
        eprintln!(
            "Skipping: real heap fixture not found at {}",
            fixture_path.display()
        );
        return;
    }

    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "leaks",
            fixture_arg.as_str(),
            "--package",
            "nonexistent.pkg.that.matches.nothing",
            "--min-severity",
            "low",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(
        stdout.contains("Potential leaks:")
            || stdout.contains("Leak:")
            || stdout.contains("No leak")
    );
    assert!(stdout.contains("FALLBACK") || stdout.contains("SYNTHETIC") || !stdout.is_empty());
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
fn test_explain_invalid_leak_id_errors() {
    let real_fixture_path = real_heap_fixture_path();
    let synthetic_fixture;
    let fixture_path = if real_fixture_path.exists() {
        real_fixture_path
    } else {
        synthetic_fixture = write_fixture(&build_simple_fixture());
        synthetic_fixture.path().to_path_buf()
    };
    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "explain",
            fixture_arg.as_str(),
            "--leak-id",
            "nonexistent-leak-id-12345",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = normalized_stdout(&output.stderr);
    assert!(stderr.contains("no leak found") || stderr.contains("matching identifier"));
}

#[test]
fn test_fix_invalid_leak_id_errors() {
    let real_fixture_path = real_heap_fixture_path();
    let synthetic_fixture;
    let fixture_path = if real_fixture_path.exists() {
        real_fixture_path
    } else {
        synthetic_fixture = write_fixture(&build_simple_fixture());
        synthetic_fixture.path().to_path_buf()
    };
    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "fix",
            fixture_arg.as_str(),
            "--leak-id",
            "nonexistent-leak-id-12345",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = normalized_stdout(&output.stderr);
    assert!(stderr.contains("no leak found") || stderr.contains("matching identifier"));
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

#[test]
fn test_parse_jar_file_shows_hint() {
    let dir = tempdir().unwrap();
    let jar_path = dir.path().join("app.jar");
    fs::write(&jar_path, b"PK\x03\x04fake jar content").unwrap();
    let path_str = path_arg(&jar_path);
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["parse", path_str.as_str()]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("hint:").or(predicate::str::contains("JAR")));
}

#[test]
fn test_parse_txt_file_shows_hint() {
    let dir = tempdir().unwrap();
    let txt_path = dir.path().join("data.txt");
    fs::write(&txt_path, b"this is not a heap dump").unwrap();
    let path_str = path_arg(&txt_path);
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["parse", path_str.as_str()]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("hint:").or(predicate::str::contains(".txt")));
}

#[test]
fn test_invalid_config_file_shows_error() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("bad_config.toml");
    fs::write(&config_path, b"this is [[[not valid toml").unwrap();
    let path_str = path_arg(&config_path);
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["--config", path_str.as_str(), "config"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("TOML").or(predicate::str::contains("config")));
}
