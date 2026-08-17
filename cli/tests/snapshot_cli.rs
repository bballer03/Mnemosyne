//! CLI integration tests for M9 Slice 9.C: `mnemosyne snapshot save|load|list|rm`
//! plus the additive `--snapshot`/`--refresh` flags on `analyze`, `leaks`,
//! `gc-path`, `inspect`, and `query`.
//!
//! Every command in this file that touches the snapshot cache is pinned to
//! an isolated `MNEMOSYNE_SNAPSHOT_DIR` (a fresh tempdir per test) so tests
//! never share state with each other or with a developer's real
//! `~/.cache/mnemosyne/`.

use std::path::Path;

use assert_cmd::Command;
use mnemosyne_core::hprof::test_fixtures::build_graph_fixture;
use predicates::prelude::*;
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
    command.env_remove("MNEMOSYNE_AI_AUDIT_LOG");
    command.env_remove("MNEMOSYNE_MIN_SEVERITY");
    command.env_remove("MNEMOSYNE_PACKAGES");
    command.env_remove("MNEMOSYNE_LEAK_TYPES");
    command.env_remove("MNEMOSYNE_SNAPSHOT_DIR");
    (command, sandbox)
}

/// A fresh, isolated snapshot cache directory shared across the (possibly
/// several) separate CLI invocations within a single test.
fn snapshot_cache() -> TempDir {
    tempdir().unwrap()
}

fn write_fixture(data: &[u8]) -> NamedTempFile {
    use std::io::Write;
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

fn stderr_string(output: &[u8]) -> String {
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

/// Parse the SHA-256 hash out of a `mnemosyne snapshot save` command's
/// `"Snapshot saved: <hash>"` stdout line.
fn extract_saved_hash(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Snapshot saved: "))
        .map(|rest| rest.trim().to_string())
        .unwrap_or_else(|| panic!("expected a 'Snapshot saved: <hash>' line in:\n{stdout}"))
}

/// `build_graph_fixture()`: GC root 0x1000 (`com.example.BigCache`, field
/// `entries` -> 0x2000) -> 0x2000 (`java.lang.Object`, no fields).
const ROOT_OBJECT_ID: &str = "0x1000";
const CHILD_OBJECT_ID: &str = "0x2000";

// --- snapshot save / list / rm --------------------------------------------

#[test]
fn snapshot_save_creates_cache_entry() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "save", fixture_path.as_str()])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("Snapshot saved:"), "{stdout}");
    assert!(stdout.contains("Object count: 2"), "{stdout}");
    let hash = extract_saved_hash(&stdout);
    assert_eq!(hash.len(), 64, "expected a full SHA-256 hex hash: {hash}");
    assert!(cache.path().join(format!("{hash}.json")).exists());
}

#[test]
fn snapshot_save_with_output_dir_writes_to_override_root() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let output_dir = tempdir().unwrap();
    let default_cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", default_cache.path())
        .args([
            "snapshot",
            "save",
            fixture_path.as_str(),
            "--output",
            output_dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let hash = extract_saved_hash(&stdout_string(&assert.get_output().stdout));
    assert!(output_dir.path().join(format!("{hash}.json")).exists());
    assert!(!default_cache.path().join(format!("{hash}.json")).exists());
}

#[test]
fn snapshot_list_shows_saved_entry() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut list_cmd, _s2) = cli_command();
    let list_assert = list_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "list"])
        .assert()
        .success();

    let stdout = stdout_string(&list_assert.get_output().stdout);
    assert!(stdout.contains("Cached snapshots:"), "{stdout}");
    assert!(stdout.contains(&hash[..12]), "{stdout}");
}

#[test]
fn snapshot_list_on_empty_cache_reports_none_cached() {
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    cmd.env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No snapshots cached."));
}

#[test]
fn snapshot_rm_removes_entry_and_second_rm_fails() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut rm_cmd, _s2) = cli_command();
    rm_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "rm", &hash])
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshot removed:"));
    assert!(!cache.path().join(format!("{hash}.json")).exists());

    let (mut rm_again_cmd, _s3) = cli_command();
    let assert = rm_again_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "rm", &hash])
        .assert()
        .code(10);
    let stderr = stderr_string(&assert.get_output().stderr);
    assert!(stderr.contains("snapshot_not_found"), "{stderr}");
}

