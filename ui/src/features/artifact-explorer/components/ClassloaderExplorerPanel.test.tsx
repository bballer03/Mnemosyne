import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { ClassloaderExplorerPanel } from "./ClassloaderExplorerPanel";

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
    ...overrides,
  };
}

describe("ClassloaderExplorerPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when classloaderReport is missing from the artifact", () => {
    const view = render(<ClassloaderExplorerPanel artifact={buildArtifact()} />);

    expect(view.getByText(/classloader analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders duplicate classes and per-loader ancestor chain/unique class count against a real backend-shaped artifact", () => {
    const artifact = buildArtifact({
      classloaderReport: {
        loaders: [
          {
            objectId: 4096,
            className: "com.example.webapp.WebappLoader",
            loadedClassCount: 1,
            instanceCount: 1,
            totalShallowBytes: 0,
            retainedBytes: 4,
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
      },
    });

    const view = render(<ClassloaderExplorerPanel artifact={artifact} />);

    const duplicatesSection = within(view.getByRole("region", { name: /duplicate classes across loaders/i }));
    expect(duplicatesSection.getByText("com.example.webapp.RequestHandler")).toBeInTheDocument();
    expect(duplicatesSection.getByText(/loaded by 2 loaders: 0x1000, 0x2000/)).toBeInTheDocument();

    const loadersSection = within(view.getByRole("region", { name: /^loaders$/i }));
    const table = loadersSection.getByRole("table");
    const rows = within(table).getAllByRole("row");
    // rows[0] is the header row; rows[1]/rows[2] are the two loader rows in
    // report order (4096, then 8192, whose ancestorChain is [4096]).
    expect(within(rows[1]).getByText("1 / 0")).toBeInTheDocument();
    expect(within(rows[2]).getByText("0x1000")).toBeInTheDocument();
  });

  it("renders explicit empty states when the report is present but empty", () => {
    const artifact = buildArtifact({
      classloaderReport: { loaders: [], potentialLeaks: [], duplicateClasses: [] },
    });

    const view = render(<ClassloaderExplorerPanel artifact={artifact} />);

    expect(
      view.getByText(/no class name was loaded by more than one distinct loader in this artifact/i),
    ).toBeInTheDocument();
    expect(view.getByText(/reports no loaders/i)).toBeInTheDocument();
  });
});
