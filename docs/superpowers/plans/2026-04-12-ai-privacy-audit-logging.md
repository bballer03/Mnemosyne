# AI Privacy Audit Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the next Step 14(d) privacy sub-slice by making provider-mode audit logging configurable, hashed, and safe to emit through existing tracing without changing the outward AI response contracts.

**Architecture:** Extend `[ai.privacy]` with a boolean `audit_log` flag that defaults to `false` and flows through TOML plus environment overrides. In `core::analysis::ai`, emit one provider audit log entry immediately before the external provider call using a SHA-256 hash of the already-redacted outbound prompt plus metadata like provider, model, prompt byte length, and redaction settings; never log raw prompt content.

**Tech Stack:** Rust, existing `tracing`/`tracing-subscriber`, SHA-256 hashing via `sha2`, existing CLI/core test harnesses

---

### Task 1: Add Red Tests For Audit-Logging Config And Runtime Behavior

**Files:**
- Modify: `cli/src/config_loader.rs`
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Write the failing config-loader test**

Add a unit test in `cli/src/config_loader.rs` for:

```toml
[ai]
enabled = true
mode = "provider"

[ai.privacy]
redact_heap_path = true
redact_patterns = ["secret-token-\\d+"]
audit_log = true
```

Assert:

```rust
assert!(cfg.ai.privacy.redact_heap_path);
assert_eq!(cfg.ai.privacy.redact_patterns, vec!["secret-token-\\d+"]);
assert!(cfg.ai.privacy.audit_log);
```

- [ ] **Step 2: Write the failing provider integration test**

Add a CLI integration test in `cli/tests/integration.rs` that:

1. creates a local mock provider server
2. writes a provider config with `[ai.privacy].audit_log = true`
3. sets `RUST_LOG=info`
4. runs `mnemosyne-cli analyze <fixture> --format json --ai`
5. asserts stderr contains an audit marker and hash metadata, but not the raw secret/prompt content

Use assertions shaped like:

```rust
let stderr = stdout_string(&output.stderr);
assert!(stderr.contains("provider_ai_audit"), "{stderr}");
assert!(stderr.contains("prompt_sha256="), "{stderr}");
assert!(stderr.contains("prompt_bytes="), "{stderr}");
assert!(!stderr.contains("secret-token-123"), "{stderr}");
assert!(!stderr.contains("custom_secret="), "{stderr}");
```

- [ ] **Step 3: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-cli --bin mnemosyne-cli config_loader::tests::parses_ai_provider_privacy_config -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content -- --exact --nocapture
```

Expected: FAIL because `audit_log` does not exist yet and no audit log line is emitted.

### Task 2: Add Audit-Logging Config Plumbing

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`

- [ ] **Step 1: Extend `AiPrivacyConfig` with the new flag**

Add the minimal field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiPrivacyConfig {
    pub redact_heap_path: bool,
    pub redact_patterns: Vec<String>,
    pub audit_log: bool,
}
```

- [ ] **Step 2: Extend TOML parsing for `[ai.privacy].audit_log`**

Update the partial config type and merge logic:

```rust
#[derive(Debug, Default, Deserialize)]
struct PartialAiPrivacyConfig {
    redact_heap_path: Option<bool>,
    redact_patterns: Option<Vec<String>>,
    audit_log: Option<bool>,
}

if let Some(value) = privacy.audit_log {
    cfg.privacy.audit_log = value;
}
```

- [ ] **Step 3: Add the environment override**

Support:

```bash
MNEMOSYNE_AI_AUDIT_LOG=true
```

Implementation shape:

```rust
if let Ok(value) = env::var("MNEMOSYNE_AI_AUDIT_LOG") {
    if let Some(parsed) = parse_bool(&value) {
        cfg.ai.privacy.audit_log = parsed;
    } else {
        warn!("Ignoring MNEMOSYNE_AI_AUDIT_LOG: expected boolean");
    }
}
```

- [ ] **Step 4: Run the config-loader test and verify it passes**

Run:

```bash
cargo test -p mnemosyne-cli --bin mnemosyne-cli config_loader::tests::parses_ai_provider_privacy_config -- --exact --nocapture
```

Expected: PASS.

### Task 3: Emit Hashed Provider Audit Logs Without Logging Prompt Content

**Files:**
- Modify: `Cargo.toml`
- Modify: `core/Cargo.toml`
- Modify: `core/src/analysis/ai.rs`

- [ ] **Step 1: Add the hashing dependency**

Add workspace and core dependency entries:

```toml
# Cargo.toml
sha2 = "0.10"

# core/Cargo.toml
sha2.workspace = true
```

- [ ] **Step 2: Add a tiny helper that hashes and logs only metadata**

Add a helper in `core/src/analysis/ai.rs` with a shape like:

```rust
fn emit_provider_audit_log(prompt: &str, config: &AiConfig) {
    if !config.privacy.audit_log {
        return;
    }

    let prompt_sha256 = sha256_hex(prompt);
    tracing::info!(
        "provider_ai_audit provider={} model={} prompt_sha256={} prompt_bytes={} redact_heap_path={} redact_pattern_count={}",
        config.provider,
        config.model,
        prompt_sha256,
        prompt.len(),
        config.privacy.redact_heap_path,
        config.privacy.redact_patterns.len(),
    );
}
```

Keep the log content strictly metadata-only. Do not include `prompt`, `response`, `heap_path`, regex values, or rendered instruction text.

- [ ] **Step 3: Call the helper after redaction and before `complete_llm(...)`**

Wire it in this order:

```rust
let prompt = redact_provider_prompt(build_provider_toon_prompt(summary, leaks, config)?, config)?;
emit_provider_audit_log(&prompt, config);
let response = complete_llm(&LlmCompletionRequest {
    prompt: prompt.clone(),
    config: config.clone(),
})?;
```

This preserves the privacy boundary: the audit log hashes the redacted outbound prompt, not the pre-redaction prompt.

- [ ] **Step 4: Run the provider audit-log integration test and verify it passes**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content -- --exact --nocapture
```

Expected: PASS, with stderr showing the audit marker but no raw secret text.

### Task 4: Format, Re-verify, And Sync Docs

**Files:**
- Modify: `docs/configuration.md`
- Modify: `README.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`

- [ ] **Step 1: Format the workspace**

Run:

```bash
cargo fmt --all
```

- [ ] **Step 2: Run the focused verification set**

Run:

```bash
cargo test -p mnemosyne-cli --bin mnemosyne-cli config_loader::tests::parses_ai_provider_privacy_config -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content -- --exact --nocapture
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_redacts_prompt_before_send -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_redacts_prompt_before_send -- --exact --nocapture
cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_rejects_invalid_redaction_pattern -- --exact --nocapture
cargo test -p mnemosyne-core --lib mcp::server -- --nocapture
cargo test -p mnemosyne-cli --test integration serve_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Update docs to reflect the second Step 14(d) slice**

Document:

```toml
[ai.privacy]
redact_heap_path = true
redact_patterns = ["secret-token-[0-9]+"]
audit_log = true
```

Clarify that audit logging records hashed metadata for the redacted outbound provider prompt and intentionally does not log raw prompt text.
