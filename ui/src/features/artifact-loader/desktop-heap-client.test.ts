import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  loadHeapFromSource,
  pickHeapFile,
  type PickHeapFileResult,
} from "./desktop-heap-client";

describe("desktop-heap-client", () => {
  afterEach(() => {
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
  });

  it("returns unavailable when the desktop bridge is missing", async () => {
    await expect(pickHeapFile()).resolves.toEqual({ status: "unavailable" });
  });

  it("forwards selected, cancelled, and invoke outcomes from the bridge", async () => {
    const selected: PickHeapFileResult = {
      status: "selected",
      sourceId: "src-1",
      displayName: "fixture.hprof",
    };

    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => selected,
      loadHeapFromSource: async () => {
        throw new Error("unused");
      },
    };
    await expect(pickHeapFile()).resolves.toEqual(selected);

    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({ status: "cancelled" }),
      loadHeapFromSource: async () => {
        throw new Error("unused");
      },
    };
    await expect(pickHeapFile()).resolves.toEqual({ status: "cancelled" });
  });

  it("loads a heap by opaque source id without exposing a path", async () => {
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({ status: "cancelled" }),
      loadHeapFromSource: async (sourceId) => ({
        displayName: "fixture.hprof",
        sourceId,
        objectCount: 10,
        classCount: 2,
        gcRootCount: 1,
      }),
    };

    const summary = await loadHeapFromSource("src-opaque");
    expect(summary.displayName).toBe("fixture.hprof");
    expect(summary.sourceId).toBe("src-opaque");
    expect(JSON.stringify(summary)).not.toContain("/");
  });

  it("rejects load when the bridge is unavailable", async () => {
    await expect(loadHeapFromSource("src-1")).rejects.toThrow(/unavailable/i);
  });
});
