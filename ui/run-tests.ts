import { spawnSync } from "node:child_process";

const bunExecutable = process.execPath;

const testBatches = [
  [
    "src/lib/analysis-types.test.ts",
    "src/lib/diff-types.test.ts",
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
    // Split off from the batch below (M14 Slice 14.C): running all of
    // heap-explorer + comparison + the new artifact-explorer/thread panels
    // in a single `bun test` process reliably triggers a V8 "RangeError:
    // Out of memory" partway through (reproduced independently of this
    // slice's changes -- `ObjectInspectorPanel.test.tsx`, untouched by this
    // slice, passes cleanly in isolation but OOMs when combined with 20
    // other heavy React-Router-rendering suites in one process). Splitting
    // keeps `bun run test` a reliable CI-equivalent gate rather than a flake.
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
  ],
  [
    "src/features/heap-explorer/components/ModeRail.test.tsx",
    "src/features/heap-explorer/components/QueryConsolePanel.test.tsx",
    "src/features/comparison/comparison-bridge-client.test.ts",
    "src/features/comparison/MatchQualityBadge.test.tsx",
    "src/features/comparison/ObjectDeltaTable.test.tsx",
    "src/features/comparison/ComparisonPicker.test.tsx",
    "src/features/comparison/ComparisonPage.test.tsx",
    "src/features/artifact-explorer/components/ReferrerPanel.test.tsx",
    "src/features/artifact-explorer/components/ClassloaderExplorerPanel.test.tsx",
    "src/features/heap-explorer/components/ThreadExplorerPanel.test.tsx",
    "src/features/heap-explorer/HeapThreadsPage.test.tsx",
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
