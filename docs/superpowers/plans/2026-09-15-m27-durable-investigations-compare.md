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

#### Frozen hydrate contract

Snapshot reopen extends the existing `openSnapshot` → `open_snapshot` → `open_snapshot_for_session` path. No second snapshot loader, command, or store lookup is introduced. The host completes all graph-backed fact derivation before entering the M26 commit gate, then installs the graph/dominator pair and returns one correlated operation envelope:

```ts
type SnapshotWorkspaceHydrate = {
  snapshot: {
    key: string;
    displayName: string;
    sourceId: string;
    schemaVersion: number;
    createdAt: string;
  };
  mode: "deep";
  capabilities: {
    graph: true;
    dominators: true;
    fieldData: boolean;
    snapshotBacked: true;
  };
  analysis: AnalysisArtifact;
};

type SnapshotWorkspaceHydrateEnvelope = OperationEnvelope<SnapshotWorkspaceHydrate>;
```

`analysis.summary.heapPath` is a basename only. Snapshot-derived summary totals are computed from the cached graph and carry explicit partial provenance when raw HPROF record facts are unavailable; they never impersonate a fresh record scan. The UI parses the complete hydrate before mutating state. A failed load, malformed response, cancelled operation, or old `{ workspaceId, revision, operationId }` response leaves the previous artifact, mode/capabilities, selection, remembered source, and persistence identity unchanged.

On accepted success, the host swaps graph + dominator + heap identity inside one session mutation gate. The UI then installs the parsed facts, `deep` mode/capabilities, snapshot-key persistence identity, and only selection IDs compatible with those facts in one synchronous commit path. No intermediate “clear artifact A” step remains, so graph B cannot be paired with artifact A as the settled workspace.

#### Task 4: Derive honest analysis facts from the cached graph

**Files:**
- Modify: `core/src/analysis/engine.rs`

**Interfaces:**
- Produces: `analyze_snapshot_from_graph_controlled(request, graph, dominator, observer) -> CoreResult<AnalyzeResponse>`.
- Preserves: `analyze_heap_from_graph` and all parse-based analysis behavior.
- Requires: a graph-derived `HeapSummary` with `header: None`, `total_records: 0`, empty `record_stats`, basename-only `heap_path`, and `ProvenanceKind::Partial`.

- [ ] **Step 1: Write a failing source-independent snapshot analysis test**

```rust
#[tokio::test]
async fn snapshot_analysis_survives_missing_source_heap() {
    let (graph, dominator) = snapshot_graph_fixture();
    let request = AnalyzeRequest {
        heap_path: "fixture.hprof".into(),
        enable_ai: false,
        enable_classloaders: true,
        enable_top_instances: true,
        ..test_request("fixture.hprof")
    };

    let response = analyze_snapshot_from_graph_controlled(
        request,
        &graph,
        &dominator,
        &NoopOperationObserver,
    )
    .await
    .expect("cached graph must hydrate without reopening the HPROF");

    assert_eq!(response.mode, AnalysisMode::Deep);
    assert_eq!(response.summary.total_objects, graph.object_count() as u64);
    assert!(response.histogram.is_some());
    assert!(response.provenance.iter().any(|marker| marker.kind == ProvenanceKind::Partial));
}
```

- [ ] **Step 2: Run the focused core test and verify RED**

Run: `cargo test -p mnemosyne-core snapshot_analysis_survives_missing_source_heap -- --nocapture`

Expected: FAIL because `analyze_snapshot_from_graph_controlled` does not exist.

- [ ] **Step 3: Implement graph-derived facts with cancellation checkpoints**

```rust
pub async fn analyze_snapshot_from_graph_controlled(
    request: AnalyzeRequest,
    obj_graph: &ObjectGraph,
    dom: &DominatorTree,
    observer: &dyn OperationObserver,
) -> CoreResult<AnalyzeResponse> {
    let summary = derive_snapshot_summary(&request.heap_path, obj_graph);
    ensure_not_cancelled(observer)?;
    let assembled = assemble_graph_backed_analysis(obj_graph, dom, &summary, &request, observer)?;
    // Build the normal deep response and append an explicit Partial marker
    // explaining that raw record/header facts are unavailable from snapshots.
}
```

