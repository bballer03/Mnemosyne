import "../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, mock } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import {
  getRememberedDesktopHeapSource,
  clearRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { SnapshotManagerPage } from "./SnapshotManagerPage";

const FULL_KEY = "abcdef0123456789deadbeefabcdef0123456789deadbeefabcdef0123456789";

describe("SnapshotManagerPage", () => {
  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    clearRememberedDesktopHeapSource();
    useArtifactStore.getState().reset();
  });

  it("shows unavailable state when the desktop bridge is missing", async () => {
    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText(/snapshot listing is unavailable/i)).toBeInTheDocument();
    });
  });

  it("renders listed snapshot manifests from the workflow bridge", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: FULL_KEY,
          heap_path: "fixture.hprof",
          created_at: "2026-09-14T00:00:00Z",
          mnemosyne_version: "0.3.0",
          object_count: 42,
          has_field_data: false,
        },
      ],
    };

    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText("fixture.hprof")).toBeInTheDocument();
    });
    expect(view.getByText("42")).toBeInTheDocument();
    expect(view.getByText(/abcdef012345/i)).toBeInTheDocument();
  });

  it("renders an empty state when the store has no snapshots", async () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [],
    };

    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText(/no snapshots are cached/i)).toBeInTheDocument();
    });
  });

  it("saves from the remembered heap and refreshes the list", async () => {
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    let listed: unknown[] = [];
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => listed,
      saveSnapshot: async () => {
        listed = [
          {
            schema_version: 1,
            heap_sha256: FULL_KEY,
            heap_path: "fixture.hprof",
            created_at: "2026-09-14T00:00:00Z",
            mnemosyne_version: "0.3.0",
            object_count: 42,
            has_field_data: false,
          },
        ];
        return listed[0];
      },
    };

    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText(/no snapshots are cached/i)).toBeInTheDocument();
    });

    fireEvent.click(view.getByRole("button", { name: /save from remembered heap/i }));

    await waitFor(() => {
      expect(view.getByText("fixture.hprof")).toBeInTheDocument();
    });
    expect(view.getByRole("status")).toHaveTextContent(/saved abcdef012345/i);
  });

  it("opens a snapshot into the desktop session and remembers the source", async () => {
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
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: FULL_KEY,
          heap_path: "fixture.hprof",
          created_at: "2026-09-14T00:00:00Z",
          mnemosyne_version: "0.3.0",
          object_count: 42,
          has_field_data: false,
        },
      ],
      openSnapshot: async (key: string) => {
        expect(key).toBe(FULL_KEY);
        return {
          snapshot: {
            key: FULL_KEY,
            displayName: "fixture.hprof",
            sourceId: "src-opened",
            schemaVersion: 1,
            createdAt: "2026-09-14T00:00:00Z",
          },
          mode: "deep",
          capabilities: {
            graph: true,
            dominators: true,
            fieldData: false,
            snapshotBacked: true,
          },
          analysis: {
            summary: {
              heap_path: "fixture.hprof",
              total_objects: 42,
              total_size_bytes: 8,
              classes: [],
              generated_at: "2026-09-15T00:00:00Z",
              header: null,
              total_records: 0,
              record_stats: [],
            },
            leaks: [],
            recommendations: [],
            elapsed: { secs: 0, nanos: 0 },
            graph: { node_count: 42, edge_count: 0, dominators: [] },
            provenance: [],
          },
        };
      },
    };

    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText("fixture.hprof")).toBeInTheDocument();
    });

    fireEvent.click(view.getByRole("button", { name: /^open$/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(
        /opened fixture\.hprof \(42 objects\).*cached snapshot/i,
      );
    });
    expect(useArtifactStore.getState().artifact?.summary.totalObjects).toBe(42);
    expect(getRememberedDesktopHeapSource()).toEqual({
      sourceId: "src-opened",
      displayName: "fixture.hprof",
    });
  });

  it("removes a snapshot after confirmation", async () => {
    let listed: unknown[] = [
      {
        schema_version: 1,
        heap_sha256: FULL_KEY,
        heap_path: "fixture.hprof",
        created_at: "2026-09-14T00:00:00Z",
        mnemosyne_version: "0.3.0",
        object_count: 42,
        has_field_data: false,
      },
    ];
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => listed,
      removeSnapshot: async (key: string) => {
        listed = [];
        return { removed: true, key };
      },
    };
    const confirm = mock(() => true);
    window.confirm = confirm;

    const view = render(
      <MemoryRouter>
        <SnapshotManagerPage />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(view.getByText("fixture.hprof")).toBeInTheDocument();
    });

    fireEvent.click(view.getByRole("button", { name: /^remove$/i }));

    await waitFor(() => {
      expect(view.getByText(/no snapshots are cached/i)).toBeInTheDocument();
    });
    expect(confirm).toHaveBeenCalled();
    expect(view.getByRole("status")).toHaveTextContent(/removed abcdef012345/i);
  });
});
