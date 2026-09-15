# M27 Durable Investigations and Compare Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore display-safe workspace state and make snapshot reopen and current-vs-baseline comparison part of the same investigation shell.

**Architecture:** Persist metadata only from `useInvestigationStore`; hydrate graph/facts transactionally through the existing snapshot and comparison bridges. Do not serialize graphs, absolute paths, or sensitive field values into browser storage.

**Tech Stack:** React, Zustand, Tauri, `SnapshotStore`, existing object-diff engine.

**Roadmap:** [M27 — Durable investigations + compare](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m27--durable-investigations--compare)

## Global Constraints

- M26 revision/op enforcement is a prerequisite for async hydration.
- Persist only layout, filters, stable selection IDs, notes/bookmarks, and opaque snapshot/source keys.
- Snapshot graph + facts + mode + compatible selection commit atomically.
- Reuse `ui/src/features/comparison/`, `diff_objects_for_session`, and `SnapshotStore`; do not add a diff analyzer.
- WSL cannot prove packaged GUI persistence; focused tests locally, full/native evidence in CI/native hosts.

---

## File map

### M27.A — Workspace persistence

- Create `ui/src/features/investigation/workspace-persistence.ts` — strict schema parser, migration boundary, compatibility filtering, and a small `sessionStorage` adapter.
- Create `ui/src/features/investigation/workspace-persistence.test.ts` — schema, hostile-payload, migration, storage, and stale-selection tests without mounting React routes.
- Modify `ui/src/features/investigation/investigation-store.ts` — additive workspace identity, notes/bookmarks, persistence activation, and metadata-only auto-save.
- Modify `ui/src/features/investigation/investigation-store.test.ts` — focused store tests for restore, stale-ID reporting, and note/bookmark isolation.
- Modify `ui/src/features/investigation/workspace-actions.ts` — activate persistence only after an opened artifact has committed and provide current-revision ID compatibility sets.
- Modify `ui/src/features/investigation/workspace-actions.test.ts` — focused return-to-workspace test using the real stores and in-memory browser storage.

### M27.B/C — Later slices

- Snapshot hydration: `ui/src/features/snapshots/`, `ui/src/features/investigation/workspace-actions.ts`, `ui/src/host/tauri-bridge.ts`, `tauri/src/commands.rs`.
- Compare integration: `ui/src/features/comparison/ComparisonPage.tsx`, `ComparisonPicker.tsx`, `comparison-store.ts`, `tauri/session-ops/src/lib.rs`.

### M27.A — Workspace persistence

#### Frozen persistence contract

Only this versioned metadata envelope may enter browser storage:

```ts
type WorkspacePersistenceIdentity = {
  kind: "workspace" | "snapshot";
  key: string; // opaque host/snapshot identity; never a path
};

type PersistedWorkspaceV1 = {
  schemaVersion: 1;
  identity: WorkspacePersistenceIdentity;
  revision: number;
  layout: { activePane?: InvestigationOriginPane };
  filters: { histogram: HistogramViewState };
  selection: {
    revision: number;
    objectId?: string;
    classKey?: string;
    leakId?: string;
  };
  notes: Array<{
    id: string;
    target: { kind: "workspace" | "object" | "class" | "leak"; id: string };
    text: string;
  }>;
  bookmarks: Array<{
    id: string;
    target: { kind: "object" | "class" | "leak"; id: string };
    label?: string;
  }>;
};
```

The parser is strict: unknown keys, absolute-path-shaped strings, `graph`/`artifact` payload keys, invalid enum values, unsafe revisions, over-limit text/arrays, and non-opaque identity keys reject the entire record. It does not retain `activeOperation`, analysis facts, field values, heap paths, source roots, graph nodes, or imported artifacts. Version `1` is the only accepted version in this slice; missing/older/unknown versions return an explicit unsupported-schema result so a future migration can be added without guessing.

Selection restoration is revision-aware and evidence-based. The adapter receives compatibility sets built from the newly committed artifact. Each stored `objectId`, `classKey`, and `leakId` is restored only when its set contains that ID; accepted IDs are rebound to the current revision. Missing IDs are omitted and returned in `droppedSelectionIds`, allowing callers/tests to report stale state honestly. Layout, filters, notes, and bookmarks may restore when the opaque identity matches even when individual selection IDs are stale.

