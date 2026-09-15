import "../test/setup";

import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";

import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../features/investigation/investigation-store";
import { openSnapshotWorkspace } from "../features/investigation/workspace-actions";
import {
  cancelOperation,
  injectHostBridges,
  isTauriRuntime,
  normalizePickHeapFileResult,
} from "./tauri-bridge";
import type { OperationContext } from "./operation-protocol";

const invokeCalls: Array<{ command: string; args?: Record<string, unknown> }> = [];
let progressListenCalls = 0;
let progressListener: ((event: { payload: unknown }) => void) | undefined;
let invokeOverride:
  | ((command: string, args?: Record<string, unknown>) => Promise<unknown>)
  | undefined;

function operationContextFromArgs(args?: Record<string, unknown>): OperationContext | undefined {
  const input =
    typeof args?.input === "object" && args.input !== null
      ? (args.input as Record<string, unknown>)
      : undefined;
  return (input?.context ?? args?.context) as OperationContext | undefined;
}

mock.module("@tauri-apps/api/core", () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokeCalls.push({ command, args });
    if (invokeOverride) {
      return invokeOverride(command, args);
    }
    if (command === "cancel_operation") {
      return { operationId: args?.operationId, accepted: true };
    }
    const context = operationContextFromArgs(args);
    return context ? { ...context, data: { command, args } } : { command, args };
  },
}));

mock.module("@tauri-apps/api/event", () => ({
  listen: async (
    eventName: string,
    listener: (event: { payload: unknown }) => void,
  ) => {
    expect(eventName).toBe("mnemosyne://operation-progress");
    progressListenCalls += 1;
    progressListener = listener;
    return () => {};
  },
}));

