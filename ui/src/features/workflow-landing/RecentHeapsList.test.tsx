import "../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import { RecentHeapsList } from "./RecentHeapsList";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
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
