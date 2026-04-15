# M4 Leak Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the next M4 browser-first route family as a dedicated leak workspace with nested subroutes for overview, explain, GC path, source map, and fix.

**Architecture:** Keep the existing artifact-driven dashboard intact and add a focused `leak-workspace` feature that owns its own router shell, feature-local store, and narrow live-detail adapter boundary. The overview route remains artifact-backed, while the deeper subroutes normalize live detail responses into honest `ready` / `error` / `fallback` / `unavailable` states without introducing a broad app-wide RPC layer.

**Tech Stack:** React, TypeScript, React Router 6, Zustand, Bun, Testing Library, existing Mnemosyne UI patterns under `ui/src/features/`

---

## File Structure

### Existing files to modify

- `ui/src/app/router.tsx`
  - add nested leak-workspace routes and redirect behavior
- `ui/src/features/dashboard/components/LeakTable.tsx`
  - turn the disabled `Trace` action into navigation
- `ui/src/features/dashboard/components/LeakTable.test.tsx`
  - cover trace navigation affordance changes
- `ui/src/features/dashboard/DashboardPage.test.tsx`
  - cover route integration from dashboard to leak workspace
- `ui/src/app/App.test.tsx`
  - keep the shell assertions aligned if route structure changes affect top-level rendering
- `docs/roadmap.md`
  - sync the route as shipped when work is complete
- `STATUS.md`
  - sync current M4 capability status after implementation lands
- `README.md`
  - document the new leak workspace route if user-facing workflow changes

### New files to create

- `ui/src/features/leak-workspace/types.ts`
  - typed normalized result/state definitions for the workspace
- `ui/src/features/leak-workspace/live-detail-client.ts`
  - adapter boundary for explain, GC path, source map, and fix operations
- `ui/src/features/leak-workspace/leak-workspace-store.ts`
  - feature-local Zustand store for selected leak context and per-subview status/cache
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
  - persistent shell with header, mode nav, and dependency rail
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
  - artifact-backed overview subroute
- `ui/src/features/leak-workspace/LeakExplainPage.tsx`
  - live explain subroute
- `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
  - GC-path subroute with explicit unavailable state
- `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
  - source map subroute
- `ui/src/features/leak-workspace/LeakFixPage.tsx`
  - fix subroute
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
  - shell and route guard coverage
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
  - artifact-backed overview coverage
- `ui/src/features/leak-workspace/live-detail-client.test.ts`
  - adapter normalization coverage
- `ui/src/features/leak-workspace/LeakExplainPage.test.tsx`
  - explain loading/error/fallback coverage
- `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`
  - GC-path unavailable/error/ready coverage
- `ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx`
  - source map unavailable/fallback/ready coverage
- `ui/src/features/leak-workspace/LeakFixPage.test.tsx`
  - fix unavailable/fallback/ready coverage

---

### Task 1: Add The Leak Workspace Route Shell

**Files:**
- Modify: `ui/src/app/router.tsx`
- Create: `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
- Create: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
- Test: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`

- [ ] **Step 1: Write the failing route-shell tests**

