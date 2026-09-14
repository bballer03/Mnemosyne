import "../../test/setup";

import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { SnapshotManagerPage } from "./SnapshotManagerPage";

describe("SnapshotManagerPage", () => {
  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
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
          heap_sha256: "abcdef0123456789deadbeef",
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
});