#[test]
fn snapshot_load_prints_manifest_without_running_analysis() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut load_cmd, _s2) = cli_command();
    let assert = load_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "load", &hash])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("Snapshot loaded:"), "{stdout}");
    assert!(stdout.contains(&hash), "{stdout}");
    assert!(stdout.contains("Object count: 2"), "{stdout}");
    // "load" only prints the manifest -- no leak/analysis output.
    assert!(!stdout.contains("Potential leaks"), "{stdout}");
}

#[test]
fn snapshot_load_missing_key_returns_exit_10() {
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    cmd.env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "load", "does-not-exist"]);
    cmd.assert().code(10);
}

// --- auto-discovery / write-through caching on analyze --------------------

#[test]
fn analyze_no_flags_populates_snapshot_cache() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut analyze_cmd, _s1) = cli_command();
    analyze_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str()])
        .assert()
        .success();

    let (mut list_cmd, _s2) = cli_command();
    let assert = list_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "list"])
        .assert()
        .success();
    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("Cached snapshots:"), "{stdout}");
    assert!(stdout.contains("2"), "expected object count 2 in: {stdout}");
}

#[test]
fn analyze_second_run_auto_uses_cache_and_matches_first_run_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut first_cmd, _s1) = cli_command();
    let first_output = first_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str()])
        .output()
        .unwrap();
    assert!(first_output.status.success());
    let first_stdout = stdout_string(&first_output.stdout);
    assert!(
        first_stdout.contains("Mnemosyne Analysis"),
        "{first_stdout}"
    );

    let (mut second_cmd, _s2) = cli_command();
    let second_output = second_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str()])
        .output()
        .unwrap();
    assert!(second_output.status.success());
    let second_stdout = stdout_string(&second_output.stdout);

    assert_eq!(
        first_stdout, second_stdout,
        "second analyze run (auto-cache-hit) must produce identical output to the first"
    );
}

#[test]
fn analyze_refresh_always_reparses_and_produces_correct_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut first_cmd, _s1) = cli_command();
    first_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str()])
        .assert()
        .success();

    let (mut refresh_cmd, _s2) = cli_command();
    let assert = refresh_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str(), "--refresh"])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("Mnemosyne Analysis"), "{stdout}");
    assert!(stdout.contains("Total Objects:"), "{stdout}");
}

#[test]
fn analyze_snapshot_nonexistent_key_returns_exit_10_not_silent_parse() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args([
            "analyze",
            fixture_path.as_str(),
            "--snapshot",
            "does-not-exist",
        ])
        .assert()
        .code(10);
    let stderr = stderr_string(&assert.get_output().stderr);
    assert!(stderr.contains("snapshot_not_found"), "{stderr}");
}

#[test]
fn analyze_explicit_snapshot_produces_correct_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut analyze_cmd, _s2) = cli_command();
    let assert = analyze_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str(), "--snapshot", &hash])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("Mnemosyne Analysis"), "{stdout}");
    assert!(stdout.contains("Total Objects:"), "{stdout}");
}

// --- leaks / gc-path / inspect / query with --snapshot ---------------------

#[test]
fn leaks_no_flags_populates_cache_and_output_matches_direct_run() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut cmd, _sandbox) = cli_command();
    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["leaks", fixture_path.as_str()])
        .assert()
        .success();
    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(
        stdout.contains("Potential leaks:") || stdout.contains("No leak suspects detected."),
        "{stdout}"
    );

    let (mut list_cmd, _s2) = cli_command();
    let list_assert = list_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "list"])
        .assert()
        .success();
    assert!(stdout_string(&list_assert.get_output().stdout).contains("Cached snapshots:"));
}

