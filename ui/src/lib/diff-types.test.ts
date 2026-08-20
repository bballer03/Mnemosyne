import { describe, expect, it } from "bun:test";

import { parseHeapDiffArtifact, parseObjectDiffReport } from "./diff-types";

const rawObjectDiffReport = {
  strategy: "ClassDominator",
  retained_bucket_bits: 10,
  retained_change_threshold: 1048576,
  match_quality: {
    strategy: "ClassDominator",
    collision_rate: 0.015,
    estimated_false_match_risk: "Low",
    estimated_false_split_risk: "Medium",
    notes: ["class+dominator adds four hops of dominator context to reduce sibling collisions"],
  },
  added: [
    {
      class_name: "com/example/CacheHolder",
      fingerprint: {
        class_id: 7,
        retained_bucket: 3,
        dominator_signature: 1234567,
        field_signature: 987654,
      },
      example_object_id: 4096,
      before_count: 0,
      after_count: 12,
      before_retained_bytes: 0,
      after_retained_bytes: 49152,
      dominator_chain: ["java/lang/Thread", "com/example/CacheHolder"],
      reference_chain: [],
      kind: "Added",
    },
  ],
  removed: [
    {
      class_name: "com/example/StaleEntry",
      fingerprint: {
        class_id: 9,
        retained_bucket: 1,
        dominator_signature: 222,
        field_signature: 333,
      },
      example_object_id: 8192,
      before_count: 4,
      after_count: 0,
      before_retained_bytes: 2048,
      after_retained_bytes: 0,
      dominator_chain: [],
      reference_chain: [],
      kind: "Removed",
    },
  ],
  retained_changed: [
    {
      class_name: "com/example/BigCache",
      fingerprint: {
        class_id: 11,
        retained_bucket: 5,
        dominator_signature: 555,
        field_signature: 666,
      },
      example_object_id: 2048,
      before_count: 2,
      after_count: 2,
      before_retained_bytes: 1024,
      after_retained_bytes: 2097152,
      dominator_chain: ["com/example/BigCache"],
      reference_chain: ["com/example/BigCache.entries"],
      kind: "RetainedChanged",
      leak_severity: "HIGH",
    },
  ],
  totals: {
    before_object_count: 6,
    after_object_count: 14,
    fingerprint_collisions_before: 0,
    fingerprint_collisions_after: 1,
    matched_pairs: 2,
  },
};

describe("parseObjectDiffReport", () => {
  it("parses the real ObjectDiffReport shape (verified against core/src/diff/object/types.rs) into camelCase TS", () => {
    const parsed = parseObjectDiffReport(rawObjectDiffReport);

    expect(parsed.strategy).toBe("ClassDominator");
    expect(parsed.retainedBucketBits).toBe(10);
    expect(parsed.retainedChangeThreshold).toBe(1048576);
    expect(parsed.matchQuality).toEqual({
      strategy: "ClassDominator",
      collisionRate: 0.015,
      estimatedFalseMatchRisk: "Low",
      estimatedFalseSplitRisk: "Medium",
      notes: ["class+dominator adds four hops of dominator context to reduce sibling collisions"],
    });

    expect(parsed.added).toHaveLength(1);
    expect(parsed.added[0]).toEqual({
      className: "com/example/CacheHolder",
      fingerprint: {
        classId: 7,
        retainedBucket: 3,
        dominatorSignature: 1234567,
        fieldSignature: 987654,
      },
      exampleObjectId: 4096,
      beforeCount: 0,
      afterCount: 12,
      beforeRetainedBytes: 0,
      afterRetainedBytes: 49152,
      dominatorChain: ["java/lang/Thread", "com/example/CacheHolder"],
      referenceChain: [],
      kind: "Added",
      leakSeverity: undefined,
    });

    expect(parsed.removed).toHaveLength(1);
    expect(parsed.removed[0]?.kind).toBe("Removed");

    expect(parsed.retainedChanged).toHaveLength(1);
    expect(parsed.retainedChanged[0]?.leakSeverity).toBe("HIGH");

    expect(parsed.totals).toEqual({
      beforeObjectCount: 6,
      afterObjectCount: 14,
      fingerprintCollisionsBefore: 0,
      fingerprintCollisionsAfter: 1,
      matchedPairs: 2,
    });
  });

  it("parses an empty diff report (no added/removed/retained_changed entries)", () => {
    const parsed = parseObjectDiffReport({
      ...rawObjectDiffReport,
      added: [],
      removed: [],
      retained_changed: [],
    });

    expect(parsed.added).toEqual([]);
    expect(parsed.removed).toEqual([]);
    expect(parsed.retainedChanged).toEqual([]);
  });

  it("throws for a non-object input", () => {
    expect(() => parseObjectDiffReport(null)).toThrow(/expected a JSON object/);
  });

  it("throws for an unrecognized identity strategy value", () => {
    expect(() => parseObjectDiffReport({ ...rawObjectDiffReport, strategy: "Bogus" })).toThrow(
      /unexpected strategy value/,
    );
  });

  it("throws for a malformed object delta entry", () => {
    expect(() =>
      parseObjectDiffReport({
        ...rawObjectDiffReport,
        added: [{ class_name: "com/example/Broken" }],
      }),
    ).toThrow(/fingerprint/);
  });
});

describe("parseHeapDiffArtifact", () => {
  it("extracts object_diff from a full HeapDiff (`mnemosyne diff --format json`) payload", () => {
    const parsed = parseHeapDiffArtifact({
      before: "before.hprof",
      after: "after.hprof",
      delta_bytes: 371,
      delta_objects: 7,
      changed_classes: [],
      class_diff: [],
      object_diff: rawObjectDiffReport,
    });

    expect(parsed.before).toBe("before.hprof");
    expect(parsed.after).toBe("after.hprof");
    expect(parsed.objectDiff.strategy).toBe("ClassDominator");
    expect(parsed.objectDiff.added).toHaveLength(1);
  });

  it("throws an explicit, actionable error when object_diff is absent (class-mode diff, not object-mode)", () => {
    expect(() =>
      parseHeapDiffArtifact({
        before: "before.hprof",
        after: "after.hprof",
        delta_bytes: 0,
        delta_objects: 0,
        changed_classes: [],
      }),
    ).toThrow(/--mode object/);
  });
});