The derived class totals use graph object shallow sizes, not raw HPROF record lengths. The provenance detail states that distinction. No absolute manifest path enters the response.

- [ ] **Step 4: Run focused core tests and verify GREEN**

Run: `cargo test -p mnemosyne-core snapshot_analysis_ -- --nocapture`

Expected: all snapshot-analysis tests PASS, including cancellation before publication.

- [ ] **Step 5: Commit source-independent snapshot facts**

```bash
git add core/src/analysis/engine.rs
git commit -m "feat(core): derive snapshot workspace facts"
```

#### Task 5: Return and commit one host hydrate envelope

**Files:**
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `tauri/src/commands.rs`

**Interfaces:**
- Reuses unchanged loader entry point: `open_snapshot_for_session(store, key)`.
- Produces: serializable `SnapshotWorkspaceHydrate` with snapshot identity, `deep` mode, capabilities, and sanitized analysis facts.
- Produces: `open_snapshot(...) -> Result<OperationEnvelope<SnapshotWorkspaceHydrate>, String>`.
- Preserves: SHA-256 key validation and `SnapshotStore` lookup semantics.

- [ ] **Step 1: Write failing hydrate-contract tests beside `open_snapshot_for_session`**

```rust
#[test]
fn snapshot_hydrate_contains_identity_mode_capabilities_and_facts() {
    let hydrate = build_snapshot_workspace_hydrate(
        saved_manifest,
        "source-opaque",
        analysis_response,
    )
    .expect("hydrate must serialize");

    assert_eq!(hydrate.snapshot.key, saved_key);
    assert_eq!(hydrate.mode, AnalysisMode::Deep);
    assert!(hydrate.capabilities.graph);
    assert!(hydrate.capabilities.dominators);
    assert_eq!(hydrate.analysis["summary"]["heap_path"], "fixture.hprof");
}
```

- [ ] **Step 2: Run the focused session test and verify RED**

Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures snapshot_hydrate_ -- --nocapture`

Expected: FAIL because the hydrate contract/builder does not exist.

- [ ] **Step 3: Build facts before the host commit gate**

```rust
let (manifest, graph, dominator) = open_snapshot_for_session(&store, &key)?;
let response = analyze_snapshot_from_graph_controlled(
    snapshot_analysis_request(&display_name, &config),
    &graph,
    &dominator,
    observer,
).await?;
let hydrate = build_snapshot_workspace_hydrate(&manifest, source_id, response)?;
```

Use incident-safe defaults: classloaders and top instances enabled; field-heavy analyzers disabled. `fieldData` reports the cached manifest capability and does not trigger a second parse.

- [ ] **Step 4: Atomically install the host session only after facts succeed**

Acquire the session mutation guard and all required state locks before mutation. Run the graph/dominator/path/source replacement inside `OperationRegistration::commit_if_current`; if the gate rejects, return `operation_cancelled` without changing any slot. Do not clear the existing session on a failed load, analysis error, or rejected old operation.

- [ ] **Step 5: Run focused host tests and verify GREEN**

Run:

```bash
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures \
  open_snapshot_for_session -- --nocapture
cargo check --manifest-path tauri/Cargo.toml
```

Expected: snapshot loader/hydrate tests PASS and the Tauri command compiles with the widened response.

- [ ] **Step 6: Commit the host hydrate envelope**

```bash
git add tauri/session-ops/src/lib.rs tauri/src/commands.rs
git commit -m "feat(desktop): hydrate snapshots transactionally"
```

#### Task 6: Parse and apply snapshot hydrate transactionally in the UI

**Files:**
- Modify: `ui/src/features/workflow-landing/workflow-bridge-client.ts`
- Modify: `ui/src/features/workflow-landing/workflow-bridge-client.test.ts`
- Modify: `ui/src/features/investigation/investigation-store.ts`
- Modify: `ui/src/features/investigation/investigation-store.test.ts`
- Modify: `ui/src/features/investigation/workspace-actions.ts`
- Modify: `ui/src/features/investigation/workspace-actions.test.ts`

**Interfaces:**
- Changes: `runOpenSnapshot(key) -> WorkflowBridgeResult<SnapshotWorkspaceHydrate>`.
- Produces: investigation fields `analysisMode` and `capabilities`.
- Produces: `applyOpenedSnapshotHydrate(hydrate)`, replacing graph-only `applyOpenedSnapshotSession`.
- Consumes: M26 `OperationEnvelope` validation in `invokeOperation`; no UI-side loader.

- [ ] **Step 1: Write failing bridge parser tests for the complete hydrate**

```ts
it("parses one snapshot workspace hydrate", async () => {
  window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
    openSnapshot: async () => rawSnapshotHydrate,
  };

  expect(await runOpenSnapshot(SNAPSHOT_KEY)).toEqual({
    status: "ready",
    data: expectedSnapshotHydrate,
  });
});

