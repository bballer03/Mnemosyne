import "../../../test/setup";

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { PluginFindingsPanel } from "./PluginFindingsPanel";

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

describe("PluginFindingsPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows a clean absence state when pluginResults are missing", () => {
    const view = render(<PluginFindingsPanel artifact={buildArtifact()} />);
    expect(view.container.textContent).toContain(
      "plugin_results were serialized into this artifact",
    );
  });

  it("renders plugin findings as provenance-bearing display text", () => {
    const view = render(
      <PluginFindingsPanel
        artifact={buildArtifact({
          pluginResults: [
            {
              name: "demo-leak-name-analyzer",
              findings: [
                {
                  summary: "Found a suspicious cache",
                  severity: "warning",
                  detail: "Retained by static field Holder.CACHE",
                },
              ],
            },
          ],
        })}
      />,
    );

    expect(view.getByText("demo-leak-name-analyzer")).toBeInTheDocument();
    expect(view.getByText("Found a suspicious cache")).toBeInTheDocument();
    expect(view.getByText("warning")).toBeInTheDocument();
    expect(view.getByText(/Retained by static field Holder.CACHE/)).toBeInTheDocument();
    expect(view.getByText(/untrusted display strings/i)).toBeInTheDocument();
  });
});