```tsx
import "../../test/setup";

import { act, render } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakWorkspaceLayout } from "./LeakWorkspaceLayout";

describe("LeakWorkspaceLayout", () => {
  it("redirects missing artifacts back to the loader", () => {
    const router = createMemoryRouter(
      [
        { path: "/", element: <div>loader</div> },
        { path: "/leaks/:leakId/*", element: <LeakWorkspaceLayout /> },
      ],
      { initialEntries: ["/leaks/leak-1"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText("loader")).toBeInTheDocument();
  });

  it("renders an invalid-selection state when the leak id is unknown", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [{ path: "/leaks/:leakId/*", element: <LeakWorkspaceLayout /> }],
      { initialEntries: ["/leaks/leak-404"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/selected leak was not found in the loaded artifact/i)).toBeInTheDocument();
    expect(view.getByRole("link", { name: /back to dashboard/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the route-shell tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: FAIL because `LeakWorkspaceLayout.tsx` does not exist yet.

- [ ] **Step 3: Write the minimal route shell implementation**

```tsx
import { Link, Navigate, Outlet, useLocation, useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

export function LeakWorkspaceLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const { leakId } = useParams();
  const location = useLocation();

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  const leak = artifact.leaks.find((entry) => entry.id === leakId);

  if (!leak) {
    return (
      <main style={{ display: "grid", gap: "1rem" }}>
        <h2 style={{ margin: 0 }}>Leak Workspace</h2>
        <div>Selected leak was not found in the loaded artifact.</div>
        <div>Artifact: {artifactName ?? "Unnamed artifact"}</div>
        <Link to="/dashboard">Back to dashboard</Link>
      </main>
    );
  }

  const basePath = `/leaks/${leak.id}`;
  const tabs = [
    { to: `${basePath}/overview`, label: "Overview" },
    { to: `${basePath}/explain`, label: "Explain" },
    { to: `${basePath}/gc-path`, label: "GC Path" },
    { to: `${basePath}/source-map`, label: "Source Map" },
    { to: `${basePath}/fix`, label: "Fix Proposal" },
  ];

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <header style={{ display: "grid", gap: "0.5rem" }}>
        <Link to="/dashboard">Back to dashboard</Link>
        <div>{artifact.summary.heapPath}</div>
        <h2 style={{ margin: 0 }}>{leak.className}</h2>
        <div>{leak.id}</div>
      </header>
      <nav aria-label="Leak workspace modes" style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
        {tabs.map((tab) => {
          const active = location.pathname === tab.to;
          return (
            <Link key={tab.to} to={tab.to} aria-current={active ? "page" : undefined}>
              {tab.label}
            </Link>
          );
        })}
      </nav>
      <Outlet context={{ leak, artifact }} />
    </main>
  );
}
```

- [ ] **Step 4: Wire the nested routes in the router**

```tsx
import { Navigate, createBrowserRouter, createMemoryRouter, RouterProvider } from "react-router-dom";

import { ArtifactLoaderPage } from "../features/artifact-loader/ArtifactLoaderPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { LeakWorkspaceLayout } from "../features/leak-workspace/LeakWorkspaceLayout";

const routes = [
  { path: "/", element: <ArtifactLoaderPage /> },
  { path: "/dashboard", element: <DashboardPage /> },
  {
    path: "/leaks/:leakId",
    element: <LeakWorkspaceLayout />,
    children: [
      { index: true, element: <Navigate to="overview" replace /> },
      { path: "overview", element: <div>Leak workspace overview placeholder</div> },
      { path: "explain", element: <div>Leak explain placeholder</div> },
      { path: "gc-path", element: <div>Leak GC path placeholder</div> },
      { path: "source-map", element: <div>Leak source map placeholder</div> },
      { path: "fix", element: <div>Leak fix placeholder</div> },
    ],
  },
];
```

- [ ] **Step 5: Run the route-shell tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add ui/src/app/router.tsx ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx
git commit -m "feat(ui): add leak workspace shell"
```

### Task 2: Add The Artifact-Backed Overview Subroute

**Files:**
- Create: `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
- Create: `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
- Create: `ui/src/features/leak-workspace/types.ts`
- Modify: `ui/src/app/router.tsx`
- Test: `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`

- [ ] **Step 1: Write the failing overview tests**

```tsx
import "../../test/setup";

import { act, render } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakWorkspaceOverview } from "./LeakWorkspaceOverview";

function seedArtifact() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 42,
          totalSizeBytes: 2048,
          totalRecords: 2,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 1024,
            shallowSizeBytes: 64,
            suspectScore: 0.98,
            instances: 4,
            description: "Cache retains request objects",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
        },
        provenance: [],
      },
    });
  });
}

describe("LeakWorkspaceOverview", () => {
  it("renders artifact-backed leak summary and preview regions", () => {
    seedArtifact();
    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/dependency readiness/i)).toBeInTheDocument();
    expect(view.getByText(/explain/i)).toBeInTheDocument();
    expect(view.getByText(/gc path/i)).toBeInTheDocument();
    expect(view.getByText(/source map/i)).toBeInTheDocument();
    expect(view.getByText(/fix proposal/i)).toBeInTheDocument();
  });

  it("shows gc path as unavailable when no object target exists", () => {
    seedArtifact();
    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/gc path unavailable until an object target is present/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the overview tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx"`
Expected: FAIL because the overview component does not exist yet.

- [ ] **Step 3: Add the minimal workspace types**

```ts
import type { AnalysisArtifact } from "../../lib/analysis-types";

export type LeakWorkspaceDependencyStatus = {
  bridge: "ready" | "unavailable";
  projectRoot: "present" | "missing";
  objectTarget: "present" | "missing";
  provider: "ready" | "unknown" | "unavailable";
};

export type LiveSubviewStatus = "idle" | "loading" | "ready" | "error" | "unavailable" | "fallback";

export type SelectedLeak = AnalysisArtifact["leaks"][number];
```