it("rejects a hydrate missing facts before workspace mutation", async () => {
  window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
    openSnapshot: async () => ({ ...rawSnapshotHydrate, analysis: undefined }),
  };
  expect((await runOpenSnapshot(SNAPSHOT_KEY)).status).toBe("error");
});
```

- [ ] **Step 2: Run the bridge test and verify RED**

Run: `cd ui && bun test src/features/workflow-landing/workflow-bridge-client.test.ts --max-concurrency=1`

Expected: FAIL because `runOpenSnapshot` still accepts only the graph summary.

- [ ] **Step 3: Parse identity, mode/capabilities, and facts as one value**

```ts
function parseSnapshotWorkspaceHydrate(value: unknown): SnapshotWorkspaceHydrate {
  // Validate the complete envelope first, sanitize displayName, require the
  // opaque snapshot key/sourceId, and parse analysis through parseAnalysisArtifact.
}
```

Do not place raw graph payloads or paths in browser persistence.

- [ ] **Step 4: Write failing atomic apply and compatibility tests**

```ts
it("commits snapshot facts, mode, capabilities, and compatible selection together", () => {
  seedSnapshotMetadata(SNAPSHOT_KEY, {
    objectId: "object-current",
    leakId: "leak-stale",
  });
  applyOpenedSnapshotHydrate(snapshotHydrate);

  expect(useArtifactStore.getState().artifact).toEqual(snapshotHydrate.analysis);
  expect(useInvestigationStore.getState()).toMatchObject({
    analysisMode: "deep",
    capabilities: snapshotHydrate.capabilities,
    persistenceIdentity: { kind: "snapshot", key: SNAPSHOT_KEY },
    objectId: "object-current",
    leakId: undefined,
  });
});
```

- [ ] **Step 5: Add the single snapshot commit action**

The action bumps revision once, installs the parsed artifact, resets heap-bound adapter stores, restores metadata against compatibility sets from the new facts, remembers the opaque host source, and adds the recent entry. The old graph-only clear path is removed.

- [ ] **Step 6: Add failure and old-revision race tests**

Use deferred promises around the real bridge client, `beginOperation("snapshot")`, and the M26 envelope checks:

```ts
it("keeps the prior workspace when snapshot open fails", async () => {
  seedPriorWorkspace();
  rejectSnapshotOpen("snapshot_corrupt");
  await attemptOpen();
  expectWorkspaceToEqual(priorWorkspace);
});

it("keeps the prior workspace when an old revision returns late", async () => {
  seedPriorWorkspace();
  const pending = beginSnapshotOpen();
  useInvestigationStore.getState().bumpRevisionOnArtifactChange();
  resolveWithOldOperationEnvelope(pending);
  await expect(pending.result).rejects.toThrow(/no longer active|identity/i);
  expectWorkspaceToEqual(workspaceAfterRevisionBump);
});
```

- [ ] **Step 7: Run focused store/action/bridge tests and verify GREEN**

Run:

```bash
cd ui
bun test src/features/workflow-landing/workflow-bridge-client.test.ts \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  --max-concurrency=1
```

Expected: all focused tests PASS without mounting a route.

- [ ] **Step 8: Commit the UI hydrate transaction**

```bash
git add ui/src/features/workflow-landing/workflow-bridge-client.ts \
  ui/src/features/workflow-landing/workflow-bridge-client.test.ts \
  ui/src/features/investigation/investigation-store.ts \
  ui/src/features/investigation/investigation-store.test.ts \
  ui/src/features/investigation/workspace-actions.ts \
  ui/src/features/investigation/workspace-actions.test.ts
