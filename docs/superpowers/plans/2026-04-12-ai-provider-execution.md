# AI Provider Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real external-provider AI execution mode using an OpenAI-compatible endpoint while preserving the existing CLI, MCP, report, and TOON response contracts.

**Architecture:** Extend `AiConfig` with provider runtime fields, add a small internal LLM transport module, keep `rules` as the default AI mode, and make `provider` mode call a real endpoint that returns a TOON payload parsed back into `AiInsights`. Use blocking HTTP in this slice to avoid unnecessary async churn across the current synchronous analysis pipeline.

**Tech Stack:** Rust, reqwest blocking client, serde/serde_json, existing CLI + MCP contracts, unit and integration tests

---

### Task 1: Add Red Tests For Provider Config Parsing

**Files:**
- Modify: `cli/src/config_loader.rs`
- Modify: `core/src/config.rs`

- [ ] Add a failing config-loader test for provider mode and runtime fields.
- [ ] Run: `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_provider_mode_config -- --exact`
- [ ] Expected: fail because `AiMode::Provider` and provider runtime fields do not exist yet.

### Task 2: Add Red Tests For Provider-Backed AI Execution

**Files:**
- Modify: `core/src/analysis/ai.rs`

- [ ] Add a failing AI unit test that stands up a minimal mock OpenAI-compatible HTTP server, configures `mode = provider`, and asserts that a valid TOON response is parsed into `AiInsights`.
- [ ] Add a failing AI unit test asserting provider mode returns an error when the required API key is missing.
- [ ] Run: `cargo test -p mnemosyne-core provider_mode_ --lib -- --nocapture`
- [ ] Expected: fail because provider execution and error handling are not implemented.

### Task 3: Implement Provider Runtime Config

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`

- [ ] Add `AiMode::Provider` plus the minimum runtime fields: `endpoint`, `api_key_env`, `max_tokens`, `timeout_secs`.
- [ ] Extend TOML/env parsing to populate those fields from `[ai]` / `[llm]`.
- [ ] Re-run the focused config-loader test until it passes.

### Task 4: Implement OpenAI-Compatible Transport

**Files:**
- Add: `core/src/llm.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/Cargo.toml`

- [ ] Add a small transport layer for OpenAI-compatible chat completions.
- [ ] Support `AiProvider::OpenAi` and `AiProvider::Local` first.
- [ ] Return explicit unsupported errors for `AiProvider::Anthropic`.
- [ ] Keep the transport isolated so later M5 slices can add async/streaming without rewriting the AI contract layer.

### Task 5: Wire Provider Mode Into AI Generation

**Files:**
- Modify: `core/src/analysis/ai.rs`
- Modify: `core/src/analysis/engine.rs`
- Modify: `core/src/mcp/server.rs`
- Modify: `cli/src/main.rs`

- [ ] Change `generate_ai_insights()` to return `CoreResult<AiInsights>`.
- [ ] Keep `stub` and `rules` behavior intact.
- [ ] Add strict TOON prompt generation for provider mode.
- [ ] Parse provider TOON output back into `AiInsights`.
- [ ] Propagate provider errors honestly through `analyze`, `explain`, and MCP `explain_leak`.

### Task 6: Verify Contracts And Update Docs

**Files:**
- Modify: `cli/tests/integration.rs`
- Modify: `docs/configuration.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`

- [ ] Re-run focused AI unit tests.
- [ ] Re-run focused MCP and CLI contract tests.
- [ ] Run `cargo check`.
- [ ] Update docs to describe the new provider mode, its config fields, current provider support, and the remaining M5 gaps honestly.
