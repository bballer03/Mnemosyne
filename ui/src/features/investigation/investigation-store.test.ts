import { beforeEach, describe, expect, it } from "bun:test";

import { useInvestigationStore } from "./investigation-store";

describe("useInvestigationStore", () => {
  beforeEach(() => {
    useInvestigationStore.setState({
      workspaceId: "workspace-1",
      revision: 0,
      activeOperation: undefined,
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
});
