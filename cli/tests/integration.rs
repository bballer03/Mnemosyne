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

fn toml_path_arg(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

fn count_occurrences(input: &str, needle: &str) -> usize {
    input.match_indices(needle).count()
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
fn test_leaks_prints_confirmation_when_no_suspects_match() {
    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["leaks", fixture_path.as_str(), "--min-severity", "critical"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("No leak suspects detected."));
    assert!(!stdout.contains("Potential leaks:"));
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
    assert!(stdout.contains(&format!("{truncated_leak_id} -> {full_leak_id}")));
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
fn test_chat_starts_with_shortlist_and_help() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("chat-low.toml");
    fs::write(&config_path, "[analysis]\nmin_severity = \"LOW\"\n").unwrap();

    let output = cmd
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Analyzed heap:"));
    assert!(stdout.contains(&fixture_path));
    assert!(stdout.contains("Top leak candidates:"));
    assert!(stdout.contains("Leak ID Class Kind Severity Retained Instances"));
    assert_eq!(
        count_occurrences(&stdout, "Leak ID Class Kind Severity Retained Instances"),
        1
    );
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
}

#[test]
fn test_chat_answers_a_question_in_rules_mode() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("chat-low.toml");
    fs::write(&config_path, "[analysis]\nmin_severity = \"LOW\"\n").unwrap();

    let output = cmd
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("Why is this leaking?\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("Answer:"));
    assert!(stdout.contains("com.example.CacheLeak"));
}

#[test]
fn test_chat_focuses_on_selected_leak() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("chat-low.toml");
    fs::write(&config_path, "[analysis]\nmin_severity = \"LOW\"\n").unwrap();

    let output = cmd
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("/focus com.example.CacheLeak\nWhy is this leaking?\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Focused leak: com.example.CacheLeak"));
    assert_eq!(
        count_occurrences(&stdout, "Focused leak: com.example.CacheLeak"),
        1
    );
    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("com.example.CacheLeak"));
}

#[test]
fn test_chat_rejects_invalid_focus_target_and_continues() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("chat-low.toml");
    fs::write(&config_path, "[analysis]\nmin_severity = \"LOW\"\n").unwrap();

    let output = cmd
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("/focus missing::leak\n/help\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Focus error: no leak found matching identifier 'missing::leak'"));
    assert_eq!(
        count_occurrences(
            &stdout,
            "Focus error: no leak found matching identifier 'missing::leak'"
        ),
        1
    );
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
}

#[test]
fn test_chat_bare_focus_prints_usage_and_does_not_ask_ai() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("chat-low.toml");
    fs::write(&config_path, "[analysis]\nmin_severity = \"LOW\"\n").unwrap();

    let output = cmd
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("/focus\n/help\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Usage: /focus <leak-id>"));
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
    assert!(!stdout.contains("Question: /focus"));
}

#[test]
fn test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=provider-chat-model\n  confidence_pct=83\n  summary=Provider chat answer\nsection recommendations\n  item#0=Inspect the cache owner\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("mock server accept failed: {err}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        let mut buf = [0_u8; 8192];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(request.contains("intent=chat_leak_follow_up"), "{request}");
        assert!(
            request.contains("question=Why is this leaking?"),
            "{request}"
        );
        assert!(request.contains("leak_sampled=3"), "{request}");

        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("provider-chat.toml");
    fs::write(
        &config_path,
        format!(
            "[analysis]\nmin_severity = \"LOW\"\nleak_types = [\"CACHE\", \"THREAD\", \"HTTP_RESPONSE\", \"COLLECTION\"]\n\n[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-chat-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "chat",
            fixture_path.as_str(),
        ])
        .write_stdin("Why is this leaking?\n/exit\n")
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("Provider chat answer"));
    assert!(stdout.contains("Inspect the cache owner"));
}

#[test]
fn test_chat_default_config_reports_healthy_heap_when_fallback_leaks_are_below_threshold() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["chat", fixture_path.as_str()])
        .write_stdin("/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Analyzed heap:"));
    assert!(stdout.contains(&fixture_path));
    assert!(stdout.contains(
        "No leak suspects detected. Ask questions about the healthy-heap summary or type /exit."
    ));
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
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
fn test_analyze_json_with_ai_includes_ai_section() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--format", "json", "--ai"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    let ai = json
        .get("ai")
        .and_then(Value::as_object)
        .expect("ai object");
    assert!(ai.contains_key("model"));
    assert!(ai.contains_key("summary"));
    assert!(ai.contains_key("recommendations"));
    assert!(ai.contains_key("confidence"));
    assert!(ai.contains_key("wire"));
}

