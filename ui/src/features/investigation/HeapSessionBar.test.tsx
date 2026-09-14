import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";
import { cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { clearRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import type { AnalysisArtifact } from "../../lib/analysis-types";
import { HeapSessionBar } from "./HeapSessionBar";

function minimalArtifact(name: string): AnalysisArtifact {
  return {
    summary: {
      heapPath: name,
      totalObjects: 10,
      totalRecords: 10,
      totalBytes: 100,
      classes: [],
      generatedAt: "2026-09-15T00:00:00Z",
      header: null,
      recordStats: [],
    },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 0, nanos: 0 },
    graph: { nodeCount: 1, edgeCount: 0, dominators: [] },
  } as AnalysisArtifact;
}

afterEach(() => {
  cleanup();
  useArtifactStore.getState().reset();
  clearRememberedDesktopHeapSource();
  delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
});

describe("HeapSessionBar", () => {
  it("is hidden when no heap is loaded", () => {
    const router = createMemoryRouter(
      [{ path: "/", element: <HeapSessionBar /> }],
      { initialEntries: ["/"] },
    );
    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    expect(page.queryByRole("region", { name: /current heap session/i })).toBeNull();
  });

  it("shows Open another and Close when a heap is loaded", () => {
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    const router = createMemoryRouter(
      [
        { path: "/", element: <HeapSessionBar /> },
        { path: "/dashboard", element: <div>Dashboard</div> },
      ],
      { initialEntries: ["/"] },
    );
    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    expect(page.getByRole("region", { name: /current heap session/i })).toBeTruthy();
    expect(page.getByRole("button", { name: /open another/i })).toBeTruthy();
    expect(page.getByRole("button", { name: /close/i })).toBeTruthy();
    expect(page.getByText("demo.hprof")).toBeTruthy();
  });

  it("Close clears the artifact and returns home", async () => {
    const user = userEvent.setup();
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    let unloaded = false;
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      unloadHeap: async () => {
        unloaded = true;
      },
    };
    const router = createMemoryRouter(
      [
        { path: "/", element: <div>Home page</div> },
        {
          path: "/dashboard",
          element: (
            <>
              <HeapSessionBar />
              <div>Dashboard</div>
            </>
          ),
        },
      ],
      { initialEntries: ["/dashboard"] },
    );
    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    await user.click(page.getByRole("button", { name: /close/i }));
    await waitFor(() => {
      expect(useArtifactStore.getState().artifact).toBeUndefined();
      expect(unloaded).toBe(true);
      expect(page.getByText("Home page")).toBeTruthy();
    });
  });
});
