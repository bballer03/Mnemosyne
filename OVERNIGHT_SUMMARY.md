# Overnight Summary

## Completed Milestones

### 1. Step 11 large-dump scaling validation

Completed end-to-end.

What changed:
- redesigned `scripts/java/SyntheticHeapApp.java` from sparse chunk-heavy heaps to dense, high-cardinality retained object clusters
- added real generator acceptance coverage:
  - `scripts/tests/test_generate_synthetic_heap_density.sh`
  - `scripts/tests/test_generate_synthetic_heap_growth.sh`
- kept existing Step 11 wrapper and RSS tooling contracts working
- calibrated dense-generator request values to real dump tiers:
  - `320` -> `~494 MB`
  - `640` -> `~982 MB`
  - `1260` -> `~1.88 GB`
- ran the full validation batch into disk-backed storage at `/mnt/d/mn-step11-runs-dense`

Decision:
- PASS - No action
- default `analyze` / `leaks` stayed at `2.87x-2.90x`
- investigation `analyze --threads --strings --collections` stayed at `3.89x-3.92x`
- current in-memory `ObjectGraph` architecture cleared the Step 11 gate for the current roadmap scope

Docs updated:
- `docs/performance/memory-scaling.md`
- `docs/design/memory-scaling.md`
- `STATUS.md`
- `docs/roadmap.md`

### 2. Configurable AI task runner (rule-engine slice)

Completed the next `STATUS.md` must-have slice after Step 11.

What changed:
- extended `AiConfig` with:
  - `mode`
  - ordered `tasks`
- added AI task config types:
  - `AiMode`
  - `AiTaskDefinition`
  - `AiTaskKind`
- taught `cli/src/config_loader.rs` to parse:
  - `mode`
  - `[[ai.tasks]]`
  - `[[llm.tasks]]`
- rewired `core/src/analysis/ai.rs` so `generate_ai_insights()` now dispatches by mode:
  - `stub`
  - `rules`
- implemented built-in rule tasks:
  - `TopLeak`
  - `HealthyHeap`
  - `RemediationChecklist`
- preserved the existing outward AI response shape:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report consumers

Tests added/updated:
- config loader unit test for AI task-runner parsing
- AI unit tests for configured-task behavior and task disabling
- CLI integration test for `analyze --ai --format json` AI section presence

Docs updated:
- `STATUS.md`
- `docs/roadmap.md`
- design spec + plan written under `docs/superpowers/`

### 3. External-provider AI execution (OpenAI-compatible first slice)

Completed the next remaining AI must-have immediately after the rule-engine slice.

What changed:
- extended `AiConfig` with provider runtime fields:
  - `endpoint`
  - `api_key_env`
  - `max_tokens`
  - `timeout_secs`
- added `AiMode::Provider`
- added env override support for:
  - `MNEMOSYNE_AI_MODE`
  - `MNEMOSYNE_AI_ENDPOINT`
  - `MNEMOSYNE_AI_API_KEY_ENV`
  - `MNEMOSYNE_AI_MAX_TOKENS`
  - `MNEMOSYNE_AI_TIMEOUT_SECS`
- added `core/src/llm.rs` with a small OpenAI-compatible chat-completions transport
- rewired `generate_ai_insights()` to return `CoreResult<AiInsights>` and support:
  - `rules`
  - `stub`
  - `provider`
- added strict TOON provider prompting + TOON response parsing back into the existing `AiInsights` schema
- added async boundary handling with `spawn_blocking` at CLI/MCP/core async call sites so provider mode works under Tokio without panicking
- kept existing outward contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes

Current provider support after this slice:
- `openai`: supported via OpenAI-compatible chat completions
- `local`: supported when `endpoint` points at an OpenAI-compatible local server
- `anthropic`: transport code has since been added in `core/src/llm.rs` and targeted verification now passes in both core and CLI integration coverage

