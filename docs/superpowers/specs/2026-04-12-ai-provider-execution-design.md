# AI Provider Execution Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: first external-provider execution slice after the rule-based AI task runner

## Goal

Wire Mnemosyne's existing configurable AI task runner to a real external provider so `analyze --ai` and `explain` can return provider-backed `AiInsights` while preserving the current CLI, MCP, report, and TOON wire contracts.

## Why This Slice

The rule-based task runner is now in place, but `STATUS.md` still lists external LLM-backed execution as a remaining must-have. The smallest honest next step is not full M5. It is a provider-backed execution path that:

- keeps `rules` as the default safe mode
- adds an explicit provider-backed mode
- supports one real protocol first: OpenAI-compatible chat completions
- returns real errors for misconfiguration, unsupported providers, and malformed model output
- avoids broad chat, streaming, privacy, or fix-generation scope expansion in this batch

## Chosen Approach

Add `AiMode::Provider` and route it through a small LLM transport layer that talks to an OpenAI-compatible endpoint using blocking HTTP.

The AI path will remain synchronous for this slice. That keeps the blast radius small because the current analysis pipeline is already synchronous internally even when called from async CLI/MCP entry points. If later M5 slices need streaming or conversation mode, the transport can be converted to async then.

Provider mode will:

1. build a stricter TOON prompt that includes the analysis context and the required output schema
2. call an OpenAI-compatible `chat/completions` endpoint
3. parse the returned TOON payload into `AiInsights`
4. fail honestly if the provider call or TOON parsing fails

## Scope

- add provider-backed AI execution for `AiMode::Provider`
- support OpenAI-compatible chat completions first
- support `AiProvider::OpenAi`
- support `AiProvider::Local` through the same OpenAI-compatible transport when a custom endpoint is configured
- extend AI config with runtime/provider fields needed for real calls
- preserve `AiInsights`, `AiWireExchange`, `AiWireFormat::Toon`, CLI `--ai`, MCP `explain_leak`, and report output shapes

## Non-Scope

- no YAML prompt-template loader yet
- no Anthropic transport in this batch
- no MCP streaming or session management
- no AI-driven fix suggestion rewrite yet
- no privacy/redaction layer yet
- no conversation/chat mode yet
- no browser/web UI work yet

## Architecture Overview

### AI modes

- `stub`
  - old deterministic fallback path
- `rules`
  - current configurable rule-engine task runner
- `provider`
  - provider-backed execution path using the configured transport

### Transport layer

Add a small internal `llm` module with:

- request struct containing endpoint, model, temperature, max tokens, prompt, and optional API key
- response struct containing provider model name and text content
- one completion function that dispatches by `AiProvider`

For this slice:

- `OpenAi` uses the default OpenAI chat-completions endpoint unless overridden
- `Local` uses a configured OpenAI-compatible endpoint
- `Anthropic` returns an explicit unsupported error

### Prompt/response contract

Provider mode keeps TOON as the machine contract.

The prompt will require the model to return only:

- `TOON v1`
- `section response`
  - `model`
  - `confidence_pct`
  - `summary`
- `section recommendations`
  - repeated `item#N`

Mnemosyne will parse this response and map it back into `AiInsights`.

## Config Model Changes

Extend `AiConfig` with the minimum runtime fields needed for real calls:

- `mode = rules | stub | provider`
- `endpoint` — optional endpoint override
- `api_key_env` — optional environment-variable name override
- `max_tokens` — optional response budget
- `timeout_secs` — request timeout

Resolution rules:

- `OpenAi`
  - default endpoint: OpenAI chat completions
  - default API key env: `OPENAI_API_KEY`
- `Local`
  - requires explicit `endpoint`
  - API key optional
- `Anthropic`
  - accepted in config, but execution returns an explicit unsupported error until a transport lands

## Error Handling

This batch must fail honestly instead of silently falling back.

Provider mode returns errors for:

- missing API key when the selected provider requires one
- missing endpoint for local provider mode
- unsupported provider implementation
- transport failures
- malformed HTTP response bodies
- malformed TOON output from the model

`rules` and `stub` modes continue to work without network dependencies.

## File Impact

- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`
- Modify: `core/src/analysis/ai.rs`
- Add: `core/src/llm.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/src/analysis/engine.rs`
- Modify: `core/src/mcp/server.rs`
- Modify: `cli/src/main.rs`
- Modify: `core/Cargo.toml`
- Modify: `docs/configuration.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`

## Testing Strategy

Use TDD.

1. Add config-loader coverage for provider-mode runtime fields.
2. Add AI unit coverage for:
   - successful provider-backed TOON parsing from a mock OpenAI-compatible server
   - honest error on missing API key / missing endpoint
3. Re-run focused CLI and MCP contract tests to prove the existing response shapes stay intact.

## Risks

- Model output may drift from the TOON contract. Mitigation: keep the prompt strict and reject malformed responses.
- Blocking HTTP could become a limitation for future streaming/chat work. Mitigation: keep the transport layer isolated so later slices can switch to async.
- Adding too much provider abstraction now would slow delivery. Mitigation: implement OpenAI-compatible transport first and defer broader provider coverage.

## Decision

Implement `AiMode::Provider` with a small OpenAI-compatible transport now, preserve `rules` as the default path, and defer prompt-template files, Anthropic support, privacy controls, chat, and streaming to later M5 slices.
