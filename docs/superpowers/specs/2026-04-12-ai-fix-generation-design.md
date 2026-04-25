# AI Fix Generation Design

> Status: approved via collaborative design
> Date: 2026-04-12
> Scope: Milestone 5 sub-project 1 - AI-driven fix generation first slice

## Goal

Upgrade `mnemosyne fix` and MCP `propose_fix` from template-only placeholder patches to provider-backed, context-aware fix suggestions while preserving the existing request/response contract.

## Why This Slice

The remaining Milestone 5 work is too broad to implement safely as one batch. It should be completed in sequence:

1. AI-driven fix generation
2. MCP session-backed conversation/context
3. Transport hardening only where verification proves it is necessary

This spec covers only the first sub-project.

It is the clearest remaining product gap in M5 because `fix` and `propose_fix` already exist, but they still return placeholder guidance that is not AI-backed and not grounded in source context.

## Scope

- keep `FixRequest`, `FixSuggestion`, and `FixResponse` unchanged
- keep CLI `fix` and MCP `propose_fix` wired through the same shared core path
- add provider-backed fix generation when `AiMode::Provider` is active
- reuse existing heap-analysis, leak-focusing, leak-ID validation, provider transport, prompt redaction, and hashed audit logging paths
- use `project_root` to target one likely source file
- read one small source snippet around the mapped location and include it in the provider prompt
- fall back to the current heuristic/template suggestion path when AI-backed generation cannot produce a trustworthy result

## Non-Goals

- no `FixResponse` shape redesign
- no `apply_fix` command or automatic file editing
- no multi-file repository search or broad codebase reasoning in this slice
- no MCP session-management or conversation work in this slice
- no MCP streaming work in this slice
- no tokenizer-accurate prompt accounting beyond the existing minimal `max_tokens` guard
- no new report output formats or wire-format redesign

## Chosen Approach

Implement `propose_fix()` as an AI-first pipeline with heuristic fallback.

High-level flow:

1. run `analyze_heap()`
2. validate `leak_id` when provided
3. narrow to the focused leak set
4. select the single leak candidate already used by the current implementation
5. resolve source targeting from `project_root`
6. if provider-backed generation is eligible, build a strict TOON fix prompt and request a provider response
7. validate and parse the provider response into the existing `FixSuggestion` shape
8. if any prerequisite or quality gate fails, fall back to the current heuristic/template builder

This keeps the public contract stable while making the returned suggestion materially better when Mnemosyne has enough source context and a configured provider.

## Eligibility Rules For AI-Backed Fixes

The first slice should only attempt AI-backed fix generation when all of the following are true:

- `config.ai.mode == AiMode::Provider`
- `project_root` is present
- source targeting finds a real file under that project root
- a small local snippet can be read successfully from that file

If any of those conditions are false, Mnemosyne should skip AI-backed fix generation and return the existing heuristic/template suggestion path with explicit provenance.

This keeps the first slice honest. The provider should not be asked to invent repository structure or write a patch against an unmapped pseudo-file.

## Contract Preservation

This slice must preserve:

- `FixRequest`
- `FixSuggestion`
- `FixResponse`
- CLI `fix` output shape
- MCP `propose_fix` result shape
- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`

The user-visible change is behavioral, not structural: the same fields become provider-backed when the AI path is eligible and succeeds.

## Source Targeting Strategy

Use the existing `map_to_code()` path rather than inventing a second source-targeting system.

Source-targeting behavior:

- when `project_root` is provided, call `map_to_code()` using the focused leak's identifier and class name
- pick the first real mapped location
- use that location's file path as `target_file`
- read a narrow local code window around the mapped line for AI context

Recommended context size for the first slice:

- 5 lines before the mapped line
- the mapped line itself
- 5 lines after the mapped line

This keeps the prompt grounded in actual code without widening into multi-file analysis.

If no real file is mapped, do not ask the provider to guess. Fall back to the heuristic/template path.

## AI Prompt Semantics

Provider-backed fix generation should use a strict TOON prompt and response contract, similar to the existing provider-backed analysis path.

The prompt should include:

- request intent: `generate_fix`
- heap path
- selected leak ID
- leak class name
- leak severity
- retained size
- existing leak description
- selected `FixStyle`
- resolved `target_file`
- the small source snippet
- explicit instructions to patch only the provided target file
- explicit instructions to stay consistent with the requested fix style

The prompt should also make the trust boundary explicit:

- if the source snippet is insufficient, the model should return a low-confidence patch candidate rather than invent unrelated files
- the model must not claim to have searched the rest of the repository

## AI Response Contract

The provider response should be parsed from a strict TOON payload into an internal fix-draft struct, then mapped into the existing `FixSuggestion` shape.

Recommended response contract:

```text
TOON v1
section response
  confidence_pct=81
  description=Release retained session entries when they age out.
section patch
  diff=--- a/src/main/java/com/example/UserSessionCache.java\n+++ b/src/main/java/com/example/UserSessionCache.java\n@@ ...
