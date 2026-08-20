// TypeScript types + parsers for Mnemosyne's M10 object-level heap diff
// output (`mnemosyne diff --mode object --format json`), grounded against
// the real Rust types (not illustrative): `core/src/diff/object/types.rs`
// (`ObjectDiffReport`, `ObjectDelta`, `MatchQuality`, `ObjectDiffTotals`,
// `IdentityStrategy`, `ObjectDeltaKind`, `Risk`) and the wrapping
// `core/src/hprof/parser.rs::HeapDiff` (the top-level shape the CLI's
// `render_json` serializes via `serde_json::to_string_pretty`).
//
// Field-naming convention (verified by reading the Rust source and by
// running the built CLI against synthetic fixtures -- see the Slice 14.A
// commit body): none of these Rust structs carry
// `#[serde(rename_all = "camelCase")]`, so JSON keys are the struct's own
// (already snake_case) field names as-is, e.g. `retained_bucket_bits`,
// `example_object_id`, `dominator_chain`. Enum variants likewise serialize
// with serde's default (no rename), i.e. their Rust PascalCase spelling,
// e.g. `"ClassDominator"`, `"RetainedChanged"`, `"Low"`. The one exception
// is `LeakSeverity` (`core/src/analysis/engine.rs`), which *does* carry
// `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`, so `ObjectDelta.leak_severity`
// serializes as `"LOW" | "MEDIUM" | "HIGH" | "CRITICAL"` -- distinct from
// every other enum in this file.
//
// This mirrors the same manual snake_case-JSON -> camelCase-TS boundary
// already established in `analysis-types.ts` for `AnalysisArtifact` --
// there is no Rust-side renaming layer; normalization happens once, here,
// at parse time.

export type IdentityStrategy = "ClassRetained" | "ClassDominator" | "FullFingerprint";

export type ObjectDeltaKind = "Added" | "Removed" | "RetainedChanged";

export type Risk = "Low" | "Medium" | "High";

export type LeakSeverity = "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";

export type ObjectFingerprint = {
  classId: number;
  retainedBucket: number;
  dominatorSignature: number;
  fieldSignature: number;
};

export type MatchQuality = {
  strategy: IdentityStrategy;
  collisionRate: number;
  estimatedFalseMatchRisk: Risk;
  estimatedFalseSplitRisk: Risk;
  notes: string[];
};

export type ObjectDiffTotals = {
  beforeObjectCount: number;
  afterObjectCount: number;
  fingerprintCollisionsBefore: number;
  fingerprintCollisionsAfter: number;
  matchedPairs: number;
};

export type ObjectDelta = {
  className: string;
  fingerprint: ObjectFingerprint;
  exampleObjectId: number;
  beforeCount: number;
  afterCount: number;
  beforeRetainedBytes: number;
  afterRetainedBytes: number;
  dominatorChain: string[];
  referenceChain: string[];
  kind: ObjectDeltaKind;
  leakSeverity?: LeakSeverity;
};

export type ObjectDiffReport = {
  strategy: IdentityStrategy;
  retainedBucketBits: number;
  retainedChangeThreshold: number;
  matchQuality: MatchQuality;
  added: ObjectDelta[];
  removed: ObjectDelta[];
  retainedChanged: ObjectDelta[];
  totals: ObjectDiffTotals;
};

/** The subset of `HeapDiff` (`core/src/hprof/parser.rs`) the comparison
 * basket cares about -- the full CLI `diff --format json` payload, with
 * `object_diff` present only when `--mode object` was used. */
export type HeapDiffArtifact = {
  before: string;
  after: string;
  objectDiff: ObjectDiffReport;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be a string`);
  }

  return value;
}

function readNumber(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be a number`);
  }

  return value;
}

function readOptionalString(value: unknown, field: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }

  return readString(value, field);
}

function readStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an array`);
  }

  return value.map((entry, index) => readString(entry, `${field}[${index}]`));
}

function readArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an array`);
  }

  return value;
}

function readIdentityStrategy(value: unknown, field: string): IdentityStrategy {
  const raw = readString(value, field);

  if (raw === "ClassRetained" || raw === "ClassDominator" || raw === "FullFingerprint") {
    return raw;
  }

  throw new Error(`Invalid Mnemosyne object diff report: unexpected ${field} value "${raw}"`);
}

function readRisk(value: unknown, field: string): Risk {
  const raw = readString(value, field);

  if (raw === "Low" || raw === "Medium" || raw === "High") {
    return raw;
  }

  throw new Error(`Invalid Mnemosyne object diff report: unexpected ${field} value "${raw}"`);
}

function readDeltaKind(value: unknown, field: string): ObjectDeltaKind {
  const raw = readString(value, field);

  if (raw === "Added" || raw === "Removed" || raw === "RetainedChanged") {
    return raw;
  }

  throw new Error(`Invalid Mnemosyne object diff report: unexpected ${field} value "${raw}"`);
}

function readOptionalLeakSeverity(value: unknown, field: string): LeakSeverity | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }

  const raw = readString(value, field);

  if (raw === "LOW" || raw === "MEDIUM" || raw === "HIGH" || raw === "CRITICAL") {
    return raw;
  }

  throw new Error(`Invalid Mnemosyne object diff report: unexpected ${field} value "${raw}"`);
}

