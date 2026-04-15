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
          name: "com.example.Cache",
          className: "com.example.Cache",
          objectId: "0xdeadbeef",
          dominates: 12,
          retainedSize: 1024,
          shallowSize: 64,
        },
        {
          name: "com.example.WorkQueue",
          className: "com.example.WorkQueue",
          objectId: "0xcafebabe",
          dominates: 5,
          retainedSize: 512,
          shallowSize: 32,
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

describe("HeapExplorerLayout", () => {
  const originalInnerWidth = window.innerWidth;

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

    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: originalInnerWidth,
    });
  });

  it("redirects heap explorer route access back to the loader when no artifact is loaded", () => {
    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/dominators"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(router.state.location.pathname).toBe("/");
    expect(page.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
  });

  it("defaults the first dominator row into the selected object inspector context", () => {
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

    expect(page.getByRole("heading", { name: /heap explorer/i })).toBeInTheDocument();
    expect(page.getByText(/selected target/i)).toBeInTheDocument();
    expect(page.getAllByText(/0xdeadbeef/i).length).toBeGreaterThan(0);
    expect(page.getAllByText(/com\.example\.Cache/i).length).toBeGreaterThan(0);
  });

  it("uses the objectId search param to seed the shared selection on route entry", () => {
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
    const page = within(view.container);

    expect(page.getAllByText(/0xcafebabe/i).length).toBeGreaterThan(0);
    expect(page.getAllByText(/com\.example\.WorkQueue/i).length).toBeGreaterThan(0);
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=0xcafebabe",
    );
  });

  it("shows no selected object when the objectId search param does not match any dominator row", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, {
      initialEntries: ["/heap-explorer/object-inspector?objectId=0xdoesnotexist"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByText(/no object selected/i)).toBeInTheDocument();
    expect(page.getByText(/awaiting selection/i)).toBeInTheDocument();
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console",
    );
  });

  it("recovers normal shared selection after a manual row click from an unmatched objectId entry", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, {
      initialEntries: ["/heap-explorer/dominators?objectId=0xdoesnotexist"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByText(/no object selected/i)).toBeInTheDocument();
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console",
    );

    await user.click(page.getByRole("button", { name: /select com\.example\.cache 0xdeadbeef/i }));

    expect(page.queryByText(/no object selected/i)).toBeNull();
    expect(page.queryByText(/awaiting selection/i)).toBeNull();
    expect(page.getAllByText(/com\.example\.Cache/i).length).toBeGreaterThan(0);
    expect(page.getAllByText(/0xdeadbeef/i).length).toBeGreaterThan(0);
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=0xdeadbeef",
    );
  });

  it("lets user row selection override the seeded objectId after route entry", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, {
      initialEntries: ["/heap-explorer/dominators?objectId=0xcafebabe"],
    });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getByRole("button", { name: /select com\.example\.workqueue 0xcafebabe/i }).getAttribute("aria-pressed")).toBe(
      "true",
    );

    await user.click(page.getByRole("button", { name: /select com\.example\.cache 0xdeadbeef/i }));

    expect(page.getByRole("button", { name: /select com\.example\.cache 0xdeadbeef/i }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(page.getByRole("button", { name: /select com\.example\.workqueue 0xcafebabe/i }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(page.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=0xdeadbeef",
    );
  });

  it("stacks the heap explorer shell into one column on narrow screens", () => {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 720,
    });

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
    const layoutSection = page.getByLabelText(/mode rail/i).parentElement;

    expect(layoutSection).toBeTruthy();
    expect(layoutSection?.getAttribute("style")).toContain("grid-template-columns: minmax(0, 1fr)");
  });

  it("marks only the active header navigation target as current", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: createArtifactFixture(),
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/object-inspector"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);
    const heapExplorerLink = page.getByRole("link", { name: /^heap explorer$/i });
    const dashboardLink = page.getByRole("link", { name: /^dashboard$/i });

    expect(heapExplorerLink.getAttribute("aria-current")).toBe("page");
    expect(dashboardLink.getAttribute("aria-current")).toBeNull();
  });

  it("renders an explicit fallback label when the selected object has an empty string object id", () => {
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
                name: "com.example.EmptyIdNode",
                className: "com.example.EmptyIdNode",
                objectId: "",
                dominates: 7,
                retainedSize: 256,
                shallowSize: 16,
              },
            ],
          },
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/heap-explorer/object-inspector"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
    const page = within(view.container);

    expect(page.getAllByText(/artifact-only row/i)).toHaveLength(2);
    expect(page.queryByText(/no object selected/i)).not.toBeInTheDocument();
  });
});