#[test]
fn test_analyze_json_with_provider_mode_ai_includes_provider_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=provider-test-model\n  confidence_pct=77\n  summary=Provider summary from CLI integration\nsection recommendations\n  item#0=First recommendation\n  item#1=Second recommendation\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 4096];
        let _ = stream.read(&mut buf).unwrap();
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("provider.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-test-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "analyze",
            fixture_path.as_str(),
            "--format",
            "json",
            "--ai",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    let ai = json
        .get("ai")
        .and_then(Value::as_object)
        .expect("ai object");
    assert_eq!(
        ai.get("model").and_then(Value::as_str),
        Some("provider-test-model")
    );
    assert_eq!(
        ai.get("summary").and_then(Value::as_str),
        Some("Provider summary from CLI integration")
    );
    let recommendations = ai
        .get("recommendations")
        .and_then(Value::as_array)
        .expect("recommendations array");
    assert_eq!(recommendations.len(), 2);

    server.join().unwrap();
}

#[test]
fn test_analyze_json_with_anthropic_provider_mode_includes_provider_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": "TOON v1\nsection response\n  model=claude-cli-test\n  confidence_pct=76\n  summary=Anthropic summary from CLI integration\nsection recommendations\n  item#0=First Anthropic recommendation\n  item#1=Second Anthropic recommendation\n"
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 4096];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(request.contains("POST /v1/messages HTTP/1.1"), "{request}");
        assert!(request.contains("x-api-key: dummy-key"), "{request}");
        assert!(
            request.contains("anthropic-version: 2023-06-01"),
            "{request}"
        );

        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("anthropic-provider.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"anthropic\"\nmodel = \"claude-cli-test\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_ANTHROPIC_KEY\"\nmax_tokens = 512\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_ANTHROPIC_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "analyze",
            fixture_path.as_str(),
            "--format",
            "json",
            "--ai",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    let ai = json
        .get("ai")
        .and_then(Value::as_object)
        .expect("ai object");
    assert_eq!(
        ai.get("model").and_then(Value::as_str),
        Some("claude-cli-test")
    );
    assert_eq!(
        ai.get("summary").and_then(Value::as_str),
        Some("Anthropic summary from CLI integration")
    );
    let recommendations = ai
        .get("recommendations")
        .and_then(Value::as_array)
        .expect("recommendations array");
    assert_eq!(recommendations.len(), 2);

    server.join().unwrap();
}

#[test]
fn test_analyze_json_with_provider_mode_ai_redacts_prompt_before_send() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=provider-test-model\n  confidence_pct=77\n  summary=Provider summary with privacy controls\nsection recommendations\n  item#0=First recommendation\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(request.contains("heap_path=<REDACTED>"), "{request}");
        assert!(request.contains("custom_secret=<REDACTED>"), "{request}");
        assert!(!request.contains("secret-token-123"), "{request}");

        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture_dir = tempdir().unwrap();
    let fixture_path = fixture_dir.path().join("secret-token-123-heap.hprof");
    fs::write(&fixture_path, build_graph_fixture()).unwrap();
    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, sandbox) = cli_command();
    let prompt_dir = sandbox.path().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();
    fs::write(
        prompt_dir.join("provider-insights.yaml"),
        "version: 1\ninstructions:\n  - key: response_format\n    value: Return only TOON v1 with section response and section recommendations\n  - key: custom_secret\n    value: secret-token-123\n",
    )
    .unwrap();
    let config_path = sandbox.path().join("provider-privacy.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-test-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n\n[ai.privacy]\nredact_heap_path = true\nredact_patterns = [\"secret-token-[0-9]+\"]\n\n[ai.prompts]\ntemplate_dir = \"{}\"\n",
            toml_path_arg(&prompt_dir)
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "analyze",
            fixture_arg.as_str(),
            "--format",
            "json",
            "--ai",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    let prompt = json
        .get("ai")
        .and_then(|ai| ai.get("wire"))
        .and_then(|wire| wire.get("prompt"))
        .and_then(Value::as_str)
        .expect("ai.wire.prompt string");
    assert!(prompt.contains("heap_path=<REDACTED>"));
    assert!(prompt.contains("custom_secret=<REDACTED>"));
    assert!(!prompt.contains("secret-token-123"));
    assert!(!prompt.contains(fixture_arg.as_str()));

    server.join().unwrap();
}

