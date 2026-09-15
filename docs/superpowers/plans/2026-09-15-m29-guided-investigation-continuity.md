# M29 Guided Investigation Continuity Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind findings, workflows, and advisory Assistant turns to the active deterministic investigation without rewriting measured facts.

**Architecture:** Extend the existing M24 `FindingsAdvisoryPane`, workflow bridge, and Assistant session adapters. Findings retain immutable source facts and stable deep-link targets; workflow/AI state stores only workspace-scoped identity and provenance.

**Tech Stack:** React, Zustand, existing `core::workflow`, MCP/Tauri workflow and AI-session adapters.

**Roadmap:** [M29 — Guided investigation continuity](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m29--guided-investigation-continuity)

## Global Constraints

- AI remains collapsible, advisory, provenance-labelled, and separate from measured facts.
- Every finding deep-links through stable `classKey`/`objectId`/`leakId`; no row-index links.
- Workflow start/resume/close is workspace-scoped; users do not paste IDs.
- Never send or persist absolute paths, raw field values, API keys, or unbounded history.
- M26 stale-result/cancellation rules apply to workflow and Assistant calls.

---

## File map

- Findings: `ui/src/features/investigation/FindingsAdvisoryPane.tsx`, `investigation-store.ts`.
- Workflows: `ui/src/features/workflow-landing/`, `ui/src/host/tauri-bridge.ts`, `tauri/session-ops/src/lib.rs`.
- Assistant: `ui/src/features/assistant/InvestigationAssistantPage.tsx`, `assistant-bridge-client.ts`.

### M29.A — Unified findings queue

#### M29.A file map

- Modify `ui/src/features/investigation/investigation-store.ts` — immutable finding facts, workspace-scoped replacement, and a separate user-status map.
- Modify `ui/src/features/investigation/investigation-store.test.ts` — focused store tests for immutable facts, status separation, source replacement, and stale workspace/revision rejection.
- Create `ui/src/features/investigation/finding-adapters.ts` — pure adapters for leak, classloader, collection, string, array, and policy findings.
- Create `ui/src/features/investigation/finding-adapters.test.ts` — adapter identity, ordering, target, and source-fact tests.
- Modify `ui/src/features/investigation/FindingsAdvisoryPane.tsx` — render the unified store queue, stable links, provenance, and status controls.
- Modify `ui/src/features/investigation/FindingsAdvisoryPane.test.tsx` — focused component tests only; do not mount a production route tree.
- Modify `ui/src/features/policy/PolicyCheckPage.tsx` — publish ready policy violations into the active workspace queue with the request’s captured workspace context.
- Modify `ui/src/features/policy/PolicyCheckPage.test.tsx` — prove policy publication and late-result rejection with a focused `MemoryRouter`.

#### Finding contract

```ts
export type FindingStatus = "open" | "resolved" | "deferred";
export type FindingSource = "artifact" | "policy";
export type FindingKind =
  | "leak"
  | "classloader"
  | "collection-waste"
  | "string-waste"
  | "array-waste"
  | "policy";

export type FindingTarget =
  | Readonly<{ kind: "leak"; leakId: string; classKey?: string; objectId?: string }>
  | Readonly<{ kind: "class"; classKey: string }>
  | Readonly<{ kind: "object"; objectId: string; classKey?: string }>;

export type FindingFact = Readonly<{
  id: string;
  source: FindingSource;
  kind: FindingKind;
  severity: string;
  title: string;
  description: string;
  target: FindingTarget;
  provenance: readonly Readonly<{ kind: string; detail?: string }>[];
  metrics: Readonly<Record<string, string | number>>;
}>;
```

The fact object and its nested target/provenance/metrics are frozen when accepted by the store. User triage lives only in `findingStatuses: Readonly<Record<string, FindingStatus>>`; changing a status must never rewrite or decorate the source fact. Source replacement removes obsolete statuses for that source, preserves statuses for unchanged IDs, and leaves the other source untouched.

Every adapter must produce a deterministic fact ID and one of the three stable targets:

- leaks: `leak:<leakId>` → `leakId` (with matching `classKey` and resolved `objectId` when available);
- classloader potential leaks: `classloader:loader:<hexObjectId>` → `objectId`;
- duplicate classes: `classloader:duplicate:<classKey>` → `classKey`;
- oversized collections: `collection:<hexObjectId>` → `objectId`;
- duplicate strings: deterministic value/count/waste identity → `classKey = "java.lang.String"`;
- duplicate arrays: element type + content hash identity → `classKey = "<elementType>[]"`;
- policy violations: `policy:<ruleId>:<predicate>` → an explicit stable target supplied from the active measured queue. If no relevant `classKey`, `objectId`, or `leakId` exists, omit the policy queue entry rather than inventing a row-index/workspace-only target.

#### Task M29.A.1: Workspace-scoped immutable finding state

**Interfaces:**
- `replaceFindings(context, source, facts): boolean` accepts only the current `{ workspaceId, revision }`.
- `setFindingStatus(findingId, status): boolean` updates only known facts.
- `clearFindings()` clears facts and status on artifact replacement/close.

- [ ] **Step 1: Write failing store tests**

Add focused tests that:

```ts
const context = { workspaceId: "workspace-1", revision: 0 };
const fact = makeFindingFact("leak:leak-1");

expect(store.replaceFindings(context, "artifact", [fact])).toBe(true);
expect(Object.isFrozen(store.findingFacts[0])).toBe(true);
expect(Object.isFrozen(store.findingFacts[0].target)).toBe(true);
expect(store.setFindingStatus(fact.id, "resolved")).toBe(true);
expect(store.findingFacts[0]).toEqual(fact);
expect(store.findingStatuses[fact.id]).toBe("resolved");
```

Also assert that a prior workspace ID and prior revision both return `false` without changing facts/statuses, policy replacement preserves artifact facts, unchanged IDs preserve status, removed IDs drop status, unknown IDs reject status changes, and `bumpRevisionOnArtifactChange()` clears the queue.

- [ ] **Step 2: Run the focused store tests and verify RED**

Run:

```bash
cd ui
bun test src/features/investigation/investigation-store.test.ts --max-concurrency=1
```

Expected: FAIL because the finding contract and actions do not exist.

- [ ] **Step 3: Implement the minimal store contract**

Add the readonly types above, recursively freeze each accepted fact’s owned fields, merge source slices in deterministic artifact-then-policy order, and keep status in a separate map. Do not persist fact payloads or statuses in `workspace-persistence`; M29.A is active-workspace state only.

- [ ] **Step 4: Re-run the focused store tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.A.2: Pure measured-fact adapters

**Interfaces:**
- `buildArtifactFindingFacts(artifact: AnalysisArtifact): readonly FindingFact[]`
- `buildPolicyFindingFacts(violations, measuredFacts): readonly FindingFact[]`
- `findingHref(target: FindingTarget): string`

- [ ] **Step 1: Write failing adapter tests**

Build one artifact fixture containing:

- one leak and matching dominator;
- one classloader potential leak and one duplicate class;
- one oversized collection;
- one duplicate string group;
- one duplicate primitive-array group.

Assert each adapter output keeps the exact source description/metrics, has a deterministic non-index ID, and links through the expected encoded `leakId`, `objectId`, or `classKey`. Reorder each source array and assert IDs/hrefs remain unchanged. Assert no output URL contains a row index or heap path.

For policy, provide violations for leak/classloader/object-growth predicates plus the measured facts. Assert each policy finding chooses a relevant stable measured target, preserves rule ID/predicate/message/severity as immutable facts, and skips an unresolvable generic violation instead of fabricating a target.

- [ ] **Step 2: Run adapter tests and verify RED**

Run:

```bash
cd ui
bun test src/features/investigation/finding-adapters.test.ts --max-concurrency=1
```

Expected: FAIL because `finding-adapters.ts` does not exist.

- [ ] **Step 3: Implement minimal pure adapters**

Use source-owned identifiers (`leak.id`, object IDs, class names, array content hashes) and encoded query/path parameters. Sort by severity rank, then measured waste/retained value, then fact ID. Cap only at presentation time so the store retains the full bounded artifact result.

Policy target resolution order is predicate-aware:

1. `leak_count` / leak-scoped `retained_size` → first measured leak target;
2. `classloader_leak_count` → first measured classloader target;
3. `object_growth_threshold` → first measured object target, then class target;
4. other predicates → no queue entry unless a stable measured target is available from an explicit adapter argument.

- [ ] **Step 4: Re-run adapter tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.A.3: Focused unified queue UI

**Interfaces:**
- `FindingsAdvisoryPane` adapts the loaded artifact into the store using the current workspace context.
- Status controls call `setFindingStatus` and never mutate a `FindingFact`.
- Primary links come only from `findingHref(fact.target)`.

