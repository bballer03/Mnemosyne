import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  isCloseWorkflowAvailable,
  isDescribeWorkflowAvailable,
  isGetWorkflowAvailable,
  isListSnapshotsAvailable,
  isNextStepAvailable,
  isOpenSnapshotAvailable,
  isStartWorkflowAvailable,
  runCloseWorkflow,
  runDescribeWorkflow,
  runGetWorkflow,
  runListSnapshots,
  runNextStep,
  runOpenSnapshot,
  runStartWorkflow,
} from "./workflow-bridge-client";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

const SNAPSHOT_KEY = "a".repeat(64);

function rawSnapshotHydrate(displayName = "fixture.hprof") {
  return {
    snapshot: {
      key: SNAPSHOT_KEY,
      displayName,
      sourceId: "src-1",
      schemaVersion: 1,
      createdAt: "1700000000",
    },
    mode: "deep",
    capabilities: {
      graph: true,
      dominators: true,
      fieldData: false,
      snapshotBacked: true,
    },
    analysis: {
      summary: {
        heap_path: displayName,
        total_objects: 1,
        total_size_bytes: 8,
        classes: [],
        generated_at: "2026-09-15T00:00:00Z",
        header: null,
        total_records: 0,
        record_stats: [],
      },
      leaks: [],
      recommendations: [],
      elapsed: { secs: 0, nanos: 0 },
      graph: { node_count: 1, edge_count: 0, dominators: [] },
      histogram: {
        group_by: "class",
        entries: [],
        total_instances: 1,
        total_shallow_size: 8,
      },
      provenance: [{ kind: "Partial", detail: "snapshot-backed facts" }],
    },
  };
}

describe("workflow-bridge-client availability probes", () => {
  it("report false for every method when no bridge is installed", () => {
    expect(isDescribeWorkflowAvailable()).toBe(false);
    expect(isStartWorkflowAvailable()).toBe(false);
    expect(isNextStepAvailable()).toBe(false);
    expect(isGetWorkflowAvailable()).toBe(false);
    expect(isCloseWorkflowAvailable()).toBe(false);
    expect(isListSnapshotsAvailable()).toBe(false);
    expect(isOpenSnapshotAvailable()).toBe(false);
  });

  it("report true only for methods the bridge actually implements", () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => ({}),
    };

    expect(isStartWorkflowAvailable()).toBe(true);
    expect(isDescribeWorkflowAvailable()).toBe(false);
    expect(isNextStepAvailable()).toBe(false);
    expect(isGetWorkflowAvailable()).toBe(false);
    expect(isCloseWorkflowAvailable()).toBe(false);
    expect(isListSnapshotsAvailable()).toBe(false);
    expect(isOpenSnapshotAvailable()).toBe(false);
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

describe("runOpenSnapshot", () => {
  it("returns unavailable when the bridge is absent", async () => {
    expect(await runOpenSnapshot("abc")).toEqual({ status: "unavailable" });
  });

  it("parses one snapshot workspace hydrate", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      openSnapshot: async (key) => {
        expect(key).toBe(SNAPSHOT_KEY);
        return rawSnapshotHydrate();
      },
    };

    const result = await runOpenSnapshot(SNAPSHOT_KEY);
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }
    expect(result.data).toMatchObject({
      snapshot: {
        key: SNAPSHOT_KEY,
        displayName: "fixture.hprof",
        sourceId: "src-1",
      },
      mode: "deep",
      capabilities: {
        graph: true,
        dominators: true,
        fieldData: false,
        snapshotBacked: true,
      },
      analysis: {
        summary: { heapPath: "fixture.hprof", totalObjects: 1 },
        graph: { nodeCount: 1 },
      },
    });
  });

  it("strips accidental absolute displayName down to basename", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      openSnapshot: async () => rawSnapshotHydrate("/var/tmp/heaps/fixture.hprof"),
    };

    const result = await runOpenSnapshot(SNAPSHOT_KEY);
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }
    expect(result.data.snapshot.displayName).toBe("fixture.hprof");
    expect(result.data.analysis.summary.heapPath).toBe("fixture.hprof");
  });

  it("rejects a hydrate missing analysis facts", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      openSnapshot: async () => ({ ...rawSnapshotHydrate(), analysis: undefined }),
    };

    expect((await runOpenSnapshot(SNAPSHOT_KEY)).status).toBe("error");
  });

  it("surfaces bridge rejections as an error status", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      openSnapshot: async () => {
        throw new Error("snapshot_not_found");
      },
    };

    expect(await runOpenSnapshot("deadbeef")).toEqual({
      status: "error",
      error: "snapshot_not_found",
    });
  });
});

describe("runGetWorkflow / runCloseWorkflow", () => {
  it("returns unavailable when the bridge lacks get/close", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {};
    expect(await runGetWorkflow("wf-1")).toEqual({ status: "unavailable" });
    expect(await runCloseWorkflow("wf-1")).toEqual({ status: "unavailable" });
  });

  it("parses get_workflow WorkflowState with basename-only heap path", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async (workflowId) => {
        expect(workflowId).toBe("wf-9");
        return {
          schema_version: 1,
          workflow_id: "wf-9",
          kind: "CLASSLOADER_LEAK",
          created_at: "1700000000",
          updated_at: "1700000001",
          heap_path: "/var/tmp/heaps/fixture.hprof",
          current_step: "select",
          step_history: [
            {
              step_name: "detect",
              input: null,
              output_summary: { duplicate_class_names: ["com/example/Dup"] },
              timestamp: "1700000000",
            },
          ],
          context: {},
        };
      },
    };

    const result = await runGetWorkflow("wf-9");
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }

    expect(result.data).toEqual({
      workflowId: "wf-9",
      kind: "classloader_leak",
      currentStep: "select",
      heapDisplayName: "fixture.hprof",
      stepHistory: [
        {
          stepName: "detect",
          outputSummary: { duplicate_class_names: ["com/example/Dup"] },
        },
      ],
      stepResult: { duplicate_class_names: ["com/example/Dup"] },
      nextExpectedInput: [],
    });
  });

  it("surfaces workflow_corrupt from get_workflow as an error status", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => {
        throw new Error("workflow_corrupt: failed to load workflow 'wf-bad'");
      },
    };

    expect(await runGetWorkflow("wf-bad")).toEqual({
      status: "error",
      error: "workflow_corrupt: failed to load workflow 'wf-bad'",
    });
  });

  it("parses close_workflow confirmation", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      closeWorkflow: async (workflowId) => {
        expect(workflowId).toBe("wf-9");
        return { workflow_id: "wf-9", closed: true };
      },
    };

    const result = await runCloseWorkflow("wf-9");
    expect(result.status).toBe("ready");
    if (result.status !== "ready") {
      throw new Error("expected ready status");
    }
    expect(result.data).toEqual({ workflowId: "wf-9", closed: true });
  });
});
