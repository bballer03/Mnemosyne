# AI CLI Chat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a CLI-only, leak-focused `mnemosyne-cli chat <heap.hprof>` command that analyzes once, shows the top 3 leak candidates, and supports interactive follow-up questions in a bounded in-process session.

**Architecture:** Keep chat orchestration in `cli/src/main.rs` and reuse the existing analysis pipeline for the one-time startup scan. Extend `core/src/analysis/ai.rs` with a chat-turn entry point that still returns `AiInsights`, uses the same rules/stub/provider mode dispatch, and keeps provider-mode TOON/privacy/audit behavior aligned with the existing explanation path.

**Tech Stack:** Rust, `clap`, `anyhow`, `assert_cmd`, existing CLI integration harness, existing `AiInsights` / TOON prompt helpers, current provider-mode mock-server tests

**Git note:** Do not create commits while executing this plan unless the user explicitly asks for one.

---

## File Map

- `cli/src/main.rs`
  - add `Chat` command wiring
  - own the REPL loop, focus switching, help output, and bounded in-memory session state
- `core/src/analysis/ai.rs`
  - add chat-turn request/history types
  - add chat-specific rules/stub/provider dispatch helpers
  - add TOON prompt builders for chat turns while preserving `AiInsights` / `AiWireExchange`
- `cli/tests/integration.rs`
  - add stdin-driven chat command integration coverage
  - add provider-mode chat integration coverage with a local mock server
- `README.md`
  - document the new `chat` command and its CLI-only first-slice scope
- `ARCHITECTURE.md`
  - mention `chat` in the shipped CLI surface and current AI snapshot
- `STATUS.md`
  - move conversation mode from pending to partial / CLI-first shipped
- `docs/roadmap.md`
  - update Step `14(e)` status language to reflect the CLI-first slice
- `docs/design/milestone-5-ai-mcp-differentiation.md`
  - record that conversation mode started as a CLI slice and MCP sessions remain future work
- `OVERNIGHT_SUMMARY.md`
  - record the batch, verification, and next follow-on work

### Task 1: Add Red CLI Integration Tests For The New `chat` Command

**Files:**
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Write the failing startup/help integration test**

Add this test near the other CLI command integration cases:

```rust
#[test]
fn test_chat_starts_with_shortlist_and_help() {
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

    assert!(stdout.contains("Top leak candidates:"));
    assert!(stdout.contains("Leak ID Class Kind Severity Retained Instances"));
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
}
```

- [ ] **Step 2: Write the failing rules-mode question/answer integration test**

Add this test immediately after the startup test:

```rust
#[test]
fn test_chat_answers_a_question_in_rules_mode() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["chat", fixture_path.as_str()])
        .write_stdin("Why is this leaking?\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("Answer:"));
    assert!(stdout.contains("com.example.CacheLeak"));
}
```

- [ ] **Step 3: Write the failing valid-focus integration test**

Use the known fallback leak class name as the focus target so the test stays deterministic:

```rust
#[test]
fn test_chat_focuses_on_selected_leak() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["chat", fixture_path.as_str()])
        .write_stdin("/focus com.example.CacheLeak\nWhy is this leaking?\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Focused leak: com.example.CacheLeak"));
    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("com.example.CacheLeak"));
}
```

- [ ] **Step 4: Write the failing invalid-focus integration test**

The chat session should report the validation error and continue running until `/exit`:

```rust
#[test]
fn test_chat_rejects_invalid_focus_target_and_continues() {
    let fixture = write_fixture(&build_fallback_leak_fixture());
    let fixture_path = path_arg(fixture.path());
    let (mut cmd, _sandbox) = cli_command();

    let output = cmd
        .args(["chat", fixture_path.as_str()])
        .write_stdin("/focus missing::leak\n/help\n/exit\n")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);

    assert!(stdout.contains("Focus error: no leak found matching identifier 'missing::leak'"));
    assert!(stdout.contains("Commands: /focus <leak-id>, /list, /help, /exit"));
}
```

- [ ] **Step 5: Write the failing provider-mode chat integration test**

