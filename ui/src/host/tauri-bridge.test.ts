import "../test/setup";

import { afterEach, beforeEach, describe, expect, it, mock } from "bun:test";

import { injectHostBridges, isTauriRuntime, normalizePickHeapFileResult } from "./tauri-bridge";

const invokeCalls: Array<{ command: string; args?: Record<string, unknown> }> = [];

mock.module("@tauri-apps/api/core", () => ({
  invoke: async (command: string, args?: Record<string, unknown>) => {
    invokeCalls.push({ command, args });
    return { command, args };
  },
}));

describe("tauri-bridge", () => {
  beforeEach(() => {
    invokeCalls.length = 0;
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