Tests added/updated:
- config loader unit test for provider-mode config parsing
- AI unit test for missing API key behavior
- AI unit test for parsing a provider TOON response from a mock OpenAI-compatible server
- CLI integration regression covering `analyze --ai --format json` through provider mode

Docs updated:
- `STATUS.md`
- `docs/roadmap.md`
- `docs/configuration.md`
- `README.md`
- `ARCHITECTURE.md`
- `docs/api.md`
- design spec + plan written under `docs/superpowers/`

### 4. Configurable provider prompt templates

Completed the first Step 14 hardening slice immediately after provider-mode execution landed.

What changed:
- extended `AiConfig` with nested prompt settings:
  - `prompts.template_dir`
- added env override support for:
  - `MNEMOSYNE_AI_PROMPT_TEMPLATE_DIR`
- added `core/src/prompts/` with:
  - embedded default `provider-insights.yaml`
  - YAML parsing for provider instruction entries
  - small placeholder rendering for provider prompt context
- rewired provider-mode prompt construction so `section instructions` now renders from YAML instead of a hardcoded builder
- kept existing outward contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes
- provider mode now fails explicitly if a configured prompt override directory is unreadable or contains invalid YAML

Tests added/updated:
- CLI integration regression for `mnemosyne config` JSON including `ai.prompts.template_dir`
- CLI integration regression proving provider mode sends YAML template override instructions through `ai.wire.prompt`
- re-verified provider AI unit tests and the existing provider CLI regression

Docs updated:
- `STATUS.md`
- `docs/roadmap.md`
- `docs/configuration.md`
- `README.md`
- `ARCHITECTURE.md`
- `CHANGELOG.md`
- design spec + plan written under `docs/superpowers/`

### 5. MCP protocol hardening (Step 14(c) first slice)

Completed the smallest honest MCP hardening slice after prompt-template and Anthropic follow-through.

What changed:
- extended the stdio MCP surface with a new discovery method:
  - `list_tools`
- added machine-readable `error_details` to failed MCP responses while preserving the existing top-level string `error` field for backward compatibility
- mapped `CoreError` variants into stable MCP error codes such as:
  - `invalid_input`
  - `config_error`
  - `file_not_found`
  - `hprof_parse_error`
  - `invalid_params`
  - `internal_error`
- added server-owned tool descriptions and parameter metadata for the live MCP handlers so clients can discover the current method surface directly from the server
- kept streaming out of scope for now because the current stdio line-delimited transport does not yet justify the extra protocol complexity

Tests added/updated:
- `core::mcp::server::tests::handle_request_list_tools_returns_descriptions`
- `core::mcp::server::tests::rpc_response_error_includes_structured_details`

Docs updated:
- `docs/api.md`
- `docs/examples/README.md`
- `README.md`
- `ARCHITECTURE.md`
- `STATUS.md`
- `docs/roadmap.md`

### 6. Provider-mode privacy controls (Step 14(d) first slice)

Completed the smallest honest privacy slice immediately after MCP protocol hardening.

What changed:
- extended `AiConfig` with nested privacy settings:
  - `privacy.redact_heap_path`
  - `privacy.redact_patterns`
- added TOML + env override support for:
  - `[ai.privacy]`
  - `MNEMOSYNE_AI_REDACT_HEAP_PATH`
  - `MNEMOSYNE_AI_REDACT_PATTERNS`
- rewired provider-mode prompt handling so redaction now runs after the full provider prompt is rendered, but before the external provider call is made
- `redact_heap_path` now replaces outbound TOON `heap_path=...` values with `<REDACTED>`
- `redact_patterns` now applies regex replacement across the fully rendered outbound provider prompt, including YAML-rendered instruction text
- invalid privacy regex patterns now fail explicitly with `CoreError::InvalidInput`
- `AiWireExchange.prompt` now records the redacted prompt that was actually sent to the provider
- kept existing outward contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes

