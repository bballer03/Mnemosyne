# Milestone 23 — AI-First Guided Investigation Polish

**Status:** in progress — Slice **23.B** desktop workflow get/close/resume  
**Plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) § M23  
**Branch:** `sync/m15g-m16bcd`  
**Design gate (23.A):** READY — React-only composition over shipped artifact / workflow / AI-session contracts; no new analyzer or AI provider.  
**Design gate (23.B):** READY — thin desktop adapters over shipped `WorkflowStore::load`/`remove` (MCP `get_workflow`/`close_workflow` semantics); resume UI on `WorkflowCard`; no new workflow kinds.

## Objective

Make existing workflows and AI sessions feel native inside the workbench without replacing deterministic power tools. Rules mode stays the offline default. Every AI statement is visually separated from measured heap facts and carries provenance.

## Non-goals (milestone-wide)

- Bundled model runtime (Ollama/LM Studio/etc.)
- Autonomous source edits
- Arbitrary workflow language
- Unbounded history (shipped default **12**, hard max **32** — `core::mcp::session`)
- Token streaming without measured need
- Raw heap-value transmission to providers
- Hiding deterministic tools behind chat

## Existing contracts (compose, do not reinvent)

| Surface | Symbol / wire | Role |
| --- | --- | --- |
| AI session lifecycle | MCP `create_ai_session` / `resume_ai_session` / `get_ai_session` / `close_ai_session` / `chat_session` | Persisted heap-bound sessions |
| Chat turn | `AiChatTurn { question, answer_summary }` | Bounded history entry |
| History bounds | `DEFAULT_SESSION_HISTORY = 12`, `HARD_MAX_SESSION_HISTORY = 32`, `effective_history_limit` | Eviction / clamp |
| Rules default | `AiMode::Rules` | Offline-safe insights |
| Guided landing | `GuidedLanding`, `__MNEMOSYNE_WORKFLOW_BRIDGE__` | Workflow start / NL routing |
| Path opacity | desktop `sourceId` + basename `displayName` | Never absolute paths in React |

## Slice 23.A — Investigation session workspace

**Verdict:** READY for UI implementation.

**Files:**

| Path | Role |
| --- | --- |
| `ui/src/features/assistant/assistant-bridge-client.ts` | Optional host probe + local rules-mode history helpers (12-turn eviction, basename opacity) |
| `ui/src/features/assistant/InvestigationAssistantPage.tsx` | Session workspace: heap/workflow/focus, facts vs AI, deep links |
| `ui/src/app/router.tsx` | Route `/assistant` |
| Tests | rules mode, provider unavailable, focus change, 12-turn eviction, deep links |

**Acceptance (23.A):**

- Page shows current heap (basename / opaque `sourceId` only), optional workflow step, focused leak/object, recent bounded turns, and direct links to deterministic workbench views.
- Rules mode is default and works with no assistant host bridge.
- AI turns render in a distinct region with explicit provenance (`rules` / `provider` / `fallback`).
- Provider mode without a host bridge surfaces an explicit unavailable state; rules remains usable.
- No new analyzer and no new AI provider transport.

**Deferred to later slices:**

- **23.C** — Tauri create/resume/get/close/chat adapters over shipped AI session behavior (12/32, timeouts, redaction).
- **23.D** — End-to-end evidence + docs closeout.

## Slice 23.B — Desktop workflow continuity (get / close / resume)

**Verdict:** READY for thin adapter + UI implementation.

**Files:**

| Path | Role |
| --- | --- |
| `tauri/session-ops/src/lib.rs` | `get_workflow_for_session` / `close_workflow_for_session` over `WorkflowStore` |
| `tauri/src/commands.rs` / `main.rs` / `bridge.ts` | Tauri commands + `__MNEMOSYNE_WORKFLOW_BRIDGE__` methods |
| `ui/.../workflow-bridge-client.ts` | Typed get/close clients; basename-only heap path |
| `ui/.../WorkflowCard.tsx` | Resume-by-id, close, step deep-links (incl. `classloader_leak`) |

**Acceptance (23.B):**

- Users can start, inspect (`get`), resume (get → continue), and close shipped workflows, including `classloader_leak`.
- Step deep-links reach Classloaders / Inspector / GC Paths / Compare where applicable.
- No new workflow kind; MCP wire semantics preserved (`workflow_not_found` / `workflow_corrupt` / `workflow_already_complete`).

**Impact note (manual; no GitNexus index in this worktree):** Low — additive session-ops + Tauri command registration + WorkflowCard UI. d=1 dependents: bridge consumers and WorkflowCard tests only.

## GitNexus

This worktree has no local `.gitnexus` index (`npx gitnexus analyze` not run here). Blast radius for 23.A is additive: new `ui/src/features/assistant/*` plus `router.tsx` / optional `TopNav` link. No Rust symbols modified for 23.A. 23.B adds thin wrappers only; does not change `core::workflow` symbols.

## Impact note (manual)

| Change | Risk | Dependents |
| --- | --- | --- |
| New `/assistant` route | Low | Tests that enumerate routes; TopNav labels |
| New optional `__MNEMOSYNE_ASSISTANT_BRIDGE__` | Low | None until 23.C wires Tauri |
| Local rules-mode history helpers | Low | Page + unit tests only |
