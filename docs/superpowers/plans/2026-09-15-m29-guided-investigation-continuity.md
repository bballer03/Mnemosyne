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

#### M29.B file map

- Create `ui/src/features/workflow-landing/workflow-types.ts` — shared workflow kind and workspace-binding types without bridge or React dependencies.
- Create `ui/src/features/workflow-landing/workflow-binding.ts` — workspace-correlated start/resume/advance/close adapter; UI callers never supply workflow IDs.
- Create `ui/src/features/workflow-landing/workflow-binding.test.ts` — focused stale-result, supersession, recovery, close, and path-safety tests.
- Modify `ui/src/features/investigation/investigation-store.ts` — one revision-bound active workflow plus separate workflow/Assistant request correlation slots.
- Modify `ui/src/features/investigation/investigation-store.test.ts` — binding uniqueness, stale request rejection, revision invalidation, and persistence recovery tests.
- Modify `ui/src/features/investigation/workspace-persistence.ts` — optional display-safe workflow binding in the existing workspace record.
- Modify `ui/src/features/investigation/workspace-persistence.test.ts` — schema validation and identity/revision compatibility tests.
- Modify `ui/src/features/investigation/workspace-actions.ts` — best-effort close, otherwise detach, before Close/replace commits a new revision.
- Modify `ui/src/features/investigation/workspace-actions.test.ts` — focused Close/replace workflow cleanup tests.
- Modify `ui/src/features/workflow-landing/WorkflowCard.tsx` and `WorkflowCard.test.tsx` — bind cards to the active workspace workflow and remove pasted-ID resume.
- Modify `ui/src/features/workflow-landing/NaturalLanguageInputBar.tsx` and `NaturalLanguageInputBar.test.tsx` — route workflow starts through the same binding adapter.
- Modify `ui/src/features/investigation/HeapSessionBar.tsx` and `HeapSessionBar.test.tsx` — show kind/current step only.
- Modify `ui/src/features/assistant/InvestigationAssistantPage.tsx` and `InvestigationAssistantPage.test.tsx` — show and consume the current bound step; reject late turns after workspace change.
- Modify `ui/src/features/assistant/assistant-bridge-client.ts` and `assistant-bridge-client.test.ts` — use display-safe workflow kind/step context without exposing the workflow ID.

#### Workflow binding contract

```ts
export type WorkspaceWorkflowBinding = Readonly<{
  workspaceId: string;
  revision: number;
  workflowId: string;
  kind: WorkflowKindId;
  currentStep: string;
}>;

export type WorkspaceRequestContext = Readonly<{
  workspaceId: string;
  revision: number;
  operationId: string;
}>;
```

Only the binding adapter reads or passes `workflowId`. React controls start, resume, advance, and close through the active binding; no text input accepts an ID and no chrome/Assistant copy renders one. Every asynchronous workflow or Assistant request captures `{ workspaceId, revision, operationId }`; only the latest matching request may commit. A revision bump clears request slots and detaches the active binding so late success cannot repopulate the replaced workspace.

Persistence stores only `{ workflowId, kind, currentStep, revision }` under the already opaque workspace/snapshot identity. It never stores a heap path, step result, workflow history, or Assistant history. Restore accepts the candidate only when the persisted identity matches, the workflow revision matches the persisted workspace revision, kind/ID/step pass strict validation, and `getWorkflow` confirms the same ID and kind. Failed/unavailable recovery detaches locally and leaves deterministic workbench tools usable.

#### Task M29.B.1: Revision-bound workflow state and persistence

**Interfaces:**
- `beginWorkspaceRequest("workflow" | "assistant"): WorkspaceRequestContext`
- `acceptWorkspaceRequest(slot, context): boolean`
- `bindWorkflow(context, kind, result): boolean`
- `detachWorkflow(expectedWorkflowId?): WorkspaceWorkflowBinding | undefined`
- `PersistedWorkspaceV1.workflow?: PersistedWorkflowBinding`

- [ ] **Step 1: Write failing store and persistence tests**

Add focused tests proving:

```ts
const request = store.beginWorkspaceRequest("workflow");
expect(store.bindWorkflow(request, "tune_gc", {
  workflowId: "wf-1",
  currentStep: "thread_local_review",
})).toBe(true);
expect(useInvestigationStore.getState().activeWorkflow).toMatchObject({
  workspaceId: "workspace-1",
  revision: 0,
  workflowId: "wf-1",
  kind: "tune_gc",
  currentStep: "thread_local_review",
});
```

