import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import type { SnapshotWorkspaceHydrate } from "../workflow-landing/workflow-bridge-client";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  clearRememberedDesktopHeapSource,
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useInvestigationStore } from "./investigation-store";
import {
  applyOpenedHeap,
  applySnapshotWorkspaceHydrate,
  closeInvestigationWorkspace,
  openDesktopHeapLean,
  openSnapshotWorkspace,
} from "./workspace-actions";
import {
  WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
  createWorkspacePersistence,
} from "./workspace-persistence";

function buildArtifact(options: {
  objectIds?: string[];
  classKeys?: string[];
  leakIds?: string[];
}): AnalysisArtifact {
  return {
    summary: {
      heapPath: "fixture.hprof",
      totalObjects: options.objectIds?.length ?? 0,
      totalSizeBytes: 0,
      totalRecords: 0,
    },
    leaks: (options.leakIds ?? []).map((id) => ({
      id,
      className: "com.example.Cache",
      leakKind: "cache",
      severity: "high",
      retainedSizeBytes: 100,
      instances: 1,
      description: "fixture",
      provenance: [],
    })),
    recommendations: [],
    elapsedSeconds: 0,
    provenance: [],
    graph: {
      nodeCount: options.objectIds?.length ?? 0,
      edgeCount: 0,
      dominatorCount: options.objectIds?.length ?? 0,
      dominators: (options.objectIds ?? []).map((objectId) => ({
        name: objectId,
        className: "com.example.Cache",
        objectId,
        dominates: 0,
        retainedSize: 100,
        shallowSize: 10,
      })),
    },
    histogram: {
      groupBy: "class",
      entries: (options.classKeys ?? []).map((key) => ({
        key,
        instanceCount: 1,
        shallowSize: 10,
        retainedSize: 100,
      })),
      totalInstances: options.classKeys?.length ?? 0,
      totalShallowSize: (options.classKeys?.length ?? 0) * 10,
    },
  };
}

beforeEach(() => {
  window.sessionStorage.clear();
  useInvestigationStore.setState({
    revision: 0,
    activeOperation: undefined,
    analysisMode: undefined,
    capabilities: undefined,
    objectId: undefined,
    classKey: undefined,
    leakId: undefined,
    originPane: undefined,
    persistenceIdentity: undefined,
    notes: [],
    bookmarks: [],
    lastPersistenceNotice: undefined,
    histogramView: {
      searchText: "",
      groupBy: "class",
      sortKey: "retained",
      sortDirection: "desc",
      pageOffset: 0,
    },
  });
});

afterEach(() => {
  useArtifactStore.getState().reset();
  clearRememberedDesktopHeapSource();
  delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
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

describe("workspace persistence lifecycle", () => {
  it("restores only selections present in the reopened artifact revision", () => {
    const artifactA = buildArtifact({
      objectIds: ["object-a"],
      classKeys: ["class-a"],
      leakIds: ["leak-removed"],
    });
    const artifactB = buildArtifact({
      objectIds: ["object-b"],
      classKeys: ["class-b"],
      leakIds: [],
    });
    const reopenedArtifactA = buildArtifact({
      objectIds: ["object-a"],
      classKeys: ["class-a"],
      leakIds: [],
    });

    applyOpenedHeap("a.hprof", artifactA, "source-a");
    useInvestigationStore.getState().setObjectId("object-a", "inspector");
    useInvestigationStore.getState().setClassKey("class-a", "histogram");
    useInvestigationStore.getState().setLeakId("leak-removed", "leak");
    useInvestigationStore.getState().setHistogramView({ searchText: "cache" });
    useInvestigationStore.getState().setHistogramView({ pageOffset: 100 });

    applyOpenedHeap("b.hprof", artifactB, "source-b");
    applyOpenedHeap("a.hprof", reopenedArtifactA, "source-a");

    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 3,
      persistenceIdentity: { kind: "workspace", key: "source-a" },
      objectId: "object-a",
      classKey: "class-a",
      leakId: undefined,
      originPane: undefined,
      histogramView: {
        searchText: "cache",
        pageOffset: 100,
      },
    });
    expect(useInvestigationStore.getState().lastPersistenceNotice).toContain(
      "leak-removed",
    );
  });

  it("keeps browser-only artifact imports out of persistence", () => {
    applyOpenedHeap("browser.json", buildArtifact({ objectIds: ["object-a"] }));
    useInvestigationStore.getState().setObjectId("object-a", "inspector");

    expect(useInvestigationStore.getState().persistenceIdentity).toBeUndefined();
    expect(window.sessionStorage.length).toBe(0);
  });
});

describe("snapshot workspace hydrate", () => {
  it("commits facts, mode, capabilities, and compatible selection together", () => {
    const artifact = buildArtifact({
      objectIds: ["object-snapshot"],
      classKeys: ["class-snapshot"],
    });
    const hydrate: SnapshotWorkspaceHydrate = {
      snapshot: {
        key: "snapshot-key",
        displayName: "snapshot.hprof",
        sourceId: "snapshot-source",
        schemaVersion: 1,
        createdAt: "1700000000",
      },
      mode: "deep",
      capabilities: {
        graph: true,
        dominators: true,
        fieldData: false,
        snapshotBacked: true,
      },
      analysis: artifact,
    };
    createWorkspacePersistence().save({
      schemaVersion: WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
      identity: { kind: "snapshot", key: "snapshot-key" },
      revision: 11,
      layout: { activePane: "inspector" },
      filters: {
        histogram: {
          searchText: "snapshot",
          groupBy: "class",
          sortKey: "retained",
          sortDirection: "desc",
          pageOffset: 0,
        },
      },
      selection: {
        revision: 11,
        objectId: "object-snapshot",
        classKey: "class-snapshot",
      },
      notes: [],
      bookmarks: [],
    });

    applySnapshotWorkspaceHydrate(hydrate);

    expect(useArtifactStore.getState()).toMatchObject({
      artifactName: "snapshot.hprof",
      artifact,
    });
    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 1,
      analysisMode: "deep",
      capabilities: hydrate.capabilities,
      persistenceIdentity: { kind: "snapshot", key: "snapshot-key" },
      objectId: "object-snapshot",
      classKey: "class-snapshot",
    });
    expect(getRememberedDesktopHeapSource()).toMatchObject({
      sourceId: "snapshot-source",
      displayName: "snapshot.hprof",
    });
  });

  it("leaves the prior workspace intact when snapshot open fails", async () => {
    const artifact = buildArtifact({ objectIds: ["object-prior"] });
    applyOpenedHeap("prior.hprof", artifact, "prior-source");
    rememberDesktopHeapSource("prior-source", "prior.hprof");
    useInvestigationStore.getState().setObjectId("object-prior", "inspector");
    const revision = useInvestigationStore.getState().revision;
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      openSnapshot: async () => {
        throw new Error("snapshot unavailable");
      },
    };

    const result = await openSnapshotWorkspace("snapshot-key");

    expect(result.status).toBe("error");
    expect(useArtifactStore.getState()).toMatchObject({
      artifactName: "prior.hprof",
      artifact,
    });
    expect(useInvestigationStore.getState()).toMatchObject({
      revision,
      objectId: "object-prior",
      persistenceIdentity: { kind: "workspace", key: "prior-source" },
    });
    expect(getRememberedDesktopHeapSource()).toMatchObject({
      sourceId: "prior-source",
      displayName: "prior.hprof",
    });
  });
});
