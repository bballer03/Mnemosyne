import "../../test/setup";

import { beforeEach, describe, expect, it } from "bun:test";

import { useInvestigationStore } from "./investigation-store";
import {
  WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
  createWorkspacePersistence,
  type PersistedWorkspaceV1,
  type WorkspacePersistenceIdentity,
} from "./workspace-persistence";

const persistenceIdentity: WorkspacePersistenceIdentity = {
  kind: "workspace",
  key: "source-a",
};

function persistedWorkspace(
  overrides: Partial<PersistedWorkspaceV1> = {},
): PersistedWorkspaceV1 {
  return {
    schemaVersion: WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
    identity: persistenceIdentity,
    revision: 2,
    layout: { activePane: "inspector" },
    filters: {
      histogram: {
        searchText: "cache",
        groupBy: "class",
        sortKey: "retained",
        sortDirection: "desc",
        pageOffset: 100,
      },
    },
    selection: {
      revision: 2,
      objectId: "object-current",
      classKey: "class-current",
      leakId: "leak-stale",
    },
    notes: [],
    bookmarks: [],
    ...overrides,
  };
}

describe("useInvestigationStore", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    useInvestigationStore.setState({
      workspaceId: "workspace-1",
      revision: 0,
      activeOperation: undefined,
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

  it("accepts, applies, and finishes a matching operation response", () => {
    const context = useInvestigationStore.getState().beginOperation("query");
    let applied = false;

    expect(context).toMatchObject({
      workspaceId: "workspace-1",
      revision: 0,
      operationId: expect.any(String),
    });
    expect(useInvestigationStore.getState().activeOperation).toMatchObject({
      ...context,
      kind: "query",
      status: "accepted",
    });
    expect(useInvestigationStore.getState().acceptOperationResult(context)).toBe(true);
    expect(
      useInvestigationStore.getState().applyOperationResult(context, () => {
        applied = true;
      }),
    ).toBe(true);
    expect(applied).toBe(true);
    expect(useInvestigationStore.getState().finishOperation(context, "complete")).toBe(true);
    expect(useInvestigationStore.getState().activeOperation).toBeUndefined();
  });

  it("rejects a response from another workspace", () => {
    const context = useInvestigationStore.getState().beginOperation("analyze");
    const wrongWorkspace = { ...context, workspaceId: "workspace-2" };
    let applied = false;

    expect(useInvestigationStore.getState().acceptOperationResult(wrongWorkspace)).toBe(false);
    expect(
      useInvestigationStore.getState().applyOperationResult(wrongWorkspace, () => {
        applied = true;
      }),
    ).toBe(false);
    expect(applied).toBe(false);
  });

  it("rejects a response from an old revision", () => {
    useInvestigationStore.setState({ revision: 2 });
    const context = useInvestigationStore.getState().beginOperation("inspect");

    expect(
      useInvestigationStore
        .getState()
        .acceptOperationResult({ ...context, revision: context.revision - 1 }),
    ).toBe(false);
  });

  it("rejects an operation superseded by a newer operation id", () => {
    const superseded = useInvestigationStore.getState().beginOperation("query");
    const current = useInvestigationStore.getState().beginOperation("diff");

    expect(useInvestigationStore.getState().acceptOperationResult(superseded)).toBe(false);
    expect(useInvestigationStore.getState().acceptOperationResult(current)).toBe(true);
  });

  it("does not finish the current operation when a stale operation completes", () => {
    const stale = useInvestigationStore.getState().beginOperation("snapshot");
    const current = useInvestigationStore.getState().beginOperation("flamegraph");

    expect(useInvestigationStore.getState().finishOperation(stale, "complete")).toBe(false);
    expect(useInvestigationStore.getState().activeOperation).toMatchObject({
      ...current,
      kind: "flamegraph",
      status: "accepted",
    });
  });

  it("rejects late progress and success after cancellation starts", () => {
    const context = useInvestigationStore.getState().beginOperation("analyze");
    useInvestigationStore.getState().updateOperationProgress({
      context,
      kind: "analyze",
      phase: "parsing",
      completed: 40,
      total: 100,
      unit: "records",
      indeterminate: false,
      elapsedMs: 500,
    });

    expect(useInvestigationStore.getState().requestOperationCancellation(context)).toBe(true);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelling");

    let applied = false;
    expect(
      useInvestigationStore.getState().applyOperationResult(context, () => {
        applied = true;
      }),
    ).toBe(false);
    expect(applied).toBe(false);
    expect(
      useInvestigationStore.getState().updateOperationProgress({
        context,
        kind: "analyze",
        phase: "complete",
        completed: 100,
        total: 100,
        unit: "records",
        indeterminate: false,
        elapsedMs: 800,
      }),
    ).toBe(false);
    expect(useInvestigationStore.getState().finishOperation(context, "complete")).toBe(false);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelling");
  });

  it("restores the in-flight phase when cancellation is rejected", () => {
    const context = useInvestigationStore.getState().beginOperation("query");
    useInvestigationStore.getState().updateOperationProgress({
      context,
      kind: "query",
      phase: "analyzing",
      indeterminate: true,
      elapsedMs: 700,
    });

    expect(useInvestigationStore.getState().requestOperationCancellation(context)).toBe(true);
    expect(useInvestigationStore.getState().rejectOperationCancellation(context)).toBe(true);
    expect(useInvestigationStore.getState().activeOperation).toMatchObject({
      ...context,
      kind: "query",
      status: "analyzing",
      indeterminate: true,
      elapsedMs: 700,
    });
  });

  it("shows cancelled only after a correlated terminal acknowledgement", () => {
    const context = useInvestigationStore.getState().beginOperation("gc-path");

    useInvestigationStore.getState().requestOperationCancellation(context);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelling");

    expect(
      useInvestigationStore.getState().updateOperationProgress({
        context,
        kind: "gc-path",
        phase: "cancelled",
        indeterminate: true,
        elapsedMs: 900,
      }),
    ).toBe(true);
    expect(useInvestigationStore.getState().activeOperation?.status).toBe("cancelled");
  });

  it("invalidates every outstanding operation when the revision changes", () => {
    const context = useInvestigationStore.getState().beginOperation("open");

    useInvestigationStore.getState().bumpRevisionOnArtifactChange();

    expect(useInvestigationStore.getState()).toMatchObject({
      workspaceId: "workspace-1",
      revision: 1,
      activeOperation: undefined,
    });
    expect(useInvestigationStore.getState().acceptOperationResult(context)).toBe(false);
    expect(useInvestigationStore.getState().finishOperation(context, "failed")).toBe(false);
  });

  it("stores a stable object id and its origin pane", () => {
    useInvestigationStore.getState().setObjectId("0xdeadbeef", "dominators");

    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 0,
      objectId: "0xdeadbeef",
      originPane: "dominators",
    });
  });

  it("updates histogram controls and resets paging when filters or sorting change", () => {
    useInvestigationStore.getState().setHistogramView({ pageOffset: 200 });

    expect(useInvestigationStore.getState().histogramView.pageOffset).toBe(200);

    useInvestigationStore.getState().setHistogramView({ searchText: "cache" });

    expect(useInvestigationStore.getState().histogramView).toMatchObject({
      searchText: "cache",
      pageOffset: 0,
    });

    useInvestigationStore.getState().setHistogramView({ pageOffset: 100 });
    useInvestigationStore.getState().setHistogramView({ groupBy: "package" });

    expect(useInvestigationStore.getState().histogramView).toMatchObject({
      groupBy: "package",
      pageOffset: 0,
    });

    useInvestigationStore.getState().setHistogramView({ pageOffset: 100 });
    useInvestigationStore.getState().setHistogramView({
      sortKey: "instances",
      sortDirection: "asc",
    });

    expect(useInvestigationStore.getState().histogramView).toEqual({
      searchText: "cache",
      groupBy: "package",
      sortKey: "instances",
      sortDirection: "asc",
      pageOffset: 0,
    });
  });

  it("preserves histogram controls when selecting an object", () => {
    useInvestigationStore.getState().setHistogramView({
      searchText: "session",
      groupBy: "superclass",
      sortKey: "shallow",
      sortDirection: "asc",
    });
    useInvestigationStore.getState().setHistogramView({ pageOffset: 300 });

    useInvestigationStore.getState().setObjectId("0xdeadbeef", "histogram");

    expect(useInvestigationStore.getState()).toMatchObject({
      objectId: "0xdeadbeef",
      originPane: "histogram",
      histogramView: {
        searchText: "session",
        groupBy: "superclass",
        sortKey: "shallow",
        sortDirection: "asc",
        pageOffset: 300,
      },
    });
  });

  it("bumps revision and resets heap-bound selection and histogram controls", () => {
    useInvestigationStore.getState().setObjectId("0xdeadbeef", "dominators");
    useInvestigationStore.getState().setClassKey("com.example.Cache", "histogram");
    useInvestigationStore.getState().setLeakId("leak-1", "leak");
    useInvestigationStore.getState().setHistogramView({
      searchText: "cache",
      groupBy: "class_loader",
      sortKey: "class",
      sortDirection: "asc",
    });
    useInvestigationStore.getState().setHistogramView({ pageOffset: 100 });

    useInvestigationStore.getState().bumpRevisionOnArtifactChange();

    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 1,
      objectId: undefined,
      classKey: undefined,
      leakId: undefined,
      originPane: undefined,
      histogramView: {
        searchText: "",
        groupBy: "class",
        sortKey: "retained",
        sortDirection: "desc",
        pageOffset: 0,
      },
    });
  });

  it("restores only IDs compatible with the current workspace revision", () => {
    createWorkspacePersistence(window.sessionStorage).save(persistedWorkspace());

    const restored = useInvestigationStore.getState().activatePersistence(
      persistenceIdentity,
      {
        identity: persistenceIdentity,
        revision: 4,
        objectIds: new Set(["object-current"]),
        classKeys: new Set(["class-current"]),
        leakIds: new Set<string>(),
      },
    );

    expect(restored?.selection).toEqual({
      revision: 4,
      objectId: "object-current",
      classKey: "class-current",
    });
    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 4,
      objectId: "object-current",
      classKey: "class-current",
      leakId: undefined,
      originPane: "inspector",
      histogramView: {
        searchText: "cache",
        pageOffset: 100,
      },
    });
    expect(useInvestigationStore.getState().lastPersistenceNotice).toContain(
      "leak-stale",
    );
  });

  it("isolates notes and bookmarks by opaque workspace identity", () => {
    const compatibility = {
      identity: persistenceIdentity,
      revision: 1,
      objectIds: new Set<string>(),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    };
    useInvestigationStore
      .getState()
      .activatePersistence(persistenceIdentity, compatibility);
    useInvestigationStore.getState().upsertNote({
      id: "note-a",
      target: { kind: "workspace", id: "workspace-note" },
      text: "Revisit retained cache.",
    });
    useInvestigationStore.getState().upsertBookmark({
      id: "bookmark-a",
      target: { kind: "class", id: "class-a" },
      label: "Cache",
    });
    useInvestigationStore.getState().deactivatePersistence();

    const otherIdentity: WorkspacePersistenceIdentity = {
      kind: "workspace",
      key: "source-b",
    };
    useInvestigationStore.getState().activatePersistence(otherIdentity, {
      ...compatibility,
      identity: otherIdentity,
      revision: 2,
    });
    expect(useInvestigationStore.getState().notes).toEqual([]);
    expect(useInvestigationStore.getState().bookmarks).toEqual([]);

    useInvestigationStore.getState().deactivatePersistence();
    useInvestigationStore
      .getState()
      .activatePersistence(persistenceIdentity, compatibility);
    expect(useInvestigationStore.getState().notes).toEqual([
      {
        id: "note-a",
        target: { kind: "workspace", id: "workspace-note" },
        text: "Revisit retained cache.",
      },
    ]);
    expect(useInvestigationStore.getState().bookmarks).toEqual([
      {
        id: "bookmark-a",
        target: { kind: "class", id: "class-a" },
        label: "Cache",
      },
    ]);
  });

  it("persists metadata without active operation payloads", () => {
    const compatibility = {
      identity: persistenceIdentity,
      revision: 0,
      objectIds: new Set(["object-current"]),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    };
    useInvestigationStore
      .getState()
      .activatePersistence(persistenceIdentity, compatibility);
    useInvestigationStore.getState().setObjectId("object-current", "inspector");
    useInvestigationStore.getState().beginOperation("inspect");

    const values = Array.from(
      { length: window.sessionStorage.length },
      (_, index) => window.sessionStorage.getItem(window.sessionStorage.key(index)!),
    ).join("\n");
    expect(values).toContain("object-current");
    expect(values).not.toContain("operationId");
    expect(values).not.toContain("activeOperation");
  });
});
