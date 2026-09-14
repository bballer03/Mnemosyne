import "../../../test/setup";

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { StringAnalysisPanel } from "./StringAnalysisPanel";

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

describe("StringAnalysisPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when stringReport is missing", () => {
    const view = render(
      <MemoryRouter>
        <StringAnalysisPanel artifact={buildArtifact()} />
      </MemoryRouter>,
    );

    expect(view.getByText(/string analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders duplicate groups from a backend-shaped string_report", () => {
    const view = render(
      <MemoryRouter>
        <StringAnalysisPanel
          artifact={buildArtifact({
            stringReport: {
              totalStrings: 100,
              totalStringBytes: 4000,
              uniqueStrings: 80,
              totalDuplicateWaste: 512,
              duplicateGroups: [{ value: "leak-token", count: 4, totalWastedBytes: 256 }],
              topStringsBySize: [{ objectId: 0xabc, value: "big", byteLength: 128 }],
            },
          })}
        />
      </MemoryRouter>,
    );

    expect(view.getByText("leak-token")).toBeInTheDocument();
    expect(view.getByText(/4 copies/i)).toBeInTheDocument();
    expect(view.getByText(/0xabc/i)).toBeInTheDocument();
    expect(view.getByRole("link", { name: /0xabc/i }).getAttribute("href")).toContain("objectId=0xabc");
  });

  it("renders an explicit empty state when the report has no duplicate groups", () => {
    const view = render(
      <MemoryRouter>
        <StringAnalysisPanel
          artifact={buildArtifact({
            stringReport: {
              totalStrings: 10,
              totalStringBytes: 40,
              uniqueStrings: 10,
              totalDuplicateWaste: 0,
              duplicateGroups: [],
              topStringsBySize: [],
            },
          })}
        />
      </MemoryRouter>,
    );

    expect(view.getByText(/no duplicate groups/i)).toBeInTheDocument();
  });
});