#### Task 1: Add the strict schema and storage adapter

**Files:**
- Create: `ui/src/features/investigation/workspace-persistence.ts`
- Create: `ui/src/features/investigation/workspace-persistence.test.ts`

**Interfaces:**
- Produces: `WORKSPACE_PERSISTENCE_SCHEMA_VERSION = 1`.
- Produces: `WorkspacePersistenceIdentity`, `WorkspaceNote`, `WorkspaceBookmark`, `PersistedWorkspaceV1`.
- Produces: `parsePersistedWorkspace(value: unknown): WorkspaceParseResult`.
- Produces: `restoreCompatibleWorkspace(record, context): WorkspaceRestoreResult`, where `context` contains exact `identity`, current `revision`, and `ReadonlySet<string>` values for object/class/leak IDs.
- Produces: `createWorkspacePersistence(storage?: Storage): WorkspacePersistence`, with `load(identity)`, `save(record)`, and `remove(identity)`.
- Consumes: `HistogramViewState` and `InvestigationOriginPane` from `investigation-store.ts` as type-only imports.

- [ ] **Step 1: Write failing schema allow-list tests**

```ts
it("accepts only versioned display-safe metadata", () => {
  const parsed = parsePersistedWorkspace(validRecord);
  expect(parsed).toEqual({ status: "ready", record: validRecord });
});

it.each([
  { ...validRecord, heapPath: "/srv/heaps/prod.hprof" },
  { ...validRecord, graph: { nodes: [{ fieldValue: "secret" }] } },
  { ...validRecord, artifact: { summary: {} } },
])("rejects paths, graphs, and artifacts", (candidate) => {
  expect(parsePersistedWorkspace(candidate).status).toBe("rejected");
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cd ui && bun test src/features/investigation/workspace-persistence.test.ts --max-concurrency=1`

Expected: FAIL because `workspace-persistence.ts` and its exported parser do not exist.

- [ ] **Step 3: Implement the V1 types and strict parser**

```ts
export const WORKSPACE_PERSISTENCE_SCHEMA_VERSION = 1 as const;

export type WorkspaceParseResult =
  | { status: "ready"; record: PersistedWorkspaceV1 }
  | { status: "unsupported-schema"; schemaVersion?: number }
  | { status: "rejected"; reason: string };

export function parsePersistedWorkspace(value: unknown): WorkspaceParseResult {
  // Validate the complete allow-listed shape and bounds before constructing
  // a fresh PersistedWorkspaceV1 object. Never spread untrusted input.
}
```

Validation bounds for V1: identity/target IDs `1..512` characters, note IDs/bookmark IDs `1..128`, note text `0..2000`, bookmark label `0..200`, at most `100` notes and `100` bookmarks, histogram page offset a non-negative safe integer, and all enum values from the existing store unions.

- [ ] **Step 4: Add failing compatibility and migration tests**

```ts
it("rebinds compatible IDs and reports stale IDs", () => {
  const restored = restoreCompatibleWorkspace(validRecord, {
    identity: validRecord.identity,
    revision: 9,
    objectIds: new Set(["obj-current"]),
    classKeys: new Set(["class-current"]),
    leakIds: new Set<string>(),
  });
  expect(restored.selection).toEqual({
    revision: 9,
    objectId: "obj-current",
    classKey: "class-current",
  });
  expect(restored.droppedSelectionIds).toEqual([{ kind: "leak", id: "leak-stale" }]);
});

it("does not guess how to migrate an unknown schema", () => {
  expect(parsePersistedWorkspace({ ...validRecord, schemaVersion: 2 })).toEqual({
    status: "unsupported-schema",
    schemaVersion: 2,
  });
});
```

- [ ] **Step 5: Implement compatibility filtering and session storage**

```ts
export type WorkspacePersistence = {
  load(identity: WorkspacePersistenceIdentity): WorkspaceParseResult | { status: "missing" };
  save(record: PersistedWorkspaceV1): { status: "saved" } | { status: "unavailable" };
  remove(identity: WorkspacePersistenceIdentity): void;
};

export function createWorkspacePersistence(
  storage: Storage | undefined = globalThis.sessionStorage,
): WorkspacePersistence {
  // Namespace by schema + encoded opaque identity, catch unavailable/quota
  // errors, and parse every loaded value through parsePersistedWorkspace.
}
```

