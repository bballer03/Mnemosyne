import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import type { ObjectDiffReport } from "../../lib/diff-types";

import { ComparisonPage } from "./ComparisonPage";
import { useComparisonStore } from "./comparison-store";

function renderComparisonPage() {
  const router = createMemoryRouter(
    [{ path: "/compare", element: <ComparisonPage /> }],
    { initialEntries: ["/compare"] },
  );

  return render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);
}

const knownFixturePair: ObjectDiffReport = {
  strategy: "ClassDominator",
  retainedBucketBits: 10,
  retainedChangeThreshold: 1048576,
  matchQuality: {
    strategy: "ClassDominator",
    collisionRate: 0.05,
    estimatedFalseMatchRisk: "Low",
    estimatedFalseSplitRisk: "Medium",
    notes: ["class+dominator adds four hops of dominator context to reduce sibling collisions"],
  },
  added: [
    {
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
    },
  ],
  removed: [
    {
      className: "com/example/StaleEntry",
      fingerprint: { classId: 9, retainedBucket: 1, dominatorSignature: 3, fieldSignature: 4 },
      exampleObjectId: 8192,
      beforeCount: 4,
      afterCount: 0,
      beforeRetainedBytes: 2048,
      afterRetainedBytes: 0,
      dominatorChain: [],
      referenceChain: [],
      kind: "Removed",
    },
  ],
  retainedChanged: [
    {
      className: "com/example/BigCache",
      fingerprint: { classId: 11, retainedBucket: 5, dominatorSignature: 5, fieldSignature: 6 },
      exampleObjectId: 2048,
      beforeCount: 2,
      afterCount: 2,
      beforeRetainedBytes: 1024,
      afterRetainedBytes: 2097152,
      dominatorChain: ["com/example/Root", "com/example/BigCache"],
      referenceChain: ["com/example/BigCache.entries"],
      kind: "RetainedChanged",
      leakSeverity: "HIGH",
    },
  ],
  totals: {
    beforeObjectCount: 6,
    afterObjectCount: 14,
    fingerprintCollisionsBefore: 0,
    fingerprintCollisionsAfter: 1,
    matchedPairs: 2,
  },
};

const emptyDiff: ObjectDiffReport = {
  ...knownFixturePair,
  added: [],
  removed: [],
  retainedChanged: [],
  matchQuality: { ...knownFixturePair.matchQuality, collisionRate: 0, notes: [] },
};

describe("ComparisonPage", () => {
  beforeEach(() => {
    act(() => {
      useComparisonStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useComparisonStore.getState().reset();
    });
  });

  it("renders the picker and heading before any diff report is loaded", () => {
    const view = renderComparisonPage();

    expect(view.getByRole("heading", { name: /compare two heaps/i })).toBeInTheDocument();
    expect(view.getByLabelText(/comparison picker/i)).toBeInTheDocument();
    expect(view.queryByLabelText(/match quality/i)).not.toBeInTheDocument();
  });

  it("renders correct added/removed/retained_changed tables against a known fixture pair", () => {
    act(() => {
      useComparisonStore.getState().setDiffReport(knownFixturePair, "fixture-diff.json", "file");
    });

    const view = renderComparisonPage();

    const addedTable = within(view.getByLabelText(/added objects/i));
    expect(addedTable.getByText("com/example/CacheHolder")).toBeInTheDocument();
    expect(addedTable.getByText("0 -> 12")).toBeInTheDocument();

    const removedTable = within(view.getByLabelText(/removed objects/i));
    expect(removedTable.getByText("com/example/StaleEntry")).toBeInTheDocument();
    expect(removedTable.getByText("4 -> 0")).toBeInTheDocument();

    const retainedChangedTable = within(view.getByLabelText(/retained changed objects/i));
    expect(retainedChangedTable.getByText("com/example/BigCache")).toBeInTheDocument();
    expect(retainedChangedTable.getByText("HIGH")).toBeInTheDocument();
  });

  it("reflects the fixture's actual collision rate in the match-quality badge", () => {
    act(() => {
      useComparisonStore.getState().setDiffReport(knownFixturePair, "fixture-diff.json", "file");
    });

    const view = renderComparisonPage();

    expect(view.getByText(/5\.00%/)).toBeInTheDocument();
  });

  it("renders an explicit no-differences state, not blank tables, when added/removed/retained_changed are all empty", () => {
    act(() => {
      useComparisonStore.getState().setDiffReport(emptyDiff, "empty-diff.json", "file");
    });

    const view = renderComparisonPage();

    expect(view.getByText(/no differences found\./i)).toBeInTheDocument();
    expect(view.queryByLabelText(/added objects/i)).not.toBeInTheDocument();
    expect(view.queryByLabelText(/removed objects/i)).not.toBeInTheDocument();
    expect(view.queryByLabelText(/retained changed objects/i)).not.toBeInTheDocument();
    expect(view.queryByRole("table")).not.toBeInTheDocument();
  });

  it("still renders the match-quality badge in the empty-diff case", () => {
    act(() => {
      useComparisonStore.getState().setDiffReport(emptyDiff, "empty-diff.json", "file");
    });

    const view = renderComparisonPage();

    expect(view.getByLabelText(/match quality/i)).toBeInTheDocument();
  });
});
