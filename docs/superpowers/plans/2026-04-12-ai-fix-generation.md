# AI Fix Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `mnemosyne fix` and MCP `propose_fix` from template-only placeholder patches to AI-backed fix suggestions that preserve the current response contract and fall back safely when provider-backed generation is not trustworthy.

**Architecture:** Thread real `AppConfig` into the fix pipeline first, then extend `core/src/fix/generator.rs` with a small AI-first decision path that reuses existing heap analysis, `map_to_code()`, provider transport, prompt redaction, and audit logging. Keep the public `FixRequest` / `FixSuggestion` / `FixResponse` shapes stable, parse a strict TOON fix response locally, and fall back to the current heuristic/template patch path with explicit provenance when the AI path is unavailable or malformed.

**Tech Stack:** Rust, existing `mnemosyne-core` analysis + mapper modules, current provider AI helpers in `core/src/analysis/ai.rs`, `assert_cmd`, Tokio async tests, MCP stdio contract tests

---

## File Map

- `core/src/fix/generator.rs`
  - keep `propose_fix` as the shared entry point for CLI and MCP
  - add provider-fix prompt building, TOON parsing, source-snippet extraction, and fallback orchestration
- `core/src/analysis/ai.rs`
  - expose the smallest shared helper needed to send a fully rendered provider prompt through redaction, audit logging, and provider completion
- `cli/src/main.rs`
  - pass loaded `AppConfig` into `handle_fix()` and then into `propose_fix()`
  - keep CLI output formatting stable
- `core/src/mcp/server.rs`
  - pass MCP server config into `propose_fix()`
  - keep `propose_fix` response shape stable for MCP callers
- `cli/tests/integration.rs`
  - add provider-mode `fix` coverage and fallback-path coverage
- `docs/api.md`
  - update `propose_fix` docs from heuristic-only wording to AI-first with fallback wording
- `README.md`
  - update fix-generation description to reflect the shipped behavior honestly
- `ARCHITECTURE.md`
  - update AI/fix architecture notes to reflect provider-backed fix generation and fallback semantics
- `STATUS.md`
  - move fix suggestions from template-only to partial / AI-backed with fallback
- `docs/roadmap.md`
  - record the AI-driven fix-generation slice as completed or partial, depending on final implementation truth
- `OVERNIGHT_SUMMARY.md`
  - record the batch and verification

### Task 1: Thread Real Config Into The Fix Path

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `core/src/mcp/server.rs`
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Write the failing core unit test proving `propose_fix` can no longer hardcode defaults**

Add this test in `core/src/fix/generator.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
    #[tokio::test]
    async fn propose_fix_uses_passed_app_config_for_ai_mode() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_string_lossy().into_owned();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let err = propose_fix(
            FixRequest {
                heap_path: path,
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &config,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("API key") || err.to_string().contains("project_root"));
    }
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test -p mnemosyne-core propose_fix_uses_passed_app_config_for_ai_mode --lib -- --exact --nocapture
```

Expected: FAIL because `propose_fix()` still takes only `FixRequest` and internally uses `AppConfig::default()`.

- [ ] **Step 3: Update the shared fix entry point signature**

In `core/src/fix/generator.rs`, change the function signature from:

```rust
pub async fn propose_fix(request: FixRequest) -> CoreResult<FixResponse> {
    let mut config = AppConfig::default();
    config.ai.enabled = true;
```

to:

```rust
pub async fn propose_fix(request: FixRequest, base_config: &AppConfig) -> CoreResult<FixResponse> {
    let mut config = base_config.clone();
    config.ai.enabled = true;
```

- [ ] **Step 4: Thread the loaded CLI config into `handle_fix()`**

In `cli/src/main.rs`, change the command dispatch and handler signature from:

```rust
        Commands::Fix(args) => handle_fix(args).await?,
```

and:

```rust
async fn handle_fix(args: FixArgs) -> Result<()> {
```

to:

```rust
        Commands::Fix(args) => handle_fix(args, &loaded_config.data).await?,
```

and:

```rust
async fn handle_fix(args: FixArgs, cfg: &AppConfig) -> Result<()> {
```

Then change the call site to:

```rust
    let response = propose_fix(
        FixRequest {
            heap_path: args.heap.to_string_lossy().into_owned(),
            leak_id: args.leak_id,
            style: args.style.into(),
            project_root: args.project_root,
        },
        cfg,
    )
```

- [ ] **Step 5: Thread the MCP config into `propose_fix()`**

In `core/src/mcp/server.rs`, change:

