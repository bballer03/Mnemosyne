# Milestone 10-B — Object-Growth CI Predicate & Leak-Progression Cross-Reference

> **Status:** ✅ Shipped — design authored 2026-08-18, implemented and merged 2026-08-19 (single slice, no sub-slicing, per §4/§5 below).
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M10 follow-up, tracked as "M10-B" since the [M10 doc-sync review](milestone-8-1-object-level-diff.md) found this was in the original roadmap's M10 scope text but never implemented in slices A–G.
> **Predecessor:** M10 (Object-Level Diff) ✅ shipped — `core::diff::object` (`ObjectFingerprint`, `ObjectDelta`, `ObjectDiffReport`, `IdentityStrategy`), `mnemosyne diff --mode object`, MCP `diff_heaps` `mode: "object"`.
> **Last updated:** 2026-08-19

---

## 1. Objective

Roadmap.md's original M10 scope text named two capabilities that M10's actual implementation (slices A–G) did not ship:

1. **`ci-check object_growth_threshold` predicate** — fail CI when a tracked object (or class of objects) grows beyond a per-class retained-size limit between two snapshots.
2. **Leak-progression cross-reference** — connect object-level diff output with `detect_leaks()`'s existing suspect ranking, so an operator can see not just "this object grew" but "this object grew *and* is already a flagged leak suspect."

Both build directly on M10's shipped `core::diff::object` infrastructure. Neither requires new fingerprinting or diff logic — this is composition, matching the discipline every M8/M9/M11/M13 slice has followed.

## 2. Context

### 2.1 The architectural gap this design resolves

`mnemosyne ci-check <heap> --policy <file>` takes exactly **one** heap path (`CiCheckArgs.heap`, confirmed in `cli/src/main.rs`). `core::policy::evaluate(policy: &Policy, input: &PolicyInput, requested_mode: AnalysisMode) -> PolicyResult` evaluates against a single `PolicyInput::{Deep(&AnalyzeResponse), Overview(&OverviewSummary)}`. Every existing predicate (`total_bytes`, `leak_count`, `classloader_leak_count`, …) is a single-heap assertion.

`object_growth_threshold` is fundamentally a **two-heap** predicate — it cannot be evaluated without a baseline to diff against. This is a genuine new input shape for `ci-check`, not just a new predicate variant, and is the reason this capability was harder to ship than M10's other seven predicates-worth of scope and was correctly deferred rather than rushed into slices A–G.

### 2.2 Resolution: opt-in `--baseline` flag, additive `evaluate()` parameter