- [ ] **Step 6: Run focused tests and verify GREEN**

Run: `cd ui && bun test src/features/investigation/workspace-persistence.test.ts --max-concurrency=1`

Expected: all adapter tests PASS with no route mount and no console warnings.

- [ ] **Step 7: Commit the adapter**

```bash
git add ui/src/features/investigation/workspace-persistence.ts \
  ui/src/features/investigation/workspace-persistence.test.ts
git commit -m "feat(ui): add safe workspace persistence"
```

#### Task 2: Persist investigation metadata, notes, and bookmarks

**Files:**
- Modify: `ui/src/features/investigation/investigation-store.ts`
- Modify: `ui/src/features/investigation/investigation-store.test.ts`

**Interfaces:**
- Consumes: `createWorkspacePersistence`, `restoreCompatibleWorkspace`, and V1 metadata types from Task 1.
- Produces: additive store fields `persistenceIdentity?`, `notes`, `bookmarks`, and `lastPersistenceNotice?`.
- Produces: `activatePersistence(identity, compatibility): WorkspaceRestoreResult | undefined`.
- Produces: `deactivatePersistence(): void`.
- Produces: `upsertNote(note)`, `removeNote(noteId)`, `upsertBookmark(bookmark)`, and `removeBookmark(bookmarkId)`.
- Preserves: every M26 operation/revision action and existing selector name.

- [ ] **Step 1: Write failing store restoration tests**

```ts
it("restores compatible metadata for one opaque workspace identity", () => {
  seedPersistedWorkspace("source-a", {
    objectId: "obj-a",
    classKey: "class-a",
    leakId: "leak-stale",
  });
  useInvestigationStore.getState().activatePersistence(
    { kind: "workspace", key: "source-a" },
    {
      revision: 4,
      objectIds: new Set(["obj-a"]),
      classKeys: new Set(["class-a"]),
      leakIds: new Set<string>(),
    },
  );
  expect(useInvestigationStore.getState()).toMatchObject({
    revision: 4,
    objectId: "obj-a",
    classKey: "class-a",
    leakId: undefined,
  });
  expect(useInvestigationStore.getState().lastPersistenceNotice).toContain("leak-stale");
});
```

- [ ] **Step 2: Run the store test and verify RED**

Run: `cd ui && bun test src/features/investigation/investigation-store.test.ts --max-concurrency=1`

Expected: FAIL because persistence activation and metadata actions do not exist.

- [ ] **Step 3: Add minimal additive store state/actions**

```ts
activatePersistence: (identity, compatibility) => {
  const loaded = workspacePersistence.load(identity);
  if (loaded.status !== "ready") {
    set({ persistenceIdentity: identity, notes: [], bookmarks: [] });
    return undefined;
  }
  const restored = restoreCompatibleWorkspace(loaded.record, compatibility);
  set({
    persistenceIdentity: identity,
    revision: compatibility.revision,
    ...restored.selection,
    originPane: restored.layout.activePane,
    histogramView: restored.filters.histogram,
    notes: restored.notes,
    bookmarks: restored.bookmarks,
    lastPersistenceNotice: formatDroppedSelectionNotice(restored.droppedSelectionIds),
  });
  return restored;
},
```

Each existing filter/selection mutation and each note/bookmark mutation writes a newly constructed V1 envelope only when `persistenceIdentity` is active. `activeOperation` is never included. `deactivatePersistence()` saves the current metadata and unbinds the identity without deleting its session record.

- [ ] **Step 4: Add note/bookmark identity-isolation tests**

```ts
it("keeps notes and bookmarks isolated by opaque identity", () => {
  activate("source-a");
  useInvestigationStore.getState().upsertNote(noteA);
  deactivate();
  activate("source-b");
  expect(useInvestigationStore.getState().notes).toEqual([]);
  expect(useInvestigationStore.getState().bookmarks).toEqual([]);
});
```

- [ ] **Step 5: Run store and adapter tests**

Run: `cd ui && bun test src/features/investigation/workspace-persistence.test.ts src/features/investigation/investigation-store.test.ts --max-concurrency=1`

Expected: all tests PASS; existing M26 operation tests remain green.

