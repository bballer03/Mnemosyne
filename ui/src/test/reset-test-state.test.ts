import "./setup";

import { describe, expect, it } from "bun:test";

import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";
import { useComparisonStore } from "../features/comparison/comparison-store";
import { useDashboardStore } from "../features/dashboard/dashboard-store";
import { useInvestigationStore } from "../features/investigation/investigation-store";
import { useLeakWorkspaceStore } from "../features/leak-workspace/leak-workspace-store";
import {
  clearHostBridgeGlobals,
  resetAllTestState,
  resetStoreStateForTests,
} from "./reset-test-state";

describe("reset-test-state", () => {
  it("clears host bridge globals", () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      describeWorkflow: async () => ({
        status: "ready",
        workflowId: "wf-1",
        kind: "triage_memory_leak",
        currentStep: "start",
        steps: [],
        stepHistory: [],
      }),
    };
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    clearHostBridgeGlobals();

    expect(window.__MNEMOSYNE_WORKFLOW_BRIDGE__).toBeUndefined();
    expect((globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__).toBeUndefined();
  });

  it("resets zustand stores to idle defaults", () => {
    useArtifactStore.getState().setLoadError("stale");
    useDashboardStore.getState().setSearch("cache");
    useComparisonStore.getState().setLiveBeforeKey("before");
    useLeakWorkspaceStore.getState().setSelection({ leakId: "leak-1" });
    useInvestigationStore.getState().setObjectId("0x1", "inspector");
    useInvestigationStore.getState().replaceFindings(
      { workspaceId: useInvestigationStore.getState().workspaceId, revision: 0 },
      "artifact",
      [
        {
          id: "leak:leak-1",
          source: "artifact",
          kind: "leak",
          severity: "high",
          title: "Leak",
          description: "desc",
          target: { kind: "leak", leakId: "leak-1" },
          provenance: [{ kind: "measured" }],
          metrics: { retainedBytes: 1 },
        },
      ],
    );

    resetStoreStateForTests();

    expect(useArtifactStore.getState().loadError).toBeUndefined();
    expect(useDashboardStore.getState().search).toBe("");
    expect(useComparisonStore.getState().liveBeforeKey).toBe("");
    expect(useLeakWorkspaceStore.getState().leakId).toBeUndefined();
    expect(useInvestigationStore.getState().objectId).toBeUndefined();
    expect(useInvestigationStore.getState().findingFacts).toEqual([]);
  });

  it("resetAllTestState clears bridges and stores together", () => {
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      chatSession: async () => ({ status: "ready", messages: [] }),
    };
    useDashboardStore.getState().setSeverity("critical");

    resetAllTestState();

    expect(window.__MNEMOSYNE_ASSISTANT_BRIDGE__).toBeUndefined();
    expect(useDashboardStore.getState().severity).toBe("all");
  });
});