- `ci-check` gains an optional `--baseline <BEFORE_HEAP>` flag. Absent by default — every existing `ci-check` invocation is unaffected.
- If the loaded policy contains an `object_growth_threshold` rule and `--baseline` was **not** supplied, `ci-check` fails loudly with a structured `object_growth_threshold_requires_baseline` error (exit code 2, same "invalid policy" family as today's malformed-TOML failure) — never silently skips the rule. This matches the honesty-first pattern every deep-only predicate already uses (silent skip is reserved for *mode* mismatches like overview vs. deep, not for *missing required input*).
- If `--baseline` **is** supplied, `ci-check` runs `core::diff::run_diff(DiffRequest::object(baseline, heap, ..))` (M10's existing entry point, `DiffMode::Object`, default `IdentityStrategy::ClassDominator`) once, up front, regardless of whether other rules need it — same "compute once, evaluate all rules against it" shape the existing single-heap `analyze_heap()` call already has.
- `core::policy::evaluate()` gains one new parameter: `object_diff: Option<&ObjectDiffReport>`. This is additive to the function signature (a handful of call sites: `handle_ci_check` in `cli/src/main.rs`, the MCP `ci_check`-equivalent handler if one exists, and `core::policy`'s own test helpers) rather than folded into the `PolicyInput` enum, because `PolicyInput` is matched exhaustively in several places and widening it would ripple further than a single new optional parameter does. Every existing call site passes `None` and is otherwise unaffected — `evaluate()`'s existing behavior for all ten current predicates is byte-identical when `object_diff` is `None`.

### 2.3 Predicate semantics

```toml
[[rule]]
id = "no-runaway-cache-growth"
predicate = "object_growth_threshold"
class = "com.example.CacheEntry"   # NEW: object_growth_threshold is the first predicate scoped to a specific class name, not the whole heap
op = "<="
value = 10485760                   # bytes; max allowed |retained_changed| delta for any single tracked object of this class
severity = "error"
```

- Evaluated against `object_diff.retained_changed` (M10's existing `ObjectDelta` list): for every entry whose `class_name` matches the rule's `class` field, compare `|after_retained_bytes - before_retained_bytes|` against the threshold. A rule with **no** `class` field applies to every `retained_changed` entry regardless of class (documented as the "any tracked object" mode).
- Also checks `object_diff.added`: a newly-added object of the matched class with `retained_bytes >= threshold` counts as a violation too (an object that "grew from nothing" is still growth, and skipping `added` would let the most severe case — a leak that only started between snapshots — through undetected).
- `object_diff.removed` is **not** evaluated by this predicate (shrinking is never a growth violation; a dedicated `object_shrink_*` predicate is out of scope here and not requested by the roadmap).
- Multiple violating objects within one rule are reported as **one** `Violation` with an aggregate count and the single worst (largest-delta) object cited by id — matching the existing single-violation-per-rule shape every other predicate already produces, rather than inventing a multi-violation-per-rule shape used nowhere else in `core::policy`.
- Mode requirement: deep-only on **both** heaps (object diff already requires deep mode on both sides per M10's own `feature_unavailable_in_overview_mode` contract) — `rule_requires_deep()` gains `Predicate::ObjectGrowthThreshold` alongside the existing `LeakCount`/`RetainedSize`/`DominatorRootCount`/`ClassloaderLeakCount` entries.

### 2.4 Leak-progression cross-reference

Extend `core::diff::object::ObjectDelta` (M10, shipped) with one new additive field:

```rust
pub struct ObjectDelta {
    // ...existing fields unchanged (class_name, fingerprint, example_object_id,
    // before_count, after_count, before_retained_bytes, after_retained_bytes,
    // dominator_chain, reference_chain, kind)...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leak_severity: Option<LeakSeverity>,   // NEW
}
```

Populated by a new function `core::diff::object::annotate_leak_progression(deltas: &mut [ObjectDelta], leak_suspects: &[LeakInsight])`, called once from `run_diff`'s object-mode path **only when the caller opts in** (a new `DiffRequest.cross_reference_leaks: bool`, default `false`, additive — every existing `diff --mode object` invocation is unaffected). When enabled, `run_diff` calls `detect_leaks()` on the "after" heap (already-shipped, unmodified function) and matches each `ObjectDelta.class_name` against `LeakInsight.class_name`, setting `leak_severity` to the matched suspect's `LeakSeverity` when found.

CLI surface: `mnemosyne diff --mode object --cross-reference-leaks` (additive flag on the existing `diff` subcommand). Text output adds a `[LEAK: <severity>]` suffix to any `added`/`retained_changed` line whose `leak_severity` is populated — same inline-annotation style already used elsewhere in this codebase's text renderers (e.g. provenance markers).

## 3. Scope

In:
1. `Predicate::ObjectGrowthThreshold` in `core::policy`, with the `class`-scoping extension to `PolicyRule` (new optional `class: Option<String>` field on `PolicyRule`, additive — every existing rule type ignores it).
2. `evaluate()` gains `object_diff: Option<&ObjectDiffReport>` parameter (additive to the function, not the enum).
3. `ci-check --baseline <heap>` CLI flag; `object_growth_threshold_requires_baseline` structured error when a policy needs it and `--baseline` is absent.
4. `ObjectDelta.leak_severity: Option<LeakSeverity>` (M10 type, additive field) + `annotate_leak_progression()` + `DiffRequest.cross_reference_leaks: bool` + CLI `--cross-reference-leaks` flag on `diff`.
5. Tests: predicate fires/stays clean on synthetic before/after fixture pairs (reuse M10's `pure-add`/`retained-grow` fixture shapes from `resources/test-fixtures/diff/`); `--baseline` missing with a growth rule present returns the structured error, not a silent skip; leak-progression annotation matches an independently-computed reference on a fixture with a known leak-suspect class.

Out:
- MCP wiring for `--baseline`/`--cross-reference-leaks` (CLI-first for this follow-up, matching how M10's own `diff --mode object` shipped CLI before MCP received the equivalent `mode` param in the same slice — here, MCP wiring is deferred to a small future slice if usage justifies it, since this is already a follow-up to a follow-up).
- Multi-baseline / trend analysis across 3+ snapshots (M14 territory per roadmap.md, unrelated to this predicate).
- Per-object (not per-class) growth thresholds (the `class`-scoped rule already covers the realistic "watch this specific cache class" use case named in the original roadmap text; per-object-id thresholds would be meaningless across snapshots anyway since M10's own design doc §3.3 established that raw object ids are never stable identity).

## 4. Implementation plan (single slice — scope is small enough not to warrant sub-slicing)

- **Files owned:** `core/src/policy/{mod.rs,eval.rs,input.rs}`, `core/src/diff/object/{mod.rs,types.rs,engine.rs}`, `core/src/diff/mod.rs` (`DiffRequest.cross_reference_leaks`), `cli/src/main.rs` (`--baseline` on `ci-check`, `--cross-reference-leaks` on `diff`), new test files under `core/tests/` and `cli/tests/`.
- **Validation gates:** every existing `ci-check`/`diff --mode object` test (no new flags) passes byte-identically (hard regression gate, same discipline as every prior slice this session). New predicate fires on a deliberately-growing fixture, stays clean on a stable one. `--baseline` omission with a growth rule present is a loud, structured failure. Leak-progression annotation is correct against a reference computation and is `None` for every entry when `cross_reference_leaks` is not set (default-off, byte-identical to pre-M10-B `diff` output).
- Full `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean before commit, per this session's established discipline.
- Documentation sync (roadmap M10 row, STATUS/CHANGELOG/README/ARCHITECTURE/user-guide) is part of this same slice given the scope is small enough not to warrant a separate closeout pass — unlike M8/M9/M11/M13, no multi-slice breakdown is needed here.

## 5. Implementation readiness verdict

**READY.** Single-slice scope: extends already-shipped M10 (`core::diff::object`) and M7-2 (`core::policy`) infrastructure with one new predicate, one new opt-in CLI-input channel, and one new opt-in cross-reference annotation. No new algorithms, no new persistence, no new MCP surface in this pass.

## 6. Slice closeout (2026-08-19)

**Shipped as designed**, single slice, no sub-slicing needed. `PolicyRule.class: Option<String>` already existed (added ahead of schedule for `class_bytes`/`class_instances`), so §3.1's "new field" turned out to be a no-op verification rather than a code change -- everything else landed per §2-§4.

- `core::policy`: `Predicate::ObjectGrowthThreshold` (12th predicate, `object_growth_threshold`); `evaluate()` gained the additive `object_diff: Option<&ObjectDiffReport>` parameter. Call sites updated: `cli/src/main.rs::handle_ci_check` (both the `Overview` and `Deep` branches), plus `core/src/policy/eval.rs`'s own 30 internal test call sites (all pass `None` except the 8 new `object_growth_threshold`-specific tests, which pass `Some(&report)`). No MCP handler called `evaluate()` directly, confirming the design doc's own note that none exists.
- `core::diff`: `ObjectDelta.leak_severity: Option<LeakSeverity>` (additive), `core::diff::object::annotate_leak_progression()`, `DiffRequest.cross_reference_leaks: bool` (default `false`). Every existing `DiffRequest` struct-literal construction site across the workspace (9 total: `core::diff::mod::DiffRequest::class()`, `cli/src/main.rs` x2, `core/src/mcp/server.rs` x2, `core/src/workflow/compare_snapshots.rs`, `core/benches/diff_object.rs`, `core/tests/diff_object_engine.rs`, `core/tests/diff_object_real_fixtures.rs`, `core/tests/workflow_compare_snapshots.rs`) updated to set the field explicitly (Rust struct literals have no partial-default shortcut without deriving `Default`, which `DiffRequest` does not).
- `cli`: `ci-check --baseline <BEFORE_HEAP>` (optional); `object_growth_threshold_requires_baseline` structured error via the existing `CoreError::ConfigError` + `exit_ci_check_with_error(2, ...)` path, same family as a malformed policy TOML. `diff --mode object --cross-reference-leaks` flag wired through to `DiffRequest`; text renderer appends `[LEAK: <SEVERITY>]`.
- Tests added (15 new, TDD-first): 8 in `core/src/policy/eval.rs` (fires on `retained_changed` growth, fires on `added` growth, stays clean on a stable diff, class filter, no-class-applies-to-all, multi-violator aggregation citing the worst offender, skipped when `object_diff` is absent, deep-only skip on overview input); 1 in `core/tests/diff_object_engine.rs` (`annotate_leak_progression` verified against an independently computed max-severity-per-class reference, using `detect_leaks_from_graph` directly with a low severity floor rather than `run_diff`'s default `LeakSeverity::High` floor, which needs gigabyte-scale retained size to reach and is impractical for a synthetic fixture); 2 in `core/tests/report_diff_renderers.rs` (text renderer emits/omits `[LEAK: ...]` correctly); 2 in `cli/tests/integration.rs` (`--baseline` missing returns exit 2 with the structured error; `--baseline` present evaluates instead of skipping); 2 in `cli/tests/diff_object_cli.rs` (regression lock: no `[LEAK:` marker without the flag; `--cross-reference-leaks` wiring smoke test).
- **Known limitation, not a gap in this design:** `run_diff`'s `cross_reference_leaks` path uses `LeakDetectionOptions::default()` (severity floor `High`, gigabyte-scale retained size) rather than a caller-tunable threshold, since the design doc specified reusing `detect_leaks()` "unmodified" with no new options surface. In practice this means the cross-reference only fires for genuinely severe leak suspects -- a reasonable, conservative default, but one a future slice could make configurable if usage shows it's too conservative.
- **Verification (this machine, 2026-08-19):** `cargo check --workspace --all-targets` clean. `cargo test --workspace`: **729 passed, 0 failed, 0 ignored** across all core/cli unit and integration test binaries (up from 15 new tests added this slice; doc-tests: 0). `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean (after one `cargo fmt --all` pass).
- Documentation synced in the same slice per §4: `docs/roadmap.md` (M10 row flipped to shipped, M10-B forward-references removed), `STATUS.md`, `CHANGELOG.md` (`[Unreleased]`), `README.md`, `ARCHITECTURE.md`, `docs/user-guide.md` (`ci-check`/`diff` sections extended), and this file's own status header.