```rust
            let response = propose_fix(FixRequest {
                heap_path: params.heap_path,
                leak_id: params.leak_id,
                style: params.style,
                project_root: params.project_root,
            })
            .await?;
```

to:

```rust
            let response = propose_fix(
                FixRequest {
                    heap_path: params.heap_path,
                    leak_id: params.leak_id,
                    style: params.style,
                    project_root: params.project_root,
                },
                config,
            )
            .await?;
```

- [ ] **Step 6: Run the focused core test and existing fix regressions**

Run:

```bash
cargo test -p mnemosyne-core propose_fix_uses_passed_app_config_for_ai_mode --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_runs_analysis --lib -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_succeeds -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_invalid_leak_id_errors -- --exact --nocapture
```

Expected: PASS.

### Task 2: Add Red Tests For Provider Fix Parsing And Fallback Provenance

**Files:**
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Add a failing provider TOON parsing unit test**

Add these tests inside `core/src/fix/generator.rs`:

```rust
    #[test]
    fn parse_provider_fix_response_parses_valid_toon_payload() {
        let parsed = parse_provider_fix_response(
            "TOON v1\nsection response\n  confidence_pct=81\n  description=Release retained cache entries.\nsection patch\n  diff=--- a/src/main/java/com/example/Cache.java\\n+++ b/src/main/java/com/example/Cache.java\\n@@ ...\n",
        )
        .unwrap();

        assert_eq!(parsed.description, "Release retained cache entries.");
        assert!((parsed.confidence - 0.81).abs() < f32::EPSILON);
        assert!(parsed.diff.contains("--- a/src/main/java/com/example/Cache.java"));
    }
```

- [ ] **Step 2: Add a failing malformed-provider-response test**

```rust
    #[test]
    fn parse_provider_fix_response_rejects_missing_patch_section() {
        let err = parse_provider_fix_response(
            "TOON v1\nsection response\n  confidence_pct=81\n  description=Missing patch section\n",
        )
        .unwrap_err();

        assert!(err.to_string().contains("section patch") || err.to_string().contains("diff"));
    }
```

- [ ] **Step 3: Add a failing fallback-provenance test**

```rust
    #[test]
    fn fallback_provenance_includes_fallback_and_placeholder_markers() {
        let provenance = fallback_provenance("provider fix generation was skipped");

        assert!(provenance.iter().any(|m| m.kind == ProvenanceKind::Synthetic));
        assert!(provenance.iter().any(|m| m.kind == ProvenanceKind::Fallback));
        assert!(provenance.iter().any(|m| m.kind == ProvenanceKind::Placeholder));
    }
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-core parse_provider_fix_response_parses_valid_toon_payload --lib -- --exact --nocapture
cargo test -p mnemosyne-core parse_provider_fix_response_rejects_missing_patch_section --lib -- --exact --nocapture
cargo test -p mnemosyne-core fallback_provenance_includes_fallback_and_placeholder_markers --lib -- --exact --nocapture
```

Expected: FAIL because the parser and fallback helpers do not exist yet.

### Task 3: Implement Local TOON Fix Parsing, Provenance, And Source Snippet Helpers

**Files:**
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Add the minimal internal structs and helper signatures**

Near the top of `core/src/fix/generator.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
struct ProviderFixDraft {
    description: String,
    diff: String,
    confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct SourceSnippet {
    target_file: String,
    snippet: String,
}
```

And add these function signatures below `propose_fix()`:

```rust
fn parse_provider_fix_response(response: &str) -> CoreResult<ProviderFixDraft> {
    todo!()
}

fn fallback_provenance(reason: &str) -> Vec<ProvenanceMarker> {
    todo!()
}

fn extract_source_snippet(file: &Path, line: u32) -> CoreResult<String> {
    todo!()
}
```

- [ ] **Step 2: Implement `fallback_provenance()`**

Use the exact marker structure below:

```rust
fn fallback_provenance(reason: &str) -> Vec<ProvenanceMarker> {
    vec![
        ProvenanceMarker::new(
            ProvenanceKind::Synthetic,
            "Fix suggestions are generated heuristically from leak summaries.",
        ),
        ProvenanceMarker::new(ProvenanceKind::Fallback, reason),
        ProvenanceMarker::new(
            ProvenanceKind::Placeholder,
            "Static-analysis-backed remediation is not wired yet; this is placeholder guidance.",
        ),
    ]
}
```

- [ ] **Step 3: Implement `extract_source_snippet()` with a narrow local window**