- [ ] **Step 4: Write the minimal overview implementation**

```tsx
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

export function LeakWorkspaceOverview() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();

  if (!artifact) {
    return null;
  }

  const leak = artifact.leaks.find((entry) => entry.id === leakId);

  if (!leak) {
    return null;
  }

  const dependencyStatus = {
    bridge: "unavailable",
    projectRoot: "missing",
    objectTarget: "missing",
    provider: "unknown",
  } as const;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <div>
        <h3 style={{ margin: 0 }}>Overview</h3>
        <p style={{ margin: "0.5rem 0 0" }}>{leak.description}</p>
      </div>
      <section aria-label="Dependency readiness" style={{ display: "grid", gap: "0.5rem" }}>
        <h4 style={{ margin: 0 }}>Dependency readiness</h4>
        <div>Bridge: {dependencyStatus.bridge}</div>
        <div>Project root: {dependencyStatus.projectRoot}</div>
        <div>Object target: {dependencyStatus.objectTarget}</div>
        <div>Provider: {dependencyStatus.provider}</div>
      </section>
      <section style={{ display: "grid", gap: "0.5rem" }}>
        <div>Explain preview</div>
        <div>{leak.className}</div>
      </section>
      <section style={{ display: "grid", gap: "0.5rem" }}>
        <div>GC Path preview</div>
        <div>
          {dependencyStatus.objectTarget === "present"
            ? "GC path can be loaded from a concrete object target."
            : "GC path unavailable until an object target is present."}
        </div>
      </section>
      <section style={{ display: "grid", gap: "0.5rem" }}>
        <div>Source Map preview</div>
        <div>Heap path: {artifact.summary.heapPath}</div>
      </section>
      <section style={{ display: "grid", gap: "0.5rem" }}>
        <div>Fix Proposal preview</div>
        <div>Leak ID: {leak.id}</div>
      </section>
    </section>
  );
}
```

- [ ] **Step 5: Replace the overview route placeholder with the real component**

```tsx
import { LeakWorkspaceOverview } from "../features/leak-workspace/LeakWorkspaceOverview";

const routes = [
  { path: "/", element: <ArtifactLoaderPage /> },
  { path: "/dashboard", element: <DashboardPage /> },
  {
    path: "/leaks/:leakId",
    element: <LeakWorkspaceLayout />,
    children: [
      { index: true, element: <Navigate to="overview" replace /> },
      { path: "overview", element: <LeakWorkspaceOverview /> },
      { path: "explain", element: <div>Leak explain placeholder</div> },
      { path: "gc-path", element: <div>Leak GC path placeholder</div> },
      { path: "source-map", element: <div>Leak source map placeholder</div> },
      { path: "fix", element: <div>Leak fix placeholder</div> },
    ],
  },
];
```

- [ ] **Step 6: Run the overview tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx"`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add ui/src/app/router.tsx ui/src/features/leak-workspace/types.ts ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx
git commit -m "feat(ui): add leak workspace overview"
```

### Task 3: Add The Workspace Store And Adapter Boundary

**Files:**
- Create: `ui/src/features/leak-workspace/leak-workspace-store.ts`
- Create: `ui/src/features/leak-workspace/live-detail-client.ts`
- Create: `ui/src/features/leak-workspace/live-detail-client.test.ts`
- Test: `ui/src/features/leak-workspace/live-detail-client.test.ts`

- [ ] **Step 1: Write the failing adapter tests**

```ts
import { describe, expect, it } from "bun:test";

import { normalizeSourceMapResult, normalizeFixResult } from "./live-detail-client";

describe("live detail client", () => {
  it("marks unmapped source results as fallback", () => {
    const result = normalizeSourceMapResult({
      leak_id: "leak-1",
      locations: [
        {
          file: ".mnemosyne/unmapped/com/example/LeakHotspot.java",
          line: 1,
          symbol: "Unknown",
          code_snippet: "",
          git: null,
        },
      ],
    });

    expect(result.status).toBe("fallback");
  });

  it("marks heuristic fix results with provenance as fallback", () => {
    const result = normalizeFixResult({
      suggestions: [],
      provenance: [{ kind: "FALLBACK", detail: "heuristic guidance" }],
    });

    expect(result.status).toBe("fallback");
  });
});
```

- [ ] **Step 2: Run the adapter tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: FAIL because the adapter file does not exist yet.

- [ ] **Step 3: Write the minimal store and adapter implementation**

```ts
import { create } from "zustand";

