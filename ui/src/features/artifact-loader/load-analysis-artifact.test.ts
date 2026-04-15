import { describe, expect, it } from "bun:test";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";
import { useArtifactStore } from "./use-artifact-store";

function buildSerializedArtifactWithOptionalAnalyzers() {
  return JSON.stringify({
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
    leaks: [],
    recommendations: ["Trim cache residency."],
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
    unreachable: {
      total_count: 3,
      total_shallow_size: 96,
      by_class: [{ class_name: "byte[]", count: 3, shallow_size: 96 }],
    },
    string_report: {
      total_strings: 10,
      total_string_bytes: 512,
      unique_strings: 4,
      duplicate_groups: [{ value: "dup", count: 3, total_wasted_bytes: 32 }],
      total_duplicate_waste: 32,
      top_strings_by_size: [
        { object_id: 1, value: "payload", byte_length: 64, retained_bytes: 128 },
      ],
    },
    collection_report: {
      total_collections: 5,
      total_waste_bytes: 128,
      empty_collections: 1,
      oversized_collections: [
        {
          object_id: 11,
          collection_type: "java.util.ArrayList",
          size: 2,
          capacity: 32,
          fill_ratio: 0.0625,
          shallow_bytes: 48,
          retained_bytes: 96,
          waste_bytes: 80,
        },
      ],
      summary_by_type: {
        "java.util.ArrayList": {
          count: 5,
          total_shallow: 240,
          total_retained: 480,
          total_waste: 128,
          avg_fill_ratio: 0.25,
        },
      },
    },
    top_instances: {
      total_count: 2,
      instances: [
        {
          object_id: 7,
          class_name: "byte[]",
          shallow_size: 4096,
          retained_size: 8192,
        },
      ],
    },
    classloader_report: {
      loaders: [
        {
          object_id: 21,
          class_name: "org.springframework.boot.loader.LaunchedURLClassLoader",
          loaded_class_count: 12,
          instance_count: 220,
          total_shallow_bytes: 1024,
          retained_bytes: 4096,
          parent_loader: 1,
        },
      ],
      potential_leaks: [
        {
          object_id: 21,
          class_name: "org.springframework.boot.loader.LaunchedURLClassLoader",
          retained_bytes: 4096,
          loaded_class_count: 12,
          reason: "Retains 4 KB but loads only 12 classes",
        },
      ],
    },
    provenance: [],
  });
}

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

  it("round-trips optional analyzer sections from serialized artifact text", () => {
    const parsed = loadAnalysisArtifactFromText(buildSerializedArtifactWithOptionalAnalyzers());

    expect(parsed).toMatchObject({
      unreachable: {
        totalCount: 3,
        totalShallowSize: 96,
        byClass: [{ className: "byte[]", count: 3, shallowSize: 96 }],
      },
      stringReport: {
        duplicateGroups: [{ value: "dup", count: 3, totalWastedBytes: 32 }],
        topStringsBySize: [
          { objectId: 1, value: "payload", byteLength: 64, retainedBytes: 128 },
        ],
      },
      collectionReport: {
        oversizedCollections: [
          {
            objectId: 11,
            collectionType: "java.util.ArrayList",
            capacity: 32,
            fillRatio: 0.0625,
          },
        ],
        summaryByType: {
          "java.util.ArrayList": {
            totalWaste: 128,
            avgFillRatio: 0.25,
          },
        },
      },
      topInstances: {
        instances: [
          { objectId: 7, className: "byte[]", shallowSize: 4096, retainedSize: 8192 },
        ],
      },
      classloaderReport: {
        loaders: [
          {
            objectId: 21,
            loadedClassCount: 12,
            totalShallowBytes: 1024,
            parentLoader: 1,
          },
        ],
        potentialLeaks: [{ reason: "Retains 4 KB but loads only 12 classes" }],
      },
    });
  });

  it("round-trips serialized dominator rows into the frontend graph shape", () => {
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
        leaks: [],
        recommendations: [],
        elapsed: { secs: 1, nanos: 0 },
        graph: {
          node_count: 200,
          edge_count: 400,
          dominators: [
            {
              name: "com.example.Cache",
              class_name: "com.example.Cache",
              object_id: "0x0000000000001234",
              dominates: 7,
              immediate_dominator: "com.example.Root",
              retained_size: 1024,
              shallow_size: 64,
            },
          ],
        },
        provenance: [],
      }),
    );

    expect(parsed.graph).toEqual({
      nodeCount: 200,
      edgeCount: 400,
      dominatorCount: 1,
      dominators: [
        {
          name: "com.example.Cache",
          className: "com.example.Cache",
          objectId: "0x0000000000001234",
          dominates: 7,
          immediateDominator: "com.example.Root",
          retainedSize: 1024,
          shallowSize: 64,
        },
      ],
    });
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
        dominators: [],
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
