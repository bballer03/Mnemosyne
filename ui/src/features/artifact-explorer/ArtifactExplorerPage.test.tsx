import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";

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
    unreachable: {
      totalCount: 3,
      totalShallowSize: 96,
      byClass: [{ className: "byte[]", count: 3, shallowSize: 96 }],
    },
    provenance: [],
  };
}

function seedArtifactWithHistogram() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: buildArtifact({
        histogram: {
          groupBy: "class",
          totalInstances: 42,
          totalShallowSize: 4096,
          entries: [
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

describe("ArtifactExplorerPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();

    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  it("redirects back to the loader when no artifact is loaded", () => {
    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
  });

  it("renders all histogram rows with retained and shallow comparisons", () => {
    seedArtifactWithHistogram();

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    expect(view.getByRole("heading", { name: /artifact explorer/i })).toBeInTheDocument();
    expect(histogramRegion.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(histogramRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(histogramRegion.getByText(/retained vs shallow/i)).toBeInTheDocument();
  });

  it("filters histogram rows by search text", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    await user.type(view.getByLabelText(/search histogram/i), "concurrent");

    expect(histogramRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(histogramRegion.queryByText(/com\.example\.Cache/i)).toBeNull();
  });

  it("marks the chosen histogram row as selected", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));

    await user.click(histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }));

    expect(
      histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("shows an explicit histogram-absent state when the artifact has no histogram", () => {
    seedArtifactWithoutHistogram();

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

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

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const analyzerRail = within(view.getByRole("complementary", { name: /analyzer rail/i }));

    expect(analyzerRail.getByText(/artifact recommendations/i)).toBeInTheDocument();
    expect(analyzerRail.getByText(/string deduplication/i)).toBeInTheDocument();
    expect(analyzerRail.getByText(/section_absent/i)).toBeInTheDocument();
  });

  it("updates the selected bucket detail from the chosen histogram row", async () => {
    const user = userEvent.setup();
    seedArtifactWithHistogram();

    const router = createMemoryRouter(routes, { initialEntries: ["/artifacts/explorer"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const histogramRegion = within(view.getByRole("region", { name: /histogram explorer/i }));
    const detailRegion = within(view.getByRole("complementary", { name: /selected bucket detail/i }));

    await user.click(histogramRegion.getByRole("button", { name: /select java\.util\.concurrent\.ConcurrentHashMap/i }));

    expect(detailRegion.getByText(/selected bucket/i)).toBeInTheDocument();
    expect(detailRegion.getByText(/java\.util\.concurrent\.ConcurrentHashMap/i)).toBeInTheDocument();
    expect(detailRegion.getByText(/artifact-backed leak hints/i)).toBeInTheDocument();
  });
});