git commit -m "feat(ui): commit snapshot hydrate atomically"
```

#### Task 7: Route both snapshot entry points through the hydrate action

**Files:**
- Modify: `ui/src/features/snapshots/SnapshotManagerPage.tsx`
- Modify: `ui/src/features/snapshots/SnapshotManagerPage.test.tsx`
- Modify: `ui/src/features/workflow-landing/RecentHeapsList.tsx`
- Modify: `ui/src/features/workflow-landing/RecentHeapsList.test.tsx`

**Interfaces:**
- Consumes: `runOpenSnapshot` and `applyOpenedSnapshotHydrate`.
- Removes: graph-only success copy and `applyOpenedSnapshotSession`.
- Preserves: browser/unavailable/list/remove/save behavior.

- [ ] **Step 1: Update focused component tests to require hydrated facts**

Each `openSnapshot` fixture returns the complete hydrate. Assertions require the new artifact and restored snapshot identity to be present after success. Add one rejected-open assertion proving the prior artifact remains unchanged.

- [ ] **Step 2: Run component tests and verify RED**

Run:

```bash
cd ui
bun test src/features/snapshots/SnapshotManagerPage.test.tsx \
  src/features/workflow-landing/RecentHeapsList.test.tsx \
  --max-concurrency=1
```

Expected: FAIL while the components still call the graph-only action.

- [ ] **Step 3: Apply the complete hydrate only on `ready`**

```ts
applyOpenedSnapshotHydrate(result.data);
setActionStatus(
  `Opened ${result.data.snapshot.displayName} ` +
  `(${result.data.analysis.summary.totalObjects.toLocaleString()} objects).`,
);
```

- [ ] **Step 4: Run the complete M27.B focused matrix**

Run:

```bash
cd ui
bun test src/features/workflow-landing/workflow-bridge-client.test.ts \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  src/features/snapshots/SnapshotManagerPage.test.tsx \
  src/features/workflow-landing/RecentHeapsList.test.tsx \
  --max-concurrency=1
bun run lint
bun run build
```

Expected: focused tests PASS, lint is clean, and the TypeScript production build succeeds. No full production route is mounted.

- [ ] **Step 5: Commit snapshot entry-point wiring**

```bash
git add ui/src/features/snapshots/SnapshotManagerPage.tsx \
  ui/src/features/snapshots/SnapshotManagerPage.test.tsx \
  ui/src/features/workflow-landing/RecentHeapsList.tsx \
  ui/src/features/workflow-landing/RecentHeapsList.test.tsx
