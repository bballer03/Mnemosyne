# M4 Competitive Explorer Surfaces 5A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first half of Slice 5 by shipping a new browser-first `Heap Explorer` route family with artifact-backed dominator exploration and a selected-object inspector.

**Architecture:** Build `5A` on top of the existing artifact-loader/dashboard/leak-workspace/artifact-explorer stack. Widen the serialized analysis contract just enough to carry real dominator rows from Rust into the browser, then add a dedicated `heap-explorer` feature area with a three-pane shell: left mode rail, center dominator explorer, right object inspector. Keep this batch artifact-backed only; no query execution transport lands yet.

**Tech Stack:** Rust serde types, React, TypeScript, React Router 6, Zustand, Bun, Testing Library, Stitch-driven screen lineage

---

## File Structure

### Existing files to modify

- `core/src/graph/metrics.rs`
  - extend serialized dominator rows so the browser can navigate real dominator data instead of graph-count summaries only
- `core/src/graph/metrics.rs` tests
  - verify the richer dominator payload serializes with the expected fields
- `ui/src/lib/analysis-types.ts`
  - parse the richer dominator rows into browser-friendly camelCase fields
- `ui/src/lib/analysis-types.test.ts`
  - verify dominator rows parse correctly and remain optional when absent
- `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`
  - verify serialized JSON round-trips dominator explorer data into the frontend shape
- `ui/src/app/router.tsx`
  - register the new heap explorer route family
- `ui/src/features/dashboard/DashboardPage.tsx`
  - add navigation into the heap explorer route
- `ui/src/features/dashboard/DashboardPage.test.tsx`
  - verify dashboard navigation into the heap explorer

### New files to create

- `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`
  - route shell, artifact guard, top navigation, selected dominator row state, recent targets rail
- `ui/src/features/heap-explorer/HeapExplorerLayout.test.tsx`
  - route-level behavior, missing-artifact redirect, default selection behavior, mode navigation
- `ui/src/features/heap-explorer/HeapDominatorPage.tsx`
  - center-pane dominator explorer view driven by artifact dominator rows
- `ui/src/features/heap-explorer/HeapDominatorPage.test.tsx`
  - search/filter/selection/empty-state coverage
- `ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx`
  - selected-object inspector using the currently selected dominator row only
- `ui/src/features/heap-explorer/HeapObjectInspectorPage.test.tsx`
  - selected/unselected inspector states and honest absent messaging
- `ui/src/features/heap-explorer/components/ModeRail.tsx`
  - left mode switcher with Dominators/Object Inspector/Query Console placeholders and recent target recall
- `ui/src/features/heap-explorer/components/ModeRail.test.tsx`
  - mode switcher rendering and placeholder state coverage
- `ui/src/features/heap-explorer/components/DominatorExplorerPanel.tsx`
  - searchable dominator table/list with retained vs dominates emphasis and row selection
- `ui/src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx`
  - focused panel behavior coverage
- `ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx`
  - object/class/retained/dominator detail panel derived from selected dominator row
- `ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx`
  - panel rendering and unselected-state coverage

### Files intentionally left unchanged in 5A

- `ui/src/features/leak-workspace/*`
- any query execution transport or browser bridge code
- any new Rust query or MCP method work beyond the dominator serialization gap

---

### Task 1: Extend The Serialized Analysis Contract For Browser Dominator Exploration

**Files:**
- Modify: `core/src/graph/metrics.rs`
- Test: `core/src/graph/metrics.rs`
- Modify: `ui/src/lib/analysis-types.ts`
- Modify: `ui/src/lib/analysis-types.test.ts`
- Modify: `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`

- [ ] **Step 1: Write the failing Rust test for richer dominator rows**

Add a focused test in `core/src/graph/metrics.rs` proving `build_graph_metrics_from_dominator()` exposes enough fields for browser selection:

```rust
#[test]
fn build_graph_metrics_from_dominator_exposes_browser_navigation_fields() {
    let graph = sample_graph();
    let dom = build_dominator_tree(&graph);
    let metrics = build_graph_metrics_from_dominator(&dom, &graph);

    let first = metrics.dominators.first().expect("expected dominator rows");
    assert!(!first.class_name.is_empty());
    assert!(!first.object_id.is_empty());
    assert!(first.retained_size > 0);
    assert!(first.dominates >= 0);
}
```