Also assert that another workspace, old revision, and superseded request ID return `false`; only one binding exists; `bumpRevisionOnArtifactChange()` clears both request slots and the binding; persisted JSON contains kind/step/ID but no step result, history, or heap path; invalid kinds/path-like IDs are rejected; and restore drops a workflow whose workflow revision differs from the persisted workspace revision.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cd ui
bun test \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-persistence.test.ts \
  --max-concurrency=1
```

Expected: FAIL because request slots, active workflow state, and persisted workflow validation do not exist.

- [ ] **Step 3: Implement the minimal state and schema**

Add the shared workflow types, one active binding, independent workflow/Assistant request slots, strict request acceptance, and an optional workflow persistence field. Rebind a structurally compatible persisted candidate to the newly opened revision, but mark it for host confirmation before a workflow card may advance it. Keep operation payloads, results, histories, absolute paths, and raw field values out of persistence.

- [ ] **Step 4: Re-run focused tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.B.2: Context-safe workflow lifecycle adapter

**Interfaces:**
- `startWorkspaceWorkflow(kind, params): Promise<WorkspaceWorkflowResult>`
- `recoverWorkspaceWorkflow(): Promise<WorkspaceWorkflowResult>`
- `advanceWorkspaceWorkflow(input?): Promise<WorkspaceWorkflowResult>`
- `closeWorkspaceWorkflow(): Promise<WorkspaceWorkflowResult>`
- `closeOrDetachWorkspaceWorkflow(): Promise<void>`

- [ ] **Step 1: Write failing adapter tests**

Using only the Zustand store plus a fake `__MNEMOSYNE_WORKFLOW_BRIDGE__`, assert:

1. start captures the current request context and binds the returned ID/kind/step;
2. a deferred start resolved after a revision bump returns `stale` and does not bind;
3. the older of two same-revision starts cannot overwrite the newer request;
4. advance obtains the ID from the active binding and rejects a late result after replacement;
5. recovery calls `getWorkflow` with the persisted internal ID, accepts only the same ID/kind, and detaches on mismatch/corruption/unavailability;
6. close sends the internal ID and detaches only the matching binding;
7. serialized/display-facing results contain no heap path.

- [ ] **Step 2: Run adapter tests and verify RED**

Run:

```bash
cd ui
bun test src/features/workflow-landing/workflow-binding.test.ts --max-concurrency=1
```

Expected: FAIL because `workflow-binding.ts` does not exist.

- [ ] **Step 3: Implement the minimal lifecycle adapter**

Wrap the existing bridge client. Begin a workflow request before each host call, commit only through the matching request context, and return a distinct `stale` result when the workspace/revision/request no longer matches. Starting a new kind first best-effort closes the prior binding; host absence or close failure detaches locally. Recovery validates host ID and kind before clearing the persisted-candidate marker. Do not add native commands or widen core workflow payloads.

- [ ] **Step 4: Re-run adapter tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.B.3: ID-free workflow cards and natural-language starts

**Interfaces:**
- `WorkflowCard` selects `activeWorkflow` and never accepts a resume ID from the user.
- A matching persisted candidate is recovered automatically on focused card mount.
- Continue/close call the binding adapter without an ID argument.
- `NaturalLanguageInputBar` uses `startWorkspaceWorkflow`.

- [ ] **Step 1: Replace pasted-ID tests with focused failing binding tests**

Render one `WorkflowCard` inside `MemoryRouter`; never mount `App` or production routes. Assert there is no “Resume workflow id” textbox and no rendered workflow ID. Seed a compatible persisted binding, render its matching card, and assert `getWorkflow("wf-internal")` runs automatically and the current step appears. Assert another kind does not recover or advance that binding. Start/continue/close through the card and verify store step updates.

For `NaturalLanguageInputBar`, assert a free-text start populates `activeWorkflow`; resolve an older deferred start after a newer start and assert the older result is not rendered or stored.

- [ ] **Step 2: Run component tests and verify RED**

Run:

```bash
cd ui
bun test \
  src/features/workflow-landing/WorkflowCard.test.tsx \
  src/features/workflow-landing/NaturalLanguageInputBar.test.tsx \
  --max-concurrency=1
