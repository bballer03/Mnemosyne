import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { CollectionAnalysisPanel } from "./CollectionAnalysisPanel";

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

describe("CollectionAnalysisPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when collectionReport is missing", () => {
    const view = render(
      <MemoryRouter>
        <CollectionAnalysisPanel artifact={buildArtifact()} />
      </MemoryRouter>,
    );

    expect(view.getByText(/collection analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders oversized collections from a backend-shaped collection_report", () => {
    const view = render(
      <MemoryRouter>
        <CollectionAnalysisPanel
          artifact={buildArtifact({
            collectionReport: {
              totalCollections: 40,
              totalWasteBytes: 8192,
              emptyCollections: 3,
              oversizedCollections: [
                {
                  objectId: 0x101,
                  collectionType: "java.util.ArrayList",
                  size: 2,
                  capacity: 64,
                  wasteBytes: 4096,
                  shallowBytes: 512,
                },
              ],
              summaryByType: {},
            },
          })}
        />
      </MemoryRouter>,
    );

    const table = view.getByRole("table");
    expect(within(table).getByText("java.util.ArrayList")).toBeInTheDocument();
    expect(within(table).getByText(/0x101/i)).toBeInTheDocument();
    expect(view.getByText(/40 collections/i)).toBeInTheDocument();
  });

  it("renders an explicit empty state when oversized rows are empty", () => {
    const view = render(
      <MemoryRouter>
        <CollectionAnalysisPanel
          artifact={buildArtifact({
            collectionReport: {
              totalCollections: 2,
              totalWasteBytes: 0,
              emptyCollections: 2,
              oversizedCollections: [],
              summaryByType: {},
            },
          })}
        />
      </MemoryRouter>,
    );

    expect(view.getByText(/no oversized collections/i)).toBeInTheDocument();
  });
});