git commit -m "feat(ui): reopen snapshots with full facts"
```

- [ ] **Step 6: Pre-push scope check**

Run: `git diff --check && git status --short && git log --oneline -8`

Expected: only M27.B plan/core/snapshot host/UI files are committed; M27.C comparison files and untracked `.claude/skills/gitnexus-*` remain untouched.

### M27.C — Integrated compare

#### Frozen integration contract

The comparison surface is one shared workbench capability, not a second analyzer or a route-owned workflow. `ComparisonWorkbenchPanel` renders beside the persistent investigation chrome whenever an artifact is open; the standalone `/compare` page remains a thin adapter that renders the same picker/results components without duplicating state or host calls. The picker lists cached snapshots through the existing workflow bridge, preselects the current side only when the active workspace has an exact `{ kind: "snapshot", key }` persistence identity, and never guesses from a display name or absolute path.

Live requests keep the existing `runDiffObjects` → `window.__MNEMOSYNE_COMPARISON_BRIDGE__.diffObjects` → Tauri `diff_objects` → `diff_objects_for_session` chain. They send:

```ts
type DiffObjectsInput = {
  beforeKey: string; // selected baseline snapshot key
  afterKey: string; // selected current snapshot key
  strategy: "ClassRetained" | "ClassDominator" | "FullFingerprint";
  topN: number; // integer, 1..500; default 50
  crossReferenceLeaks: boolean; // default false
};
```

No graph, heap path, field value, or diff payload enters workspace persistence. Snapshot labels are display-safe basenames from `runListSnapshots`; option values are opaque SHA-256 keys. The result continues to land in `useComparisonStore`, and `MatchQualityBadge` remains the single renderer for collision rate and false-match/false-split risk.

Only `Added` and `RetainedChanged` rows have an after-side object. Their Inspector action converts `exampleObjectId` to the shared string ID, calls `useInvestigationStore.getState().setObjectId(id, "inspector")`, and navigates to `/heap-explorer/object-inspector?objectId=<encoded>`. `Removed` rows remain non-navigable because that object is absent from the current/after heap.

#### Task 8: Add snapshot-backed live comparison controls

**Files:**
- Modify: `ui/src/features/comparison/comparison-store.ts`
- Modify: `ui/src/features/comparison/comparison-bridge-client.ts`
- Modify: `ui/src/features/comparison/comparison-bridge-client.test.ts`
- Modify: `ui/src/features/comparison/ComparisonPicker.tsx`
- Modify: `ui/src/features/comparison/ComparisonPicker.test.tsx`

**Interfaces:**
- Consumes: `runListSnapshots()` and display-safe `SnapshotManifest` from `workflow-landing/workflow-bridge-client.ts`.
- Consumes: active `persistenceIdentity` from `useInvestigationStore`; only exact snapshot identity may seed `liveAfterKey`.
- Produces: comparison-store fields `identityStrategy`, `topN`, and `crossReferenceLeaks` plus setters.
- Changes: `DiffObjectsInput` requires the selected strategy, bounded top-N, and leak-cross-reference flag.
- Preserves: diff-report JSON upload and explicit bridge-unavailable behavior.

- [ ] **Step 1: Write failing store/bridge option-forwarding tests**

```ts
it("forwards the selected comparison options unchanged", async () => {
  let received: DiffObjectsInput | undefined;
  setComparisonBridge({
    diffObjects: async (input) => {
      received = input;
      return rawEmptyObjectDiff;
    },
  });

  await runDiffObjects({
    beforeKey: "baseline-key",
    afterKey: "current-key",
    strategy: "FullFingerprint",
    topN: 25,
    crossReferenceLeaks: true,
  });

  expect(received).toEqual({
    beforeKey: "baseline-key",
    afterKey: "current-key",
    strategy: "FullFingerprint",
    topN: 25,
    crossReferenceLeaks: true,
  });
});
```

- [ ] **Step 2: Run focused bridge tests and verify RED**

Run: `cd ui && bun test src/features/comparison/comparison-bridge-client.test.ts --max-concurrency=1`

Expected: TypeScript/test failure because `DiffObjectsInput` does not expose `crossReferenceLeaks` and the required option contract is absent.

- [ ] **Step 3: Add minimal comparison option state and bridge contract**

```ts
const initialState = {
  // existing fields
  identityStrategy: "ClassDominator" as IdentityStrategy,
  topN: 50,
  crossReferenceLeaks: false,
};

export type DiffObjectsInput = {
  beforeKey: string;
  afterKey: string;
  strategy: IdentityStrategy;
  topN: number;
  crossReferenceLeaks: boolean;
};
```

The store setters clamp top-N to an integer in `1..500`. `reset()` restores all three defaults.

- [ ] **Step 4: Write failing picker tests for current/baseline snapshots and controls**

```ts
it("lists snapshot basenames and seeds current from the exact snapshot identity", async () => {
  seedSnapshotIdentity("current-key");
  installSnapshotList([
    snapshot("baseline-key", "/private/heaps/baseline.hprof"),
    snapshot("current-key", "/private/heaps/current.hprof"),
  ]);

  const view = render(<ComparisonPicker />);

  expect(await view.findByRole("option", { name: "current.hprof" })).toBeInTheDocument();
  expect(view.getByLabelText(/current snapshot/i)).toHaveValue("current-key");
  expect(view.getByLabelText(/baseline snapshot/i)).toHaveValue("");
  expect(view.queryByText("/private/heaps/current.hprof")).not.toBeInTheDocument();
});

