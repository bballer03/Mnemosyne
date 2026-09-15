import "../../test/setup";

import { describe, expect, it } from "bun:test";

import type {
  FindingFact,
  FindingTarget,
  InvestigationSelection,
} from "../investigation/investigation-store";
import {
  MAX_ASSISTANT_CONTEXT_FINDINGS,
  buildAssistantMeasuredContext,
} from "./assistant-context";

function makeFact(
  id: string,
  target: FindingTarget,
  overrides: Partial<FindingFact> = {},
): FindingFact {
  return {
    id,
    source: "artifact",
    kind: "collection-waste",
    severity: "WARNING",
    title: `Measured ${id}`,
    description: `Measured description for ${id}`,
    target,
    provenance: [{ kind: "Measured", detail: "artifact analysis" }],
    metrics: { retainedBytes: 128 },
    ...overrides,
  };
}

describe("buildAssistantMeasuredContext", () => {
  it("projects matching object and class findings in store order as frozen copies", () => {
    const selection: InvestigationSelection = {
      revision: 7,
      objectId: "0x10",
      classKey: "com.example.Cache",
      originPane: "inspector",
    };
    const unrelatedFact = makeFact("collection:0x20", {
      kind: "object",
      objectId: "0x20",
      classKey: "com.example.Other",
    });
    const objectFact = makeFact("collection:0x10", {
      kind: "object",
      objectId: "0x10",
      classKey: "java.util.HashMap",
    });
    const classFact = makeFact(
      "classloader:duplicate:com.example.Cache",
      { kind: "class", classKey: "com.example.Cache" },
      { kind: "classloader" },
    );
    const input = [unrelatedFact, objectFact, classFact] as const;

    const projected = buildAssistantMeasuredContext(selection, input);

    expect(projected.map((fact) => fact.id)).toEqual([
      "collection:0x10",
      "classloader:duplicate:com.example.Cache",
    ]);
    expect(Object.isFrozen(projected)).toBe(true);
    expect(Object.isFrozen(projected[0])).toBe(true);
    expect(Object.isFrozen(projected[0]?.target)).toBe(true);
    expect(Object.isFrozen(projected[0]?.provenance)).toBe(true);
    expect(Object.isFrozen(projected[0]?.metrics)).toBe(true);
    expect(projected[0]).not.toBe(objectFact);
    expect(input).toEqual([unrelatedFact, objectFact, classFact]);
  });

  it("matches a leak only through its stable leak id", () => {
    const leakFact = makeFact(
      "leak:leak-7",
      {
        kind: "leak",
        leakId: "leak-7",
        classKey: "com.example.Cache",
        objectId: "0x70",
      },
      { kind: "leak", severity: "HIGH" },
    );

    expect(
      buildAssistantMeasuredContext({ revision: 1, leakId: "leak-7" }, [leakFact]),
    ).toHaveLength(1);
    expect(
      buildAssistantMeasuredContext({ revision: 1, leakId: "leak-other" }, [leakFact]),
    ).toHaveLength(0);
  });

  it("returns no findings when the workspace has no stable selected identifiers", () => {
    const fact = makeFact("collection:0x10", {
      kind: "object",
      objectId: "0x10",
    });

    expect(buildAssistantMeasuredContext({ revision: 1 }, [fact])).toEqual([]);
  });

  it("caps matched measured findings without sorting or consulting advisory state", () => {
    const findings = Array.from(
      { length: MAX_ASSISTANT_CONTEXT_FINDINGS + 2 },
      (_, index) =>
        makeFact(`policy:${index}`, {
          kind: "class",
          classKey: "com.example.Cache",
        }),
    );

    const projected = buildAssistantMeasuredContext(
      {
        revision: 3,
        classKey: "com.example.Cache",
        originPane: "findings",
      },
      findings,
    );

    expect(projected).toHaveLength(MAX_ASSISTANT_CONTEXT_FINDINGS);
    expect(projected.map((fact) => fact.id)).toEqual(
      findings.slice(0, MAX_ASSISTANT_CONTEXT_FINDINGS).map((fact) => fact.id),
    );
    expect(JSON.stringify(projected)).not.toContain("findingStatuses");
    expect(JSON.stringify(projected)).not.toContain("notes");
    expect(JSON.stringify(projected)).not.toContain("answerSummary");
    expect(JSON.stringify(projected)).not.toContain("/secret/heaps");
  });
});
