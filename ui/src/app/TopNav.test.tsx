import "../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "./router";
import { loadAnalysisArtifactFromText } from "../features/artifact-loader/load-analysis-artifact";
import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";
import { useDashboardStore } from "../features/dashboard/dashboard-store";

// Several power routes (`/dashboard`, `/artifacts/explorer`,
// `/heap-explorer/*`) redirect back to `/` when no artifact is loaded --
// pre-existing behavior in `DashboardPage`/`ArtifactExplorerPage`/
// `HeapExplorerLayout`, unrelated to this slice. A valid loaded artifact is
// required to actually verify each nav link's destination renders instead
// of bouncing straight back.
function fixtureArtifactJson() {
  return JSON.stringify({
    summary: {
      heap_path: "fixture.hprof",
      total_objects: 42,
      total_size_bytes: 2048,
      classes: [],
      generated_at: "2026-04-14T00:00:00Z",
      header: null,
      total_records: 2,
      record_stats: [],
    },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 1, nanos: 0 },
    graph: { node_count: 12, edge_count: 24, dominators: [] },
    histogram: {
      group_by: "class",
      entries: [],
      total_instances: 42,
      total_shallow_size: 2048,
    },
    provenance: [],
  });
}

const POWER_ROUTE_LABELS: Array<[label: string, path: string]> = [
  ["Home", "/"],
  ["Dashboard", "/dashboard"],
  ["Artifact Explorer", "/artifacts/explorer"],
  ["Dominators", "/heap-explorer/dominators"],
  ["Object Inspector", "/heap-explorer/object-inspector"],
  ["Query Console", "/heap-explorer/query-console"],
  ["Threads", "/heap-explorer/threads"],
  ["Compare", "/compare"],
  ["Policies", "/workbench/policies"],
  ["Snapshots", "/workbench/snapshots"],
  ["Flamegraphs", "/workbench/flamegraphs"],
];

describe("TopNav reachability", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useDashboardStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useDashboardStore.getState().reset();
    });
  });

  it("renders a nav link for every power route without any workflow step first", () => {
    const router = createMemoryRouter(routes, { initialEntries: ["/"] });
    const view = render(<RouterProvider router={router} />);
    const nav = within(view.container.querySelector('nav[aria-label="Power routes"]')!);

    for (const [label] of POWER_ROUTE_LABELS) {
      expect(nav.getByRole("link", { name: label })).toBeInTheDocument();
    }
  });

  it("actually navigates to every power route when its link is clicked", async () => {
    // `TopNav` is rendered only on `/` (see `ArtifactLoaderPage`'s and
    // `router.tsx`'s own comments on why it isn't a shared layout wrapping
    // every route), so each iteration below returns to `/` first -- this is
    // exactly the "reachable from the landing page" property this slice's
    // task brief asks to verify, not a persistent-across-every-page nav.
    const user = userEvent.setup();

    act(() => {
      useArtifactStore
        .getState()
        .setArtifact("fixture.json", loadAnalysisArtifactFromText(fixtureArtifactJson()));
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/"] });
    render(<RouterProvider router={router} />);

    for (const [label, path] of POWER_ROUTE_LABELS) {
      if (router.state.location.pathname !== "/") {
        act(() => {
          void router.navigate("/");
        });
        await waitFor(
          () => {
            expect(router.state.location.pathname).toBe("/");
          },
          { timeout: 3000 },
        );
      }

      const nav = document.querySelector('nav[aria-label="Power routes"]');
      expect(nav).not.toBeNull();
      const link = within(nav as HTMLElement).getByRole("link", { name: label });
      expect(link.getAttribute("href")).toBe(path);
      await user.click(link);
      await waitFor(
        () => {
          expect(router.state.location.pathname).toBe(path);
        },
        { timeout: 3000 },
      );
    }
  });

  it("does not show a leak-workspace link when no artifact has any leaks", () => {
    const router = createMemoryRouter(routes, { initialEntries: ["/"] });
    const view = render(<RouterProvider router={router} />);
    const nav = within(view.container.querySelector('nav[aria-label="Power routes"]')!);

    expect(nav.queryByRole("link", { name: /leak workspace/i })).not.toBeInTheDocument();
  });

  it("shows and navigates to a leak-workspace link once a leak context exists", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.getState().setArtifact("fixture.json", {
        summary: { heapPath: "fixture.hprof", totalObjects: 1, totalSizeBytes: 1, totalRecords: 1 },
        leaks: [
          {
            id: "leak-low",
            className: "com.example.Low",
            leakKind: "CACHE",
            severity: "LOW",
            retainedSizeBytes: 10,
            suspectScore: 0.2,
            instances: 1,
            description: "low",
            provenance: [],
          },
          {
            id: "leak-high",
            className: "com.example.High",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 100,
            suspectScore: 0.9,
            instances: 1,
            description: "high",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 0,
        graph: { nodeCount: 0, edgeCount: 0, dominatorCount: 0, dominators: [] },
        unreachable: { totalUnreachableObjects: 0, totalUnreachableBytes: 0, entries: [] },
        provenance: [],
      } as never);
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/"] });
    const view = render(<RouterProvider router={router} />);
    const nav = within(view.container.querySelector('nav[aria-label="Power routes"]')!);

    const link = nav.getByRole("link", { name: /leak workspace/i });
    await user.click(link);
    expect(router.state.location.pathname).toBe("/leaks/leak-high/overview");
  });
});
