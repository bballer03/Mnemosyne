import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { DuplicateArrayPanel } from "./DuplicateArrayPanel";

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

describe("DuplicateArrayPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when arrayReport is missing from the artifact", () => {
    const view = render(<DuplicateArrayPanel artifact={buildArtifact()} />);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/duplicate-array analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders duplicate groups against a backend-shaped array_report", () => {
    const artifact = buildArtifact({
      arrayReport: {
        totalArrays: 12,
        uniqueContents: 8,
        totalDuplicateWaste: 4096,
        duplicateGroups: [
          {
            elementType: "byte",
            contentHash: 0xdeadbeef,
            length: 64,
            count: 3,
            totalWastedBytes: 2048,
          },
          {
            elementType: "int",
            contentHash: 0xcafebabe,
            length: 16,
            count: 2,
            totalWastedBytes: 128,
          },
        ],
      },
    });

    const view = render(<DuplicateArrayPanel artifact={artifact} />);
    const table = view.getByRole("table");
    const rows = within(table).getAllByRole("row");

    const firstRowCells = within(rows[1]).getAllByRole("cell");
    expect(within(firstRowCells[0]).getByText("byte")).toBeInTheDocument();
    expect(within(firstRowCells[1]).getByText("64")).toBeInTheDocument();
    expect(within(firstRowCells[2]).getByText("3")).toBeInTheDocument();

    expect(view.getByText(/2 duplicate groups/i)).toBeInTheDocument();
    expect(view.getByText(/12 arrays scanned/i)).toBeInTheDocument();
  });

  it("renders an explicit empty state when the report is present but has no groups", () => {
    const artifact = buildArtifact({
      arrayReport: {
        totalArrays: 4,
        uniqueContents: 4,
        duplicateGroups: [],
        totalDuplicateWaste: 0,
      },
    });

    const view = render(<DuplicateArrayPanel artifact={artifact} />);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/reports no duplicate groups/i)).toBeInTheDocument();
  });
});
