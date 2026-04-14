import "../../../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { useDashboardStore } from "../dashboard-store";

import { LeakTable } from "./LeakTable";

describe("LeakTable", () => {
  beforeEach(() => {
    useDashboardStore.getState().reset();
  });

  afterEach(() => {
    useDashboardStore.getState().reset();
    cleanup();
  });

  it("renders leak rows from the artifact", () => {
    const view = render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [{ kind: "FALLBACK" }],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    expect(view.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(view.getByText(/fallback/i)).toBeInTheDocument();
  });

  it("renders the explicit empty-state copy when there are no leaks", () => {
    const view = render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    expect(view.getByRole("heading", { name: /top leak suspects/i })).toBeInTheDocument();
    expect(view.getByText(/no leak suspects detected\./i)).toBeInTheDocument();
    expect(view.getByText(/load an artifact with retained-memory findings to continue triage/i)).toBeInTheDocument();
  });

  it("renders a filter-specific empty state when artifact leaks exist but filters match none", () => {
    const view = render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    act(() => {
      useDashboardStore.getState().setSearch("not-present");
    });

    expect(view.getByText(/no leak suspects match the current filters\./i)).toBeInTheDocument();
    expect(view.getByText(/adjust or clear the current filters to restore matching rows/i)).toBeInTheDocument();
    expect(view.queryByText(/no leak suspects detected\./i)).not.toBeInTheDocument();
  });

  it("toggles inline leak details through inspect and hide", async () => {
    const user = userEvent.setup();
    const view = render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    expect(view.queryByText(/inline drilldown is intentionally restrained in this slice/i)).not.toBeInTheDocument();

    await act(async () => {
      await user.click(view.getByRole("button", { name: /inspect/i }));
    });

    expect(view.getByText(/inline drilldown is intentionally restrained in this slice/i)).toBeInTheDocument();
    expect(view.getByRole("button", { name: /hide/i })).toBeInTheDocument();

    await act(async () => {
      await user.click(view.getByRole("button", { name: /hide/i }));
    });

    expect(view.queryByText(/inline drilldown is intentionally restrained in this slice/i)).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: /inspect/i })).toBeInTheDocument();
  });

  it("filters leak rows by search, severity, provenance state, and retained bytes", async () => {
    const user = userEvent.setup();
    const view = render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 3,
            totalSizeBytes: 4096,
            totalRecords: 3,
          },
          leaks: [
            {
              id: "leak-cache",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 4096,
              shallowSizeBytes: 128,
              suspectScore: 0.92,
              instances: 4,
              description: "Cache leak",
              provenance: [{ kind: "FALLBACK" }],
            },
            {
              id: "leak-session",
              className: "com.example.SessionStore",
              leakKind: "SESSION",
              severity: "MEDIUM",
              retainedSizeBytes: 1536,
              shallowSizeBytes: 96,
              suspectScore: 0.66,
              instances: 2,
              description: "Session leak",
              provenance: [],
            },
            {
              id: "leak-renderer",
              className: "com.example.Renderer",
              leakKind: "UI",
              severity: "CRITICAL",
              retainedSizeBytes: 8192,
              shallowSizeBytes: 256,
              suspectScore: 0.99,
              instances: 1,
              description: "Renderer leak",
              provenance: [{ kind: "HEURISTIC" }],
            },
            {
              id: "leak-finalizer",
              className: "com.example.BufferOwner",
              leakKind: "FINALIZER",
              severity: "LOW",
              retainedSizeBytes: 768,
              shallowSizeBytes: 48,
              suspectScore: 0.35,
              instances: 1,
              description: "Delayed buffer cleanup",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    const table = () => view.getByRole("table");
    const searchInput = view.getByRole("textbox", { name: /search leaks/i });

    expect(searchInput.getAttribute("placeholder")).toBe("class, id, or description");

    expect(within(table()).getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(within(table()).getByText(/com\.example\.SessionStore/i)).toBeInTheDocument();
    expect(within(table()).getByText(/com\.example\.Renderer/i)).toBeInTheDocument();
    expect(within(table()).getByText(/com\.example\.BufferOwner/i)).toBeInTheDocument();

    act(() => {
      useDashboardStore.getState().setSearch("renderer");
    });

    await waitFor(() => {
      expect(view.getByRole("textbox", { name: /search leaks/i })).toHaveValue("renderer");
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 1 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.Cache/i)).not.toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.SessionStore/i)).not.toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.Renderer/i)).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.BufferOwner/i)).not.toBeInTheDocument();
    });

    act(() => {
      useDashboardStore.getState().setSearch("finalizer");
    });

    await waitFor(() => {
      expect(view.getByRole("textbox", { name: /search leaks/i })).toHaveValue("finalizer");
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 1 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.BufferOwner/i)).toBeInTheDocument();
    });

    act(() => {
      useDashboardStore.getState().setSearch("");
    });

    await act(async () => {
      await user.selectOptions(view.getByLabelText(/severity filter/i), "HIGH");
    });

    await waitFor(() => {
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 1 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.Cache/i)).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.SessionStore/i)).not.toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.Renderer/i)).not.toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.BufferOwner/i)).not.toBeInTheDocument();
    });

    await act(async () => {
      await user.selectOptions(view.getByLabelText(/severity filter/i), "all");
      await user.selectOptions(view.getByLabelText(/provenance filter/i), "present");
    });

    await waitFor(() => {
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 2 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.Cache/i)).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.SessionStore/i)).not.toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.Renderer/i)).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.BufferOwner/i)).not.toBeInTheDocument();
    });

    await act(async () => {
      await user.selectOptions(view.getByLabelText(/provenance filter/i), "none");
    });

    await waitFor(() => {
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 2 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.Cache/i)).not.toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.SessionStore/i)).toBeInTheDocument();
      expect(within(table()).queryByText(/com\.example\.Renderer/i)).not.toBeInTheDocument();
      expect(within(table()).getByText(/com\.example\.BufferOwner/i)).toBeInTheDocument();
    });

    act(() => {
      useDashboardStore.getState().setMinimumRetainedBytes(5000);
    });

    await waitFor(() => {
      expect(view.getByRole("spinbutton", { name: /minimum retained bytes/i })).toHaveValue(5000);
      expect(
        view.getByText((content) => content.replace(/\s+/g, " ").includes("Displaying 0 of 4 potential leaks")),
      ).toBeInTheDocument();
      expect(view.getByText(/no leak suspects match the current filters\./i)).toBeInTheDocument();
      expect(view.queryByText(/no leak suspects detected\./i)).not.toBeInTheDocument();
    });
  });
});
