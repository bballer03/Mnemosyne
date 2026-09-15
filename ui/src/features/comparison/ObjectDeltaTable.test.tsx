import "../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, MemoryRouter, RouterProvider } from "react-router-dom";

import type { ObjectDelta } from "../../lib/diff-types";
import { useInvestigationStore } from "../investigation/investigation-store";

import { ObjectDeltaTable } from "./ObjectDeltaTable";

const addedDelta: ObjectDelta = {
  className: "com/example/CacheHolder",
  fingerprint: { classId: 7, retainedBucket: 3, dominatorSignature: 1, fieldSignature: 2 },
  exampleObjectId: 4096,
  beforeCount: 0,
  afterCount: 12,
  beforeRetainedBytes: 0,
  afterRetainedBytes: 49152,
  dominatorChain: ["java/lang/Thread", "com/example/CacheHolder"],
  referenceChain: [],
  kind: "Added",
};

const retainedChangedWithLeak: ObjectDelta = {
  className: "com/example/BigCache",
  fingerprint: { classId: 11, retainedBucket: 5, dominatorSignature: 3, fieldSignature: 4 },
  exampleObjectId: 2048,
  beforeCount: 2,
  afterCount: 2,
  beforeRetainedBytes: 1024,
  afterRetainedBytes: 2097152,
  dominatorChain: ["com/example/BigCache"],
  referenceChain: ["com/example/BigCache.entries"],
  kind: "RetainedChanged",
  leakSeverity: "HIGH",
};

const removedDelta: ObjectDelta = {
  ...addedDelta,
  kind: "Removed",
  beforeCount: 12,
  afterCount: 0,
  beforeRetainedBytes: 49152,
  afterRetainedBytes: 0,
};

function renderTable(kind: ObjectDelta["kind"], deltas: ObjectDelta[]) {
  return render(
    <MemoryRouter>
      <ObjectDeltaTable kind={kind} deltas={deltas} />
    </MemoryRouter>,
  );
}

describe("ObjectDeltaTable", () => {
  afterEach(() => {
    cleanup();
    useInvestigationStore.getState().clearSelection();
  });

  it("renders added rows with class name and before->after count/retained size", () => {
    const view = renderTable("Added", [addedDelta]);
    const table = view.getByRole("table");

    expect(within(table).getByText("com/example/CacheHolder")).toBeInTheDocument();
    expect(within(table).getByText("0 -> 12")).toBeInTheDocument();
    expect(within(table).getByText(/0 B -> 48\.0 KB/)).toBeInTheDocument();
    expect(within(table).getByText(/java\/lang\/Thread -> com\/example\/CacheHolder/)).toBeInTheDocument();
  });

  it("renders an explicit empty state instead of a blank table when there are no deltas", () => {
    const view = renderTable("Added", []);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/no object classes were added between these two heaps\./i)).toBeInTheDocument();
  });

  it("renders removed-specific empty copy", () => {
    const view = renderTable("Removed", []);

    expect(view.getByText(/no object classes were removed between these two heaps\./i)).toBeInTheDocument();
  });

  it("renders retained-changed-specific empty copy", () => {
    const view = renderTable("RetainedChanged", []);

    expect(
      view.getByText(/no object classes changed retained size beyond the configured threshold\./i),
    ).toBeInTheDocument();
  });

  it("renders a leak-severity badge when a retained-changed delta carries leak_severity", () => {
    const view = renderTable("RetainedChanged", [retainedChangedWithLeak]);
    const table = view.getByRole("table");

    expect(within(table).getByText("HIGH")).toBeInTheDocument();
  });

  it("renders a dash placeholder when leak_severity is absent", () => {
    const view = renderTable("Added", [addedDelta]);
    const table = view.getByRole("table");

    const cells = within(table).getAllByText("-");
    expect(cells.length).toBeGreaterThan(0);
  });

  it.each([
    ["Added", addedDelta],
    ["RetainedChanged", retainedChangedWithLeak],
  ] as const)("sets shared objectId and opens Inspector for %s rows", async (kind, delta) => {
    const router = createMemoryRouter(
      [
        { path: "/compare", element: <ObjectDeltaTable kind={kind} deltas={[delta]} /> },
        { path: "/heap-explorer/object-inspector", element: <div>Inspector</div> },
      ],
      { initialEntries: ["/compare"] },
    );
    const user = userEvent.setup();
    const view = render(<RouterProvider router={router} />);

    await user.click(view.getByRole("link", { name: new RegExp(`inspect after object ${delta.exampleObjectId}`, "i") }));

    expect(useInvestigationStore.getState()).toMatchObject({
      objectId: String(delta.exampleObjectId),
      originPane: "inspector",
    });
    expect(view.getByText("Inspector")).toBeInTheDocument();
  });

  it("does not offer after-side navigation for removed rows", () => {
    const view = renderTable("Removed", [removedDelta]);

    expect(view.queryByRole("link", { name: /inspect after object/i })).toBeNull();
  });
});
