import "../../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { useArtifactStore } from "./use-artifact-store";
import { ArtifactLoaderPage } from "./ArtifactLoaderPage";
import { DashboardPage } from "../dashboard/DashboardPage";
import { useDashboardStore } from "../dashboard/dashboard-store";

function createArtifactJson(overrides?: Partial<Record<string, unknown>>) {
  return JSON.stringify({
    summary: {
      heap_path: "fixture.hprof",
      total_objects: 42,
      total_size_bytes: 2048,
      classes: [],
      generated_at: "2026-04-14T00:00:00Z",
      header: null,
      total_records: 2,
      record_stats: [],
    },
    leaks: [],
    recommendations: [],
    elapsed: { secs: 1, nanos: 0 },
    graph: { node_count: 12, edge_count: 24, dominators: [] },
    histogram: {
      group_by: "class",
      entries: [],
      total_instances: 42,
      total_shallow_size: 2048,
    },
    provenance: [],
    ...overrides,
  });
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });

  return { promise, resolve };
}

describe("ArtifactLoaderPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useDashboardStore.getState().reset();
    });
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 1280,
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useDashboardStore.getState().reset();
    });
  });

  it("shows the selected artifact name after a valid JSON load", async () => {
    const user = userEvent.setup();
    const view = render(<ArtifactLoaderPage />);
    const page = within(view.container);

    expect(
      page.getByRole("heading", { name: /load analysis artifact/i }),
    ).toBeInTheDocument();
    expect(
      page.getByText(/expects mnemosyne analysis json derived from analyzeresponse/i),
    ).toBeInTheDocument();
    expect(page.getByText(/dashboard preview/i)).toBeInTheDocument();
    expect(page.getByText(/validation console/i)).toBeInTheDocument();
    expect(page.getByText(/recent loads/i)).toBeInTheDocument();

    const file = new File([createArtifactJson()], "fixture.json", { type: "application/json" });

    const input = page.getByLabelText(/analysis json artifact/i);
    await user.upload(input, file);

    expect(page.getByText(/artifact loaded:\s*fixture\.json/i)).toBeInTheDocument();
    expect(page.getByText(/heap path:\s*fixture\.hprof/i)).toBeInTheDocument();
  });

  it("stacks the preview panel below the loader content on narrow screens", () => {
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 720,
    });

    const view = render(<ArtifactLoaderPage />);
    const page = within(view.container);
    const layoutSection = page.getByText(/dashboard preview/i).closest("aside")?.parentElement;

    expect(layoutSection).toBeTruthy();
    expect(layoutSection?.getAttribute("style")).toContain("grid-template-columns: minmax(0, 1fr)");
  });

  it("keeps recent loads after the route remounts in the same browser session", async () => {
    const user = userEvent.setup();
    const firstView = render(<ArtifactLoaderPage />);
    const firstPage = within(firstView.container);
    const file = new File([createArtifactJson()], "fixture.json", { type: "application/json" });

    await user.upload(firstPage.getByLabelText(/analysis json artifact/i), file);
    await waitFor(() => {
      expect(firstPage.getByRole("table")).toBeInTheDocument();
    });

    firstView.unmount();

    const secondView = render(<ArtifactLoaderPage />);
    const secondPage = within(secondView.container);
    const recentLoadsTable = secondPage.getByRole("table");

    expect(within(recentLoadsTable).getByText(/^fixture\.json$/i)).toBeInTheDocument();
    expect(within(recentLoadsTable).getByText(/fixture\.hprof/i)).toBeInTheDocument();
  });

  it("keeps the latest file selection when an older load finishes later", async () => {
    const user = userEvent.setup();
    const firstLoad = createDeferred<string>();
    const view = render(<ArtifactLoaderPage />);
    const page = within(view.container);

    const olderFile = new File([createArtifactJson()], "older.json", { type: "application/json" });
    Object.defineProperty(olderFile, "text", {
      configurable: true,
      value: () => firstLoad.promise,
    });

    const newerFile = new File(
      [createArtifactJson({ summary: { heap_path: "newer.hprof", total_objects: 7, total_size_bytes: 512, classes: [], generated_at: "2026-04-14T00:00:00Z", header: null, total_records: 1, record_stats: [] } })],
      "newer.json",
      { type: "application/json" },
    );

    const input = page.getByLabelText(/analysis json artifact/i);
    await user.upload(input, olderFile);
    await user.upload(input, newerFile);

    await act(async () => {
      firstLoad.resolve(createArtifactJson());
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(
        page.getByText((content) => content.replace(/\s+/g, " ").includes("Artifact loaded: newer.json")),
      ).toBeInTheDocument();
      expect(
        page.getByText((content) => content.replace(/\s+/g, " ").includes("Heap path: newer.hprof")),
      ).toBeInTheDocument();
    });
  });

  it("shows readable feedback for malformed JSON uploads", async () => {
    const user = userEvent.setup();
    const view = render(<ArtifactLoaderPage />);
    const page = within(view.container);
    const file = new File(["{ definitely not valid json"], "broken.json", {
      type: "application/json",
    });

    await user.upload(page.getByLabelText(/analysis json artifact/i), file);

    await waitFor(() => {
      expect(page.getByRole("alert")).toHaveTextContent(/invalid json artifact/i);
    });

    expect(page.getByRole("status")).toHaveTextContent(/validation error/i);
    expect(page.queryByText(/artifact loaded:/i)).not.toBeInTheDocument();
  });

  it("resets expanded leak rows when a new artifact load reuses a leak id", async () => {
    const user = userEvent.setup();
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: <ArtifactLoaderPage />,
        },
        {
          path: "/dashboard",
          element: <DashboardPage />,
        },
      ],
      { initialEntries: ["/"] },
    );

    act(() => {
      useDashboardStore.getState().toggleLeakExpanded("shared-leak");
    });

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    const file = new File(
      [
        createArtifactJson({
          leaks: [
            {
              id: "shared-leak",
              class_name: "com.example.Cache",
              leak_kind: "CACHE",
              severity: "HIGH",
              retained_size_bytes: 1024,
              shallow_size_bytes: 64,
              suspect_score: 0.98,
              instances: 4,
              description: "Cache retains request objects",
              provenance: [],
            },
          ],
        }),
      ],
      "fixture.json",
      { type: "application/json" },
    );

    await user.upload(page.getByLabelText(/analysis json artifact/i), file);

    await waitFor(() => {
      expect(page.getByRole("heading", { name: /mnemosyne triage dashboard/i })).toBeInTheDocument();
    });

    expect(page.queryByText(/inline drilldown is intentionally restrained in this slice/i)).not.toBeInTheDocument();
    expect(page.getByRole("button", { name: /inspect/i })).toBeInTheDocument();
  });

  it("clears dashboard filters when a new artifact finishes loading", async () => {
    const user = userEvent.setup();
    const router = createMemoryRouter(
      [
        {
          path: "/",
          element: <ArtifactLoaderPage />,
        },
        {
          path: "/dashboard",
          element: <DashboardPage />,
        },
      ],
      { initialEntries: ["/"] },
    );

    act(() => {
      useDashboardStore.setState({
        search: "renderer",
        severity: "CRITICAL",
        provenanceFilter: "none",
        minimumRetainedBytes: 5000,
      });
    });

    const view = render(<RouterProvider router={router} />);
    const page = within(view.container);
    const file = new File([createArtifactJson()], "fixture.json", { type: "application/json" });

    await user.upload(page.getByLabelText(/analysis json artifact/i), file);

    await waitFor(() => {
      expect(page.getByRole("heading", { name: /mnemosyne triage dashboard/i })).toBeInTheDocument();
    });

    expect(page.getByRole("textbox", { name: /search leaks/i })).toHaveValue("");
    expect(page.getByLabelText(/severity filter/i)).toHaveValue("all");
    expect(page.getByLabelText(/provenance filter/i)).toHaveValue("all");
    expect(page.getByRole("spinbutton", { name: /minimum retained bytes/i })).toHaveValue(null);
  });
});
