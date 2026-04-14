import { describe, expect, it } from "bun:test";

import { parseAnalysisArtifact } from "./analysis-types";

describe("parseAnalysisArtifact", () => {
  it("accepts a valid Mnemosyne analysis artifact", () => {
    const parsed = parseAnalysisArtifact({
      summary: {
        heap_path: "heap.hprof",
        total_objects: 42,
        total_size_bytes: 2048,
        classes: [],
        generated_at: "2026-04-14T00:00:00Z",
        header: null,
        total_records: 2,
        record_stats: [],
      },
      leaks: [
        {
          id: "leak-1",
          class_name: "com.example.Cache",
          leak_kind: "CACHE",
          severity: "HIGH",
          retained_size_bytes: 1024,
          shallow_size_bytes: 64,
          suspect_score: 0.98,
          instances: 4,
          description: "Cache retains request objects",
          provenance: [],
        },
      ],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 200,
        edge_count: 400,
        dominators: [],
      },
      histogram: {
        group_by: "class",
        entries: [
          {
            key: "com.example.Cache",
            instance_count: 4,
            shallow_size: 64,
            retained_size: 1024,
          },
        ],
        total_instances: 42,
        total_shallow_size: 2048,
      },
      provenance: [],
    });

    expect(parsed.summary.heapPath).toBe("heap.hprof");
    expect(parsed.summary.totalObjects).toBe(42);
    expect(parsed.summary.totalSizeBytes).toBe(2048);
    expect(parsed.summary.totalRecords).toBe(2);
    expect(parsed.leaks).toHaveLength(1);
    expect(parsed.leaks[0]?.className).toBe("com.example.Cache");
    expect(parsed.leaks[0]?.leakKind).toBe("CACHE");
    expect(parsed.leaks[0]?.retainedSizeBytes).toBe(1024);
    expect(parsed.leaks[0]?.shallowSizeBytes).toBe(64);
    expect(parsed.leaks[0]?.suspectScore).toBe(0.98);
    expect(parsed.graph.nodeCount).toBe(200);
    expect(parsed.graph.edgeCount).toBe(400);
    expect(parsed.graph.dominatorCount).toBe(0);
    expect(parsed.histogram?.groupBy).toBe("class");
    expect(parsed.histogram?.totalInstances).toBe(42);
    expect(parsed.histogram?.totalShallowSize).toBe(2048);
    expect(parsed.histogram?.entries[0]?.instanceCount).toBe(4);
    expect(parsed.histogram?.entries[0]?.shallowSize).toBe(64);
    expect(parsed.histogram?.entries[0]?.retainedSize).toBe(1024);
    expect(parsed.elapsedSeconds).toBe(1);
  });

  it("rejects artifacts that are missing required sections", () => {
    expect(() => parseAnalysisArtifact({ summary: {} })).toThrow(/invalid mnemosyne analysis artifact/i);
  });

  it("defaults omitted response and leak provenance to empty arrays", () => {
    const parsed = parseAnalysisArtifact({
      summary: {
        heap_path: "heap.hprof",
        total_objects: 42,
        total_size_bytes: 2048,
        classes: [],
        generated_at: "2026-04-14T00:00:00Z",
        header: null,
        total_records: 2,
        record_stats: [],
      },
      leaks: [
        {
          id: "leak-1",
          class_name: "com.example.Cache",
          leak_kind: "CACHE",
          severity: "HIGH",
          retained_size_bytes: 1024,
          shallow_size_bytes: 64,
          suspect_score: 0.98,
          instances: 4,
          description: "Cache retains request objects",
        },
      ],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 200,
        edge_count: 400,
        dominators: [],
      },
    });

    expect(parsed.provenance).toEqual([]);
    expect(parsed.leaks[0]?.provenance).toEqual([]);
    expect(parsed.summary.heapPath).toBe("heap.hprof");
  });
});