it("submits strategy, top-N, and leak cross-reference", async () => {
  const calls: DiffObjectsInput[] = [];
  installSnapshotList([
    snapshot("baseline-key", "baseline.hprof"),
    snapshot("current-key", "current.hprof"),
  ]);
  installComparisonBridge(async (input) => {
    calls.push(input);
    return rawEmptyObjectDiff;
  });
  const user = userEvent.setup();
  const view = render(<ComparisonPicker />);

  await user.selectOptions(await view.findByLabelText(/current snapshot/i), "current-key");
  await user.selectOptions(view.getByLabelText(/baseline snapshot/i), "baseline-key");
  await user.selectOptions(view.getByLabelText(/identity strategy/i), "FullFingerprint");
  await user.clear(view.getByLabelText(/top n/i));
  await user.type(view.getByLabelText(/top n/i), "25");
  await user.click(view.getByLabelText(/cross-reference leaks/i));
  await user.click(view.getByRole("button", { name: /run live diff/i }));

  expect(calls).toEqual([{
    beforeKey: "baseline-key",
    afterKey: "current-key",
    strategy: "FullFingerprint",
    topN: 25,
    crossReferenceLeaks: true,
  }]);
});
```

- [ ] **Step 5: Run picker tests and verify RED**

Run: `cd ui && bun test src/features/comparison/ComparisonPicker.test.tsx --max-concurrency=1`

Expected: FAIL because live compare still uses opaque text fields, does not list snapshots, and does not expose the three options.

- [ ] **Step 6: Implement the display-safe snapshot controls**

Use `runListSnapshots()` once per picker mount when both list and diff bridges are available. Render explicit loading, empty, unavailable, and error states. The current and baseline `<select>` values are snapshot keys; labels use only `manifest.heapPath`, which the existing parser has already reduced to a basename. Seed the current side with:

```ts
const persistenceIdentity = useInvestigationStore.getState().persistenceIdentity;
if (persistenceIdentity?.kind === "snapshot") {
  setLiveAfterKey(persistenceIdentity.key);
}
```

Submit the live request as:

```ts
await runDiffObjects({
  beforeKey: liveBeforeKey,
  afterKey: liveAfterKey,
  strategy: identityStrategy,
  topN,
  crossReferenceLeaks,
});
```

- [ ] **Step 7: Run picker, bridge, and store tests and verify GREEN**

Run:

```bash
cd ui
bun test src/features/comparison/comparison-bridge-client.test.ts \
  src/features/comparison/ComparisonPicker.test.tsx \
  --max-concurrency=1
```

Expected: all focused controls and forwarding tests PASS; the JSON-upload and unavailable-state tests remain green.

- [ ] **Step 8: Commit live comparison controls**

```bash
git add ui/src/features/comparison/comparison-store.ts \
  ui/src/features/comparison/comparison-bridge-client.ts \
  ui/src/features/comparison/comparison-bridge-client.test.ts \
  ui/src/features/comparison/ComparisonPicker.tsx \
  ui/src/features/comparison/ComparisonPicker.test.tsx
git commit -m "feat(ui): add snapshot comparison controls"
```

#### Task 9: Share compare results with investigation chrome

**Files:**
- Create: `ui/src/features/comparison/ComparisonResults.tsx`
- Create: `ui/src/features/comparison/ComparisonWorkbenchPanel.tsx`
- Create: `ui/src/features/comparison/ComparisonWorkbenchPanel.test.tsx`
- Modify: `ui/src/features/comparison/ComparisonPage.tsx`
- Modify: `ui/src/features/comparison/ComparisonPage.test.tsx`
- Modify: `ui/src/app/router.tsx`

**Interfaces:**
- Produces: `ComparisonResults`, the single store-backed match-quality/delta renderer.
- Produces: `ComparisonWorkbenchPanel({ variant: "chrome" | "route" })`.
- Consumes: `useArtifactStore` in chrome mode so no compare control renders without an active artifact.
- Preserves: `/compare`, report JSON loading, no-differences copy, and all existing comparison-store state.

- [ ] **Step 1: Write a failing focused workbench-panel test**

```ts
it("opens current-vs-baseline compare beside an active investigation", async () => {
  seedArtifact("current.hprof");
  const view = render(
    <MemoryRouter>
      <ComparisonWorkbenchPanel variant="chrome" />
    </MemoryRouter>,
  );

  await userEvent.click(view.getByRole("button", { name: /compare current to baseline/i }));
  expect(view.getByLabelText(/comparison picker/i)).toBeInTheDocument();
});