function parseFingerprint(value: unknown, field: string): ObjectFingerprint {
  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an object`);
  }

  return {
    classId: readNumber(value.class_id, `${field}.class_id`),
    retainedBucket: readNumber(value.retained_bucket, `${field}.retained_bucket`),
    dominatorSignature: readNumber(value.dominator_signature, `${field}.dominator_signature`),
    fieldSignature: readNumber(value.field_signature, `${field}.field_signature`),
  };
}

function parseMatchQuality(value: unknown, field: string): MatchQuality {
  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an object`);
  }

  return {
    strategy: readIdentityStrategy(value.strategy, `${field}.strategy`),
    collisionRate: readNumber(value.collision_rate, `${field}.collision_rate`),
    estimatedFalseMatchRisk: readRisk(value.estimated_false_match_risk, `${field}.estimated_false_match_risk`),
    estimatedFalseSplitRisk: readRisk(value.estimated_false_split_risk, `${field}.estimated_false_split_risk`),
    notes: readStringArray(value.notes, `${field}.notes`),
  };
}

function parseTotals(value: unknown, field: string): ObjectDiffTotals {
  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an object`);
  }

  return {
    beforeObjectCount: readNumber(value.before_object_count, `${field}.before_object_count`),
    afterObjectCount: readNumber(value.after_object_count, `${field}.after_object_count`),
    fingerprintCollisionsBefore: readNumber(
      value.fingerprint_collisions_before,
      `${field}.fingerprint_collisions_before`,
    ),
    fingerprintCollisionsAfter: readNumber(
      value.fingerprint_collisions_after,
      `${field}.fingerprint_collisions_after`,
    ),
    matchedPairs: readNumber(value.matched_pairs, `${field}.matched_pairs`),
  };
}

function parseObjectDelta(value: unknown, field: string): ObjectDelta {
  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne object diff report: expected ${field} to be an object`);
  }

  return {
    className: readString(value.class_name, `${field}.class_name`),
    fingerprint: parseFingerprint(value.fingerprint, `${field}.fingerprint`),
    exampleObjectId: readNumber(value.example_object_id, `${field}.example_object_id`),
    beforeCount: readNumber(value.before_count, `${field}.before_count`),
    afterCount: readNumber(value.after_count, `${field}.after_count`),
    beforeRetainedBytes: readNumber(value.before_retained_bytes, `${field}.before_retained_bytes`),
    afterRetainedBytes: readNumber(value.after_retained_bytes, `${field}.after_retained_bytes`),
    dominatorChain: readStringArray(value.dominator_chain, `${field}.dominator_chain`),
    referenceChain: readStringArray(value.reference_chain, `${field}.reference_chain`),
    kind: readDeltaKind(value.kind, `${field}.kind`),
    leakSeverity: readOptionalLeakSeverity(value.leak_severity, `${field}.leak_severity`),
  };
}

function parseDeltaArray(value: unknown, field: string): ObjectDelta[] {
  return readArray(value, field).map((entry, index) => parseObjectDelta(entry, `${field}[${index}]`));
}

export function parseObjectDiffReport(input: unknown): ObjectDiffReport {
  if (!isRecord(input)) {
    throw new Error("Invalid Mnemosyne object diff report: expected a JSON object");
  }

  return {
    strategy: readIdentityStrategy(input.strategy, "strategy"),
    retainedBucketBits: readNumber(input.retained_bucket_bits, "retained_bucket_bits"),
    retainedChangeThreshold: readNumber(input.retained_change_threshold, "retained_change_threshold"),
    matchQuality: parseMatchQuality(input.match_quality, "match_quality"),
    added: parseDeltaArray(input.added, "added"),
    removed: parseDeltaArray(input.removed, "removed"),
    retainedChanged: parseDeltaArray(input.retained_changed, "retained_changed"),
    totals: parseTotals(input.totals, "totals"),
  };
}

/**
 * Parses the full `mnemosyne diff --mode object --format json` payload
 * (the `HeapDiff` shape) and extracts the `object_diff` section. Throws an
 * explicit, actionable error when the file was produced without
 * `--mode object` (so `object_diff` is absent) -- surfaced as a load error
 * in the comparison basket UI rather than a silent blank screen.
 */
export function parseHeapDiffArtifact(input: unknown): HeapDiffArtifact {
  if (!isRecord(input)) {
    throw new Error("Invalid Mnemosyne diff artifact: expected a JSON object");
  }

  const before = readOptionalString(input.before, "before") ?? "";
  const after = readOptionalString(input.after, "after") ?? "";

  if (input.object_diff === undefined || input.object_diff === null) {
    throw new Error(
      "Invalid Mnemosyne diff artifact: no object_diff section present. " +
        "Re-run `mnemosyne diff --mode object --format json` (the default --mode is class-level and omits object_diff).",
    );
  }

  return {
    before,
    after,
    objectDiff: parseObjectDiffReport(input.object_diff),
  };
}