- [ ] **Step 1: Replace the production-route tests with focused failing tests**

Render only:

```tsx
<MemoryRouter>
  <FindingsAdvisoryPane />
</MemoryRouter>
```

Assert all six finding kinds render, the queue uses stable encoded links, `open` is the default status, status changes survive a same-ID artifact refresh, and resolved/deferred controls do not change rendered measured descriptions or metrics. Retain the collapse, Rules/offline label, provenance, and no-heap-path assertions.

- [ ] **Step 2: Run pane tests and verify RED**

Run:

```bash
cd ui
bun test src/features/investigation/FindingsAdvisoryPane.test.tsx --max-concurrency=1
```

Expected: FAIL because the pane still builds a leak-only local queue.

- [ ] **Step 3: Implement the unified pane**

Publish artifact facts in an effect with the captured current `{ workspaceId, revision }`, select the merged store queue, show at most `MAX_VISIBLE_FINDINGS`, and render the target-specific stable link. Keep facts visually authoritative and advisory guidance/provenance visually separate. Use a labelled status `<select>` per finding with `open`, `resolved`, and `deferred`.

- [ ] **Step 4: Re-run pane tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.A.4: Policy result publication and stale rejection

**Interfaces:**
- `PolicyCheckPage.handleRun()` captures `{ workspaceId, revision }` before awaiting the bridge.
- A ready response adapts violations against the current measured facts and calls `replaceFindings(capturedContext, "policy", facts)`.
- Stale replacement returns `false`; the page does not overwrite the newer workspace queue.

- [ ] **Step 1: Write focused failing policy-page tests**

With only `MemoryRouter`, a remembered opaque source, and a fake bridge:

1. resolve a policy violation and assert a `policy:*` fact is added with its measured stable target;
2. start a deferred bridge response, bump the investigation revision, resolve the old response, and assert no old policy fact enters the queue;
3. assert the bridge input and rendered queue contain no heap path.

- [ ] **Step 2: Run policy tests and verify RED**

Run:

```bash
cd ui
bun test src/features/policy/PolicyCheckPage.test.tsx --max-concurrency=1
```

Expected: FAIL because policy results are not published to the investigation store.

- [ ] **Step 3: Implement policy publication**

Capture the workspace context synchronously at run start. After a ready response, derive policy facts from the current measured artifact finding slice and submit them through `replaceFindings`; rely on the store’s workspace/revision gate for late results. Preserve the page’s existing incomplete/skipped semantics.

- [ ] **Step 4: Run the complete focused M29.A suite**

Run:

```bash
cd ui
bun test \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/finding-adapters.test.ts \
  src/features/investigation/FindingsAdvisoryPane.test.tsx \
  src/features/policy/PolicyCheckPage.test.tsx \
  --max-concurrency=1
bun run build
```

Expected: all focused tests PASS and the TypeScript/Vite production build exits 0. Existing Vite advisory/chunk-size warnings are not packaged-GUI evidence.

- [ ] **Step 5: Commit M29.A implementation**

```bash
git add \
  ui/src/features/investigation/investigation-store.ts \
  ui/src/features/investigation/investigation-store.test.ts \
  ui/src/features/investigation/finding-adapters.ts \
  ui/src/features/investigation/finding-adapters.test.ts \
  ui/src/features/investigation/FindingsAdvisoryPane.tsx \
  ui/src/features/investigation/FindingsAdvisoryPane.test.tsx \
  ui/src/features/policy/PolicyCheckPage.tsx \
  ui/src/features/policy/PolicyCheckPage.test.tsx
git commit -m "feat(ui): unify workspace findings queue"
```

### M29.B — Workflow binding

- [ ] Bind one active workflow ID/kind/current step to workspace revision.
- [ ] Auto-close or detach on workspace Close/replace; recover persisted workflows only when compatible.
- [ ] Surface current step in workbench chrome and Assistant without exposing heap paths.

### M29.C — Contextual Assistant

- [ ] Seed advisory context from current stable selection and measured findings only.
- [ ] Preserve 12-turn default/32-turn hard history bounds and provenance on every turn.
- [ ] Keep deterministic panes usable when provider/bridge is unavailable.
- [ ] Record focused rules-mode evidence and mark live-provider/native behavior `NOT PROVEN` unless run.
