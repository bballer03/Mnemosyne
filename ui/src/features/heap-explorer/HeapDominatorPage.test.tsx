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

describe("HeapDominatorPage", () => {
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

  it("renders the real dominator explorer route content", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("heading", { name: /dominator explorer/i })).toBeInTheDocument();
    expect(page.getByRole("button", { name: /select com\.example\.cache\.lrucache 0xdeadbeef/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(page.queryByText(/dominator table placeholder/i)).toBeNull();
  });

  it("updates the shared heap explorer selection when a dominator row is clicked", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    await user.click(page.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i }));

    expect(page.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(page.getByText(/selected target/i)).toBeInTheDocument();
    expect(page.getAllByText(/0xcafebabe/i).length).toBeGreaterThan(0);
  });

  it("renders cross-navigation links for the selected object with encoded query handoff", () => {
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
                objectId: "cache root/0x2a?",
              },
              ...createArtifactFixture().graph.dominators.slice(1),
            ],
          },
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=cache%20root%2F0x2a%3F",
    );
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=cache%20root%2F0x2a%3F",
    );
  });

  it("selects exactly the clicked artifact-only row when multiple rows have empty object ids", async () => {
    const user = userEvent.setup();

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
                name: "FirstArtifactOnly",
                className: "com.example.FirstArtifactOnly",
                objectId: "",
                dominates: 3,
                retainedSize: 256,
                shallowSize: 16,
              },
              {
                name: "SecondArtifactOnly",
                className: "com.example.SecondArtifactOnly",
                objectId: "",
                dominates: 7,
                retainedSize: 512,
                shallowSize: 32,
              },
            ],
          },
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);
    const firstRow = page.getByRole("button", { name: /select com\.example\.firstartifactonly/i });
    const secondRow = page.getByRole("button", { name: /select com\.example\.secondartifactonly/i });

    expect(firstRow.getAttribute("aria-pressed")).toBe("true");
    expect(secondRow.getAttribute("aria-pressed")).toBe("false");

    await user.click(secondRow);

    expect(page.getByRole("button", { name: /select com\.example\.firstartifactonly/i }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(page.getByRole("button", { name: /select com\.example\.secondartifactonly/i }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(page.getAllByText(/com\.example\.secondartifactonly/i)).toHaveLength(3);
    expect(page.getByRole("button", { name: /select com\.example\.secondartifactonly/i })).toBeInTheDocument();
  });

  it("selects duplicate artifact rows independently even when their labels collide", async () => {
    const user = userEvent.setup();

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
                name: "DuplicateArtifactRow",
                className: "com.example.DuplicateArtifactRow",
                objectId: "",
                dominates: 3,
                retainedSize: 256,
                shallowSize: 16,
              },
              {
                name: "DuplicateArtifactRow",
                className: "com.example.DuplicateArtifactRow",
                objectId: "",
                dominates: 7,
                retainedSize: 512,
                shallowSize: 32,
              },
            ],
          },
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);
    const duplicateButtons = page.getAllByRole("button", { name: /select com\.example\.duplicateartifactrow/i });

    expect(duplicateButtons).toHaveLength(2);
    expect(duplicateButtons[0]?.getAttribute("aria-pressed")).toBe("true");
    expect(duplicateButtons[1]?.getAttribute("aria-pressed")).toBe("false");

    await user.click(duplicateButtons[1]!);

    const updatedButtons = page.getAllByRole("button", { name: /select com\.example\.duplicateartifactrow/i });
    expect(updatedButtons[0]?.getAttribute("aria-pressed")).toBe("false");
    expect(updatedButtons[1]?.getAttribute("aria-pressed")).toBe("true");
  });
});
