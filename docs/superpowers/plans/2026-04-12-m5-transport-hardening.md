# M5 Transport Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify the current stdio MCP transport under AI-backed delay and payload stress, distinguish provider failures from provider timeouts in MCP error codes, and document that the current one-request/one-response model remains acceptable unless tests prove otherwise.

**Architecture:** Keep `core/src/mcp/server.rs` as the single stdio transport boundary and use black-box `mnemosyne serve` integration tests as the source of truth for transport behavior. Add only the smallest error-classification changes needed in `core/src/llm.rs`, `core/src/errors.rs`, and `core/src/mcp/server.rs`, while preserving the existing final `RpcResponse` envelope and the stable `AiInsights` / `FixResponse` payload shapes.

**Tech Stack:** Rust, Tokio stdio MCP server, reqwest blocking client, assert_cmd CLI integration tests, serde_json, local TCP test servers

---

### Task 1: Add Black-Box Serve Evidence Tests

**Files:**
- Modify: `cli/tests/integration.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Write delayed-success and large-payload evidence tests**

```rust
#[test]
fn test_serve_explain_leak_delayed_provider_response_returns_single_json_line() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "TOON v1\nsection response\n  model=transport-test\n  confidence_pct=72\n  summary=Delayed provider response\nsection recommendations\n  item#0=First delayed recommendation\n"
                }
            }
        ]
    })
    .to_string();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).unwrap();
        thread::sleep(Duration::from_millis(250));
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
    let config_path = sandbox.path().join("serve-provider-delay.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"transport-test\"\nendpoint = \"http://{addr}/v1\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();
    let request = serde_json::json!({
        "id": 21,
        "method": "explain_leak",
        "params": { "heap_path": fixture_path }
    })
    .to_string()
        + "\n";

    let output = cmd
        .args(["--config", config_path.to_string_lossy().as_ref(), "serve"])
        .write_stdin(request)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    assert_eq!(stdout.lines().filter(|line| !line.trim().is_empty()).count(), 1);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(true)));
    assert_eq!(json.get("id"), Some(&serde_json::json!(21)));
    assert_eq!(
        json.get("result")
            .and_then(|value| value.get("summary"))
            .and_then(Value::as_str),
        Some("Delayed provider response")
    );

    server.join().unwrap();
}

#[test]
fn test_serve_explain_leak_large_provider_payload_remains_single_json_line() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let large_summary = "transport-payload".repeat(256);
    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": format!(
                        "TOON v1\nsection response\n  model=transport-test\n  confidence_pct=74\n  summary={}\nsection recommendations\n  item#0={}\n",
                        large_summary,
                        large_summary,
                    )
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

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("serve-provider-large.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"transport-test\"\nendpoint = \"http://{addr}/v1\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();
    let request = serde_json::json!({
        "id": 22,
        "method": "explain_leak",
        "params": { "heap_path": fixture_path }
    })
    .to_string()
        + "\n";

    let output = cmd
        .args(["--config", config_path.to_string_lossy().as_ref(), "serve"])
        .write_stdin(request)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    assert_eq!(stdout.lines().filter(|line| !line.trim().is_empty()).count(), 1);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(true)));
    assert_eq!(json.get("id"), Some(&serde_json::json!(22)));
    assert!(json
        .get("result")
        .and_then(|value| value.get("summary"))
        .and_then(Value::as_str)
        .is_some_and(|summary| summary.contains("transport-payload")));

    server.join().unwrap();
}
```

- [ ] **Step 2: Run the evidence tests**

Run: `cargo test -p mnemosyne-cli --test integration test_serve_explain_leak_ -- --nocapture`
Expected: PASS, proving the current one-response stdio transport already survives delayed AI calls and larger payloads.

### Task 2: Add Failing Transport-Error Classification Tests

**Files:**
- Modify: `cli/tests/integration.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Write failing serve tests for provider HTTP errors and provider timeouts**

