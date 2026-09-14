import "../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, mock } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { clearRememberedDesktopHeapSource, rememberDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { SnapshotManagerPage } from "./SnapshotManagerPage";

const FULL_KEY = "abcdef0123456789deadbeefabcdef0123456789deadbeefabcdef0123456789";

describe("SnapshotManagerPage", () => {
  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    clearRememberedDesktopHeapSource();
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
