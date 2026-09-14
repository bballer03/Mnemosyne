import "../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, Outlet, RouterProvider, type RouteObject } from "react-router-dom";

import { POWER_ROUTES, TopNav } from "./TopNav";
import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";
import { useDashboardStore } from "../features/dashboard/dashboard-store";

/**
 * TopNav reachability must not mount the real app `routes` tree.
 * Importing / navigating the full router previously allocated multi-GB under
 * jsdom (observed ~14GB RSS for this file alone) and crashed WSL / CI.
 * A stub tree with TopNav + empty path shells still proves every power href
 * is linked and navigable from the landing surface.
 */
function stubRoutes(): RouteObject[] {
  const shells: RouteObject[] = POWER_ROUTES.filter((item) => item.to !== "/").map((item) => ({
    path: item.to,
    element: <div data-testid={`stub-${item.to}`}>{item.label}</div>,
  }));

  return [
    {
      path: "/",
      element: (
        <>
          <TopNav />
          <Outlet />
        </>
      ),
      children: [
        {
          index: true,
          element: <div data-testid="stub-home">Home</div>,
        },
      ],
    },
    ...shells,
    {
      path: "/leaks/:leakId/overview",
      element: <div data-testid="stub-leak-workspace">Leak Workspace</div>,
    },
  ];
}

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
    const router = createMemoryRouter(stubRoutes(), { initialEntries: ["/"] });
    const view = render(<RouterProvider router={router} />);
    const nav = within(view.container.querySelector('nav[aria-label="Power routes"]')!);

    for (const item of POWER_ROUTES) {
      expect(nav.getByRole("link", { name: item.label })).toBeInTheDocument();
      expect(nav.getByRole("link", { name: item.label }).getAttribute("href")).toBe(item.to);
    }
  });

  it("actually navigates to every power route when its link is clicked", async () => {
    const user = userEvent.setup();
    const router = createMemoryRouter(stubRoutes(), { initialEntries: ["/"] });
    render(<RouterProvider router={router} />);

    for (const item of POWER_ROUTES) {
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
      const link = within(nav as HTMLElement).getByRole("link", { name: item.label });
      expect(link.getAttribute("href")).toBe(item.to);
      await user.click(link);
      await waitFor(
        () => {
          expect(router.state.location.pathname).toBe(item.to);
        },
        { timeout: 3000 },
      );
    }
  });

  it("does not show a leak-workspace link when no artifact has any leaks", () => {
    const router = createMemoryRouter(stubRoutes(), { initialEntries: ["/"] });
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

    const router = createMemoryRouter(stubRoutes(), { initialEntries: ["/"] });
    const view = render(<RouterProvider router={router} />);
    const nav = within(view.container.querySelector('nav[aria-label="Power routes"]')!);

    const link = nav.getByRole("link", { name: /leak workspace/i });
    await user.click(link);
    expect(router.state.location.pathname).toBe("/leaks/leak-high/overview");
  });
});