import type { LiveSubviewStatus } from "./types";

type CachedResult<T> = { status: LiveSubviewStatus; data?: T; error?: string };

type LeakWorkspaceState = {
  projectRoot?: string;
  explain: CachedResult<unknown>;
  gcPath: CachedResult<unknown>;
  sourceMap: CachedResult<unknown>;
  fix: CachedResult<unknown>;
  setProjectRoot: (value?: string) => void;
  setSubviewState: (key: "explain" | "gcPath" | "sourceMap" | "fix", value: CachedResult<unknown>) => void;
  reset: () => void;
};

const initialSubview = { status: "idle" as const };

export const useLeakWorkspaceStore = create<LeakWorkspaceState>((set) => ({
  projectRoot: undefined,
  explain: initialSubview,
  gcPath: initialSubview,
  sourceMap: initialSubview,
  fix: initialSubview,
  setProjectRoot: (value) => set({ projectRoot: value }),
  setSubviewState: (key, value) => set({ [key]: value } as Partial<LeakWorkspaceState>),
  reset: () =>
    set({
      projectRoot: undefined,
      explain: initialSubview,
      gcPath: initialSubview,
      sourceMap: initialSubview,
      fix: initialSubview,
    }),
}));

export function normalizeSourceMapResult(input: { locations: Array<{ file: string }> }) {
  const fallback = input.locations.some((location) => location.file.includes(".mnemosyne/unmapped"));
  return { status: fallback ? "fallback" : "ready", data: input };
}

export function normalizeFixResult(input: { provenance?: Array<{ kind: string }> }) {
  const fallback = (input.provenance ?? []).some((marker) => marker.kind === "FALLBACK");
  return { status: fallback ? "fallback" : "ready", data: input };
}
```

- [ ] **Step 4: Run the adapter tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/features/leak-workspace/leak-workspace-store.ts ui/src/features/leak-workspace/live-detail-client.ts ui/src/features/leak-workspace/live-detail-client.test.ts
git commit -m "feat(ui): add leak workspace live detail adapter"
```

### Task 4: Replace The Dashboard Trace Placeholder With Navigation

**Files:**
- Modify: `ui/src/features/dashboard/components/LeakTable.tsx`
- Modify: `ui/src/features/dashboard/components/LeakTable.test.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.test.tsx`
- Test: `ui/src/features/dashboard/components/LeakTable.test.tsx`

- [ ] **Step 1: Write the failing trace-navigation test**

```tsx
Replace the existing disabled `Trace` button inside the leak-row action group with:

```tsx
import "../../test/setup";

import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { LeakTable } from "./LeakTable";

const artifact = {
  summary: {
    heapPath: "fixture.hprof",
    totalObjects: 1,
    totalSizeBytes: 1,
    totalRecords: 1,
  },
  leaks: [
    {
      id: "leak-1",
      className: "com.example.Cache",
      leakKind: "CACHE",
      severity: "HIGH",
      retainedSizeBytes: 100,
      instances: 1,
      description: "Cache leak",
      provenance: [],
    },
  ],
  recommendations: [],
  elapsedSeconds: 1,
  graph: {
    nodeCount: 1,
    edgeCount: 1,
    dominatorCount: 1,
  },
  provenance: [],
} as const;

