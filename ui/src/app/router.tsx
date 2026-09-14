import { Navigate, createBrowserRouter, createMemoryRouter, RouterProvider, type RouteObject } from "react-router-dom";

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
import { WorkbenchPlaceholderPage } from "../features/workbench/WorkbenchPlaceholderPage";
import { SnapshotManagerPage } from "../features/snapshots/SnapshotManagerPage";

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
export const routes: RouteObject[] = [
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
    element: (
      <WorkbenchPlaceholderPage
        title="Policies"
        summary="Policy-check workbench for shipped policy-engine findings. Dedicated UI lands in M20.F."
      />
    ),
  },
  {
    path: "/workbench/snapshots",
    element: <SnapshotManagerPage />,
  },
  {
    path: "/workbench/flamegraphs",
    element: (
      <WorkbenchPlaceholderPage
        title="Flamegraphs"
        summary="Allocation / retained-size flamegraph explorer for managed artifacts. UI panel lands after policy/snapshot slices."
      />
    ),
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

const future = {
  v7_startTransition: true,
};

const browserRouter = typeof document === "undefined" ? null : createBrowserRouter(routes);
const memoryRouter = createMemoryRouter(routes);

export function AppRouter() {
  const router = typeof document === "undefined" ? memoryRouter : browserRouter!;

  return <RouterProvider router={router} future={future} />;
}
