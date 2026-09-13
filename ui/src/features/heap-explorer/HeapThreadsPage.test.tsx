import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";

function createArtifactFixture() {
  return {
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
      dominatorCount: 1,
      dominators: [
        {
          name: "com.example.WorkerThread",
          className: "com.example.WorkerThread",
          objectId: "0x5000",
          dominates: 0,
          retainedSize: 2048,
          shallowSize: 64,
        },
      ],
    },
    threadReport: {
      threads: [
        {
          objectId: 20480,
          name: "Thread-7",
          daemon: false,
          stackTrace: [
            {
              methodName: "run",
              className: "com/example/WorkerThread",
              sourceFile: "WorkerThread.java",
              lineNumber: 42,
              locals: [
                {
                  variableSlot: 0,
                  objectId: "0x00003000",
                  className: "com.example.webapp.RequestHandler",
                  rootKind: "JavaFrame" as const,
                },
              ],
            },
          ],
          retainedBytes: 2048,
          threadLocalCount: 1,
          threadLocalBytes: 128,
        },
      ],
      totalThreadCount: 1,
      totalThreadRetained: 2048,
      topRetainers: [],
    },
    provenance: [],
  };
}

function seedArtifact() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: createArtifactFixture(),
    });
  });
}

describe("HeapThreadsPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();

    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  it("renders the thread explorer panel with cross-navigation actions at /heap-explorer/threads", () => {
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/threads"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=0x5000",
    );

    const detail = within(page.getByRole("region", { name: /selected thread detail/i }));
    expect(detail.getByText("Thread-7")).toBeInTheDocument();
    expect(detail.getByText(/com\/example\/WorkerThread\.run/)).toBeInTheDocument();
  });

  it("is reachable via the Threads nav link in the mode rail", () => {
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("link", { name: /^threads$/i })).toHaveAttribute("href", "/heap-explorer/threads");
  });

  it("redirects to the artifact loader when no artifact is loaded", () => {
    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/threads"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
  });
});
