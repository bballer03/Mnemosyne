/**
 * UI test batch layout and per-batch RSS ceilings for `run-tests.ts`.
 *
 * Ceilings were baselined on Bun 1.2.5 / jsdom 24.1.1 (2026-09-15) with ~50%
 * headroom above observed VmHWM peaks on WSL2. CI uses the same runner pin.
 * Regressions fail the gate before the job hits the GitHub runner SIGTERM.
 */
export type UiTestBatch = {
  /** Stable gate label surfaced in timeout/OOM/RSS failure output. */
  id: string;
  files: readonly string[];
  /** Maximum allowed peak RSS (MiB) for this batch's Bun test process. */
  rssCeilingMiB: number;
  /** Per-batch wall-clock limit; default 120s when omitted. */
  timeoutMs?: number;
};

export const uiTestBatches: readonly UiTestBatch[] = [
  {
    id: "leak-workspace-core",
    rssCeilingMiB: 512,
    files: [
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
  },
  {
    id: "heap-explorer-core",
    rssCeilingMiB: 512,
    files: [
      // Split off from comparison-panels (M14 Slice 14.C): running all of
      // heap-explorer + comparison + artifact-explorer/thread panels in one
      // `bun test` process reliably triggers V8 OOM partway through.
      "src/features/heap-explorer/heap-explorer-query-client.test.ts",
      "src/features/heap-explorer/HeapObjectInspectorPage.test.tsx",
      "src/features/heap-explorer/HeapExplorerLayout.test.tsx",
      "src/features/heap-explorer/components/ExplorerCrossNavActions.test.tsx",
      "src/features/heap-explorer/HeapQueryConsolePage.test.tsx",
      "src/features/dashboard/DashboardPage.test.tsx",
    ],
  },
  {
    id: "comparison-panels",
    rssCeilingMiB: 512,
    files: [
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
  },
  {
    id: "bridge-light",
    rssCeilingMiB: 512,
    files: [
      // Bridge/unit-light files only — keep card mounts out of this process.
      "src/app/TopNav.test.tsx",
      "src/features/assistant/assistant-bridge-client.test.ts",
      "src/features/workflow-landing/workflow-bridge-client.test.ts",
      "src/features/workflow-landing/natural-language-router.test.ts",
      "src/features/investigation/investigation-store.test.ts",
      "src/features/investigation/workspace-actions.test.ts",
      "src/features/investigation/PerspectiveSwitcher.test.tsx",
      "src/app/InvestigationBreadcrumbs.test.tsx",
    ],
  },
  {
    id: "workflow-card",
    rssCeilingMiB: 384,
    files: [
      // WorkflowCard alone: after TopNav/assistant-bridge in one process, CI
      // hung ~67s then SIGTERM (exit 143) with zero card tests printed.
      "src/features/workflow-landing/WorkflowCard.test.tsx",
    ],
  },
  {
    id: "workflow-cards",
    rssCeilingMiB: 384,
    files: [
      "src/features/workflow-landing/TriageSummaryCard.test.tsx",
      "src/features/workflow-landing/WorkflowCards.test.tsx",
    ],
  },
  {
    id: "investigation-assistant",
    // Observed ~438 MiB on Bun 1.2.5/jsdom 24; keep leaner headroom for ~10Gi WSL.
    rssCeilingMiB: 560,
    files: [
      // InvestigationAssistant used to import production `routes` and OOM WSL/CI
      // when batched after TopNav. Fresh process even with assistantRoutes().
      "src/features/assistant/InvestigationAssistantPage.test.tsx",
    ],
  },
  {
    id: "workflow-landing",
    rssCeilingMiB: 512,
    files: [
      // NaturalLanguageInputBar + landing suites after heavy WorkflowCard/TopNav
      // can hang until SIGTERM (~90s). Fresh process keeps the gate reliable.
      "src/features/workflow-landing/NaturalLanguageInputBar.test.tsx",
      "src/features/workflow-landing/RecentHeapsList.test.tsx",
      "src/features/workflow-landing/GuidedLanding.test.tsx",
    ],
  },
  {
    id: "m20-surfaces",
    rssCeilingMiB: 512,
    files: [
      "src/features/policy/PolicyCheckPage.test.tsx",
      "src/features/snapshots/SnapshotManagerPage.test.tsx",
      "src/features/flamegraph/FlamegraphPage.test.tsx",
      "src/host/tauri-bridge.test.ts",
      "src/host/format-host-error.test.ts",
    ],
  },
  {
    id: "artifact-explorer-instances",
    rssCeilingMiB: 512,
    files: [
      "src/features/artifact-explorer/ArtifactExplorerPage.test.tsx",
      "src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx",
    ],
  },
  {
    id: "histogram-explorer",
    // Fresh process: pagination fixtures + jsdom row mounts previously hung/OOM'd
    // lean WSL hosts when combined with other artifact-explorer suites.
    rssCeilingMiB: 384,
    timeoutMs: 90_000,
    files: [
      "src/features/artifact-explorer/components/histogram-hierarchy.test.ts",
      "src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx",
    ],
  },
  {
    id: "dominator-explorer",
    rssCeilingMiB: 512,
    files: [
      "src/features/heap-explorer/HeapDominatorPage.test.tsx",
      "src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx",
    ],
  },
  {
    id: "object-inspector",
    rssCeilingMiB: 512,
    files: ["src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx"],
  },
  {
    id: "operation-status",
    rssCeilingMiB: 384,
    files: [
      "src/host/operation-protocol.test.ts",
      "src/features/investigation/HeapSessionBar.test.tsx",
    ],
  },
  {
    id: "findings-advisory",
    rssCeilingMiB: 512,
    files: [
      "src/features/investigation/finding-adapters.test.ts",
      "src/features/investigation/FindingsAdvisoryPane.test.tsx",
    ],
  },
];

export const defaultBatchTimeoutMs = 120_000;