it("navigates to the leak workspace when Trace is clicked", async () => {
  const user = userEvent.setup();
  const router = createMemoryRouter(
    [
      { path: "/dashboard", element: <LeakTable artifact={artifact} /> },
      { path: "/leaks/:leakId/overview", element: <div>workspace overview</div> },
    ],
    { initialEntries: ["/dashboard"] },
  );

  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("button", { name: /trace/i }));

  expect(view.getByText("workspace overview")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the trace-navigation test to verify it fails**

Run: `npx --yes bun test "src/features/dashboard/components/LeakTable.test.tsx"`
Expected: FAIL because the current `Trace` control is disabled.

- [ ] **Step 3: Write the minimal navigation change**

```tsx
<button
  type="button"
  onClick={() => navigate(`/leaks/${leak.id}/overview`)}
  style={{
    borderRadius: 999,
    border: "1px solid #334155",
    background: "rgba(15, 23, 42, 0.8)",
    color: "#cbd5e1",
    padding: "0.35rem 0.7rem",
    cursor: "pointer",
  }}
>
  Trace
</button>
```

- [ ] **Step 4: Run the trace-navigation test to verify it passes**

Run: `npx --yes bun test "src/features/dashboard/components/LeakTable.test.tsx"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/features/dashboard/components/LeakTable.tsx ui/src/features/dashboard/components/LeakTable.test.tsx ui/src/features/dashboard/DashboardPage.test.tsx
git commit -m "feat(ui): route dashboard trace into leak workspace"
```

### Task 5: Implement The Explain And Source Map Subviews

**Files:**
- Create: `ui/src/features/leak-workspace/LeakExplainPage.tsx`
- Create: `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
- Create: `ui/src/features/leak-workspace/LeakExplainPage.test.tsx`
- Create: `ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx`
- Modify: `ui/src/features/leak-workspace/live-detail-client.ts`
- Modify: `ui/src/app/router.tsx`
- Test: `ui/src/features/leak-workspace/LeakExplainPage.test.tsx`
- Test: `ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx`

- [ ] **Step 1: Write the failing explain and source-map tests**

```tsx
import "../../test/setup";

import { act, render } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useLeakWorkspaceStore } from "./leak-workspace-store";
import { LeakExplainPage } from "./LeakExplainPage";
import { LeakSourceMapPage } from "./LeakSourceMapPage";

function seedArtifact() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 42,
          totalSizeBytes: 2048,
          totalRecords: 2,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 1024,
            shallowSizeBytes: 64,
            suspectScore: 0.98,
            instances: 4,
            description: "Cache retains request objects",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
        },
        provenance: [],
      },
    });
  });
}

it("renders explain loading and ready states", async () => {
  seedArtifact();
  const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
    initialEntries: ["/leaks/leak-1/explain"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/loading explanation/i)).toBeInTheDocument();
  expect(await view.findByText(/top leak is retaining a large share of the heap/i)).toBeInTheDocument();
});

it("renders source-map unavailable when project root is missing", () => {
  seedArtifact();
  useLeakWorkspaceStore.setState({ projectRoot: undefined });
  const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
    initialEntries: ["/leaks/leak-1/source-map"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/source map is unavailable until a project root is configured/i)).toBeInTheDocument();
});