```

Expected: FAIL because both components still call raw bridge methods and the card exposes pasted-ID resume.

- [ ] **Step 3: Implement bound workflow controls**

Remove `resumeId`, the ID textbox, and every rendered workflow ID. Select the matching active binding, automatically confirm compatible persisted state, and route start/continue/close through the lifecycle adapter. Render only display-safe kind labels, current step, deterministic deep links, and bounded status/error copy; do not render raw host workflow state or heap paths.

- [ ] **Step 4: Re-run component tests and verify GREEN**

Run the Step 2 command. Expected: PASS.

#### Task M29.B.4: Workspace cleanup, chrome, and Assistant correlation

**Interfaces:**
- `applyOpenedHeap`, `applySnapshotWorkspaceHydrate`, and `closeInvestigationWorkspace` close-or-detach the prior active workflow before committing replacement.
- `HeapSessionBar` renders `Workflow: <kind label> · current step: <step>`.
- `InvestigationAssistantPage` reads the same binding and captures an Assistant request context before awaiting provider/rules execution.

- [ ] **Step 1: Write focused failing lifecycle and UI tests**

Add tests that:

- Close calls `closeWorkflow` once with the internal ID and clears the binding even when close rejects or is unavailable;
- heap and snapshot replacement detach immediately, and a late close result cannot clear a newer binding;
- `HeapSessionBar` shows kind/current step but contains neither workflow ID nor `/secret/heap.hprof`;
- Assistant measured facts show the same kind/current step without ID/path;
- Assistant rules context mentions kind/current step, not ID;
- a deferred Assistant response resolved after `bumpRevisionOnArtifactChange()` does not append a turn or restore a session into the new workspace.

Render `HeapSessionBar` with a small `MemoryRouter` and render `InvestigationAssistantPage` with `MemoryRouter`; do not use `App`, `router.tsx`, or a production route tree.

- [ ] **Step 2: Run lifecycle and UI tests and verify RED**

Run:

```bash
cd ui
bun test \
  src/features/investigation/workspace-actions.test.ts \
  src/features/investigation/HeapSessionBar.test.tsx \
  src/features/assistant/assistant-bridge-client.test.ts \
  src/features/assistant/InvestigationAssistantPage.test.tsx \
  --max-concurrency=1
```

Expected: FAIL because workspace actions do not close workflows, chrome/Assistant do not select the binding, and Assistant responses are not revision-correlated.

- [ ] **Step 3: Implement lifecycle cleanup and display-safe step surfaces**

Capture and detach the old binding before revision replacement; invoke host close best-effort so failure cannot block Open/Close. In chrome and Assistant, map kind to a human label and render only current step. Replace Assistant’s local workflow placeholders with the active binding, omit workflow ID from local rules/provider context, and append a turn/session only when the captured Assistant request remains current.

- [ ] **Step 4: Run the complete focused M29.B suite**

Run:

```bash
cd ui
bun test \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-persistence.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  src/features/investigation/HeapSessionBar.test.tsx \
  src/features/workflow-landing/workflow-bridge-client.test.ts \
  src/features/workflow-landing/workflow-binding.test.ts \
  src/features/workflow-landing/WorkflowCard.test.tsx \
  src/features/workflow-landing/NaturalLanguageInputBar.test.tsx \
  src/features/assistant/assistant-bridge-client.test.ts \
  src/features/assistant/InvestigationAssistantPage.test.tsx \
  --max-concurrency=1
bun run build
```

Expected: all focused tests PASS and the TypeScript/Vite production build exits 0.

- [ ] **Step 5: Commit M29.B implementation**

```bash
git add \
  ui/src/features/investigation/investigation-store.ts \
  ui/src/features/investigation/investigation-store.test.ts \
  ui/src/features/investigation/workspace-persistence.ts \
  ui/src/features/investigation/workspace-persistence.test.ts \
  ui/src/features/investigation/workspace-actions.ts \
  ui/src/features/investigation/workspace-actions.test.ts \
  ui/src/features/investigation/HeapSessionBar.tsx \
  ui/src/features/investigation/HeapSessionBar.test.tsx \
  ui/src/features/workflow-landing/workflow-types.ts \
  ui/src/features/workflow-landing/workflow-binding.ts \
  ui/src/features/workflow-landing/workflow-binding.test.ts \
  ui/src/features/workflow-landing/WorkflowCard.tsx \
  ui/src/features/workflow-landing/WorkflowCard.test.tsx \
  ui/src/features/workflow-landing/NaturalLanguageInputBar.tsx \
  ui/src/features/workflow-landing/NaturalLanguageInputBar.test.tsx \
  ui/src/features/assistant/assistant-bridge-client.ts \
  ui/src/features/assistant/assistant-bridge-client.test.ts \
  ui/src/features/assistant/InvestigationAssistantPage.tsx \
  ui/src/features/assistant/InvestigationAssistantPage.test.tsx
git commit -m "feat(ui): bind workflows to workspaces"
```

### M29.C — Contextual Assistant

- [ ] Seed advisory context from current stable selection and measured findings only.
- [ ] Preserve 12-turn default/32-turn hard history bounds and provenance on every turn.
- [ ] Keep deterministic panes usable when provider/bridge is unavailable.
- [ ] Record focused rules-mode evidence and mark live-provider/native behavior `NOT PROVEN` unless run.