Add one provider-mode chat regression so the new command proves it still uses the shared provider path:

```rust
#[test]
fn test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
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
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0_u8; 8192];
        let read = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();
        assert!(request.contains("intent=chat_leak_follow_up"), "{request}");
        assert!(request.contains("question=Why is this leaking?"), "{request}");

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
            "[ai]\nenabled = true\nmode = \"provider\"\nprovider = \"local\"\nmodel = \"provider-chat-model\"\nendpoint = \"http://{addr}/v1\"\napi_key_env = \"MNEMOSYNE_TEST_LOCAL_KEY\"\ntimeout_secs = 2\n"
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

    assert!(output.status.success(), "{}", stdout_string(&output.stderr));
    let stdout = normalized_stdout(&output.stdout);
    assert!(stdout.contains("Question: Why is this leaking?"));
    assert!(stdout.contains("Provider chat answer"));
    assert!(stdout.contains("Inspect the cache owner"));

    server.join().unwrap();
}
```

- [ ] **Step 6: Run the focused chat integration tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_chat_starts_with_shortlist_and_help -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_answers_a_question_in_rules_mode -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_focuses_on_selected_leak -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_rejects_invalid_focus_target_and_continues -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response -- --exact --nocapture
```

Expected: FAIL because the `chat` subcommand does not exist yet.

### Task 2: Add Red Core AI Unit Tests For Chat Prompt Semantics

**Files:**
- Modify: `core/src/analysis/ai.rs`

- [ ] **Step 1: Add small reusable chat test fixtures inside `core::analysis::ai::tests`**

Add these helpers near the top of the existing `#[cfg(test)] mod tests` block:

```rust
fn sample_chat_summary() -> HeapSummary {
    HeapSummary {
        heap_path: "heap.hprof".into(),
        total_objects: 128,
        total_size_bytes: 512 * 1024 * 1024,
        classes: Vec::new(),
        generated_at: SystemTime::now(),
        header: None,
        total_records: 0,
        record_stats: Vec::new(),
    }
}

fn sample_chat_leak() -> LeakInsight {
    LeakInsight {
        id: "com.example.CacheLeak::deadbeef".into(),
        class_name: "com.example.CacheLeak".into(),
        leak_kind: LeakKind::Cache,
        severity: LeakSeverity::High,
        retained_size_bytes: 256 * 1024 * 1024,
        shallow_size_bytes: None,
        suspect_score: None,
        instances: 42,
        description: "Cache entries stay reachable from a singleton owner.".into(),
        provenance: Vec::new(),
    }
}
```

- [ ] **Step 2: Add the failing provider-chat prompt test for question/focus/history**

Add this test in the same module:

```rust
#[test]
fn provider_chat_prompt_includes_question_focus_and_recent_history() {
    let summary = sample_chat_summary();
    let leak = sample_chat_leak();
    let config = AiConfig {
        mode: AiMode::Provider,
        ..AiConfig::default()
    };
    let history = vec![AiChatTurn {
        question: "What should I inspect first?".into(),
        answer_summary: "Start with the singleton cache owner.".into(),
    }];

    let prompt = build_provider_chat_toon_prompt(
        &summary,
        &[leak],
        "Why is this leaking?",
        &history,
        Some("com.example.CacheLeak"),
        &config,
    )
    .unwrap();

    assert!(prompt.contains("intent=chat_leak_follow_up"));
    assert!(prompt.contains("active_leak_id=com.example.CacheLeak"));
    assert!(prompt.contains("question=Why is this leaking?"));
    assert!(prompt.contains("section conversation"));
    assert!(prompt.contains("What should I inspect first?"));
}
```

- [ ] **Step 3: Add the failing low-budget history-trimming test**

The low-budget guard for chat should trim history first and still preserve the selected leak context:

