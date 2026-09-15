# M25 MAT Investigation-Loop Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the deterministic MAT loop from histogram class selection through bounded instances, lazy dominator context, object fields/references, and navigable GC-path nodes without losing workspace selection or filters.

**Architecture:** Extend the existing heap-explorer bridge and `tauri/session-ops` with bounded projections over the already-parsed `ObjectGraph` and `DominatorTree`; do not add a second analyzer. Keep stable `classKey` and `objectId` in `useInvestigationStore`, keep pane filters there, and render only bounded pages or expanded tree branches.

**Tech Stack:** React 19, TypeScript 7, Zustand 5, React Router 7, Bun tests, Tauri 2, Rust, `mnemosyne-core`.

**Specs:** [UI → MAT maturity roadmap §6 M25](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m25--mat-investigation-loop-depth), [architecture decisions](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#4-architecture-decisions-maintainability--debugability)

## Global Constraints

- Reuse `ObjectGraph`, `DominatorTree`, `resolve_live_instances_by_class`, and `inspect_object`; do not invent a parallel heap analyzer.
- Use stable `classKey` and `objectId` from `ui/src/features/investigation/investigation-store.ts`; never persist a row index as object identity.
- Preserve histogram search, grouping, sort, and bounded-page state when opening Inspector and returning.
- Keep first open lean. Field values are opt-in through `retainFieldData: true` and must disclose the possible full-heap reparse and higher memory cost before starting.
- Every host collection response is bounded and reports `total`, `returned`, and `truncated`; no full instance list, dominator tree, references list, or referrers list is mounted in the DOM.
- A missing bridge, artifact-only row, unavailable field data, fallback provenance, and truncation are explicit UI states, never synthetic success.
- Absolute heap paths and field values must not be logged. React receives opaque source identity and display-safe names only.
- WSL: run focused component/client tests and `tauri/session-ops` tests only. Never mount the full production route tree in unit tests; use `MemoryRouter` with one minimal route or direct component mounts. Prefer CI for full UI/Rust validation and native-host evidence.
- Keep changes slice-local and commit after each independently testable task.

---

## File map

| Unit | Existing path(s) | Planned responsibility |
|---|---|---|
| Shared selection + histogram view state | `ui/src/features/investigation/investigation-store.ts`, `investigation-store.test.ts` | Persist `classKey`, `objectId`, histogram search/group/sort/page across route changes |
| Histogram UI | `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`, `components/HistogramExplorerPanel.tsx`, matching tests | Sortable bounded histogram and class → instance drill-down |
| Heap explorer client | `ui/src/features/heap-explorer/heap-explorer-query-client.ts`, `.test.ts` | Parse bounded class-instance and dominator-child responses; preserve `inspectObject` |
| Instance list | Create `ui/src/features/artifact-explorer/components/ClassInstancesPanel.tsx`, `.test.tsx` | Fetch one bounded class page and select an object |
| Dominator UI | `ui/src/features/heap-explorer/HeapDominatorPage.tsx`, `components/DominatorExplorerPanel.tsx`, matching tests | Lazy expansion, retained-percent filter, bounded child pages |
| Inspector UI | `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`, `HeapObjectInspectorPage.tsx`, `components/ObjectInspectorPanel.tsx`, matching tests | Inspect by stable object ID, opt-in fields, synchronized refs/referrers |
| GC paths | `ui/src/features/leak-workspace/LeakGcPathPage.tsx`, `.test.tsx` | Make every real path node navigate through shared object selection; retain truncation banner |
| Tauri bridge | `ui/src/host/tauri-bridge.ts`, `.test.ts` | Inject `listClassInstances` and `getDominatorChildren` |
| Native command adapters | `tauri/src/commands.rs`, `tauri/src/main.rs`, `tauri/src/state.rs` | Bounded commands over the loaded graph/tree; retain one dominator tree per session |
| Headless session helpers | `tauri/session-ops/src/lib.rs` | Pure bounded projections with fixture tests |
| Existing core APIs | `core/src/graph/gc_path.rs`, `core/src/graph/dominator.rs`, `core/src/analysis/inspector.rs` | Reuse only; change only if a bounded projection cannot be expressed in `session-ops` |
| UI test batching | `ui/run-tests.ts` | Add new focused suites to a bounded batch; no route-tree mount |
| Milestone closeout | `STATUS.md`, `docs/product/ui-capability-matrix.md`, `docs/roadmap.md`, `docs/evidence/m25-mat-loop-depth.md` | Record verified behavior and explicit non-evidence |

---

### Task 25.A.1: Persist histogram controls by workspace revision

**Files:**
- Modify: `ui/src/features/investigation/investigation-store.ts`
- Test: `ui/src/features/investigation/investigation-store.test.ts`

**Interfaces:**
- Produce `HistogramSortKey = "retained" | "shallow" | "instances" | "class"`
- Produce `HistogramSortDirection = "asc" | "desc"`
- Add `histogramView: { searchText: string; groupBy: HistogramGroupByMode; sortKey: HistogramSortKey; sortDirection: HistogramSortDirection; pageOffset: number }`
- Add `setHistogramView(patch: Partial<HistogramViewState>): void`
- `bumpRevisionOnArtifactChange()` resets heap-bound selection and histogram view; route navigation within one revision does not.

- [x] **Step 1: Write failing store tests**

  Cover updating search/sort/page, preserving them through `setObjectId(..., "histogram")`, and resetting them only when `bumpRevisionOnArtifactChange()` runs.

- [x] **Step 2: Run the focused test and verify failure**

  Run: `cd ui && bun test src/features/investigation/investigation-store.test.ts --max-concurrency=1`

  Expected: FAIL because `histogramView` and `setHistogramView` do not exist.

- [x] **Step 3: Add the minimal typed store state**

  Keep the default `{ searchText: "", groupBy: "class", sortKey: "retained", sortDirection: "desc", pageOffset: 0 }`. Any search/group/sort change resets `pageOffset` to `0`.

- [x] **Step 4: Re-run the focused test**

  Expected: PASS with route-local selection changes leaving histogram controls unchanged.

- [x] **Step 5: Commit**

  ```bash
  git add ui/src/features/investigation/investigation-store.ts ui/src/features/investigation/investigation-store.test.ts
  git commit -m "feat(ui): persist histogram investigation state"
  ```

### Task 25.A.2: Add a bounded class-instance host projection

**Files:**
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/host/tauri-bridge.test.ts`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
- Test: `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts`

**Interfaces:**
- Rust helper: `list_class_instances_for_session(graph: &ObjectGraph, dominator: &DominatorTree, class_key: &str, offset: usize, limit: usize) -> Result<ClassInstancesPage, String>`
- Tauri command: `list_class_instances(class_key: String, offset: Option<usize>, limit: Option<usize>, state: State<'_, HeapSession>)`
- Bridge: `listClassInstances(classKey: string, offset?: number, limit?: number): Promise<unknown>`
- Response: `{ class_key, total, returned, offset, limit, truncated, instances: [{ object_id, class_name, shallow_size, retained_size }] }`
- Limits: default `100`, hard maximum `200`; sort by retained size descending, shallow size descending, then object ID.

- [x] **Step 1: Write failing `session-ops` fixture tests**

  Use `build_graph_fixture()` and assert class matching delegates to the existing dotted-name semantics of `resolve_live_instances_by_class`, output ordering is deterministic, `limit` is capped at 200, and `truncated` is true when `offset + returned < total`.

- [x] **Step 2: Run the focused Rust test**

  Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures list_class_instances`

  Expected: FAIL because the helper and response structs do not exist.

- [x] **Step 3: Implement the projection without new graph analysis**

  Call `resolve_live_instances_by_class(graph, class_key)`, look up shallow size from `graph.get_object(id)`, and retained size from the already-built `DominatorTree`. Never serialize the full matching ID vector.

- [x] **Step 4: Add the command and bridge/client contract**

  Register `commands::list_class_instances` in `tauri/src/main.rs`. Validate snake_case Rust payloads in `heap-explorer-query-client.ts`; malformed totals, IDs, or rows fail with the existing `Invalid heap explorer bridge payload` prefix.

- [x] **Step 5: Run focused native and client tests**

  Run:

  ```bash
  cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures list_class_instances
  cd ui && bun test src/features/heap-explorer/heap-explorer-query-client.test.ts src/host/tauri-bridge.test.ts --max-concurrency=1
  ```

  Expected: PASS; unavailable bridge returns `{ status: "unavailable" }`.

- [x] **Step 6: Commit**

  ```bash
  git add tauri/session-ops/src/lib.rs tauri/src/commands.rs tauri/src/main.rs ui/src/host/tauri-bridge.ts ui/src/host/tauri-bridge.test.ts ui/src/features/heap-explorer/heap-explorer-query-client.ts ui/src/features/heap-explorer/heap-explorer-query-client.test.ts
  git commit -m "feat(desktop): expose bounded class instances"
  ```

### Task 25.A.3: Make the histogram sortable with a bounded DOM

**Files:**
- Modify: `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.tsx`
- Test: `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Test: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`

**Interfaces:**
- Consume `histogramView` and `setHistogramView` from the investigation store.
- Render at most `100` histogram rows per page; controls move the bounded window and announce `Showing X–Y of Z`.
- Sort all filtered entries before slicing; deterministic tie-breaker is `entry.key`.

- [x] **Step 1: Write failing direct-component tests**

  Assert sort by class/count/shallow/retained in both directions, search before pagination, no more than 100 row buttons mounted for a 1,000-entry fixture, and page controls preserve `classKey`.

- [x] **Step 2: Run the two focused suites**

  Run: `cd ui && bun test src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx src/features/artifact-explorer/ArtifactExplorerPage.test.tsx --max-concurrency=1`

  Expected: FAIL because controls are local and every filtered row mounts.

- [x] **Step 3: Implement bounded sorting and paging**

  Keep superclass hierarchy honest: parent-linked hierarchy remains expandable, while flat superclass data stays flat. Pagination applies to flat lists; parent-linked data renders only the current bounded root window and expanded descendants.

- [x] **Step 4: Re-run focused suites**

  Expected: PASS with 100 or fewer flat histogram row buttons mounted.

- [x] **Step 5: Commit**

  ```bash
  git add ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx ui/src/features/artifact-explorer/components/HistogramExplorerPanel.tsx ui/src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx
  git commit -m "feat(ui): bound and sort histogram rows"
  ```

### Task 25.A.4: Drill class to instances and open Inspector

**Files:**
- Create: `ui/src/features/artifact-explorer/components/ClassInstancesPanel.tsx`
- Test: `ui/src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Test: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`
- Modify: `ui/run-tests.ts`

**Interfaces:**
- Consume current `classKey`; fetch `listClassInstances(classKey, offset, 100)` only for a class grouping and a connected host.
- On instance pick: `setObjectId(instance.objectId, "histogram")`, then navigate to `/heap-explorer/object-inspector?objectId=<encoded>`.

- [x] **Step 1: Write failing panel tests**

  Directly mount under a one-route `MemoryRouter`. Assert no request without `classKey`, a visible unavailable state without a bridge, exactly the bounded page when ready, an honest truncation/total label, and selected object navigation.

- [x] **Step 2: Run the focused tests**

  Run: `cd ui && bun test src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx src/features/artifact-explorer/ArtifactExplorerPage.test.tsx --max-concurrency=1`

  Expected: FAIL because the panel is absent.

- [x] **Step 3: Implement the class-instance panel**

  Replace the right-side aggregate-only detail with aggregate detail plus bounded instances. Do not request instances for package/classloader/superclass buckets; label that state and offer regroup-to-Class.

- [x] **Step 4: Verify filter continuity**

  Add a regression that selects an instance, unmounts/remounts `ArtifactExplorerPage`, and confirms histogram search/group/sort remain in the same investigation revision.

- [x] **Step 5: Run focused tests and type-check**

  Run:

  ```bash
  cd ui
  bun test src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx src/features/artifact-explorer/ArtifactExplorerPage.test.tsx src/features/investigation/investigation-store.test.ts --max-concurrency=1
  bun run lint
  ```

  Expected: PASS and TypeScript exits 0.

- [x] **Step 6: Commit**

  ```bash
  git add ui/src/features/artifact-explorer ui/src/features/investigation ui/run-tests.ts
  git commit -m "feat(ui): drill histogram classes into inspector"
  ```

### Task 25.B.1: Retain one dominator tree and expose bounded children

**Files:**
- Modify: `tauri/src/state.rs`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
- Test: matching Rust/client/bridge tests

**Interfaces:**
- Add `HeapSession.dominator: RwLock<Option<DominatorTree>>`; install/clear it atomically with `graph` on analyze/load/snapshot-open/unload.
- Rust helper: `dominator_children_for_session(graph, dominator, parent_object_id: Option<&str>, offset, limit, min_retained_bytes) -> DominatorChildrenPage`.
- `None` parent means `VIRTUAL_ROOT_ID`.
- Response node: `{ object_id, class_name, shallow_size, retained_size, dominated_count, has_children }`.
- Response page includes `{ total, returned, offset, limit, truncated }`; default 50, hard maximum 100.

- [x] **Step 1: Write failing lifecycle and projection tests**

  Assert virtual-root children, one expanded parent, retained-byte filtering before pagination, deterministic retained/object-ID ordering, and tree clearing on unload/replacement.

- [x] **Step 2: Run focused Rust tests**

  Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures dominator_children`

  Expected: FAIL until the projection exists.

- [x] **Step 3: Retain the dominator produced by `analyze_heap_capturing_graph`**

  Stop discarding `_dominator` in `run_desktop_analysis`. For `load_heap_internal`, build once after parse. Do not rebuild it in `query_heap`, `regroup_histogram`, or per tree expansion after the cache is installed.

- [x] **Step 4: Add and validate the host contract**

  Inject `getDominatorChildren(parentObjectId?, offset?, limit?, minRetainedBytes?)` and parse all counts/booleans strictly.

- [x] **Step 5: Run focused Rust/client tests**

  Expected: PASS; no command returns an unbounded child vector.

- [x] **Step 6: Commit**

  ```bash
  git add tauri/src tauri/session-ops/src/lib.rs ui/src/host/tauri-bridge.ts ui/src/features/heap-explorer/heap-explorer-query-client.ts ui/src/features/heap-explorer/heap-explorer-query-client.test.ts
  git commit -m "feat(desktop): expose lazy dominator children"
  ```

### Task 25.B.2: Replace the flat dominator list with lazy expansion

**Files:**
- Modify: `ui/src/features/heap-explorer/HeapDominatorPage.tsx`
- Test: `ui/src/features/heap-explorer/HeapDominatorPage.test.tsx`
- Modify: `ui/src/features/heap-explorer/components/DominatorExplorerPanel.tsx`
- Test: `ui/src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx`

**Interfaces:**
- Select nodes by `objectId`, not row index, when live tree data exists.
- Request roots once; request children only after the user expands a node.
- Retained-percent input range: `0`–`100`; convert to `minRetainedBytes = totalSizeBytes * percent / 100`.
- Artifact-only fallback remains a clearly labelled bounded flat preview.

- [x] **Step 1: Write failing lazy-tree tests**

  Assert children are absent before expansion, one request occurs after expansion, collapse removes descendant DOM, a 1% filter changes the request floor, truncated children show `Load next children`, and selecting a child updates shared `objectId`.

- [x] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx src/features/heap-explorer/HeapDominatorPage.test.tsx --max-concurrency=1`

  Expected: FAIL against the current flat `rows.map`.

- [x] **Step 3: Implement branch-local state**

  Key node state by object ID: `collapsed | loading | ready(page) | error`. A failed child request keeps the parent visible and offers retry. Never recursively preload descendants.

- [x] **Step 4: Re-run tests and type-check**

  Expected: PASS; the fixture’s unexpanded descendants are not in the DOM.

- [x] **Step 5: Commit**

  ```bash
  git add ui/src/features/heap-explorer/HeapDominatorPage.tsx ui/src/features/heap-explorer/HeapDominatorPage.test.tsx ui/src/features/heap-explorer/components/DominatorExplorerPanel.tsx ui/src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx
  git commit -m "feat(ui): add lazy dominator tree"
  ```

### Task 25.C.1: Inspect any shared object ID and request fields explicitly

**Files:**
- Modify: `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`
- Modify: `ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx`
- Modify: `ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx`
- Test: matching layout/page/panel tests

**Interfaces:**
- Replace `selectedRowIndex` as the inspector’s primary input with `objectId?: string`; artifact row data is optional decoration.
- Initial `inspectObject(objectId, false)` remains lean.
- Explicit CTA calls `inspectObject(objectId, true)` after disclosing: “May reparse the heap and retain field bytes; memory use can increase.”
- `fields === undefined` after an opt-in response means field bytes were unavailable; `fields.length === 0` means no decoded fields.

- [ ] **Step 1: Write failing inspector tests**

  Assert unmatched URL/shared IDs still inspect, no field-data request runs automatically, the disclosure appears before opt-in, clicking the CTA sends `true`, and primitive/object-reference fields render with type/name/value.

- [ ] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx src/features/heap-explorer/HeapObjectInspectorPage.test.tsx src/features/heap-explorer/HeapExplorerLayout.test.tsx --max-concurrency=1`

  Expected: FAIL because the panel derives identity only from a dominator row and never renders `fields`.

- [ ] **Step 3: Implement stable-ID inspection and field states**

  Keep field values out of logs and error strings. Render long values with wrapping; if UI display is capped, add a per-value `truncated` label and show the exact cap rather than silently slicing.

- [ ] **Step 4: Re-run tests**

  Expected: PASS; field data stays opt-in and an object absent from the artifact shortlist is inspectable.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/features/heap-explorer
  git commit -m "feat(ui): render opt-in object fields"
  ```

### Task 25.C.2: Synchronize references, referrers, dominators, and GC-path nodes

**Files:**
- Modify: `ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx`
- Test: `ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx`
- Modify: `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
- Test: `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`
- Modify: `ui/src/features/leak-workspace/live-detail-client.ts`
- Test: `ui/src/features/leak-workspace/live-detail-client.test.ts`

**Interfaces:**
- Every real object link first calls `setObjectId(id, originPane)` and then navigates to Inspector.
- Bound each rendered relation section to 100 rows and disclose `Showing first N of M returned`; do not imply backend completeness when the response was truncated or fallback.
- Preserve `GcPathResult.truncated` and provenance exactly.

- [ ] **Step 1: Write failing cross-navigation tests**

  Cover outgoing reference, incoming referrer, dominator parent, dominator child, shortest-path node, and all-path node. Assert each updates the shared store and produces an encoded Inspector URL.

- [ ] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx src/features/leak-workspace/LeakGcPathPage.test.tsx src/features/leak-workspace/live-detail-client.test.ts --max-concurrency=1`

  Expected: FAIL because current links update only the URL and GC-path nodes are non-interactive articles.

- [ ] **Step 3: Implement synchronized navigation**

  Root/synthetic nodes without a real object ID remain non-clickable and labelled. Real nodes use `originPane: "gc-path"`; refs/referrers/dominator chips use `"inspector"`.

- [ ] **Step 4: Add honest truncation regressions**

  Assert backend `truncated: true` remains visible, bounded relation rendering states the cap, and fallback provenance never uses a green/complete label.

- [ ] **Step 5: Re-run focused tests and type-check**

  Expected: PASS and `bun run lint` exits 0.

- [ ] **Step 6: Commit**

  ```bash
  git add ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx ui/src/features/leak-workspace
  git commit -m "feat(ui): synchronize object path navigation"
  ```

### Task 25.C.3: Focused milestone verification and documentation

**Files:**
- Modify: `ui/run-tests.ts`
- Modify: `STATUS.md`
- Modify: `docs/product/ui-capability-matrix.md`
- Modify: `docs/roadmap.md`
- Create: `docs/evidence/m25-mat-loop-depth.md`

- [ ] **Step 1: Add new suites to a fresh or light test batch**

  Keep `ClassInstancesPanel.test.tsx` and lazy-tree tests out of any batch already known to approach the Bun/jsdom RSS ceiling.

- [ ] **Step 2: Run supported local gates**

  Run:

  ```bash
  cd ui
  bun run lint
  bun test src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx --max-concurrency=1
  bun test src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx --max-concurrency=1
  bun test src/features/leak-workspace/LeakGcPathPage.test.tsx --max-concurrency=1
  cd ..
  cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures
  cargo check --manifest-path tauri/Cargo.toml
  ```

  Expected: all focused commands exit 0. Do not run or claim packaged GUI smoke from WSL.

- [ ] **Step 3: Record evidence honestly**

  Document bounded row limits, lazy request behavior, field-data cost disclosure, fallback/truncation states, commands run, and a `NOT PROVEN` section for packaged/native UI behavior.

- [ ] **Step 4: Update status/capability docs**

  Mark only evidenced slices shipped. Keep native-host and MAT-golden claims gated.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/run-tests.ts STATUS.md docs/product/ui-capability-matrix.md docs/roadmap.md docs/evidence/m25-mat-loop-depth.md
  git commit -m "docs(m25): record MAT loop depth evidence"
  ```

## M25 completion gate

- Histogram rows are sortable and DOM-bounded; class filters survive instance → Inspector → back.
- Class drill-down returns a bounded, deterministic page and selecting an instance sets shared `objectId`.
- Dominator children load only on expansion, retained-percent filtering is test-covered, and no full tree is mounted.
- Fields are requested only after explicit cost disclosure and rendered with honest unavailable/truncated states.
- References, referrers, dominator nodes, and GC-path nodes synchronize shared object identity.
- Focused WSL-safe tests pass; CI owns full-suite validation; packaged native behavior remains unclaimed until evidenced.
