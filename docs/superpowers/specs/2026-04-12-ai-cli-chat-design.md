# AI CLI Chat Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: Step 14(e) CLI conversation mode first slice only

## Goal

Add a leak-focused `mnemosyne chat <heap.hprof>` command that analyzes a heap once, shows the top 3 ranked leak candidates, and supports interactive follow-up questions within a single in-process terminal session.

## Why This Slice

Step 14(e) in `docs/roadmap.md` calls for conversation mode after the provider, prompt-template, privacy, and protocol-hardening slices are stable enough to support a more interactive surface.

The smallest honest first slice is:

- add CLI chat before MCP session work
- keep the conversation leak-focused rather than turning it into general heap exploration
- analyze once at session start and reuse that context for follow-up questions
- keep state in memory only for the running process
- reuse the existing AI/provider pipeline instead of inventing a second transport path

This makes chat useful immediately without widening scope into persistent sessions, streaming, or new external contracts.

## Non-Goals

- No MCP session-management changes in this batch
- No streaming responses in this batch
- No resumable or persisted chat sessions
- No general heap browser or open-ended query assistant
- No AI-driven fix generation changes
- No changes to report output formats or MCP response shapes
- No tokenizer-accurate prompt accounting beyond the current minimal `max_tokens` guard

## Chosen Approach

Implement `mnemosyne chat <heap.hprof>` as a thin interactive wrapper around the existing analysis and AI explanation flow.

Startup behavior:

1. validate the heap path
2. run `analyze_heap()` once
3. derive the top 3 ranked leaks from the analysis response
4. print a compact summary of the top leak candidates plus chat help
5. enter a simple REPL loop

Turn behavior:

- free-form input asks a question about the current leak focus
- when no explicit focus is set, the prompt context uses the top 3 shortlist entries
- each turn reuses the existing AI generation path and returns a normal `AiInsights` result to the CLI layer

The session is intentionally ephemeral. Closing the process discards history.

## User Experience

### Command shape

```text
mnemosyne chat heap.hprof
```

No new required flags are introduced in the first slice. The command inherits the existing config loading and AI mode behavior.

### Startup output

Startup should print:

- a short header confirming the analyzed heap
- a ranked shortlist of the top 3 leaks with leak ID, class, severity, and retained size
- a short help blurb explaining free-form questions plus `/focus`, `/list`, `/help`, and `/exit`

If no leaks are found, startup should say so explicitly and still allow chat to continue from the healthy-heap context.

### REPL commands

Keep the first command set intentionally small:

- free-form question text
- `/focus <leak-id>`
- `/list`
- `/help`
- `/exit`

Behavior notes:

- `/focus <leak-id>` validates the identifier using the same leak-ID matching semantics already used by `explain`
- `/list` reprints the startup shortlist
- `/help` reprints the minimal command guide
- `/exit` ends the session cleanly

No slash-command aliases or command nesting are needed in the first slice.

## Session State Model

Introduce a small internal chat-session model owned by the CLI layer.

Minimum state:

- heap path
- `HeapSummary`
- analyzed leak list
- current focused leak ID or `None`
- bounded recent conversation turns

Recommended history policy:

- keep only the most recent 3 completed turns in memory
- retain both user questions and assistant summaries for those 3 turns
- drop oldest turns first when trimming

This is enough to provide conversational continuity while avoiding persistence, migration concerns, or new config surface area.

## AI Reuse Strategy

