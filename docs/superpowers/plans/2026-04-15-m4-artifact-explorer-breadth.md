# M4 Artifact Explorer Breadth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated artifact-backed `Artifact Explorer` route with a dominant histogram explorer plus optional analyzer rail modules and selected-bucket detail, all driven only by the loaded browser artifact.

**Architecture:** Extend the frontend artifact adapter to carry the already-shipped optional analyzer sections the Rust `AnalyzeResponse` already serializes, then add a new `ui/src/features/artifact-explorer/` route family. Keep the route strictly artifact-driven: no live bridge, no invented data, and clear `absent` versus `empty` states for optional analyzer modules. Use the Stitch-backed three-column layout direction from project `11463443557609217785` screens `783bcf05d0264375a8ad5f6cabcf707c` and `60a7c166dfa24684b9edf9c87a39d204`.

**Tech Stack:** React, TypeScript, React Router 6, Zustand, Bun, Testing Library

---

## File Structure

### Existing files to modify

- `ui/src/lib/analysis-types.ts`
  - extend the parsed `AnalysisArtifact` shape with artifact-backed optional sections already present in Rust output:
    - `unreachable`
    - `stringReport`
    - `collectionReport`
    - `topInstances`
    - `classloaderReport`
- `ui/src/lib/analysis-types.test.ts`
  - verify those sections parse correctly and remain optional
- `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`
  - verify real serialized artifact text round-trips into the extended frontend shape
- `ui/src/app/router.tsx`
  - register the new artifact explorer route
- `ui/src/features/dashboard/DashboardPage.tsx`
  - add navigation into the explorer route
- `ui/src/features/dashboard/DashboardPage.test.tsx`
  - verify dashboard route access into the explorer

### New files to create

