import "../../test/setup";

import { describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import {
  buildArtifactFindingFacts,
  buildPolicyFindingFacts,
  findingHref,
} from "./finding-adapters";

function artifactFixture(): AnalysisArtifact {
  return {
    summary: {
      heapPath: "/secret/fixture.hprof",
      totalObjects: 100,
      totalSizeBytes: 8192,
      totalRecords: 4,
    },
    leaks: [
      {
        id: "leak/cache:1",
        className: "com.example.Cache",
        leakKind: "CACHE",
        severity: "HIGH",
        retainedSizeBytes: 4096,
        instances: 4,
        description: "Cache retains request objects.",
        provenance: [{ kind: "FALLBACK", detail: "retained-size heuristic" }],
      },
    ],
    recommendations: [],
    elapsedSeconds: 0,
    graph: {
      nodeCount: 4,
      edgeCount: 3,
      dominatorCount: 1,
      dominators: [
        {
          name: "com.example.Cache",
          className: "com.example.Cache",
          objectId: "0x2a",
          dominates: 3,
          retainedSize: 4096,
          shallowSize: 64,
        },
      ],
    },
    classloaderReport: {
      loaders: [],
      potentialLeaks: [
        {
          objectId: 43,
          className: "org.example.WebAppClassLoader",
          retainedBytes: 3072,
          loadedClassCount: 1,
          reason: "Loader retains classes after redeploy.",
        },
      ],
      duplicateClasses: [
        {
          className: "com.example.Handler",
          loaderObjectIds: [43, 44],
          loaderCount: 2,
        },
      ],
    },
    collectionReport: {
      totalCollections: 1,
      totalWasteBytes: 2048,
      emptyCollections: 0,
      oversizedCollections: [
        {
          objectId: 45,
          collectionType: "java.util.HashMap",
          size: 1,
          capacity: 128,
          fillRatio: 1 / 128,
          shallowBytes: 128,
          retainedBytes: 2304,
          wasteBytes: 2048,
        },
      ],
      summaryByType: {},
    },
    stringReport: {
      totalStrings: 8,
      totalStringBytes: 512,
      uniqueStrings: 7,
      duplicateGroups: [
        {
          value: "sensitive fixture value",
          count: 2,
          totalWastedBytes: 128,
        },
      ],
      totalDuplicateWaste: 128,
      topStringsBySize: [],
    },
    arrayReport: {
      totalArrays: 2,
      uniqueContents: 1,
      duplicateGroups: [
        {
          elementType: "byte",
          contentHash: 12345,
          length: 64,
          count: 2,
          totalWastedBytes: 64,
        },
      ],
      totalDuplicateWaste: 64,
    },
    provenance: [{ kind: "PARTIAL", detail: "fixture analyzer coverage" }],
  };
}

describe("finding adapters", () => {
  it("adapts every measured finding kind to a stable non-index target", () => {
    const facts = buildArtifactFindingFacts(artifactFixture());

    expect(facts.map((fact) => fact.kind)).toEqual(
      expect.arrayContaining([
        "leak",
        "classloader",
        "collection-waste",
        "string-waste",
        "array-waste",
      ]),
    );

    const leak = facts.find((fact) => fact.id === "leak:leak/cache:1");
    expect(leak).toMatchObject({
      description: "Cache retains request objects.",
      target: {
        kind: "leak",
        leakId: "leak/cache:1",
        classKey: "com.example.Cache",
        objectId: "0x2a",
      },
      metrics: { retainedBytes: 4096, instances: 4 },
    });
    expect(findingHref(leak!.target)).toBe("/leaks/leak%2Fcache%3A1/overview");

    const loader = facts.find((fact) => fact.id === "classloader:loader:0x2b");
    expect(loader?.target).toEqual({
      kind: "object",
      objectId: "0x2b",
      classKey: "org.example.WebAppClassLoader",
    });
    expect(findingHref(loader!.target)).toBe(
      "/heap-explorer/object-inspector?objectId=0x2b",
    );

    const duplicateClass = facts.find(
      (fact) => fact.id === "classloader:duplicate:com.example.Handler",
    );
    expect(findingHref(duplicateClass!.target)).toBe(
      "/artifacts/explorer?classKey=com.example.Handler",
    );

    const collection = facts.find((fact) => fact.id === "collection:0x2d");
    expect(findingHref(collection!.target)).toBe(
      "/heap-explorer/object-inspector?objectId=0x2d",
    );

    const stringWaste = facts.find((fact) => fact.kind === "string-waste");
    expect(stringWaste?.id).not.toContain("sensitive fixture value");
    expect(findingHref(stringWaste!.target)).toBe(
      "/artifacts/explorer?classKey=java.lang.String",
    );

    const arrayWaste = facts.find((fact) => fact.id === "array:byte:12345");
    expect(findingHref(arrayWaste!.target)).toBe(
      "/artifacts/explorer?classKey=byte%5B%5D",
    );

    for (const fact of facts) {
      expect(fact.id).not.toMatch(/row|index/i);
      expect(findingHref(fact.target)).not.toContain("/secret/");
    }
  });

  it("keeps fact identities and links stable when source rows reorder", () => {
    const artifact = artifactFixture();
    artifact.leaks.push({
      ...artifact.leaks[0],
      id: "leak-second",
      className: "com.example.Second",
      severity: "LOW",
      retainedSizeBytes: 16,
      description: "Second measured leak.",
    });

    const first = buildArtifactFindingFacts(artifact).map((fact) => [
      fact.id,
      findingHref(fact.target),
    ]);
    artifact.leaks.reverse();
    artifact.classloaderReport!.potentialLeaks.reverse();
    artifact.classloaderReport!.duplicateClasses.reverse();
    artifact.collectionReport!.oversizedCollections.reverse();
    artifact.stringReport!.duplicateGroups.reverse();
    artifact.arrayReport!.duplicateGroups.reverse();
    const reordered = buildArtifactFindingFacts(artifact).map((fact) => [
      fact.id,
      findingHref(fact.target),
    ]);

    expect(reordered).toEqual(first);
  });

  it("adapts policy violations only when a relevant stable measured target exists", () => {
    const measured = buildArtifactFindingFacts(artifactFixture());
    const policyFacts = buildPolicyFindingFacts(
      [
        {
          rule_id: "leak-budget",
          predicate: "leak_count",
          severity: "error",
          message: "expected leak_count <= 0, got 1",
          remediation_hint: "Inspect the measured leak.",
        },
        {
          rule_id: "loader-budget",
          predicate: "classloader_leak_count",
          severity: "warning",
          message: "expected classloader_leak_count <= 0, got 1",
        },
        {
          rule_id: "growth",
          predicate: "object_growth_threshold",
          severity: "critical",
          message: "one object exceeded the growth threshold",
        },
        {
          rule_id: "generic",
          predicate: "total_bytes",
          severity: "info",
          message: "heap exceeds generic budget",
        },
      ],
      measured,
    );

    expect(policyFacts.map((fact) => fact.id)).toEqual([
      "policy:growth:object_growth_threshold",
      "policy:leak-budget:leak_count",
      "policy:loader-budget:classloader_leak_count",
    ]);
    expect(policyFacts[0]).toMatchObject({
      description: "one object exceeded the growth threshold",
      target: { kind: "object", objectId: "0x2b" },
      metrics: {
        ruleId: "growth",
        predicate: "object_growth_threshold",
        remediationHint: "",
      },
    });
    expect(policyFacts.some((fact) => fact.id.includes("generic"))).toBe(false);
  });
});
