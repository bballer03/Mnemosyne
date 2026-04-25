# AI MCP Session Design

> Status: approved via interactive brainstorming
> Date: 2026-04-12
> Scope: M5 MCP session-backed conversation/context only

## Goal

Add persisted, heap-bound MCP AI sessions so clients can analyze a heap once, keep bounded conversation context across MCP calls and server restarts, and reuse that context for AI follow-up workflows without re-sending `heap_path` on every request.

## Why This Slice

`mnemosyne-cli chat` already proved the leak-focused conversation model: analyze once, keep a small focus/history state, and reuse the existing AI pipeline for follow-up questions.

The next smallest useful slice is to bring that same model to MCP with explicit session lifecycle methods instead of inventing a broader heap workspace.

This slice should:

- persist AI session state across MCP server restarts
- bind each session to one analyzed heap result
- keep the scope limited to AI follow-up workflows
- reuse existing `AiInsights` / `AiWireExchange` / `AiWireFormat::Toon` contracts
- keep current non-session MCP tools working as they do today

## Non-Goals

- No streaming-response work in this batch
- No session support for non-AI MCP tools such as `detect_leaks`, `query_heap`, `find_gc_path`, or `map_to_code`
- No generic heap-workspace abstraction
- No change to existing CLI chat behavior
- No change to report output formats
- No stable public on-disk session-file schema
- No persistence of provider credentials, prompt overrides, or full prior `AiWireExchange` payloads
- No session-listing or tombstone management in this slice

## Chosen Approach

Implement explicit MCP AI-session lifecycle methods backed by persisted local session files.

Each session is:

- created from a single `heap_path`
- initialized by running `analyze_heap()` once
- persisted as an internal JSON document with a version marker
- resumed later by `session_id`
- used only by AI follow-up methods

The server owns session persistence and state transitions. The AI layer continues to operate only on in-memory `HeapSummary`, leak lists, bounded `AiChatTurn` history, and optional focus state.

## Public MCP Surface

### New methods

Add these MCP methods:

1. `create_ai_session`
2. `resume_ai_session`
3. `get_ai_session`
4. `close_ai_session`
5. `chat_session`

### Updated methods

Keep these MCP methods, but add a session-backed branch:

- `explain_leak`
- `propose_fix`

### Unchanged non-session methods

Do not add session support to:

- `parse_heap`
- `detect_leaks`
- `analyze_heap`
- `query_heap`
- `map_to_code`
- `find_gc_path`

Those tools continue to require explicit request parameters and remain stateless.

## Method Contracts

### `create_ai_session`

Purpose:

- analyze a heap once
- create a persisted AI session
- return a compact startup payload

Required params:

- `heap_path: string`

Optional params:

- `min_severity: string`
- `packages: array<string>`
- `leak_types: array<string>`

Explicitly out of scope for this method:

- no `enable_classloaders`, `enable_threads`, `enable_strings`, `enable_collections`, or `enable_top_instances`
- no `histogram_group_by`
- no `enable_ai` flag

Behavior:

1. validate the heap path through the existing core path
2. run `analyze_heap()` once with AI disabled and only the filters needed for leak-focused conversation context
3. derive the top 3 ranked leak candidates from the resulting analysis
4. create a new session with empty history and no focus
5. persist the session
6. return:
   - `session_id`
   - `created_at`
   - `updated_at`
   - `heap_path`
   - `summary`
   - `leak_count`
   - `top_leaks`
   - `focus_leak_id`

If no leaks survive filtering, session creation still succeeds. `top_leaks` is empty and later `chat_session` calls operate from the healthy-heap context.

### `resume_ai_session`

Purpose:

- explicitly reopen a persisted session after reconnect or restart

Required params:

- `session_id: string`

Behavior:

1. load the persisted session by `session_id`
2. validate the session-file version
3. return the same startup payload shape as `create_ai_session` plus the current bounded history
4. update `updated_at` because resume is a real session activity

`resume_ai_session` does not re-run heap analysis.

### `get_ai_session`

Purpose:

- inspect a session without treating the read as a resume event

Required params:

- `session_id: string`

Behavior:

- load the session and return compact metadata only:
  - `session_id`
  - `created_at`
  - `updated_at`
  - `heap_path`
  - `leak_count`
  - `focus_leak_id`
  - `history_length`