```rust
#[test]
fn provider_chat_prompt_trims_history_before_selected_leak_context() {
    let summary = sample_chat_summary();
    let leak = sample_chat_leak();
    let config = AiConfig {
        mode: AiMode::Provider,
        max_tokens: Some(256),
        ..AiConfig::default()
    };
    let history = vec![
        AiChatTurn {
            question: "q1".into(),
            answer_summary: "a1".into(),
        },
        AiChatTurn {
            question: "q2".into(),
            answer_summary: "a2".into(),
        },
        AiChatTurn {
            question: "q3".into(),
            answer_summary: "a3".into(),
        },
        AiChatTurn {
            question: "q4".into(),
            answer_summary: "a4".into(),
        },
    ];

    let prompt = build_provider_chat_toon_prompt(
        &summary,
        &[leak],
        "What keeps this alive?",
        &history,
        Some("com.example.CacheLeak"),
        &config,
    )
    .unwrap();

    assert!(prompt.contains("question=What keeps this alive?"));
    assert!(prompt.contains("class=com.example.CacheLeak"));
    assert!(!prompt.contains("q1"));
    assert!(!prompt.contains("q2"));
    assert!(!prompt.contains("q3"));
    assert!(!prompt.contains("q4"));
}
```

- [ ] **Step 4: Run the focused core tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_includes_question_focus_and_recent_history -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_trims_history_before_selected_leak_context -- --exact --nocapture
```

Expected: FAIL because `AiChatTurn` and `build_provider_chat_toon_prompt(...)` do not exist yet.

### Task 3: Implement Core AI Chat-Turn Support Without Changing `AiInsights`

**Files:**
- Modify: `core/src/analysis/ai.rs`

- [ ] **Step 1: Add the new chat-turn history type and public async entry point**

Add these definitions near the existing AI types and public entry points:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiChatTurn {
    pub question: String,
    pub answer_summary: String,
}

pub fn generate_ai_chat_turn(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    match config.mode {
        AiMode::Stub => Ok(generate_stub_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        )),
        AiMode::Rules => Ok(generate_rule_based_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        )),
        AiMode::Provider => generate_provider_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        ),
    }
}

pub async fn generate_ai_chat_turn_async(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    if matches!(config.mode, AiMode::Provider) {
        let summary = summary.clone();
        let leaks = leaks.to_vec();
        let history = history.to_vec();
        let question = question.to_string();
        let focus_leak_id = focus_leak_id.map(str::to_string);
        let config = config.clone();
        return tokio::task::spawn_blocking(move || {
            generate_ai_chat_turn(
                &summary,
                &leaks,
                &question,
                &history,
                focus_leak_id.as_deref(),
                &config,
            )
        })
        .await
        .map_err(|err| CoreError::Other(err.into()))?;
    }

    generate_ai_chat_turn(summary, leaks, question, history, focus_leak_id, config)
}
```

- [ ] **Step 2: Add chat-specific TOON helpers and deterministic rules/stub generation**

Implement a small internal chat prompt/response path inside the same file:

```rust
fn build_chat_toon_prompt(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> String {
    let mut body = build_request_section(summary, leaks, "chat_leak_follow_up", config);
    body.push_str("section chat\n");
    push_kv(&mut body, 2, "question", question);
    if let Some(focus) = focus_leak_id {
        push_kv(&mut body, 2, "active_leak_id", focus);
    }
    push_chat_history(&mut body, history, config);
    body
}

fn push_chat_history(buf: &mut String, history: &[AiChatTurn], config: &AiConfig) {
    let keep = match config.max_tokens {
        Some(limit) if limit <= 256 => 0,
        _ => history.len().min(3),
    };
    if keep == 0 {
        return;
    }

    buf.push_str("section conversation\n");
    for (idx, turn) in history[history.len() - keep..].iter().enumerate() {
        buf.push_str(&format!("  turn#{idx}\n"));
        push_kv(buf, 4, "question", &turn.question);
        push_kv(buf, 4, "answer_summary", &turn.answer_summary);
    }
}

fn build_chat_toon_response(
    summary_text: &str,
    recommendations: &[String],
    confidence: f32,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section response\n");
    push_kv(&mut body, 2, "model", &config.model);
    push_kv(&mut body, 2, "confidence_pct", format!("{:.0}", confidence * 100.0));
    push_kv(&mut body, 2, "summary", summary_text);
    body.push_str("section recommendations\n");
    for (idx, item) in recommendations.iter().enumerate() {
        push_kv(&mut body, 2, &format!("item#{idx}"), item);
    }
    body
}

fn generate_rule_based_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> AiInsights {
    let prompt = build_chat_toon_prompt(summary, leaks, question, history, focus_leak_id, config);
    let top = leaks.iter().max_by_key(|leak| leak.retained_size_bytes);
    let summary_text = match top {
        Some(leak) => format!(
            "Answering question '{}': {} retains ~{:.2} MB via {} instances and should be investigated first.",
            question,
            leak.class_name,
            bytes_to_mb(leak.retained_size_bytes),
            leak.instances,
        ),
        None => format!(
            "Answering question '{}': Heap '{}' currently looks healthy based on the analyzed summary.",
            question,
            summary.heap_path,
        ),
    };
    let recommendations = if let Some(leak) = top {
        vec![
            format!("Inspect the owner path retaining {}.", leak.class_name),
            "Compare the active leak against the top shortlist before expanding scope.".into(),
        ]
    } else {
        vec!["Capture another heap under load to confirm the healthy-heap baseline.".into()]
    };
    let confidence = 0.68;

    AiInsights {
        model: config.model.clone(),
        summary: summary_text.clone(),
        recommendations: recommendations.clone(),
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt,
            response: build_chat_toon_response(&summary_text, &recommendations, confidence, config),
        },
    }
}

fn generate_stub_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> AiInsights {
    let mut ai = generate_rule_based_chat_insights(
        summary,
        leaks,
        question,
        history,
        focus_leak_id,
        config,
    );
    ai.confidence = 0.55;
    ai.wire.prompt = build_chat_toon_prompt(summary, leaks, question, history, focus_leak_id, config);
    ai.wire.response = build_chat_toon_response(
        &ai.summary,
        &ai.recommendations,
        ai.confidence,
        config,
    );
    ai
}
```

- [ ] **Step 3: Add the provider chat prompt builder and provider chat dispatcher**

Refactor the current request-section helper so chat and one-shot explain can share it:

```rust
fn build_request_section(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    intent: &str,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    push_kv(&mut body, 2, "intent", intent);
    push_kv(&mut body, 2, "heap_path", &summary.heap_path);
    push_kv(&mut body, 2, "total_bytes", summary.total_size_bytes);
    push_kv(&mut body, 2, "total_objects", summary.total_objects);
    push_kv(&mut body, 2, "leak_sampled", leaks.len());

    body.push_str("section leaks\n");
    if leaks.is_empty() {
        push_kv(&mut body, 2, "status", "empty");
        return body;
    }

    let leak_limit = provider_leak_limit(config);
    if leak_limit < leaks.len().min(3) {
        push_kv(&mut body, 2, "context_truncated", "true");
    }

    for (idx, leak) in leaks.iter().enumerate().take(leak_limit) {
        body.push_str(&format!("  leak#{idx}\n"));
        push_kv(&mut body, 4, "id", &leak.id);
        push_kv(&mut body, 4, "class", &leak.class_name);
        push_kv(&mut body, 4, "kind", format!("{:?}", leak.leak_kind));
        push_kv(&mut body, 4, "severity", format!("{:?}", leak.severity));
        push_kv(
            &mut body,
            4,
            "retained_mb",
            format!("{:.2}", bytes_to_mb(leak.retained_size_bytes)),
        );
        push_kv(&mut body, 4, "instances", leak.instances);
        push_kv(
            &mut body,
            4,
            "description",
            truncate_prompt_description(&leak.description, config),
        );
    }

    body
}

fn build_provider_chat_toon_prompt(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<String> {
    let mut body = build_chat_toon_prompt(summary, leaks, question, history, focus_leak_id, config);
    let instructions = render_provider_instructions(
        config,
        &ProviderPromptContext {
            model: &config.model,
            provider: &config.provider.to_string(),
            heap_path: &summary.heap_path,
            total_bytes: summary.total_size_bytes,
            total_objects: summary.total_objects,
            leak_sampled: leaks.len(),
        },
    )?;
    body.push_str("section instructions\n");
    for (key, value) in instructions {
        push_kv(&mut body, 2, &key, value);
    }
    Ok(body)
}

fn generate_provider_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    let prompt = redact_provider_prompt(
        build_provider_chat_toon_prompt(summary, leaks, question, history, focus_leak_id, config)?,
        config,
    )?;
    emit_provider_audit_log(&prompt, config);
    let response = complete_llm(&LlmCompletionRequest {
        prompt: prompt.clone(),
        config: config.clone(),
    })?;
    parse_provider_toon_response(config, prompt, response.text)
}
```

