# AI Task Runner Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: next post-Step-11 milestone slice from `STATUS.md`

## Goal

Replace the single hardcoded AI insight heuristic with a configurable AI task runner that can assemble `AiInsights` from multiple named tasks while preserving the existing CLI, MCP, and report contracts.

## Why This Slice

`STATUS.md` lists the next remaining must-have as a configurable AI task runner that can call an LLM or rule engine for higher-fidelity insights. The broader M5 design also includes HTTP LLM backends, chat, MCP streaming, privacy/redaction, and multi-provider support, but that is a larger milestone than the immediate must-have requires.

The smallest correct next slice is:

- keep current `AiInsights` / `AiWireExchange` types stable
- keep current `--ai` CLI and MCP surfaces stable
- replace the single deterministic heuristic with a configurable task runner
- implement the runner using embedded rule-based tasks first, so the architecture becomes real without requiring networked LLM integration yet

## Chosen Approach

Use a config-driven rule-engine task runner.

`generate_ai_insights()` will stop directly building the final response itself. Instead, it will build an `AiTaskContext`, execute a configured list of task definitions, and merge their outputs into the existing `AiInsights` shape.

Tasks will be rule-based in this batch. Each task will read structured heap context and produce one or more outputs such as summary fragments, recommendations, confidence adjustments, or wire sections. Task selection/order will be configurable through `AiConfig`, with sensible embedded defaults.

This preserves the current public contracts while making the AI layer genuinely configurable and extensible.

## Non-Goals

- No external HTTP LLM provider in this batch
- No new CLI subcommands such as `chat`
- No MCP streaming or session management yet
- No fix-generator rewrite yet
- No prompt-template file loading from disk unless needed to support stable config semantics

## Architecture

### New concepts

- `AiTaskContext`
  - structured inputs derived from `HeapSummary`, selected leaks, and `AiConfig`
  - includes top leak, retained percentages, leak count, heap size, and model/provider metadata

- `AiTaskKind`
  - a small enum of built-in tasks for this batch, for example:
    - `HeapSummary`
    - `TopLeak`
    - `RemediationChecklist`
    - `HealthyHeapObservation`

- `AiTaskDefinition`
  - config-driven task entry with:
    - kind
    - enabled
    - weight or priority

- `AiTaskRunner`
  - executes configured task definitions in order
  - merges task outputs into final `AiInsights`

### Data flow

1. `generate_ai_insights()` builds an `AiTaskContext`
2. it resolves enabled task definitions from config defaults plus overrides
3. it runs each built-in rule task
4. it combines:
   - summary text
   - recommendations
   - confidence
   - TOON wire prompt/response sections
5. it returns the same `AiInsights` shape that current CLI/MCP/report surfaces already expect

## Config Model

Extend `AiConfig` with embedded task-runner configuration rather than introducing a separate top-level config section.

Proposed additions:

- `mode`
  - `stub` or `rules`
  - default should become `rules`
- `tasks`
  - ordered list of task definitions
- optional knobs for recommendation count / summary style only if needed by tests

The loader should support TOML `[ai]` / `[llm]` as today and allow configuring tasks there.

## Contract Preservation

This batch must preserve:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`
- existing CLI `--ai` behavior shape
- existing MCP `analyze_heap` / `explain_leak` JSON response shape

The content will change from one hardcoded heuristic to configurable rule-runner output, but the response schema must remain stable.

## Testing Strategy

Use TDD.

1. Add unit tests for config parsing of AI task definitions.
2. Add unit tests for task-runner behavior:
   - top leak present
   - no leaks / healthy heap path
   - task disabling changes the output
3. Keep existing `generate_ai_insights()` tests, but update expectations only as required by the new rules.
4. Add one integration-level contract test through CLI or MCP if needed to prove output shape is preserved.

## File Impact

- Modify: `core/src/config.rs`
- Modify: `cli/src/config_loader.rs`
- Modify: `core/src/analysis/ai.rs`
- Possibly add: `core/src/analysis/ai_runner.rs` if `ai.rs` becomes too dense
- Possibly modify tests in:
  - `core/src/analysis/ai.rs`
  - `core/src/mcp/server.rs`
  - `cli/tests/integration.rs`

## Risks

- Over-scoping into full external LLM integration would slow the overnight run and expand risk.
- Changing output wording too aggressively could break text assertions in tests or docs.
- Adding too much config complexity now would create churn before provider-backed execution exists.

## Decision

Implement the rule-based configurable task runner now, and treat external provider execution as the next slice after this one if time remains.
