import "../../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { ComparisonPicker } from "./ComparisonPicker";
import { useComparisonStore } from "./comparison-store";

function createDiffReportJson(overrides?: Partial<Record<string, unknown>>) {
  return JSON.stringify({
    before: "before.hprof",
    after: "after.hprof",
    delta_bytes: 371,
    delta_objects: 7,
    changed_classes: [],
    class_diff: [],
    object_diff: {
      strategy: "ClassDominator",
      retained_bucket_bits: 10,
      retained_change_threshold: 1048576,
      match_quality: {
        strategy: "ClassDominator",
        collision_rate: 0,
        estimated_false_match_risk: "Low",
        estimated_false_split_risk: "Low",
        notes: [],
      },
      added: [],
      removed: [],
      retained_changed: [],
      totals: {
        before_object_count: 0,
        after_object_count: 0,
        fingerprint_collisions_before: 0,
        fingerprint_collisions_after: 0,
        matched_pairs: 0,
      },
    },
    ...overrides,
  });
}

describe("ComparisonPicker", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_COMPARISON_BRIDGE__?: unknown;
    };
  };

  beforeEach(() => {
    act(() => {
      useComparisonStore.getState().reset();
    });
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__;
    }
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useComparisonStore.getState().reset();
    });
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__;
    }
  });

  it("loads a valid diff report JSON file and updates the comparison store", async () => {
    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    const file = new File([createDiffReportJson()], "diff-report.json", { type: "application/json" });
    const input = page.getByLabelText(/diff report json/i);
    await user.upload(input, file);

    await waitFor(() => {
      expect(useComparisonStore.getState().loadStatus).toBe("ready");
    });

    expect(useComparisonStore.getState().diffReport?.strategy).toBe("ClassDominator");
    expect(page.getByText(/loaded: diff-report\.json/i)).toBeInTheDocument();
  });

  it("shows an explicit, actionable error for a diff report JSON with no object_diff section", async () => {
    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    const file = new File(
      [createDiffReportJson({ object_diff: undefined })],
      "class-mode-diff.json",
      { type: "application/json" },
    );
    const input = page.getByLabelText(/diff report json/i);
    await user.upload(input, file);

    await waitFor(() => {
      expect(page.getByRole("alert")).toHaveTextContent(/--mode object/i);
    });

    expect(useComparisonStore.getState().diffReport).toBeUndefined();
  });

  it("shows an explicit error for malformed JSON", async () => {
    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    const file = new File(["{ not valid json"], "broken.json", { type: "application/json" });
    const input = page.getByLabelText(/diff report json/i);
    await user.upload(input, file);

    await waitFor(() => {
      expect(page.getByRole("alert")).toBeInTheDocument();
    });
  });

  it("renders an explicit bridge-unavailable state for live diff when no comparison bridge is connected", () => {
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    expect(page.getByText(/live diff unavailable: no comparison bridge is connected\./i)).toBeInTheDocument();
    expect(page.queryByText(/before snapshot key/i)).not.toBeInTheDocument();
    expect(page.queryByRole("button", { name: /run live diff/i })).not.toBeInTheDocument();
  });

  it("runs a live diff through the bridge when available and updates the store", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
      diffObjects: async () => ({
        strategy: "ClassDominator",
        retained_bucket_bits: 10,
        retained_change_threshold: 1048576,
        match_quality: {
          strategy: "ClassDominator",
          collision_rate: 0,
          estimated_false_match_risk: "Low",
          estimated_false_split_risk: "Low",
          notes: [],
        },
        added: [],
        removed: [],
        retained_changed: [],
        totals: {
          before_object_count: 0,
          after_object_count: 0,
          fingerprint_collisions_before: 0,
          fingerprint_collisions_after: 0,
          matched_pairs: 0,
        },
      }),
    };

    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    await user.type(page.getByLabelText(/before snapshot key/i), "snap-a");
    await user.type(page.getByLabelText(/after snapshot key/i), "snap-b");
    await user.click(page.getByRole("button", { name: /run live diff/i }));

    await waitFor(() => {
      expect(useComparisonStore.getState().loadStatus).toBe("ready");
    });

    expect(useComparisonStore.getState().sourceKind).toBe("live");
    expect(useComparisonStore.getState().sourceLabel).toBe("live: snap-a -> snap-b");
  });
});
