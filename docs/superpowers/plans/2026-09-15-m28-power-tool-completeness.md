# M28 Power-Tool Completeness Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mature the existing query, analyzer, policy, visualization, and export surfaces without widening the bounded backend scope.

**Architecture:** Treat each power tool as an on-demand projection of the active investigation. Reuse shipped query/analyzer/flamegraph/policy APIs and M26 operation envelopes; preserve lean open and provenance.

**Tech Stack:** React, Tauri, `core::query`, existing analysis modules, policy engine, flamegraph/report renderers.

**Roadmap:** [M28 — Power-tool completeness](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m28--power-tool-completeness)

## Global Constraints

- Keep named OQL deferrals explicit: `eval(...)`, arbitrary-depth nesting, and other unscheduled grammar.
- Strings/collections/arrays/threads/referrers/classloaders remain opt-in with memory/cost disclosure.
- Reuse `AnalyzeRequest` flags and existing analyzer reports; no duplicate frontend analyzer.
- Preserve HTML escaping and provenance on every export.
- Use focused component/client tests; never mount the production route tree in WSL tests.

---

## File map

- OQL: `ui/src/features/heap-explorer/HeapQueryConsolePage.tsx`, `components/QueryConsolePanel.tsx`, `heap-explorer-query-client.ts`, `core/src/query/`.
- Analyzers: `ui/src/features/artifact-explorer/components/*Panel.tsx`, `tauri/src/commands.rs`, `tauri/session-ops/src/lib.rs`.
- Policy/compare controls: `ui/src/features/policy/`, `ui/src/features/comparison/`.
- Visualization/export: `ui/src/features/flamegraph/`, `core/src/report/`, existing desktop flamegraph command.

### M28.A — OQL workbench

**Owned files:**
- Modify: `ui/src/features/heap-explorer/components/QueryConsolePanel.tsx`
- Modify: `ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/host/tauri-bridge.test.ts`

**Interfaces:**
- `runHeapQuery(input)` keeps the existing ready/unavailable/error result union, with the error branch widened to include an optional structured `{ byteOffset, line, column }` location.
- Query result object IDs remain scalar cells; the panel recognizes only object-ID columns and commits navigation through `useInvestigationStore.getState().setObjectId(id, "inspector")`.
- The desktop bridge continues to use M26 `{ workspaceId, revision, operationId, data }` envelopes. The panel adds a workspace/revision guard for every browser-bridge response before committing UI state.
- Query text remains an invocation-only value. It is never added to progress payloads, persistence, or console logging.

#### Task 1: Structured query errors and privacy-safe invocation

- [ ] **Step 1: Add failing client tests for structured locations**

  Add focused tests to `heap-explorer-query-client.test.ts` proving that a bridge rejection ending in `at byte 14` returns an error result with `byteOffset: 14`, one-based `line`/`column`, and no copied query text in the error object. Add a multiline UTF-8 case so byte offsets are not treated as JavaScript character indexes.

- [ ] **Step 2: Run the client test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/heap-explorer-query-client.test.ts --max-concurrency=1`

  Expected: FAIL because query errors expose only a flat `error` string.

- [ ] **Step 3: Implement the minimal structured error parser**

  In `heap-explorer-query-client.ts`, add:

  ```ts
  export type HeapQueryErrorLocation = {
    byteOffset: number;
    line: number;
    column: number;
  };
  ```

  Extract a terminal `at byte N` marker from the host error, map the UTF-8 byte prefix into one-based line/column coordinates, and return the location separately from the display message. Do not return or persist the query text.

- [ ] **Step 4: Add failing native bridge privacy test**

  In `tauri-bridge.test.ts`, reject `query_heap` with an error value containing a unique query sentinel and spy on `console.error`. Assert the sentinel is absent from all logged arguments and that emitted operation-progress payloads contain only the M26 context/kind/phase/progress fields.

- [ ] **Step 5: Run the bridge test and verify RED**

  Run: `cd ui && bun test src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: FAIL because the generic native invoke logger currently logs the raw rejection object.

- [ ] **Step 6: Redact raw query invocation failures**

  Add a privacy option to the internal `invokeOrThrow` helper and enable it only for `query_heap`. Keep the command-level failure marker, but never pass the raw rejection to `console.error`. Do not alter result envelopes or progress payloads.