```

Notes:

- `target_file` remains Mnemosyne-owned and should come from source targeting, not from the model
- `style` remains Mnemosyne-owned and should echo the request style as it does today
- `confidence_pct` maps into the existing `confidence: f32` field
- `description` maps into the existing `description` field
- `diff` maps into the existing `diff` field

## Quality Gates

An AI response is usable only if all of the following are true:

- the response begins with `TOON v1`
- it includes `section response`
- it includes `section patch`
- `description` is non-empty
- `confidence_pct` parses successfully
- `diff` is non-empty
- `diff` references the targeted file path or a matching relative path
- `diff` looks like a unified diff with the usual header markers

If any quality gate fails, Mnemosyne should fall back to the heuristic/template path instead of returning malformed or overclaimed output.

## Behavior And Provenance

### AI-backed success

On successful provider-backed fix generation:

- return the AI-backed `FixSuggestion`
- preserve the existing response shape
- do not attach the current placeholder provenance marker

The first slice does not need a new explicit "AI-generated" marker. The distinction is that placeholder/fallback markers disappear when the provider-backed path succeeds.

### Fallback behavior

When provider-backed fix generation is not eligible or fails validation, return the current heuristic/template suggestion path.

Fallback cases include:

- non-provider AI mode
- missing `project_root`
- unmapped source file
- source snippet read failure
- provider transport failure
- prompt redaction failure
- malformed provider TOON output
- AI response that fails fix quality gates

Fallback responses should carry explicit provenance:

- `Synthetic` - fix suggestion came from heuristic generation rather than verified source transformation
- `Fallback` - preferred provider-backed fix generation was skipped or failed
- `Placeholder` - returned guidance is still the template-style patch path

This keeps offline and partially configured environments useful without pretending the fix is source-verified.

## Architecture

### Core fix layer

`core/src/fix/generator.rs` remains the single entry point for both CLI and MCP fix generation.

Expected additions inside or adjacent to this module:

- a small internal provider-fix draft type for parsed TOON output
- a source-context helper for one mapped file + narrow snippet window
- an AI-fix prompt builder
- an AI-fix TOON parser with validation
- a decision point that chooses provider-backed generation or heuristic fallback

Keep these helpers close to `propose_fix()` in the first slice rather than introducing a large new fix subsystem.

### Shared provider execution path

Do not duplicate provider transport, prompt redaction, or audit logging logic.

Instead, extract or reuse a small shared helper so fix generation can send a TOON prompt through the same provider/privacy path already used by provider-backed AI insights and chat.

The reusable boundary should be conceptually:

- input: fully rendered TOON prompt + `AiConfig`
- shared work: redaction, hashed audit logging, provider completion call
- output: raw provider text for fix-specific TOON parsing

Fix generation should parse its own TOON response schema locally rather than forcing the result through `AiInsights`.

### CLI and MCP boundaries

- `cli/src/main.rs` should keep calling `propose_fix()`
- `core/src/mcp/server.rs` should keep returning `FixResponse` from `propose_fix()`
- neither surface should grow a second fix-generation code path

## File Impact

- Modify: `core/src/fix/generator.rs`
- Modify: `core/src/analysis/ai.rs` only as needed to expose a small shared provider-execution helper
- Modify: `core/src/mcp/server.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/integration.rs`
- Modify: `docs/api.md`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`

`core/src/mapper/source.rs` should be reused as-is unless testing shows a narrow, targeted adjustment is required for the mapped line/snippet handoff.

## Error Handling

- invalid `leak_id` continues to return the current explicit error
- heap-analysis failures continue to error normally
- provider-backed fix-generation failures should not crash the CLI or MCP path; they should fall back to heuristic guidance
- only unexpected failures outside the fallback envelope should escape as errors

This keeps `fix` resilient while still preserving honest analysis errors.

## Testing Strategy

Use TDD.

### Core unit tests

Add focused tests for:

1. provider fix TOON response parsing
2. malformed or partial provider fix TOON response rejection
3. fallback provenance when provider generation is skipped or fails
4. source-snippet extraction around a mapped line
5. mapped target-file reuse in the final `FixSuggestion`

### CLI integration tests

Add or update CLI coverage for:

1. provider-mode `fix` returns AI-backed content
2. `fix` without provider eligibility still returns heuristic guidance
3. invalid `leak_id` still errors

### MCP integration tests

Add focused MCP coverage for:

1. `propose_fix` preserves the existing `FixResponse` shape
2. provider fix-generation failure still returns a fallback response rather than a transport-level crash

### Regression verification

Re-run existing targeted provider redaction and audit-log regressions so the new fix path does not bypass privacy controls.

## Risks

- The mapped file may be only approximately correct. Mitigation: limit provider-backed generation to one mapped file plus one local snippet and fall back when mapping is too weak.
- The model may return malformed patch text. Mitigation: strict TOON parsing and diff-shape validation.
- Reusing provider helpers carelessly could entangle fix-generation and analysis-insight code. Mitigation: extract only the smallest shared execution helper.
- A wider source-context reader would increase scope and privacy surface area. Mitigation: keep the first slice to one file and one narrow window.

## Rollout Notes

This slice completes the most concrete remaining product feature in M5, but it does not complete M5 by itself.

The next follow-on specs after this slice should cover:

1. MCP session-backed conversation/context
2. transport hardening only where verification proves it is required

## Decision

Implement the first remaining Milestone 5 sub-project as an AI-first fix-generation upgrade that preserves the existing `fix` and `propose_fix` contracts, uses one mapped source file plus a small local snippet for provider-backed patch generation, and falls back to the current heuristic/template path with explicit provenance when the AI path is unavailable or not trustworthy.
