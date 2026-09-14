import { describe, expect, it } from "bun:test";

import { parseAnalysisArtifact } from "./analysis-types";

function buildArtifactWithOptionalAnalyzers() {
  return {
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
  };
}

// Shape verified against a real `mnemosyne analyze --by-referrer
// --classloaders --threads --format json` run (Slice 14.C) against a
// synthetic HPROF fixture combining a cross-loader duplicate class, a
// two-frame thread stack with ROOT_JAVA_FRAME locals, and a parent-loader
// reference edge.
function buildArtifactWithReferrerClassloaderThreadSections() {
  return {
    summary: {
      heap_path: "heap.hprof",
      total_objects: 20,
      total_size_bytes: 987,
      classes: [],
      generated_at: "2026-04-14T00:00:00Z",
      header: null,
      total_records: 20,
      record_stats: [],
    },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 0, nanos: 0 },
    graph: {
      node_count: 5,
      edge_count: 1,
      dominators: [],
    },
    thread_report: {
      threads: [
        {
          object_id: 20480,
          name: "Thread-7",
          daemon: false,
          stack_trace: [
            {
              method_name: "run",
              class_name: "com/example/WorkerThread",
              source_file: "WorkerThread.java",
              line_number: 42,
              locals: [
                {
                  variable_slot: 0,
                  object_id: "0x00003000",
                  class_name: "com.example.webapp.RequestHandler",
                  root_kind: "JavaFrame",
                },
              ],
            },
            {
              method_name: "mainLoop",
              class_name: "com/example/WorkerThread",
              source_file: "WorkerThread.java",
              line_number: 17,
              locals: [],
            },
          ],
          retained_bytes: 0,
          thread_local_count: 0,
          thread_local_bytes: 0,
        },
      ],
      total_thread_count: 1,
      total_thread_retained: 0,
      top_retainers: [
        {
          object_id: 20480,
          name: "Thread-7",
          daemon: false,
          stack_trace: null,
          retained_bytes: 0,
          thread_local_count: 0,
          thread_local_bytes: 0,
        },
      ],
    },
    classloader_report: {
      loaders: [
        {
          object_id: 4096,
          class_name: "com.example.webapp.WebappLoader",
          loaded_class_count: 1,
          instance_count: 1,
          total_shallow_bytes: 0,
          retained_bytes: 4,
          parent_loader: null,
          unique_class_count: 0,
          ancestor_chain: [],
        },
        {
          object_id: 8192,
          class_name: "com.example.webapp.WebappLoader",
          loaded_class_count: 1,
          instance_count: 1,
          total_shallow_bytes: 0,
          retained_bytes: 4,
          parent_loader: 4096,
          unique_class_count: 0,
          ancestor_chain: [4096],
        },
      ],
      potential_leaks: [],
      duplicate_classes: [
        {
          class_name: "com.example.webapp.RequestHandler",
          loader_object_ids: [4096, 8192],
          loader_count: 2,
        },
      ],
    },
    referrer_report: {
      entries: [
        {
          object_id: "0x00001000",
          class_name: "com/example/webapp/WebappLoader",
          retained_size: 4,
          referrer_count: 1,
          top_referrer_classes: [["com/example/webapp/WebappLoader", 1]],
        },
        {
          object_id: "0x00002000",
          class_name: "com/example/webapp/WebappLoader",
          retained_size: 4,
          referrer_count: 0,
          top_referrer_classes: [],
        },
      ],
      total_objects_considered: 5,
    },
    provenance: [
      {
        kind: "FALLBACK",
        detail: "Graph-backed dominator analysis was available but leak filters produced no results; heuristic fallback was used for leak detection.",
      },
    ],
  };
}

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

  it("parses optional artifact-backed analyzer sections", () => {
    const parsed = parseAnalysisArtifact(buildArtifactWithOptionalAnalyzers());

    expect(parsed).toMatchObject({
      unreachable: {
        totalCount: 3,
        totalShallowSize: 96,
        byClass: [{ className: "byte[]", count: 3, shallowSize: 96 }],
      },
      stringReport: {
        totalStrings: 10,
        totalStringBytes: 512,
        uniqueStrings: 4,
        duplicateGroups: [{ value: "dup", count: 3, totalWastedBytes: 32 }],
        totalDuplicateWaste: 32,
        topStringsBySize: [
          { objectId: 1, value: "payload", byteLength: 64, retainedBytes: 128 },
        ],
      },
      collectionReport: {
        totalCollections: 5,
        totalWasteBytes: 128,
        emptyCollections: 1,
        oversizedCollections: [
          {
            objectId: 11,
            collectionType: "java.util.ArrayList",
            size: 2,
            capacity: 32,
            fillRatio: 0.0625,
            shallowBytes: 48,
            retainedBytes: 96,
            wasteBytes: 80,
          },
        ],
        summaryByType: {
          "java.util.ArrayList": {
            count: 5,
            totalShallow: 240,
            totalRetained: 480,
            totalWaste: 128,
            avgFillRatio: 0.25,
          },
        },
      },
      topInstances: {
        totalCount: 2,
        instances: [
          { objectId: 7, className: "byte[]", shallowSize: 4096, retainedSize: 8192 },
        ],
      },
      classloaderReport: {
        loaders: [
          {
            objectId: 21,
            className: "org.springframework.boot.loader.LaunchedURLClassLoader",
            loadedClassCount: 12,
            instanceCount: 220,
            totalShallowBytes: 1024,
            retainedBytes: 4096,
            parentLoader: 1,
          },
        ],
        potentialLeaks: [
          {
            objectId: 21,
            className: "org.springframework.boot.loader.LaunchedURLClassLoader",
            retainedBytes: 4096,
            loadedClassCount: 12,
            reason: "Retains 4 KB but loads only 12 classes",
          },
        ],
      },
    });
  });

  it("preserves serialized dominator rows for browser navigation", () => {
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
    });

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

  it("falls back to name when fallback dominator rows serialize an empty class_name", () => {
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
      leaks: [],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 2,
        edge_count: 1,
        dominators: [
          {
            name: "com.example.FallbackNode",
            class_name: "",
            object_id: "",
            dominates: 1,
            retained_size: 0,
            shallow_size: 0,
          },
        ],
      },
      provenance: [],
    });

    expect(parsed.graph.dominators[0]?.className).toBe("com.example.FallbackNode");
  });

  it("maps null immediate_dominator to undefined for root dominator rows", () => {
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
      leaks: [],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 1,
        edge_count: 0,
        dominators: [
          {
            name: "com.example.Root",
            class_name: "com.example.Root",
            object_id: "0x0000000000001000",
            dominates: 0,
            immediate_dominator: null,
            retained_size: 1024,
            shallow_size: 64,
          },
        ],
      },
      provenance: [],
    });

    expect(parsed.graph.dominators[0]?.immediateDominator).toBeUndefined();
  });

  it("rejects null for optional strings outside dominator immediate_dominator", () => {
    expect(() =>
      parseAnalysisArtifact({
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
            instances: 4,
            description: "Cache retains request objects",
            provenance: [{ kind: "FALLBACK", detail: null }],
          },
        ],
        recommendations: [],
        elapsed: { secs: 1, nanos: 0 },
        graph: {
          node_count: 1,
          edge_count: 0,
          dominators: [
            {
              name: "com.example.Root",
              class_name: "com.example.Root",
              object_id: "0x0000000000001000",
              dominates: 0,
              immediate_dominator: null,
              retained_size: 1024,
              shallow_size: 64,
            },
          ],
        },
        provenance: [],
      }),
    ).toThrow(/expected leaks\[0\]\.provenance\[0\]\.detail to be a string/i);
  });

  it("parses referrer, extended-classloader, and thread report sections from a real backend shape", () => {
    const parsed = parseAnalysisArtifact(buildArtifactWithReferrerClassloaderThreadSections());

    expect(parsed.referrerReport).toEqual({
      entries: [
        {
          objectId: "0x00001000",
          className: "com/example/webapp/WebappLoader",
          retainedSize: 4,
          referrerCount: 1,
          topReferrerClasses: [["com/example/webapp/WebappLoader", 1]],
        },
        {
          objectId: "0x00002000",
          className: "com/example/webapp/WebappLoader",
          retainedSize: 4,
          referrerCount: 0,
          topReferrerClasses: [],
        },
      ],
      totalObjectsConsidered: 5,
    });

    expect(parsed.classloaderReport).toEqual({
      loaders: [
        {
          objectId: 4096,
          className: "com.example.webapp.WebappLoader",
          loadedClassCount: 1,
          instanceCount: 1,
          totalShallowBytes: 0,
          retainedBytes: 4,
          parentLoader: undefined,
          uniqueClassCount: 0,
          ancestorChain: [],
        },
        {
          objectId: 8192,
          className: "com.example.webapp.WebappLoader",
          loadedClassCount: 1,
          instanceCount: 1,
          totalShallowBytes: 0,
          retainedBytes: 4,
          parentLoader: 4096,
          uniqueClassCount: 0,
          ancestorChain: [4096],
        },
      ],
      potentialLeaks: [],
      duplicateClasses: [
        {
          className: "com.example.webapp.RequestHandler",
          loaderObjectIds: [4096, 8192],
          loaderCount: 2,
        },
      ],
    });

    expect(parsed.threadReport?.totalThreadCount).toBe(1);
    expect(parsed.threadReport?.totalThreadRetained).toBe(0);
    expect(parsed.threadReport?.threads[0]).toEqual({
      objectId: 20480,
      name: "Thread-7",
      daemon: false,
      stackTrace: [
        {
          methodName: "run",
          className: "com/example/WorkerThread",
          sourceFile: "WorkerThread.java",
          lineNumber: 42,
          locals: [
            {
              variableSlot: 0,
              objectId: "0x00003000",
              className: "com.example.webapp.RequestHandler",
              rootKind: "JavaFrame",
            },
          ],
        },
        {
          methodName: "mainLoop",
          className: "com/example/WorkerThread",
          sourceFile: "WorkerThread.java",
          lineNumber: 17,
          locals: [],
        },
      ],
      retainedBytes: 0,
      threadLocalCount: 0,
      threadLocalBytes: 0,
    });
    expect(parsed.threadReport?.topRetainers[0]?.stackTrace).toBeUndefined();
  });

  it("leaves referrerReport and threadReport undefined when absent from the artifact (pre-M8 artifact)", () => {
    const parsed = parseAnalysisArtifact(buildArtifactWithOptionalAnalyzers());

    expect(parsed.referrerReport).toBeUndefined();
    expect(parsed.threadReport).toBeUndefined();
  });

  it("defaults classloader duplicateClasses/uniqueClassCount/ancestorChain when absent from an older artifact", () => {
    const parsed = parseAnalysisArtifact(buildArtifactWithOptionalAnalyzers());

    expect(parsed.classloaderReport?.duplicateClasses).toEqual([]);
    expect(parsed.classloaderReport?.loaders[0]).toMatchObject({
      uniqueClassCount: 0,
      ancestorChain: [],
    });
  });

  it("leaves classloaderReport, referrerReport, and threadReport entirely undefined when absent", () => {
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
      leaks: [],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 1,
        edge_count: 0,
        dominators: [],
      },
      provenance: [],
    });

    expect(parsed.classloaderReport).toBeUndefined();
    expect(parsed.referrerReport).toBeUndefined();
    expect(parsed.threadReport).toBeUndefined();
  });

  it("parses array_report duplicate groups from snake_case JSON (M19.A)", () => {
    const parsed = parseAnalysisArtifact({
      ...buildArtifactWithOptionalAnalyzers(),
      array_report: {
        total_arrays: 12,
        unique_contents: 8,
        total_duplicate_waste: 4096,
        duplicate_groups: [
          {
            element_type: "byte",
            content_hash: 3735928559,
            length: 64,
            count: 3,
            total_wasted_bytes: 2048,
          },
        ],
      },
    });

    expect(parsed.arrayReport).toEqual({
      totalArrays: 12,
      uniqueContents: 8,
      totalDuplicateWaste: 4096,
      duplicateGroups: [
        {
          elementType: "byte",
          contentHash: 3735928559,
          length: 64,
          count: 3,
          totalWastedBytes: 2048,
        },
      ],
    });
  });

  it("leaves arrayReport undefined on older artifacts without array_report (no fabricated zeroes)", () => {
    const parsed = parseAnalysisArtifact(buildArtifactWithOptionalAnalyzers());
    expect(parsed.arrayReport).toBeUndefined();
  });
});
