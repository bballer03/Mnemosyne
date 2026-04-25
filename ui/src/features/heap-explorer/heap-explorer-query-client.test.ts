import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import {
  getObjectReferrers,
  getObjectReferences,
  isReferencesAvailable,
  isReferrersAvailable,
  runHeapQuery,
} from "./heap-explorer-query-client";

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

  function setHeapExplorerBridge(bridge: NonNullable<Window["__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__"]>) {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = bridge;
  }

  beforeEach(() => {
    clearHeapExplorerBridge();
  });

  afterEach(() => {
    clearHeapExplorerBridge();
  });

  it("returns unavailable when no heap explorer query bridge exists", async () => {
    expect(await runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id" })).toEqual({
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

    expect(await runHeapQuery({ heapPath: "heap.hprof", query: "SELECT object_id, class_name" })).toEqual({
      status: "ready",
      data: {
        columns: ["object_id", "class_name"],
        rows: [["0x2a", "com.example.Cache"]],
      },
    });
  });

  it("getObjectReferences returns unavailable when no bridge exists", async () => {
    expect(await getObjectReferences("0xabc")).toEqual({
      status: "unavailable",
    });
  });

  it("getObjectReferences returns unavailable when the bridge has no getReferences method", async () => {
    setHeapExplorerBridge({
      queryHeap: async () => ({
        columns: [],
        rows: [],
      }),
    });

    expect(await getObjectReferences("0xabc")).toEqual({
      status: "unavailable",
    });
  });

  it("getObjectReferences validates and returns reference entries", async () => {
    setHeapExplorerBridge({
      getReferences: async () => ({
        objectId: "0xabc",
        references: [{ objectId: "0x1", className: "java.lang.String", shallowSize: 32 }],
      }),
    });

    expect(await getObjectReferences("0xabc")).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        references: [
          {
            objectId: "0x1",
            className: "java.lang.String",
            shallowSize: 32,
            displayName: undefined,
          },
        ],
      },
    });
  });

  it("getObjectReferences handles entries with displayName", async () => {
    setHeapExplorerBridge({
      getReferences: async () => ({
        objectId: "0xabc",
        references: [
          {
            objectId: "0x1",
            className: "java.lang.String",
            shallowSize: 32,
            displayName: "MyField",
          },
        ],
      }),
    });

    expect(await getObjectReferences("0xabc")).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        references: [
          {
            objectId: "0x1",
            className: "java.lang.String",
            shallowSize: 32,
            displayName: "MyField",
          },
        ],
      },
    });
  });

  it("getObjectReferences returns an error when the bridge throws", async () => {
    setHeapExplorerBridge({
      getReferences: async () => {
        throw new Error("bridge down");
      },
    });

    expect(await getObjectReferences("0xabc")).toEqual({
      status: "error",
      error: "bridge down",
    });
  });

  it("getObjectReferences rejects malformed payloads", async () => {
    setHeapExplorerBridge({
      getReferences: async () => ({ objectId: 42, references: [] }),
    });

    const result = await getObjectReferences("0xabc");

    expect(result).toMatchObject({
      status: "error",
    });

    if (result.status !== "error") {
      throw new Error("Expected malformed references payload to return an error state.");
    }

    expect(result.error).toContain("expected references.objectId to be a string");
  });

  it("getObjectReferrers returns unavailable when no bridge exists", async () => {
    expect(await getObjectReferrers("0xabc")).toEqual({
      status: "unavailable",
    });
  });

  it("getObjectReferrers validates and returns referrer entries", async () => {
    setHeapExplorerBridge({
      getReferrers: async () => ({
        objectId: "0xabc",
        referrers: [{ objectId: "0x2", className: "com.example.Cache", shallowSize: 128 }],
      }),
    });

    expect(await getObjectReferrers("0xabc")).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        referrers: [
          {
            objectId: "0x2",
            className: "com.example.Cache",
            shallowSize: 128,
            displayName: undefined,
          },
        ],
      },
    });
  });

  it("getObjectReferrers returns an error when the bridge throws", async () => {
    setHeapExplorerBridge({
      getReferrers: async () => {
        throw new Error("bridge down");
      },
    });

    expect(await getObjectReferrers("0xabc")).toEqual({
      status: "error",
      error: "bridge down",
    });
  });

  it("isReferencesAvailable returns false when no bridge exists", () => {
    expect(isReferencesAvailable()).toBeFalse();
  });

  it("isReferencesAvailable returns true when the bridge has getReferences", () => {
    setHeapExplorerBridge({
      getReferences: async () => ({
        objectId: "0xabc",
        references: [],
      }),
    });

    expect(isReferencesAvailable()).toBeTrue();
  });

  it("isReferrersAvailable returns false when no bridge exists", () => {
    expect(isReferrersAvailable()).toBeFalse();
  });
});
