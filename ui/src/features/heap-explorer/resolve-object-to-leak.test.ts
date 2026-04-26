import "../../test/setup";

import { describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../lib/analysis-types";

import { resolveObjectToLeak } from "./resolve-object-to-leak";

function createArtifactFixture(): AnalysisArtifact {
  return {
    summary: {
      heapPath: "fixture.hprof",
      totalObjects: 42,
      totalSizeBytes: 2048,
      generatedAt: "2026-04-14T00:00:00Z",
      totalRecords: 2,
    },
    leaks: [
      {
        id: "leak-cache-1",
        className: "com.example.cache.LruCache",
        leakKind: "dominator",
        severity: "high",
        retainedSizeBytes: 1024,
        instances: 1,
        description: "Large cache",
        provenance: [],
      },
    ],
    recommendations: [],
    elapsedSeconds: 1,
    graph: {
      nodeCount: 200,
      edgeCount: 400,
      dominatorCount: 2,
      dominators: [
        {
          name: "LruCache#root",
          className: "com.example.cache.LruCache",
          objectId: "0xdeadbeef",
          dominates: 12,
          immediateDominator: "GC Root",
          retainedSize: 1024,
          shallowSize: 64,
        },
        {
          name: "WorkerQueue#17",
          className: "com.example.jobs.WorkerQueue",
          objectId: "0xcafebabe",
          dominates: 5,
          immediateDominator: "com.example.cache.LruCache@0xdeadbeef",
          retainedSize: 768,
          shallowSize: 48,
        },
      ],
    },
    histogram: {
      groupBy: "class",
      totalInstances: 42,
      totalShallowSize: 2048,
      entries: [],
    },
    provenance: [],
  };
}

describe("resolveObjectToLeak", () => {
  it("returns undefined when objectId is undefined", () => {
    expect(resolveObjectToLeak(undefined, createArtifactFixture())).toBeUndefined();
  });

  it("returns undefined when the objectId does not match a dominator row", () => {
    expect(resolveObjectToLeak("0xnot-found", createArtifactFixture())).toBeUndefined();
  });

  it("returns the matching leak id when the dominator class name matches a leak", () => {
    expect(resolveObjectToLeak("0xdeadbeef", createArtifactFixture())).toBe("leak-cache-1");
  });

  it("returns undefined when the dominator exists but no leak matches its class name", () => {
    const artifact = {
      ...createArtifactFixture(),
      leaks: [
        {
          id: "leak-thread-1",
          className: "com.example.threads.ExecutorThread",
          leakKind: "thread",
          severity: "medium",
          retainedSizeBytes: 256,
          instances: 1,
          description: "Thread retained unexpectedly",
          provenance: [],
        },
      ],
    } satisfies AnalysisArtifact;

    expect(resolveObjectToLeak("0xdeadbeef", artifact)).toBeUndefined();
  });

  it("returns the first matching leak id when multiple leaks share the same class name", () => {
    const artifact = {
      ...createArtifactFixture(),
      leaks: [
        {
          id: "leak-cache-1",
          className: "com.example.cache.LruCache",
          leakKind: "dominator",
          severity: "high",
          retainedSizeBytes: 1024,
          instances: 1,
          description: "Large cache",
          provenance: [],
        },
        {
          id: "leak-cache-2",
          className: "com.example.cache.LruCache",
          leakKind: "dominator",
          severity: "medium",
          retainedSizeBytes: 768,
          instances: 1,
          description: "Secondary cache suspect",
          provenance: [],
        },
      ],
    } satisfies AnalysisArtifact;

    expect(resolveObjectToLeak("0xdeadbeef", artifact)).toBe("leak-cache-1");
  });
});