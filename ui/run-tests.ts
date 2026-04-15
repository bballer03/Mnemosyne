import { spawnSync } from "node:child_process";

const bunExecutable = process.execPath;

const testBatches = [
  [
    "src/lib/analysis-types.test.ts",
    "src/app/App.test.tsx",
    "src/features/leak-workspace/LeakSourceMapPage.test.tsx",
    "src/features/leak-workspace/LeakFixPage.test.tsx",
    "src/features/leak-workspace/live-detail-client.test.ts",
    "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx",
    "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx",
    "src/features/leak-workspace/LeakExplainPage.test.tsx",
    "src/features/leak-workspace/LeakGcPathPage.test.tsx",
    "src/features/artifact-loader/load-analysis-artifact.test.ts",
    "src/features/artifact-loader/ArtifactLoaderPage.test.tsx",
    "src/features/dashboard/components/LeakTable.test.tsx",
  ],
  [
    "src/features/heap-explorer/heap-explorer-query-client.test.ts",
    "src/features/heap-explorer/HeapObjectInspectorPage.test.tsx",
    "src/features/heap-explorer/HeapExplorerLayout.test.tsx",
    "src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx",
    "src/features/heap-explorer/HeapDominatorPage.test.tsx",
    "src/features/heap-explorer/HeapQueryConsolePage.test.tsx",
    "src/features/dashboard/DashboardPage.test.tsx",
    "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx",
    "src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx",
    "src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx",
    "src/features/heap-explorer/components/ModeRail.test.tsx",
    "src/features/heap-explorer/components/QueryConsolePanel.test.tsx",
  ],
];

for (const batch of testBatches) {
  const result = spawnSync(bunExecutable, ["test", ...batch, "--max-concurrency=1"], {
    stdio: "inherit",
  });

  if (typeof result.status === "number" && result.status !== 0) {
    process.exit(result.status);
  }

  if (result.error) {
    throw result.error;
  }
}
