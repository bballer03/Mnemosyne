import { useState } from "react";
import {
  Navigate,
  Outlet,
  createBrowserRouter,
  RouterProvider,
  type RouteObject,
  useLocation,
} from "react-router-dom";

import { ArtifactLoaderPage } from "../features/artifact-loader/ArtifactLoaderPage";
import { ArtifactExplorerPage } from "../features/artifact-explorer/ArtifactExplorerPage";
import { ComparisonPage } from "../features/comparison/ComparisonPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { HeapDominatorPage } from "../features/heap-explorer/HeapDominatorPage";
import { HeapExplorerLayout } from "../features/heap-explorer/HeapExplorerLayout";
import { HeapObjectInspectorPage } from "../features/heap-explorer/HeapObjectInspectorPage";
import { HeapQueryConsolePage } from "../features/heap-explorer/HeapQueryConsolePage";
import { HeapThreadsPage } from "../features/heap-explorer/HeapThreadsPage";
import { LeakExplainPage } from "../features/leak-workspace/LeakExplainPage";
import { LeakFixPage } from "../features/leak-workspace/LeakFixPage";
import { LeakGcPathPage } from "../features/leak-workspace/LeakGcPathPage";
import { LeakSourceMapPage } from "../features/leak-workspace/LeakSourceMapPage";
import { LeakWorkspaceOverview } from "../features/leak-workspace/LeakWorkspaceOverview";
import { LeakWorkspaceLayout } from "../features/leak-workspace/LeakWorkspaceLayout";
import { SnapshotManagerPage } from "../features/snapshots/SnapshotManagerPage";
import { PolicyCheckPage } from "../features/policy/PolicyCheckPage";
import { FlamegraphPage } from "../features/flamegraph/FlamegraphPage";
import { InvestigationAssistantPage } from "../features/assistant/InvestigationAssistantPage";
import { FindingsAdvisoryPane } from "../features/investigation/FindingsAdvisoryPane";
import { HeapSessionBar } from "../features/investigation/HeapSessionBar";
import { ComparisonWorkbenchPanel } from "../features/comparison/ComparisonWorkbenchPanel";

/**
 * Persistent Open another / Close chrome for every route once a heap is loaded.
 * Intentionally does NOT duplicate TopNav labels (Dashboard, …) to avoid
 * test collisions — see M14 note below.
 */
function InvestigationChromeLayout() {
  const location = useLocation();

  return (
    <>
      <div style={{ display: "grid", gap: "0.75rem", marginBottom: "1rem" }}>
        <HeapSessionBar />
        <FindingsAdvisoryPane />
        {location.pathname === "/compare" ? null : <ComparisonWorkbenchPanel variant="chrome" />}
      </div>
      <Outlet />
    </>
  );
}

// M14 Slice 14.D note: the persistent top-nav (`TopNav`) is NOT wired in
// here as a shared layout route wrapping every entry below. That was tried
// first and reverted -- several existing pages (`HeapExplorerLayout`,
// `DashboardPage`'s own cross-links, etc.) already render their own
// in-page navigation with the same accessible names ("Dashboard",
// "Artifact Explorer", "Heap Explorer", ...); a global nav with equally
// natural labels collided with those, turning single-match `getByRole`
// queries across many *pre-existing* test files into "found multiple
// elements" failures. Wrapping every route was also more than this slice's
// own task actually requires: the concrete ask is "reachable from the
// landing page" specifically. `TopNav` is rendered directly inside
// `ArtifactLoaderPage` (the `/` route) instead -- see that file's own doc
// comment for the full placement rationale.
//
// M24: `InvestigationChromeLayout` wraps all routes with `HeapSessionBar`
// only (Open another / Close / current heap). Those labels do not collide
// with page-local nav names.
const pageRoutes: RouteObject[] = [
  {
    path: "/",
    element: <ArtifactLoaderPage />,
  },
  {
    path: "/dashboard",
    element: <DashboardPage />,
  },
  {
    path: "/artifacts/explorer",
    element: <ArtifactExplorerPage />,
  },
  {
    path: "/compare",
    element: <ComparisonPage />,
  },
  {
    path: "/workbench/policies",
    element: <PolicyCheckPage />,
  },
  {
    path: "/workbench/snapshots",
    element: <SnapshotManagerPage />,
  },
  {
    path: "/workbench/flamegraphs",
    element: <FlamegraphPage />,
  },
  {
    path: "/assistant",
    element: <InvestigationAssistantPage />,
  },
  {
    path: "/heap-explorer",
    element: <HeapExplorerLayout />,
    children: [
      {
        index: true,
        element: <Navigate to="dominators" replace />,
      },
      {
        path: "dominators",
        element: <HeapDominatorPage />,
      },
      {
        path: "object-inspector",
        element: <HeapObjectInspectorPage />,
      },
      {
        path: "query-console",
        element: <HeapQueryConsolePage />,
      },
      {
        path: "threads",
        element: <HeapThreadsPage />,
      },
    ],
  },
  {
    path: "/leaks/:leakId",
    element: <LeakWorkspaceLayout />,
    children: [
      {
        index: true,
        element: <Navigate to="overview" replace />,
      },
      {
        path: "overview",
        element: <LeakWorkspaceOverview />,
      },
      {
        path: "explain",
        element: <LeakExplainPage />,
      },
      {
        path: "gc-path",
        element: <LeakGcPathPage />,
      },
      {
        path: "source-map",
        element: <LeakSourceMapPage />,
      },
      {
        path: "fix",
        element: <LeakFixPage />,
      },
    ],
  },
];

export const routes: RouteObject[] = [
  {
    element: <InvestigationChromeLayout />,
    children: pageRoutes,
  },
];

/**
 * Create the data router once per app mount — never at module scope.
 *
 * Previously this module always ran both `createBrowserRouter(routes)` and
 * `createMemoryRouter(routes)` on import. Every unit test that imported
 * `{ routes }` therefore pinned two full app trees for the process lifetime.
 * Combined with TopNav tests that remounted/navigated the real tree, that
 * drove ~14GB RSS and crashed WSL / CI.
 *
 * Tests must not `createMemoryRouter(routes)`. Use the minimal trees in
 * `ui/src/test/app-route-trees.tsx` instead.
 */
export function AppRouter() {
  const [router] = useState(() => createBrowserRouter(routes));
  return <RouterProvider router={router} />;
}
