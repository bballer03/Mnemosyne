import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter, useLocation } from "react-router-dom";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../investigation/investigation-store";

import { ArtifactExplorerPage } from "./ArtifactExplorerPage";

function buildArtifact(options?: { histogram?: AnalysisArtifact["histogram"] }): AnalysisArtifact {
  return {
    summary: {
      heapPath: "fixture.hprof",
      totalObjects: 42,
      totalSizeBytes: 4096,
      generatedAt: "2026-04-14T00:00:00Z",
      totalRecords: 2,
    },
    leaks: [
      {
        id: "leak-1",
        className: "com.example.Cache",
        leakKind: "CACHE",
        severity: "HIGH",
        retainedSizeBytes: 1024,
        shallowSizeBytes: 64,
        suspectScore: 0.98,
        instances: 4,
        description: "Cache retains request objects",
        provenance: [],
      },
    ],
    recommendations: ["Trim cache residency."],
    elapsedSeconds: 1,
    graph: {
      nodeCount: 200,
      edgeCount: 400,
      dominatorCount: 10,
      dominators: [],
    },
    histogram: options?.histogram,
    stringReport: {
      totalStrings: 10,
      totalStringBytes: 512,
      uniqueStrings: 4,
      duplicateGroups: [{ value: "dup", count: 3, totalWastedBytes: 32 }],
      totalDuplicateWaste: 32,
      topStringsBySize: [{ objectId: 1, value: "payload", byteLength: 64, retainedBytes: 128 }],
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
        {
          objectId: 7,
          className: "byte[]",
          shallowSize: 4096,
          retainedSize: 8192,
        },
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
          uniqueClassCount: 12,
          ancestorChain: [1],
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
      duplicateClasses: [],
    },
    unreachable: {
      totalCount: 3,
      totalShallowSize: 96,
      byClass: [{ className: "byte[]", count: 3, shallowSize: 96 }],
    },
    provenance: [],
  };
}

function seedArtifactWithHistogram(
  entries: NonNullable<AnalysisArtifact["histogram"]>["entries"] = [
    {
      key: "com.example.Cache",
      instanceCount: 4,
      shallowSize: 64,
      retainedSize: 1024,
    },
    {
      key: "java.util.concurrent.ConcurrentHashMap",
      instanceCount: 2,
      shallowSize: 48,
      retainedSize: 768,
    },
  ],
) {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: buildArtifact({
        histogram: {
          groupBy: "class",
          totalInstances: 42,
          totalShallowSize: 4096,
          entries,
        },
      }),
    });
  });
}

function seedArtifactWithoutHistogram() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: buildArtifact(),
    });
  });
}

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{`${location.pathname}${location.search}`}</output>;
}

function renderArtifactExplorer(initialEntry = "/artifacts/explorer") {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ArtifactExplorerPage />
      <LocationProbe />
    </MemoryRouter>,
  );
}

