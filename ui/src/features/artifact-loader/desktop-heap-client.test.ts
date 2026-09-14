import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  getDesktopLogPath,
  loadHeapFromSource,
  pickHeapFile,
  runDesktopAnalysis,
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
      runDesktopAnalysis: async () => {
        throw new Error("unused");
      },
    };
    await expect(pickHeapFile()).resolves.toEqual(selected);

    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({ status: "cancelled" }),
      loadHeapFromSource: async () => {
        throw new Error("unused");
      },
      runDesktopAnalysis: async () => {
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
      runDesktopAnalysis: async () => {
        throw new Error("unused");
      },
    };

    const summary = await loadHeapFromSource("src-opaque");
    expect(summary.displayName).toBe("fixture.hprof");
    expect(summary.sourceId).toBe("src-opaque");
    expect(JSON.stringify(summary)).not.toContain("/");
  });

  it("runs desktop analysis by opaque source id and returns sanitized payload", async () => {
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({ status: "cancelled" }),
      loadHeapFromSource: async () => {
        throw new Error("unused");
      },
      runDesktopAnalysis: async (input) => ({
        summary: { heap_path: "fixture.hprof", total_objects: 3 },
        sourceId: input.sourceId,
      }),
    };

    const artifact = await runDesktopAnalysis({ sourceId: "src-opaque", mode: "incident" });
    expect(JSON.stringify(artifact)).toContain("fixture.hprof");
    expect(JSON.stringify(artifact)).not.toMatch(/[/\\]tmp[/\\]/i);
  });

  it("rejects load when the bridge is unavailable", async () => {
    await expect(loadHeapFromSource("src-1")).rejects.toThrow(/unavailable/i);
  });

  it("returns undefined log path without a bridge and forwards when present", async () => {
    await expect(getDesktopLogPath()).resolves.toBeUndefined();

    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      getDesktopLogPath: async () => "/tmp/mnemosyne-logs/desktop.log",
    };
    await expect(getDesktopLogPath()).resolves.toBe("/tmp/mnemosyne-logs/desktop.log");
  });
});
