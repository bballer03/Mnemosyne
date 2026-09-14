import "../../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { HistogramExplorerPanel } from "./HistogramExplorerPanel";

function buildArtifact(overrides?: Partial<AnalysisArtifact>): AnalysisArtifact {
  return {
    summary: {
      heapPath: "fixture.hprof",
      totalObjects: 20,
      totalSizeBytes: 987,
      generatedAt: "2026-04-14T00:00:00Z",
      totalRecords: 20,
    },
    leaks: [],
    recommendations: [],
    elapsedSeconds: 1,
    graph: { nodeCount: 5, edgeCount: 1, dominatorCount: 0, dominators: [] },
    provenance: [],
    histogram: {
      groupBy: "class",
      totalInstances: 2,
      totalShallowSize: 40,
      entries: [
        { key: "java.lang.String", instanceCount: 1, shallowSize: 24, retainedSize: 100 },
        { key: "byte[]", instanceCount: 1, shallowSize: 16, retainedSize: 16 },
      ],
    },
    ...overrides,
  };
}

describe("HistogramExplorerPanel", () => {
  afterEach(() => {
    cleanup();
    delete globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("labels precomputed artifact grouping and exposes superclass option", () => {
    const view = render(
      <HistogramExplorerPanel
        artifact={buildArtifact()}
        selectedKey="java.lang.String"
        onSelectKey={() => undefined}
      />,
    );

    expect(view.getAllByText(/precomputed artifact grouping/i).length).toBeGreaterThan(0);
    const select = view.getByLabelText("Histogram group by") as HTMLSelectElement;
    expect(select.value).toBe("class");
    expect([...select.options].some((option) => option.value === "superclass")).toBe(true);
  });

  it("live-regroups via bridge and labels live source", async () => {
    globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      regroupHistogram: async (groupBy) => ({
        group_by: groupBy,
        total_instances: 1,
        total_shallow_size: 8,
        entries: [{ key: "java.lang.Object", instance_count: 1, shallow_size: 8, retained_size: 8 }],
      }),
    };

    let liveSource: "artifact" | "live" | undefined;
    const view = render(
      <HistogramExplorerPanel
        artifact={buildArtifact()}
        selectedKey="java.lang.String"
        onSelectKey={() => undefined}
        histogramSource="artifact"
        onLiveHistogramChange={(_histogram, source) => {
          liveSource = source;
        }}
      />,
    );

    fireEvent.change(view.getByLabelText("Histogram group by"), { target: { value: "superclass" } });

    await waitFor(() => {
      expect(liveSource).toBe("live");
    });
  });

  it("keeps artifact view and explains missing bridge when regroup is unavailable", () => {
    const view = render(
      <HistogramExplorerPanel
        artifact={buildArtifact()}
        selectedKey="java.lang.String"
        onSelectKey={() => undefined}
      />,
    );

    fireEvent.change(view.getByLabelText("Histogram group by"), { target: { value: "superclass" } });
    expect(view.getByRole("alert")).toHaveTextContent(/regroupHistogram/i);
    expect(view.getAllByText(/precomputed artifact grouping/i).length).toBeGreaterThan(0);
  });

  it("keeps flat superclass regroup when returned data has no parent relation", () => {
    const view = render(
      <HistogramExplorerPanel
        artifact={buildArtifact()}
        selectedKey="java.lang.Object"
        onSelectKey={() => undefined}
        histogramSource="live"
        liveHistogram={{
          groupBy: "superclass",
          totalInstances: 2,
          totalShallowSize: 32,
          entries: [
            { key: "java.lang.Object", instanceCount: 1, shallowSize: 16, retainedSize: 48 },
            { key: "java.util.AbstractList", instanceCount: 1, shallowSize: 16, retainedSize: 32 },
          ],
        }}
      />,
    );

    expect(view.getByText(/flat superclass list/i)).toBeTruthy();
    expect(view.queryByLabelText("Superclass hierarchy")).toBeNull();
    expect(view.queryByLabelText(/Expand /i)).toBeNull();
    expect(view.queryByLabelText(/Collapse /i)).toBeNull();
    const select = view.getByLabelText("Histogram group by") as HTMLSelectElement;
    expect([...select.options].find((option) => option.value === "superclass")?.textContent).toMatch(
      /flat list/i,
    );
  });

  it("offers expand/collapse only when returned data supplies deterministic parentKey links", () => {
    const view = render(
      <HistogramExplorerPanel
        artifact={buildArtifact()}
        selectedKey="java.lang.Object"
        onSelectKey={() => undefined}
        histogramSource="live"
        liveHistogram={{
          groupBy: "superclass",
          totalInstances: 3,
          totalShallowSize: 48,
          entries: [
            { key: "java.lang.Object", instanceCount: 1, shallowSize: 16, retainedSize: 80 },
            {
              key: "java.util.AbstractList",
              parentKey: "java.lang.Object",
              instanceCount: 1,
              shallowSize: 16,
              retainedSize: 48,
            },
            {
              key: "java.util.ArrayList",
              parentKey: "java.util.AbstractList",
              instanceCount: 1,
              shallowSize: 16,
              retainedSize: 16,
            },
          ],
        }}
      />,
    );

    expect(view.getByLabelText("Superclass hierarchy")).toBeTruthy();
    expect(view.getByText(/deterministic parent hierarchy/i)).toBeTruthy();
    const collapse = view.getByLabelText("Collapse java.lang.Object");
    fireEvent.click(collapse);
    expect(view.getByLabelText("Expand java.lang.Object")).toBeTruthy();
    expect(view.queryByLabelText("Select java.util.ArrayList")).toBeNull();
    fireEvent.click(view.getByLabelText("Expand java.lang.Object"));
    expect(view.getByLabelText("Select java.util.ArrayList")).toBeTruthy();
  });
});