`get_ai_session` does not update `updated_at`.

### `close_ai_session`

Purpose:

- end a session and remove its persisted state

Required params:

- `session_id: string`

Behavior:

1. delete the persisted session file immediately
2. return a small success payload confirming deletion

This slice does not use tombstones. After `close_ai_session`, later reads or follow-up calls fail with `session_not_found`.

### `chat_session`

Purpose:

- ask a follow-up question against a persisted AI session

Required params:

- `session_id: string`
- `question: string`

Optional params:

- `focus_leak_id: string`

Behavior:

1. load the session
2. if `focus_leak_id` is present, validate it using the same matching rules already used by `validate_leak_id()`
3. derive the active focus for the turn:
   - request `focus_leak_id`, if present
   - otherwise the session's current `focus_leak_id`, if present
   - otherwise no single focus, which means the AI prompt uses the session shortlist context
4. call `generate_ai_chat_turn_async()` with the session `summary`, session leak list, bounded history, active focus, and the current server AI config
5. on success:
   - append one `AiChatTurn` using the user's question and the returned answer summary
   - trim history to the most recent 3 completed turns
   - if request `focus_leak_id` was provided, persist it as the session's new focus
   - update `updated_at`
   - persist the session
6. return the normal `AiInsights` payload

On AI failure, the request fails but the session file remains unchanged.

### Session-backed `explain_leak`

Keep the current `heap_path`-based contract working.

Add an alternate session-backed request path:

- `session_id: string`
- optional `leak_id: string`

Rules:

- exactly one of `heap_path` or `session_id` must be present
- when `session_id` is used, `min_severity` is rejected because the leak set is already fixed by the persisted session
- when `leak_id` is omitted for a session-backed request:
  - use the session focus if one exists
  - otherwise explain against the full persisted leak set

The response stays `AiInsights`.

### Session-backed `propose_fix`

Keep the current `heap_path`-based contract working.

Add an alternate session-backed request path:

- `session_id: string`
- optional `leak_id: string`
- optional `project_root: string`
- optional `style: string`

Rules:

- exactly one of `heap_path` or `session_id` must be present
- when `leak_id` is omitted for a session-backed request:
  - use the session focus if one exists
  - otherwise propose fixes against the full persisted leak set
- preserve existing `FixRequest`, `FixSuggestion`, and `FixResponse` contracts

If the current fix-generator entry points need explicit summary/leak access for the session-backed branch, add a new internal helper instead of breaking the existing public request type.

## Session State Model

Persist the following minimum state for each AI session:

- `session_version: 1`
- `session_id`
- `created_at`
- `updated_at`
- `heap_path`
- analysis input snapshot:
  - `min_severity`
  - `packages`
  - `leak_types`
- analysis output snapshot:
  - `HeapSummary`
  - analyzed leak list
  - top-3 shortlist
- conversation state:
  - `focus_leak_id`
  - bounded `AiChatTurn` history

Do not persist:

- provider API keys or env-var names
- full prompt/response `AiWireExchange` pairs from prior turns
- unrelated analysis reports outside the leak-focused session scope

## AI and Config Semantics

The persisted session stores analysis evidence and chat state, not provider runtime state.

Follow-up AI calls should:

- use the persisted `summary`, leak list, shortlist, focus, and history from the session
- use the current server `AppConfig.ai` at request time for provider/rules/stub execution
- continue to pass through the existing privacy redaction, audit logging, and prompt-budget logic

This avoids persisting secrets and makes resumed sessions honor the current server AI configuration.

## Persistence Model

Persist one session per file.

Recommended on-disk shape:

```json
{
  "session_version": 1,
  "session_id": "opaque-id",
  "created_at": "2026-04-12T00:00:00Z",
  "updated_at": "2026-04-12T00:00:00Z",
  "heap_path": "heap.hprof",
  "analysis": {
    "min_severity": "HIGH",
    "packages": [],
    "leak_types": [],
    "summary": {},
    "leaks": [],
    "top_leaks": []
  },
  "conversation": {
    "focus_leak_id": null,
    "history": []
  }
}
```

The file format is internal. It is not a stable public API.

