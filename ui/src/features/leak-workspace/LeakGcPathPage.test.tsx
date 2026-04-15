import "../../test/setup";

import { act, cleanup, render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakGcPathPage } from "./LeakGcPathPage";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

function seedArtifact(heapPath = "fixture.hprof") {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath,
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

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });

  return { promise, resolve };
}

describe("LeakGcPathPage", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?: unknown;
    };
  };

  function clearLeakWorkspaceBridge() {
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
    }
  }

  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });

    clearLeakWorkspaceBridge();
  });

  afterEach(() => {
    cleanup();

    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });

    clearLeakWorkspaceBridge();
  });

  it("renders gc-path unavailable when no object target exists", () => {
    seedArtifact();

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText(/gc path is unavailable for this leak until an object target is present/i)).toBeInTheDocument();
  });

  it("renders gc-path unavailable when the host bridge is absent", async () => {
    seedArtifact();

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/gc path unavailable: local gc path bridge is unavailable\./i)).toBeInTheDocument();
  });

  it("renders a bridge-backed gc path with root and edge details", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    seedArtifact();
    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => ({
        object_id: "0x0000000000001000",
        path_length: 2,
        path: [
          {
            object_id: "0x0000000000000001",
            class_name: "java.lang.Thread",
            field: "ROOT",
            is_root: true,
          },
          {
            object_id: "0x0000000000001000",
            class_name: "com.example.Cache",
            field: "entries",
            is_root: false,
          },
        ],
        provenance: [],
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/current object target: 0x1000/i)).toBeInTheDocument();
    expect(view.getByText(/root node/i)).toBeInTheDocument();
    expect(view.getByText(/via: entries/i)).toBeInTheDocument();
  });

  it("renders fallback provenance details for a bridge-backed fallback gc path", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    seedArtifact();
    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async () => ({
        object_id: "0x1000",
        path_length: 2,
        path: [
          {
            object_id: "GC_ROOT_thread",
            class_name: "java.lang.Thread",
            field: "ROOT",
            is_root: true,
          },
          {
            object_id: "0x1000",
            class_name: "com.example.Cache",
            field: "entries",
            is_root: false,
          },
        ],
        provenance: [
          { kind: "SYNTHETIC", detail: "GC path was synthesized from summary-level heap information." },
          { kind: "FALLBACK", detail: "No real GC root chain could be resolved; best-effort fallback path returned." },
        ],
      }),
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/gc path includes backend-reported fallback provenance\./i)).toBeInTheDocument();
    expect(view.getByText(/gc path was synthesized from summary-level heap information\./i)).toBeInTheDocument();
    expect(view.getByText(/no real gc root chain could be resolved; best-effort fallback path returned\./i)).toBeInTheDocument();
  });

  it("refreshes the current gc path when the operator requests it", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    seedArtifact();
    const calls: string[] = [];
    const user = userEvent.setup();
    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: async ({ objectId }) => {
        if (!objectId) {
          throw new Error("Expected objectId for refresh test.");
        }

        calls.push(objectId);
        return {
          object_id: objectId,
          path_length: 1,
          path: [
            {
              object_id: objectId,
              class_name: "com.example.Cache",
              field: "ROOT",
              is_root: true,
            },
          ],
          provenance: [],
        };
      },
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await view.findByText(/current object target: 0x1000/i);
    await user.click(view.getByRole("button", { name: /refresh gc path/i }));

    expect(calls).toEqual(["0x1000", "0x1000"]);
  });

  it("does not show stale gc-path data while a new heap request is loading", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    const deferred = createDeferred<{
      object_id: string;
      path_length: number;
      path: Array<{ object_id: string; class_name: string; field: string; is_root: boolean }>;
      provenance: [];
    }>();

    seedArtifact("fixture.hprof");
    globalWindow.window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
      findGcPath: ({ heapPath }: { heapPath: string }) => {
        if (heapPath === "fixture.hprof") {
          return Promise.resolve({
            object_id: "0x1234",
            path_length: 1,
            path: [
              {
                object_id: "0x1234",
                class_name: "old.heap.Node",
                field: "ROOT",
                is_root: true,
              },
            ],
            provenance: [],
          });
        }

        return deferred.promise;
      },
    };

    act(() => {
      useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1234" });
    });

    const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
      initialEntries: ["/leaks/leak-1/gc-path"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/^old\.heap\.node$/i)).toBeInTheDocument();

    act(() => {
      seedArtifact("fixture-2.hprof");
    });

    expect(view.queryByText(/^old\.heap\.node$/i)).toBeNull();
    expect(await view.findByText(/loading gc path/i)).toBeInTheDocument();

    act(() => {
      deferred.resolve({
        object_id: "0x1234",
        path_length: 1,
        path: [
          {
            object_id: "0x1234",
            class_name: "new.heap.Node",
            field: "ROOT",
            is_root: true,
          },
        ],
        provenance: [],
      });
    });

    expect(await view.findByText(/^new\.heap\.node$/i)).toBeInTheDocument();
  });
});
