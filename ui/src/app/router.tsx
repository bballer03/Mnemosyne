import { Navigate, createBrowserRouter, createMemoryRouter, RouterProvider, type RouteObject } from "react-router-dom";

import { ArtifactLoaderPage } from "../features/artifact-loader/ArtifactLoaderPage";
import { ArtifactExplorerPage } from "../features/artifact-explorer/ArtifactExplorerPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { HeapDominatorPage } from "../features/heap-explorer/HeapDominatorPage";
import { HeapExplorerLayout } from "../features/heap-explorer/HeapExplorerLayout";
import { HeapObjectInspectorPage } from "../features/heap-explorer/HeapObjectInspectorPage";
import { HeapQueryConsolePage } from "../features/heap-explorer/HeapQueryConsolePage";
import { LeakExplainPage } from "../features/leak-workspace/LeakExplainPage";
import { LeakFixPage } from "../features/leak-workspace/LeakFixPage";
import { LeakGcPathPage } from "../features/leak-workspace/LeakGcPathPage";
import { LeakSourceMapPage } from "../features/leak-workspace/LeakSourceMapPage";
import { LeakWorkspaceOverview } from "../features/leak-workspace/LeakWorkspaceOverview";
import { LeakWorkspaceLayout } from "../features/leak-workspace/LeakWorkspaceLayout";

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
