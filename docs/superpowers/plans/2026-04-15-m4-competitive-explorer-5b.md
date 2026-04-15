# M4 Competitive Explorer Surfaces 5B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the second half of Slice 5 by shipping a heap-explorer query console, explorer cross-navigation, and a final project review pass after `5A` and `5B` are complete.

**Architecture:** Reuse the `5A` heap-explorer route shell instead of inventing another workspace. Add a new heap-explorer-local query adapter boundary for executing the already-shipped `query_heap` capability, then wire the dominator explorer, object inspector, leak workspace, and artifact explorer together through explicit route/state handoffs. Finish with a full-project review pass covering code, tests, docs, and remaining milestone gaps.

**Tech Stack:** React, TypeScript, React Router 6, Bun, Testing Library, existing `query_heap` MCP/backend capability, review tooling

---

## File Structure

### Existing files to modify

- `ui/src/app/router.tsx`
  - keep the heap explorer route family and point `query-console` to the real page
- `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`
  - share selected object/context into query console and richer cross-navigation affordances
- `ui/src/features/heap-explorer/HeapDominatorPage.tsx`
  - add explicit actions into object inspector, leak workspace, and query console
- `ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx`
  - expose query-console jump actions based on current selection
- `ui/src/features/leak-workspace/leak-workspace-store.ts`
  - only if a narrow selected-target handoff is needed for explorer-to-workspace jumps
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
  - only if route params/search state need to seed object-target handoff explicitly
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
  - verify cross-navigation handoff if touched

### New files to create

- `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
  - heap-explorer-local query adapter boundary, parallel to leak-workspace local bridge discipline
- `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts`
  - payload normalization, unavailable/error states, and result parsing
- `ui/src/features/heap-explorer/HeapQueryConsolePage.tsx`
  - query-console route view, request lifecycle, and result rendering
- `ui/src/features/heap-explorer/HeapQueryConsolePage.test.tsx`
  - query execution flow, unavailable/error states, and result-table rendering
- `ui/src/features/heap-explorer/components/QueryConsolePanel.tsx`
  - monospace query editor, run button, result table, and selection actions
- `ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx`
  - panel interaction coverage
- `ui/src/features/heap-explorer/components/ExplorerCrossNavActions.tsx`
  - small action group reused by dominator/object/query panes for route jumps
- `ui/src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx`
  - route generation and handoff coverage
- `docs/superpowers/reviews/2026-04-15-project-review.md`
  - final structured review findings after 5A and 5B land

### Files intentionally left unchanged in 5B

- Rust query parser/executor semantics unless the UI discovers a real contract mismatch
- any generalized app-wide browser RPC layer

---

### Task 1: Add A Heap-Explorer-Local Query Adapter Boundary

**Files:**
- Create: `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
- Create: `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts`

- [ ] **Step 1: Write the failing query-adapter tests**

Create `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts` with:

```ts
it("returns unavailable when no heap explorer query bridge exists", async () => {
  delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;

  await expect(runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id" })).resolves.toEqual({
    status: "unavailable",
  });
});

it("normalizes query rows from the host bridge", async () => {
  window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: async () => ({
      columns: ["object_id", "class_name"],
      rows: [["0x2a", "com.example.Cache"]],
    }),
  };

  await expect(runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id, class_name" })).resolves.toEqual({
    status: "ready",
    data: {
      columns: ["object_id", "class_name"],
      rows: [["0x2a", "com.example.Cache"]],
    },
  });
});
```

- [ ] **Step 2: Run the query-adapter tests to verify they fail**

Run:

`npx --yes bun test "src/features/heap-explorer/heap-explorer-query-client.test.ts"`

Expected: FAIL because the adapter file does not exist.

- [ ] **Step 3: Implement the minimal query adapter**

Create `heap-explorer-query-client.ts` with:

```ts
export type HeapQueryInput = {
  heapPath: string;
  query: string;
};

export type HeapQueryResult = {
  columns: string[];
  rows: Array<Array<string | number | boolean | null>>;
};

export type HeapExplorerBridge = {
  queryHeap?: (input: HeapQueryInput) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: HeapExplorerBridge;
  }
}

export async function runHeapQuery(input: HeapQueryInput) {
  const bridge = window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  if (!bridge?.queryHeap) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.queryHeap(input);
    if (!raw || typeof raw !== "object" || !Array.isArray((raw as { columns?: unknown }).columns) || !Array.isArray((raw as { rows?: unknown }).rows)) {
      return { status: "error" as const, error: "Invalid heap query response." };
    }

    return {
      status: "ready" as const,
      data: raw as HeapQueryResult,
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown heap query failure.",
    };
  }
}
```

