import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  clearRememberedDesktopHeapSource,
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { closeInvestigationWorkspace, openDesktopHeapLean } from "./workspace-actions";

afterEach(() => {
  useArtifactStore.getState().reset();
  clearRememberedDesktopHeapSource();
  delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
});

describe("openDesktopHeapLean", () => {
  it("does not remember source when analysis fails", async () => {
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({
        status: "selected",
        sourceId: "src-fail",
        displayName: "fail.hprof",
      }),
      runDesktopAnalysis: async () => {
        throw new Error("boom");
      },
    };
    const result = await openDesktopHeapLean();
    expect(result.status).toBe("error");
    expect(getRememberedDesktopHeapSource()).toBeUndefined();
  });

  it("remembers source only after successful analysis", async () => {
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({
        status: "selected",
        sourceId: "src-ok",
        displayName: "ok.hprof",
      }),
      runDesktopAnalysis: async () => ({
        summary: {
          heap_path: "ok.hprof",
          total_objects: 1,
          total_size_bytes: 8,
          classes: [],
          generated_at: "2026-09-15T00:00:00Z",
          header: null,
          total_records: 1,
          record_stats: [],
        },
        leaks: [],
        recommendations: [],
        elapsed: { secs: 0, nanos: 0 },
        graph: { node_count: 1, edge_count: 0, dominators: [] },
      }),
    };
    const result = await openDesktopHeapLean();
    expect(result.status).toBe("ready");
    expect(getRememberedDesktopHeapSource()?.sourceId).toBe("src-ok");
  });
});

describe("closeInvestigationWorkspace", () => {
  it("clears artifact and remembered source", async () => {
    rememberDesktopHeapSource("src-x", "x.hprof");
    useArtifactStore.getState().setArtifact("x.hprof", {
      summary: {
        heapPath: "x.hprof",
        totalObjects: 1,
        totalRecords: 1,
        totalBytes: 1,
      },
      leaks: [],
      graph: { nodeCount: 1, edgeCount: 0 },
    } as never);
    let unloaded = false;
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      unloadHeap: async () => {
        unloaded = true;
      },
    };
    await closeInvestigationWorkspace();
    expect(unloaded).toBe(true);
    expect(useArtifactStore.getState().artifact).toBeUndefined();
    expect(getRememberedDesktopHeapSource()).toBeUndefined();
  });
});