Keep the low-budget behavior conservative: chat history trims to zero when `max_tokens <= 256`, and selected leak context stays intact.

- [ ] **Step 4: Run the focused core chat tests and make them pass**

Run:

```bash
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_includes_question_focus_and_recent_history -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_trims_history_before_selected_leak_context -- --exact --nocapture
```

Expected: PASS.

### Task 4: Add The CLI `chat` Command And Bounded REPL Session

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add `ChatArgs` and wire the new subcommand into `run()`**

Extend the CLI surface with:

```rust
#[derive(Debug, Parser)]
struct ChatArgs {
    heap: PathBuf,
}

enum Commands {
    Parse(ParseArgs),
    Leaks(LeakArgs),
    Analyze(AnalyzeArgs),
    Diff(DiffArgs),
    Map(MapArgs),
    GcPath(GcPathArgs),
    Query(QueryArgs),
    Explain(ExplainArgs),
    Chat(ChatArgs),
    Fix(FixArgs),
    Serve(ServeArgs),
    Config,
}
```

Then add the `match` arm:

```rust
Commands::Chat(args) => handle_chat(args, &loaded_config.data).await?,
```

- [ ] **Step 2: Add small CLI-local session and rendering helpers in `cli/src/main.rs`**

Keep the new helper types inside the existing CLI file:

```rust
#[derive(Debug, Clone)]
struct ChatSession {
    summary: HeapSummary,
    leaks: Vec<mnemosyne_core::analysis::LeakInsight>,
    focus_leak_id: Option<String>,
    history: Vec<mnemosyne_core::analysis::AiChatTurn>,
}

fn print_chat_help() {
    println!("Type a question about the current leak context.");
    println!("Commands: /focus <leak-id>, /list, /help, /exit");
}

fn print_chat_shortlist(leaks: &[mnemosyne_core::analysis::LeakInsight]) {
    let shortlist: Vec<_> = leaks.iter().take(3).cloned().collect();
    println!("{}", bold_label("Top leak candidates:"));
    if shortlist.is_empty() {
        println!(
            "No leak suspects detected. Ask questions about the healthy-heap summary or type /exit."
        );
        return;
    }

    let (table, truncated_ids, truncated_classes) = build_leaks_table(&shortlist);
    println!("{table}");
    print_full_value_section("Full leak IDs for truncated rows:", &truncated_ids);
    print_full_value_section("Full class names for truncated leak rows:", &truncated_classes);
}
```

- [ ] **Step 3: Implement `handle_chat(...)` using one startup analysis and a line-oriented REPL**

Add a new handler with this shape:

