import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";
import { useComparisonStore } from "../features/comparison/comparison-store";
import { useDashboardStore } from "../features/dashboard/dashboard-store";
import { useInvestigationStore } from "../features/investigation/investigation-store";
import { useLeakWorkspaceStore } from "../features/leak-workspace/leak-workspace-store";
import { createWorkspaceId } from "../host/operation-protocol";

const hostBridgeKeys = [
  "__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__",
  "__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__",
  "__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__",
  "__MNEMOSYNE_COMPARISON_BRIDGE__",
  "__MNEMOSYNE_WORKFLOW_BRIDGE__",
  "__MNEMOSYNE_ASSISTANT_BRIDGE__",
] as const;

type HostBridgeKey = (typeof hostBridgeKeys)[number];

/**
 * Clear optional Tauri host bridge globals installed by tests.
 */
export function clearHostBridgeGlobals(): void {
  if (typeof window !== "undefined") {
    for (const key of hostBridgeKeys) {
      delete (window as Window & Partial<Record<HostBridgeKey, unknown>>)[key];
    }
  }

  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
}

/**
 * Reset Zustand stores that leak cross-test state when suites share a process.
 */
export function resetStoreStateForTests(): void {
  useArtifactStore.getState().reset();
  useDashboardStore.getState().reset();
  useComparisonStore.getState().reset();
  useLeakWorkspaceStore.getState().reset();
  resetInvestigationStoreForTests();
}

function resetInvestigationStoreForTests(): void {
  useInvestigationStore.getState().deactivatePersistence();
  useInvestigationStore.setState({
    workspaceId: createWorkspaceId(),
    revision: 0,
    activeOperation: undefined,
    analysisMode: undefined,
    capabilities: undefined,
    objectId: undefined,
    classKey: undefined,
    leakId: undefined,
    originPane: undefined,
    histogramView: {
      searchText: "",
      groupBy: "class",
      sortKey: "retained",
      sortDirection: "desc",
      pageOffset: 0,
    },
    persistenceIdentity: undefined,
    notes: [],
    bookmarks: [],
    findingFacts: [],
    findingStatuses: {},
    activeWorkflow: undefined,
    workflowNeedsRecovery: false,
    workspaceRequests: {},
    lastPersistenceNotice: undefined,
  });
}

/** Full per-test cleanup: stores plus host bridge globals. */
export function resetAllTestState(): void {
  resetStoreStateForTests();
  clearHostBridgeGlobals();
}
