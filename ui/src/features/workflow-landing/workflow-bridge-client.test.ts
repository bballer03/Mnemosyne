import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  isDescribeWorkflowAvailable,
  isListSnapshotsAvailable,
  isNextStepAvailable,
  isStartWorkflowAvailable,
  runDescribeWorkflow,
  runListSnapshots,
  runNextStep,
  runStartWorkflow,
} from "./workflow-bridge-client";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

describe("workflow-bridge-client availability probes", () => {
  it("report false for every method when no bridge is installed", () => {
    expect(isDescribeWorkflowAvailable()).toBe(false);
    expect(isStartWorkflowAvailable()).toBe(false);
    expect(isNextStepAvailable()).toBe(false);
    expect(isListSnapshotsAvailable()).toBe(false);
  });

  it("report true only for methods the bridge actually implements", () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => ({}),
    };

    expect(isStartWorkflowAvailable()).toBe(true);
    expect(isDescribeWorkflowAvailable()).toBe(false);
    expect(isNextStepAvailable()).toBe(false);
    expect(isListSnapshotsAvailable()).toBe(false);
  });
});

describe("runDescribeWorkflow", () => {
  it("returns unavailable when the bridge is absent", async () => {
    const result = await runDescribeWorkflow("triage_memory_leak");
    expect(result).toEqual({ status: "unavailable" });
  });

  it("parses the real MCP describe_workflow shape into camelCase", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      describeWorkflow: async () => ({
        kind: "TRIAGE_MEMORY_LEAK",
        steps: [
          {
            name: "detect",
            description: "Run leak detection.",
            expected_input: [
              { name: "min_severity", type: "string", required: false, description: "Minimum severity." },
            ],
            underlying_primitives: ["detect_leaks"],
          },
          {
            name: "investigate_suspect",
            description: "Investigate the top suspect.",
            expected_input: [],
            underlying_primitives: ["find_all_gc_paths"],
          },
        ],
      }),
    };

    const result = await runDescribeWorkflow("triage_memory_leak");
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }

    expect(result.data.steps).toHaveLength(2);
    expect(result.data.steps[0]).toEqual({
      name: "detect",
      description: "Run leak detection.",
      expectedInput: [
        { name: "min_severity", type: "string", required: false, description: "Minimum severity." },
      ],
      underlyingPrimitives: ["detect_leaks"],
    });
  });

  it("surfaces bridge rejections as an error status", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      describeWorkflow: async () => {
        throw new Error("boom");
      },
    };

    const result = await runDescribeWorkflow("tune_gc");
    expect(result).toEqual({ status: "error", error: "boom" });
  });
});

describe("runStartWorkflow / runNextStep round trip", () => {
  it("parses the workflow_step_response envelope for start_workflow", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async (kind, params) => {
        expect(kind).toBe("triage_memory_leak");
        expect(params).toEqual({ heapPath: "fixture.hprof" });
        return {
          workflow_id: "wf-1",
          current_step: "investigate_suspect",
          step_result: { leaks: [{ id: "leak-1" }] },
          next_expected_input: [{ name: "leak_id", type: "string", required: false, description: "Focus leak." }],
        };
      },
    };

    const result = await runStartWorkflow("triage_memory_leak", { heapPath: "fixture.hprof" });
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }

    expect(result.data).toEqual({
      workflowId: "wf-1",
      currentStep: "investigate_suspect",
      stepResult: { leaks: [{ id: "leak-1" }] },
      nextExpectedInput: [{ name: "leak_id", type: "string", required: false, description: "Focus leak." }],
    });
  });

  it("parses next_step's response and reaches the complete step", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      nextStep: async (workflowId, input) => {
        expect(workflowId).toBe("wf-1");
        expect(input).toEqual({ leak_id: "leak-1" });
        return {
          workflow_id: "wf-1",
          current_step: "complete",
          step_result: { skipped: true },
          next_expected_input: [],
        };
      },
    };

    const result = await runNextStep("wf-1", { leak_id: "leak-1" });
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }

    expect(result.data.currentStep).toBe("complete");
    expect(result.data.stepResult).toEqual({ skipped: true });
  });

  it("returns unavailable for both methods when the bridge lacks them", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {};

    expect(await runStartWorkflow("tune_gc")).toEqual({ status: "unavailable" });
    expect(await runNextStep("wf-1")).toEqual({ status: "unavailable" });
  });
});

describe("runListSnapshots", () => {
  it("returns unavailable when the bridge is absent", async () => {
    expect(await runListSnapshots()).toEqual({ status: "unavailable" });
  });

  it("parses SnapshotManifest[] into camelCase", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "abc123",
          heap_path: "fixture.hprof",
          created_at: "1700000000",
          mnemosyne_version: "0.4.0",
          object_count: 42,
          has_field_data: true,
        },
      ],
    };

    const result = await runListSnapshots();
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }

    expect(result.data).toEqual([
      {
        schemaVersion: 1,
        heapSha256: "abc123",
        heapPath: "fixture.hprof",
        createdAt: "1700000000",
        mnemosyneVersion: "0.4.0",
        objectCount: 42,
        hasFieldData: true,
      },
    ]);
  });

  it("strips absolute heap_path down to basename in React state", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "abc123",
          heap_path: "/var/tmp/heaps/fixture.hprof",
          created_at: "1700000000",
          mnemosyne_version: "0.4.0",
          object_count: 42,
          has_field_data: false,
        },
      ],
    };

    const result = await runListSnapshots();
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }
    expect(result.data[0]?.heapPath).toBe("fixture.hprof");
  });

  it("surfaces a malformed payload as an error status rather than throwing", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [{ heap_path: "fixture.hprof" }],
    };

    const result = await runListSnapshots();
    expect(result.status).toBe("error");
  });
});
