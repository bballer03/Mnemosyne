import "../test/setup";

import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";

import { useInvestigationStore } from "../features/investigation/investigation-store";
import {
  cancelOperation,
  injectHostBridges,
  isTauriRuntime,
  normalizePickHeapFileResult,
} from "./tauri-bridge";

const invokeCalls: Array<{ command: string; args?: Record<string, unknown> }> = [];
let progressListenCalls = 0;
let progressListener: ((event: { payload: unknown }) => void) | undefined;
let invokeOverride:
  | ((command: string, args?: Record<string, unknown>) => Promise<unknown>)
  | undefined;

mock.module("@tauri-apps/api/core", () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokeCalls.push({ command, args });
    if (invokeOverride) {
      return invokeOverride(command, args);
    }
    if (command === "cancel_operation") {
      return { operationId: args?.operationId, accepted: true };
    }
    return { command, args };
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

    resolveQuery({ rows: [{ objectId: "0x1" }] });

    await expect(pending!).rejects.toThrow(/no longer active/i);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelling");
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