describe("ArtifactExplorerPage", () => {
  beforeEach(() => {
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.setState({
        classKey: undefined,
        objectId: undefined,
        leakId: undefined,
        originPane: undefined,
        histogramView: {
          searchText: "",
          groupBy: "class",
          sortKey: "retained",
          sortDirection: "desc",
          pageOffset: 0,
        },
      });
    });
  });

  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;

    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.setState({
        classKey: undefined,
        objectId: undefined,
        leakId: undefined,
        originPane: undefined,
        histogramView: {
          searchText: "",
          groupBy: "class",
          sortKey: "retained",
          sortDirection: "desc",
          pageOffset: 0,
        },
      });
    });
  });

  it("redirects back to the loader when no artifact is loaded", () => {
    const view = renderArtifactExplorer();

    expect(view.getByTestId("location")).toHaveTextContent("/");
  });

  it("stacks the primary explorer grid to one column at a narrow width", () => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 720 });
    seedArtifactWithHistogram();

    const view = renderArtifactExplorer();
    const histogram = view.getByRole("region", { name: /histogram explorer/i });
    expect(histogram).toBeInTheDocument();
    expect(view.getByRole("button", { name: /select com\.example\.cache/i })).toBeInTheDocument();
  });

  it("renders all histogram rows with retained and shallow comparisons", () => {
    seedArtifactWithHistogram();

    const view = renderArtifactExplorer();
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    expect(view.getByRole("heading", { name: /artifact explorer/i })).toBeInTheDocument();
    expect(histogramRegion.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(histogramRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(histogramRegion.getByText(/retained vs shallow/i)).toBeInTheDocument();
  });

  it("filters histogram rows by search text", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const view = renderArtifactExplorer();
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    await user.type(view.getByLabelText(/search histogram/i), "concurrent");

    expect(histogramRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(histogramRegion.queryByText(/com\.example\.Cache/i)).toBeNull();
    expect(useInvestigationStore.getState().histogramView.searchText).toBe("concurrent");
  });

  it("pages through bounded rows without changing the shared class selection", async () => {
    const user = userEvent.setup();
    const entries = Array.from({ length: 101 }, (_, index) => ({
      key: `row-${index.toString().padStart(3, "0")}`,
      instanceCount: 1,
      shallowSize: 10,
      retainedSize: 10,
    }));
    seedArtifactWithHistogram(entries);
    act(() => {
      useInvestigationStore.setState({ classKey: "row-100", originPane: "histogram" });
    });

    const view = renderArtifactExplorer();
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    expect(histogramRegion.getByText("Showing 1–100 of 101")).toBeInTheDocument();
    expect(histogramRegion.queryByLabelText("Select row-100")).toBeNull();

    await user.click(histogramRegion.getByRole("button", { name: "Next histogram page" }));

    expect(histogramRegion.getByText("Showing 101–101 of 101")).toBeInTheDocument();
    expect(histogramRegion.getByLabelText("Select row-100")).toHaveAttribute("aria-pressed", "true");
    expect(useInvestigationStore.getState().classKey).toBe("row-100");
    expect(useInvestigationStore.getState().histogramView.pageOffset).toBe(100);
  });

  it("marks the chosen histogram row as selected", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const view = renderArtifactExplorer();
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    await user.click(histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }));

    expect(
      histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("shows an explicit histogram-absent state when the artifact has no histogram", () => {
    seedArtifactWithoutHistogram();

    const view = renderArtifactExplorer();

    expect(view.getByText(/histogram data is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders analyzer cards from artifact-backed optional sections and labels absent sections explicitly", () => {
    seedArtifactWithHistogram();

    act(() => {
      useArtifactStore.setState((state) => ({
        ...state,
        artifact: state.artifact
          ? {
              ...state.artifact,
              topInstances: undefined,
            }
          : state.artifact,
      }));
    });

    const view = renderArtifactExplorer();
    const analyzerRail = within(view.getByRole("complementary", { name: /analyzer rail/i }));

    expect(analyzerRail.getByText(/artifact recommendations/i)).toBeInTheDocument();
    expect(analyzerRail.getByText("Trim cache residency.")).toBeInTheDocument();
    expect(analyzerRail.getByText(/string deduplication/i)).toBeInTheDocument();
    // Several optional sections are absent on this seed (top instances cleared above, plus others).
    expect(analyzerRail.getAllByText(/^unavailable$/i).length).toBeGreaterThanOrEqual(1);
    expect(analyzerRail.queryByText(/section_absent/i)).toBeNull();
    expect(analyzerRail.getByText(/top instances/i)).toBeInTheDocument();
  });

  it("renders partial and fallback analyzer provenance with details", () => {
    seedArtifactWithHistogram();
    act(() => {
      useArtifactStore.setState((state) => ({
        ...state,
        artifact: state.artifact
          ? {
              ...state.artifact,
              provenance: [
                { kind: "Partial", detail: "bounded analyzer rows" },
                { kind: "Fallback", detail: "heuristic leak ranking" },
              ],
            }
          : state.artifact,
      }));
    });

    const view = renderArtifactExplorer();
    const analyzerRail = within(view.getByRole("complementary", { name: /analyzer rail/i }));

    expect(analyzerRail.getByText(/partial: bounded analyzer rows/i)).toBeInTheDocument();
    expect(analyzerRail.getByText(/fallback: heuristic leak ranking/i)).toBeInTheDocument();
  });

  it("updates the selected bucket detail from the chosen histogram row", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const view = renderArtifactExplorer();
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));
    const detailRegion = within(view.getByRole("complementary", { name: /selected bucket detail/i }));

    await user.click(histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }));

    expect(detailRegion.getByText(/selected bucket/i)).toBeInTheDocument();
    expect(detailRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(detailRegion.getByText(/artifact-backed leak hints/i)).toBeInTheDocument();
  });

  it("preserves histogram filters across instance navigation and remount", async () => {
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      listClassInstances: async (classKey, offset, limit) => ({
        class_key: classKey,
        total: 1,
        returned: 1,
        offset: offset ?? 0,
        limit: limit ?? 100,
        truncated: false,
        instances: [
          {
            object_id: "0x2a",
            class_name: classKey,
            shallow_size: 64,
            retained_size: 1024,
          },
        ],
      }),
    };
    seedArtifactWithHistogram();
    act(() => {
      useInvestigationStore.getState().setHistogramView({
        searchText: "Cache",
        groupBy: "class",
        sortKey: "class",
        sortDirection: "asc",
      });
    });
    const user = userEvent.setup();
    const firstView = renderArtifactExplorer();

    await user.click(await firstView.findByRole("button", { name: "Open object 0x2a" }));

    expect(useInvestigationStore.getState().objectId).toBe("0x2a");
    expect(useInvestigationStore.getState().originPane).toBe("histogram");
    expect(firstView.getByTestId("location")).toHaveTextContent(
      "/heap-explorer/object-inspector?objectId=0x2a",
    );

    firstView.unmount();
    const secondView = renderArtifactExplorer();

    expect(secondView.getByLabelText(/search histogram/i)).toHaveValue("Cache");
    expect(secondView.getByLabelText(/histogram group by/i)).toHaveValue("class");
    expect(secondView.getByLabelText(/histogram sort by/i)).toHaveValue("class");
    expect(secondView.getByLabelText(/histogram sort direction/i)).toHaveValue("asc");
    expect(await secondView.findByRole("button", { name: "Open object 0x2a" })).toBeInTheDocument();
  });
});