```rust
async fn handle_chat(args: ChatArgs, base_config: &AppConfig) -> Result<()> {
    use std::io::{self, Write};

    validate_heap_file(&args.heap)?;

    let mut config = base_config.clone();
    config.ai.enabled = true;

    let pb = start_spinner("Analyzing heap for chat...");
    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options: LeakDetectionOptions::from(&config.analysis),
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await
    .with_context(|| format!("Failed to start chat from heap dump: {}", args.heap.display()))?;
    finish_spinner(&pb, "Chat context ready.");

    let mut session = ChatSession {
        summary: response.summary,
        leaks: response.leaks,
        focus_leak_id: None,
        history: Vec::new(),
    };

    print_chat_shortlist(&session.leaks);
    print_chat_help();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "chat> ")?;
        stdout.flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input == "/exit" {
            break;
        }
        if input == "/help" {
            print_chat_help();
            continue;
        }
        if input == "/list" {
            print_chat_shortlist(&session.leaks);
            continue;
        }
        if let Some(target) = input.strip_prefix("/focus ") {
            let target = target.trim();
            match validate_leak_id(&session.leaks, target) {
                Ok(()) => {
                    session.focus_leak_id = Some(target.to_string());
                    println!("{} {}", bold_label("Focused leak:"), target);
                }
                Err(err) => {
                    println!("{} {err}", bold_label("Focus error:"));
                }
            }
            continue;
        }

        let targeted = focus_leaks(&session.leaks, session.focus_leak_id.as_deref());
        println!("{} {}", bold_label("Question:"), input);
        let ai = generate_ai_chat_turn_async(
            &session.summary,
            &targeted,
            input,
            &session.history,
            session.focus_leak_id.as_deref(),
            &config.ai,
        )
        .await?;
        println!("{}", bold_label("Answer:"));
        println!("{}", ai.summary);
        if !ai.recommendations.is_empty() {
            println!("{}", bold_label("Recommendations:"));
            for rec in &ai.recommendations {
                println!("- {rec}");
            }
        }

        session.history.push(mnemosyne_core::analysis::AiChatTurn {
            question: input.to_string(),
            answer_summary: ai.summary.clone(),
        });
        if session.history.len() > 3 {
            let excess = session.history.len() - 3;
            session.history.drain(0..excess);
        }
    }

    Ok(())
}
```

This keeps the first slice bounded and avoids a second AI call during startup.

- [ ] **Step 4: Run the focused chat integration tests and make them pass**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_chat_starts_with_shortlist_and_help -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_answers_a_question_in_rules_mode -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_focuses_on_selected_leak -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_rejects_invalid_focus_target_and_continues -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response -- --exact --nocapture
```

Expected: PASS.

### Task 5: Format, Re-Verify, And Sync Docs

**Files:**
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`
- Modify: `OVERNIGHT_SUMMARY.md`

- [ ] **Step 1: Format the workspace**

Run:

```bash
cargo fmt --all
```

- [ ] **Step 2: Run the focused verification set**

Run:

```bash
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_includes_question_focus_and_recent_history -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_trims_history_before_selected_leak_context -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_starts_with_shortlist_and_help -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_answers_a_question_in_rules_mode -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_focuses_on_selected_leak -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_rejects_invalid_focus_target_and_continues -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_redacts_prompt_before_send -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_truncates_leak_context_when_max_tokens_is_small -- --exact --nocapture
cargo check
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: PASS.

Important: do not replace this with unscoped `cargo test -p mnemosyne-core`. That still has the unrelated pre-existing `mnemosyne_core::test_fixtures` integration-test/export issue and should not be used as the success claim for this batch.

- [ ] **Step 3: Update docs to reflect the CLI-first Step `14(e)` slice**

Add or update content with wording shaped like:

```md
- `mnemosyne-cli chat <heap.hprof>` starts a CLI-only, leak-focused conversation session.
- The command analyzes the heap once, prints the top 3 leak candidates, and supports `/focus <leak-id>`, `/list`, `/help`, and `/exit`.
- Conversation history is in-process only for the running CLI session.
- Provider-mode privacy redaction, audit logging, and the minimal `max_tokens` guard still apply to chat prompts.
```

Apply that truth across:

- `README.md`
- `ARCHITECTURE.md`
- `STATUS.md`
- `docs/roadmap.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`
- `OVERNIGHT_SUMMARY.md`

- [ ] **Step 4: Record the roadmap/state transition honestly**

Use these status updates:

- backlog item `26` in `docs/roadmap.md`: change from `⚬ Pending` to `⚠️ Partial — CLI-first leak-focused chat landed; MCP/session follow-through remains`
- Step `14(e)` in the milestone detail: mark the CLI-first conversation slice complete and leave MCP session semantics out of scope
- `STATUS.md`: move AI conversation mode from pending to partial / CLI-first shipped
- `OVERNIGHT_SUMMARY.md`: record the new command, verification run, and the next remaining M5 follow-on work
