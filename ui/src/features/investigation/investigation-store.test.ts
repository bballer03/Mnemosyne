import { beforeEach, describe, expect, it } from "bun:test";

import { useInvestigationStore } from "./investigation-store";

describe("useInvestigationStore", () => {
  beforeEach(() => {
    useInvestigationStore.setState({
      revision: 0,
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
