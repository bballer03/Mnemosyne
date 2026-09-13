# Milestone 8-1 — Object-Level Heap Diff

> **Status:** 🟡 Slices A–G **shipped** (merged via PR #38 "object-fingerprint baseline (M8-1 Slice A)" and PR #41 "CLI integration (M8-1 Slice D)" — that merge bundled B/C/E/F/G alongside D). Slice H (this doc-sync pass) in progress.
> **Roadmap correspondence:** This design doc predates the post-v0.3.0 roadmap refresh (PR #36) and used a standalone `8-1.*` slice numbering scheme. The refreshed [docs/roadmap.md §5](../roadmap.md) calls this same body of work **M10 — Compare Two Heaps (Object-Level Diff)**. Treat "Slice 8-1.X" below as M10's internal slice numbering; there is no separate M8-1 milestone. `ci-check object_growth_threshold` (in roadmap M10 scope, not in this doc's §4 scope) remains unshipped — tracked as follow-up M10-B.
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M8+ backlog
> **Predecessor work:** M7-1 streaming overview mode ✅, M7 class-level diff ✅ (`core::analysis::engine::diff_heaps` + `ClassLevelDelta`)
> **Last updated:** 2026-04-26

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Slice | M8-1 |
| Phase | M8 — backlog promotion (the first M8 design doc to land) |
| Type | Parity (closes the last "❌" cell on the MAT scorecard table in [docs/roadmap.md §6](../roadmap.md)) |
| Predecessors | M7-1 ✅ (overview mode — defines the bounded-memory boundary for very large diffs); class-level diff ✅ shipped in `core::analysis::engine::{diff_heaps, ClassLevelDelta, build_heap_diff, compute_class_level_diff}` |
| Successors | M8-5 (multi-snapshot trends), M8-8 (persistent indexes), M8-9 (Tauri UI surfacing), M8-10 (streaming diff over MCP) |
| Touched crates | `core`, `cli`. No `tauri` / `ui` change in this milestone (see §13). |
| Test count target | +30 to +40 net new Rust tests across the eight sub-slices (push workspace from 448 → ≥ 478). |
| Memory budget | Object-level diff must work on the existing dense-tier ceiling: ≥ 2 GB heap dumps in deep mode, peak RSS ≤ 4× the deep-mode baseline of a single dump. See §8. |

## 2. Objective

After M8-1, a Mnemosyne user can answer the following questions from two heap dumps captured at different points in time:

1. **"Which retainer instances appeared between dump A and dump B?"**
   `mnemosyne diff before.hprof after.hprof --mode object` → list of objects present in B with no fingerprint match in A, sorted by retained bytes.
2. **"Which retainer instances disappeared between A and B?"**
   Same command, `removed` section.
3. **"Which retainer instances are still alive in both dumps but now retain materially more / less memory?"**
   `retained_changed` section (matched fingerprint, retained-size delta beyond a configurable threshold).
4. **"For each diff bucket, which class is dominant and where in the dominator tree does it live?"**
   Each `ObjectDelta` carries `class_name`, `dominator_signature`, and a short reference chain so the operator can jump straight to the suspect.

Today these questions cannot be answered. Today's `mnemosyne diff` only ships per-class aggregates: it tells the user that `com.example.UserSession` grew by `296 MB`, but it cannot tell them whether that growth is one new dominant instance or 10 000 small ones, and it cannot tell them whether the same dominator object survived the snapshot interval or was replaced.

M8-1 closes that gap with a fingerprint-based, opt-in object-level diff that runs on top of the existing class-level diff path without breaking it.

## 3. Context

### 3.1 What Mnemosyne ships today

Inspected files:

- [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `diff_heaps`, `build_heap_diff`, `compute_class_level_diff`, `collect_class_level_stats`, `diff_named_totals`, `try_build_dominator`. Existing entry point: `pub async fn diff_heaps(before_path: &str, after_path: &str) -> CoreResult<HeapDiff>`.
- [core/src/hprof/parser.rs](../../core/src/hprof/parser.rs) — `HeapDiff { before, after, delta_bytes, delta_objects, changed_classes, class_diff: Option<Vec<ClassLevelDelta>> }`, `ClassDelta`, `ClassLevelDelta`.
- [core/src/lib.rs](../../core/src/lib.rs) — `HeapDiff`, `ClassLevelDelta` re-exported.
- [cli/src/main.rs](../../cli/src/main.rs) — `Commands::Diff(DiffArgs { before: PathBuf, after: PathBuf })`, `handle_diff`, prints `delta_bytes`, `delta_objects`, `changed_classes`, optional `class_diff`. No JSON/TOON output for diff today.
- [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `list_tools` returns `parse_heap`, `detect_leaks`, `analyze_heap`, `query_heap`, `gc_root_path`, `map_leak`, `explain`, `chat`, `fix_leak`, etc. **No `diff_heaps` tool is registered today.** This is a known omission documented in M5/M7 follow-ups; M8-1 closes it.

The class-level diff path is sound and must be preserved exactly:

```text
diff_heaps
 ├─ parse_heap(before)            (lightweight summary)
 ├─ parse_heap(after)             (lightweight summary)
 ├─ build_heap_diff               (totals + changed_classes)
 ├─ try_build_dominator(before)?  (full HPROF graph + dominator tree)
 ├─ try_build_dominator(after)?   (full HPROF graph + dominator tree)
 └─ compute_class_level_diff      (ClassLevelDelta — present when both graphs build)
```

Object-level diff is strictly an additional pass that consumes the same two `(ObjectGraph, DominatorTree)` pairs and produces an `ObjectDelta`. Class-diff and object-diff are independent: object-diff degrades cleanly to `None` when either dump cannot build a dominator tree, and class-diff is unaffected by the new path.

### 3.2 What MAT does

Eclipse MAT supports a **"Compare Basket"** workflow: the user opens two `.hprof` files, sends both class histograms to a basket, and runs *Compare to another Heap Dump*. MAT produces:

1. A class-aggregated diff (Mnemosyne already has parity — M7 `class_diff`).
2. A *retained set comparison* between two dominator subtrees (Mnemosyne does not have this).
3. A *unique objects* report based on a heuristic that combines class identity + a hash of outbound references + the immediate dominator (Mnemosyne does not have this).
4. An *interactive merge* that lets the user pick equivalence rules per class (Mnemosyne explicitly skips this — see §13).

MAT does **not** rely on stable HPROF object IDs across dumps; it cannot, because HPROF IDs are not stable. Its identity heuristic is essentially `(class, immediate-dominator-class, outbound-reference-class-set-hash)` with optional retained-size bucketing. M8-1 deliberately matches that floor.

What we deliberately skip:
- Interactive equivalence-rule editing (manual merge UI is M8-9 territory if at all).
- Side-by-side dominator-tree visual diff (UI surface; deferred).
- Three-way diff and trend lines across ≥ 3 dumps (M8-5).

### 3.3 The central technical risk: stable object identity

**HPROF object IDs are not stable across dumps.** They are assigned by the JVM when the dump is written, and they reflect the live address (or the GC's compacted address) of the object at dump time. Two dumps of the same JVM, taken seconds apart, will assign different IDs to the same logical instance. Therefore: **object-level diff must never use HPROF IDs as identity.**

Identity has to be reconstructed from observable, dump-stable properties. Each candidate has a different false-match / false-split trade-off:

| Component | What it captures | False-match risk | False-split risk | Cost to compute |
|---|---|---|---|---|
| `class_id` (resolved to class name) | "same Java class" | Very high — every instance of `String` is identical here | None | O(1) per object — already on `HeapObject` |
| `retained_size` exact value | "same exact dominator-subtree byte total" | Medium — collisions across siblings of the same class | Very high — any churn in any descendant changes retained size | O(1) per object after dominator pass — already computed |
| `retained_size` log-bucketed (e.g., power-of-two ≥ 1 KB) | "same order-of-magnitude retainer" | High but tunable | Low when bucket is wide | O(1) per object |
| Immediate-dominator class chain (depth N, e.g., 4) | "same retainer context" | Medium — collisions among siblings under the same dominator | Low — chain is stable across snapshots provided the retention shape doesn't change | O(N) per object — N parent walks |
| Field-shape signature (sorted set of `(field_name, field_type, is_null_bit)`) | "same field layout and which refs are null" | Low for distinct subtypes; high when many Pojos share a layout | Medium — depends on how many fields actually flip null / non-null between snapshots | O(F) per object where F = field count; requires `retain_field_data: true` |
| Full outbound-reference class-set hash | "same outbound shape" | Very low | High — any reference churn invalidates it | O(R) per object where R = outbound refs |

**Pure HPROF ID matching is unsafe and is not offered as a strategy.** Even an `--identity-strategy hprof-id` flag would silently mislead users. We refuse to ship it.

### 3.4 Identity-strategy choices in M8-1

M8-1 ships three composable strategies. Each is a `BitOr` of the components in §3.3. The default is the strongest strategy that is always available in deep mode without requiring `retain_field_data`.

| Strategy | Components | Default? | When to use |
|---|---|---|---|
| `class+retained` | `class_id` + log-bucketed retained size | No (Slice 8-1.A baseline) | Fast, mode-honest in any deep dump. Coarsest of the three. |
| `class+dominator` | `class_id` + log-bucketed retained size + immediate-dominator class chain (depth 4) | **Yes (default after Slice 8-1.B)** | Best stability/cost trade-off. Default. |
| `full-fingerprint` | `class+dominator` + field-shape signature + outbound-class-set hash | No | Maximum precision. Requires `--retain-field-data` (slower parse, higher RSS). For the operator who already accepts deep-mode cost. |

**Default rationale:** `class+dominator` is the one strategy that:
1. Catches the failure mode of `class+retained` (sibling cache instances of the same retained bucket are no longer indistinguishable — they each have their own dominator class chain).
2. Does **not** require `retain_field_data` (which doubles deep-mode peak RSS in current measurements).
3. Is fully available the moment Slice 8-1.B lands.

Until Slice 8-1.B ships, the default is `class+retained` (Slice 8-1.A's only available strategy). The default flips to `class+dominator` on the same PR that lands Slice 8-1.B; this is documented in §11 / §15.

### 3.5 False-match vs false-split semantics

Every fingerprint strategy faces a tension:

- **False match** = two distinct logical instances collapse into one identity. This *understates* `added` and `removed` and *overstates* `retained_changed`.
- **False split** = the same logical instance produces different fingerprints in A and B. This *overstates* both `added` and `removed`.

M8-1's policy:

1. **Bias toward false-match** in the default strategy. Operators who want sharper identity opt in to `full-fingerprint`. Reasoning: a false-match in the default report still shows the right class and right retained-size bucket; the operator can drill in. A false-split inflates `added`/`removed` lists with phantom churn that wastes triage time.
2. **Always emit a `match_quality` field** per `ObjectDelta` and per top-level diff so the operator can see which strategy ran and what its known limitations are. See §6.
3. **Never silently fall back** between strategies. If the requested strategy can't run (e.g., `full-fingerprint` without `retain_field_data`), return a structured error (`feature_unavailable_without_field_data`), not a quiet downgrade.

## 4. Scope

In:

1. New module `core::diff::object` (sibling of the existing class-diff code in `core::analysis::engine`). Contains `ObjectFingerprint`, `ObjectDelta`, the three identity strategies, and the diff engine.
2. Refactor: lift the existing `compute_class_level_diff` and `collect_class_level_stats` out of `core::analysis::engine` and into a new `core::diff::class` submodule, behind a re-export so the existing `core::analysis::engine::diff_heaps` signature is byte-identical. Source of truth for class-diff is now `core::diff::class`. (Slice 8-1.A.)
3. New `core::diff::DiffMode` enum: `Class` (default) or `Object`. Class-mode behavior matches today exactly.
4. CLI flag surface (additive) on `mnemosyne diff`:
   - `--mode {class,object}` (default `class`).
   - `--identity-strategy {class+retained,class+dominator,full-fingerprint}` (default `class+dominator`; ignored when `--mode class`).
   - `--retained-bucket-bits <u8>` (default `10`, meaning power-of-two bucket of `1 KB`).
   - `--retained-change-threshold <bytes>` (default `1_048_576` = 1 MB; cutoff for `retained_changed`).
   - `--top <n>` (default `50`; per-section cap for `added`, `removed`, `retained_changed`).
   - `--retain-field-data` (default `false`; required for `full-fingerprint`).
   - `--format {text,json,toon}` (default `text` — additive; today's diff is text-only).
5. MCP method `diff_heaps` registered in `core::mcp::server` — schema and naming in §7.
6. Reporting: text/JSON/TOON renderers under `core::report::diff` (new submodule) — see §10.
7. Validation: synthetic fixtures under `resources/test-fixtures/diff/` exercising known additions, removals, and retained-size changes; regression boundary against existing class-diff tests in `core/src/analysis/engine.rs` and CLI tests in `cli/tests/`.

Out:

- **No multi-snapshot trends.** Three-or-more-snapshot timelines belong to M8-5. M8-1 is strictly two-snapshot.
- **No persistent on-disk index of fingerprints.** Recomputed per invocation. Persistent indexes are M8-8.
- **No streaming object-diff across overview mode.** Object-level diff requires deep mode on both dumps; overview mode returns a structured `feature_unavailable_in_overview_mode` error envelope. Streaming diff is M8-10.
- **No Tauri UI / browser UI surfacing of object diff.** M8-9 owns UI work. JSON envelope shape is designed to be UI-ready, but no UI code is written here.
- **No interactive equivalence-rule editing.** MAT-style "merge by hand" is intentionally skipped.
- **No breaking change to today's `mnemosyne diff` defaults.** Without `--mode object`, behavior, exit codes, and output are byte-identical to v0.3.0.
- **No edits to `docs/roadmap.md`.** PR #36 owns roadmap state; documentation sync for M8-1 happens after #36 lands (Slice 8-1.H).

## 5. Architecture overview

```
                      ┌──────────────────────────────────────────┐
                      │ mnemosyne diff <before> <after>          │  CLI: cli/src/main.rs
                      │  --mode {class,object}                   │
                      │  --identity-strategy …                   │
                      │  --format {text,json,toon}               │
                      └──────────────┬───────────────────────────┘
                                     │ DiffArgs (extended)
                                     ▼
                      ┌──────────────────────────────────────────┐
                      │ core::diff::run_diff(request)            │  new entry point
                      │  - validate strategy / mode combos       │  core::diff::mod
                      └──────────┬─────────────────┬─────────────┘
                                 │                 │
                  Mode::Class    │                 │   Mode::Object
                                 ▼                 ▼
              ┌──────────────────────────┐   ┌────────────────────────────┐
              │ core::diff::class        │   │ try_build_dominator(before)│
              │  diff_heaps_class()      │   │ try_build_dominator(after) │
              │  (lifted, behavior-      │   │  ↳ both required; otherwise│
              │   identical to today)    │   │  return DiffMode::Object → │
              │  → HeapDiff              │   │  feature_unavailable error │
              └──────────────────────────┘   └────────────┬───────────────┘
                                                          │ (graph_b, dom_b, graph_a, dom_a)
                                                          ▼
                                       ┌────────────────────────────────────┐
                                       │ core::diff::object::fingerprint    │
                                       │  per-strategy fingerprint builder  │
                                       │  → HashMap<ObjectFingerprint,      │
                                       │           SmallVec<ObjectId>>      │
                                       │  computed once for before, once    │
                                       │  for after                         │
                                       └────────────┬───────────────────────┘
                                                    │
                                                    ▼
                                       ┌────────────────────────────────────┐
                                       │ core::diff::object::engine         │
                                       │  pair-up by fingerprint:           │
                                       │   - present in B not A → added     │
                                       │   - present in A not B → removed   │
                                       │   - both, with retained delta ≥ θ  │
                                       │     → retained_changed             │
                                       │  → ObjectDelta                     │
                                       └────────────┬───────────────────────┘
                                                    │
                                                    ▼
                                       ┌────────────────────────────────────┐
                                       │ HeapDiff { ..class fields..,       │
                                       │            object_diff: Option<..> │
                                       │            match_quality, …}      │
                                       └────────────┬───────────────────────┘
                                                    │
                                                    ▼
                                       ┌────────────────────────────────────┐
                                       │ core::report::diff                 │
                                       │  text / json / toon renderers      │
                                       └────────────────────────────────────┘
```

Module placement rationale:

- New crate-internal namespace `core::diff` becomes the **single home** for all heap-diff code. Today's `compute_class_level_diff` lives inside `core::analysis::engine` for historical reasons; lifting it out is a no-behavior-change refactor that avoids splitting diff logic across two modules. (Slice 8-1.A.)
- `core::analysis::engine::diff_heaps` retains its current signature and delegates to `core::diff::run_diff` with `DiffMode::Class`. CLI today keeps working.
- Renderers live under `core::report::diff`, matching the placement of `core::report::flamegraph` from M7-3. The diff is a *report* projected from analysis output.

## 6. Data model

All new types live in `core::diff`. None of these types ship serialized inside `AnalyzeResponse`; they only appear inside `HeapDiff` (extended) and the new `ObjectDiffReport`.

```rust
// core/src/diff/mod.rs

pub enum DiffMode { Class, Object }

pub enum IdentityStrategy {
    ClassRetained,            // class_id + retained-bucket
    ClassDominator,           // + immediate-dominator class chain (depth 4)  -- DEFAULT
    FullFingerprint,          // + field-shape sig + outbound-class-set hash
}

pub struct DiffRequest {
    pub before: String,
    pub after: String,
    pub mode: DiffMode,
    pub identity_strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,         // power-of-two bucket: bytes >> bits
    pub retained_change_threshold: u64,   // bytes
    pub top_n: usize,
    pub retain_field_data: bool,
}
```

```rust
// core/src/diff/object/fingerprint.rs

pub struct ObjectFingerprint {
    pub class_id: u32,
    pub retained_bucket: u32,             // log2(retained_size) bucketed by retained_bucket_bits
    pub dominator_signature: u64,         // 0 when strategy < ClassDominator
    pub field_signature: u64,             // 0 when strategy < FullFingerprint
}

impl ObjectFingerprint {
    pub fn build(
        graph: &ObjectGraph,
        dom: &DominatorTree,
        obj_id: ObjectId,
        strategy: IdentityStrategy,
        bucket_bits: u8,
    ) -> Self { /* … */ }
}
```

The hash is **structural**, not nominal: `class_id` is resolved to the canonical class name string for cross-dump comparison (since two dumps will have different `class_id` integers for the same class). Implementation: store `(class_name_intern_id, …)` in the fingerprint, with a per-diff string interner so the fingerprint stays a flat 32-byte struct.

```rust
// core/src/diff/object/types.rs

pub struct ObjectDelta {
    pub class_name: String,
    pub fingerprint: ObjectFingerprint,
    pub example_object_id: ObjectId,      // representative HPROF id from the side that has it
    pub before_count: u64,
    pub after_count: u64,
    pub before_retained_bytes: u64,
    pub after_retained_bytes: u64,
    pub dominator_chain: Vec<String>,     // up to 4 class names from immediate dominator outward
    pub reference_chain: Vec<String>,     // truncated GC-root → object chain (reuses build_reference_chain)
    pub kind: ObjectDeltaKind,            // Added | Removed | RetainedChanged
}

pub enum ObjectDeltaKind { Added, Removed, RetainedChanged }

pub struct ObjectDiffReport {
    pub strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub retained_change_threshold: u64,
    pub match_quality: MatchQuality,      // see §6.1
    pub added: Vec<ObjectDelta>,          // capped to top_n by retained delta
    pub removed: Vec<ObjectDelta>,
    pub retained_changed: Vec<ObjectDelta>,
    pub totals: ObjectDiffTotals,
}

pub struct ObjectDiffTotals {
    pub before_object_count: u64,
    pub after_object_count: u64,
    pub fingerprint_collisions_before: u64,   // see §6.1
    pub fingerprint_collisions_after: u64,
    pub matched_pairs: u64,
}
```

`HeapDiff` (in `core::hprof::parser`) gains exactly one new optional field, behind `#[serde(default, skip_serializing_if = "Option::is_none")]`, so existing JSON consumers see no diff:

```rust
pub struct HeapDiff {
    pub before: String,
    pub after: String,
    pub delta_bytes: i64,
    pub delta_objects: i64,
    pub changed_classes: Vec<ClassDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_diff: Option<Vec<ClassLevelDelta>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_diff: Option<ObjectDiffReport>,        // NEW (Slice 8-1.C)
}
```

### 6.1 Match quality envelope

```rust
pub struct MatchQuality {
    pub strategy: IdentityStrategy,
    pub collision_rate: f64,              // collisions / total_objects
    pub estimated_false_match_risk: Risk, // Low | Medium | High
    pub estimated_false_split_risk: Risk,
    pub notes: Vec<String>,               // human-readable caveats
}
```

`collision_rate` is the fraction of fingerprints in either snapshot that map to more than one object. A high collision rate is a signal to escalate to `full-fingerprint`. Tested in Slice 8-1.G.

### 6.2 Memory budget

For each side of the diff:

- **Fingerprint table.** `HashMap<ObjectFingerprint, SmallVec<[ObjectId; 4]>>`. `ObjectFingerprint` is 28 bytes packed; SmallVec inline tail is 4 × 8 = 32 bytes; entry overhead ~16 bytes. Worst case 80 bytes per *distinct* fingerprint. With 2 GB dump, ~10–40 M live objects; even at 10 M distinct fingerprints we are at ~800 MB *per side* — too high.
- **Mitigation.** The fingerprint table is **bounded** by `retained_bucket_bits`: any object whose retained size is below a `min_retained` floor (default 4 KB) is skipped — it cannot drive an interesting dominant-instance diff anyway. This collapses ≥ 90 % of objects in typical dumps. After the floor, the practical fingerprint count is on the order of 10⁵–10⁶ entries → ≤ 80 MB per side → ≤ 160 MB combined, well within the 4× headroom.
- **`min_retained` is documented and tunable.** Hidden flag `--object-diff-min-retained <bytes>` (default `4096`) for users who need to find low-retained churn.
- **Strict ceiling.** If after floor-filtering the fingerprint table would exceed `MAX_OBJECT_DIFF_FINGERPRINTS = 8_000_000` entries on either side, the diff returns a structured `feature_unavailable_object_diff_too_large` error with the actual count, and the user is told to raise `--object-diff-min-retained` or run on a smaller dump. No silent truncation.

This budget is validated in Slice 8-1.G against the `medium`/`large` synthetic fixtures.

## 7. CLI surface

Today (preserved):

```
mnemosyne diff <BEFORE> <AFTER>
```

After M8-1:

```
mnemosyne diff <BEFORE> <AFTER>
    [--mode {class|object}]                       # default: class
    [--identity-strategy {class+retained|class+dominator|full-fingerprint}]
                                                  # default: class+dominator (object mode only)
    [--retained-bucket-bits <u8>]                 # default: 10
    [--retained-change-threshold <bytes>]         # default: 1048576
    [--top <n>]                                   # default: 50
    [--object-diff-min-retained <bytes>]          # default: 4096; hidden in --help short, shown in --help-long
    [--retain-field-data]                         # required for full-fingerprint
    [--format {text|json|toon}]                   # default: text
```

Exit codes (extension of today's `cli/src/main.rs` mapping):

| Code | Meaning |
|---|---|
| 0 | Diff produced |
| 2 | I/O error |
| 3 | Heap parse error on either side |
| 5 | Mode mismatch (e.g., `--mode object` on overview-only dumps) |
| 6 | `feature_unavailable_object_diff_too_large` — fingerprint budget exceeded |
| 7 | `feature_unavailable_without_field_data` — `full-fingerprint` requested without `--retain-field-data` |

Codes 5/6/7 are new and additive. They do not collide with any code emitted by today's `mnemosyne diff` (which only emits 0/2/3).

## 8. MCP surface

Add a new tool `diff_heaps` to `core::mcp::server`'s `list_tools` registry. Today this method does not exist; this is purely additive.

```jsonc
{
  "name": "diff_heaps",
  "description": "Compute class-level and (optionally) object-level diff between two HPROF heap dumps.",
  "input": [
    { "name": "before", "type": "string", "required": true,
      "description": "Path to the BEFORE heap dump." },
    { "name": "after",  "type": "string", "required": true,
      "description": "Path to the AFTER heap dump." },
    { "name": "mode", "type": "string", "required": false,
      "description": "'class' (default) or 'object'." },
    { "name": "identity_strategy", "type": "string", "required": false,
      "description": "'class+retained' | 'class+dominator' (default) | 'full-fingerprint'. Ignored when mode='class'." },
    { "name": "retained_bucket_bits", "type": "number", "required": false,
      "description": "Power-of-two bucket exponent for retained sizes; default 10 (1 KB)." },
    { "name": "retained_change_threshold", "type": "number", "required": false,
      "description": "Minimum |retained delta| in bytes for inclusion in retained_changed; default 1048576." },
    { "name": "top_n", "type": "number", "required": false,
      "description": "Per-section result cap; default 50." },
    { "name": "object_diff_min_retained", "type": "number", "required": false,
      "description": "Skip objects whose retained size is below this floor; default 4096." },
    { "name": "retain_field_data", "type": "boolean", "required": false,
      "description": "Required when identity_strategy='full-fingerprint'." }
  ],
  "output_schema": "HeapDiff (existing) extended with optional object_diff: ObjectDiffReport"
}
```

**Schema-stability commitment.** The new `object_diff` field is `Option<ObjectDiffReport>` and serialized with `skip_serializing_if = "Option::is_none"`. Existing MCP consumers that rely on the v0.3.0 `HeapDiff` shape see no change. The MCP surface adds one new tool name; it does not modify any existing tool.

Error envelopes follow the MCP `error_details` pattern already used by `analyze_heap` for mode mismatches:

```json
{ "error": "feature_unavailable_in_overview_mode",
  "tool": "diff_heaps",
  "detail": "Object-level diff requires deep mode on both dumps; <path> parsed in overview mode." }
```

## 9. Identity-strategy details

### 9.1 `class+retained`

```text
fingerprint = (class_name_intern_id, retained_size >> bucket_bits, 0, 0)
```

Fastest. Available the moment Slice 8-1.A lands. Default *until* Slice 8-1.B ships.

### 9.2 `class+dominator`  (default after Slice 8-1.B)

```text
fingerprint = (class_name_intern_id,
               retained_size >> bucket_bits,
               h(dominator_class_chain[0..4]),
               0)
```

`dominator_class_chain` is built by walking `dom.immediate_dominator(obj_id)` four times, taking the class name at each level (with `<gc-root>` for `VIRTUAL_ROOT_ID` and `<unknown>` for missing class metadata). The chain is hashed with `xxhash64` to a `u64`.

Why depth 4? Empirically (M3 dominator tree validation) most distinguishing context for retainer instances comes from the first three hops; depth 4 adds margin without inflating cost. Tunable via a private constant.

### 9.3 `full-fingerprint`

```text
fingerprint = (class_name_intern_id,
               retained_size >> bucket_bits,
               h(dominator_class_chain[0..4]),
               h(field_layout_signature ⊕ outbound_class_set_signature))
```

`field_layout_signature` requires `retain_field_data: true`. It is the `xxhash64` of:
- The instance class's resolved field list, sorted by `(field_name, field_type)`.
- For each ref-typed field, the *bit* indicating whether the field is null or non-null at dump time.

`outbound_class_set_signature` is the `xxhash64` of the sorted multiset of class names of the object's outbound references (capped to 64 distinct classes; collisions beyond that are bucketed into `<other>`).

If `retain_field_data == false` and `identity_strategy == FullFingerprint`, return error code 7 / `feature_unavailable_without_field_data` immediately. No silent downgrade.

## 10. Output formats

### 10.1 Text (today's format, extended)

Today's text output prints:

```
Heap diff:
  before -> after
  delta bytes: …
  delta objects: …
  changed classes:
    com.example.Cache  before=… after=…  delta=…
  class diff (graph-backed):
    com.example.Cache  before_instances=… after_instances=… …
```

After M8-1, when `--mode object` is set, append:

```
object diff (strategy=class+dominator, bucket=1KB, threshold=1MB):
  added (3):
    com.example.UserSession    +52428800 bytes  count=1  dom=[Server,Pool,Cache,...]
    com.fasterxml.jackson.…    +24117248 bytes  count=4  dom=…
    …
  removed (1):
    com.example.LegacyCache    -67108864 bytes  count=2  dom=…
  retained_changed (5):
    com.example.RequestMap    +12582912 bytes  count=1->1  dom=…
    …
  match quality: collision_rate=0.012  false_match_risk=Low  false_split_risk=Medium
    notes: "1.2% of fingerprints collided in BEFORE — consider --identity-strategy full-fingerprint"
```

### 10.2 JSON

Top-level shape is `HeapDiff` (existing) with the `object_diff` field optionally populated. Exact field order, naming, and serialization rules are governed by Serde derivations on the structs in §6.

### 10.3 TOON

TOON renderer mirrors the JSON shape but uses the existing TOON section structure (one `section` per top-level field). The renderer lives in `core::report::diff::toon`, sibling to other TOON renderers.

## 11. Defaults and feature flags

| Surface | Pre-M8-1 | After Slice 8-1.A | After Slice 8-1.B | After Slice 8-1.D (full ship) |
|---|---|---|---|---|
| `mnemosyne diff` (no flags) | text class diff | text class diff (unchanged) | text class diff (unchanged) | text class diff (unchanged) |
| `--mode object` default strategy | n/a | `class+retained` | `class+dominator` | `class+dominator` |
| `--mode object` available | no | yes (limited) | yes | yes |
| `--format json/toon` | unsupported | unsupported | unsupported | supported (Slice 8-1.F) |
| MCP `diff_heaps` | unregistered | unregistered | unregistered | registered (Slice 8-1.E) |

## 12. Performance budget

Targets, measured on the existing `medium` synthetic fixture (~512 MB heap, ~3 M objects) in deep mode on the standard Mnemosyne benchmark host:

| Metric | Class diff (today) | Object diff target | Tolerance |
|---|---|---|---|
| Wall-clock (cold) | T₀ | ≤ 2.0 × T₀ | +25 % allowed in CI before red |
| Peak RSS | R₀ | ≤ 1.6 × R₀ | +20 % allowed in CI before red |
| Output size (JSON) | n/a | ≤ 5 MB at `--top 50` | hard cap |

Class-diff path performance must not regress: same `T₀` and `R₀` measured against `main` after the Slice 8-1.A refactor. Slice 8-1.G is responsible for capturing baselines and running the comparison.

## 13. Sub-slice plan

All eight slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice 8-1.A — `ObjectFingerprint` + class-only baseline

- **Scope:** Introduce `core::diff` module tree. Lift `compute_class_level_diff` and helpers from `core::analysis::engine` to `core::diff::class` (no behavior change). Add `ObjectFingerprint`, `IdentityStrategy::ClassRetained`, fingerprint builder for the `ClassRetained` strategy only. Add `core::diff::run_diff` with `DiffMode::{Class, Object}` plumbing; `Object` mode wired only for `ClassRetained`. **No CLI flag, no MCP, no JSON/TOON yet.** Returns `ObjectDiffReport` from a new internal `pub(crate)` entry point exercised only by tests.
- **Files owned:**
  - `core/src/diff/mod.rs` (new)
  - `core/src/diff/class.rs` (new — content lifted from `engine.rs`)
  - `core/src/diff/object/mod.rs` (new)
  - `core/src/diff/object/fingerprint.rs` (new)
  - `core/src/diff/object/engine.rs` (new — pair-up + diff)
  - `core/src/diff/object/types.rs` (new)
  - `core/src/analysis/engine.rs` (delete lifted helpers; delegate to `core::diff::class`)
  - `core/src/lib.rs` (re-export `DiffMode`, `IdentityStrategy`, `ObjectDelta`, `ObjectDiffReport`, `ObjectFingerprint`)
  - `core/tests/diff_object_class_retained.rs` (new — synthetic fixture coverage)
- **Validation gates:**
  - All existing class-diff tests in `core/src/analysis/engine.rs` still pass byte-identically.
  - New tests: `class_retained_added_objects_appear_in_added`, `class_retained_removed_objects_appear_in_removed`, `class_retained_unchanged_objects_omitted`, `bucket_bits_zero_treats_each_byte_as_distinct`, `bucket_bits_too_large_collapses_all_into_one_bucket`, `min_retained_floor_filters_small_objects`.
  - `cargo bench` deltas (if any) within ±5 % of baseline on the existing diff bench.
- **Exit criteria:** Class-diff regression-free. `core::diff::run_diff` produces correct `ObjectDiffReport` for `ClassRetained` against synthetic fixtures. Public surface unchanged from outside `core::diff`.
- **Target size:** ~450 LOC + ~300 LOC tests.

### Slice 8-1.B — Dominator-path signature for fingerprints

- **Scope:** Add `IdentityStrategy::ClassDominator`. Build `dominator_class_chain` walker (depth 4, configurable via private const). Hash via `xxhash64` (already in `Cargo.lock` transitively; verify). Default flips to `ClassDominator` in this slice.
- **Files owned:**
  - `core/src/diff/object/fingerprint.rs` (extend)
  - `core/src/diff/object/dominator_chain.rs` (new — chain walker)
  - `core/Cargo.toml` (only if `xxhash-rust` is not already direct; otherwise no diff)
  - `core/tests/diff_object_class_dominator.rs` (new)
- **Validation gates:**
  - Test: two synthetic dumps where two `Cache` instances have identical retained sizes but different parents — `class+retained` collapses them, `class+dominator` separates them.
  - Test: collision-rate computed correctly for both strategies on the same fixture.
  - Test: `class+retained` results unchanged from Slice 8-1.A on the original fixtures (regression).
- **Exit criteria:** `class+dominator` produces strictly better separation than `class+retained` on the targeted fixture; default is now `ClassDominator`.
- **Target size:** ~250 LOC + ~250 LOC tests.

### Slice 8-1.C — Diff engine producing `ObjectDelta`

- **Scope:** Promote the `engine.rs` pair-up logic from Slice 8-1.A from prototype to production: support all three section kinds (`Added`, `Removed`, `RetainedChanged`); enforce per-section `top_n` cap; populate `dominator_chain` and `reference_chain`; compute `MatchQuality`; enforce `MAX_OBJECT_DIFF_FINGERPRINTS` budget. Add `IdentityStrategy::FullFingerprint` skeleton wired but **field/outbound signatures stubbed to `0`** (Slice 8-1.C does not depend on `retain_field_data`); `FullFingerprint` is unlocked end-to-end in Slice 8-1.D.
- **Files owned:**
  - `core/src/diff/object/engine.rs` (extend)
  - `core/src/diff/object/match_quality.rs` (new)
  - `core/src/hprof/parser.rs` (`HeapDiff` gains `object_diff: Option<ObjectDiffReport>`)
  - `core/tests/diff_object_engine.rs` (new)
- **Validation gates:**
  - All three sections populated correctly on synthetic fixture with known additions / removals / retained-size changes.
  - `MAX_OBJECT_DIFF_FINGERPRINTS` exceeded → returns `feature_unavailable_object_diff_too_large` error envelope, no panic, no truncation.
  - `MatchQuality` collision-rate matches an independently computed reference value.
  - JSON serialization of `HeapDiff` with `object_diff = None` is byte-identical to today.
- **Exit criteria:** Engine produces a complete `ObjectDiffReport`; `HeapDiff` schema is forward-compatible; budget guard fires correctly.
- **Target size:** ~400 LOC + ~400 LOC tests.

### Slice 8-1.D — CLI integration (`mnemosyne diff --mode object`)

- **Scope:** Extend `DiffArgs` in `cli/src/main.rs` with the flags from §7. Wire `handle_diff` to call `core::diff::run_diff`. Add the text output extension from §10.1. Implement `IdentityStrategy::FullFingerprint` end-to-end (field-shape + outbound-class-set signatures) gated on `--retain-field-data`. Wire exit codes 5/6/7. **No JSON/TOON flag yet** (Slice 8-1.F).
- **Files owned:**
  - `cli/src/main.rs` (extend `DiffArgs`, extend `handle_diff`, exit-code mapping)
  - `core/src/diff/object/fingerprint.rs` (full-fingerprint hashes)
  - `core/src/diff/object/field_signature.rs` (new)
  - `cli/tests/diff_object_cli.rs` (new — integration tests)
- **Validation gates:**
  - Six CLI integration tests: default still ships class-diff; `--mode object` ships object-diff; bad strategy combo returns code 7; overview-mode dump returns code 5; oversized dump returns code 6; `--top` is honored.
  - `--mode class` output is byte-identical to v0.3.0.
- **Exit criteria:** Object-level diff usable from the CLI in text mode with all three strategies.
- **Target size:** ~250 LOC + ~300 LOC tests.

### Slice 8-1.E — MCP integration

- **Scope:** Register `diff_heaps` tool in `core::mcp::server`'s `list_tools`. Implement the dispatch handler. Map error envelopes to MCP `error_details` shape. Add MCP method tests under `core/src/mcp/server.rs`'s test module.
- **Files owned:**
  - `core/src/mcp/server.rs` (registration + handler)
  - `core/src/mcp/server.rs` test module (new tests)
- **Validation gates:**
  - `list_tools` includes `diff_heaps` with the schema in §8.
  - Class-mode call returns the same `HeapDiff` shape as the CLI.
  - Object-mode call with budget-exceeding inputs returns `error_details` with `feature_unavailable_object_diff_too_large`.
- **Exit criteria:** MCP-driven diff works end-to-end against synthetic fixtures.
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 8-1.F — Reporting integration (text/JSON/TOON)

- **Scope:** Promote text rendering to a proper renderer. Add JSON and TOON renderers. Wire `--format` flag.
- **Files owned:**
  - `core/src/report/diff/mod.rs` (new)
  - `core/src/report/diff/text.rs` (new)
  - `core/src/report/diff/json.rs` (new)
  - `core/src/report/diff/toon.rs` (new)
  - `cli/src/main.rs` (`--format` wiring)
  - `core/tests/report_diff_renderers.rs` (new)
- **Validation gates:**
  - Snapshot tests for all three formats.
  - JSON round-trip via Serde.
  - TOON layout matches existing TOON section conventions.
- **Exit criteria:** All three formats produce reproducible, deterministic output.
- **Target size:** ~300 LOC + ~250 LOC tests.

### Slice 8-1.G — Validation against synthetic + real fixtures

- **Scope:** Stand up synthetic fixtures under `resources/test-fixtures/diff/` with known additions, removals, and retained-size changes. Add a regression bench under `core/benches/` that exercises class-diff before/after the Slice 8-1.A refactor. Validate the §12 performance budget on the `medium` fixture. Capture and commit the baselines.
- **Files owned:**
  - `resources/test-fixtures/diff/*` (new fixtures)
  - `core/benches/diff_object.rs` (new bench)
  - `core/tests/diff_object_real_fixtures.rs` (new — gated behind `cargo test --features fixtures-real`)
  - `docs/benchmarks/diff-object-baselines.md` (new — captures the §12 numbers)
- **Validation gates:**
  - Performance budget in §12 holds.
  - Class-diff path performance is within ±5 % of pre-Slice-8-1.A baseline.
  - Real-fixture coverage exercises at least one realistic dump pair.
- **Exit criteria:** Performance is documented and CI-tracked.
- **Target size:** Mostly tests + fixtures + bench harness.

### Slice 8-1.H — Documentation update (after PR #36 lands)

- **Scope:** Update `docs/roadmap.md` to mark M8-1 complete and link this design doc. Update `STATUS.md`, `CHANGELOG.md`, `README.md` (CLI table), and `docs/user-guide.md` (new "Object-level diff" section). Update `ARCHITECTURE.md` to mention the new `core::diff` module home.
- **Files owned:** Documentation files only.
- **Validation gates:** Documentation Sync Agent runs in impact-driven mode and confirms cross-doc alignment.
- **Exit criteria:** All public-facing docs reflect the shipped object-diff surface.
- **Blocking dependency:** Must wait for PR #36 (roadmap refresh) to land before this slice runs. Until then, any roadmap edits would conflict-merge.
- **Target size:** Documentation-only.

## 14. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | False-match cascade — sibling cache instances under the same dominator collapse into one fingerprint and `added` is silently understated | Default strategy is `class+dominator` (richer than `class+retained`); operators who hit collisions can escalate to `full-fingerprint`; `MatchQuality.collision_rate` surfaces collisions in every report. |
| R2 | Memory blowup on large dumps | `--object-diff-min-retained` floor (default 4 KB) collapses ≥ 90 % of objects; `MAX_OBJECT_DIFF_FINGERPRINTS` hard cap returns a structured error rather than OOM. |
| R3 | MCP schema breaking change | `object_diff` is `Option<…>` with `skip_serializing_if = "Option::is_none"`; existing consumers see no diff. New tool name, no rename of existing tools. |
| R4 | Overview-mode incompatibility silently produces wrong results | `try_build_dominator` failure on either side returns a `feature_unavailable_in_overview_mode` envelope; CLI exits 5; MCP returns `error_details`. No silent downgrade. |
| R5 | Refactor in Slice 8-1.A (lifting `compute_class_level_diff`) regresses class-diff | Class-diff tests are run unmodified after the lift; Slice 8-1.A is rejected if any class-diff test changes behavior. Diff bench captures performance regression too. |
| R6 | `xxhash` choice drifts across Rust versions | Pin `xxhash-rust` to a specific version with stable hash output; tests embed expected hash digests. |
| R7 | Field-shape signature explodes RSS when `--retain-field-data` is set on huge dumps | `full-fingerprint` is documented as opt-in; `--retain-field-data` is the user's explicit consent to higher memory use. |
| R8 | Two dumps from different JVM builds report the same logical class under different names (e.g., `synthetic` lambda class names) | Fingerprint normalizes only by class name string; cross-JVM comparisons remain the user's responsibility. Documented in §15.3. |

## 15. Test strategy

### 15.1 Synthetic fixtures

Three fixture pairs under `resources/test-fixtures/diff/`:

1. **`pure-add/`** — `before.hprof` and `after.hprof` differ by exactly N new instances of `com.example.UserSession`, all retained > 1 MB.
2. **`pure-remove/`** — converse.
3. **`retained-grow/`** — same instances on both sides; `after` has each retain materially more (a known set of objects gain children).

Plus one **collision** fixture where two siblings under the same dominator have identical retained-size buckets — exercises the `class+retained` false-match and validates `class+dominator` separation.

### 15.2 Regression boundary against existing class diff

All existing tests in `core/src/analysis/engine.rs::tests` (specifically `heap_diff_prefers_class_stats`, `heap_diff_falls_back_to_record_stats`) and CLI tests of `mnemosyne diff` must pass unchanged after Slice 8-1.A. This is enforced as the exit criterion of Slice 8-1.A.

### 15.3 Negative tests

- `full-fingerprint` requested without `--retain-field-data` → exit 7.
- Overview-mode dump on either side → exit 5.
- Fingerprint count over `MAX_OBJECT_DIFF_FINGERPRINTS` → exit 6.
- `--retained-bucket-bits 0` produces a finer (slower) diff but still terminates.
- Two identical dumps → object-diff reports zero in all three sections; `MatchQuality.collision_rate` matches reference.

### 15.4 Determinism

Output ordering: `added` and `removed` sorted by `(retained_bytes desc, class_name asc, fingerprint asc)`; `retained_changed` sorted by `(|delta| desc, class_name asc, fingerprint asc)`. Two runs against the same input pair must produce byte-identical text/JSON/TOON output. Locked by `diff_object_text_is_deterministic` and renderer snapshot tests.

## 16. Out-of-scope (explicit non-goals)

- **M8-5 — Multi-snapshot trends.** Three-or-more-snapshot timelines, leak progression curves, retention-velocity calculations.
- **M8-8 — Persistent indexes.** On-disk fingerprint caches, parse-once-query-many.
- **M8-9 — Tauri/UI surfacing.** No UI work; JSON envelope is designed to be consumable, but no UI wiring lands here.
- **M8-10 — Streaming diff.** Object-diff requires deep mode on both dumps.
- **MAT-style interactive equivalence-rule editing.** Operators tune behavior via `--identity-strategy` and `--retained-bucket-bits` only.
- **Cross-JVM-build object identity.** Lambdas, anonymous classes, and synthetic class names are reported as observed; we do not attempt to canonicalize them across JVM builds.
- **Roadmap edits in this design pass.** PR #36 owns roadmap refresh; Slice 8-1.H performs the linkback **after** PR #36 lands.

## 17. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M8-1 backlog row.
- Sibling design (style template): [milestone-7-3-allocation-site-flame-graphs.md](milestone-7-3-allocation-site-flame-graphs.md) (slice breakdown shape, renderer placement under `core::report`).
- Sibling design (mode-honesty pattern): [milestone-7-4-oql-targeted-expansion.md](milestone-7-4-oql-targeted-expansion.md) (`feature_unavailable_in_overview_mode` envelope precedent).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 8-1.H.
- Existing diff implementation:
  - [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `diff_heaps`, `compute_class_level_diff`, `collect_class_level_stats` (to be lifted in Slice 8-1.A).
  - [core/src/hprof/parser.rs](../../core/src/hprof/parser.rs) — `HeapDiff`, `ClassDelta`, `ClassLevelDelta`.
  - [cli/src/main.rs](../../cli/src/main.rs) — `Commands::Diff`, `DiffArgs`, `handle_diff`.
- MCP surface: [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `list_tools` registry.
- Reporting analogue: [core/src/report/](../../core/src/report/) — module layout to mirror.
- Roadmap-archived M3 dominator design: [design/m3-p1-b2-core-analysis-features.md](m3-p1-b2-core-analysis-features.md).

## 18. Implementation readiness verdict

**READY AFTER DOC UPDATE** — this design doc is implementation-depth and was authored in this pass. The Implementation Agent may proceed with **Slice 8-1.A (`ObjectFingerprint` + class-only baseline)** as the first task. Slices 8-1.B → 8-1.H are gated behind their predecessors and must each end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean before handing off to the next slice. Slice 8-1.H is additionally gated on PR #36 landing.

## 19. Slice 8-1.H closeout (documentation sync)

PR #36 landed 2026-04-26. Slice H ran 2026-04-27 and updated: `docs/roadmap.md` (M10 row marked shipped, MAT parity matrix, scorecard, design-doc index — see §"Roadmap correspondence" above), `STATUS.md`, `CHANGELOG.md`, `README.md` CLI usage, `ARCHITECTURE.md` (`core::diff` module in the project-structure tree), `docs/user-guide.md` (`diff --mode object` section). Full-workspace verification at closeout: `cargo check` clean, `cargo test --workspace` 466 passed / 0 failed, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean.

Remaining M10 scope not covered by slices A–H: `ci-check object_growth_threshold` predicate, leak-progression cross-reference with `detect_leaks()`. Tracked as **M10-B** in `docs/roadmap.md`.
