import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { runHeapQuery } from "./heap-explorer-query-client";

describe("heap explorer query client", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: unknown;
    };
  };

  function clearHeapExplorerBridge() {
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
    }
  }

  beforeEach(() => {
    clearHeapExplorerBridge();
  });

  afterEach(() => {
    clearHeapExplorerBridge();
  });

  it("returns unavailable when no heap explorer query bridge exists", async () => {
    await expect(
      runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id" }),
    ).resolves.toEqual({
      status: "unavailable",
    });
  });

  it("normalizes query rows from the host bridge", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => ({
        columns: ["object_id", "class_name"],
        rows: [["0x2a", "com.example.Cache"]],
      }),
    };

    await expect(
      runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id, class_name" }),
    ).resolves.toEqual({
      status: "ready",
      data: {
        columns: ["object_id", "class_name"],
        rows: [["0x2a", "com.example.Cache"]],
      },
    });
  });
});
