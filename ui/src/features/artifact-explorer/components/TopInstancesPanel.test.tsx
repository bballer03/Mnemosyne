import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { TopInstancesPanel } from "./TopInstancesPanel";

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

describe("TopInstancesPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when topInstances is missing", () => {
    const view = render(
      <MemoryRouter>
        <TopInstancesPanel artifact={buildArtifact()} />
      </MemoryRouter>,
    );

    expect(view.getByText(/top-instance analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("renders ranked instances from a backend-shaped top_instances section", () => {
    const view = render(
      <MemoryRouter>
        <TopInstancesPanel
          artifact={buildArtifact({
            topInstances: {
              totalCount: 1000,
              instances: [
                {
                  objectId: 0x42,
                  className: "com.example.Cache",
                  shallowSize: 128,
                  retainedSize: 4096,
                },
              ],
            },
          })}
        />
      </MemoryRouter>,
    );

    const table = view.getByRole("table");
    expect(within(table).getByText("com.example.Cache")).toBeInTheDocument();
    expect(within(table).getByText(/0x42/i)).toBeInTheDocument();
    expect(within(table).getByRole("link", { name: /0x42/i }).getAttribute("href")).toContain(
      "objectId=0x42",
    );
  });

  it("renders an explicit empty state when instances are empty", () => {
    const view = render(
      <MemoryRouter>
        <TopInstancesPanel
          artifact={buildArtifact({
            topInstances: { totalCount: 0, instances: [] },
          })}
        />
      </MemoryRouter>,
    );

    expect(view.getByText(/contains no ranked rows/i)).toBeInTheDocument();
  });
});
