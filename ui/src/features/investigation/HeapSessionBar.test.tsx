import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";
import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { clearRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import type { AnalysisArtifact } from "../../lib/analysis-types";
import { HeapSessionBar } from "./HeapSessionBar";
import { useInvestigationStore } from "./investigation-store";

function minimalArtifact(name: string): AnalysisArtifact {
  return {
    summary: {
      heapPath: name,
      totalObjects: 10,
      totalRecords: 10,
      totalSizeBytes: 100,
      generatedAt: "2026-09-15T00:00:00Z",
    },
    leaks: [],
    recommendations: [],
    elapsedSeconds: 0,
    graph: { nodeCount: 1, edgeCount: 0, dominatorCount: 0, dominators: [] },
    provenance: [],
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

afterEach(() => {
  cleanup();
  useArtifactStore.getState().reset();
  useInvestigationStore.setState({
    workspaceId: "workspace-1",
    revision: 0,
    activeOperation: undefined,
    activeWorkflow: undefined,
    workflowNeedsRecovery: false,
    workspaceRequests: {},
  });
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

  it("shows the active workflow kind and step without its id or heap path", () => {
    useArtifactStore.getState().setArtifact(
      "demo.hprof",
      minimalArtifact("/secret/heaps/demo.hprof"),
    );
    const request = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    useInvestigationStore.getState().bindWorkflow(request, "tune_gc", {
      workflowId: "wf-secret",
      currentStep: "thread_local_review",
    });
    const router = createMemoryRouter(
      [{ path: "/", element: <HeapSessionBar /> }],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const text = view.container.textContent ?? "";
    expect(text).toMatch(/Tune GC.*current step.*thread_local_review/i);
    expect(text).not.toContain("wf-secret");
    expect(text).not.toContain("/secret/heaps");
  });

  it("renders named determinate progress with percent, unit, operation id, and elapsed time", () => {
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    const context = useInvestigationStore.getState().beginOperation("analyze");
    useInvestigationStore.getState().updateOperationProgress({
      context,
      kind: "analyze",
      phase: "parsing",
      completed: 25,
      total: 100,
      unit: "records",
      indeterminate: false,
      elapsedMs: 1_500,
    });
    const router = createMemoryRouter(
      [{ path: "/", element: <HeapSessionBar /> }],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    const progress = page.getByRole("progressbar", { name: /analyze operation progress/i });

    expect(page.getByText(/Parsing/)).toBeTruthy();
    expect(page.getByText(new RegExp(context.operationId.slice(0, 8)))).toBeTruthy();
    expect(page.getByText(/1\.5s/)).toBeTruthy();
    expect(page.getByText(/25%.*25.*100 records/)).toBeTruthy();
    expect(progress.getAttribute("aria-valuenow")).toBe("25");
    expect(progress.getAttribute("aria-busy")).toBeNull();
  });

  it("renders indeterminate progress without claiming a percentage", () => {
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    const context = useInvestigationStore.getState().beginOperation("gc-path");
    useInvestigationStore.getState().updateOperationProgress({
      context,
      kind: "gc-path",
      phase: "analyzing",
      completed: undefined,
      total: undefined,
      unit: undefined,
      indeterminate: true,
      elapsedMs: 800,
    });
    const router = createMemoryRouter(
      [{ path: "/", element: <HeapSessionBar /> }],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    const progress = page.getByRole("progressbar", { name: /gc-path operation progress/i });

    expect(page.getByText(/Analyzing/)).toBeTruthy();
    expect(page.getByText(/Indeterminate/)).toBeTruthy();
    expect(page.getByText(/800ms/)).toBeTruthy();
    expect(progress.getAttribute("aria-valuenow")).toBeNull();
    expect(progress.getAttribute("aria-busy")).toBe("true");
  });

  it("hides Cancel when the host does not support operation cancellation", () => {
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    useInvestigationStore.getState().beginOperation("analyze");
    const router = createMemoryRouter(
      [{ path: "/", element: <HeapSessionBar operationHost={undefined} /> }],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    expect(within(view.container).queryByRole("button", { name: /^cancel$/i })).toBeNull();
  });

  it("sends the active id and stays cancelling until the host terminal event", async () => {
    const user = userEvent.setup();
    const cancellation = createDeferred<{ operationId: string; accepted: boolean }>();
    const cancelledIds: string[] = [];
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    const context = useInvestigationStore.getState().beginOperation("analyze");
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: (
            <HeapSessionBar
              operationHost={{
                cancelOperation: async (operationId) => {
                  cancelledIds.push(operationId);
                  return cancellation.promise;
                },
              }}
            />
          ),
        },
      ],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    await user.click(page.getByRole("button", { name: /^cancel$/i }));

    expect(cancelledIds).toEqual([context.operationId]);
    expect(page.getByText(/Cancelling/)).toBeTruthy();
    expect(page.getByRole("button", { name: /^cancel$/i }).hasAttribute("disabled")).toBe(true);

    cancellation.resolve({ operationId: context.operationId, accepted: true });
    await waitFor(() => expect(page.getByText(/Cancelling/)).toBeTruthy());

    act(() => {
      useInvestigationStore.getState().updateOperationProgress({
        context,
        kind: "analyze",
        phase: "cancelled",
        indeterminate: true,
        elapsedMs: 1_200,
      });
    });
    expect(page.getByText(/Cancelled/)).toBeTruthy();
    expect(page.queryByRole("button", { name: /^cancel$/i })).toBeNull();
  });

  it("restores in-flight status with an explanation when cancellation is rejected", async () => {
    const user = userEvent.setup();
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    const context = useInvestigationStore.getState().beginOperation("query");
    useInvestigationStore.getState().updateOperationProgress({
      context,
      kind: "query",
      phase: "analyzing",
      indeterminate: true,
      elapsedMs: 600,
    });
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: (
            <HeapSessionBar
              operationHost={{
                cancelOperation: async (operationId) => ({
                  operationId,
                  accepted: false,
                }),
              }}
            />
          ),
        },
      ],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    await user.click(page.getByRole("button", { name: /^cancel$/i }));

    await waitFor(() => {
      expect(page.getByText(/Analyzing/)).toBeTruthy();
      expect(page.getByText(/host did not accept cancellation/i)).toBeTruthy();
    });
    expect(page.getByRole("button", { name: /^cancel$/i }).hasAttribute("disabled")).toBe(false);
  });

  it("treats a structured cancelled error as terminal acknowledgement", async () => {
    const user = userEvent.setup();
    useArtifactStore.getState().setArtifact("demo.hprof", minimalArtifact("demo.hprof"));
    useInvestigationStore.getState().beginOperation("gc-path");
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: (
            <HeapSessionBar
              operationHost={{
                cancelOperation: async () => {
                  throw new Error("operation_cancelled: Operation cancelled");
                },
              }}
            />
          ),
        },
      ],
      { initialEntries: ["/"] },
    );

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    await user.click(page.getByRole("button", { name: /^cancel$/i }));

    await waitFor(() => expect(page.getByText(/Cancelled/)).toBeTruthy());
    expect(page.queryByRole("button", { name: /^cancel$/i })).toBeNull();
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