- `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
  - route shell, selected histogram bucket state, artifact guard, top-level three-column layout
- `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`
  - route-level behavior, missing-artifact redirect, histogram selection, analyzer present/absent states
- `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.tsx`
  - search, sort, selected-row state, retained vs shallow comparison bars
- `ui/src/features/artifact-explorer/components/AnalyzerRail.tsx`
  - analyzer summary cards with explicit `SECTION_ABSENT` / `NOT_AVAILABLE` / empty-state rendering
- `ui/src/features/artifact-explorer/components/SelectedBucketDetail.tsx`
  - selected histogram bucket detail derived only from artifact data

### Files intentionally left unchanged

- `ui/src/features/leak-workspace/*`
- Rust-side analysis code and serializers unless frontend verification reveals a concrete artifact-contract gap

---

### Task 1: Extend The Frontend Artifact Adapter For Optional Analyzer Sections

**Files:**
- Modify: `ui/src/lib/analysis-types.ts`
- Modify: `ui/src/lib/analysis-types.test.ts`
- Modify: `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`

- [ ] **Step 1: Write the failing parser tests**

Add a focused parsing test in `ui/src/lib/analysis-types.test.ts` proving the adapter preserves optional analyzer sections when present:

```ts
it("parses optional artifact-backed analyzer sections", () => {
  const parsed = parseAnalysisArtifact({
    summary: {
      heap_path: "heap.hprof",
      total_objects: 42,
      total_size_bytes: 2048,
      total_records: 2,
    },
    leaks: [],
    recommendations: ["Trim cache residency."],
    elapsed: { secs: 1, nanos: 0 },
    graph: {
      node_count: 200,
      edge_count: 400,
      dominators: [],
    },
    histogram: {
      group_by: "class",
      entries: [
        {
          key: "com.example.Cache",
          instance_count: 4,
          shallow_size: 64,
          retained_size: 1024,
        },
      ],
      total_instances: 42,
      total_shallow_size: 2048,
    },
    unreachable: {
      total_count: 3,
      total_shallow_size: 96,
      by_class: [{ class_name: "byte[]", count: 3, shallow_size: 96 }],
    },
    string_report: {
      total_strings: 10,
      total_string_bytes: 512,
      unique_strings: 4,
      duplicate_groups: [{ value: "dup", count: 3, total_wasted_bytes: 32 }],
      total_duplicate_waste: 32,
      top_strings_by_size: [{ object_id: 1, value: "payload", byte_length: 64, retained_bytes: 128 }],
    },
    collection_report: {
      total_collections: 5,
      total_waste_bytes: 128,
      empty_collections: 1,
      oversized_collections: [
        {
          object_id: 11,
          collection_type: "java.util.ArrayList",
          size: 2,
          capacity: 32,
          fill_ratio: 0.0625,
          shallow_bytes: 48,
          retained_bytes: 96,
          waste_bytes: 80,
        },
      ],
      summary_by_type: {
        "java.util.ArrayList": {
          count: 5,
          total_shallow: 240,
          total_retained: 480,
          total_waste: 128,
          avg_fill_ratio: 0.25,
        },
      },
    },
    top_instances: {
      total_count: 2,
      instances: [
        {
          object_id: 7,
          class_name: "byte[]",
          shallow_size: 4096,
          retained_size: 8192,
        },
      ],
    },
    classloader_report: {
      loaders: [
        {
          object_id: 21,
          class_name: "org.springframework.boot.loader.LaunchedURLClassLoader",
          loaded_class_count: 12,
          instance_count: 220,
          total_shallow_bytes: 1024,
          retained_bytes: 4096,
          parent_loader: 1,
        },
      ],
      potential_leaks: [
        {
          object_id: 21,
          class_name: "org.springframework.boot.loader.LaunchedURLClassLoader",
          retained_bytes: 4096,
          loaded_class_count: 12,
          reason: "Retains 4 MB but loads only 12 classes",
        },
      ],
    },
    provenance: [],
  });

  expect(parsed.unreachable?.totalCount).toBe(3);
  expect(parsed.stringReport?.duplicateGroups[0]?.value).toBe("dup");
  expect(parsed.collectionReport?.oversizedCollections[0]?.capacity).toBe(32);
  expect(parsed.topInstances?.instances[0]?.className).toBe("byte[]");
  expect(parsed.classloaderReport?.loaders[0]?.loadedClassCount).toBe(12);
});
```

Extend `ui/src/features/artifact-loader/load-analysis-artifact.test.ts` with one real JSON-text round-trip assertion using the same fields.

- [ ] **Step 2: Run the parser tests to verify they fail**

Run:

`npx --yes bun test "src/lib/analysis-types.test.ts" "src/features/artifact-loader/load-analysis-artifact.test.ts"`

Expected: FAIL because the frontend artifact adapter currently drops those optional analyzer sections.

- [ ] **Step 3: Implement the minimal adapter extensions**

Extend `AnalysisArtifact` in `ui/src/lib/analysis-types.ts` with optional sections shaped from existing Rust output names but converted into frontend camelCase fields:

```ts
type AnalysisArtifact = {
  // existing fields
  histogram?: { ... };
  unreachable?: {
    totalCount: number;
    totalShallowSize: number;
    byClass: Array<{
      className: string;
      count: number;
      shallowSize: number;
    }>;
  };
  stringReport?: {
    totalStrings: number;
    totalStringBytes: number;
    uniqueStrings: number;
    duplicateGroups: Array<{
      value: string;
      count: number;
      totalWastedBytes: number;
    }>;
    totalDuplicateWaste: number;
    topStringsBySize: Array<{
      objectId: number;
      value: string;
      byteLength: number;
      retainedBytes?: number;
    }>;
  };
  collectionReport?: {
    totalCollections: number;
    totalWasteBytes: number;
    emptyCollections: number;
    oversizedCollections: Array<{
      objectId: number;
      collectionType: string;
      size: number;
      capacity?: number;
      fillRatio?: number;
      shallowBytes: number;
      retainedBytes?: number;
      wasteBytes: number;
    }>;
    summaryByType: Record<string, {
      count: number;
      totalShallow: number;
      totalRetained: number;
      totalWaste: number;
      avgFillRatio: number;
    }>;
  };
  topInstances?: {
    totalCount: number;
    instances: Array<{
      objectId: number;
      className: string;
      shallowSize: number;
      retainedSize?: number;
    }>;
  };
  classloaderReport?: {
    loaders: Array<{
      objectId: number;
      className: string;
      loadedClassCount: number;
      instanceCount: number;
      totalShallowBytes: number;
      retainedBytes?: number;
      parentLoader?: number;
    }>;
    potentialLeaks: Array<{
      objectId: number;
      className: string;
      retainedBytes: number;
      loadedClassCount: number;
      reason: string;
    }>;
  };
};
```

Add only the minimal parsing helpers needed for these sections. Keep each section optional if absent from the artifact.

- [ ] **Step 4: Re-run the parser tests to verify they pass**

Run:

`npx --yes bun test "src/lib/analysis-types.test.ts" "src/features/artifact-loader/load-analysis-artifact.test.ts"`

Expected: PASS

---

### Task 2: Add The Artifact Explorer Route And Dashboard Navigation

**Files:**
- Modify: `ui/src/app/router.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.test.tsx`
- Create: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Create: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`

- [ ] **Step 1: Write the failing route tests**

Add a dashboard navigation test:

```tsx
it("opens the artifact explorer from the dashboard", async () => {
  const user = userEvent.setup();

  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: buildArtifactWithHistogram(),
    });
  });

  const router = createMemoryRouter(routes, { initialEntries: ["/dashboard"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("link", { name: /artifact explorer/i }));

  expect(router.state.location.pathname).toBe("/artifacts/explorer");
  expect(view.getByRole("heading", { name: /artifact explorer/i })).toBeInTheDocument();
});
```

Add a route guard test in `ArtifactExplorerPage.test.tsx`:

```tsx
it("redirects back to the loader when no artifact is loaded", () => {
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the route tests to verify they fail**

Run:

`npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx" "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`

Expected: FAIL because the route and page do not exist yet.

- [ ] **Step 3: Add the minimal route shell and navigation**

Create `ArtifactExplorerPage.tsx` with an artifact guard and high-level three-column shell:

```tsx
export function ArtifactExplorerPage() {
  const { artifact, artifactName } = useArtifactStore();

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <section>{/* header with Dashboard / Artifact Explorer / Leak Workspace */}</section>
      <section style={{ display: "grid", gridTemplateColumns: "280px minmax(0, 1fr) 320px", gap: "1rem" }}>
        <aside aria-label="Analyzer rail" />
        <section aria-label="Histogram explorer" />
        <aside aria-label="Selected bucket detail" />
      </section>
    </main>
  );
}
```

Update `router.tsx`:

```tsx
{
  path: "/artifacts/explorer",
  element: <ArtifactExplorerPage />,
}
```

Add a dashboard link in `DashboardPage.tsx` near the top operational context:

```tsx
<Link to="/artifacts/explorer">Artifact Explorer</Link>
```

- [ ] **Step 4: Re-run the route tests to verify they pass**

Run:

`npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx" "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`

Expected: PASS

---

### Task 3: Build The Dominant Histogram Explorer Panel

**Files:**
- Create: `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`

- [ ] **Step 1: Write the failing histogram explorer tests**

Add tests proving:

```tsx
it("renders all histogram rows with retained and shallow comparisons", () => {
  seedArtifactWithHistogram();
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByRole("heading", { name: /artifact explorer/i })).toBeInTheDocument();
  expect(view.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
  expect(view.getByText(/retained vs shallow/i)).toBeInTheDocument();
});

