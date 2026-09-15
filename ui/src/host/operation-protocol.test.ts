import { describe, expect, it } from "bun:test";

import {
  OPERATION_KINDS,
  OPERATION_PHASES,
  isOperationContext,
  isOperationEnvelope,
} from "./operation-protocol";

describe("operation protocol", () => {
  it("recognizes the supported operation kinds and phases", () => {
    expect(OPERATION_KINDS).toEqual([
      "open",
      "analyze",
      "enrich",
      "query",
      "diff",
      "snapshot",
      "flamegraph",
      "gc-path",
      "inspect",
    ]);
    expect(OPERATION_PHASES).toEqual([
      "accepted",
      "opening",
      "parsing",
      "building-graph",
      "computing-dominators",
      "analyzing",
      "rendering",
      "committing",
      "cancelling",
      "cancelled",
      "complete",
      "failed",
    ]);
  });

  it("accepts a valid operation context and response envelope", () => {
    const context = {
      workspaceId: "workspace-1",
      revision: 3,
      operationId: "operation-7",
    };

    expect(isOperationContext(context)).toBe(true);
    expect(isOperationEnvelope({ ...context, data: { rows: 4 } })).toBe(true);
  });

  it("rejects malformed operation identities", () => {
    expect(
      isOperationContext({
        workspaceId: "",
        revision: 0,
        operationId: "operation-1",
      }),
    ).toBe(false);
    expect(
      isOperationContext({
        workspaceId: "workspace-1",
        revision: -1,
        operationId: "operation-1",
      }),
    ).toBe(false);
    expect(
      isOperationContext({
        workspaceId: "workspace-1",
        revision: 1.5,
        operationId: "operation-1",
      }),
    ).toBe(false);
    expect(
      isOperationEnvelope({
        workspaceId: "workspace-1",
        revision: 0,
        operationId: "operation-1",
      }),
    ).toBe(false);
  });
});