Tests added/updated:
- `cli::config_loader::tests::parses_ai_provider_privacy_config`
- `core::analysis::ai::tests::provider_mode_redacts_prompt_before_send`
- `core::analysis::ai::tests::provider_mode_rejects_invalid_redaction_pattern`
- `cli/tests/integration.rs::test_analyze_json_with_provider_mode_ai_redacts_prompt_before_send`

Docs updated:
- `docs/configuration.md`
- `README.md`
- `ARCHITECTURE.md`
- `STATUS.md`

### 7. MCP AI sessions (Step 14(e) follow-through)

Completed the MCP session-backed conversation/context slice after the CLI-first chat and AI-backed fix-generation work.

What changed:
- extended `AiConfig` with:
  - `[ai.sessions].directory`
- added a new MCP-local persisted session store:
  - `core/src/mcp/session.rs`
- added explicit MCP lifecycle methods:
  - `create_ai_session`
  - `resume_ai_session`
  - `get_ai_session`
  - `close_ai_session`
  - `chat_session`
- sessions now persist:
  - `heap_path`
  - analysis summary
  - leak list
  - top-3 shortlist leak IDs
  - current focus leak ID
  - bounded recent `AiChatTurn` history
- resumed MCP AI follow-up now works across server restarts for:
  - `chat_session`
  - `explain_leak` via `session_id`
  - `propose_fix` via `session_id`
- preserved existing contracts:
  - `AiInsights`
  - `AiWireExchange`
  - `AiWireFormat::Toon`
  - `FixRequest`
  - `FixSuggestion`
  - `FixResponse`
  - existing `heap_path`-based `explain_leak` / `propose_fix`

Tests added/updated:
- `config_loader::tests::parses_ai_session_directory_config`
- `mcp::session::tests::session_store_round_trips_persisted_session`
- `mcp::session::tests::append_turn_trims_history_to_three_entries`
- `mcp::server::tests::handle_request_create_ai_session_returns_session_metadata`
- `mcp::server::tests::handle_request_resume_ai_session_reads_persisted_state`
- `mcp::server::tests::handle_request_close_ai_session_removes_persisted_state`
- `mcp::server::tests::handle_request_chat_session_updates_history_and_focus`
- `mcp::server::tests::handle_request_explain_leak_supports_session_id`
- `mcp::server::tests::handle_request_propose_fix_supports_session_id`
- `mcp::server::tests::handle_request_explain_leak_rejects_conflicting_context_sources`
- `mcp::server::tests::handle_request_list_tools_includes_ai_session_methods`

Verification:
- `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_session_directory_config -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::session::tests::session_store_round_trips_persisted_session --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::session::tests::append_turn_trims_history_to_three_entries --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_create_ai_session_returns_session_metadata --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_resume_ai_session_reads_persisted_state --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_close_ai_session_removes_persisted_state --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_chat_session_updates_history_and_focus --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_explain_leak_supports_session_id --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_propose_fix_supports_session_id --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_explain_leak_rejects_conflicting_context_sources --lib -- --exact --nocapture`
- `cargo test -p mnemosyne-core mcp::server::tests::handle_request_list_tools_includes_ai_session_methods --lib -- --exact --nocapture`
- `docs/roadmap.md`
- `OVERNIGHT_SUMMARY.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`

### 7. Provider-mode privacy audit logging (Step 14(d) second slice)

Completed the next smallest privacy follow-through after prompt redaction.

What changed:
- extended `[ai.privacy]` with:
  - `audit_log`
- added env override support for:
  - `MNEMOSYNE_AI_AUDIT_LOG`
- added provider-mode audit logging immediately before external model calls, after prompt redaction has already been applied
- audit events now record hashed metadata only for the redacted outbound provider prompt:
  - provider
  - model
  - SHA-256 prompt hash
  - prompt byte length
  - redaction flags/counts
- audit events intentionally do not log:
  - raw prompt text
  - raw response text
  - regex values
  - original heap path
- kept existing outward contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes

Tests added/updated:
- extended `cli::config_loader::tests::parses_ai_provider_privacy_config` to cover `audit_log`
- added `cli/tests/integration.rs::test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content`

Docs updated:
- `docs/configuration.md`
- `README.md`
- `STATUS.md`
- `docs/roadmap.md`
- `OVERNIGHT_SUMMARY.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`

### 8. Provider prompt-budget guard (Step 14(d) third slice)

Completed the remaining small privacy/safety follow-through under Step 14(d).

What changed:
- reused existing `ai.max_tokens` rather than adding a new config field
- provider-mode prompt construction now treats very small `max_tokens` values as a conservative prompt-budget guard before the external call
- Mnemosyne now trims leak-context detail first instead of truncating the instruction section
- when truncation happens, the outbound provider prompt now includes:
  - `context_truncated=true`
- the provider instruction section remains intact so the TOON response contract stays explicit even under a tight budget
- kept existing outward contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes

Tests added/updated:
- added `core::analysis::ai::tests::provider_mode_truncates_leak_context_when_max_tokens_is_small`
- re-verified provider redaction, audit logging, and prompt-template override regressions against the new prompt builder behavior

Docs updated:
- `docs/configuration.md`
- `README.md`
- `STATUS.md`
- `docs/roadmap.md`
- `OVERNIGHT_SUMMARY.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`

### 9. CLI-first AI conversation mode (Step 14(e) first slice)

Completed the first conversation-mode slice after the provider, prompt-template, privacy, and MCP protocol groundwork stabilized.

What changed:
- added `mnemosyne-cli chat <heap.hprof>`
- chat startup now analyzes the heap once, prints the analyzed heap path, and shows the top 3 leak candidates
- free-form follow-up questions reuse the shared `rules` / `stub` / `provider` AI pipeline instead of creating a second AI response contract
- chat supports:
  - `/focus <leak-id>`
  - `/list`
  - `/help`
  - `/exit`
- unfocused follow-up turns now stay bounded to the top-3 shortlist instead of widening to the full leak set
- focused turns narrow to the selected leak identifier using the same leak-ID matching semantics already used by `explain`
- chat startup now respects the normal `[analysis]` filters instead of widening severity locally; if no leaks survive those filters, chat remains available against the healthy-heap context
- conversation history is in-process only and keeps the most recent 3 completed turns
- provider-mode chat prompts reuse the same prompt-template, redaction, hashed audit logging, and minimal prompt-budget guard path already shipped for `explain`
- kept outward AI contracts stable:
  - `AiInsights`
  - `AiWireExchange`
  - TOON wire format
  - CLI/MCP/report AI response shapes

Tests added/updated:
- `core::analysis::ai::tests::provider_chat_prompt_includes_question_focus_and_recent_history`
- `core::analysis::ai::tests::provider_chat_prompt_trims_history_before_selected_leak_context`
- `core::analysis::ai::tests::rules_chat_focus_uses_selected_leak_for_answer`
- `cli/tests/integration.rs::test_chat_starts_with_shortlist_and_help`
- `cli/tests/integration.rs::test_chat_answers_a_question_in_rules_mode`
- `cli/tests/integration.rs::test_chat_focuses_on_selected_leak`
- `cli/tests/integration.rs::test_chat_rejects_invalid_focus_target_and_continues`
- `cli/tests/integration.rs::test_chat_bare_focus_prints_usage_and_does_not_ask_ai`
- `cli/tests/integration.rs::test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response`
- `cli/tests/integration.rs::test_chat_default_config_reports_healthy_heap_when_fallback_leaks_are_below_threshold`

Docs updated:
- `README.md`
- `ARCHITECTURE.md`
- `STATUS.md`
- `docs/roadmap.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`
- `OVERNIGHT_SUMMARY.md`
- `docs/QUICKSTART.md`
- `docs/configuration.md`
- `CHANGELOG.md`

## Major Architectural Decisions

