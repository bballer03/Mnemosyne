import "../../test/setup";

import userEvent from "@testing-library/user-event";
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
      dominatorCount: 2,
      dominators: [
        {
          name: "LruCache#root",
          className: "com.example.cache.LruCache",
          objectId: "0xdeadbeef",
          dominates: 12,
          retainedSize: 1024,
          shallowSize: 64,
        },
        {
          name: "WorkerQueue#17",
          className: "com.example.jobs.WorkerQueue",
          objectId: "0xcafebabe",
          dominates: 5,
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

function createMatchingLeak() {
  return {
    id: "leak-cache-1",
    className: "com.example.cache.LruCache",
    leakKind: "dominator",
    severity: "high",
    retainedSizeBytes: 1024,
    instances: 1,
    description: "Large cache",
    provenance: [],
  };
}

function seedArtifactWithDominators() {
  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: createArtifactFixture(),
    });
  });
}

describe("HeapQueryConsolePage", () => {
  beforeEach(() => {
    delete globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;

    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();
    delete globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;

    act(() => {
      useArtifactStore.getState().reset();
    });
  });

  it("shows an unavailable state when no query bridge exists", () => {
    seedArtifactWithDominators();

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByText(/query execution is unavailable in this browser session/i)).toBeInTheDocument();
  });

  it("renders cross-navigation actions alongside the unavailable state", () => {
    seedArtifactWithDominators();

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=0xdeadbeef",
    );
    expect(page.getByText(/query execution is unavailable in this browser session/i)).toBeInTheDocument();
  });

  it("renders the leak workspace link when the selected object resolves to a leak", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          ...createArtifactFixture(),
          leaks: [createMatchingLeak()],
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("link", { name: /open leak workspace/i })).toHaveAttribute(
      "href",
      "/leaks/leak-cache-1/overview",
    );
  });

  it("runs a query and renders result rows", async () => {
    const user = userEvent.setup();
    seedArtifactWithDominators();
    globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => ({
        columns: ["object_id", "class_name"],
        rows: [["0x2a", "com.example.Cache"]],
      }),
    };

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/query-console"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    await user.click(page.getByRole("button", { name: /run query/i }));

    expect(await page.findByRole("columnheader", { name: /object_id/i })).toBeInTheDocument();
    expect(page.getByRole("cell", { name: /0x2a/i })).toBeInTheDocument();
    expect(page.getByRole("cell", { name: /^com\.example\.Cache$/i })).toBeInTheDocument();
  });
});
