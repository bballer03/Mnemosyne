import "../../test/setup";

import { act, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";

import {
  clearRememberedDesktopHeapSource,
  getRememberedDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { RecentHeapsList } from "./RecentHeapsList";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
  clearRememberedDesktopHeapSource();
  act(() => {
    useArtifactStore.getState().reset();
  });
});

describe("RecentHeapsList", () => {
  it("renders nothing at all when the bridge lacks listSnapshots", () => {
    const view = render(<RecentHeapsList />);
    expect(view.container.innerHTML).toBe("");
  });

  it("renders the snapshot manifests when the bridge provides them", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "abc123",
          heap_path: "fixture.hprof",
          created_at: "1700000000",
          mnemosyne_version: "0.4.0",
          object_count: 4200,
          has_field_data: true,
        },
      ],
    };

    const view = render(<RecentHeapsList />);
    const page = within(view.container);

    await waitFor(() => {
      expect(page.getByText(/fixture\.hprof/i)).toBeInTheDocument();
    });
    expect(page.getByText(/4,200/)).toBeInTheDocument();
    expect(page.getByText(/^yes$/i)).toBeInTheDocument();
  });

  it("opens a snapshot, clears the prior artifact, and reports the graph-only state", async () => {
    const user = userEvent.setup();
    act(() => {
      useArtifactStore.getState().setArtifact("older.json", {
        summary: {
          heapPath: "older.hprof",
          totalObjects: 1,
          totalSizeBytes: 8,
          totalRecords: 1,
        },
        leaks: [],
        recommendations: [],
        elapsedSeconds: 0,
        graph: {
          nodeCount: 1,
          edgeCount: 0,
          dominatorCount: 0,
          dominators: [],
        },
        provenance: [],
      });
    });
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "abc123",
          heap_path: "fixture.hprof",
          created_at: "1700000000",
          mnemosyne_version: "0.4.0",
          object_count: 4200,
          has_field_data: true,
        },
      ],
      openSnapshot: async () => ({
        displayName: "fixture.hprof",
        sourceId: "src-snapshot",
        objectCount: 4200,
        classCount: 12,
        gcRootCount: 3,
      }),
    };

    const view = render(<RecentHeapsList />);
    const page = within(view.container);
    await user.click(await page.findByRole("button", { name: /open fixture\.hprof/i }));

    await waitFor(() => {
      expect(page.getByRole("status")).toHaveTextContent(
        /opened fixture\.hprof.*artifact views were cleared/i,
      );
    });
    expect(useArtifactStore.getState().artifact).toBeUndefined();
    expect(getRememberedDesktopHeapSource()).toEqual({
      sourceId: "src-snapshot",
      displayName: "fixture.hprof",
    });
  });

  it("disables snapshot Open when the host only supports listing", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "abc123",
          heap_path: "fixture.hprof",
          created_at: "1700000000",
          mnemosyne_version: "0.4.0",
          object_count: 4200,
          has_field_data: true,
        },
      ],
    };

    const view = render(<RecentHeapsList />);
    const page = within(view.container);

    expect(await page.findByRole("button", { name: /open fixture\.hprof/i })).toBeDisabled();
    expect(page.getByText(/opening snapshots requires the desktop host bridge/i)).toBeInTheDocument();
  });

  it("shows an explicit empty state when the cache has no snapshots", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [],
    };

    const view = render(<RecentHeapsList />);
    const page = within(view.container);

    await waitFor(() => {
      expect(page.getByText(/no cached snapshots yet/i)).toBeInTheDocument();
    });
  });

  it("renders a bridge error gracefully", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => {
        throw new Error("snapshot cache unreachable");
      },
    };

    const view = render(<RecentHeapsList />);
    const page = within(view.container);

    await waitFor(() => {
      expect(page.getByRole("alert")).toHaveTextContent(/snapshot cache unreachable/i);
    });
  });
});
