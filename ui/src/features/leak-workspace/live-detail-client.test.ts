import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import {
  explainLeak,
  getLeakWorkspaceBridgeStatus,
  findLeakGcPath,
  normalizeFixResult,
  normalizeSourceMapResult,
  proposeLeakFix,
  resolveLeakSourceMap,
} from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

describe("live detail client", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?: unknown;
    };
  };

  function clearLeakWorkspaceBridge() {
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
    }
  }

  beforeEach(() => {
    clearLeakWorkspaceBridge();
  });

  afterEach(() => {
    clearLeakWorkspaceBridge();
  });

  it("full selection switch clears stale dependent fields and cached subviews", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1",
      projectRoot: "D:/repo",
    });
    useLeakWorkspaceStore.getState().setSubviewState("explain", {
      status: "ready",
      data: { leak_id: "leak-1", summary: "stale" },
    });

    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-2",
      heapPath: "fixture-2.hprof",
    });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-2");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture-2.hprof");
    expect(useLeakWorkspaceStore.getState().objectId).toBeUndefined();
    expect(useLeakWorkspaceStore.getState().projectRoot).toBeUndefined();
    expect(useLeakWorkspaceStore.getState().explain.status).toBe("idle");
  });

  it("incremental object target updates preserve leak identity context", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
    });

    useLeakWorkspaceStore.getState().setSelection({
      objectId: "0x1",
    });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-1");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture.hprof");
    expect(useLeakWorkspaceStore.getState().objectId).toBe("0x1");
  });

  it("explicitly clears objectId within the same selection while preserving leak identity", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1",
    });

    useLeakWorkspaceStore.getState().setSelection({ objectId: undefined });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-1");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture.hprof");
    expect(useLeakWorkspaceStore.getState().objectId).toBeUndefined();
  });

  it("records recent object targets per leak, moves duplicates to the front, and preserves history when clearing", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
    });

    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x2000" });
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    useLeakWorkspaceStore.getState().setSelection({ objectId: undefined });

    expect(useLeakWorkspaceStore.getState().recentObjectTargetsByLeak["leak-1"]).toEqual(["0x1000", "0x2000"]);
  });

  it("refreshes recent object target ordering when the current non-empty objectId is explicitly re-applied", () => {
    useLeakWorkspaceStore.getState().reset();

    useLeakWorkspaceStore.setState({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
      recentObjectTargetsByLeak: {
        "leak-1": ["0x2000", "0x1000"],
      },
    });

    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });

    expect(useLeakWorkspaceStore.getState().recentObjectTargetsByLeak["leak-1"]).toEqual(["0x1000", "0x2000"]);
  });

  it("caps recent object target history at five entries and keeps histories leak scoped", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
    });

    for (const objectId of ["0x1", "0x2", "0x3", "0x4", "0x5", "0x6"]) {
      useLeakWorkspaceStore.getState().setSelection({ objectId });
    }

    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-2",
      heapPath: "fixture-2.hprof",
    });
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0xaa" });

    expect(useLeakWorkspaceStore.getState().recentObjectTargetsByLeak["leak-1"]).toEqual([
      "0x6",
      "0x5",
      "0x4",
      "0x3",
      "0x2",
    ]);
    expect(useLeakWorkspaceStore.getState().recentObjectTargetsByLeak["leak-2"]).toEqual(["0xaa"]);
  });

  it("increments the gc path refresh nonce on explicit refresh requests", () => {
    useLeakWorkspaceStore.getState().reset();

    expect(useLeakWorkspaceStore.getState().gcPathRefreshNonce).toBe(0);

    useLeakWorkspaceStore.getState().requestGcPathRefresh();
    useLeakWorkspaceStore.getState().requestGcPathRefresh();

    expect(useLeakWorkspaceStore.getState().gcPathRefreshNonce).toBe(2);
  });

  it("preserves omitted identity fields on partial identity updates", () => {
    useLeakWorkspaceStore.getState().reset();
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1",
      projectRoot: "D:/repo",
    });

    useLeakWorkspaceStore.getState().setSelection({ leakId: "leak-2" });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-2");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture.hprof");
    expect(useLeakWorkspaceStore.getState().objectId).toBeUndefined();
    expect(useLeakWorkspaceStore.getState().projectRoot).toBeUndefined();

    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1",
      projectRoot: "D:/repo",
    });

    useLeakWorkspaceStore.getState().setSelection({ heapPath: "fixture-2.hprof" });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-1");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture-2.hprof");
    expect(useLeakWorkspaceStore.getState().objectId).toBeUndefined();
    expect(useLeakWorkspaceStore.getState().projectRoot).toBeUndefined();
  });

  it("exposes minimal adapter entry points for all live detail operations", async () => {
    const explain = await explainLeak({ leakId: "leak-1", heapPath: "fixture.hprof" });
    const gcPath = await findLeakGcPath({ leakId: "leak-1", heapPath: "fixture.hprof", objectId: "0x1" });
    const sourceMap = await resolveLeakSourceMap({
      leakId: "leak-1",
      className: "com.example.Cache",
      projectRoot: "D:/repo",
    });
    const fix = await proposeLeakFix({ leakId: "leak-1", heapPath: "fixture.hprof", projectRoot: "D:/repo" });

    expect(explain.status).toBeString();
    expect(gcPath.status).toBeString();
    expect(sourceMap.status).toBeString();
    expect(fix.status).toBeString();
    expect(explain.error).toBe("Local explain bridge is unavailable.");
    expect(gcPath.error).toBe("Local GC path bridge is unavailable.");
    expect(sourceMap.error).toBe("Local source map bridge is unavailable.");
    expect(fix.error).toBe("Local fix bridge is unavailable.");
  });

  it("marks explain, source-map, and fix as unavailable when no host bridge is installed", async () => {
    const explain = await explainLeak({ leakId: "leak-1", heapPath: "fixture.hprof" });
    const sourceMap = await resolveLeakSourceMap({
      leakId: "leak-1",
      className: "com.example.Cache",
      projectRoot: "D:/repo",
    });
    const fix = await proposeLeakFix({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      projectRoot: "D:/repo",
    });

    expect(explain.status).toBe("unavailable");
    expect(sourceMap.status).toBe("unavailable");
    expect(fix.status).toBe("unavailable");
  });

  it("normalizes bridge-backed explain, source-map, and fix payloads", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      capabilities: { provider: "ready" },
      explainLeak: async () => ({
        leak_id: "leak-1",
        summary: "Bridge explanation.",
      }),
      mapToCode: async () => ({
        leak_id: "leak-1",
        locations: [
          {
            file: "D:/repo/src/main/java/com/example/Cache.java",
            line: 42,
            symbol: "com.example.Cache",
            code_snippet: "return cache;",
            git: { commit: "abc123" },
          },
        ],
      }),
      proposeFix: async () => ({
        suggestions: [
          {
            target_file: "D:/repo/src/main/java/com/example/Cache.java",
            description: "Release entries sooner.",
            diff: "@@ -1 +1 @@",
            confidence: 0.8,
            style: "Minimal",
          },
        ],
      }),
    };

    const explain = await explainLeak({ leakId: "leak-1", heapPath: "fixture.hprof" });
    const sourceMap = await resolveLeakSourceMap({
      leakId: "leak-1",
      className: "com.example.Cache",
      projectRoot: "D:/repo",
    });
    const fix = await proposeLeakFix({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      projectRoot: "D:/repo",
    });

    expect(getLeakWorkspaceBridgeStatus()).toEqual({ bridge: "ready", provider: "ready" });
    expect(explain.status).toBe("ready");
    expect(explain.data?.summary).toBe("Bridge explanation.");
    expect(sourceMap.status).toBe("ready");
    expect(sourceMap.data?.locations[0]?.file).toBe("D:/repo/src/main/java/com/example/Cache.java");
    expect(fix.status).toBe("ready");
    expect(fix.data?.suggestions[0]?.target_file).toBe("D:/repo/src/main/java/com/example/Cache.java");
  });

  it("marks gc path as unavailable when no object target is present", async () => {
    const gcPath = await findLeakGcPath({ leakId: "leak-1", heapPath: "fixture.hprof" });

    expect(gcPath.status).toBe("unavailable");
    expect(gcPath.data?.path).toEqual([]);
  });

  it("marks gc path as unavailable when no host bridge is installed", async () => {
    const gcPath = await findLeakGcPath({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });

    expect(gcPath.status).toBe("unavailable");
    expect(gcPath.error).toBe("Local GC path bridge is unavailable.");
  });

  it("normalizes a bridge-backed gc path payload", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => ({
        object_id: "0x0000000000001000",
        path_length: 2,
        path: [
          {
            object_id: "0x0000000000000001",
            class_name: "java.lang.Thread",
            field: "ROOT",
            is_root: true,
          },
          {
            object_id: "0x0000000000001000",
            class_name: "com.example.Cache",
            field: "entries",
            is_root: false,
          },
        ],
        provenance: [],
      }),
    };

    const gcPath = await findLeakGcPath({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });

    expect(gcPath.status).toBe("ready");
    expect(gcPath.data?.object_id).toBe("0x0000000000001000");
    expect(gcPath.data?.path_length).toBe(2);
    expect(gcPath.data?.path[0]).toEqual({
      object_id: "0x0000000000000001",
      class_name: "java.lang.Thread",
      via: "ROOT",
      is_root: true,
    });
  });

  it("marks bridge-backed synthetic/fallback gc paths as fallback", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => ({
        object_id: "0x1000",
        path_length: 2,
        path: [
          {
            object_id: "GC_ROOT_thread",
            class_name: "java.lang.Thread",
            field: "ROOT",
            is_root: true,
          },
          {
            object_id: "0x1000",
            class_name: "com.example.Cache",
            field: "entries",
            is_root: false,
          },
        ],
        provenance: [
          { kind: "SYNTHETIC", detail: "GC path was synthesized from summary-level heap information." },
          { kind: "FALLBACK", detail: "No real GC root chain could be resolved; best-effort fallback path returned." },
        ],
      }),
    };

    const gcPath = await findLeakGcPath({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });

    expect(gcPath.status).toBe("fallback");
    expect(gcPath.data?.provenance?.map((marker) => marker.kind)).toEqual(["SYNTHETIC", "FALLBACK"]);
  });

  it("marks gc path as error when the bridge throws", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => {
        throw new Error("bridge exploded");
      },
    };

    const gcPath = await findLeakGcPath({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });

    expect(gcPath.status).toBe("error");
    expect(gcPath.error).toBe("bridge exploded");
  });

  it("marks gc path as error when the bridge payload is malformed", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => ({
        object_id: "0x1000",
        path_length: 1,
        path: "not-an-array",
      }),
    };

    const gcPath = await findLeakGcPath({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });

    expect(gcPath.status).toBe("error");
    expect(gcPath.error).toBe("Invalid leak workspace bridge payload: gc-path path must be an array.");
  });

  it("treats gc-path-only bridge installs as ready", () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      capabilities: { provider: "ready" },
      findGcPath: async () => ({
        object_id: "0x1000",
        path_length: 0,
        path: [],
        provenance: [],
      }),
    };

    expect(getLeakWorkspaceBridgeStatus()).toEqual({ bridge: "ready", provider: "ready" });
  });

  it("marks unmapped source results as fallback", () => {
    const result = normalizeSourceMapResult({
      leak_id: "leak-1",
      locations: [
        {
          file: ".mnemosyne/unmapped/com/example/LeakHotspot.java",
          line: 1,
          symbol: "Unknown",
          code_snippet: "",
          git: null,
        },
      ],
    });

    expect(result.status).toBe("fallback");
  });

  it("marks heuristic fix results with provenance as fallback", () => {
    const result = normalizeFixResult({
      suggestions: [],
      provenance: [{ kind: "FALLBACK", detail: "heuristic guidance" }],
    });

    expect(result.status).toBe("fallback");
  });

  it("marks fix proposals as unavailable when project root is missing", async () => {
    const result = await proposeLeakFix({ leakId: "leak-1", heapPath: "fixture.hprof" });

    expect(result.status).toBe("unavailable");
    expect(result.error).toBe("Required local context is missing.");
    expect(result.data?.suggestions).toEqual([]);
  });
});
