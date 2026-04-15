import "../../test/setup";

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakWorkspaceOverview } from "./LeakWorkspaceOverview";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

function seedArtifact(overrides?: Partial<ReturnType<typeof buildLeak>>) {
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
          buildLeak(overrides),
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

function buildLeak(overrides?: Partial<{
  id: string;
  className: string;
  leakKind: string;
  severity: string;
  retainedSizeBytes: number;
  shallowSizeBytes: number;
  suspectScore: number;
  instances: number;
  description: string;
  provenance: Array<{ kind: string; detail?: string }>;
}>) {
  return {
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
    ...overrides,
  };
}

describe("LeakWorkspaceOverview", () => {
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

  it("renders artifact-backed leak summary and preview regions", () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /overview/i })).toBeInTheDocument();
    expect(view.getByText(/cache retains request objects/i)).toBeInTheDocument();
    expect(view.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(view.getByText(/fixture\.hprof/i)).toBeInTheDocument();
    expect(view.getByText(/dependency readiness/i)).toBeInTheDocument();
    expect(view.getByText(/project root: missing/i)).toBeInTheDocument();
    expect(view.getByText(/bridge: unavailable/i)).toBeInTheDocument();
    expect(view.getByText(/provider: unavailable/i)).toBeInTheDocument();
    expect(view.getByText(/explain preview/i)).toBeInTheDocument();
    expect(view.getByText(/gc path preview/i)).toBeInTheDocument();
    expect(view.getByText(/source map preview/i)).toBeInTheDocument();
    expect(view.getByText(/fix proposal preview/i)).toBeInTheDocument();
  });

  it("shows gc path as unavailable when no object target exists", () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/gc path unavailable until an object target is present/i)).toBeInTheDocument();
  });

  it("keeps gc path unavailable when provenance strings imply data the artifact contract does not prove", () => {
    seedArtifact({
      provenance: [{ kind: "OBJECT_TARGET", detail: "objectId=0x1234" }],
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/object target: missing/i)).toBeInTheDocument();
    expect(view.getByText(/project root: missing/i)).toBeInTheDocument();
    expect(view.getByText(/provider: unavailable/i)).toBeInTheDocument();
    expect(view.getByText(/gc path unavailable until an object target is present/i)).toBeInTheDocument();
  });

  it("shows project root and object target as present when the workspace store has been activated", () => {
    seedArtifact();

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({
        leakId: "leak-1",
        heapPath: "fixture.hprof",
        projectRoot: "D:/repo",
        objectId: "0x1234",
      });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/project root: present/i)).toBeInTheDocument();
    expect(view.getByText(/object target: present/i)).toBeInTheDocument();
    expect(view.getByText(/gc path can be loaded from a concrete object target/i)).toBeInTheDocument();
  });

  it("shows bridge ready and provider ready when the host bridge reports those capabilities", () => {
    seedArtifact();

    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      capabilities: { provider: "ready" },
      explainLeak: async () => ({ summary: "Bridge explanation." }),
      mapToCode: async () => ({ leak_id: "leak-1", locations: [] }),
      proposeFix: async () => ({ suggestions: [] }),
    };

    const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
      initialEntries: ["/leaks/leak-1/overview"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/bridge: ready/i)).toBeInTheDocument();
    expect(view.getByText(/provider: ready/i)).toBeInTheDocument();
  });

  it("uses the exported app routes to render the overview route replacement", async () => {
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByRole("heading", { name: /overview/i })).toBeInTheDocument();
    expect(view.getByText(/dependency readiness/i)).toBeInTheDocument();
    expect(view.getByText(/gc path unavailable until an object target is present/i)).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/leaks/leak-1/overview");
  });
});
