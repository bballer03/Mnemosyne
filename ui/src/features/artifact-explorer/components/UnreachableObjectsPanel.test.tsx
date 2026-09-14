import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { UnreachableObjectsPanel } from "./UnreachableObjectsPanel";

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

describe("UnreachableObjectsPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when unreachable is missing", () => {
    const view = render(<UnreachableObjectsPanel artifact={buildArtifact()} />);

    expect(view.getByText(/unreachable-object analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders by-class rows from a backend-shaped unreachable section", () => {
    const view = render(
      <UnreachableObjectsPanel
        artifact={buildArtifact({
          unreachable: {
            totalCount: 12,
            totalShallowSize: 2048,
            byClass: [{ className: "byte[]", count: 8, shallowSize: 1024 }],
          },
        })}
      />,
    );

    const table = view.getByRole("table");
    expect(within(table).getByText("byte[]")).toBeInTheDocument();
    expect(within(table).getByText("8")).toBeInTheDocument();
    expect(view.getByText(/12 unreachable objects/i)).toBeInTheDocument();
  });

  it("renders an explicit empty state when byClass is empty", () => {
    const view = render(
      <UnreachableObjectsPanel
        artifact={buildArtifact({
          unreachable: { totalCount: 0, totalShallowSize: 0, byClass: [] },
        })}
      />,
    );

    expect(view.getByText(/zero class rows/i)).toBeInTheDocument();
  });
});