Use this implementation shape:

```rust
fn extract_source_snippet(file: &Path, line: u32) -> CoreResult<String> {
    let contents = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }

    let idx = line.saturating_sub(1) as usize;
    let start = idx.saturating_sub(5);
    let end = usize::min(lines.len(), idx.saturating_add(6));

    Ok(lines[start..end].join("\n"))
}
```

- [ ] **Step 4: Implement `parse_provider_fix_response()`**

Parse the following keys:

- `response.confidence_pct`
- `response.description`
- `patch.diff`

Use this implementation outline:

```rust
fn parse_provider_fix_response(response: &str) -> CoreResult<ProviderFixDraft> {
    if !response.starts_with("TOON v1") {
        return Err(crate::CoreError::InvalidInput(
            "provider returned malformed TOON output".into(),
        ));
    }

    let mut section = String::new();
    let mut description = None;
    let mut diff = None;
    let mut confidence = None;

    for raw_line in response.lines().skip(1) {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("section ") {
            section = rest.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.replace("\\n", "\n").replace("\\r", "\r").replace("\\\\", "\\");

        match (section.as_str(), key.trim()) {
            ("response", "confidence_pct") => {
                let pct: f32 = value.parse().map_err(|_| {
                    crate::CoreError::InvalidInput(
                        "provider returned invalid confidence_pct".into(),
                    )
                })?;
                confidence = Some((pct / 100.0).clamp(0.0, 1.0));
            }
            ("response", "description") => description = Some(value),
            ("patch", "diff") => diff = Some(value),
            _ => {}
        }
    }

    let description = description.ok_or_else(|| {
        crate::CoreError::InvalidInput("provider TOON output missing response description".into())
    })?;
    let diff = diff.ok_or_else(|| {
        crate::CoreError::InvalidInput("provider TOON output missing patch diff".into())
    })?;
    let confidence = confidence.ok_or_else(|| {
        crate::CoreError::InvalidInput("provider TOON output missing response confidence_pct".into())
    })?;

    Ok(ProviderFixDraft {
        description,
        diff,
        confidence,
    })
}
```

- [ ] **Step 5: Re-run the focused unit tests**

Run:

```bash
cargo test -p mnemosyne-core parse_provider_fix_response_parses_valid_toon_payload --lib -- --exact --nocapture
cargo test -p mnemosyne-core parse_provider_fix_response_rejects_missing_patch_section --lib -- --exact --nocapture
cargo test -p mnemosyne-core fallback_provenance_includes_fallback_and_placeholder_markers --lib -- --exact --nocapture
```

Expected: PASS.

### Task 4: Add Red Tests For Source-Backed Provider Fix Generation

**Files:**
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Add a failing unit test for mapped source snippet extraction**

Add this test:

```rust
    #[test]
    fn extract_source_snippet_reads_small_window_around_line() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Example.java");
        std::fs::write(
            &file,
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n",
        )
        .unwrap();

        let snippet = extract_source_snippet(&file, 6).unwrap();
        assert!(snippet.contains("line1"));
        assert!(snippet.contains("line6"));
        assert!(snippet.contains("line11"));
    }
```

- [ ] **Step 2: Add a failing provider-backed fix generation test**

Add this async test:

```rust
    #[tokio::test]
    async fn propose_fix_returns_provider_backed_fix_when_source_context_is_available() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "TOON v1\nsection response\n  confidence_pct=82\n  description=Evict idle cache entries before they accumulate.\nsection patch\n  diff=--- a/src/main/java/com/example/CacheLeak.java\\n+++ b/src/main/java/com/example/CacheLeak.java\\n@@ ...\n"
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
            assert!(request.contains("target_file="), "{request}");
            assert!(request.contains("source_snippet="), "{request}");

            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let mut heap = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut heap);

        let project = tempfile::tempdir().unwrap();
        let source_dir = project
            .path()
            .join("src")
            .join("main")
            .join("java")
            .join("com")
            .join("example");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("CacheLeak.java"),
            "package com.example;\npublic class CacheLeak {\n  void retain() {}\n}\n",
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;
        config.ai.provider = crate::config::AiProvider::Local;
        config.ai.endpoint = Some(format!("http://{addr}/v1"));
        config.ai.api_key_env = Some("MNEMOSYNE_TEST_LOCAL_KEY".into());
        config.ai.timeout_secs = 2;
        std::env::set_var("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key");

        let response = propose_fix(
            FixRequest {
                heap_path: heap.path().to_string_lossy().into_owned(),
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: Some(project.path().to_path_buf()),
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(response.suggestions.len(), 1);
        assert_eq!(
            response.suggestions[0].description,
            "Evict idle cache entries before they accumulate."
        );
        assert!(response.suggestions[0].diff.contains("CacheLeak.java"));
        assert!(response.provenance.is_empty());

        std::env::remove_var("MNEMOSYNE_TEST_LOCAL_KEY");
        server.join().unwrap();
    }
```

