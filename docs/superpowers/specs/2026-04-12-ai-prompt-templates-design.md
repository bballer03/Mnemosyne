# AI Prompt Templates Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: Step 14(a) configurable provider prompt templates only

## Goal

Externalize the provider-mode prompt instructions into YAML templates while preserving the existing `AiInsights`, `AiWireExchange`, TOON request shape, and CLI/MCP/report response contracts.

## Why This Slice

The first provider-backed AI slice made real external execution possible, but the provider prompt is still hardcoded inside `core/src/analysis/ai.rs`. Step 14(a) in `docs/roadmap.md` calls out configurable prompt templates as the next smallest M5 hardening slice.

The smallest honest step is:

- keep the existing TOON request context builder
- externalize the provider-specific instruction section into YAML
- ship a default embedded template in the binary
- allow a config-driven override directory for provider mode
- fail explicitly when a configured override is unreadable or invalid

## Non-Goals

- No privacy/redaction work in this batch
- No Anthropic transport in this batch
- No conversation mode or MCP streaming changes
- No tokenizer-based budget management yet
- No template-selection matrix per command; only the provider-insights prompt is configurable here

## Chosen Approach

Introduce a small internal prompt module that loads a single provider prompt template from YAML.

Provider-mode prompt construction will remain two-part:

1. `build_toon_prompt()` keeps producing the request/leak context in TOON.
2. A new prompt-template layer renders `section instructions` from YAML entries and appends it to the TOON request.

This keeps the high-signal heap context stable while making the provider instructions data-driven.

## Configuration Model

Extend `AiConfig` with a nested prompt section:

```toml
[ai.prompts]
template_dir = "/absolute/path/to/prompts"
```

Behavior:

- if `template_dir` is absent, Mnemosyne uses an embedded default YAML template
- if `template_dir` is present, Mnemosyne looks for `provider-insights.yaml` in that directory
- if that file is missing or invalid, provider mode returns an explicit configuration error

Optional environment override for parity with the rest of the config loader:

- `MNEMOSYNE_AI_PROMPT_TEMPLATE_DIR`

## YAML Shape

Keep the first template intentionally small and explicit:

```yaml
version: 1
instructions:
  - key: response_format
    value: Return only TOON v1 with section response and section recommendations
  - key: required_model
    value: "{{model}}"
  - key: required_keys
    value: model, confidence_pct, summary, item#N
```

Each instruction entry renders into TOON using the existing escaping path, so external templates can change instruction wording without bypassing the current wire-safety rules.

## Context Injection

This slice supports a small placeholder set inside YAML values:

- `{{model}}`
- `{{provider}}`
- `{{heap_path}}`
- `{{total_bytes}}`
- `{{total_objects}}`
- `{{leak_sampled}}`

Unknown placeholders are left untouched so template errors remain visible in prompt captures instead of being silently rewritten.

## Architecture

### New internal module

- `core/src/prompts/mod.rs`
  - loads the embedded default template
  - loads an optional override from disk
  - parses YAML
  - renders instruction key/value entries with placeholder substitution

### Existing code changes

- `core/src/analysis/ai.rs`
  - `build_provider_toon_prompt()` delegates instruction rendering to the prompt module
- `core/src/config.rs`
  - add `AiPromptConfig`
- `cli/src/config_loader.rs`
  - parse `[ai.prompts]` / `[llm.prompts]`
  - support `MNEMOSYNE_AI_PROMPT_TEMPLATE_DIR`

## Contract Preservation

This batch must preserve:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`
- CLI `--ai`
- MCP AI response shapes
- report AI sections

The only intentional behavior change is that provider-mode `wire.prompt` can now reflect a user-supplied YAML template override.

## Testing Strategy

Use TDD.

1. Add a CLI integration test proving `config` output includes `[ai.prompts].template_dir`.
2. Add a provider-mode CLI integration test that writes a temporary `provider-insights.yaml` override and asserts the captured AI prompt contains template-defined instruction text.
3. Add focused unit coverage for prompt rendering only if the implementation becomes harder to reason about from the end-to-end tests alone.
4. Re-run the existing provider-mode AI unit and integration coverage to preserve the current wire contract.

## File Impact

- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`
- Modify: `core/src/analysis/ai.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/Cargo.toml`
- Modify: `Cargo.toml`
- Add: `core/src/prompts/mod.rs`
- Add: `core/src/prompts/defaults/provider-insights.yaml`
- Modify: `cli/tests/integration.rs`

## Risks

- Over-scoping into full context redaction or multi-template routing would slow Step 14(a) unnecessarily.
- Rendering raw template bodies would bypass the current TOON escaping path; the YAML format therefore stays key/value based for this slice.
- A configured but broken template must fail honestly instead of silently falling back, otherwise prompt debugging becomes misleading.

## Decision

Implement one YAML-defined provider-insights template with an embedded default and an optional `template_dir` override. Defer privacy controls, multi-provider hardening, and richer prompt engines to later Step 14 slices.
