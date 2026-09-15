import "../../test/setup";

import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import {
  clearRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../investigation/investigation-store";
import { runAnalyzerEnrichment } from "./analyzer-enrichment-client";

function currentArtifact(): AnalysisArtifact {
  return {
    summary: {
      heapPath: "current.hprof",
      totalObjects: 3,
      totalSizeBytes: 128,
      totalRecords: 1,
    },
    leaks: [],
    recommendations: [],
    elapsedSeconds: 0,
    graph: { nodeCount: 3, edgeCount: 2, dominatorCount: 0, dominators: [] },
    topInstances: { totalCount: 3, instances: [] },
    classloaderReport: {
      loaders: [],
      potentialLeaks: [],
      duplicateClasses: [],
    },
    provenance: [],
  };
}

function rawEnrichedArtifact(options?: {
  omitCollections?: boolean;
  provenance?: Array<{ kind: string; detail?: string }>;
}) {
  return {
    summary: {
      heap_path: "current.hprof",
      total_objects: 4,
      total_size_bytes: 256,
      total_records: 1,
    },
    leaks: [],
    recommendations: ["Inspect incoming references."],
    elapsed: { secs: 1, nanos: 0 },
    graph: {
      node_count: 4,
      edge_count: 3,
      dominator_count: 0,
      dominators: [],
    },
    string_report: {
      total_strings: 0,
      total_string_bytes: 0,
      unique_strings: 0,
      duplicate_groups: [],
      total_duplicate_waste: 0,
      top_strings_by_size: [],
    },
    ...(options?.omitCollections
      ? {}
      : {
          collection_report: {
            total_collections: 0,
            total_waste_bytes: 0,
            empty_collections: 0,
            oversized_collections: [],
            summary_by_type: {},
          },
        }),
    top_instances: { total_count: 4, instances: [] },
    classloader_report: {
      loaders: [],
      potential_leaks: [],
      duplicate_classes: [],
    },
    referrer_report: {
      entries: [],
      total_objects_considered: 4,
    },
    provenance: options?.provenance ?? [],
  };
}

describe("runAnalyzerEnrichment", () => {
  beforeEach(() => {
    useArtifactStore.getState().reset();
    useInvestigationStore.setState({
      workspaceId: "workspace-enrich",
      revision: 7,
      activeOperation: undefined,
    });
    useArtifactStore.getState().setArtifact("current.hprof", currentArtifact());
    rememberDesktopHeapSource("src-current", "current.hprof");
  });

  afterEach(() => {
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    clearRememberedDesktopHeapSource();
    useArtifactStore.getState().reset();
  });

  it("sends one accumulated host request and commits without changing revision", async () => {
    const calls: unknown[] = [];
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runDesktopAnalysis: async (input) => {
        calls.push(input);
        return rawEnrichedArtifact();
      },
    };

    const result = await runAnalyzerEnrichment({
      strings: true,
      collections: true,
      duplicateArrays: false,
      threads: false,
      referrers: true,
      classloaders: false,
    });

    expect(calls).toEqual([
      {
        sourceId: "src-current",
        mode: "custom",
        enableClassloaders: true,
        enableThreads: false,
        enableStrings: true,
        enableCollections: true,
        enableTopInstances: true,
        enableByReferrer: true,
        enableDuplicateArrays: false,
      },
    ]);
    expect(result).toMatchObject({ status: "ready", unavailable: [] });
    expect(useArtifactStore.getState().artifact?.summary.totalObjects).toBe(4);
    expect(useInvestigationStore.getState().revision).toBe(7);
  });

  it("rejects a response when the workspace revision changed while it was running", async () => {
    let resolve!: (value: unknown) => void;
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runDesktopAnalysis: () =>
        new Promise((resolvePromise) => {
          resolve = resolvePromise;
        }),
    };

    const pending = runAnalyzerEnrichment({
      strings: true,
      collections: false,
      duplicateArrays: false,
      threads: false,
      referrers: false,
      classloaders: false,
    });
    act(() => {
      useInvestigationStore.getState().bumpRevisionOnArtifactChange();
      useArtifactStore.getState().setArtifact("newer.hprof", currentArtifact());
    });
    resolve(rawEnrichedArtifact());

    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(useArtifactStore.getState().artifactName).toBe("newer.hprof");
    expect(useArtifactStore.getState().artifact?.summary.totalObjects).toBe(3);
  });

  it("returns unavailable without a remembered source or desktop bridge", async () => {
    clearRememberedDesktopHeapSource();
    await expect(
      runAnalyzerEnrichment({
        strings: true,
        collections: false,
        duplicateArrays: false,
        threads: false,
        referrers: false,
        classloaders: false,
      }),
    ).resolves.toMatchObject({ status: "unavailable" });

    rememberDesktopHeapSource("src-current", "current.hprof");
    await expect(
      runAnalyzerEnrichment({
        strings: true,
        collections: false,
        duplicateArrays: false,
        threads: false,
        referrers: false,
        classloaders: false,
      }),
    ).resolves.toMatchObject({ status: "unavailable" });
  });

  it("reports missing requested sections and preserves response provenance", async () => {
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runDesktopAnalysis: async () =>
        rawEnrichedArtifact({
          omitCollections: true,
          provenance: [
            { kind: "Partial", detail: "bounded analyzer output" },
            { kind: "Fallback", detail: "heuristic leak ranking" },
          ],
        }),
    };

    const result = await runAnalyzerEnrichment({
      strings: true,
      collections: true,
      duplicateArrays: false,
      threads: false,
      referrers: false,
      classloaders: false,
    });

    expect(result).toEqual({
      status: "ready",
      requested: ["Strings", "Collections"],
      unavailable: ["Collections"],
      provenance: [
        { kind: "Partial", detail: "bounded analyzer output" },
        { kind: "Fallback", detail: "heuristic leak ranking" },
      ],
    });
  });
});
