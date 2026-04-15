import "../../test/setup";

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { MemoryRouter, Route, Routes, createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakExplainPage } from "./LeakExplainPage";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

function seedArtifact() {
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
          {
            id: "leak-2",
            className: "com.example.SessionRegistry",
            leakKind: "CACHE",
            severity: "MEDIUM",
            retainedSizeBytes: 512,
            shallowSizeBytes: 48,
            suspectScore: 0.76,
            instances: 2,
            description: "Session registry retains expired entries",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
          dominators: [],
        },
        provenance: [],
      },
    });
  });
}

describe("LeakExplainPage", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?: unknown;
    };
  };

  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });

    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
    }
  });

  afterEach(() => {
    cleanup();

    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });

    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
    }
  });

  it("renders explain unavailable when the host bridge is absent", async () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
      initialEntries: ["/leaks/leak-1/explain"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/loading explanation/i)).toBeInTheDocument();
    expect(await view.findByText(/explain unavailable: local explain bridge is unavailable\./i)).toBeInTheDocument();
  });

  it("renders explain loading and ready states when the host bridge is present", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      explainLeak: async () => ({
        leak_id: "leak-1",
        summary: "Bridge explanation.",
      }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
      initialEntries: ["/leaks/leak-1/explain"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/loading explanation/i)).toBeInTheDocument();
    expect(await view.findByText(/bridge explanation\./i)).toBeInTheDocument();
  });

  it("does not show stale explanation data from a previous leak while a new leak loads", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      explainLeak: async () => ({
        summary: "Bridge explanation.",
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSubviewState("explain", {
        status: "ready",
        data: {
          leak_id: "leak-1",
          summary: "Stale explanation for leak one.",
        },
      });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
      initialEntries: ["/leaks/leak-2/explain"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.queryByText(/stale explanation for leak one/i)).not.toBeInTheDocument();
    expect(view.getByText(/loading explanation/i)).toBeInTheDocument();

    await waitFor(() => {
      expect(view.getByText(/bridge explanation\./i)).toBeInTheDocument();
    });

    expect(view.queryByText(/stale explanation for leak one/i)).not.toBeInTheDocument();
  });

  it("does not show a stale fallback banner from a previous leak before the next leak request starts", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      explainLeak: async () => ({
        summary: "Bridge explanation.",
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSubviewState("explain", {
        status: "fallback",
        data: {
          leak_id: "leak-1",
          summary: "Fallback explanation for leak one.",
        },
      });
    });

    const container = document.createElement("div");
    const root = createRoot(container);

    flushSync(() => {
      root.render(
        <MemoryRouter initialEntries={["/leaks/leak-2/explain"]}>
          <Routes>
            <Route path="/leaks/:leakId/explain" element={<LeakExplainPage />} />
          </Routes>
        </MemoryRouter>,
      );
    });

    expect(container.textContent).not.toContain("backend-reported fallback provenance");
    expect(container.textContent).not.toContain("Fallback explanation for leak one.");
    expect(container.textContent).toContain("Loading explanation...");

    flushSync(() => {
      root.unmount();
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
      initialEntries: ["/leaks/leak-2/explain"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await waitFor(() => {
      expect(view.getByText(/bridge explanation\./i)).toBeInTheDocument();
    });

    expect(view.queryByText(/backend-reported fallback provenance/i)).not.toBeInTheDocument();
  });
});
