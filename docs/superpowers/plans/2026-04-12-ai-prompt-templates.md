# AI Prompt Templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Externalize provider-mode prompt instructions into YAML with an embedded default template and a config-driven override directory, while preserving the existing TOON response contract.

**Architecture:** Add a small internal prompt module under `core/src/prompts/`, extend `AiConfig` with nested prompt settings, and keep `build_provider_toon_prompt()` as the only caller that renders template instructions into the existing TOON request body. The request/leak context builder stays embedded; only the provider instruction layer becomes configurable in this slice.

**Tech Stack:** Rust, serde/TOML config parsing, serde_yaml, existing CLI + provider integration tests

---

### Task 1: Add Red Integration Tests For Prompt Config And Override Behavior

**Files:**
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Write a failing `config` integration test for `[ai.prompts].template_dir`**

Add a test that writes a config file containing:

```toml
[ai]
enabled = true

[ai.prompts]
template_dir = "/tmp/mnemosyne-prompts"
```

Run `mnemosyne --config <file> config`, parse the JSON output, and assert `json["ai"]["prompts"]["template_dir"]` matches the configured path.

- [ ] **Step 2: Write a failing provider integration test for YAML prompt override**

Add a test that:

1. creates a temporary prompt directory
2. writes `provider-insights.yaml`
3. runs `mnemosyne analyze <fixture> --format json --ai` with `mode = "provider"`
4. asserts `ai.wire.prompt` contains a template-defined instruction line not present in the embedded default

- [ ] **Step 3: Run the focused CLI integration tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_config_json_includes_ai_prompt_template_dir -- --exact
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_uses_prompt_template_override -- --exact
```

Expected: FAIL because prompt settings and YAML overrides do not exist yet.

### Task 2: Add Prompt Configuration Types And Loader Support

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`

- [ ] **Step 1: Add nested prompt settings to `AiConfig`**

Add minimal config types:

```rust
pub struct AiConfig {
    // existing fields...
    pub prompts: AiPromptConfig,
}

pub struct AiPromptConfig {
    pub template_dir: Option<String>,
}
```

- [ ] **Step 2: Teach the TOML loader to parse `[ai.prompts]` and `[llm.prompts]`**

Add nested partial config types and merge logic so `template_dir` flows into `cfg.ai.prompts.template_dir`.

- [ ] **Step 3: Add the matching environment override**

Support:

```bash
MNEMOSYNE_AI_PROMPT_TEMPLATE_DIR=/path/to/prompts
```

- [ ] **Step 4: Re-run the focused `config` integration test and make it pass**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_config_json_includes_ai_prompt_template_dir -- --exact
```

Expected: PASS

### Task 3: Implement YAML Prompt Template Loading

**Files:**
- Modify: `Cargo.toml`
- Modify: `core/Cargo.toml`
- Add: `core/src/prompts/mod.rs`
- Add: `core/src/prompts/defaults/provider-insights.yaml`
- Modify: `core/src/lib.rs`

- [ ] **Step 1: Add `serde_yaml` dependency**

Add `serde_yaml` to the workspace and `mnemosyne-core` dependencies.

- [ ] **Step 2: Add the embedded default YAML template file**

Create `core/src/prompts/defaults/provider-insights.yaml` with ordered instruction entries for the current provider prompt behavior.

- [ ] **Step 3: Add a small prompt module**

Implement:

```rust
pub struct ProviderPromptContext<'a> {
    pub model: &'a str,
    pub provider: &'a str,
    pub heap_path: &'a str,
    pub total_bytes: u64,
    pub total_objects: u64,
    pub leak_sampled: usize,
}

pub fn render_provider_instructions(
    config: &AiConfig,
    context: &ProviderPromptContext<'_>,
) -> CoreResult<Vec<(String, String)>>
```

The module should:

- load the embedded default when no `template_dir` is configured
- load `<template_dir>/provider-insights.yaml` when configured
- parse YAML into ordered key/value instruction entries
- render placeholders like `{{model}}` and `{{heap_path}}`
- return `CoreError::ConfigError` when a configured file is missing or invalid

### Task 4: Wire Provider Prompt Rendering Into AI

**Files:**
- Modify: `core/src/analysis/ai.rs`

- [ ] **Step 1: Replace the hardcoded instruction section with prompt-module output**

Update `build_provider_toon_prompt()` so it still begins with `build_toon_prompt(summary, leaks)`, then appends:

```rust
body.push_str("section instructions\n");
for (key, value) in render_provider_instructions(config, &context)? {
    push_kv(&mut body, 2, &key, value);
}
```

- [ ] **Step 2: Propagate the new `CoreResult<String>` return shape if needed**

If prompt rendering can now fail, update the provider path so config/template failures bubble up honestly through `generate_provider_ai_insights()`.

- [ ] **Step 3: Re-run the prompt-override integration test and make it pass**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_uses_prompt_template_override -- --exact
```

Expected: PASS

### Task 5: Re-verify Existing Provider Contracts

**Files:**
- Test: `core/src/analysis/ai.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Re-run focused provider unit tests**

Run:

```bash
cargo test -p mnemosyne-core provider_mode_ --lib -- --nocapture
```

Expected: PASS

- [ ] **Step 2: Re-run the existing provider CLI integration test**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_includes_provider_response -- --exact
```

Expected: PASS

- [ ] **Step 3: Run the focused prompt-template coverage together**

Run:

```bash
cargo test -p mnemosyne-cli --test integration test_config_json_includes_ai_prompt_template_dir -- --exact
cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_uses_prompt_template_override -- --exact
```

Expected: PASS

### Task 6: Validate The Batch And Sync Docs

**Files:**
- Modify: `STATUS.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `docs/configuration.md`
- Modify: `docs/roadmap.md`
- Modify: `CHANGELOG.md`
- Modify: `OVERNIGHT_SUMMARY.md`

- [ ] **Step 1: Run compilation, lint, and format validation**

Run:

```bash
cargo check
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: PASS, or document any pre-existing unrelated failure honestly.

- [ ] **Step 2: Update docs to reflect shipped prompt-template support**

Document:

- `[ai.prompts].template_dir`
- embedded-default plus override behavior
- provider-mode prompt template support as partial Step 14 hardening

- [ ] **Step 3: Update overnight status for the next slice**

Record this batch as complete, then identify the next Step 14 item to continue immediately afterward.