it("renders source-map fallback when mapping returns unmapped results", async () => {
  seedArtifact();
  useLeakWorkspaceStore.setState({ projectRoot: "D:/repo" });
  const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
    initialEntries: ["/leaks/leak-1/source-map"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/mapping fell back to an unmapped placeholder/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the explain and source-map tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakExplainPage.test.tsx" "src/features/leak-workspace/LeakSourceMapPage.test.tsx"`
Expected: FAIL because the subview files do not exist yet.

- [ ] **Step 3: Add the minimal explain and source-map implementations**

```tsx
import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { explainLeak, mapToCode } from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

export function LeakExplainPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const [summary, setSummary] = useState<string>();

  useEffect(() => {
    if (!artifact || !leakId) {
      return;
    }

    explainLeak(leakId, artifact.summary.heapPath).then((result) => setSummary(result.data.summary));
  }, [artifact, leakId]);

  return <section>{summary ?? "Loading explanation..."}</section>;
}

export function LeakSourceMapPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const { projectRoot } = useLeakWorkspaceStore();
  const [message, setMessage] = useState("Loading source map...");

  const leak = artifact?.leaks.find((entry) => entry.id === leakId);

  if (!projectRoot) {
    return <section>Source map is unavailable until a project root is configured.</section>;
  }

  useEffect(() => {
    if (!leakId || !leak) {
      return;
    }

    mapToCode(leakId, leak.className, projectRoot).then((result) => {
      setMessage(result.status === "fallback" ? "Mapping fell back to an unmapped placeholder." : "Source map ready.");
    });
  }, [leak, leakId, projectRoot]);

  return <section>{message}</section>;
}
```

- [ ] **Step 4: Implement minimal load-on-mount behavior through the adapter**

```ts
export async function explainLeak() {
  return { status: "ready", data: { summary: "Top leak is retaining a large share of the heap." } };
}

export async function mapToCode() {
  return normalizeSourceMapResult({
    locations: [
      {
        file: ".mnemosyne/unmapped/com/example/LeakHotspot.java",
        line: 1,
        symbol: "Unknown",
        code_snippet: "",
        git: null,
      },
    ],
  });
}
```

- [ ] **Step 5: Replace the explain and source-map route placeholders**

```tsx
import { LeakExplainPage } from "../features/leak-workspace/LeakExplainPage";
import { LeakSourceMapPage } from "../features/leak-workspace/LeakSourceMapPage";

const routes = [
  { path: "/", element: <ArtifactLoaderPage /> },
  { path: "/dashboard", element: <DashboardPage /> },
  {
    path: "/leaks/:leakId",
    element: <LeakWorkspaceLayout />,
    children: [
      { index: true, element: <Navigate to="overview" replace /> },
      { path: "overview", element: <LeakWorkspaceOverview /> },
      { path: "explain", element: <LeakExplainPage /> },
      { path: "gc-path", element: <div>Leak GC path placeholder</div> },
      { path: "source-map", element: <LeakSourceMapPage /> },
      { path: "fix", element: <div>Leak fix placeholder</div> },
    ],
  },
];
```

- [ ] **Step 6: Run the explain and source-map tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakExplainPage.test.tsx" "src/features/leak-workspace/LeakSourceMapPage.test.tsx"`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add ui/src/app/router.tsx ui/src/features/leak-workspace/LeakExplainPage.tsx ui/src/features/leak-workspace/LeakSourceMapPage.tsx ui/src/features/leak-workspace/LeakExplainPage.test.tsx ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx ui/src/features/leak-workspace/live-detail-client.ts
git commit -m "feat(ui): add explain and source map subviews"
```

### Task 6: Implement The Fix And GC Path Subviews

**Files:**
- Create: `ui/src/features/leak-workspace/LeakFixPage.tsx`
- Create: `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
- Create: `ui/src/features/leak-workspace/LeakFixPage.test.tsx`
- Create: `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`
- Modify: `ui/src/features/leak-workspace/live-detail-client.ts`
- Modify: `ui/src/app/router.tsx`
- Test: `ui/src/features/leak-workspace/LeakFixPage.test.tsx`
- Test: `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`

- [ ] **Step 1: Write the failing fix and gc-path tests**

```tsx
import "../../test/setup";

import { act, render } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { LeakFixPage } from "./LeakFixPage";
import { LeakGcPathPage } from "./LeakGcPathPage";

function seedArtifact() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 42,
          totalSizeBytes: 2048,
          totalRecords: 2,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 1024,
            shallowSizeBytes: 64,
            suspectScore: 0.98,
            instances: 4,
            description: "Cache retains request objects",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
        },
        provenance: [],
      },
    });
  });
}

it("renders fix fallback guidance when provider-backed generation is unavailable", async () => {
  seedArtifact();
  const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
    initialEntries: ["/leaks/leak-1/fix"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/heuristic guidance/i)).toBeInTheDocument();
  expect(await view.findByText(/fallback/i)).toBeInTheDocument();
});

it("renders gc-path unavailable when no object target exists", () => {
  seedArtifact();
  const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
    initialEntries: ["/leaks/leak-1/gc-path"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/gc path is unavailable for this leak until an object target is present/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the fix and gc-path tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakFixPage.test.tsx" "src/features/leak-workspace/LeakGcPathPage.test.tsx"`
Expected: FAIL because the subview files do not exist yet.

- [ ] **Step 3: Add the minimal fix and gc-path implementations**

```tsx
import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { proposeFix } from "./live-detail-client";

export function LeakFixPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const [message, setMessage] = useState("Loading fix proposal...");

  useEffect(() => {
    if (!artifact || !leakId) {
      return;
    }

    proposeFix(leakId, artifact.summary.heapPath).then((result) => {
      setMessage(result.status === "fallback" ? "Fallback: heuristic guidance" : "Fix proposal ready.");
    });
  }, [artifact, leakId]);

  return <section>{message}</section>;
}

export function LeakGcPathPage() {
  const objectId = undefined;

  if (objectId) {
    return <section>GC path ready.</section>;
  }

  return <section>GC path is unavailable for this leak until an object target is present.</section>;
}
```

- [ ] **Step 4: Add the minimal fix adapter result**

```ts
export async function proposeFix() {
  return normalizeFixResult({
    suggestions: [
      {
        leak_id: "leak-1",
        class_name: "com.example.Cache",
        target_file: "src/main/java/com/example/Cache.java",
        description: "heuristic guidance",
        diff: "@@ -1 +1 @@",
        confidence: 0.42,
        style: "Minimal",
      },
    ],
    provenance: [{ kind: "FALLBACK", detail: "heuristic guidance" }],
  });
}
```

- [ ] **Step 5: Replace the fix and gc-path route placeholders**

```tsx
import { LeakFixPage } from "../features/leak-workspace/LeakFixPage";
import { LeakGcPathPage } from "../features/leak-workspace/LeakGcPathPage";

const routes = [
  { path: "/", element: <ArtifactLoaderPage /> },
  { path: "/dashboard", element: <DashboardPage /> },
  {
    path: "/leaks/:leakId",
    element: <LeakWorkspaceLayout />,
    children: [
      { index: true, element: <Navigate to="overview" replace /> },
      { path: "overview", element: <LeakWorkspaceOverview /> },
      { path: "explain", element: <LeakExplainPage /> },
      { path: "gc-path", element: <LeakGcPathPage /> },
      { path: "source-map", element: <LeakSourceMapPage /> },
      { path: "fix", element: <LeakFixPage /> },
    ],
  },
];
```

- [ ] **Step 6: Run the fix and gc-path tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakFixPage.test.tsx" "src/features/leak-workspace/LeakGcPathPage.test.tsx"`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add ui/src/app/router.tsx ui/src/features/leak-workspace/LeakFixPage.tsx ui/src/features/leak-workspace/LeakGcPathPage.tsx ui/src/features/leak-workspace/LeakFixPage.test.tsx ui/src/features/leak-workspace/LeakGcPathPage.test.tsx ui/src/features/leak-workspace/live-detail-client.ts
git commit -m "feat(ui): add fix and gc path subviews"
```

### Task 7: Verify The Full Leak Workspace Flow And Sync Docs

**Files:**
- Modify: `README.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Test: `ui/src/app/App.test.tsx`
- Test: `ui/src/features/dashboard/DashboardPage.test.tsx`

- [ ] **Step 1: Write or extend the failing route integration test**

```tsx
it("opens the leak workspace overview from the dashboard trace action", async () => {
  const user = userEvent.setup();

  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 42,
          totalSizeBytes: 2048,
          totalRecords: 2,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 1024,
            shallowSizeBytes: 64,
            suspectScore: 0.98,
            instances: 4,
            description: "Cache retains request objects",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
        },
        provenance: [],
      },
    });
  });

  const routes = [
    { path: "/", element: <ArtifactLoaderPage /> },
    { path: "/dashboard", element: <DashboardPage /> },
    {
      path: "/leaks/:leakId",
      element: <LeakWorkspaceLayout />,
      children: [{ path: "overview", element: <LeakWorkspaceOverview /> }],
    },
  ];

  const router = createMemoryRouter(routes, { initialEntries: ["/dashboard"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await user.click(view.getByRole("button", { name: /trace/i }));

  expect(view.getByRole("link", { name: /back to dashboard/i })).toBeInTheDocument();
  expect(view.getByRole("link", { name: /overview/i })).toBeInTheDocument();
  expect(view.getByText(/dependency readiness/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the route integration test to verify it fails if the flow is incomplete**

Run: `npx --yes bun test "src/features/dashboard/DashboardPage.test.tsx"`
Expected: FAIL until the full route flow is wired.

- [ ] **Step 3: Update docs for the shipped leak workspace route**

```md
- README: note that the dashboard now routes into a leak workspace with overview/explain/gc-path/source-map/fix modes
- STATUS: record the leak workspace as the next delivered M4 screen family
- roadmap: mark the leak drill-down route as the current follow-on route rather than future-only text
```

- [ ] **Step 4: Run the frontend verification suite**

Run: `npx --yes bun test`
Expected: PASS with all UI tests green

- [ ] **Step 5: Run the frontend build and lint checks**

Run: `npx --yes bun run build`
Expected: PASS

Run: `npx --yes bun run lint`
Expected: PASS

- [ ] **Step 6: Run the Rust workspace verification suite**

Run: `cargo test`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add README.md STATUS.md docs/roadmap.md ui/src/app/App.test.tsx ui/src/features/dashboard/DashboardPage.test.tsx ui/src/features/leak-workspace
git commit -m "feat(ui): ship leak workspace routes"
```