describe("tauri-bridge", () => {
  beforeEach(() => {
    invokeCalls.length = 0;
    invokeOverride = undefined;
    useInvestigationStore.setState({
      workspaceId: "workspace-1",
      revision: 4,
      activeOperation: undefined,
    });
  });

  afterEach(() => {
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
    delete window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
    delete window.__MNEMOSYNE_COMPARISON_BRIDGE__;
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
    delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
    useArtifactStore.getState().reset();
  });

  it("treats missing __TAURI_INTERNALS__ as non-Tauri", () => {
    expect(isTauriRuntime()).toBe(false);
  });

  it("no-ops inject outside Tauri and leaves bridges unset", async () => {
    await expect(injectHostBridges()).resolves.toBe(false);
    expect(window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__).toBeUndefined();
  });

  it("normalizes snake_case pick payloads from older hosts", () => {
    expect(
      normalizePickHeapFileResult({
        status: "selected",
        source_id: "src-opaque",
        display_name: "fixture.hprof",
      }),
    ).toEqual({
      status: "selected",
      sourceId: "src-opaque",
      displayName: "fixture.hprof",
    });
  });

  it("rejects incomplete selected payloads", () => {
    expect(() =>
      normalizePickHeapFileResult({ status: "selected", source_id: "src-only" }),
    ).toThrow(/incomplete selection/i);
  });

  it("subscribes once and accepts only correlated operation progress", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    await expect(injectHostBridges()).resolves.toBe(true);
    await expect(injectHostBridges()).resolves.toBe(true);
    expect(progressListenCalls).toBe(1);

    const stale = useInvestigationStore.getState().beginOperation("analyze");
    const current = useInvestigationStore.getState().beginOperation("query");

    progressListener?.({
      payload: {
        context: stale,
        kind: "analyze",
        phase: "parsing",
        completed: 20,
        total: 100,
        unit: "records",
        indeterminate: false,
        elapsedMs: 700,
      },
    });
    expect(useInvestigationStore.getState().activeOperation).toMatchObject({
      ...current,
      kind: "query",
      status: "accepted",
    });

    progressListener?.({
      payload: {
        context: current,
        kind: "query",
        phase: "analyzing",
        completed: null,
        total: null,
        unit: null,
        indeterminate: true,
        elapsedMs: 900,
      },
    });
    expect(useInvestigationStore.getState().activeOperation).toMatchObject({
      ...current,
      kind: "query",
      status: "analyzing",
      completed: undefined,
      total: undefined,
      unit: undefined,
      indeterminate: true,
      elapsedMs: 900,
    });
  });

  it("adds operation context to long native calls", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    await expect(injectHostBridges()).resolves.toBe(true);

    await window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.queryHeap?.({
      heapPath: "fixture.hprof",
      query: "SELECT *",
    });

    expect(invokeCalls).toContainEqual({
      command: "query_heap",
      args: {
        input: {
          heapPath: "fixture.hprof",
          query: "SELECT *",
          context: {
            workspaceId: "workspace-1",
            revision: 4,
            operationId: expect.any(String),
          },
        },
      },
    });
  });

  it("sends the active operation id to cancel_operation", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    await expect(cancelOperation("operation-7")).resolves.toEqual({
      operationId: "operation-7",
      accepted: true,
    });
    expect(invokeCalls).toContainEqual({
      command: "cancel_operation",
      args: { operationId: "operation-7" },
    });
  });

  it("does not return a late success after cancellation starts", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    let resolveQuery!: (value: unknown) => void;
    const queryResult = new Promise<unknown>((resolve) => {
      resolveQuery = resolve;
    });
    invokeOverride = async (command) =>
      command === "query_heap" ? queryResult : { operationId: "unused", accepted: false };
    await expect(injectHostBridges()).resolves.toBe(true);

    const pending = window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.queryHeap?.({
      heapPath: "fixture.hprof",
      query: "SELECT *",
    });
    const operation = useInvestigationStore.getState().activeOperation;
    expect(operation).toBeDefined();
    useInvestigationStore.getState().requestOperationCancellation(operation!);

    resolveQuery({
      ...operation!,
      data: { rows: [{ objectId: "0x1" }] },
    });

    await expect(pending!).rejects.toThrow(/no longer active/i);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelling");
  });

  const raceCases: Array<{
    name: string;
    command: string;
    kind:
      | "open"
      | "analyze"
      | "query"
      | "diff"
      | "snapshot"
      | "flamegraph"
      | "gc-path"
      | "inspect";
    call: () => Promise<unknown> | undefined;
    latestData: unknown;
  }> = [
    {
      name: "open",
      command: "load_heap_from_source",
      kind: "open",
      call: () => window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__?.loadHeapFromSource?.("src-1"),
      latestData: {
        displayName: "latest.hprof",
        sourceId: "src-1",
        objectCount: 2,
        classCount: 1,
        gcRootCount: 1,
      },
    },
    {
      name: "analyze",
      command: "run_desktop_analysis",
      kind: "analyze",
      call: () =>
        window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__?.runDesktopAnalysis?.({
          sourceId: "src-1",
          mode: "incident",
        }),
      latestData: { generation: "latest-analysis" },
    },
    {
      name: "query",
      command: "query_heap",
      kind: "query",
      call: () =>
        window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.queryHeap?.({
          heapPath: "fixture.hprof",
          query: "SELECT *",
        }),
      latestData: { columns: ["object_id"], rows: [["0x2"]] },
    },
    {
      name: "diff",
      command: "diff_objects",
      kind: "diff",
      call: () =>
        window.__MNEMOSYNE_COMPARISON_BRIDGE__?.diffObjects?.({
          beforeKey: "before",
          afterKey: "after",
          strategy: "ClassDominator",
          topN: 50,
          crossReferenceLeaks: false,
        }),
      latestData: { generation: "latest-diff" },
    },
    {
      name: "snapshot",
      command: "save_snapshot",
      kind: "snapshot",
      call: () => window.__MNEMOSYNE_WORKFLOW_BRIDGE__?.saveSnapshot?.("src-1", false),
      latestData: { heap_sha256: "latest-snapshot" },
    },
    {
      name: "snapshot reopen",
      command: "open_snapshot",
      kind: "snapshot",
      call: () => window.__MNEMOSYNE_WORKFLOW_BRIDGE__?.openSnapshot?.("snapshot-key"),
      latestData: { generation: "latest-snapshot-hydrate" },
    },
    {
      name: "flamegraph",
      command: "generate_desktop_flamegraph",
      kind: "flamegraph",
      call: () =>
        window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__?.generateFlamegraph?.({
          sourceId: "src-1",
          format: "svg",
        }),
      latestData: { format: "svg", content: "<svg />", byteLength: 7 },
    },
    {
      name: "GC paths",
      command: "find_all_gc_paths",
      kind: "gc-path",
      call: () => window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?.findAllGcPaths?.("0x2", 5),
      latestData: {
        object_id: "0x2",
        path_length: 0,
        path: [],
        all_paths: [],
        truncated: false,
      },
    },
    {
      name: "field-data inspect",
      command: "inspect_object",
      kind: "inspect",
      call: () => window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.inspectObject?.("0x2", true),
      latestData: {
        object_id: "0x2",
        class_name: "example.Latest",
        shallow_size: 16,
        retained_size: 16,
        fields: [],
        references_out: [],
        referrers_in: [],
        dominator_parent: null,
        dominator_children: [],
      },
    },
  ];

  for (const raceCase of raceCases) {
    it(`prevents generation N ${raceCase.name} from reaching generation N+1`, async () => {
      (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
      const contexts: OperationContext[] = [];
      const deferreds = Array.from({ length: 2 }, () => {
        let resolve!: (value: unknown) => void;
        const promise = new Promise<unknown>((resolvePromise) => {
          resolve = resolvePromise;
        });
        return { promise, resolve };
      });
      invokeOverride = async (command, args) => {
        if (command !== raceCase.command) {
          return { command, args };
        }
        const context = operationContextFromArgs(args);
        if (!context) {
          throw new Error(`Missing operation context for ${command}`);
        }
        contexts.push(context);
        return deferreds[contexts.length - 1]!.promise;
      };
      await expect(injectHostBridges()).resolves.toBe(true);

      const generationN = raceCase.call();
      expect(generationN).toBeDefined();
      expect(contexts).toHaveLength(1);

      useInvestigationStore.getState().bumpRevisionOnArtifactChange();
      const generationNPlusOne = raceCase.call();
      expect(generationNPlusOne).toBeDefined();
      expect(contexts).toHaveLength(2);
      expect(useInvestigationStore.getState().activeOperation).toMatchObject({
        ...contexts[1],
        kind: raceCase.kind,
      });

      deferreds[1]!.resolve({ ...contexts[1]!, data: raceCase.latestData });
      await expect(generationNPlusOne!).resolves.toEqual(raceCase.latestData);

      deferreds[0]!.resolve({ ...contexts[0]!, data: { generation: "stale" } });
      await expect(generationN!).rejects.toThrow(/no longer active/i);
      expect(useInvestigationStore.getState().revision).toBe(5);
    });
  }

  it("leaves prior facts and selection intact when a snapshot hydrate is stale", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    let resolveOpen!: (value: unknown) => void;
    const deferredOpen = new Promise<unknown>((resolve) => {
      resolveOpen = resolve;
    });
    let context: OperationContext | undefined;
    invokeOverride = async (command, args) => {
      if (command !== "open_snapshot") {
        return { command, args };
      }
      context = operationContextFromArgs(args);
      return deferredOpen;
    };
    useArtifactStore.getState().setArtifact("prior.hprof", {
      summary: {
        heapPath: "prior.hprof",
        totalObjects: 1,
        totalSizeBytes: 8,
        totalRecords: 1,
      },
      leaks: [],
      recommendations: [],
      elapsedSeconds: 0,
      graph: { nodeCount: 1, edgeCount: 0, dominatorCount: 0, dominators: [] },
      provenance: [],
    });
    useInvestigationStore.setState({ objectId: "object-prior" });
    await expect(injectHostBridges()).resolves.toBe(true);

    const pending = openSnapshotWorkspace("snapshot-key");
    expect(context).toBeDefined();
    useInvestigationStore.setState({ revision: 5, activeOperation: undefined });
    resolveOpen({ ...context!, data: { generation: "stale-hydrate" } });

    await expect(pending).resolves.toMatchObject({ status: "error" });
    expect(useArtifactStore.getState()).toMatchObject({
      artifactName: "prior.hprof",
      artifact: { summary: { heapPath: "prior.hprof" } },
    });
    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 5,
      objectId: "object-prior",
    });
  });

  it("rejects a response envelope whose identity differs from the active request", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    invokeOverride = async (command, args) => {
      const context = operationContextFromArgs(args);
      if (command !== "query_heap" || !context) {
        return { command, args };
      }
      return {
        ...context,
        operationId: `${context.operationId}-wrong`,
        data: { columns: [], rows: [] },
      };
    };
    await expect(injectHostBridges()).resolves.toBe(true);

    await expect(
      window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.queryHeap?.({
        heapPath: "fixture.hprof",
        query: "SELECT *",
      }),
    ).rejects.toThrow(/identity/i);
  });

  it("wires bounded class-instance arguments to the native command", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    await expect(injectHostBridges()).resolves.toBe(true);

    await window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.listClassInstances?.(
      "com.example.BigCache",
      25,
      200,
    );

    expect(invokeCalls).toContainEqual({
      command: "list_class_instances",
      args: {
        classKey: "com.example.BigCache",
        offset: 25,
        limit: 200,
      },
    });
  });

  it("wires lazy dominator-child arguments to the native command", async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    await expect(injectHostBridges()).resolves.toBe(true);

    await window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?.getDominatorChildren?.(
      "0x00001000",
      25,
      100,
      1_048_576,
    );

    expect(invokeCalls).toContainEqual({
      command: "get_dominator_children",
      args: {
        parentObjectId: "0x00001000",
        offset: 25,
        limit: 100,
        minRetainedBytes: 1_048_576,
      },
    });
  });
});