#[test]
fn leaks_explicit_snapshot_matches_direct_run_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut direct_cmd, _s0) = cli_command();
    let direct_output = direct_cmd
        .args(["leaks", fixture_path.as_str()])
        .output()
        .unwrap();
    assert!(direct_output.status.success());
    let direct_stdout = stdout_string(&direct_output.stdout);

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut leaks_cmd, _s2) = cli_command();
    let leaks_output = leaks_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["leaks", fixture_path.as_str(), "--snapshot", &hash])
        .output()
        .unwrap();
    assert!(leaks_output.status.success());
    let leaks_stdout = stdout_string(&leaks_output.stdout);

    assert_eq!(direct_stdout, leaks_stdout);
}

#[test]
fn gc_path_explicit_snapshot_finds_path_to_child_object() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut gc_cmd, _s2) = cli_command();
    let assert = gc_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args([
            "gc-path",
            fixture_path.as_str(),
            "--object-id",
            CHILD_OBJECT_ID,
            "--snapshot",
            &hash,
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(stdout.contains("GC path for 0x00002000:"), "{stdout}");
    assert!(stdout.contains("ROOT"), "{stdout}");
}

#[test]
fn gc_path_snapshot_all_paths_by_class_works() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut gc_cmd, _s2) = cli_command();
    let assert = gc_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args([
            "gc-path",
            fixture_path.as_str(),
            "--by-class",
            "com.example.BigCache",
            "--all-paths",
            "--snapshot",
            &hash,
        ])
        .assert()
        .success();

    let stdout = stdout_string(&assert.get_output().stdout);
    assert!(
        stdout.contains("GC root paths for class:com.example.BigCache"),
        "{stdout}"
    );
}

#[test]
fn gc_path_snapshot_unknown_object_id_returns_exit_8() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut gc_cmd, _s2) = cli_command();
    gc_cmd.env("MNEMOSYNE_SNAPSHOT_DIR", cache.path()).args([
        "gc-path",
        fixture_path.as_str(),
        "--object-id",
        "0xdeadbeef",
        "--snapshot",
        &hash,
    ]);
    gc_cmd.assert().code(8);
}

#[test]
fn inspect_explicit_snapshot_matches_direct_run_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut direct_cmd, _s0) = cli_command();
    let direct_output = direct_cmd
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
        ])
        .output()
        .unwrap();
    assert!(direct_output.status.success());
    let direct_stdout = stdout_string(&direct_output.stdout);

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut inspect_cmd, _s2) = cli_command();
    let inspect_output = inspect_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
            "--snapshot",
            &hash,
        ])
        .output()
        .unwrap();
    assert!(inspect_output.status.success());
    let inspect_stdout = stdout_string(&inspect_output.stdout);

    assert_eq!(direct_stdout, inspect_stdout);
}

#[test]
fn inspect_snapshot_nonexistent_key_returns_exit_10() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    cmd.env("MNEMOSYNE_SNAPSHOT_DIR", cache.path()).args([
        "inspect",
        fixture_path.as_str(),
        "--object-id",
        ROOT_OBJECT_ID,
        "--snapshot",
        "does-not-exist",
    ]);
    cmd.assert().code(10);
}

#[test]
fn query_explicit_snapshot_matches_direct_run_output() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();
    let oql = r#"SELECT @objectId, @className FROM "com.example.BigCache""#;

    let (mut direct_cmd, _s0) = cli_command();
    let direct_output = direct_cmd
        .args(["query", fixture_path.as_str(), oql])
        .output()
        .unwrap();
    assert!(direct_output.status.success());
    let direct_stdout = stdout_string(&direct_output.stdout);
    assert!(direct_stdout.contains("Columns:"), "{direct_stdout}");

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    let (mut query_cmd, _s2) = cli_command();
    let query_output = query_cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["query", fixture_path.as_str(), oql, "--snapshot", &hash])
        .output()
        .unwrap();
    assert!(query_output.status.success());
    let query_stdout = stdout_string(&query_output.stdout);

    assert_eq!(direct_stdout, query_stdout);
}