- [ ] **Step 4: Run the query-adapter tests to verify they pass**

Run:

`npx --yes bun test "src/features/heap-explorer/heap-explorer-query-client.test.ts"`

Expected: PASS.

- [ ] **Step 5: Commit the query adapter slice**

```bash
git add ui/src/features/heap-explorer/heap-explorer-query-client.ts ui/src/features/heap-explorer/heap-explorer-query-client.test.ts
git commit -m "feat(ui): add heap explorer query adapter"
```

### Task 2: Add The Heap Query Console Route And Result Surface

**Files:**
- Modify: `ui/src/app/router.tsx`
- Create: `ui/src/features/heap-explorer/HeapQueryConsolePage.tsx`
- Create: `ui/src/features/heap-explorer/HeapQueryConsolePage.test.tsx`
- Create: `ui/src/features/heap-explorer/components/QueryConsolePanel.tsx`
- Create: `ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx`

- [ ] **Step 1: Write the failing query-console tests**

Create `ui/src/features/heap-explorer/HeapQueryConsolePage.test.tsx`:

```ts
it("shows an unavailable state when no query bridge exists", () => {
  delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  seedArtifactWithDominators();
  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/query execution is unavailable in this browser session/i)).toBeInTheDocument();
});

it("runs a query and renders result rows", async () => {
  const user = userEvent.setup();
  seedArtifactWithDominators();
  window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: async () => ({
      columns: ["object_id", "class_name"],
      rows: [["0x2a", "com.example.Cache"]],
    }),
  };

  const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("button", { name: /run query/i }));

  expect(view.getByText(/object_id/i)).toBeInTheDocument();
  expect(view.getByText(/0x2a/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the query-console tests to verify they fail**

Run:

`npx --yes bun test "src/features/heap-explorer/HeapQueryConsolePage.test.tsx"`

Expected: FAIL because the route still renders a placeholder.

- [ ] **Step 3: Implement the minimal query-console page**

Replace the placeholder route in `ui/src/app/router.tsx` with `HeapQueryConsolePage`.

Create `QueryConsolePanel.tsx` with:

```tsx
export function QueryConsolePanel({ heapPath }: { heapPath: string }) {
  const [queryText, setQueryText] = useState("SELECT object_id, class_name LIMIT 20");
  const [result, setResult] = useState<Awaited<ReturnType<typeof runHeapQuery>> | undefined>();

  return (
    <div style={{ display: "grid", gap: "0.9rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Query Console</h2>
      <textarea aria-label="Heap query" value={queryText} onChange={(event) => setQueryText(event.target.value)} />
      <button type="button" onClick={async () => setResult(await runHeapQuery({ heapPath, query: queryText }))}>
        Run Query
      </button>
      {result?.status === "unavailable" ? <p>Query execution is unavailable in this browser session.</p> : null}
      {result?.status === "error" ? <p>{result.error}</p> : null}
      {result?.status === "ready" ? (
        <table>
          <thead>
            <tr>{result.data.columns.map((column) => <th key={column}>{column}</th>)}</tr>
          </thead>
          <tbody>
            {result.data.rows.map((row, rowIndex) => (
              <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={`${rowIndex}-${cellIndex}`}>{String(cell)}</td>)}</tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </div>
  );
}
```

Create `HeapQueryConsolePage.tsx` to read `artifact.summary.heapPath` from outlet context and render the panel.

- [ ] **Step 4: Run the query-console tests to verify they pass**

Run:

`npx --yes bun test "src/features/heap-explorer/HeapQueryConsolePage.test.tsx" "src/features/heap-explorer/components/QueryConsolePanel.test.tsx"`

Expected: PASS.

- [ ] **Step 5: Commit the query-console slice**

```bash
git add ui/src/app/router.tsx ui/src/features/heap-explorer/HeapQueryConsolePage.tsx ui/src/features/heap-explorer/HeapQueryConsolePage.test.tsx ui/src/features/heap-explorer/components/QueryConsolePanel.tsx ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx
git commit -m "feat(ui): add heap query console"
```

### Task 3: Add Richer Explorer Cross-Navigation

**Files:**
- Modify: `ui/src/features/heap-explorer/HeapDominatorPage.tsx`
- Modify: `ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx`
- Create: `ui/src/features/heap-explorer/components/ExplorerCrossNavActions.tsx`
- Create: `ui/src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx`
- Modify only if needed: `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
- Modify only if needed: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`

- [ ] **Step 1: Write the failing cross-navigation tests**

Create `ExplorerCrossNavActions.test.tsx`:

```ts
it("links from a selected object to object inspector and query console routes", () => {
  render(
    <MemoryRouter>
      <ExplorerCrossNavActions leakId="leak-1" objectId="0x2a" />
    </MemoryRouter>,
  );

  expect(screen.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
    "href",
    "/heap-explorer/object-inspector?objectId=0x2a",
  );
  expect(screen.getByRole("link", { name: /open query console/i })).toHaveAttribute(
    "href",
    "/heap-explorer/query-console?objectId=0x2a",
  );
});
```

If you need leak-workspace handoff, add a route test in `LeakWorkspaceLayout.test.tsx` asserting `objectId` can be seeded from search params.

- [ ] **Step 2: Run the cross-navigation tests to verify they fail**

Run:

`npx --yes bun test "src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx"`

Expected: FAIL because the action component does not exist.

- [ ] **Step 3: Implement the minimal cross-navigation actions**

Create `ExplorerCrossNavActions.tsx`:

```tsx
export function ExplorerCrossNavActions({ leakId, objectId }: { leakId?: string; objectId?: string }) {
  const encodedObjectId = objectId ? encodeURIComponent(objectId) : undefined;

  return (
    <div style={{ display: "flex", gap: "0.65rem", flexWrap: "wrap" }}>
      <Link to={encodedObjectId ? `/heap-explorer/object-inspector?objectId=${encodedObjectId}` : "/heap-explorer/object-inspector"}>
        Open Object Inspector
      </Link>
      <Link to={encodedObjectId ? `/heap-explorer/query-console?objectId=${encodedObjectId}` : "/heap-explorer/query-console"}>
        Open Query Console
      </Link>
      {leakId ? <Link to={`/leaks/${encodeURIComponent(leakId)}/overview`}>Open Leak Workspace</Link> : null}
    </div>
  );
}
```

Render the action component inside both `HeapDominatorPage.tsx` and `HeapObjectInspectorPage.tsx` using the currently selected object ID, and only pass `leakId` when there is a safe artifact-backed match.

- [ ] **Step 4: Run the cross-navigation tests to verify they pass**

Run:

`npx --yes bun test "src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx"`

Expected: PASS.

- [ ] **Step 5: Commit the cross-navigation slice**

```bash
git add ui/src/features/heap-explorer/HeapDominatorPage.tsx ui/src/features/heap-explorer/HeapObjectInspectorPage.tsx ui/src/features/heap-explorer/components/ExplorerCrossNavActions.tsx ui/src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx
git commit -m "feat(ui): connect heap explorer routes"
```

### Task 4: Run Full 5B Verification And Write The Final Project Review

**Files:**
- Create: `docs/superpowers/reviews/2026-04-15-project-review.md`

- [ ] **Step 1: Run the focused heap explorer tests**

Run:

`npx --yes bun test "src/features/heap-explorer/heap-explorer-query-client.test.ts" "src/features/heap-explorer/HeapQueryConsolePage.test.tsx" "src/features/heap-explorer/components/QueryConsolePanel.test.tsx" "src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx"`

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

- [ ] **Step 5: Run the full Rust verification**

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

- [ ] **Step 8: Write the final project review document**

Create `docs/superpowers/reviews/2026-04-15-project-review.md` with this exact structure, filling it from fresh evidence only:

```md
# Project Review

## Verified Scope
- browser-first routes now present
- current artifact-backed and bridge-backed surfaces
- commands run and pass/fail results

## Findings
- list bugs, risks, regressions, or remaining gaps in severity order
- if none, say "No critical or important findings from this review pass."

## Residual Risks
- artifact-only limits
- bridge-only limits
- remaining milestone decisions

## Recommended Next Moves
1. ...
2. ...
3. ...
```

Do not invent a clean bill of health if the review turns up real gaps.

- [ ] **Step 9: Commit the final review and any verification fixes**

```bash
git add ui/src/app/router.tsx ui/src/features/heap-explorer ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx docs/superpowers/reviews/2026-04-15-project-review.md
git commit -m "docs: add post-slice project review"
```
