import "../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import { injectHostBridges, isTauriRuntime, normalizePickHeapFileResult } from "./tauri-bridge";

describe("tauri-bridge", () => {
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
});