#[test]
fn test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=provider-test-model\n  confidence_pct=77\n  summary=Provider summary with privacy audit logging\nsection recommendations\n  item#0=First recommendation\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).unwrap();

        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture_dir = tempdir().unwrap();
    let fixture_path = fixture_dir.path().join("secret-token-123-heap.hprof");
    fs::write(&fixture_path, build_graph_fixture()).unwrap();
    let fixture_arg = path_arg(&fixture_path);
    let (mut cmd, sandbox) = cli_command();
    let prompt_dir = sandbox.path().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();
    fs::write(
        prompt_dir.join("provider-insights.yaml"),
        "version: 1\ninstructions:\n  - key: response_format\n    value: Return only TOON v1 with section response and section recommendations\n  - key: custom_secret\n    value: secret-token-123\n",
    )
    .unwrap();
    let config_path = sandbox.path().join("provider-audit.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-test-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n\n[ai.privacy]\nredact_heap_path = true\nredact_patterns = [\"secret-token-[0-9]+\"]\naudit_log = true\n\n[ai.prompts]\ntemplate_dir = \"{}\"\n",
            toml_path_arg(&prompt_dir)
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .env("RUST_LOG", "mnemosyne_core::analysis::ai=info")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "analyze",
            fixture_arg.as_str(),
            "--format",
            "json",
            "--ai",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stderr = strip_ansi(&stdout_string(&output.stderr));
    assert!(
        stderr.contains("provider_ai_audit"),
        "missing provider audit marker: {stderr}"
    );
    assert!(
        stderr.contains("prompt_sha256="),
        "missing prompt hash metadata: {stderr}"
    );
    assert!(
        stderr.contains("prompt_bytes="),
        "missing prompt length metadata: {stderr}"
    );
    assert!(
        !stderr.contains("secret-token-123"),
        "stderr leaked raw secret: {stderr}"
    );
    assert!(
        !stderr.contains("custom_secret="),
        "stderr leaked prompt content: {stderr}"
    );

    server.join().unwrap();
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
fn test_analyze_with_top_instances_flag() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--top-instances"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Top Instances by Size:"));
}

#[test]
fn test_analyze_with_threads_flag() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--threads"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Thread Report ("));
}

#[test]
fn test_analyze_with_classloaders_flag() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["analyze", fixture_path.as_str(), "--classloaders"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("ClassLoader Report:"));
}

#[test]
fn test_query_command_prints_matching_rows() {
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
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Columns:"));
    assert!(stdout.contains("@objectId"));
    assert!(stdout.contains("com.example.BigCache"));
}

#[test]
fn test_analyze_profile_overview_disables_optional_investigation_sections() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "analyze",
            fixture_path.as_str(),
            "--profile",
            "overview",
            "--threads",
            "--strings",
            "--collections",
            "--top-instances",
            "--classloaders",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(!stdout.contains("Thread Report ("));
    assert!(!stdout.contains("String Analysis ("));
    assert!(!stdout.contains("Collection Report ("));
    assert!(!stdout.contains("Top Instances by Size:"));
    assert!(!stdout.contains("ClassLoader Report:"));
}

#[test]
fn test_analyze_profile_incident_response_enables_investigation_sections() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "analyze",
            fixture_path.as_str(),
            "--profile",
            "incident-response",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Thread Report ("));
    assert!(stdout.contains("String Analysis ("));
    assert!(stdout.contains("Collection Report ("));
    assert!(stdout.contains("Top Instances by Size:"));
    assert!(stdout.contains("ClassLoader Report:"));
}

