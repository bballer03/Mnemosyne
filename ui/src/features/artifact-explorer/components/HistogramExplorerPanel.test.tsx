import "../../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";
import { useState } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";
import type { HistogramViewState } from "../../investigation/investigation-store";

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

const defaultHistogramView: HistogramViewState = {
  searchText: "",
  groupBy: "class",
  sortKey: "retained",
  sortDirection: "desc",
  pageOffset: 0,
};

const staticHistogramViewProps = {
  histogramView: defaultHistogramView,
  setHistogramView: () => undefined,
};

function ControlledHistogramExplorerPanel({
  artifact,
  initialView,
  selectedKey,
  onSelectKey = () => undefined,
}: {
  artifact: AnalysisArtifact;
  initialView?: Partial<HistogramViewState>;
  selectedKey?: string;
  onSelectKey?: (key: string | undefined) => void;
}) {
  const [histogramView, setView] = useState<HistogramViewState>({
    ...defaultHistogramView,
    ...initialView,
  });

  return (
    <HistogramExplorerPanel
      artifact={artifact}
      selectedKey={selectedKey}
      onSelectKey={onSelectKey}
      histogramView={histogramView}
      setHistogramView={(patch) => {
        setView((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function selectedRowKeys(view: ReturnType<typeof render>) {
  return view
    .getAllByRole("button", { name: /^Select / })
    .map((button) => button.getAttribute("aria-label")?.replace(/^Select /, ""));
}

describe("HistogramExplorerPanel", () => {
  afterEach(() => {
    cleanup();
    delete globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("labels precomputed artifact grouping and exposes superclass option", () => {
    const view = render(
      <HistogramExplorerPanel
        {...staticHistogramViewProps}
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
        {...staticHistogramViewProps}
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
        {...staticHistogramViewProps}
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
        {...staticHistogramViewProps}
        histogramView={{ ...defaultHistogramView, groupBy: "superclass" }}
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
        {...staticHistogramViewProps}
        histogramView={{ ...defaultHistogramView, groupBy: "superclass" }}
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

  it("sorts every supported histogram metric in both directions with key tie-breaking", () => {
    const artifact = buildArtifact({
      histogram: {
        groupBy: "class",
        totalInstances: 6,
        totalShallowSize: 60,
        entries: [
          { key: "beta", instanceCount: 3, shallowSize: 10, retainedSize: 100 },
          { key: "gamma", instanceCount: 1, shallowSize: 20, retainedSize: 50 },
          { key: "alpha", instanceCount: 2, shallowSize: 30, retainedSize: 100 },
        ],
      },
    });
    const view = render(<ControlledHistogramExplorerPanel artifact={artifact} />);
    const sortBy = view.getByLabelText("Histogram sort by");
    const direction = view.getByLabelText("Histogram sort direction");

    const expectedOrders = [
      ["class", "asc", ["alpha", "beta", "gamma"]],
      ["class", "desc", ["gamma", "beta", "alpha"]],
      ["instances", "asc", ["gamma", "alpha", "beta"]],
      ["instances", "desc", ["beta", "alpha", "gamma"]],
      ["shallow", "asc", ["beta", "gamma", "alpha"]],
      ["shallow", "desc", ["alpha", "gamma", "beta"]],
      ["retained", "asc", ["gamma", "alpha", "beta"]],
      ["retained", "desc", ["alpha", "beta", "gamma"]],
    ] as const;

    for (const [sortKey, sortDirection, expected] of expectedOrders) {
      fireEvent.change(sortBy, { target: { value: sortKey } });
      fireEvent.change(direction, { target: { value: sortDirection } });
      expect(selectedRowKeys(view)).toEqual(expected);
    }
  });

  it("filters before pagination and mounts at most 100 flat histogram rows", async () => {
    const user = userEvent.setup();
    const matchingEntries = Array.from({ length: 150 }, (_, index) => ({
      key: `match-${index.toString().padStart(3, "0")}`,
      instanceCount: 1,
      shallowSize: index + 1,
      retainedSize: index + 1,
    }));
    const otherEntries = Array.from({ length: 850 }, (_, index) => ({
      key: `other-${index.toString().padStart(3, "0")}`,
      instanceCount: 1,
      shallowSize: index + 1,
      retainedSize: index + 1,
    }));
    const artifact = buildArtifact({
      histogram: {
        groupBy: "class",
        totalInstances: 1_000,
        totalShallowSize: 1_000,
        entries: [...otherEntries, ...matchingEntries].reverse(),
      },
    });
    const view = render(
      <ControlledHistogramExplorerPanel
        artifact={artifact}
        initialView={{ sortKey: "class", sortDirection: "asc" }}
      />,
    );

    await user.type(view.getByLabelText("Search histogram"), "match-");

    expect(view.getByText("Showing 1–100 of 150")).toBeTruthy();
    expect(selectedRowKeys(view)).toHaveLength(100);
    expect(selectedRowKeys(view)[0]).toBe("match-000");
    expect(view.queryByLabelText("Select match-100")).toBeNull();

    fireEvent.click(view.getByRole("button", { name: "Next histogram page" }));

    expect(view.getByText("Showing 101–150 of 150")).toBeTruthy();
    expect(selectedRowKeys(view)).toHaveLength(50);
    expect(selectedRowKeys(view)[0]).toBe("match-100");
  });
});