- [ ] **Step 3: Add a failing provider-fallback test**

```rust
    #[tokio::test]
    async fn propose_fix_falls_back_to_template_when_provider_fix_is_unavailable() {
        let mut heap = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut heap);

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let response = propose_fix(
            FixRequest {
                heap_path: heap.path().to_string_lossy().into_owned(),
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(response.suggestions.len(), 1);
        assert!(response.suggestions[0].diff.contains("SAFE_CAPACITY"));
        assert!(response.provenance.iter().any(|m| m.kind == ProvenanceKind::Fallback));
        assert!(response.provenance.iter().any(|m| m.kind == ProvenanceKind::Placeholder));
    }
```

- [ ] **Step 4: Run the focused tests to verify they fail**

Run:

```bash
cargo test -p mnemosyne-core extract_source_snippet_reads_small_window_around_line --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_returns_provider_backed_fix_when_source_context_is_available --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_falls_back_to_template_when_provider_fix_is_unavailable --lib -- --exact --nocapture
```

Expected: FAIL because `propose_fix()` still only uses heuristic builders.

### Task 5: Implement AI-First Fix Generation With Safe Fallback

**Files:**
- Modify: `core/src/fix/generator.rs`
- Modify: `core/src/analysis/ai.rs`

- [ ] **Step 1: Add the smallest shared provider helper in `core/src/analysis/ai.rs`**

Just below `generate_provider_chat_insights()`, add:

```rust
pub(crate) fn complete_provider_prompt(prompt: String, config: &AiConfig) -> CoreResult<String> {
    let prompt = redact_provider_prompt(prompt, config)?;
    emit_provider_audit_log(&prompt, config);
    let response = complete_llm(&LlmCompletionRequest {
        prompt,
        config: config.clone(),
    })?;
    Ok(response.text)
}
```

Then rewrite the existing provider paths to use it:

```rust
    let prompt = build_provider_toon_prompt(summary, leaks, config)?;
    let response = complete_provider_prompt(prompt.clone(), config)?;
    parse_provider_toon_response(config, redact_provider_prompt(prompt, config)?, response)
```

Use the same idea for `generate_provider_chat_insights()` so the redaction/audit logic still has one execution path.

- [ ] **Step 2: Add prompt-building and validation helpers to `core/src/fix/generator.rs`**

Add these function signatures below the snippet/parser helpers:

```rust
fn build_provider_fix_prompt(
    leak: &LeakInsight,
    style: &FixStyle,
    target_file: &str,
    source_snippet: &str,
    heap_path: &str,
) -> String {
    todo!()
}

fn validate_provider_fix_diff(diff: &str, target_file: &str) -> bool {
    todo!()
}
```

Implement them using this exact outline:

```rust
fn build_provider_fix_prompt(
    leak: &LeakInsight,
    style: &FixStyle,
    target_file: &str,
    source_snippet: &str,
    heap_path: &str,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    body.push_str("  intent=generate_fix\n");
    body.push_str(&format!("  heap_path={}\n", escape_toon_value(heap_path)));
    body.push_str(&format!("  leak_id={}\n", escape_toon_value(&leak.id)));
    body.push_str(&format!("  class_name={}\n", escape_toon_value(&leak.class_name)));
    body.push_str(&format!("  severity={:?}\n", leak.severity));
    body.push_str(&format!("  retained_bytes={}\n", leak.retained_size_bytes));
    body.push_str(&format!("  style={:?}\n", style));
    body.push_str(&format!("  target_file={}\n", escape_toon_value(target_file)));
    body.push_str(&format!("  leak_description={}\n", escape_toon_value(&leak.description)));
    body.push_str("section source\n");
    body.push_str(&format!("  source_snippet={}\n", escape_toon_value(source_snippet)));
    body.push_str("section instructions\n");
    body.push_str("  item#0=Return only TOON v1 output.\n");
    body.push_str("  item#1=Patch only the provided target_file.\n");
    body.push_str("  item#2=Stay consistent with the requested fix style.\n");
    body.push_str("  item#3=If context is weak, keep the patch minimal and confidence lower rather than inventing files.\n");
    body
}

fn validate_provider_fix_diff(diff: &str, target_file: &str) -> bool {
    let relative = target_file.replace('\\', "/");
    !diff.trim().is_empty()
        && diff.contains("--- a/")
        && diff.contains("+++ b/")
        && (diff.contains(&relative)
            || std::path::Path::new(target_file)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| diff.contains(name)))
}
```