- [ ] **Step 7: Run focused client/bridge tests**

  Run: `cd ui && bun test src/features/heap-explorer/heap-explorer-query-client.test.ts src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: PASS with no query sentinel in captured logs or progress.

#### Task 2: Bounded history, vetted examples, and syntax disclosure

- [ ] **Step 1: Add failing panel tests for bounded history**

  Extend `QueryConsolePanel.test.tsx` with focused component tests that submit more than the history limit, assert newest-first ordering, assert duplicate queries move to the front without duplication, and assert clicking a history item restores it to the editor.

- [ ] **Step 2: Add failing panel tests for examples and syntax**

  Assert that vetted examples can replace the editor text, the supported syntax list is visible beside the editor, and named deferrals explicitly include `eval(...)`, arbitrary-depth nesting, multi-class `FROM`, and arbitrary-depth/multi-hop traversal beyond the shipped bounds.

- [ ] **Step 3: Run the panel test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: FAIL because the panel currently renders only a textarea, button, and raw table.

- [ ] **Step 4: Implement the workbench side rail**

  Add immutable vetted examples and supported/deferred syntax copy beside the editor. Keep history in component memory only, cap it at 10 unique entries, record non-empty submissions, and provide buttons that restore history/examples without auto-running them.

- [ ] **Step 5: Run the panel test and verify GREEN**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 3: Inspector navigation and stale-response rejection

- [ ] **Step 1: Add failing object-navigation test**

  Render only `QueryConsolePanel` under a memory router, return an `object_id`/`@objectId` cell, click it, and assert the investigation store contains that ID with `originPane: "inspector"` and the location is `/heap-explorer/object-inspector?objectId=<encoded-id>`. Assert non-object scalar cells remain plain text.

- [ ] **Step 2: Add failing stale-response tests**

  Use deferred bridge promises. Change the investigation revision before resolving ready, error, and unavailable responses, then assert none replace the currently rendered result/status. These tests mount only the focused panel, never production routes.

- [ ] **Step 3: Run the panel test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: FAIL because result cells are not navigable and panel responses are committed without a workspace/revision check.

- [ ] **Step 4: Implement navigation and response guards**

  Recognize case-insensitive `object_id`, `objectId`, and `@objectId` result columns. For valid hexadecimal object-ID cells, commit the ID to the investigation store with inspector origin and navigate to the object-inspector route. Capture `{ workspaceId, revision }` plus a local request sequence before every query; commit ready, error, or unavailable state only when both identities are still current.

- [ ] **Step 5: Render structured error coordinates**

  Display parser errors as an alert with a separate `Line N, column M` location. Keep location absent for execution/transport errors that do not provide a parser byte offset.

- [ ] **Step 6: Run all focused M28.A tests**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx src/features/heap-explorer/heap-explorer-query-client.test.ts src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: PASS.

#### Task 4: M28.A verification and implementation commits

- [ ] **Step 1: Run the related route adapter test**

  Run: `cd ui && bun test src/features/heap-explorer/HeapQueryConsolePage.test.tsx --max-concurrency=1`

  Expected: PASS without mounting the production route tree.

- [ ] **Step 2: Run TypeScript lint and production build**

  Run: `cd ui && bun run lint && bun run build`

  Expected: both commands exit 0.

- [ ] **Step 3: Review scope**

  Run: `git diff --check && git status --short`

  Expected: only M28.A-owned source/tests plus this plan are changed; untracked `.claude/skills/gitnexus-*` remain untouched.

- [ ] **Step 4: Commit implementation**

  ```bash
  git add ui/src/features/heap-explorer/components/QueryConsolePanel.tsx \
    ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx \
    ui/src/features/heap-explorer/heap-explorer-query-client.ts \
    ui/src/features/heap-explorer/heap-explorer-query-client.test.ts \
    ui/src/host/tauri-bridge.ts \
    ui/src/host/tauri-bridge.test.ts
  git commit -m "feat(ui): mature OQL workbench"
  ```

### M28.B — On-demand analyzers

- [ ] Plan one host enrichment request over existing analyzer flags with field-data cost preview.
- [ ] Commit results only to the matching revision/op and label partial/fallback/unavailable sections.
- [ ] Add policy baseline picker, compare identity strategy controls, and recommendation content rather than count-only cards.

### M28.C — Visualization and export

- [ ] Expose existing flamegraph formats and report exports from the workspace.
- [ ] Sanitize filenames/content, preserve provenance/mode labels, and never inject untrusted HTML.
- [ ] Add focused export contract tests and honest native/download evidence.