The first slice should preserve the current outward AI contract:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`

Chat should therefore reuse the current `generate_ai_insights_async()` path rather than introducing a chat-specific AI response type.

The implementation will add a chat-specific prompt builder inside `core/src/analysis/ai.rs` so a turn can include:

- the current question
- the current leak focus or shortlist context
- bounded recent history

But the returned result should still parse back into the existing `AiInsights` structure so provider/rules/stub behavior stays aligned across CLI and MCP call sites.

## Prompt Semantics

The prompt for chat turns should remain TOON-based and explicit about intent.

Expected additions for the chat path:

- a request intent of `chat_leak_follow_up` to distinguish chat from one-shot leak explanation
- the active leak ID when one is selected
- a short conversation-history section with up to the 3 most recent completed turns
- the same leak-context and instruction sections already used by provider mode

Prompt construction should stay conservative:

- leak context remains the primary evidence
- history is supportive context only
- when prompt size gets tight, trim history before trimming the selected leak context
- the existing provider privacy redaction, audit logging, and minimal `max_tokens` guard still apply

This keeps the first slice honest: chat is a conversational layer over leak-analysis evidence, not a second independent reasoning system.

## Architecture

### CLI layer

`cli/src/main.rs` gains a new `Chat` subcommand and a small REPL orchestration path.

Responsibilities:

- validate startup arguments
- perform the initial one-time analysis
- render the shortlist/help text
- read terminal input line by line
- dispatch slash commands locally
- invoke shared AI generation for free-form questions
- print answers and maintain bounded in-memory history

### Core AI layer

The existing `core/src/analysis/ai.rs` logic remains the AI execution center.

Expected changes are intentionally narrow:

- add a small way to build leak-focused chat prompts from existing summary/leak data plus recent turn context
- preserve current rule/stub/provider dispatch
- preserve current response parsing and `AiInsights` return shape

### Boundaries

This slice should not move chat orchestration into MCP or a new session service yet.

The boundary is:

- CLI owns session lifecycle and REPL behavior
- core AI owns prompt construction and AI execution
- existing analysis engine remains the source of leak evidence

## File Impact

- Modify: `cli/src/main.rs`
- Keep any first-slice chat session helper types inside `cli/src/main.rs`; do not add a new CLI chat module in this batch
- Modify: `core/src/analysis/ai.rs`
- Modify: `cli/tests/integration.rs`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `OVERNIGHT_SUMMARY.md`

The first implementation should keep the REPL loop and chat session helper types in the existing CLI file so the change stays small and localized.

## Contract Preservation

This batch must preserve:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`
- CLI `--ai` behavior on existing commands
- MCP AI response shapes
- current report output shapes

The only intentional new public surface is the `chat` CLI command.

## Error Handling

- Invalid heap paths continue through the existing CLI validation path.
- Invalid `/focus` targets should fail fast with the same leak-ID validation style used by `explain`.
- AI/provider failures during a turn should report the turn failure without destroying the in-memory session state.
- Empty leak sets should produce an explicit healthy-heap startup message rather than aborting chat.
- Provider privacy controls continue to govern outbound chat prompts exactly as they already govern provider-mode analysis prompts.

## Testing Strategy

Use TDD.

1. Add CLI integration coverage for `mnemosyne chat` startup output using scripted stdin.
2. Add CLI integration coverage for one successful free-form question/answer turn.
3. Add CLI integration coverage for `/focus <leak-id>` switching context.
4. Add CLI integration coverage for invalid `/focus` handling.
5. Add focused unit coverage in `core::analysis::ai` only if the chat prompt-builder behavior is not clear enough from CLI integration tests alone.
6. Re-run existing targeted provider privacy/audit regressions to ensure chat prompt reuse does not bypass current safeguards.

## Rollout Notes

The first slice is intentionally CLI-only.

If this proves useful and stable, later follow-through can add:

- MCP session semantics
- richer slash commands
- optional persisted sessions
- broader heap Q&A beyond leak-focused conversation

Those are explicit follow-ons, not part of this batch.

## Risks

- Over-scoping into general-purpose heap Q&A would expand prompt design and testing substantially.
- Reusing the current `AiInsights` response shape limits how rich chat responses can be in the first slice, but that trade-off preserves existing contracts.
- If the prompt builder grows too many branches inside `core/src/analysis/ai.rs`, a later refactor may be needed; this batch should still prefer the smallest safe change.
- Interactive CLI tests can become brittle if they depend on dynamic formatting details, so assertions should target stable markers and behavior.

## Decision

Implement Step 14(e) as a CLI-only, leak-focused, in-process chat command that analyzes once, shows top leaks first, reuses the existing AI/provider pipeline for follow-up answers, and defers persistence, MCP sessions, and broader conversational scope to later slices.
