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
  [
    // TopNav uses a stub route tree (not production `routes`). Other suites
    // in this batch are bridge/unit-light; keep InvestigationAssistant in the
    // next process — mounting even a minimal assistant tree after heavy
    // workflow cards has hung CI historically.
    "src/app/TopNav.test.tsx",
    "src/features/assistant/assistant-bridge-client.test.ts",
    "src/features/workflow-landing/workflow-bridge-client.test.ts",
    "src/features/workflow-landing/natural-language-router.test.ts",
    "src/features/workflow-landing/WorkflowCard.test.tsx",
    "src/features/workflow-landing/TriageSummaryCard.test.tsx",
    "src/features/workflow-landing/WorkflowCards.test.tsx",
  ],
  [
    // InvestigationAssistant used to import production `routes` and OOM WSL/CI
    // when batched after TopNav. Keep it in a fresh process even with the
    // minimal assistantRoutes() fixture.
    "src/features/assistant/InvestigationAssistantPage.test.tsx",
  ],
  [
    // Split from the batch above: on CI (Bun 1.4.x) NaturalLanguageInputBar
    // + landing suites after heavy WorkflowCard/TopNav work can hang until
    // the job is SIGTERM'd (~90s). Fresh process keeps the gate reliable.
    "src/features/workflow-landing/NaturalLanguageInputBar.test.tsx",
    "src/features/workflow-landing/RecentHeapsList.test.tsx",
    "src/features/workflow-landing/GuidedLanding.test.tsx",
  ],
  [
    // M20 workbench surfaces (policies / snapshots / flamegraphs).
    "src/features/policy/PolicyCheckPage.test.tsx",
    "src/features/snapshots/SnapshotManagerPage.test.tsx",
    "src/features/flamegraph/FlamegraphPage.test.tsx",
    "src/host/tauri-bridge.test.ts",
    "src/host/format-host-error.test.ts",
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
