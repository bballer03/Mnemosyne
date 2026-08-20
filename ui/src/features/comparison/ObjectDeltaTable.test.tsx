import "../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { ObjectDelta } from "../../lib/diff-types";

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

describe("ObjectDeltaTable", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders added rows with class name and before->after count/retained size", () => {
    const view = render(<ObjectDeltaTable kind="Added" deltas={[addedDelta]} />);
    const table = view.getByRole("table");

    expect(within(table).getByText("com/example/CacheHolder")).toBeInTheDocument();
    expect(within(table).getByText("0 -> 12")).toBeInTheDocument();
    expect(within(table).getByText(/0 B -> 48\.0 KB/)).toBeInTheDocument();
    expect(within(table).getByText(/java\/lang\/Thread -> com\/example\/CacheHolder/)).toBeInTheDocument();
  });

  it("renders an explicit empty state instead of a blank table when there are no deltas", () => {
    const view = render(<ObjectDeltaTable kind="Added" deltas={[]} />);

    expect(view.queryByRole("table")).not.toBeInTheDocument();
    expect(view.getByText(/no object classes were added between these two heaps\./i)).toBeInTheDocument();
  });

  it("renders removed-specific empty copy", () => {
    const view = render(<ObjectDeltaTable kind="Removed" deltas={[]} />);

    expect(view.getByText(/no object classes were removed between these two heaps\./i)).toBeInTheDocument();
  });

  it("renders retained-changed-specific empty copy", () => {
    const view = render(<ObjectDeltaTable kind="RetainedChanged" deltas={[]} />);

    expect(
      view.getByText(/no object classes changed retained size beyond the configured threshold\./i),
    ).toBeInTheDocument();
  });

  it("renders a leak-severity badge when a retained-changed delta carries leak_severity", () => {
    const view = render(<ObjectDeltaTable kind="RetainedChanged" deltas={[retainedChangedWithLeak]} />);
    const table = view.getByRole("table");

    expect(within(table).getByText("HIGH")).toBeInTheDocument();
  });

  it("renders a dash placeholder when leak_severity is absent", () => {
    const view = render(<ObjectDeltaTable kind="Added" deltas={[addedDelta]} />);
    const table = view.getByRole("table");

    const cells = within(table).getAllByText("-");
    expect(cells.length).toBeGreaterThan(0);
  });
});
