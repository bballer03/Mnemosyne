import "../../../test/setup";

import { render, within } from "@testing-library/react";
import { describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { ObjectInspectorPanel } from "./ObjectInspectorPanel";

const artifact: AnalysisArtifact = {
  summary: {
    heapPath: "fixture.hprof",
    totalObjects: 42,
    totalSizeBytes: 2048,
    generatedAt: "2026-04-14T00:00:00Z",
    totalRecords: 2,
  },
  leaks: [],
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
        immediateDominator: "GC Root <system class>",
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

describe("ObjectInspectorPanel", () => {
  it("renders selected dominator row details from the artifact", () => {
    const view = render(<ObjectInspectorPanel artifact={artifact} selectedRowIndex={1} />);
    const panel = within(view.container);

    expect(panel.getByRole("heading", { name: /object inspector/i })).toBeInTheDocument();
    expect(panel.getByText(/com\.example\.jobs\.workerqueue/i)).toBeInTheDocument();
    expect(panel.getByText(/0xcafebabe/i)).toBeInTheDocument();
    expect(panel.getByText(/48 b/i)).toBeInTheDocument();
    expect(panel.getByText(/768 b/i)).toBeInTheDocument();
    expect(panel.getByText(/5 objects/i)).toBeInTheDocument();
    expect(panel.getByText(/com\.example\.cache\.lrucache@0xdeadbeef/i)).toBeInTheDocument();
    expect(panel.getByText(/live references and referrers are not yet available/i)).toBeInTheDocument();
  });

  it("renders an honest unselected state when no row is selected", () => {
    const view = render(<ObjectInspectorPanel artifact={artifact} selectedRowIndex={undefined} />);
    const panel = within(view.container);

    expect(panel.getByRole("heading", { name: /object inspector/i })).toBeInTheDocument();
    expect(panel.getByText(/select a dominator row to inspect its artifact-backed details/i)).toBeInTheDocument();
    expect(panel.getByText(/live references and referrers are not yet available/i)).toBeInTheDocument();
    expect(panel.queryByText(/0xdeadbeef/i)).toBeNull();
  });
});