- [ ] **Step 6: Commit store wiring**

```bash
git add ui/src/features/investigation/investigation-store.ts \
  ui/src/features/investigation/investigation-store.test.ts
git commit -m "feat(ui): persist investigation metadata"
```

#### Task 3: Restore compatible state after an artifact commits

**Files:**
- Modify: `ui/src/features/investigation/workspace-actions.ts`
- Modify: `ui/src/features/investigation/workspace-actions.test.ts`

**Interfaces:**
- Consumes: `activatePersistence` after `setArtifact`, never before it.
- Produces: compatibility sets from `artifact.graph.dominators[].objectId`, `artifact.histogram.entries[].key`, and `artifact.leaks[].id`.
- Preserves: failed/cancelled opens do not switch persistence identity or alter the current workspace.
- Defers: snapshot-first hydration and snapshot-key activation to M27.B.

- [ ] **Step 1: Write a failing return-to-workspace test**

```ts
it("restores only selections present in the reopened artifact", () => {
  applyOpenedHeap("a.hprof", artifactA, "source-a");
  useInvestigationStore.getState().setObjectId("obj-a", "inspector");
  useInvestigationStore.getState().setLeakId("leak-removed", "leak");
  applyOpenedHeap("b.hprof", artifactB, "source-b");
  applyOpenedHeap("a.hprof", artifactAWithoutRemovedLeak, "source-a");
  expect(useInvestigationStore.getState().objectId).toBe("obj-a");
  expect(useInvestigationStore.getState().leakId).toBeUndefined();
  expect(useInvestigationStore.getState().lastPersistenceNotice).toContain("leak-removed");
});
```

- [ ] **Step 2: Run the workspace-actions test and verify RED**

Run: `cd ui && bun test src/features/investigation/workspace-actions.test.ts --max-concurrency=1`

Expected: FAIL because `applyOpenedHeap` does not activate or restore persisted metadata.

- [ ] **Step 3: Activate persistence after artifact commit**

```ts
useInvestigationStore.getState().bumpRevisionOnArtifactChange();
useArtifactStore.getState().setArtifact(displayName, artifact);
const revision = useInvestigationStore.getState().revision;
if (sourceId) {
  useInvestigationStore.getState().activatePersistence(
    { kind: "workspace", key: sourceId },
    buildArtifactCompatibility(artifact, revision),
  );
}
```

Browser-only imported JSON has no opaque host identity, so it remains intentionally session-memory-only. `applyOpenedSnapshotSession` stays unchanged in M27.A because the current bridge does not return a complete hydrate envelope or stable snapshot identity; M27.B owns that transaction.

- [ ] **Step 4: Run the focused M27.A matrix**

Run:

```bash
cd ui
bun test src/features/investigation/workspace-persistence.test.ts \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  --max-concurrency=1
bun run lint
bun run build
```

Expected: all focused tests PASS, TypeScript emits no errors, and the production UI build completes.

- [ ] **Step 5: Commit lifecycle integration**

```bash
git add ui/src/features/investigation/workspace-actions.ts \
  ui/src/features/investigation/workspace-actions.test.ts
git commit -m "feat(ui): restore compatible workspace state"
```

- [ ] **Step 6: Pre-push scope check**

Run: `git diff --check && git status --short && git log --oneline -4`

Expected: only M27.A plan/adapter/store/action files are changed or committed; M27.B/C, comparison, Tauri, Rust, and untracked `.claude/skills/gitnexus-*` remain untouched.

### M27.B — Snapshot-first reopen

- [ ] Expand the plan around existing `openSnapshot`/`open_snapshot_for_session`; no parallel snapshot loader.
- [ ] Return one hydrate envelope containing snapshot identity, mode/capabilities, analysis facts, and revision/op context.
- [ ] Race-test failure/old-revision behavior so the prior workspace remains intact.

### M27.C — Integrated compare

- [ ] Move current/baseline selection into workbench chrome while preserving the standalone `/compare` route as an adapter.
- [ ] Expose identity strategy, top-N, cross-reference-leaks, and match quality.
- [ ] Make after-side object deltas set shared `objectId` and open Inspector.
- [ ] Record focused evidence and `NOT PROVEN` packaged behavior before marking M27 shipped.