- [ ] **Step 2: Run the Rust test to verify it fails**

Run:

`cargo test graph::metrics::tests::build_graph_metrics_from_dominator_exposes_browser_navigation_fields`

Expected: FAIL because `DominatorNode` currently only exposes `name`, `dominates`, `immediate_dominator`, and `retained_size`.

- [ ] **Step 3: Implement the minimal Rust dominator payload expansion**

Update `core/src/graph/metrics.rs` so `DominatorNode` becomes:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DominatorNode {
    pub name: String,
    pub class_name: String,
    pub object_id: String,
    pub dominates: usize,
    pub immediate_dominator: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retained_size: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shallow_size: u64,
}
```

When constructing rows in `build_graph_metrics_from_dominator()`, populate:

```rust
dominators.push(DominatorNode {
    name: label.clone(),
    class_name: graph.class_name(obj_id),
    object_id: format!("0x{obj_id:x}"),
    dominates: dominated_count,
    immediate_dominator: immediate,
    retained_size: dom.retained_size(obj_id),
    shallow_size: u64::from(graph.objects.get(&obj_id).map(|obj| obj.shallow_size).unwrap_or(0)),
});
```

Keep the existing `name` field for report compatibility rather than renaming it in this batch.

- [ ] **Step 4: Run the Rust test to verify it passes**

Run:

`cargo test graph::metrics::tests::build_graph_metrics_from_dominator_exposes_browser_navigation_fields`

Expected: PASS.

- [ ] **Step 5: Write the failing frontend parser tests**

In `ui/src/lib/analysis-types.test.ts`, add:

```ts
it("parses serialized dominator explorer rows", () => {
  const parsed = parseAnalysisArtifact({
    summary: { heap_path: "heap.hprof", total_objects: 42, total_size_bytes: 2048, total_records: 2 },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 1, nanos: 0 },
    graph: {
      node_count: 200,
      edge_count: 400,
      dominators: [
        {
          name: "com.example.Cache",
          class_name: "com.example.Cache",
          object_id: "0x2a",
          dominates: 7,
          immediate_dominator: "<heap-root>",
          retained_size: 1024,
          shallow_size: 64,
        },
      ],
    },
    provenance: [],
  });

  expect(parsed.graph.dominators[0]).toEqual({
    name: "com.example.Cache",
    className: "com.example.Cache",
    objectId: "0x2a",
    dominates: 7,
    immediateDominator: "<heap-root>",
    retainedSize: 1024,
    shallowSize: 64,
  });
});
```

In `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`, add one JSON-text round-trip assertion for the same fields.

- [ ] **Step 6: Run the frontend parser tests to verify they fail**

Run:

`npx --yes bun test "src/lib/analysis-types.test.ts" "src/features/artifact-loader/load-analysis-artifact.test.ts"`

Expected: FAIL because the current frontend `graph` shape only preserves `nodeCount`, `edgeCount`, and `dominatorCount`.

- [ ] **Step 7: Implement the minimal frontend parsing changes**

Extend `AnalysisArtifact["graph"]` in `ui/src/lib/analysis-types.ts` to:

```ts
graph: {
  nodeCount: number;
  edgeCount: number;
  dominatorCount: number;
  dominators: Array<{
    name: string;
    className: string;
    objectId: string;
    dominates: number;
    immediateDominator?: string;
    retainedSize: number;
    shallowSize: number;
  }>;
};
```

Parse the serialized rows inside `parseAnalysisArtifact()` with camelCase conversion.

- [ ] **Step 8: Run the focused parser tests to verify they pass**

Run:

`npx --yes bun test "src/lib/analysis-types.test.ts" "src/features/artifact-loader/load-analysis-artifact.test.ts"`

Expected: PASS.

- [ ] **Step 9: Commit the contract slice**

```bash
git add core/src/graph/metrics.rs ui/src/lib/analysis-types.ts ui/src/lib/analysis-types.test.ts ui/src/features/artifact-loader/load-analysis-artifact.test.ts
git commit -m "feat(ui): expose dominator rows to the browser"
```

### Task 2: Add The Heap Explorer Route Shell And Mode Rail

**Files:**
- Modify: `ui/src/app/router.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.test.tsx`
- Create: `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`
- Create: `ui/src/features/heap-explorer/HeapExplorerLayout.test.tsx`
- Create: `ui/src/features/heap-explorer/components/ModeRail.tsx`
- Create: `ui/src/features/heap-explorer/components/ModeRail.test.tsx`

- [ ] **Step 1: Write the failing route tests**

Add one dashboard navigation test in `ui/src/features/dashboard/DashboardPage.test.tsx`:

```ts
it("opens the heap explorer from the dashboard", async () => {
  const user = userEvent.setup();
  seedArtifact();
  const router = createMemoryRouter(routes, { initialEntries: ["/dashboard"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("link", { name: /heap explorer/i }));

  expect(router.state.location.pathname).toBe("/heap-explorer/dominators");
});
```

Create `ui/src/features/heap-explorer/HeapExplorerLayout.test.tsx` with:

```ts
it("redirects to the loader when no artifact is loaded", () => {
  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
  expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
});

it("defaults the first dominator row into the object inspector context", () => {
  seedArtifactWithDominators();
  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
  expect(view.getByText(/selected target/i)).toBeInTheDocument();
  expect(view.getByText(/0x2a/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the route tests to verify they fail**

Run:

`npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx" "src/features/heap-explorer/HeapExplorerLayout.test.tsx"`

Expected: FAIL because no heap explorer route or layout exists yet.

- [ ] **Step 3: Implement the minimal route shell and mode rail**

Add this route family in `ui/src/app/router.tsx`:

```tsx
{
  path: "/heap-explorer",
  element: <HeapExplorerLayout />,
  children: [
    { index: true, element: <Navigate to="dominators" replace /> },
    { path: "dominators", element: <HeapDominatorPage /> },
    { path: "object-inspector", element: <HeapObjectInspectorPage /> },
    { path: "query-console", element: <div>Query Console coming in Slice 5B.</div> },
  ],
}
```

In `ui/src/features/dashboard/DashboardPage.tsx`, add:

```tsx
<Link to="/heap-explorer/dominators">Heap Explorer</Link>
```

Create `ui/src/features/heap-explorer/components/ModeRail.tsx` with a small nav list:

```tsx
export function ModeRail({ selectedObjectId }: { selectedObjectId?: string }) {
  return (
    <div style={{ display: "grid", gap: "0.85rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Heap Explorer</h2>
      <Link to="/heap-explorer/dominators">Dominators</Link>
      <Link to="/heap-explorer/object-inspector">Object Inspector</Link>
      <Link to="/heap-explorer/query-console">Query Console</Link>
      <div style={{ color: "#94a3b8" }}>Selected target: {selectedObjectId ?? "None"}</div>
      <div style={{ color: "#94a3b8" }}>Recent targets appear here after selection.</div>
    </div>
  );
}
```

Create `HeapExplorerLayout.tsx` to:

```tsx
export function HeapExplorerLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const location = useLocation();
  const [selectedObjectId, setSelectedObjectId] = useState(artifact?.graph.dominators[0]?.objectId);

  useEffect(() => {
    setSelectedObjectId(artifact?.graph.dominators[0]?.objectId);
  }, [artifact]);

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <section>{/* top header with Dashboard / Artifact Explorer / Heap Explorer */}</section>
      <section style={{ display: "grid", gridTemplateColumns: "280px minmax(0, 1fr) 320px", gap: "1rem" }}>
        <aside><ModeRail selectedObjectId={selectedObjectId} /></aside>
        <section><Outlet context={{ selectedObjectId, setSelectedObjectId, artifact, location, artifactName }} /></section>
        <aside><ObjectInspectorPanel artifact={artifact} selectedObjectId={selectedObjectId} /></aside>
      </section>
    </main>
  );
}
```

- [ ] **Step 4: Run the route tests to verify they pass**

Run:

`npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx" "src/features/heap-explorer/HeapExplorerLayout.test.tsx"`

Expected: PASS.

- [ ] **Step 5: Commit the route shell slice**

```bash
git add ui/src/app/router.tsx ui/src/features/dashboard/DashboardPage.tsx ui/src/features/dashboard/DashboardPage.test.tsx ui/src/features/heap-explorer/HeapExplorerLayout.tsx ui/src/features/heap-explorer/HeapExplorerLayout.test.tsx ui/src/features/heap-explorer/components/ModeRail.tsx ui/src/features/heap-explorer/components/ModeRail.test.tsx
git commit -m "feat(ui): add heap explorer route shell"
```

### Task 3: Build The Dominator Explorer Center Surface

**Files:**
- Create: `ui/src/features/heap-explorer/HeapDominatorPage.tsx`
- Create: `ui/src/features/heap-explorer/HeapDominatorPage.test.tsx`
- Create: `ui/src/features/heap-explorer/components/DominatorExplorerPanel.tsx`
- Create: `ui/src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx`

- [ ] **Step 1: Write the failing dominator explorer tests**

Add `ui/src/features/heap-explorer/HeapDominatorPage.test.tsx`:

```ts
it("renders searchable dominator rows", () => {
  seedArtifactWithDominators();
  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByRole("heading", { name: /dominator explorer/i })).toBeInTheDocument();
  expect(view.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
  expect(view.getByText(/retained vs dominates/i)).toBeInTheDocument();
});

it("filters dominator rows by search text", async () => {
  const user = userEvent.setup();
  seedArtifactWithDominators();
  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.type(view.getByLabelText(/search dominators/i), "ConcurrentHashMap");

  expect(view.getByText(/ConcurrentHashMap/i)).toBeInTheDocument();
  expect(view.queryByText(/com\.example\.Cache/i)).toBeNull();
});
```

- [ ] **Step 2: Run the dominator explorer tests to verify they fail**

Run:

`npx --yes bun test "src/features/heap-explorer/HeapDominatorPage.test.tsx"`

Expected: FAIL because the page and panel do not exist yet.

- [ ] **Step 3: Implement the minimal dominator explorer UI**

Create `DominatorExplorerPanel.tsx` with:

```tsx
export function DominatorExplorerPanel({
  dominators,
  selectedObjectId,
  onSelectObjectId,
}: {
  dominators: AnalysisArtifact["graph"]["dominators"];
  selectedObjectId?: string;
  onSelectObjectId: (value: string) => void;
}) {
  const [searchText, setSearchText] = useState("");
  const filtered = dominators.filter((row) => {
    const needle = searchText.trim().toLowerCase();
    return !needle || row.className.toLowerCase().includes(needle) || row.objectId.toLowerCase().includes(needle);
  });

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Dominator Explorer</h2>
      <label>
        <span>Search dominators</span>
        <input aria-label="Search dominators" value={searchText} onChange={(event) => setSearchText(event.target.value)} />
      </label>
      <div style={{ color: "#94a3b8" }}>Retained vs dominates</div>
      {filtered.map((row) => (
        <button
          key={row.objectId}
          type="button"
          aria-pressed={row.objectId === selectedObjectId}
          aria-label={`Select ${row.className}`}
          onClick={() => onSelectObjectId(row.objectId)}
        >
          <strong>{row.className}</strong>
          <span>{row.objectId}</span>
          <span>{row.retainedSize} retained</span>
          <span>{row.dominates} dominated</span>
        </button>
      ))}
    </div>
  );
}
```

Create `HeapDominatorPage.tsx` to read the outlet context and render the panel.

- [ ] **Step 4: Run the dominator explorer tests to verify they pass**

Run:

`npx --yes bun test "src/features/heap-explorer/HeapDominatorPage.test.tsx"`

Expected: PASS.

- [ ] **Step 5: Commit the dominator explorer slice**

```bash
git add ui/src/features/heap-explorer/HeapDominatorPage.tsx ui/src/features/heap-explorer/HeapDominatorPage.test.tsx ui/src/features/heap-explorer/components/DominatorExplorerPanel.tsx ui/src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx
git commit -m "feat(ui): add artifact-backed dominator explorer"
```

### Task 4: Build The Selected Object Inspector Pane

**Files:**
- Create: `ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx`
- Create: `ui/src/features/heap-explorer/HeapObjectInspectorPage.test.tsx`
- Create: `ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx`
- Create: `ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx`

- [ ] **Step 1: Write the failing object inspector tests**

Add `ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx`:

```ts
it("renders selected dominator row details", () => {
  const artifact = buildArtifactWithDominators();
  render(<ObjectInspectorPanel artifact={artifact} selectedObjectId="0x2a" />);

  expect(screen.getByText(/object inspector/i)).toBeInTheDocument();
  expect(screen.getByText(/0x2a/i)).toBeInTheDocument();
  expect(screen.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
  expect(screen.getByText(/immediate dominator/i)).toBeInTheDocument();
});

it("renders an honest unselected state when no row is selected", () => {
  const artifact = buildArtifactWithDominators();
  render(<ObjectInspectorPanel artifact={artifact} selectedObjectId={undefined} />);
  expect(screen.getByText(/select a dominator row to inspect it/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the object inspector tests to verify they fail**

Run:

`npx --yes bun test "src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx"`

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the minimal object inspector**

Create `ObjectInspectorPanel.tsx` with:

```tsx
export function ObjectInspectorPanel({
  artifact,
  selectedObjectId,
}: {
  artifact: AnalysisArtifact;
  selectedObjectId?: string;
}) {
  const selected = artifact.graph.dominators.find((row) => row.objectId === selectedObjectId);

  if (!selected) {
    return <p style={{ margin: 0, color: "#94a3b8" }}>Select a dominator row to inspect it.</p>;
  }

  return (
    <div style={{ display: "grid", gap: "0.75rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Object Inspector</h2>
      <strong>{selected.className}</strong>
      <div>Object ID: {selected.objectId}</div>
      <div>Shallow size: {selected.shallowSize}</div>
      <div>Retained size: {selected.retainedSize}</div>
      <div>Dominates: {selected.dominates}</div>
      <div>Immediate dominator: {selected.immediateDominator ?? "Unknown"}</div>
      <p style={{ margin: 0, color: "#94a3b8" }}>
        This panel is artifact-backed and does not yet resolve live references or referrers.
      </p>
    </div>
  );
}
```

Create `HeapObjectInspectorPage.tsx` to render a full-page inspector route using the same outlet context and panel.

- [ ] **Step 4: Run the object inspector tests to verify they pass**

Run:

`npx --yes bun test "src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx" "src/features/heap-explorer/HeapObjectInspectorPage.test.tsx"`

Expected: PASS.

- [ ] **Step 5: Commit the object inspector slice**

```bash
git add ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx ui/src/features/heap-explorer/HeapObjectInspectorPage.test.tsx ui/src/features/heap-explorer/components/ObjectInspectorPanel.tsx ui/src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx
git commit -m "feat(ui): add heap object inspector"
```

### Task 5: Run 5A Verification

**Files:**
- Modify only if verification finds breakage

- [ ] **Step 1: Run the focused 5A frontend route tests**

Run:

`npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx" "src/features/heap-explorer/HeapExplorerLayout.test.tsx" "src/features/heap-explorer/HeapDominatorPage.test.tsx" "src/features/heap-explorer/HeapObjectInspectorPage.test.tsx" "src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx" "src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx"`

Expected: PASS.

- [ ] **Step 2: Run the full frontend verification**

Run:

`npx --yes bun test`

Expected: PASS.

- [ ] **Step 3: Run the frontend build**

Run:

`npx --yes bun run build`

Expected: PASS.

- [ ] **Step 4: Run the frontend lint/type-check**

Run:

`npx --yes bun run lint`

Expected: PASS.

- [ ] **Step 5: Run the Rust verification**

Run:

`cargo test`

Expected: PASS.

- [ ] **Step 6: Run the Rust static checks**

Run:

`cargo clippy --workspace --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 7: Run the Rust formatting check**

Run:

`cargo fmt --all -- --check`

Expected: PASS.

- [ ] **Step 8: Commit any verification fixes**

```bash
git add core/src/graph/metrics.rs ui/src/app/router.tsx ui/src/features/dashboard/DashboardPage.tsx ui/src/features/dashboard/DashboardPage.test.tsx ui/src/features/heap-explorer ui/src/lib/analysis-types.ts ui/src/lib/analysis-types.test.ts ui/src/features/artifact-loader/load-analysis-artifact.test.ts
git commit -m "fix(ui): harden heap explorer 5A verification issues"
```
