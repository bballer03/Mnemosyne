import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import {
  getObjectReferrers,
  getObjectReferences,
  inspectObject,
  isInspectObjectAvailable,
  isReferencesAvailable,
  isReferrersAvailable,
  isRegroupHistogramAvailable,
  normalizeHistogramGroupBy,
  regroupHistogram,
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

  it("isInspectObjectAvailable returns false when no bridge exists", () => {
    expect(isInspectObjectAvailable()).toBeFalse();
  });

  it("isInspectObjectAvailable returns true when the bridge has inspectObject", () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({
        object_id: "0xabc",
        class_name: "com.example.Cache",
        shallow_size: 32,
        retained_size: null,
        references_out: [],
        referrers_in: [],
        dominator_parent: null,
        dominator_children: [],
      }),
    });

    expect(isInspectObjectAvailable()).toBeTrue();
  });

  it("inspectObject returns unavailable when no bridge exists", async () => {
    expect(await inspectObject("0xabc")).toEqual({
      status: "unavailable",
    });
  });

  it("inspectObject returns unavailable when the bridge has no inspectObject method", async () => {
    setHeapExplorerBridge({
      queryHeap: async () => ({
        columns: [],
        rows: [],
      }),
    });

    expect(await inspectObject("0xabc")).toEqual({
      status: "unavailable",
    });
  });

  it("inspectObject validates and returns a full object inspection", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({
        object_id: "0xabc",
        class_name: "com.example.Cache",
        shallow_size: 32,
        retained_size: 128,
        references_out: [{ object_id: "0x1", class_name: "java.lang.String" }],
        referrers_in: [{ object_id: "0x2", class_name: "com.example.Owner" }],
        dominator_parent: { object_id: "0x3", class_name: "com.example.Root" },
        dominator_children: [{ object_id: "0x4", class_name: "com.example.Leaf" }],
      }),
    });

    expect(await inspectObject("0xabc")).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        className: "com.example.Cache",
        shallowSize: 32,
        retainedSize: 128,
        fields: undefined,
        referencesOut: [{ objectId: "0x1", className: "java.lang.String" }],
        referrersIn: [{ objectId: "0x2", className: "com.example.Owner" }],
        dominatorParent: { objectId: "0x3", className: "com.example.Root" },
        dominatorChildren: [{ objectId: "0x4", className: "com.example.Leaf" }],
      },
    });
  });

  it("inspectObject handles a null retained_size and dominator_parent", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({
        object_id: "0xabc",
        class_name: "com.example.Cache",
        shallow_size: 32,
        retained_size: null,
        references_out: [],
        referrers_in: [],
        dominator_parent: null,
        dominator_children: [],
      }),
    });

    expect(await inspectObject("0xabc")).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        className: "com.example.Cache",
        shallowSize: 32,
        retainedSize: undefined,
        fields: undefined,
        referencesOut: [],
        referrersIn: [],
        dominatorParent: undefined,
        dominatorChildren: [],
      },
    });
  });

  it("inspectObject passes retainFieldData through to the bridge and parses fields", async () => {
    let receivedRetainFieldData: boolean | undefined;
    setHeapExplorerBridge({
      inspectObject: async (_objectId, retainFieldData) => {
        receivedRetainFieldData = retainFieldData;
        return {
          object_id: "0xabc",
          class_name: "com.example.Cache",
          shallow_size: 32,
          retained_size: null,
          fields: [{ name: "count", type_name: "int", value: "7" }],
          references_out: [],
          referrers_in: [],
          dominator_parent: null,
          dominator_children: [],
        };
      },
    });

    const result = await inspectObject("0xabc", true);

    expect(receivedRetainFieldData).toBeTrue();
    expect(result).toEqual({
      status: "ready",
      data: {
        objectId: "0xabc",
        className: "com.example.Cache",
        shallowSize: 32,
        retainedSize: undefined,
        fields: [{ name: "count", typeName: "int", value: "7" }],
        referencesOut: [],
        referrersIn: [],
        dominatorParent: undefined,
        dominatorChildren: [],
      },
    });
  });

  it("inspectObject returns an error when the bridge throws", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => {
        throw new Error("bridge down");
      },
    });

    expect(await inspectObject("0xabc")).toEqual({
      status: "error",
      error: "bridge down",
    });
  });

  it("inspectObject rejects malformed payloads", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({ object_id: 42 }),
    });

    const result = await inspectObject("0xabc");

    expect(result).toMatchObject({ status: "error" });

    if (result.status !== "error") {
      throw new Error("Expected malformed inspection payload to return an error state.");
    }

    expect(result.error).toContain("expected inspection.references_out to be an array");
  });

  it("normalizeHistogramGroupBy accepts classloader alias", () => {
    expect(normalizeHistogramGroupBy("classloader")).toBe("class_loader");
    expect(normalizeHistogramGroupBy("superclass")).toBe("superclass");
  });

  it("isRegroupHistogramAvailable and regroupHistogram follow optional-bridge semantics", async () => {
    expect(isRegroupHistogramAvailable()).toBeFalse();
    expect(await regroupHistogram("superclass")).toEqual({ status: "unavailable" });

    setHeapExplorerBridge({
      regroupHistogram: async (groupBy) => ({
        group_by: groupBy,
        total_instances: 2,
        total_shallow_size: 16,
        entries: [
          {
            key: "java.lang.Object",
            instance_count: 2,
            shallow_size: 16,
            retained_size: 40,
          },
        ],
      }),
    });

    expect(isRegroupHistogramAvailable()).toBeTrue();
    expect(await regroupHistogram("superclass")).toEqual({
      status: "ready",
      data: {
        groupBy: "superclass",
        totalInstances: 2,
        totalShallowSize: 16,
        entries: [
          {
            key: "java.lang.Object",
            instanceCount: 2,
            shallowSize: 16,
            retainedSize: 40,
          },
        ],
      },
    });
  });
});