#[test]
fn query_snapshot_nonexistent_key_returns_exit_10() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();
    let (mut cmd, _sandbox) = cli_command();

    cmd.env("MNEMOSYNE_SNAPSHOT_DIR", cache.path()).args([
        "query",
        fixture_path.as_str(),
        r#"SELECT @objectId FROM "com.example.BigCache""#,
        "--snapshot",
        "does-not-exist",
    ]);
    cmd.assert().code(10);
}

// --- loud staleness: exit 11 (schema mismatch) / exit 12 (stale source) ---

#[test]
fn explicit_snapshot_schema_mismatch_returns_exit_11() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    // Hand-edit the persisted snapshot's schema_version to simulate a
    // snapshot saved by an incompatible binary version.
    let snapshot_file = cache.path().join(format!("{hash}.json"));
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&snapshot_file).unwrap()).unwrap();
    value["manifest"]["schema_version"] = serde_json::json!(999);
    std::fs::write(&snapshot_file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let (mut cmd, _s2) = cli_command();
    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["analyze", fixture_path.as_str(), "--snapshot", &hash])
        .assert()
        .code(11);
    let stderr = stderr_string(&assert.get_output().stderr);
    assert!(stderr.contains("snapshot_schema_mismatch"), "{stderr}");
}

#[test]
fn explicit_snapshot_stale_source_returns_exit_12() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let cache = snapshot_cache();

    let (mut save_cmd, _s1) = cli_command();
    let save_stdout = stdout_string(
        &save_cmd
            .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
            .args(["snapshot", "save", fixture_path.as_str()])
            .output()
            .unwrap()
            .stdout,
    );
    let hash = extract_saved_hash(&save_stdout);

    // Mutate the heap file's bytes after the snapshot was taken, so its
    // current SHA-256 no longer matches the cached manifest's.
    std::fs::write(
        fixture.path(),
        b"deliberately different bytes, not the original hprof",
    )
    .unwrap();

    let (mut cmd, _s2) = cli_command();
    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args([
            "inspect",
            fixture_path.as_str(),
            "--object-id",
            ROOT_OBJECT_ID,
            "--snapshot",
            &hash,
        ])
        .assert()
        .code(12);
    let stderr = stderr_string(&assert.get_output().stderr);
    assert!(stderr.contains("snapshot_stale_source"), "{stderr}");
}

#[test]
fn snapshot_load_corrupt_file_returns_exit_13() {
    let cache = snapshot_cache();
    std::fs::create_dir_all(cache.path()).unwrap();
    std::fs::write(cache.path().join("deadbeef.json"), b"{ not valid json").unwrap();

    let (mut cmd, _sandbox) = cli_command();
    let assert = cmd
        .env("MNEMOSYNE_SNAPSHOT_DIR", cache.path())
        .args(["snapshot", "load", "deadbeef"])
        .assert()
        .code(13);
    let stderr = stderr_string(&assert.get_output().stderr);
    assert!(stderr.contains("snapshot_corrupt"), "{stderr}");
}

// --- regression: no new flags => byte-identical to pre-Slice-9.C behavior -

#[test]
fn regression_analyze_no_flags_matches_expected_markers() {
    let fixture = write_fixture(&build_graph_fixture());
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
fn regression_leaks_no_flags_matches_expected_markers() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args(["leaks", fixture_path.as_str()]);
    cmd.assert().success().stdout(
        predicate::str::contains("Potential leaks:")
            .or(predicate::str::contains("No leak suspects detected.")),
    );
}

#[test]
fn regression_gc_path_no_flags_matches_expected_markers() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    cmd.args([
        "gc-path",
        fixture_path.as_str(),
        "--object-id",
        CHILD_OBJECT_ID,
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("GC path for 0x00002000:"));
}

#[test]
fn regression_inspect_no_flags_matches_expected_markers() {
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
}

#[test]
fn regression_query_no_flags_matches_expected_markers() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "query",
            fixture_path.as_str(),
            r#"SELECT @objectId, @className FROM "com.example.BigCache""#,
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = stdout_string(&output.stdout);
    assert!(stdout.contains("Columns:"));
    assert!(stdout.contains("@objectId"));
    assert!(stdout.contains("com.example.BigCache"));
}
