import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import { ArtifactLoaderPage } from "../artifact-loader/ArtifactLoaderPage";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { LeakWorkspaceLayout } from "../leak-workspace/LeakWorkspaceLayout";
import { LeakWorkspaceOverview } from "../leak-workspace/LeakWorkspaceOverview";

import { DashboardPage } from "./DashboardPage";

describe("DashboardPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  afterEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
    });
    cleanup();
  });

  it("renders summary metrics and top leak section from loaded artifact", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            generatedAt: "2026-04-14T00:00:00Z",
            totalRecords: 2,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 1024,
              shallowSizeBytes: 64,
              suspectScore: 0.98,
              instances: 4,
              description: "Cache retains request objects",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          histogram: {
            groupBy: "class",
            totalInstances: 42,
            totalShallowSize: 2048,
            entries: [],
          },
          provenance: [],
        },
      });
    });

    const view = render(<DashboardPage />);
    const page = within(view.container);

    expect(
      page.getByRole("heading", { name: /mnemosyne triage dashboard/i }),
    ).toBeInTheDocument();
    expect(page.getByText(/top leak suspects/i)).toBeInTheDocument();
    expect(page.getByText(/^fixture\.hprof$/i)).toBeInTheDocument();
    expect(page.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
  });

  it("shows an artifact-level provenance summary in the loaded artifact context", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            generatedAt: "2026-04-14T00:00:00Z",
            totalRecords: 2,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          provenance: [{ kind: "FALLBACK" }, { kind: "HEURISTIC" }],
        },
      });
    });

    const view = render(<DashboardPage />);

    expect(view.getByText(/provenance: fallback, heuristic/i)).toBeInTheDocument();
  });

  it("shows an explicit empty state when the loaded artifact has no leaks", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            generatedAt: "2026-04-14T00:00:00Z",
            totalRecords: 2,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          provenance: [],
        },
      });
    });

    const view = render(<DashboardPage />);

    expect(view.getByText(/no leak suspects detected\./i)).toBeInTheDocument();
  });

  it("stacks the dashboard body into one column on narrow screens", () => {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 720,
    });

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            generatedAt: "2026-04-14T00:00:00Z",
            totalRecords: 2,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          provenance: [],
        },
      });
    });

    const view = render(<DashboardPage />);
    const layoutSection = view.getByText(/graph metrics/i).closest("aside")?.parentElement;

    expect(layoutSection).toBeTruthy();
    expect(layoutSection?.getAttribute("style")).toContain("grid-template-columns: minmax(0, 1fr)");
  });

  it("redirects dashboard route access back to the loader when no artifact is loaded", () => {
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: <ArtifactLoaderPage />,
        },
        {
          path: "/dashboard",
          element: <DashboardPage />,
        },
      ],
      { initialEntries: ["/dashboard"] },
    );

    const view = render(<RouterProvider router={router} />);

    expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
    expect(view.queryByRole("heading", { name: /mnemosyne triage dashboard/i })).not.toBeInTheDocument();
  });

  it("opens the leak workspace overview from the dashboard trace action with an encoded leak id", async () => {
    const user = userEvent.setup();
    const leakId = "leak/needs encoding?#x";
    const encodedLeakId = encodeURIComponent(leakId);

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            totalRecords: 2,
          },
          leaks: [
            {
              id: leakId,
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 1024,
              shallowSizeBytes: 64,
              suspectScore: 0.98,
              instances: 4,
              description: "Cache retains request objects",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [
        { path: "/", element: <ArtifactLoaderPage /> },
        { path: "/dashboard", element: <DashboardPage /> },
        {
          path: "/leaks/:leakId",
          element: <LeakWorkspaceLayout />,
          children: [{ path: "overview", element: <LeakWorkspaceOverview /> }],
        },
      ],
      { initialEntries: ["/dashboard"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await user.click(view.getByRole("button", { name: /trace/i }));

    expect(router.state.location.pathname).toBe(`/leaks/${encodedLeakId}/overview`);
    expect(view.getByRole("link", { name: /back to dashboard/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /overview/i })).toBeInTheDocument();
    expect(view.getByText(/dependency readiness/i)).toBeInTheDocument();
  });

  it("opens the artifact explorer from the dashboard", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 42,
            totalSizeBytes: 2048,
            generatedAt: "2026-04-14T00:00:00Z",
            totalRecords: 2,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 200,
            edgeCount: 400,
            dominatorCount: 10,
          },
          histogram: {
            groupBy: "class",
            totalInstances: 42,
            totalShallowSize: 2048,
            entries: [
              {
                key: "com.example.Cache",
                instanceCount: 4,
                shallowSize: 64,
                retainedSize: 1024,
              },
            ],
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/dashboard"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await user.click(view.getByRole("link", { name: /artifact explorer/i }));

    expect(router.state.location.pathname).toBe("/artifacts/explorer");
    expect(view.getByRole("heading", { name: /artifact explorer/i })).toBeInTheDocument();
  });
});
