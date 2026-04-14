import { describe, expect, it } from "bun:test";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";
import { useArtifactStore } from "./use-artifact-store";

describe("loadAnalysisArtifactFromText", () => {
  it("throws a readable error for malformed JSON", () => {
    expect(() => loadAnalysisArtifactFromText("not-json")).toThrow(/invalid json/i);
  });

  it("accepts structured generated_at timestamps from serialized Rust artifacts", () => {
    const parsed = loadAnalysisArtifactFromText(
      JSON.stringify({
        summary: {
          heap_path: "heap.hprof",
          total_objects: 42,
          total_size_bytes: 2048,
          classes: [],
          generated_at: {
            secs_since_epoch: 1713052800,
            nanos_since_epoch: 123000000,
          },
          header: null,
          total_records: 2,
          record_stats: [],
        },
        leaks: [],
        recommendations: [],
        elapsed: { secs: 1, nanos: 0 },
        graph: {
          node_count: 200,
          edge_count: 400,
          dominators: [],
        },
      }),
    );

    expect(parsed.summary.generatedAt).toBe("2024-04-14T00:00:00.123Z");
  });

  it("parses real serialized JSON text into the adapted frontend artifact shape", () => {
    const parsed = loadAnalysisArtifactFromText(
      JSON.stringify({
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
        elapsed: { secs: 1, nanos: 500000000 },
        graph: {
          node_count: 200,
          edge_count: 400,
          dominators: [{ name: "root" }],
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
      }),
    );

    expect(parsed.summary.heapPath).toBe("heap.hprof");
    expect(parsed.leaks[0]?.className).toBe("com.example.Cache");
    expect(parsed.elapsedSeconds).toBe(1.5);
    expect(parsed.graph.dominatorCount).toBe(1);
    expect(parsed.histogram?.entries[0]?.instanceCount).toBe(4);
    expect(parsed.provenance).toEqual([]);
    expect(parsed.leaks[0]?.provenance).toEqual([]);
  });
});

describe("useArtifactStore", () => {
  it("stores loaded artifacts and clears previous errors", () => {
    useArtifactStore.getState().reset();
    useArtifactStore.getState().setLoadError("bad artifact");

    useArtifactStore.getState().setArtifact("fixture.json", {
      summary: {
        heapPath: "heap.hprof",
        totalObjects: 42,
        totalSizeBytes: 2048,
        totalRecords: 2,
        generatedAt: "2026-04-14T00:00:00Z",
      },
      leaks: [],
      recommendations: [],
      elapsedSeconds: 1,
      graph: {
        nodeCount: 1,
        edgeCount: 2,
        dominatorCount: 0,
      },
      provenance: [],
    });

    const state = useArtifactStore.getState();
    expect(state.artifactName).toBe("fixture.json");
    expect(state.artifact?.summary.heapPath).toBe("heap.hprof");
    expect(state.artifact?.summary.generatedAt).toBe("2026-04-14T00:00:00Z");
    expect(state.loadError).toBeUndefined();
  });
});
