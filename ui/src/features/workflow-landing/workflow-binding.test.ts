import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { useInvestigationStore } from "../investigation/investigation-store";
import {
  advanceWorkspaceWorkflow,
  closeWorkspaceWorkflow,
  recoverWorkspaceWorkflow,
  startWorkspaceWorkflow,
} from "./workflow-binding";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

beforeEach(() => {
  useInvestigationStore.setState({
    workspaceId: "workspace-1",
    revision: 0,
    activeWorkflow: undefined,
    workflowNeedsRecovery: false,
    workspaceRequests: {},
  });
});

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

describe("workspace workflow binding", () => {
  it("binds start and advance responses without a caller-supplied workflow id", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => ({
        workflow_id: "wf-1",
        current_step: "thread_local_review",
        step_result: {},
        next_expected_input: [],
      }),
      nextStep: async (workflowId) => {
        expect(workflowId).toBe("wf-1");
        return {
          workflow_id: "wf-1",
          current_step: "top_retainers",
          step_result: {},
          next_expected_input: [],
        };
      },
    };

    expect((await startWorkspaceWorkflow("tune_gc", { heapPath: "fixture.hprof" })).status).toBe(
      "ready",
    );
    expect(useInvestigationStore.getState().activeWorkflow).toMatchObject({
      workflowId: "wf-1",
      kind: "tune_gc",
      currentStep: "thread_local_review",
    });

    expect((await advanceWorkspaceWorkflow()).status).toBe("ready");
    expect(useInvestigationStore.getState().activeWorkflow?.currentStep).toBe("top_retainers");
  });

  it("rejects late and superseded start responses", async () => {
    const first = deferred<unknown>();
    const second = deferred<unknown>();
    let calls = 0;
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => (++calls === 1 ? first.promise : second.promise),
    };

    const oldRequest = startWorkspaceWorkflow("tune_gc", { heapPath: "fixture.hprof" });
    const currentRequest = startWorkspaceWorkflow("triage_memory_leak", {
      heapPath: "fixture.hprof",
    });
    second.resolve({
      workflow_id: "wf-current",
      current_step: "investigate_suspect",
      step_result: {},
      next_expected_input: [],
    });
    expect((await currentRequest).status).toBe("ready");
    first.resolve({
      workflow_id: "wf-old",
      current_step: "thread_local_review",
      step_result: {},
      next_expected_input: [],
    });
    expect((await oldRequest).status).toBe("stale");
    expect(useInvestigationStore.getState().activeWorkflow?.workflowId).toBe("wf-current");

    const late = deferred<unknown>();
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__.startWorkflow = async () => late.promise;
    const replaced = startWorkspaceWorkflow("tune_gc", { heapPath: "fixture.hprof" });
    useInvestigationStore.getState().bumpRevisionOnArtifactChange();
    late.resolve({
      workflow_id: "wf-late",
      current_step: "thread_local_review",
      step_result: {},
      next_expected_input: [],
    });
    expect((await replaced).status).toBe("stale");
    expect(useInvestigationStore.getState().activeWorkflow).toBeUndefined();
  });

  it("confirms only a matching persisted workflow and strips host path state", async () => {
    useInvestigationStore.setState({
      activeWorkflow: {
        workspaceId: "workspace-1",
        revision: 0,
        workflowId: "wf-restored",
        kind: "tune_gc",
        currentStep: "thread_local_review",
      },
      workflowNeedsRecovery: true,
    });
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => ({
        workflow_id: "wf-restored",
        kind: "TUNE_GC",
        heap_path: "/secret/heaps/fixture.hprof",
        current_step: "top_retainers",
        step_history: [],
      }),
    };

    const result = await recoverWorkspaceWorkflow();

    expect(result.status).toBe("ready");
    expect(JSON.stringify(result)).not.toContain("/secret/heaps");
    expect(useInvestigationStore.getState().activeWorkflow?.currentStep).toBe("top_retainers");
    expect(useInvestigationStore.getState().workflowNeedsRecovery).toBe(false);
  });

  it("detaches on incompatible recovery and closes using the internal id", async () => {
    useInvestigationStore.setState({
      activeWorkflow: {
        workspaceId: "workspace-1",
        revision: 0,
        workflowId: "wf-restored",
        kind: "tune_gc",
        currentStep: "thread_local_review",
      },
      workflowNeedsRecovery: true,
    });
    let closedId: string | undefined;
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => ({
        workflow_id: "wf-other",
        kind: "TUNE_GC",
        heap_path: "fixture.hprof",
        current_step: "top_retainers",
        step_history: [],
      }),
      closeWorkflow: async (workflowId) => {
        closedId = workflowId;
        return { workflow_id: workflowId, closed: true };
      },
    };

    expect((await recoverWorkspaceWorkflow()).status).toBe("incompatible");
    expect(useInvestigationStore.getState().activeWorkflow).toBeUndefined();

    const request = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    useInvestigationStore.getState().bindWorkflow(request, "tune_gc", {
      workflowId: "wf-close",
      currentStep: "top_retainers",
    });
    expect((await closeWorkspaceWorkflow()).status).toBe("ready");
    expect(closedId).toBe("wf-close");
    expect(useInvestigationStore.getState().activeWorkflow).toBeUndefined();
  });
});