1. Step 11 synthetic heaps should be validated against graph-backed object metrics, not `parse`'s top-level record estimate.
2. Dense synthetic validation is sufficient to close Step 11 for the current roadmap gate, while still leaving room for future real-world large-heap validation.
3. The next AI milestone slice should be the smallest honest step: configurable rule-based task execution first, external provider execution second.
4. Existing `AiInsights` / CLI / MCP / TOON contracts were preserved rather than redesigned.
5. Provider prompt hardening should externalize only the instruction layer first; the TOON request context builder remains embedded until privacy/token-budget work is ready.
6. Provider-mode privacy redaction should run after the full provider prompt is assembled, because sensitive content can come from either TOON request fields or YAML-rendered provider instructions.
7. Provider-mode audit logging should hash the already-redacted outbound prompt and log metadata only, so governance improves without creating a second prompt-leak surface in logs.
8. The first prompt-budget guard should reuse `max_tokens`, preserve the instruction section, and trim only leak context so provider behavior stays deterministic under tight budgets.
9. Conversation mode should ship CLI-first, leak-focused, and in-process before any MCP/session semantics are added.

## Verification Run

Verified successfully during the overnight run:
- `bash scripts/tests/test_generate_synthetic_heap_density.sh`
- `bash scripts/tests/test_generate_synthetic_heap_growth.sh`
- `bash scripts/tests/test_generate_synthetic_heap.sh`
- `bash scripts/tests/test_measure_rss_short_parse.sh`
- `bash scripts/tests/test_measure_rss_450mb.sh`
- `bash scripts/tests/test_run_step11_scaling_validation.sh`
- `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_task_runner_config --bin mnemosyne-cli -- --exact`
- `cargo test -p mnemosyne-cli config_loader::tests::parses_ai_provider_mode_config -- --exact`
- `cargo test -p mnemosyne-core --lib analysis::ai -- --nocapture`
- `cargo test -p mnemosyne-core --lib mcp::server -- --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_ai_includes_ai_section -- --exact`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_includes_provider_response -- --exact`
- `cargo test -p mnemosyne-cli --test integration test_config_json_includes_ai_prompt_template_dir -- --exact`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_uses_prompt_template_override -- --exact`
- `cargo test -p mnemosyne-cli --bin mnemosyne-cli config_loader::tests::parses_ai_provider_privacy_config -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_emits_audit_log_without_prompt_content -- --exact --nocapture`
- `cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_redacts_prompt_before_send -- --exact --nocapture`
- `cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_truncates_leak_context_when_max_tokens_is_small -- --exact --nocapture`
- `cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_mode_rejects_invalid_redaction_pattern -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_redacts_prompt_before_send -- --exact --nocapture`
- `cargo test -p mnemosyne-core provider_mode_ --lib -- --nocapture`
- `cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_includes_question_focus_and_recent_history -- --exact --nocapture`
- `cargo test -p mnemosyne-core --lib analysis::ai::tests::provider_chat_prompt_trims_history_before_selected_leak_context -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_starts_with_shortlist_and_help -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_answers_a_question_in_rules_mode -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_focuses_on_selected_leak -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_rejects_invalid_focus_target_and_continues -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_bare_focus_prints_usage_and_does_not_ask_ai -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_with_provider_mode_sends_chat_follow_up_prompt_and_renders_response -- --exact --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_chat_default_config_reports_healthy_heap_when_fallback_leaks_are_below_threshold -- --exact --nocapture`
- `cargo check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

## Notable Caveat / Unresolved Blocker

`cargo test -p mnemosyne-core` without additional scoping still trips an unrelated pre-existing integration-test setup issue:
- `core/tests/query_executor.rs` imports `mnemosyne_core::test_fixtures`
- `core/src/lib.rs` only re-exports `test_fixtures` behind `#[cfg(any(test, feature = "test-fixtures"))]`
- plain core integration-test runs therefore fail unless the crate is built with the `test-fixtures` feature or the export strategy changes

