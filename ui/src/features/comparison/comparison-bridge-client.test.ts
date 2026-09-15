import "../../test/setup";

import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import {
  isDiffObjectsAvailable,
  runDiffObjects,
  type DiffObjectsInput,
} from "./comparison-bridge-client";

function diffInput(overrides: Partial<DiffObjectsInput> = {}): DiffObjectsInput {
  return {
    beforeKey: "a",
    afterKey: "b",
    strategy: "ClassDominator",
    topN: 50,
    crossReferenceLeaks: false,
    ...overrides,
  };
}

describe("comparison bridge client", () => {
  const globalWindow = globalThis as typeof globalThis & {
    window?: Window & {
      __MNEMOSYNE_COMPARISON_BRIDGE__?: unknown;
    };
  };

  function clearComparisonBridge() {
    if (globalWindow.window) {
      delete globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__;
    }
  }

  function setComparisonBridge(bridge: NonNullable<Window["__MNEMOSYNE_COMPARISON_BRIDGE__"]>) {
    if (!globalWindow.window) {
      throw new Error("Expected window to exist in UI tests.");
    }

    globalWindow.window.__MNEMOSYNE_COMPARISON_BRIDGE__ = bridge;
  }

  beforeEach(() => {
    clearComparisonBridge();
  });

  afterEach(() => {
    clearComparisonBridge();
  });

  it("isDiffObjectsAvailable returns false when no comparison bridge exists", () => {
    expect(isDiffObjectsAvailable()).toBeFalse();
  });

  it("isDiffObjectsAvailable returns true when the bridge exposes diffObjects", () => {
    setComparisonBridge({
      diffObjects: async () => ({}),
    });

    expect(isDiffObjectsAvailable()).toBeTrue();
  });

  it("runDiffObjects returns unavailable when no bridge exists", async () => {
    expect(await runDiffObjects(diffInput())).toEqual({
      status: "unavailable",
    });
  });

  it("runDiffObjects parses a valid object diff report from the bridge", async () => {
    setComparisonBridge({
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
    });

    const result = await runDiffObjects(diffInput({ beforeKey: "snap-a", afterKey: "snap-b" }));

    expect(result.status).toBe("ready");
    if (result.status === "ready") {
      expect(result.data.strategy).toBe("ClassDominator");
      expect(result.data.added).toEqual([]);
    }
  });

  it("runDiffObjects returns an error when the bridge throws", async () => {
    setComparisonBridge({
      diffObjects: async () => {
        throw new Error("bridge down");
      },
    });

    expect(await runDiffObjects(diffInput())).toEqual({
      status: "error",
      error: "bridge down",
    });
  });

  it("runDiffObjects returns an error when the bridge payload is malformed", async () => {
    setComparisonBridge({
      diffObjects: async () => ({ strategy: "NotARealStrategy" }),
    });

    const result = await runDiffObjects(diffInput());

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error).toContain("unexpected strategy value");
    }
  });

  it("forwards identity strategy, top-N, and leak cross-reference unchanged", async () => {
    const calls: DiffObjectsInput[] = [];
    setComparisonBridge({
      diffObjects: async (input) => {
        calls.push(input);
        return {
          strategy: "FullFingerprint",
          retained_bucket_bits: 10,
          retained_change_threshold: 1048576,
          match_quality: {
            strategy: "FullFingerprint",
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
      },
    });

    await runDiffObjects(
      diffInput({
        beforeKey: "baseline-key",
        afterKey: "current-key",
        strategy: "FullFingerprint",
        topN: 25,
        crossReferenceLeaks: true,
      }),
    );

    expect(calls).toEqual([
      {
        beforeKey: "baseline-key",
        afterKey: "current-key",
        strategy: "FullFingerprint",
        topN: 25,
        crossReferenceLeaks: true,
      },
    ]);
  });
});
