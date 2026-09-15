import "../../test/setup";

import { act, render, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  DesktopOpenHeapMenuHandler,
  type DesktopOpenHeapMenuHost,
} from "./DesktopOpenHeapMenuHandler";

function analysisPayload() {
  return {
    summary: {
      heap_path: "menu.hprof",
      total_objects: 1,
      total_size_bytes: 8,
      classes: [],
      generated_at: "2026-09-16T00:00:00Z",
      header: null,
      total_records: 1,
      record_stats: [],
    },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 0, nanos: 0 },
    graph: { node_count: 1, edge_count: 0, dominators: [] },
  };
}

afterEach(() => {
  useArtifactStore.getState().reset();
  delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
});

describe("DesktopOpenHeapMenuHandler", () => {
  it("routes the native menu request through the opaque picker source id", async () => {
    let openFromMenu: (() => void) | undefined;
    let analyzedSourceId: string | undefined;
    const menuHost: DesktopOpenHeapMenuHost = {
      listen: async (listener) => {
        openFromMenu = listener;
        return () => {
          openFromMenu = undefined;
        };
      },
    };
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({
        status: "selected",
        sourceId: "opaque-menu-source",
        displayName: "menu.hprof",
      }),
      runDesktopAnalysis: async (input) => {
        analyzedSourceId = input.sourceId;
        return analysisPayload();
      },
    };
    const router = createMemoryRouter(
      [
        {
          element: (
            <>
              <DesktopOpenHeapMenuHandler menuHost={menuHost} />
              <Outlet />
            </>
          ),
          children: [
            { path: "/", element: <div>Home</div> },
            { path: "/dashboard", element: <div>Dashboard</div> },
          ],
        },
      ],
      { initialEntries: ["/"] },
    );
    const view = render(<RouterProvider router={router} />);

    await waitFor(() => expect(openFromMenu).toBeDefined());
    act(() => openFromMenu?.());

    await waitFor(() => {
      expect(within(view.container).getByText("Dashboard")).toBeInTheDocument();
    });
    expect(analyzedSourceId).toBe("opaque-menu-source");
    expect(useArtifactStore.getState().artifactName).toBe("menu.hprof");
    expect(view.container.textContent).not.toMatch(/[/\\](Users|home)[/\\]/i);
  });
});
