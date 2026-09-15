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

  it("bumps revision and clears heap-bound selection", () => {
    useInvestigationStore.getState().setObjectId("0xdeadbeef", "dominators");
    useInvestigationStore.getState().setClassKey("com.example.Cache", "histogram");
    useInvestigationStore.getState().setLeakId("leak-1", "leak");

    useInvestigationStore.getState().bumpRevisionOnArtifactChange();

    expect(useInvestigationStore.getState()).toMatchObject({
      revision: 1,
      objectId: undefined,
      classKey: undefined,
      leakId: undefined,
      originPane: undefined,
    });
  });
});
