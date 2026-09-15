import "../../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { useInvestigationStore } from "../investigation/investigation-store";
import type { DiffObjectsInput } from "./comparison-bridge-client";
import { ComparisonPicker } from "./ComparisonPicker";
import { useComparisonStore } from "./comparison-store";

function createRawObjectDiff() {
  return {
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
  };
}

function createDiffReportJson(overrides?: Partial<Record<string, unknown>>) {
  return JSON.stringify({
    before: "before.hprof",
    after: "after.hprof",
    delta_bytes: 371,
    delta_objects: 7,
    changed_classes: [],
    class_diff: [],
    object_diff: createRawObjectDiff(),
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
      delete globalWindow.window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
    }
    useInvestigationStore.setState({ persistenceIdentity: undefined });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useComparisonStore.getState().reset();
    });
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__;
      delete globalWindow.window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
    }
    useInvestigationStore.setState({ persistenceIdentity: undefined });
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
      diffObjects: async () => createRawObjectDiff(),
    };
    globalWindow.window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "snap-a",
          heap_path: "snap-a.hprof",
          created_at: "2026-09-15T00:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 10,
          has_field_data: false,
        },
        {
          schema_version: 1,
          heap_sha256: "snap-b",
          heap_path: "snap-b.hprof",
          created_at: "2026-09-15T01:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 12,
          has_field_data: false,
        },
      ],
    };

    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    await page.findAllByRole("option", { name: /snap-a\.hprof/i });
    await user.selectOptions(page.getByRole("combobox", { name: /^baseline snapshot$/i }), "snap-a");
    await user.selectOptions(page.getByRole("combobox", { name: /^current snapshot$/i }), "snap-b");
    await user.click(page.getByRole("button", { name: /run live diff/i }));

    await waitFor(() => {
      expect(useComparisonStore.getState().loadStatus).toBe("ready");
    });

    expect(useComparisonStore.getState().sourceKind).toBe("live");
    expect(useComparisonStore.getState().sourceLabel).toBe("live: snap-a -> snap-b");
  });

  it("lists display-safe snapshot names and seeds current from the exact snapshot identity", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    useInvestigationStore.setState({
      persistenceIdentity: { kind: "snapshot", key: "current-key" },
    });
    globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
      diffObjects: async () => createRawObjectDiff(),
    };
    globalWindow.window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "baseline-key",
          heap_path: "/private/heaps/baseline.hprof",
          created_at: "2026-09-15T00:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 10,
          has_field_data: false,
        },
        {
          schema_version: 1,
          heap_sha256: "current-key",
          heap_path: "/private/heaps/current.hprof",
          created_at: "2026-09-15T01:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 12,
          has_field_data: true,
        },
      ],
    };

    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    expect(await page.findAllByRole("option", { name: /current\.hprof/i })).toHaveLength(2);
    expect(page.getByRole("combobox", { name: /^current snapshot$/i })).toHaveValue("current-key");
    expect(page.getByRole("combobox", { name: /^baseline snapshot$/i })).toHaveValue("");
    expect(page.queryByText(/\/private\/heaps/i)).not.toBeInTheDocument();
  });

  it("submits identity strategy, top-N, and leak cross-reference", async () => {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    const calls: DiffObjectsInput[] = [];
    globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
      diffObjects: async (input) => {
        calls.push(input);
        return createRawObjectDiff();
      },
    };
    globalWindow.window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      listSnapshots: async () => [
        {
          schema_version: 1,
          heap_sha256: "baseline-key",
          heap_path: "baseline.hprof",
          created_at: "2026-09-15T00:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 10,
          has_field_data: false,
        },
        {
          schema_version: 1,
          heap_sha256: "current-key",
          heap_path: "current.hprof",
          created_at: "2026-09-15T01:00:00Z",
          mnemosyne_version: "0.5.0",
          object_count: 12,
          has_field_data: true,
        },
      ],
    };

    const user = userEvent.setup();
    const view = render(<ComparisonPicker />);
    const page = within(view.container);

    await page.findAllByRole("option", { name: /current\.hprof/i });
    await user.selectOptions(page.getByRole("combobox", { name: /^current snapshot$/i }), "current-key");
    await user.selectOptions(page.getByRole("combobox", { name: /^baseline snapshot$/i }), "baseline-key");
    await user.selectOptions(page.getByRole("combobox", { name: /^identity strategy$/i }), "FullFingerprint");
    await user.clear(page.getByRole("spinbutton", { name: /^top n$/i }));
    await user.type(page.getByRole("spinbutton", { name: /^top n$/i }), "25");
    await user.click(page.getByRole("checkbox", { name: /cross-reference leaks/i }));
    await user.click(page.getByRole("button", { name: /run live diff/i }));

    await waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toEqual({
      beforeKey: "baseline-key",
      afterKey: "current-key",
      strategy: "FullFingerprint",
      topN: 25,
      crossReferenceLeaks: true,
    });
  });
});