it("filters histogram rows by search text", async () => {
  const user = userEvent.setup();
  seedArtifactWithHistogram();
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.type(view.getByLabelText(/search histogram/i), "concurrent");

  expect(view.getByText(/concurrenthashmap/i)).toBeInTheDocument();
  expect(view.queryByText(/com\.example\.Cache/i)).toBeNull();
});

it("shows an explicit histogram-absent state when the artifact has no histogram", () => {
  seedArtifactWithoutHistogram();
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/histogram data is absent from this artifact/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the histogram explorer tests to verify they fail**

Run: `npx --yes bun test "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`
Expected: FAIL because the route shell does not yet render histogram behavior.

- [ ] **Step 3: Implement the minimal histogram explorer**

Create `HistogramExplorerPanel.tsx` with:

- local search state
- stable sort by retained size descending, with a secondary sort by shallow size
- selected-row callback
- explicit absent state when `artifact.histogram` is missing
- explicit empty state when `artifact.histogram.entries` is empty
- retained and shallow comparison bars using relative widths against the largest retained size entry

Wire it into `ArtifactExplorerPage.tsx`:

```tsx
const [selectedHistogramKey, setSelectedHistogramKey] = useState<string | undefined>(artifact.histogram?.entries[0]?.key);

<HistogramExplorerPanel
  artifact={artifact}
  selectedKey={selectedHistogramKey}
  onSelectKey={setSelectedHistogramKey}
/>
```

Keep the panel artifact-only: no virtual data and no bridge assumptions.

- [ ] **Step 4: Re-run the histogram explorer tests to verify they pass**

Run: `npx --yes bun test "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`
Expected: PASS

---

### Task 4: Add The Analyzer Rail And Selected Bucket Detail

**Files:**
- Create: `ui/src/features/artifact-explorer/components/AnalyzerRail.tsx`
- Create: `ui/src/features/artifact-explorer/components/SelectedBucketDetail.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`

- [ ] **Step 1: Write the failing analyzer/detail tests**

Add tests proving:

```tsx
it("renders analyzer cards from artifact-backed optional sections and labels absent sections explicitly", () => {
  seedArtifactWithAnalyzers();
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/artifact recommendations/i)).toBeInTheDocument();
  expect(view.getByText(/string deduplication/i)).toBeInTheDocument();
  expect(view.getByText(/section_absent/i)).toBeInTheDocument();
});

it("updates the selected bucket detail from the chosen histogram row", async () => {
  const user = userEvent.setup();
  seedArtifactWithHistogram();
  const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("button", { name: /select java\.util\.concurrent\.concurrenthashmap/i }));

  expect(view.getByText(/selected bucket/i)).toBeInTheDocument();
  expect(view.getByText(/java\.util\.concurrent\.concurrenthashmap/i)).toBeInTheDocument();
  expect(view.getByText(/artifact-backed leak hints/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the analyzer/detail tests to verify they fail**

Run: `npx --yes bun test "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`
Expected: FAIL because the rail/detail panels do not exist yet.

- [ ] **Step 3: Implement the analyzer rail and selected detail**

`AnalyzerRail.tsx` should render one compact card per module:

- Recommendations
- Strings
- Collections
- Top Instances
- Classloaders
- Unreachable Summary

Each card must distinguish:

- section present with summary values
- section present but empty
- section absent from artifact using explicit text such as `SECTION_ABSENT` or `NOT_AVAILABLE`

`SelectedBucketDetail.tsx` should render:

- selected histogram key
- instance/shallow/retained values from the selected histogram entry
- group-by context
- artifact-backed leak hints derived only from `artifact.leaks`
- provenance/readiness notes when no direct leak relationship can be derived

Keep derivation rules narrow:

- exact class-name match when histogram `groupBy === "class"`
- package-prefix grouping only when string prefix matching is straightforward
- otherwise say that no direct leak relationship is proven by this artifact bucket

- [ ] **Step 4: Re-run the analyzer/detail tests to verify they pass**

Run: `npx --yes bun test "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx"`
Expected: PASS

---

### Task 5: Verify Slice 4 End-To-End

**Files:**
- Verify only

- [ ] **Step 1: Run the full frontend test suite**

Run: `npx --yes bun test`
Expected: PASS

- [ ] **Step 2: Run the frontend build**

Run: `npx --yes bun run build`
Expected: PASS

- [ ] **Step 3: Run the frontend lint/type check**

Run: `npx --yes bun run lint`
Expected: PASS

- [ ] **Step 4: Run repo-standard Rust verification**

Run: `cargo test`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 5: Inspect final local scope**

Run: `git status --short`
Expected: intended artifact-explorer files, updated parser/router/dashboard files, Slice 4 spec + plan files, and unrelated pre-existing worktree changes

---

## Slice Queue After This Plan

After this slice lands, continue in this order:

1. competitive explorer surfaces (dominator/object/query)

## Execution Note

This repository is already being worked from an isolated worktree. Do not create commits unless the user explicitly asks for them.