### Storage location

Add one optional configuration override:

```toml
[ai.sessions]
directory = "C:/path/to/mnemosyne/sessions"
```

If the override is absent, resolve a default per-user local Mnemosyne session directory with a small cross-platform helper.

### Write policy

- write to a temporary file first
- atomically replace the target session file after a successful serialize
- fail explicitly on write or rename errors

### ID policy

- `session_id` is an opaque random ASCII identifier
- the persisted filename is `<session_id>.json`

## Runtime Boundaries

### MCP layer

`core/src/mcp/server.rs` remains responsible for:

- request routing
- backward-compatible request parsing
- session lifecycle orchestration
- mapping session failures into structured MCP `error_details`

Because `server.rs` is already large, session model and store logic should move into a small MCP-local helper module rather than expanding `handle_request()` further.

### AI layer

`core/src/analysis/ai.rs` remains responsible for:

- `generate_ai_insights_async()`
- `generate_ai_chat_turn_async()`
- focus handling and leak-ID validation helpers
- provider/rules/stub dispatch

It should not know about session persistence.

### Fix layer

`core/src/fix/generator.rs` should preserve its existing public contracts.

If the session-backed MCP path needs to generate fixes from persisted summary/leak state rather than a raw `heap_path`, add a new internal helper instead of changing the current request/response shapes.

## Contract Preservation

This slice must preserve:

- `AiInsights`
- `AiWireExchange`
- `AiWireFormat::Toon`
- the existing `heap_path` request path for `explain_leak`
- the existing `heap_path` request path for `propose_fix`
- `FixRequest`
- `FixSuggestion`
- `FixResponse`
- the current MCP response envelope: `success`, `result`, `error`, and `error_details`

## Error Handling

Add structured MCP error codes for session-specific failures:

- `session_not_found`
- `session_load_failed`
- `session_version_unsupported`
- `session_persist_failed`

Input validation rules:

- reject requests that provide both `heap_path` and `session_id` when exactly one source of context is required
- reject requests that provide neither `heap_path` nor `session_id`
- reject invalid `focus_leak_id` or `leak_id` values with the existing invalid-input semantics
- reject `min_severity` on session-backed `explain_leak`

Failure behavior:

- failed AI turns do not append history
- failed AI turns do not mutate focus
- failed AI turns do not update `updated_at`
- session close removes state immediately; later access becomes `session_not_found`

## File Impact

- Modify: `core/src/mcp/server.rs`
- Add: `core/src/mcp/session.rs`
- Modify: `core/src/mcp/mod.rs`
- Modify: `core/src/config.rs`
- Modify: `core/src/fix/generator.rs` only if a new internal helper is needed for session-backed fix generation
- Modify: `core/src/lib.rs` only if internal plumbing requires re-export updates
- Modify: MCP tests in `core/src/mcp/server.rs` and/or dedicated session-store tests alongside the new helper
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/api.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`
- Modify: `OVERNIGHT_SUMMARY.md`

## Testing Strategy

Use TDD.

Add MCP coverage for:

1. `create_ai_session` returns session metadata and shortlist
2. `resume_ai_session` reloads persisted state without re-analysis
3. `get_ai_session` returns compact metadata without mutating timestamps
4. `chat_session` appends bounded history and preserves focus semantics
5. session-backed `explain_leak` works with `session_id`
6. session-backed `propose_fix` works with `session_id`
7. `close_ai_session` removes persisted state and later resume fails with `session_not_found`

Add store-level coverage for:

1. round-trip persistence
2. atomic-write happy path
3. history trimming to 3 turns
4. corrupt-file handling
5. unsupported-version handling

Keep and re-run regression coverage for:

- current `heap_path`-based `explain_leak`
- current `heap_path`-based `propose_fix`
- targeted provider privacy/audit behavior so session-backed follow-ups still route through the existing redaction path

## Rollout Notes

This slice is intentionally MCP-only. It should reuse the existing CLI chat semantics but should not attempt to merge CLI and MCP session persistence in the same batch.

If this slice proves stable, later follow-through can evaluate:

- session listing
- tombstones or expiration policies
- broader heap-workspace sessions
- streaming for long-running MCP AI calls, but only if verification shows it is needed