This was not introduced by tonight's work. I avoided claiming a fully green unscoped `cargo test -p mnemosyne-core` because the evidence does not support that claim.

## Current Next Step

Next logical milestone from the current roadmap/status:
- remaining Step 14 / M5 hardening after the CLI-first conversation slice

The immediate follow-on candidates are now:
- MCP/session follow-through after the first AI-backed fix-generation slice
- broader provider or transport hardening only where verification shows a concrete gap

## Documentation Sync Follow-Through

After the overnight feature work, a documentation truth pass was started in the Step 11 worktree to realign the major docs with the live code.

What changed in the doc-sync pass:
- recreated `docs/api.md` from scratch as a real MCP/stdin-stdout API reference
- corrected the live MCP method list to:
  - `list_tools`
  - `parse_heap`
  - `detect_leaks`
  - `analyze_heap`
  - `query_heap`
  - `map_to_code`
  - `find_gc_path`
  - `explain_leak`
  - `propose_fix`
- removed stale `apply_fix` claims from user-facing docs
- rewrote `docs/QUICKSTART.md` to the current `mnemosyne-cli` command surface
- rewrote `docs/configuration.md` to match the active config loader and current env overrides
- rewrote `docs/examples/README.md` into a truthful lightweight examples page
- updated `README.md` examples and MCP sections to use `mnemosyne-cli`, the current AI/provider story, and the completed Step 11 scaling numbers
- corrected stale Step 11 wording in `ARCHITECTURE.md`, `docs/roadmap.md`, `docs/design/M3-phase2-analysis.md`, and `docs/release-notes-v0.2.0.md`
- updated the MCP method count to nine where `list_tools` now exists and documented the new `error_details` field

Important documentation truths established during the pass:
- the packaged/runtime binary name is `mnemosyne-cli`
- the live CLI commands are:
  - `parse`
  - `leaks`
  - `analyze`
  - `diff`
  - `map`
  - `gc-path`
  - `query`
  - `explain`
  - `chat`
  - `fix`
  - `serve`
  - `config`
- `mnemosyne-cli chat <heap.hprof>` is the current CLI-only first slice of conversation mode:
  - it analyzes once
  - prints the top 3 leak candidates
  - supports `/focus <leak-id>`, `/list`, `/help`, and `/exit`
  - keeps only the running process' recent history in memory
- there is no global `--quiet`, no global `--format`, no `--no-ai`, no `config show`, and no `config validate`
- `analyze` does not currently expose `--min-severity`
- `analysis.accumulation_threshold` exists in core config/runtime logic, but the current config loader does not yet load it from TOML or environment
- `parser.use_mmap` and `parser.threads` are parsed into config, but are not currently documented as affecting the active CLI runtime path in this branch
- Step 11 is complete, with dense synthetic validation at roughly:
  - `~500 MB`
  - `~1 GB`
  - `~2 GB`
- published Step 11 ratios are now:
  - default path: `2.87x-2.90x`
  - investigation path: `3.89x-3.92x`

Follow-up verification completed after the doc pass:
- `cargo test -p mnemosyne-core --lib provider_mode_ -- --nocapture`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_includes_provider_response -- --exact`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_provider_mode_ai_uses_prompt_template_override -- --exact`
- `cargo test -p mnemosyne-cli --test integration test_analyze_json_with_anthropic_provider_mode_includes_provider_response -- --exact`

That verification closes the earlier Anthropic uncertainty from the interrupted handoff: the first-slice Anthropic provider path is now verified at the targeted core + CLI integration level, even though broader provider/privacy/protocol hardening remains future work.

Files updated in the doc-sync pass:
- `docs/api.md`
- `docs/QUICKSTART.md`
- `docs/configuration.md`
- `docs/examples/README.md`
- `README.md`
- `ARCHITECTURE.md`
- `CONTRIBUTING.md`
- `docs/roadmap.md`
- `docs/design/M3-phase2-analysis.md`
- `docs/release-notes-v0.2.0.md`
