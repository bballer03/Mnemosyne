import "../../test/setup";

import { beforeEach, describe, expect, it } from "bun:test";

import {
  useInvestigationStore,
  type FindingFact,
} from "./investigation-store";
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

function findingFact(
  id: string,
  source: FindingFact["source"] = "artifact",
): FindingFact {
  return {
    id,
    source,
    kind: source === "policy" ? "policy" : "leak",
    severity: "HIGH",
    title: `Finding ${id}`,
    description: `Measured fact ${id}`,
    target: { kind: "leak", leakId: id.replace(/^leak:/, "") },
    provenance: [{ kind: "FALLBACK", detail: "fixture" }],
    metrics: { retainedBytes: 1024 },
  };
}

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
      activeWorkflow: undefined,
      workflowNeedsRecovery: false,
      workspaceRequests: {},
      analysisMode: undefined,
      capabilities: undefined,
      objectId: undefined,
      classKey: undefined,
      leakId: undefined,
      originPane: undefined,
      persistenceIdentity: undefined,
      notes: [],
      bookmarks: [],
      findingFacts: [],
      findingStatuses: {},
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

  it("binds only the latest matching workflow request to this revision", () => {
    const stale = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    const current = useInvestigationStore.getState().beginWorkspaceRequest("workflow");

    expect(
      useInvestigationStore.getState().bindWorkflow(stale, "tune_gc", {
        workflowId: "wf-stale",
        currentStep: "thread_local_review",
      }),
    ).toBe(false);
    expect(
      useInvestigationStore.getState().bindWorkflow(current, "tune_gc", {
        workflowId: "wf-current",
        currentStep: "thread_local_review",
      }),
    ).toBe(true);
    expect(useInvestigationStore.getState().activeWorkflow).toEqual({
      workspaceId: "workspace-1",
      revision: 0,
      workflowId: "wf-current",
      kind: "tune_gc",
      currentStep: "thread_local_review",
    });
  });

  it("invalidates workflow and Assistant requests with the workspace revision", () => {
    const workflow = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    const assistant = useInvestigationStore.getState().beginWorkspaceRequest("assistant");
    useInvestigationStore.getState().bindWorkflow(workflow, "triage_memory_leak", {
      workflowId: "wf-1",
      currentStep: "investigate_suspect",
    });

    useInvestigationStore.getState().bumpRevisionOnArtifactChange();

    expect(useInvestigationStore.getState().activeWorkflow).toBeUndefined();
    expect(useInvestigationStore.getState().acceptWorkspaceRequest("workflow", workflow)).toBe(false);
    expect(useInvestigationStore.getState().acceptWorkspaceRequest("assistant", assistant)).toBe(false);
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

  it("persists and restores only a revision-compatible workflow candidate", () => {
    const compatibility = {
      identity: persistenceIdentity,
      revision: 0,
      objectIds: new Set<string>(),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    };
    useInvestigationStore.getState().activatePersistence(persistenceIdentity, compatibility);
    const request = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    useInvestigationStore.getState().bindWorkflow(request, "tune_gc", {
      workflowId: "wf-persisted",
      currentStep: "thread_local_review",
    });
    useInvestigationStore.getState().deactivatePersistence();

    useInvestigationStore.getState().activatePersistence(persistenceIdentity, {
      ...compatibility,
      revision: 4,
    });

    expect(useInvestigationStore.getState().activeWorkflow).toEqual({
      workspaceId: "workspace-1",
      revision: 4,
      workflowId: "wf-persisted",
      kind: "tune_gc",
      currentStep: "thread_local_review",
    });
    expect(useInvestigationStore.getState().workflowNeedsRecovery).toBe(true);
    const persisted = Array.from(
      { length: window.sessionStorage.length },
      (_, index) => window.sessionStorage.getItem(window.sessionStorage.key(index)!),
    ).join("\n");
    expect(persisted).toContain("wf-persisted");
    expect(persisted).not.toContain("stepResult");
    expect(persisted).not.toContain("heapPath");
  });

  it("freezes accepted facts while keeping user status separate", () => {
    const fact = findingFact("leak:leak-1");
    const context = { workspaceId: "workspace-1", revision: 0 };

    expect(
      useInvestigationStore
        .getState()
        .replaceFindings(context, "artifact", [fact]),
    ).toBe(true);

    const accepted = useInvestigationStore.getState().findingFacts[0];
    expect(accepted).toEqual(fact);
    expect(Object.isFrozen(accepted)).toBe(true);
    expect(Object.isFrozen(accepted.target)).toBe(true);
    expect(Object.isFrozen(accepted.provenance)).toBe(true);
    expect(Object.isFrozen(accepted.provenance[0])).toBe(true);
    expect(Object.isFrozen(accepted.metrics)).toBe(true);

    expect(
      useInvestigationStore.getState().setFindingStatus(fact.id, "resolved"),
    ).toBe(true);
    expect(useInvestigationStore.getState().findingFacts[0]).toBe(accepted);
    expect(useInvestigationStore.getState().findingStatuses).toEqual({
      [fact.id]: "resolved",
    });
  });

  it("rejects finding payloads from stale workspaces and revisions", () => {
    const current = findingFact("leak:current");
    const stale = findingFact("leak:stale");
    const store = useInvestigationStore.getState();

    expect(
      store.replaceFindings(
        { workspaceId: "workspace-1", revision: 0 },
        "artifact",
        [current],
      ),
    ).toBe(true);
    expect(
      useInvestigationStore.getState().replaceFindings(
        { workspaceId: "workspace-old", revision: 0 },
        "artifact",
        [stale],
      ),
    ).toBe(false);
    expect(
      useInvestigationStore.getState().replaceFindings(
        { workspaceId: "workspace-1", revision: -1 },
        "artifact",
        [stale],
      ),
    ).toBe(false);
    expect(useInvestigationStore.getState().findingFacts).toEqual([current]);
  });

  it("replaces one source without disturbing the other source or unchanged statuses", () => {
    const context = { workspaceId: "workspace-1", revision: 0 };
    const retained = findingFact("leak:retained");
    const removed = findingFact("leak:removed");
    const policy = findingFact("policy:rule:leak_count", "policy");

    useInvestigationStore
      .getState()
      .replaceFindings(context, "artifact", [retained, removed]);
    useInvestigationStore
      .getState()
      .setFindingStatus(retained.id, "deferred");
    useInvestigationStore
      .getState()
      .setFindingStatus(removed.id, "resolved");
    useInvestigationStore
      .getState()
      .replaceFindings(context, "policy", [policy]);

    expect(
      useInvestigationStore
        .getState()
        .replaceFindings(context, "artifact", [retained]),
    ).toBe(true);
    expect(
      useInvestigationStore.getState().findingFacts.map((fact) => fact.id),
    ).toEqual([retained.id, policy.id]);
    expect(useInvestigationStore.getState().findingStatuses).toEqual({
      [retained.id]: "deferred",
    });
    expect(
      useInvestigationStore
        .getState()
        .setFindingStatus("missing-finding", "resolved"),
    ).toBe(false);
  });

  it("clears findings and statuses when the artifact revision changes", () => {
    const context = { workspaceId: "workspace-1", revision: 0 };
    const fact = findingFact("leak:leak-1");
    useInvestigationStore
      .getState()
      .replaceFindings(context, "artifact", [fact]);
    useInvestigationStore
      .getState()
      .setFindingStatus(fact.id, "resolved");

    useInvestigationStore.getState().bumpRevisionOnArtifactChange();

    expect(useInvestigationStore.getState().findingFacts).toEqual([]);
    expect(useInvestigationStore.getState().findingStatuses).toEqual({});
  });
});