it("does not render chrome controls without an active artifact", () => {
  const view = render(
    <MemoryRouter>
      <ComparisonWorkbenchPanel variant="chrome" />
    </MemoryRouter>,
  );
  expect(view.queryByRole("button", { name: /compare current to baseline/i })).toBeNull();
});
```

- [ ] **Step 2: Run the panel test and verify RED**

Run: `cd ui && bun test src/features/comparison/ComparisonWorkbenchPanel.test.tsx --max-concurrency=1`

Expected: FAIL because the shared workbench panel does not exist.

- [ ] **Step 3: Extract one results renderer and add the chrome panel**

```tsx
export function ComparisonResults() {
  const { diffReport, loadStatus } = useComparisonStore();
  // Render loading, MatchQualityBadge, explicit empty state, and the three
  // ObjectDeltaTable sections exactly once for both hosts.
}
```

Chrome mode renders a compact closed button first and mounts `ComparisonPicker`/`ComparisonResults` only after the user expands it. Route mode is always expanded and keeps the report-file adapter visible.

- [ ] **Step 4: Make `/compare` a thin adapter and wire chrome placement**

`ComparisonPage` keeps its heading/navigation and renders only:

```tsx
<ComparisonWorkbenchPanel variant="route" />
```

`InvestigationChromeLayout` renders `<ComparisonWorkbenchPanel variant="chrome" />` below `HeapSessionBar`/`FindingsAdvisoryPane`, except on `/compare` where the route adapter already owns the shared panel. Use `useLocation()` for that duplicate-surface guard.

- [ ] **Step 5: Run focused panel/page tests and verify GREEN**

Run:

```bash
cd ui
bun test src/features/comparison/ComparisonWorkbenchPanel.test.tsx \
  src/features/comparison/ComparisonPage.test.tsx \
  --max-concurrency=1
```

Expected: all tests PASS using minimal `MemoryRouter` trees; no test imports or mounts production `routes`.

- [ ] **Step 6: Commit workbench integration**

```bash
git add ui/src/features/comparison/ComparisonResults.tsx \
  ui/src/features/comparison/ComparisonWorkbenchPanel.tsx \
  ui/src/features/comparison/ComparisonWorkbenchPanel.test.tsx \
  ui/src/features/comparison/ComparisonPage.tsx \
  ui/src/features/comparison/ComparisonPage.test.tsx \
  ui/src/app/router.tsx
git commit -m "feat(ui): integrate compare into workbench"
```

#### Task 10: Navigate after-side deltas to Inspector

**Files:**
- Modify: `ui/src/features/comparison/ObjectDeltaTable.tsx`
- Modify: `ui/src/features/comparison/ObjectDeltaTable.test.tsx`

**Interfaces:**
- Consumes: shared `useInvestigationStore.setObjectId`.
- Produces: Inspector links for `Added` and `RetainedChanged` example objects.
- Preserves: `Removed` rows as evidence-only because no after-side object exists.

- [ ] **Step 1: Write failing after-side navigation tests**

```ts
it.each(["Added", "RetainedChanged"] as const)(
  "sets shared objectId and opens Inspector for %s rows",
  async (kind) => {
    const delta = deltaFor(kind, 4096);
    const router = createMemoryRouter(
      [
        { path: "/compare", element: <ObjectDeltaTable kind={kind} deltas={[delta]} /> },
        { path: "/heap-explorer/object-inspector", element: <div>Inspector</div> },
      ],
      { initialEntries: ["/compare"] },
    );
    const view = render(<RouterProvider router={router} />);

    await userEvent.click(view.getByRole("link", { name: /inspect after object 4096/i }));

    expect(useInvestigationStore.getState()).toMatchObject({
      objectId: "4096",
      originPane: "inspector",
    });
    expect(view.getByText("Inspector")).toBeInTheDocument();
  },
);

it("does not offer after-side navigation for removed rows", () => {
  const view = render(
    <MemoryRouter>
      <ObjectDeltaTable kind="Removed" deltas={[removedDelta]} />
    </MemoryRouter>,
  );
  expect(view.queryByRole("link", { name: /inspect after object/i })).toBeNull();
});
```

- [ ] **Step 2: Run the table test and verify RED**

Run: `cd ui && bun test src/features/comparison/ObjectDeltaTable.test.tsx --max-concurrency=1`

Expected: FAIL because delta rows render example IDs as text only.

- [ ] **Step 3: Add the after-side Inspector link**

For `kind !== "Removed"`, render:

```tsx
<Link
  to={`/heap-explorer/object-inspector?objectId=${encodeURIComponent(String(delta.exampleObjectId))}`}
  onClick={() =>
    useInvestigationStore.getState().setObjectId(String(delta.exampleObjectId), "inspector")
  }
