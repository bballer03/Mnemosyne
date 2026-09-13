import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { ReferrerPanel } from "./ReferrerPanel";

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

describe("ReferrerPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when referrerReport is missing from the artifact", () => {
    const view = render(<ReferrerPanel artifact={buildArtifact()} />);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/referrer analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders ranked referrer entries against a real backend-shaped artifact", () => {
    const artifact = buildArtifact({
      referrerReport: {
        entries: [
          {
            objectId: "0x00001000",
            className: "com/example/webapp/WebappLoader",
            retainedSize: 4,
            referrerCount: 1,
            topReferrerClasses: [["com/example/webapp/WebappLoader", 1]],
          },
          {
            objectId: "0x00002000",
            className: "com/example/webapp/WebappLoader",
            retainedSize: 4,
            referrerCount: 0,
            topReferrerClasses: [],
          },
        ],
        totalObjectsConsidered: 5,
      },
    });

    const view = render(<ReferrerPanel artifact={artifact} />);
    const table = view.getByRole("table");
    const rows = within(table).getAllByRole("row");

    // rows[0] is the header row; rows[1]/rows[2] are the two entries in report order.
    const firstRowCells = within(rows[1]).getAllByRole("cell");
    expect(within(firstRowCells[0]).getByText("com/example/webapp/WebappLoader")).toBeInTheDocument();
    expect(within(firstRowCells[0]).getByText("0x00001000")).toBeInTheDocument();
    expect(within(firstRowCells[1]).getByText("1")).toBeInTheDocument();
    expect(within(firstRowCells[3]).getByText("com/example/webapp/WebappLoader")).toBeInTheDocument();
    expect(within(firstRowCells[3]).getByText("x1")).toBeInTheDocument();

    const secondRowCells = within(rows[2]).getAllByRole("cell");
    expect(within(secondRowCells[0]).getByText("0x00002000")).toBeInTheDocument();
    expect(within(secondRowCells[3]).getByText("-")).toBeInTheDocument();

    expect(view.getByText(/5 objects considered/i)).toBeInTheDocument();
  });

  it("renders an explicit empty state when the report is present but has no entries", () => {
    const artifact = buildArtifact({
      referrerReport: { entries: [], totalObjectsConsidered: 0 },
    });

    const view = render(<ReferrerPanel artifact={artifact} />);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/reports no ranked entries/i)).toBeInTheDocument();
  });
});
