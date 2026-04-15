import "../../test/setup";

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakFixPage } from "./LeakFixPage";
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

describe("LeakFixPage", () => {
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

  it("renders fix unavailable when the host bridge is absent", async () => {
    seedArtifact();
    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
      initialEntries: ["/leaks/leak-1/fix"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/fix proposal unavailable: local fix bridge is unavailable\./i)).toBeInTheDocument();
  });

  it("renders fix unavailable copy when project root is missing", async () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
      initialEntries: ["/leaks/leak-1/fix"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/fix proposal unavailable: required local context is missing\./i)).toBeInTheDocument();
  });

  it("renders bridge-backed fix results when the host bridge returns a ready payload", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      proposeFix: async () => ({
        suggestions: [
          {
            target_file: "D:/repo/src/main/java/com/example/Cache.java",
            description: "Release entries sooner.",
            diff: "@@ -1 +1 @@",
            confidence: 0.8,
            style: "Minimal",
          },
        ],
      }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
      initialEntries: ["/leaks/leak-1/fix"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/target file: d:\/repo\/src\/main\/java\/com\/example\/cache\.java/i)).toBeInTheDocument();
    expect(view.queryByText(/fallback: provider-backed generation was unavailable/i)).not.toBeInTheDocument();
  });

  it("renders fix fallback guidance when the host bridge returns fallback provenance", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
    });

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      proposeFix: async () => ({
        suggestions: [
          {
            target_file: "D:/repo/src/main/java/com/example/Cache.java",
            description: "Fallback fix guidance.",
            diff: "@@ -1 +1 @@",
            confidence: 0.42,
            style: "Minimal",
          },
        ],
        provenance: [{ kind: "FALLBACK", detail: "Provider unavailable." }],
      }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
      initialEntries: ["/leaks/leak-1/fix"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/fallback: provider-backed generation was unavailable, showing heuristic guidance\./i)).toBeInTheDocument();
  });

  it("does not flash stale fix state when the same leak is requested under a different project root", async () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      proposeFix: async () => ({
        suggestions: [
          {
            target_file: "D:/repo/src/main/java/com/example/Cache.java",
            description: "Fallback fix guidance.",
            diff: "@@ -1 +1 @@",
            confidence: 0.42,
            style: "Minimal",
          },
        ],
        provenance: [{ kind: "FALLBACK", detail: "Provider unavailable." }],
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo-a" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/fix", element: <LeakFixPage /> }], {
      initialEntries: ["/leaks/leak-1/fix"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/fallback: provider-backed generation was unavailable, showing heuristic guidance\./i)).toBeInTheDocument();

    act(() => {
      useLeakWorkspaceStore.getState().setSubviewState("fix", {
        status: "error",
        error: "Stale root A error",
      });
    });

    expect(await view.findByText(/fix proposal failed: stale root a error/i)).toBeInTheDocument();

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo-b" });
    });

    expect(view.queryByText(/fix proposal failed: stale root a error/i)).not.toBeInTheDocument();
    expect(view.getByText(/loading fix proposal/i)).toBeInTheDocument();
    expect(await view.findByText(/fallback: provider-backed generation was unavailable, showing heuristic guidance\./i)).toBeInTheDocument();
  });
});
