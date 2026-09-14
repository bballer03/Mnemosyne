import "../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import { injectHostBridges, isTauriRuntime } from "./tauri-bridge";

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
});