```rust
#[test]
fn test_serve_explain_leak_provider_http_error_has_provider_error_code() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .unwrap();
        stream.flush().unwrap();
    });

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("serve-provider-http-error.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"transport-test\"\nendpoint = \"http://{addr}/v1\"\ntimeout_secs = 2\n"
        ),
    )
    .unwrap();
    let request = serde_json::json!({
        "id": 23,
        "method": "explain_leak",
        "params": { "heap_path": fixture_path }
    })
    .to_string()
        + "\n";

    let output = cmd
        .args(["--config", config_path.to_string_lossy().as_ref(), "serve"])
        .write_stdin(request)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(false)));
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("code"))
            .and_then(Value::as_str),
        Some("provider_error")
    );
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("details"))
            .and_then(|details| details.get("status"))
            .and_then(Value::as_u64),
        Some(500)
    );

    server.join().unwrap();
}

#[test]
fn test_serve_explain_leak_provider_timeout_has_provider_timeout_code() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let _ = stream.read(&mut buf).unwrap();
        thread::sleep(Duration::from_secs(3));
    });

    let fixture = write_fixture(&build_graph_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, sandbox) = cli_command();
    let config_path = sandbox.path().join("serve-provider-timeout.toml");
    fs::write(
        &config_path,
        format!(
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"transport-test\"\nendpoint = \"http://{addr}/v1\"\ntimeout_secs = 1\n"
        ),
    )
    .unwrap();
    let request = serde_json::json!({
        "id": 24,
        "method": "explain_leak",
        "params": { "heap_path": fixture_path }
    })
    .to_string()
        + "\n";

    let output = cmd
        .args(["--config", config_path.to_string_lossy().as_ref(), "serve"])
        .write_stdin(request)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = stdout_string(&output.stdout);
    let json = parse_first_json_value(&stdout);
    assert_eq!(json.get("success"), Some(&Value::Bool(false)));
    assert_eq!(
        json.get("error_details")
            .and_then(|details| details.get("code"))
            .and_then(Value::as_str),
        Some("provider_timeout")
    );

    server.join().unwrap();
}
```

- [ ] **Step 2: Run the failing tests to verify the current gap**

Run: `cargo test -p mnemosyne-cli --test integration test_serve_explain_leak_provider_ -- --nocapture`
Expected: FAIL because the current transport maps provider HTTP failures and provider timeouts to generic `internal_error` behavior.

### Task 3: Implement Provider Error Classification

**Files:**
- Modify: `core/src/errors.rs`
- Modify: `core/src/llm.rs`
- Modify: `core/src/mcp/server.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Add explicit core error variants for provider failure and timeout**

```rust
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("AI provider request failed: {detail}")]
    AiProviderError {
        detail: String,
        status: Option<u16>,
    },

    #[error("AI provider request timed out: {detail}")]
    AiProviderTimeout { detail: String },

    // existing variants...
}
```

- [ ] **Step 2: Map reqwest failures into the new core errors**

```rust
fn map_provider_error(err: reqwest::Error) -> CoreError {
    if err.is_timeout() {
        return CoreError::AiProviderTimeout {
            detail: err.to_string(),
        };
    }

    CoreError::AiProviderError {
        status: err.status().map(|status| status.as_u16()),
        detail: err.to_string(),
    }
}

let response = builder.send().map_err(map_provider_error)?;
let response = response.error_for_status().map_err(map_provider_error)?;
let payload: OpenAiChatResponse = response.json().map_err(map_provider_error)?;
```

- [ ] **Step 3: Expose provider-specific MCP `error_details.code` values**

```rust
CoreError::AiProviderError { detail, status } => Self {
    code: "provider_error",
    message,
    details: Some(json!({
        "detail": detail,
        "status": status,
    })),
},
CoreError::AiProviderTimeout { detail } => Self {
    code: "provider_timeout",
    message,
    details: Some(json!({ "detail": detail })),
},
```

- [ ] **Step 4: Re-run the transport tests**

Run: `cargo test -p mnemosyne-cli --test integration test_serve_explain_leak_ -- --nocapture`
Expected: PASS, including the previously failing provider HTTP error and provider timeout tests.

### Task 4: Document the Verified Transport Outcome

**Files:**
- Modify: `docs/api.md`
- Modify: `STATUS.md`
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`

- [ ] **Step 1: Document the verified request/response transport model in `docs/api.md`**

```markdown
## Transport Notes

Mnemosyne's MCP stdio server currently uses a one-request/one-response JSON-line transport for all methods, including AI-backed calls such as `explain_leak`, `chat_session`, and `propose_fix`.

Streaming/progress events are not part of the live contract in this branch because the current transport has dedicated coverage for delayed AI-backed responses and larger single-response payloads.

Provider-backed failures expose machine-readable MCP error codes:

- `provider_error` for upstream provider HTTP or decode failures
- `provider_timeout` for provider timeout failures
```

- [ ] **Step 2: Update M5 status notes to reflect the evidence-first outcome**

```markdown
- ✅ MCP transport hardening now has direct `serve` coverage for delayed AI-backed responses and larger payloads, and provider failures/timeouts surface dedicated machine-readable MCP error codes. The current stdio request/response transport remains sufficient; streaming is not required by the verified contract in this branch.
```

- [ ] **Step 3: Run formatting and the full verification set**

Run: `cargo test && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS for the full workspace.
