# Milestone 14 — UI Backend-Parity & AI-Native Redesign

> **Status:** 🔲 Pending — design authored 2026-08-20, awaiting Implementation Agent pickup of Slice 14.A. Brainstormed and approved in-conversation with the user 2026-08-20 (architectural path, per `superpowers:brainstorming`).
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M14
> **Predecessors:** M8, M9, M10, M11, M13 — all shipped, all core+CLI+MCP-only by explicit design (see each milestone's own "Not shipped" UI note). This milestone is that accumulated backlog.
> **Last updated:** 2026-08-20

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M14 |
| Type | Usability / parity-closing (UI side) |
| Touched crates/packages | `ui/` only. `tauri/src/commands.rs` gains new native commands where the desktop host bridge needs them; `core`/`cli` untouched (this milestone surfaces existing backend capability, adds none). |
| Test bar | `bun run test` (existing Bun test runner), `tsc --noEmit` (the project's `lint` script), plus in-browser verification via the preview tool for each slice's golden path + one edge case — same rigor the Rust side gets from `cargo test`/clippy/fmt, adapted to this stack. |
| Visual budget | Extend `ui/src/app/globals.css`'s existing dark/Inter tokens. No new design system, no new component library dependency beyond what's already in `ui/package.json`. |

## 2. Objective

After M14, every backend capability shipped in M8–M13 has a working UI surface, and the browser app's navigation has an AI-guided default entry point (built on M11's workflows) while every existing MAT-equivalent power view stays fully present, unhidden, and undiminished. Per the user's explicit constraint from the brainstorming session: **"do not dumb down the product... everyone should have the MAT capabilities but also should have a simple to use interface."** The guided layer is an accelerator on top of full power, never a replacement for it.

## 3. Context

### 3.1 What the UI ships today (inspected, `ui/src/`)

- **Routes** (`ui/src/app/router.tsx`): `/` (artifact loader), `/dashboard`, `/artifacts/explorer`, `/heap-explorer/{dominators,object-inspector,query-console}`, `/leaks/:leakId/{overview,explain,gc-path,source-map,fix}`. No thread view, no classloader view, no diff/comparison view, no snapshot view, no workflow view anywhere in this list.
- **Two host bridges**, both optional-capability-probe pattern (confirmed in `ui/src/features/heap-explorer/heap-explorer-query-client.ts` and `ui/src/features/leak-workspace/live-detail-client.ts`): `window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__` (`queryHeap`, `getReferences`, `getReferrers`) and `window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__` (`explainLeak`, `findGcPath`, `mapToCode`, `proposeFix`). Every bridge method is checked with an `isXAvailable()` helper before use; absence renders an explicit "unavailable" state, never synthetic/fake data. **M14 extends both bridges with new optional methods following this exact pattern — no new pattern introduced.**
- **`AnalysisArtifact`** (`ui/src/lib/analysis-types.ts`) is the typed JSON artifact shape the artifact-loader flow parses — mirrors `AnalyzeResponse` but stops at roughly the M3–M6 field set (`summary`, `leaks`, `recommendations`, `graph`, `histogram`, `unreachable`, and presumably `threads`/`strings`/`collections`/`classloader` from M3 — verify exact current field list at implementation time). **Missing:** `referrer_report` (M8), `classloader_report`'s new `duplicate_classes`/`unique_class_count`/`ancestor_chain` fields (M13). These are the two purely additive-type-extension items in this milestone — no new bridge call needed, just widening the parsed-artifact type to match what `mnemosyne analyze --format json` already emits.
- **Tauri desktop shell** (`tauri/src/commands.rs`): native commands for `load_heap`, `unload_heap`, `query_heap`, `get_references`, `get_referrers`, `explain_leak`, `find_gc_path`, `map_to_code`, `propose_fix`. M9 (snapshots), M10 (object diff), M11 (workflows), and M13's `detect_classloader_leaks` have **no** Tauri command yet — needed only if the desktop shell (not just the browser-with-live-MCP-bridge story) should expose these; scoped as in-scope for parity but lower priority than the browser-first surfaces (see §4 "Out").
- **State management:** Zustand stores per feature (`use-artifact-store.ts`, `dashboard-store.ts`, `leak-workspace-store.ts`) — established pattern for new stores this milestone adds (a `comparison-store.ts`, a `workflow-store.ts` on the client side — not to be confused with the Rust `core::workflow::WorkflowStore`).
- **Data tables:** `@tanstack/react-table` already a dependency, used by existing panels (`LeakTable`, `HistogramExplorerPanel`) — reuse for the new comparison-basket and referrer tables rather than hand-rolling table UI.

### 3.2 What MAT's GUI does that this milestone targets

- **Compare Basket** — drag two heap dumps' histograms into a basket, run a comparison, see a ranked diff. M14's comparison-basket UI is the direct equivalent, backed by M10's `diff_heaps` MCP tool (`mode: "object"`) instead of MAT's in-process compare.
- **Inspector pane** — field values, refs in/out, dominator context for a selected object. Already partially shipped (`ObjectInspectorPanel.tsx`); M14 extends it with the M8 `inspect_object` MCP tool's fuller data (structured `ObjectRef` refs/referrers, dominator parent/children as navigable chips — not just inline text).
- **Thread view with locals** — MAT's Threads Overview shows per-frame local variables. M14 adds a thread view (none exists today) backed by M8's frame-locals data.
- **Classloader explorer** — MAT's classloader-duplicate report. M14 adds a panel backed by M13's `duplicate_classes`/`ancestor_chain`.
- **OQL console** — already shipped (`HeapQueryConsolePage.tsx`/`QueryConsolePanel.tsx`); no changes needed in this milestone (query depth itself is M15, backend-only until M15 ships, at which point the existing console UI already handles it — OQL text in, results out, no UI-side awareness of *which* OQL features are supported).

### 3.3 The AI-native layer (user-delegated design decision, from brainstorming session)

The user explicitly delegated the concrete shape of "AI native" to this design pass, with one hard constraint: don't reduce capability for power users. Resolution: a **new default landing route** (replacing today's plain `/` artifact-drop-and-nothing-else) that, once an artifact loads, immediately shows an AI-generated triage summary (composing M11's `triage_memory_leak` workflow) and a natural-language input that resolves to either an OQL query (via the existing query console's execution path) or a workflow `start_workflow` call, depending on what the input looks like. Every power route (`/heap-explorer/*`, `/leaks/*`, plus this milestone's new `/compare`, `/threads`, `/classloaders`) remains fully reachable via persistent top-level navigation — the guided landing is not a gate in front of them.

## 4. Scope

In:

1. **Artifact-type extension (no bridge changes):** widen `AnalysisArtifact` in `ui/src/lib/analysis-types.ts` to include `referrerReport` (M8) and the extended `classloaderReport` shape (`duplicateClasses`, and per-loader `uniqueClassCount`/`ancestorChain`) (M13), matching the actual current `mnemosyne analyze --format json` output field-for-field (verify against a real `--format json` run at implementation time, not just this doc's description).
2. **Object Inspector extension** (M8): `ObjectInspectorPanel.tsx` renders `references_out`/`referrers_in` as clickable chips (navigate to that object id) instead of/in addition to today's inline display; dominator parent/children become navigable the same way. New optional bridge method `inspectObject(objectId, retainFieldData)` on the heap-explorer bridge, mirroring the `getReferences`/`getReferrers` optional-capability pattern exactly.
3. **GC-path visualizer extension** (M8): `LeakGcPathPage.tsx` gains a path-count selector; when the bridge supports it (new optional `findAllGcPaths(objectId, maxPaths)` method), shows a list of paths instead of only the shortest. Falls back to today's single-path view when the bridge lacks the new method (graceful degradation, same pattern as every other optional bridge capability).
4. **Referrer panel** (M8): new panel in `ArtifactExplorerPage.tsx` (or a new sibling component) — "Top referenced objects" table sourced from the widened `AnalysisArtifact.referrerReport` (item 1; no new bridge call needed since this is artifact-level data, not live).
5. **Thread view** (M8 frame-locals): **new route** `/heap-explorer/threads` (or equivalent), new `ThreadExplorerPage.tsx` + panel, sourced from `AnalysisArtifact.threads` (verify this field's current shape includes M8's `locals` per frame — likely does since it's additive on the Rust side; if the existing `AnalysisArtifact` type doesn't yet have a `threads` field at all, that's this slice's actual scope, not just the frame-locals sub-piece).
6. **Comparison basket UI** (M10): **new route** `/compare`, new feature directory `ui/src/features/comparison/`. Pick two artifacts (from already-loaded artifacts, or trigger a live diff via a new bridge method `diffObjects(beforeKey, afterKey, options)` when the bridge/MCP connection supports it) → ranked `added`/`removed`/`retained_changed` tables (reuse `@tanstack/react-table`) with a match-quality indicator badge. Biggest net-new UI surface in this milestone; no dependency on any other slice.
7. **Snapshot picker** (M9): a "Recent heaps" list component on the new guided landing (item 9), backed by a new optional bridge method `listSnapshots()` — when unavailable (browser-only, no live MCP connection), the guided landing simply omits this section rather than showing an empty/broken one.
8. **Classloader panel** (M13): new panel (`ClassloaderExplorerPanel.tsx`) — most naturally placed in `ArtifactExplorerPage.tsx` alongside the referrer panel (item 4), showing `duplicateClasses` and per-loader `ancestorChain`/`uniqueClassCount` from the widened artifact type (item 1).
9. **AI-guided landing + workflow cards** (M11): replaces today's `/` route content (artifact-drop-and-stop) with: artifact drop (unchanged) → once loaded, an AI-generated triage summary card (composes `triage_memory_leak` via a new optional bridge method `startWorkflow(kind, params)` / `nextStep(workflowId, input)`, mirroring M11's MCP tool pair) + a natural-language input bar + three more workflow-kind cards (`tune_gc`, `traverse_object_graph`, `compare_snapshots`) that deep-link into item 6's comparison view or a new minimal `traverse_object_graph` stepper. Persistent top-nav to every power route added/confirmed in this same slice so the guided layer never gates access.
10. **Visual work:** every new component styled against `ui/src/app/globals.css`'s existing tokens (dark background `#0a0c10`, `#e2e2e8` text, Inter). Load the `frontend-design` skill before writing any new component's markup/CSS, per this project's own design discipline (not the earlier Rust-only backend work, which had no such skill dependency).

Out:

- **Any new backend/Rust capability.** Every item above surfaces something `core`/`cli`/`mcp` already ships. If implementation discovers a genuine backend gap while wiring a panel (e.g., a JSON field the artifact renderer doesn't actually emit yet), that is a scope-boundary violation to flag and stop on, not silently patch around in Rust mid-UI-slice.
- **MAT backend-parity gaps** (OQL depth, duplicate arrays, group-by-superclass, plugin runtime) — M15.
- **Desktop packaging/installers** — M16. This milestone's Tauri command additions (item 2's `inspectObject`, etc., where the desktop shell needs a native-command equivalent of a browser-bridge method) are in scope only where a slice's UI work requires them to function in the Tauri shell at all; a full audit of every M9/M10/M11/M13 capability's Tauri native-command equivalent is explicitly deferred to M16 unless a specific M14 slice's own UI is broken without one.
- **Replacing the existing artifact-JSON-file workflow.** Artifact-backed panels (most of today's UI) keep working with zero live connection; only the genuinely-live features (comparison-diff-on-demand, snapshot picker, workflows) require a bridge.

## 5. Architecture overview

```
                    ┌─────────────────────────────────────────┐
                    │  /  (AI-guided landing, REPLACES today's │
                    │      bare artifact-drop content)         │
                    │  - artifact drop (unchanged)              │
                    │  - triage summary card (Workflow bridge) │
                    │  - NL input bar                          │
                    │  - 4 workflow-kind cards                 │
                    │  - "Recent heaps" (Snapshot bridge, opt) │
                    └──────────────┬────────────────────────────┘
                                   │ persistent top-nav, never gated
          ┌────────────────────────┼─────────────────────────────────┐
          ▼                        ▼                                 ▼
  /heap-explorer/*         /leaks/:leakId/*                    /compare (NEW)
  (existing + extended)    (existing, unchanged)                (NEW, M10-backed)
   - dominators                                                  - basket picker
   - object-inspector (extended: chips)                          - added/removed/
   - query-console (unchanged)                                     retained_changed
   - threads (NEW)                                                 tables
                                                                    - match-quality
  /artifacts/explorer (extended)                                     badge
   - existing histogram/analyzer-rail panels
   - referrer panel (NEW)
   - classloader panel (NEW)


 Two bridges extended (optional-capability-probe pattern, unchanged shape):

   __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__          __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__
     queryHeap (existing)                        explainLeak (existing)
     getReferences (existing)                     findGcPath (existing)
     getReferrers (existing)                       mapToCode (existing)
     inspectObject (NEW)                            proposeFix (existing)
     findAllGcPaths (NEW)
     diffObjects (NEW)                          NEW third bridge, or extend an
     listSnapshots (NEW)                        existing one — see §6 open question
     startWorkflow / nextStep (NEW)
```

Module placement rationale: new routes live as new sibling directories under `ui/src/features/` (`comparison/`, `threads/` — or `heap-explorer/HeapThreadsPage.tsx` if it fits better as a `heap-explorer` sibling once implementation looks at the existing `HeapExplorerLayout.tsx` nav structure), matching the existing one-feature-per-directory convention. No new top-level app-shell restructuring beyond the landing-route swap and persistent nav, which is the explicit ask.

## 6. Data model (TypeScript types, additive)

```typescript
// ui/src/lib/analysis-types.ts (extended)

export type ReferrerEntry = {
  objectId: string;
  className: string;
  retainedSize?: number;
  referrerCount: number;
  topReferrerClasses: Array<[string, number]>;
};

export type ReferrerReport = {
  entries: ReferrerEntry[];
  totalObjectsConsidered: number;
};

export type DuplicateClassGroup = {
  className: string;
  loaderObjectIds: string[];
  loaderCount: number;
};

// AnalysisArtifact gains (verify exact casing/nesting against a real
// `--format json` run before implementing -- this is illustrative, not
// binding, unlike the Rust-side data-model sections in prior milestone docs):
//   referrerReport?: ReferrerReport
//   classloaderReport.duplicateClasses?: DuplicateClassGroup[]
//   classloaderReport.loaders[].uniqueClassCount?: number
//   classloaderReport.loaders[].ancestorChain?: string[]
```

```typescript
// ui/src/features/heap-explorer/heap-explorer-query-client.ts (extended)

export type HeapExplorerHostBridge = {
  queryHeap?: (input: HeapQueryInput) => Promise<unknown>;
  getReferences?: (objectId: string) => Promise<unknown>;
  getReferrers?: (objectId: string) => Promise<unknown>;
  inspectObject?: (objectId: string, retainFieldData?: boolean) => Promise<unknown>;   // NEW
  findAllGcPaths?: (objectId: string, maxPaths?: number) => Promise<unknown>;          // NEW
};
```

An `isInspectObjectAvailable()` / `isFindAllGcPathsAvailable()` pair follows the exact existing `isReferencesAvailable()`/`isReferrersAvailable()` pattern — this is a mechanical extension of an already-proven shape, not a new design.

### 6.1 Open question for Slice 14.A to resolve, not this doc

Whether `diffObjects`/`listSnapshots`/`startWorkflow`/`nextStep` belong on the existing `HeapExplorerHostBridge`, the existing `LeakWorkspaceHostBridge`, or a new third bridge (e.g. `__MNEMOSYNE_WORKFLOW_BRIDGE__`) is a real design decision this doc deliberately leaves open — resolve it by reading `tauri/src/bridge.ts` (the actual injection site for both existing bridges) at implementation time and following whichever grouping that file's own conventions suggest, documenting the choice in the slice's commit body per this session's established practice of recording judgment calls inline.

## 7. Sub-slice plan

All slices end with `bun run test` (or `cd ui && bun run test`) and `tsc --noEmit` clean, plus in-browser verification (golden path + one edge case) via the preview tool, before commit — mirroring the Rust side's `cargo {check,test,clippy,fmt}` gate.

### Slice 14.A — Artifact-type extension + Comparison Basket UI

- **Scope:** Widen `AnalysisArtifact` (§6, item 1). New `/compare` route, `ui/src/features/comparison/` (picker, tables, match-quality badge), new `comparison-store.ts`. Resolves §6.1's bridge-placement question.
- **Files owned:** `ui/src/lib/analysis-types.ts`, `ui/src/app/router.tsx` (new route), `ui/src/features/comparison/*` (new).
- **Validation gates:** picking two loaded artifacts renders correct added/removed/retained_changed tables against a known fixture pair; match-quality badge reflects the fixture's actual collision rate; empty-diff case renders an explicit "no differences" state, not a blank table.
- **Target size:** ~400 LOC + tests.

### Slice 14.B — Object Inspector + GC-path extensions

- **Scope:** §4 items 2–3. New `inspectObject`/`findAllGcPaths` bridge methods, `ObjectInspectorPanel.tsx` chip navigation, `LeakGcPathPage.tsx` multi-path list.
- **Files owned:** `ui/src/features/heap-explorer/{heap-explorer-query-client.ts,ObjectInspectorPanel.tsx,HeapObjectInspectorPage.tsx}`, `ui/src/features/leak-workspace/{live-detail-client.ts,LeakGcPathPage.tsx}` (whichever bridge §6.1 resolved to own the new methods).
- **Validation gates:** clicking a ref/referrer chip navigates to that object's inspector view; bridge-absent state still renders today's existing view unchanged (regression gate).
- **Target size:** ~300 LOC + tests.

### Slice 14.C — Referrer panel + Classloader panel + Thread view

- **Scope:** §4 items 4, 5, 8. Three new/extended panels, all artifact-backed (no new bridge calls — pure consumption of Slice 14.A's widened type).
- **Files owned:** `ui/src/features/artifact-explorer/*` (referrer + classloader panels), new `ui/src/features/threads/*` or `heap-explorer/HeapThreadsPage.tsx` (implementation's call per §5's placement note), `ui/src/app/router.tsx` (thread route).
- **Validation gates:** each panel renders correctly against a real artifact JSON containing the relevant M8/M13 fields; each panel's absence-state (field not present in an older/pre-M8 artifact) degrades gracefully, not a crash.
- **Target size:** ~350 LOC + tests.

### Slice 14.D — AI-guided landing + workflow cards + snapshot picker

- **Scope:** §4 items 7, 9. Depends on 14.A (comparison view it deep-links to) and the bridge extensions from 14.A/14.B for the workflow/snapshot bridge methods.
- **Files owned:** `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx` (becomes the guided landing) or a new sibling route if replacing in place proves disruptive to the existing artifact-drop flow (implementation's call, documented), new workflow-bridge client file, new persistent top-nav component.
- **Validation gates:** guided landing still accepts a dropped artifact exactly as today (hard regression gate — the existing drop flow must not break); every power route remains reachable from top-nav; NL input bar correctly routes a query-shaped input to the query console path and a workflow-shaped input to `start_workflow` (define the disambiguation heuristic during implementation, document it).
- **Target size:** ~450 LOC + tests.

### Slice 14.E — Visual pass + documentation sync

- **Scope:** Load `frontend-design` skill, review all four prior slices' components against `globals.css`'s tokens for consistency, fix any drift. Doc sync: `docs/roadmap.md` (M14 shipped), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md`'s Browser-First UI Layer section (route map table, feature-areas list), design doc closeout.
- **Files owned:** Any UI file needing visual-consistency fixes (small, targeted); documentation files.
- **Validation gates:** `bun run test` / `tsc --noEmit` clean across the whole `ui/` workspace (not just this slice's own files). Full route map in `ARCHITECTURE.md` matches `router.tsx` exactly.
- **Target size:** Small (fixes) + documentation.

## 8. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | UI test/verification discipline is lighter-precedent in this repo than the Rust side | Hold `bun run test` + `tsc --noEmit` + in-browser preview-tool verification as a hard per-slice gate, same as `cargo` on the Rust side — no slice merges without it. |
| R2 | Guided landing accidentally becomes a gate in front of power views (violates the explicit "don't dumb down" constraint) | Persistent top-nav to every power route is part of Slice 14.D's own validation gates, not a follow-up. |
| R3 | `AnalysisArtifact`'s actual current field set (pre-M14) is not fully known until implementation reads it directly — this doc's §6 types are illustrative | Slice 14.A's first action is reading the real current type file end-to-end before writing any new field, exactly as every Rust slice this session read its target file in full before extending it. |
| R4 | New bridge methods invented here don't match what `tauri/src/bridge.ts`/`commands.rs` can actually support without new Rust work | §4's "Out" scope explicitly caps Tauri command additions to what a slice's own UI needs to function at all; anything wider is M16's problem. |
| R5 | Visual drift across four slices built somewhat independently | Slice 14.E is a dedicated visual-consistency pass before this milestone is considered done, not an afterthought. |

## 9. Testing strategy

- Per-slice: `bun run test`, `tsc --noEmit`, in-browser golden-path + one-edge-case verification via the preview tool (screenshot/read_page as needed to confirm rendering, not just "the dev server started").
- Regression: the existing artifact-drop flow, existing `/heap-explorer/*` and `/leaks/*` routes, and every existing bridge method's current behavior must be unchanged when a new optional bridge method is absent — same additive-and-degrade-gracefully discipline the Rust side held for every additive JSON field this whole session.
- Cross-slice: Slice 14.E's full-workspace `bun run test`/`tsc --noEmit` run is the final regression gate before doc sync.

## 10. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M14 entry.
- Backend capability being surfaced: [milestone-8-reachability-references.md](milestone-8-reachability-references.md), [milestone-9-snapshot-persistence.md](milestone-9-snapshot-persistence.md), [milestone-8-1-object-level-diff.md](milestone-8-1-object-level-diff.md) (M10), [milestone-11-mcp-workflow-suite.md](milestone-11-mcp-workflow-suite.md), [milestone-13-classloader-explorer.md](milestone-13-classloader-explorer.md).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) §"Browser-First UI Layer" — to be updated in Slice 14.E.
- Existing bridge pattern (structural template): `ui/src/features/heap-explorer/heap-explorer-query-client.ts`, `ui/src/features/leak-workspace/live-detail-client.ts`.
- Successor: M16 (desktop packaging) depends on this milestone shipping a GUI worth packaging.

## 11. Implementation readiness verdict

**READY.** Slice 14.A first (establishes the widened artifact type every later slice's panels consume, and resolves the open bridge-placement question in §6.1). Slices 14.B and 14.C are file-disjoint from each other once 14.A lands and may run in **separate isolated git worktrees** (this repository's own session history has repeatedly required isolated worktrees for real parallel agent work — same requirement applies here, not just to the Rust side). Slice 14.D depends on both 14.A (comparison deep-link) and whichever of 14.B/14.C land the bridge methods it reuses. Slice 14.E is gated behind 14.A–14.D.