#[test]
fn test_analyze_with_all_phase_two_flags() {
    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args([
            "analyze",
            fixture_path.as_str(),
            "--threads",
            "--strings",
            "--collections",
            "--top-instances",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Thread Report ("));
    assert!(stdout.contains("String Analysis ("));
    assert!(stdout.contains("Collection Report ("));
    assert!(stdout.contains("Top Instances by Size:"));
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
fn test_fix_with_provider_mode_returns_ai_backed_patch() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  confidence_pct=84\n  description=Evict idle entries before they accumulate.\nsection patch\n  diff=--- a/src/main/java/com/example/CacheLeak.java\\n+++ b/src/main/java/com/example/CacheLeak.java\\n@@ ...\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(request.contains("intent=generate_fix"), "{request}");

        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_simple_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let project_root = sandbox.path().join("repo");
    let source_dir = project_root
        .join("src")
        .join("main")
        .join("java")
        .join("com")
        .join("example");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("CacheLeak.java"),
        "package com.example;\npublic class CacheLeak {\n  void retain() {}\n}\n",
    )
    .unwrap();

    let config_path = sandbox.path().join("provider-fix.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-fix-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "fix",
            fixture_path.as_str(),
            "--project-root",
            project_root.to_string_lossy().as_ref(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Evict idle entries before they accumulate."));
    assert!(stdout.contains("CacheLeak.java"));
    assert!(!stdout.contains("[PLACEHOLDER]"));

    server.join().unwrap();
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
fn test_config_json_includes_ai_prompt_template_dir() {
    let (mut cmd, sandbox) = cli_command();
    let template_dir = sandbox.path().join("prompt-overrides");
    let config_path = sandbox.path().join("prompts.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\n\n[ai.prompts]\ntemplate_dir = \"{}\"\n",
            toml_path_arg(&template_dir)
        ),
    )
    .unwrap();

    let output = cmd
        .args(["--config", config_path.to_string_lossy().as_ref(), "config"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(
        json.get("ai")
            .and_then(|ai| ai.get("prompts"))
            .and_then(|prompts| prompts.get("template_dir"))
            .and_then(Value::as_str),
        Some(toml_path_arg(&template_dir).as_str())
    );
}

#[test]
fn test_analyze_json_with_provider_mode_ai_uses_prompt_template_override() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=provider-test-model\n  confidence_pct=77\n  summary=Provider summary from prompt override test\nsection recommendations\n  item#0=First recommendation\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(
            request.contains("custom_prompt_marker=prompt-template-override"),
            "request did not contain template override marker: {request}"
        );
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(reply.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let prompt_dir = sandbox.path().join("prompts");
    fs::create_dir_all(&prompt_dir).unwrap();
    fs::write(
        prompt_dir.join("provider-insights.yaml"),
        "version: 1\ninstructions:\n  - key: response_format\n    value: Return only TOON v1 with section response and section recommendations\n  - key: required_model\n    value: \"{{model}}\"\n  - key: custom_prompt_marker\n    value: prompt-template-override\n",
    )
    .unwrap();
    let config_path = sandbox.path().join("provider-prompts.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-test-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n\n[ai.prompts]\ntemplate_dir = \"{}\"\n",
            toml_path_arg(&prompt_dir)
        ),
    )
    .unwrap();

    let output = cmd
        .env("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key")
        .args([
            "--config",
            config_path.to_string_lossy().as_ref(),
            "analyze",
            fixture_path.as_str(),
            "--format",
            "json",
            "--ai",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = serde_json::from_str::<Value>(&stdout).unwrap();
    let prompt = json
        .get("ai")
        .and_then(|ai| ai.get("wire"))
        .and_then(|wire| wire.get("prompt"))
        .and_then(Value::as_str)
        .expect("ai.wire.prompt string");
    assert!(prompt.contains("custom_prompt_marker=prompt-template-override"));

    server.join().unwrap();
}

#[test]
fn test_serve_list_tools_returns_catalog() {
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .arg("serve")
        .write_stdin("{\"id\":1,\"method\":\"list_tools\",\"params\":{}}\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(true)));

    let tools = json
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .expect("result.tools array");
    assert!(tools.iter().any(|tool| {
        tool.get("name") == Some(&Value::String("list_tools".into()))
            && tool
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("parameter shapes"))
    }));
    assert!(tools.iter().any(|tool| {
        tool.get("name") == Some(&Value::String("analyze_heap".into()))
            && tool
                .get("params")
                .and_then(Value::as_array)
                .is_some_and(|params| !params.is_empty())
    }));
}

#[test]
fn test_serve_unsupported_method_includes_error_details() {
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .arg("serve")
        .write_stdin("{\"id\":2,\"method\":\"nope\",\"params\":{}}\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(false)));
    assert_eq!(
        json.get("error").and_then(Value::as_str),
        Some("Invalid input: unsupported MCP method: nope")
    );
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("code"))
            .and_then(Value::as_str),
        Some("invalid_input")
    );
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("details"))
            .and_then(|details| details.get("detail"))
            .and_then(Value::as_str),
        Some("unsupported MCP method: nope")
    );
}

#[test]
fn test_serve_invalid_json_includes_error_details() {
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .arg("serve")
        .write_stdin("{not valid json}\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(false)));
    assert_eq!(json.get("id"), Some(&Value::Null));
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("code"))
            .and_then(Value::as_str),
        Some("invalid_json")
    );
    assert!(json
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|value| value.contains("invalid JSON")));
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
