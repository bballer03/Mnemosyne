import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { dashboardRoutes } from "../../test/app-route-trees";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "./investigation-store";

function seedArtifact() {
  useArtifactStore.getState().setArtifact("fixture.json", {
    summary: {
      heapPath: "/secret/path/fixture.hprof",
      totalObjects: 42,
      totalSizeBytes: 2048,
      totalRecords: 2,
    },
    leaks: [
      {
        id: "leak-cache",
        className: "com.example.Cache",
        leakKind: "CACHE",
        severity: "HIGH",
        retainedSizeBytes: 1024,
        suspectScore: 0.9,
        instances: 4,
        description: "Cache retains request objects.",
        provenance: [{ kind: "FALLBACK", detail: "heuristic retained-size candidate" }],
      },
    ],
    recommendations: [],
    elapsedSeconds: 0,
    graph: {
      nodeCount: 4,
      edgeCount: 3,
      dominatorCount: 1,
      dominators: [
        {
          name: "com.example.Cache",
          className: "com.example.Cache",
          objectId: "0x2a",
          dominates: 3,
          retainedSize: 1024,
          shallowSize: 64,
        },
      ],
    },
    provenance: [],
  });
}

describe("FindingsAdvisoryPane", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.setState({
        revision: 0,
        objectId: undefined,
        classKey: undefined,
        leakId: undefined,
        originPane: undefined,
      });
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.getState().clearSelection();
    });
  });

  it("labels offline rules and fallback provenance without replacing artifact facts", () => {
    seedArtifact();
    const router = createMemoryRouter(dashboardRoutes(), { initialEntries: ["/dashboard"] });
    const view = render(<RouterProvider router={router} />);

    const pane = view.getByRole("region", { name: /findings advisory/i });
    expect(within(pane).getByText(/advisory provenance:\s*rules · offline/i)).toBeInTheDocument();
    expect(within(pane).getByText("FALLBACK")).toBeInTheDocument();
    expect(within(pane).getByText(/loaded artifact remains authoritative/i)).toBeInTheDocument();
    expect(pane.textContent ?? "").not.toContain("/secret/path");
  });

  it("collapses and expands the findings queue", async () => {
    const user = userEvent.setup();
    seedArtifact();
    const router = createMemoryRouter(dashboardRoutes(), { initialEntries: ["/dashboard"] });
    const view = render(<RouterProvider router={router} />);

    const toggle = view.getByRole("button", { name: /collapse findings advisory/i });
    await user.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(view.queryByText("Cache retains request objects.")).not.toBeInTheDocument();

    await user.click(view.getByRole("button", { name: /expand findings advisory/i }));
    expect(view.getByText("Cache retains request objects.")).toBeInTheDocument();
  });

  it("deep-links a finding and synchronizes stable leak, class, and object selection", async () => {
    const user = userEvent.setup();
    seedArtifact();
    const router = createMemoryRouter(dashboardRoutes(), { initialEntries: ["/dashboard"] });
    const view = render(<RouterProvider router={router} />);

    const pane = view.getByRole("region", { name: /findings advisory/i });
    const inspectLink = within(pane).getByRole("link", { name: /inspect com\.example\.cache/i });
    expect(inspectLink.getAttribute("href")).toBe(
      "/heap-explorer/object-inspector?objectId=0x2a",
    );
    expect(
      within(pane).getByRole("link", { name: /open leak workspace/i }).getAttribute("href"),
    ).toBe("/leaks/leak-cache/overview");
    expect(
      within(pane).getByRole("link", { name: /show class in histogram/i }).getAttribute("href"),
    ).toBe("/artifacts/explorer");
    expect(
      within(pane).getByRole("link", { name: /open full assistant/i }).getAttribute("href"),
    ).toBe("/assistant");

    await user.click(inspectLink);

    expect(router.state.location.pathname).toBe("/heap-explorer/object-inspector");
    expect(router.state.location.search).toBe("?objectId=0x2a");
    expect(useInvestigationStore.getState()).toMatchObject({
      leakId: "leak-cache",
      classKey: "com.example.Cache",
      objectId: "0x2a",
    });
  });
});