>
  Inspect after object {delta.exampleObjectId}
</Link>
```

- [ ] **Step 4: Run the table and shared-selection tests and verify GREEN**

Run:

```bash
cd ui
bun test src/features/comparison/ObjectDeltaTable.test.tsx \
  src/features/investigation/investigation-store.test.ts \
  --max-concurrency=1
```

Expected: all tests PASS and removed rows remain non-navigable.

- [ ] **Step 5: Commit Inspector drill-down**

```bash
git add ui/src/features/comparison/ObjectDeltaTable.tsx \
  ui/src/features/comparison/ObjectDeltaTable.test.tsx
git commit -m "feat(ui): inspect after-side diff objects"
```

#### Task 11: Verify and close M27

**Files:**
- Create: `docs/evidence/m27-durable-investigations.md`
- Modify: `docs/superpowers/plans/2026-09-15-m27-durable-investigations-compare.md`
- Modify: `STATUS.md`
- Modify: `docs/product/ui-capability-matrix.md`
- Modify: `docs/roadmap.md`

**Interfaces:**
- Records: command evidence for M27.A/B/C and the exact commit range.
- Records: packaged GUI launch as `NOT PROVEN` on WSL; unit/build evidence is not launch evidence.
- Preserves: M28 as next work only; no M28 implementation begins.

- [ ] **Step 1: Run the complete focused M27.C matrix**

```bash
cd ui
bun test src/features/comparison/comparison-bridge-client.test.ts \
  src/features/comparison/ComparisonPicker.test.tsx \
  src/features/comparison/ComparisonWorkbenchPanel.test.tsx \
  src/features/comparison/ComparisonPage.test.tsx \
  src/features/comparison/ObjectDeltaTable.test.tsx \
  src/features/investigation/investigation-store.test.ts \
  --max-concurrency=1
bun run lint
bun run build
```

Expected: focused tests PASS, lint is clean, and the production TypeScript build succeeds. No test mounts the full production route tree.

- [ ] **Step 2: Re-run the M27.B host/core evidence needed for milestone closeout**

```bash
cargo test -p mnemosyne-core snapshot_analysis_ -- --nocapture
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures \
  open_snapshot_for_session -- --nocapture
```

Expected: source-independent snapshot facts and session snapshot reopen tests PASS. Do not claim a packaged GUI launch or native Tauri bundle from these commands.

- [ ] **Step 3: Record evidence and synchronize milestone status**

The evidence file names M27.A display-safe persistence tests, M27.B transactional hydrate tests, M27.C picker/options/match-quality/Inspector tests, lint/build results, and the unchanged analyzer chain. It includes this explicit row:

```md
| Packaged desktop GUI: reopen persisted workspace, compare snapshots, open Inspector | NOT PROVEN | WSL cannot provide native packaged-GUI launch evidence. |
```

Mark completed A/B/C plan checkboxes, set M27 shipped-with-caveat in `STATUS.md`, update the capability matrix comparison/durability rows, and change roadmap “next” to M28 without starting it.

- [ ] **Step 4: Run documentation and diff checks**

Run:

```bash
git diff --check
git status --short
git log --oneline -12
```

Expected: only M27.C implementation/tests and M27 closeout docs are changed or newly committed; untracked `.claude/skills/gitnexus-*` remain ignored.

- [ ] **Step 5: Commit M27 closeout**

```bash
git add docs/evidence/m27-durable-investigations.md \
  docs/superpowers/plans/2026-09-15-m27-durable-investigations-compare.md \
  STATUS.md docs/product/ui-capability-matrix.md docs/roadmap.md
git commit -m "docs(m27): record durable compare evidence"
```

- [ ] **Step 6: Push the completed milestone**

Run: `git push origin feature/mat-maturity-m25-plus`

Expected: the remote branch advances through the M27.C implementation and closeout commits without force.