- [ ] **Step 3: Add a helper to resolve one mapped file plus snippet**

In `core/src/fix/generator.rs`, add:

```rust
fn source_snippet_for_leak(leak: &LeakInsight, project_root: &Path) -> Option<SourceSnippet> {
    let mapped = crate::mapper::map_to_code(&crate::mapper::MapToCodeRequest {
        leak_id: leak.id.clone(),
        class_name: Some(leak.class_name.clone()),
        project_root: project_root.to_path_buf(),
        include_git_info: false,
    })
    .ok()?;

    let location = mapped.locations.into_iter().next()?;
    if !location.file.exists() {
        return None;
    }

    let snippet = extract_source_snippet(&location.file, location.line).ok()?;
    Some(SourceSnippet {
        target_file: location.file.display().to_string(),
        snippet,
    })
}
```

- [ ] **Step 4: Rewrite `propose_fix()` to choose AI-backed generation or fallback**

Replace the current suggestion-building block with this structure:

```rust
    let Some(leak) = leaks.into_iter().next() else {
        return Ok(FixResponse {
            suggestions: Vec::new(),
            project_root: request.project_root,
            provenance: Vec::new(),
        });
    };

    let mut provenance = Vec::new();
    let suggestion = if matches!(config.ai.mode, crate::config::AiMode::Provider) {
        if let Some(root) = request.project_root.as_deref() {
            if let Some(source) = source_snippet_for_leak(&leak, root) {
                let prompt = build_provider_fix_prompt(
                    &leak,
                    &request.style,
                    &source.target_file,
                    &source.snippet,
                    &request.heap_path,
                );
                match crate::analysis::complete_provider_prompt(prompt, &config.ai)
                    .and_then(|raw| parse_provider_fix_response(&raw))
                {
                    Ok(draft) if validate_provider_fix_diff(&draft.diff, &source.target_file) => {
                        FixSuggestion {
                            leak_id: leak.id.clone(),
                            class_name: leak.class_name.clone(),
                            target_file: source.target_file,
                            description: draft.description,
                            diff: draft.diff,
                            confidence: draft.confidence,
                            style: request.style.clone(),
                        }
                    }
                    Ok(_) => {
                        provenance = fallback_provenance(
                            "Provider-backed fix generation returned a diff that failed local validation.",
                        );
                        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
                    }
                    Err(_) => {
                        provenance = fallback_provenance(
                            "Provider-backed fix generation was unavailable; returned heuristic guidance instead.",
                        );
                        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
                    }
                }
            } else {
                provenance = fallback_provenance(
                    "Provider-backed fix generation was skipped because source targeting or snippet extraction was unavailable.",
                );
                build_suggestion(&leak, request.project_root.as_deref(), &request.style)
            }
        } else {
            provenance = fallback_provenance(
                "Provider-backed fix generation was skipped because project_root was not provided.",
            );
            build_suggestion(&leak, request.project_root.as_deref(), &request.style)
        }
    } else {
        provenance = fallback_provenance(
            "Provider-backed fix generation was skipped because AI provider mode is not active.",
        );
        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
    };

    Ok(FixResponse {
        suggestions: vec![suggestion],
        project_root: request.project_root,
        provenance,
    })
```

- [ ] **Step 5: Re-run the focused unit tests**

Run:

```bash
cargo test -p mnemosyne-core extract_source_snippet_reads_small_window_around_line --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_returns_provider_backed_fix_when_source_context_is_available --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_falls_back_to_template_when_provider_fix_is_unavailable --lib -- --exact --nocapture
```

Expected: PASS.

### Task 6: Add Red CLI And MCP Contract Tests For The New Behavior

**Files:**
- Modify: `cli/tests/integration.rs`
- Modify: `core/src/mcp/server.rs`

- [ ] **Step 1: Add a failing CLI provider-mode fix integration test**

Add this test in `cli/tests/integration.rs` near `test_fix_succeeds`:

```rust
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
```

- [ ] **Step 2: Add a failing MCP fallback test**

Add this test in `core/src/mcp/server.rs`:

```rust
    #[tokio::test]
    async fn handle_request_propose_fix_falls_back_without_transport_error() {
        let fixture = build_graph_fixture();
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&fixture).unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let result = handle_request(
            RpcRequest {
                id: json!(8),
                method: "propose_fix".into(),
                params: json!({
                    "heap_path": file.path().to_string_lossy().into_owned(),
                    "style": "Minimal"
                }),
            },
            &config,
        )
        .await
        .unwrap();

        let suggestions = result
            .get("suggestions")
            .and_then(Value::as_array)
            .expect("suggestions array");
        assert_eq!(suggestions.len(), 1);

        let provenance = result
            .get("provenance")
            .and_then(Value::as_array)
            .expect("provenance array");
        assert!(provenance.iter().any(|m| m.get("kind") == Some(&json!("FALLBACK"))));
    }
```

- [ ] **Step 3: Run the focused tests to verify they fail**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_fix_with_provider_mode_returns_ai_backed_patch -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_propose_fix_falls_back_without_transport_error --lib -- --exact --nocapture
```

Expected: FAIL until the new provider-backed path is fully wired.

### Task 7: Make CLI And MCP Pass With The New Shared Behavior

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `core/src/mcp/server.rs`
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Keep CLI output stable while allowing empty provenance on AI success**

No structural rewrite is needed in `handle_fix()`. Keep this block exactly as the display logic:

```rust
    for suggestion in response.suggestions {
        println!(
            "Fix for {} [{}] ({:?}, confidence {:.0}%):",
            suggestion.class_name,
            suggestion.leak_id,
            suggestion.style,
            suggestion.confidence * 100.0
        );
        println!("{} {}", bold_label("File:"), suggestion.target_file);
        println!("{}", suggestion.description);
        println!("{}\n{}", bold_label("Patch:"), suggestion.diff);
    }

    if !response.provenance.is_empty() {
        println!();
        for marker in &response.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            println!("  [{}] {}", styled_provenance(marker.kind), detail);
        }
    }
```

This should already show no provenance block on successful AI-backed fixes.

- [ ] **Step 2: Update MCP tool metadata description for `propose_fix`**

In `core/src/mcp/server.rs`, change the tool description from:

```rust
                "description": "Generate heuristic fix suggestions for a heap or a specific leak candidate.",
```

to:

```rust
                "description": "Generate AI-backed fix suggestions when provider mode and source context are available, otherwise fall back to heuristic guidance.",
```

- [ ] **Step 3: Run the focused CLI and MCP tests**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_fix_succeeds -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_with_provider_mode_returns_ai_backed_patch -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_propose_fix_falls_back_without_transport_error --lib -- --exact --nocapture
```

Expected: PASS.

### Task 8: Update Docs And Re-Verify The Whole Slice

**Files:**
- Modify: `docs/api.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`

- [ ] **Step 1: Update `docs/api.md`**

Change the `propose_fix` section so it says:

- it is no longer heuristic-only
- provider mode can return AI-backed patch text when `project_root` yields a mapped source file and snippet
- fallback to heuristic guidance still occurs when provider mode is inactive or insufficient

- [ ] **Step 2: Update `README.md` and `ARCHITECTURE.md`**

Describe fix generation as:

- AI-backed when provider mode and source context are available
- still fallback-safe and explicitly labeled when Mnemosyne returns heuristic/template guidance

- [ ] **Step 3: Update `STATUS.md`, `docs/roadmap.md`, and `OVERNIGHT_SUMMARY.md`**

Record the implementation truth exactly:

- `fix` / `propose_fix` are no longer template-only
- AI-backed fix generation is now partial but real
- the current slice is still one-file, one-snippet, fallback-safe rather than full static analysis

- [ ] **Step 4: Run the final verification set**

Run:

```bash
cargo test -p mnemosyne-core parse_provider_fix_response_parses_valid_toon_payload --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_returns_provider_backed_fix_when_source_context_is_available --lib -- --exact --nocapture
cargo test -p mnemosyne-core propose_fix_falls_back_to_template_when_provider_fix_is_unavailable --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_propose_fix_falls_back_without_transport_error --lib -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_succeeds -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_with_provider_mode_returns_ai_backed_patch -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_fix_invalid_leak_id_errors -- --exact --nocapture
cargo test -p mnemosyne-core --lib provider_mode_ -- --nocapture
cargo check
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 5: Review git scope before any commit or PR work**

Run:

```bash
git diff --stat
git status --short
```

Expected: only the planned fix-generation files and docs should be modified in this worktree.
