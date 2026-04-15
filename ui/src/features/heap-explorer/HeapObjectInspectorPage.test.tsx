import "../../test/setup";

import { act, cleanup, render } from "@testing-library/react";
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
      dominatorCount: 2,
      dominators: [
        {
          name: "LruCache#root",
          className: "com.example.cache.LruCache",
          objectId: "0xdeadbeef",
          dominates: 12,
          immediateDominator: "GC Root <system class>",
          retainedSize: 1024,
          shallowSize: 64,
        },
        {
          name: "WorkerQueue#17",
          className: "com.example.jobs.WorkerQueue",
          objectId: "0xcafebabe",
          dominates: 5,
          immediateDominator: "com.example.cache.LruCache@0xdeadbeef",
          retainedSize: 768,
          shallowSize: 48,
        },
      ],
    },
    histogram: {
      groupBy: "class",
      totalInstances: 42,
      totalShallowSize: 2048,
      entries: [],
    },
    provenance: [],
  };
}

describe("HeapObjectInspectorPage", () => {
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

  it("renders the full-page inspector route using the shared selected dominator row", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/object-inspector"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /object inspector/i })).toBeInTheDocument();
    expect(view.getAllByText(/com\.example\.cache\.lrucache/i).length).toBeGreaterThan(0);
    expect(view.getAllByText(/0xdeadbeef/i).length).toBeGreaterThan(0);
    expect(view.queryByText(/inspector placeholder seeded from the current heap explorer target/i)).toBeNull();
  });

  it("honors the objectId search param on direct object inspector entry", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, {
      initialEntries: ["/heap-explorer/object-inspector?objectId=0xcafebabe"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getAllByText(/com\.example\.jobs\.WorkerQueue/i).length).toBeGreaterThan(0);
    expect(view.getAllByText(/0xcafebabe/i).length).toBeGreaterThan(0);
    expect(view.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=0xcafebabe",
    );
  });

  it("renders cross-navigation links for the selected object", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          ...createArtifactFixture(),
          graph: {
            ...createArtifactFixture().graph,
            dominators: [
              {
                ...createArtifactFixture().graph.dominators[0],
                objectId: "worker queue/0xdead beef",
              },
              ...createArtifactFixture().graph.dominators.slice(1),
            ],
          },
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/object-inspector"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=worker%20queue%2F0xdead%20beef",
    );
    expect(view.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=worker%20queue%2F0xdead%20beef",
    );
  });
});
