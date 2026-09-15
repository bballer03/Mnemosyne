import { Navigate, type RouteObject } from "react-router-dom";

import { ArtifactLoaderPage } from "../features/artifact-loader/ArtifactLoaderPage";
import { ArtifactExplorerPage } from "../features/artifact-explorer/ArtifactExplorerPage";
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
import { LeakWorkspaceLayout } from "../features/leak-workspace/LeakWorkspaceLayout";
import { LeakWorkspaceOverview } from "../features/leak-workspace/LeakWorkspaceOverview";
import { InvestigationAssistantPage } from "../features/assistant/InvestigationAssistantPage";
import { FindingsAdvisoryPane } from "../features/investigation/FindingsAdvisoryPane";

/**
 * Minimal route trees for unit tests.
 *
 * Never import production `routes` from `app/router` in tests: that module
 * eagerly pulls every page, and `createMemoryRouter(routes)` under jsdom has
 * OOM'd WSL (~15GB anon RSS) and CI (SIGTERM mid-batch).
 */

const homeRoute: RouteObject = {
  path: "/",
  element: <ArtifactLoaderPage />,
};

const heapExplorerBranch: RouteObject = {
  path: "/heap-explorer",
  element: <HeapExplorerLayout />,
  children: [
    { index: true, element: <Navigate to="dominators" replace /> },
    { path: "dominators", element: <HeapDominatorPage /> },
    { path: "object-inspector", element: <HeapObjectInspectorPage /> },
    { path: "query-console", element: <HeapQueryConsolePage /> },
    { path: "threads", element: <HeapThreadsPage /> },
  ],
};

const leakWorkspaceBranch: RouteObject = {
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
};

/** Assistant page + home redirect target. */
export function assistantRoutes(): RouteObject[] {
  return [homeRoute, { path: "/assistant", element: <InvestigationAssistantPage /> }];
}

/** Heap explorer layout/pages + home redirect target. */
export function heapExplorerRoutes(): RouteObject[] {
  return [homeRoute, heapExplorerBranch];
}

/** Artifact explorer + home redirect target. */
export function artifactExplorerRoutes(): RouteObject[] {
  return [homeRoute, { path: "/artifacts/explorer", element: <ArtifactExplorerPage /> }];
}

/**
 * Dashboard cross-nav coverage: dashboard, artifact explorer, and heap
 * dominators (the destinations exercised by DashboardPage tests).
 */
export function dashboardRoutes(): RouteObject[] {
  return [
    homeRoute,
    {
      path: "/dashboard",
      element: (
        <>
          <FindingsAdvisoryPane />
          <DashboardPage />
        </>
      ),
    },
    { path: "/artifacts/explorer", element: <ArtifactExplorerPage /> },
    heapExplorerBranch,
  ];
}

/** Leak workspace shell + home redirect target. */
export function leakWorkspaceRoutes(): RouteObject[] {
  return [homeRoute, leakWorkspaceBranch];
}
