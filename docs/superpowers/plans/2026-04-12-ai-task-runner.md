# AI Task Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single hardcoded AI heuristic with a configurable rule-based AI task runner while preserving current CLI, MCP, and report contracts.

**Architecture:** Extend `AiConfig` with task-runner settings, introduce a task execution layer inside the AI analysis module, and keep `generate_ai_insights()` as the stable entry point that returns the existing `AiInsights` schema. The first batch uses built-in rule tasks rather than external LLM providers so the architecture becomes real without pulling in network dependencies yet.

**Tech Stack:** Rust, serde/TOML config parsing, existing CLI + MCP contracts, unit/integration tests

---

### Task 1: Add Red Tests For AI Task Config Parsing

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`
- Test: `cli/src/config_loader.rs`

- [ ] **Step 1: Write a failing test for parsing AI task definitions from TOML**

Add a test in `cli/src/config_loader.rs` that expects a config such as:

```toml
[ai]
enabled = true
mode = "rules"

[[ai.tasks]]
kind = "top-leak"
enabled = true

[[ai.tasks]]
kind = "healthy-heap"
enabled = false
```

The test should assert that the parsed `cfg.ai` contains:
- mode `rules`
- two task definitions
- the second task disabled

- [ ] **Step 2: Run the focused config-loader test to verify it fails**

Run: `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_task_runner_config -- --exact`

Expected: FAIL because `AiConfig` and `PartialAiConfig` do not yet support task-runner fields.

### Task 2: Add Red Tests For Rule-Based Task Runner Behavior

**Files:**
- Modify: `core/src/analysis/ai.rs`
- Test: `core/src/analysis/ai.rs`

- [ ] **Step 1: Write a failing test for configured top-leak output**

Add a unit test that builds an `AiConfig` with `mode = rules` and tasks including `top-leak`, then asserts that `generate_ai_insights()` includes the top leak's class in its summary and at least one recommendation referencing cleanup/instrumentation.

- [ ] **Step 2: Write a failing test for disabling the top-leak task**

Add a second unit test that disables the `top-leak` task and asserts the resulting summary does not use the top-leak-focused wording.

- [ ] **Step 3: Run the focused AI tests to verify they fail**

Run: `cargo test -p mnemosyne-core generates_summary_with_configured_tasks disabled_top_leak_task_changes_summary -- --exact`

Expected: FAIL because no task-runner configuration or task dispatch exists yet.

### Task 3: Implement AI Task Runner Config

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`
- Test: `cli/src/config_loader.rs`

- [ ] **Step 1: Extend `AiConfig` with task-runner fields**

Add minimal types such as:

```rust
pub struct AiConfig {
    pub enabled: bool,
    pub provider: AiProvider,
    pub model: String,
    pub temperature: f32,
    pub mode: AiMode,
    pub tasks: Vec<AiTaskDefinition>,
}
```

```rust
pub enum AiMode {
    Stub,
    Rules,
}

pub struct AiTaskDefinition {
    pub kind: AiTaskKind,
    pub enabled: bool,
}
```

- [ ] **Step 2: Update TOML/env loader support for the new AI fields**

Implement parsing for:
- `mode`
- `[[ai.tasks]]`
- `[[llm.tasks]]`

Keep existing `[ai]` and `[llm]` compatibility intact.

- [ ] **Step 3: Run the focused config-loader test and make it pass**

Run: `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_task_runner_config -- --exact`

Expected: PASS

### Task 4: Implement Rule-Based Task Execution

**Files:**
- Modify: `core/src/analysis/ai.rs`
- Test: `core/src/analysis/ai.rs`

- [ ] **Step 1: Add a small task-runner layer in `ai.rs`**

Implement internal helpers such as:

```rust
struct AiTaskContext<'a> {
    summary: &'a HeapSummary,
    leaks: &'a [LeakInsight],
    top_leak: Option<&'a LeakInsight>,
    config: &'a AiConfig,
}

struct AiTaskOutput {
    summary_fragment: Option<String>,
    recommendations: Vec<String>,
    confidence_delta: f32,
}
```

and built-in rule handlers for a minimal set of task kinds:
- `TopLeak`
- `HealthyHeap`
- `RemediationChecklist`

- [ ] **Step 2: Make `generate_ai_insights()` dispatch by `AiMode`**

Required behavior:

```rust
match config.mode {
    AiMode::Stub => existing_stub_path(...),
    AiMode::Rules => rule_runner_path(...),
}
```

Keep the returned `AiInsights` schema unchanged.

- [ ] **Step 3: Run the focused AI tests and make them pass**

Run: `cargo test -p mnemosyne-core generates_summary_with_configured_tasks disabled_top_leak_task_changes_summary -- --exact`

Expected: PASS

### Task 5: Re-verify Existing AI Contracts

**Files:**
- Test: `core/src/analysis/ai.rs`
- Test: `core/src/mcp/server.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Re-run all AI unit tests**

Run: `cargo test -p mnemosyne-core analysis::ai -- --nocapture`

Expected: PASS

- [ ] **Step 2: Re-run MCP server tests touching AI output**

Run: `cargo test -p mnemosyne-core mcp::server -- --nocapture`

Expected: PASS

- [ ] **Step 3: Add or re-run one CLI contract test for `analyze --ai` output shape if needed**

Run: `cargo test -p mnemosyne-cli --test integration -- --nocapture`

Expected: PASS with `--ai` behavior still returning the existing response shape.
