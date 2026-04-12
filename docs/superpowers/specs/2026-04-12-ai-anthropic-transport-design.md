# AI Anthropic Transport Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: Step 14(b) Anthropic provider transport only

## Goal

Add a real Anthropic provider transport to provider mode while preserving the existing `AiInsights`, TOON wire exchange, YAML prompt-template path, and CLI/MCP/report response contracts.

## Why This Slice

Step 14(b) in `docs/roadmap.md` calls for a second real provider path after the OpenAI-compatible transport. The current code already parses `provider = "anthropic"` and resolves the default Anthropic endpoint and API key environment variable, but execution still returns an explicit unsupported error.

The smallest correct next slice is:

- keep provider mode semantics unchanged
- keep the TOON prompt/response contract unchanged
- add a blocking Anthropic messages API transport in `core::llm`
- preserve the existing async `spawn_blocking` isolation at call sites

## Non-Goals

- No MCP error-contract redesign in this batch
- No privacy/redaction work in this batch
- No streaming support in this batch
- No conversation mode in this batch
- No Anthropic-specific prompt-template branching yet; the current provider prompt template stays shared

## Chosen Approach

Implement Anthropic support inside the existing `core::llm` module using the blocking `reqwest` client already in use for the OpenAI-compatible path.

Provider dispatch will become:

- `OpenAi` and `Local` -> current OpenAI-compatible chat-completions path
- `Anthropic` -> new Anthropic messages path

The request body will send the rendered TOON prompt as a single user message. The response parser will extract returned text content and pass it through the existing TOON response parser in `core::analysis::ai`.

## API Shape

Anthropic transport will target:

- endpoint default: `https://api.anthropic.com/v1`
- path: `/messages`
- headers:
  - `x-api-key: <key>`
  - `anthropic-version: 2023-06-01`

Minimal request body:

```json
{
  "model": "claude-3-5-sonnet-latest",
  "max_tokens": 2000,
  "temperature": 0.3,
  "messages": [
    {
      "role": "user",
      "content": "TOON v1 ..."
    }
  ]
}
```

Minimal supported response extraction:

- read `content[]`
- concatenate `text` segments in order
- reject empty text content explicitly

## Configuration Model

No new config fields are required.

Existing fields remain the source of truth:

- `provider = "anthropic"`
- `model`
- `api_key_env` or default `ANTHROPIC_API_KEY`
- `endpoint`
- `max_tokens`
- `timeout_secs`
- `temperature`

## Contract Preservation

This batch must preserve:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`
- CLI `--ai`
- MCP AI response shapes
- YAML prompt-template behavior from the previous slice

The only intentional behavior change is that `provider = "anthropic"` now executes a real request instead of returning `Unsupported`.

## Testing Strategy

Use TDD.

1. Replace the current unsupported-provider expectation with a failing unit test that exercises a mock Anthropic server.
2. Add a CLI integration test for `analyze --ai --format json` through `provider = "anthropic"`.
3. Re-run the existing provider-mode coverage to keep OpenAI-compatible behavior stable.

## File Impact

- Modify: `core/src/llm.rs`
- Possibly modify: `core/src/analysis/ai.rs` only if provider tests need small config fixture updates
- Modify: `cli/tests/integration.rs`
- Modify: `docs/configuration.md`
- Modify: `STATUS.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `docs/roadmap.md`
- Modify: `CHANGELOG.md`
- Modify: `OVERNIGHT_SUMMARY.md`

## Risks

- Anthropic response content can contain multiple text blocks; empty or non-text-only content must fail explicitly.
- Defaulting `max_tokens` too low could produce clipped TOON responses, so the transport should honor configured values and keep a sane fallback when omitted.
- Over-scoping into shared abstraction layers would slow this slice; the current `core::llm` file is still the smallest safe place for the change.

## Decision

Implement the Anthropic messages transport as a minimal second provider path inside `core::llm`, reuse the current prompt-template and TOON parsing layers, and defer broader provider abstraction work until a later hardening slice.
