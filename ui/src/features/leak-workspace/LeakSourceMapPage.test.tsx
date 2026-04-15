import "../../test/setup";

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { MemoryRouter, Route, Routes, createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakSourceMapPage } from "./LeakSourceMapPage";
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
        },
        provenance: [],
      },
    });
  });
}

describe("LeakSourceMapPage", () => {
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

  it("renders source-map unavailable when project root is missing", () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-1/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/source map is unavailable until a project root is configured/i)).toBeInTheDocument();
  });

  it("renders source-map unavailable when the host bridge is absent", async () => {
    seedArtifact();

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-1/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/source map unavailable: local source map bridge is unavailable\./i)).toBeInTheDocument();
  });

  it("renders bridge-backed source-map locations", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      mapToCode: async () => ({
        leak_id: "leak-1",
        locations: [
          {
            file: "D:/repo/src/main/java/com/example/Cache.java",
            line: 42,
            symbol: "com.example.Cache",
            code_snippet: "return cache;",
            git: { commit: "abc123" },
          },
        ],
      }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-1/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/d:\/repo\/src\/main\/java\/com\/example\/cache\.java/i)).toBeInTheDocument();
    expect(view.getByText(/git metadata: present/i)).toBeInTheDocument();
  });

  it("renders source-map fallback when mapping returns unmapped results", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      mapToCode: async () => ({
        leak_id: "leak-1",
        locations: [
          {
            file: ".mnemosyne/unmapped/com/example/Cache.java",
            line: 1,
            symbol: "com.example.Cache",
            code_snippet: "",
            git: null,
          },
        ],
      }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-1/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/mapping fell back to an unmapped placeholder/i)).toBeInTheDocument();
  });

  it("does not show stale source-map data from a previous leak while a new leak loads", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      mapToCode: async ({ leakId }: { leakId: string }) => ({
        leak_id: leakId,
        locations: [
          {
            file: ".mnemosyne/unmapped/com/example/Cache.java",
            line: 1,
            symbol: "com.example.Cache",
            code_snippet: "",
            git: null,
          },
        ],
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
      useLeakWorkspaceStore.getState().setSubviewState("sourceMap", {
        status: "ready",
        data: {
          leak_id: "leak-1",
          locations: [
            {
              file: "src/old/LeakOne.java",
              line: 12,
              symbol: "com.example.Cache",
              code_snippet: "return cache;",
              git: null,
            },
          ],
        },
      });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-2/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.queryByText(/src\/old\/LeakOne\.java/i)).not.toBeInTheDocument();
    expect(view.getByText(/loading source map/i)).toBeInTheDocument();

    await waitFor(() => {
      expect(view.getByText(/mapping fell back to an unmapped placeholder/i)).toBeInTheDocument();
    });

    expect(view.queryByText(/src\/old\/LeakOne\.java/i)).not.toBeInTheDocument();
  });

  it("does not show a stale unavailable banner from a previous leak before the next leak request starts", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      mapToCode: async ({ leakId }: { leakId: string }) => ({
        leak_id: leakId,
        locations: [
          {
            file: ".mnemosyne/unmapped/com/example/Cache.java",
            line: 1,
            symbol: "com.example.Cache",
            code_snippet: "",
            git: null,
          },
        ],
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
      useLeakWorkspaceStore.getState().setSubviewState("sourceMap", {
        status: "unavailable",
        error: "Repository source index is unavailable.",
      });
    });

    const container = document.createElement("div");
    const root = createRoot(container);

    flushSync(() => {
      root.render(
        <MemoryRouter initialEntries={["/leaks/leak-2/source-map"]}>
          <Routes>
            <Route path="/leaks/:leakId/source-map" element={<LeakSourceMapPage />} />
          </Routes>
        </MemoryRouter>,
      );
    });

    expect(container.textContent).not.toContain("Source map unavailable: Repository source index is unavailable.");
    expect(container.textContent).toContain("Loading source map...");

    flushSync(() => {
      root.unmount();
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
      initialEntries: ["/leaks/leak-2/source-map"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await waitFor(() => {
      expect(view.getByText(/mapping fell back to an unmapped placeholder/i)).toBeInTheDocument();
    });

    expect(view.queryByText(/source map unavailable: repository source index is unavailable\./i)).not.toBeInTheDocument();
  });
});
