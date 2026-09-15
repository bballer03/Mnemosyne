import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useComparisonStore } from "./comparison-store";
import { ComparisonWorkbenchPanel } from "./ComparisonWorkbenchPanel";

function minimalArtifact(): AnalysisArtifact {
  return {
    summary: {
      heapPath: "current.hprof",
      totalObjects: 10,
      totalRecords: 10,
      totalSizeBytes: 100,
      generatedAt: "2026-09-15T00:00:00Z",
    },
    leaks: [],
    recommendations: [],
    elapsedSeconds: 0,
    graph: { nodeCount: 1, edgeCount: 0, dominatorCount: 0, dominators: [] },
    provenance: [],
  };
}

describe("ComparisonWorkbenchPanel", () => {
  beforeEach(() => {
    useArtifactStore.getState().reset();
    act(() => {
      useComparisonStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();
    useArtifactStore.getState().reset();
    act(() => {
      useComparisonStore.getState().reset();
    });
  });

  it("opens current-vs-baseline compare beside an active investigation", async () => {
    useArtifactStore.getState().setArtifact("current.hprof", minimalArtifact());
    const user = userEvent.setup();
    const view = render(
      <MemoryRouter>
        <ComparisonWorkbenchPanel variant="chrome" />
      </MemoryRouter>,
    );
    const page = within(view.container);

    expect(page.queryByLabelText(/comparison picker/i)).toBeNull();
    await user.click(page.getByRole("button", { name: /compare current to baseline/i }));

    expect(page.getByLabelText(/comparison picker/i)).toBeInTheDocument();
  });

  it("does not render chrome controls without an active artifact", () => {
    const view = render(
      <MemoryRouter>
        <ComparisonWorkbenchPanel variant="chrome" />
      </MemoryRouter>,
    );

    expect(
      within(view.container).queryByRole("button", { name: /compare current to baseline/i }),
    ).toBeNull();
  });

  it("keeps the standalone route adapter expanded without an active artifact", () => {
    const view = render(
      <MemoryRouter>
        <ComparisonWorkbenchPanel variant="route" />
      </MemoryRouter>,
    );

    expect(within(view.container).getByLabelText(/comparison picker/i)).toBeInTheDocument();
  });
});
