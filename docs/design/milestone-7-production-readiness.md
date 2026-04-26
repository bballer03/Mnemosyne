# Milestone 7 — Production Readiness & Scale

> **Status:** ⚠️ In progress — M7-1 ✅ shipped (commits `7e27e0e`, `8ceb43b`, `c128131`, `995f92b`, `8534f37`, `09ee9c5`, `c64b4a1`). M7-2 ✅ shipped (commits `2cbd4cc`, `2c9ff65`, `bdc6b5f`, `c6812a5`, `3c81006`, `5b147a0`, `846be08`). M7-3 ✅ shipped (commits `4539f1a`, `b2f32c0`, `cfe36f0`, `e6e7458`, `8b15fad`, `adaf46c`). M7-4 ✅ shipped (commits `2311d5c`, `4afd062`, `7be4799`, `ea8421b`, `fd4c787`, `2f029bb`, `bfa4e19`). M7-5 entering design.
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Predecessor analysis:** [docs/roadmap-archive.md §4.5](../roadmap-archive.md)
> **Related design:** [docs/design/memory-scaling.md](memory-scaling.md)
> **Slice addenda:** M7-2 → [docs/design/milestone-7-2-ci-regression-policies.md](milestone-7-2-ci-regression-policies.md); M7-3 → [docs/design/milestone-7-3-allocation-site-flame-graphs.md](milestone-7-3-allocation-site-flame-graphs.md); M7-4 → [docs/design/milestone-7-4-oql-targeted-expansion.md](milestone-7-4-oql-targeted-expansion.md); M7-5 → [docs/design/milestone-7-5-comparative-benchmarks.md](milestone-7-5-comparative-benchmarks.md)
> **Last updated:** 2026-04-26

---

## 1. Objective

M7 closes the gap from "credible alpha" (v0.2.0) to "credible MAT alternative at production scale" (v0.3.0). The milestone is deliberately narrow:

1. Make Mnemosyne usable on **10 GB+ heap dumps** without exhausting host memory.
2. Widen the moat in **CI automation** and **allocation-site visualization** — categories MAT and hprof-slurp do not own.
3. Ship a small, targeted **OQL parity** expansion — not the full MAT treadmill.
4. Publish reproducible **comparative benchmarks** so performance claims are defensible.
5. Cut **v0.3.0** with an honest, evidence-backed story.

This document is the architectural reference for all M7 work. M7-1 is specified at implementation depth; M7-2 through M7-6 are framed and gated, with their detailed design left to the slice that picks them up.

## 2. Context

v0.2.0 ships a deep, in-memory analysis pipeline that has been validated through the ~2 GB tier (see [memory-scaling.md](memory-scaling.md)). The deep path builds an `ObjectGraph` with all objects, classes, references, and GC roots in `HashMap`/`Vec`. For a 10 GB+ dump (~hundreds of millions of objects) this in-memory model is not viable on a developer workstation.

hprof-slurp demonstrates that a Rust streaming HPROF parser can sustain ~2 GB/s with ~500 MB RSS even on 34 GB dumps — but only by trading away analysis depth (no dominator tree, no retained sizes, no leak suspects). Mnemosyne already has a streaming record-scanner (`core::hprof::parser::parse_heap`) that runs at 2.25 GiB/s but currently only attributes bytes by **record tag** (`INSTANCE_DUMP`, `OBJECT_ARRAY_DUMP`, `PRIMITIVE_ARRAY_DUMP`), not by **class name**. That is the seed M7-1 will grow into a real triage mode.

The strategy is **two-mode parity**:

- **Deep mode (default, current behavior):** unchanged. Builds `ObjectGraph`, computes dominator tree, retained sizes, leaks, and the full analyzer surface.
- **Overview mode (new):** streaming, bounded-memory, class-resolved triage. Real class names and approximate sizes; **no graph, no retained sizes, no leak suspects**. Honest provenance markers (`Partial`) make the limitation machine-readable.

## 3. Scope

| # | Item | Type | Status | Slice doc |
|---|------|------|--------|-----------|
| M7-1 | Streaming overview mode | Parity + Differentiation | ✅ shipped (`7e27e0e`, `8ceb43b`, `c128131`, `995f92b`, `8534f37`, `09ee9c5`, `c64b4a1`) | this doc, §6 |
| M7-2 | CI regression policies (`mnemosyne ci-check`) | Differentiation | ✅ shipped (`2cbd4cc`, `2c9ff65`, `bdc6b5f`, `c6812a5`, `3c81006`, `5b147a0`, `846be08`) | [milestone-7-2-ci-regression-policies.md](milestone-7-2-ci-regression-policies.md) |
| M7-3 | Allocation-site flame graphs (`mnemosyne flamegraph`) | Differentiation | ✅ shipped (`4539f1a`, `b2f32c0`, `cfe36f0`, `e6e7458`, `8b15fad`, `adaf46c`) | [milestone-7-3-allocation-site-flame-graphs.md](milestone-7-3-allocation-site-flame-graphs.md) |
| M7-4 | OQL targeted expansion (5–6 high-value predicates) | Parity | ✅ shipped (`2311d5c`, `4afd062`, `7be4799`, `ea8421b`, `fd4c787`, `2f029bb`, `bfa4e19`) | [milestone-7-4-oql-targeted-expansion.md](milestone-7-4-oql-targeted-expansion.md) |
| M7-5 | Comparative benchmarks vs MAT and hprof-slurp | Credibility | 🟡 design in progress | [milestone-7-5-comparative-benchmarks.md](milestone-7-5-comparative-benchmarks.md) |
| M7-6 | v0.3.0 release | Release | 🔲 pending | release-prep prompt |

## 4. Non-scope

- **No reopening of M1–M6.** Archive content is historical.
- **No persistent index / disk cache.** That is M8-8.
- **No object-level diff.** That is M8-1.
- **No full OQL expansion.** Targeted predicates only; full breadth is M8-2.
- **No streaming MCP responses.** That is M8-10.
- **No Tauri release work.** That is M8-9.
- **No breaking changes to CLI, MCP, or core API.** Deep mode behavior must be byte-identical to v0.2.0 on all existing fixtures.
- **Overview mode is not a replacement for deep mode.** It is a triage tool with explicit, documented limitations.

## 5. Cross-cutting architecture overview

### 5.1 Mode selection

A new `AnalysisMode` enum is introduced once and reused across CLI, MCP, and core:

```rust
// core/src/analysis/mode.rs (new) — re-exported from core::analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisMode {
    /// File-size driven. Default. Picks Deep for small dumps and Overview for large ones.
    #[default]
    Auto,
    /// Build the full ObjectGraph and run all analyzers (current v0.2.0 behavior).
    Deep,
    /// Streaming, bounded-memory triage. No graph, no retained sizes.
    Overview,
}
```

`Auto` resolves at the boundary (CLI / MCP) using a single threshold:

| Input size | Resolves to |
|---|---|
| `< OVERVIEW_AUTO_THRESHOLD_BYTES` (default `4 GiB`) | `Deep` |
| `>= OVERVIEW_AUTO_THRESHOLD_BYTES` | `Overview` |

The threshold is a `pub const` in `core::analysis::mode` and may be overridden via `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` for benchmarking. `Auto` is a *resolution-time* convenience; once resolved, downstream code only sees `Deep` or `Overview`.

### 5.2 Single response shape

`AnalyzeResponse` is **not** forked. Instead it gains:

```rust
pub struct AnalyzeResponse {
    pub mode: AnalysisMode, // NEW (defaults to Deep for back-compat deserialization)
    pub overview: Option<OverviewSummary>, // NEW — populated only when mode == Overview
    // ...all existing fields unchanged...
    // graph, leaks, histogram, dominator-derived data are None / empty in overview mode.
    pub provenance: Vec<ProvenanceMarker>, // existing — gains Partial markers in overview mode
}
```

Backwards compatibility:

- `mode` deserializes to `Deep` when missing (existing v0.2.0 JSON consumers stay correct).
- `overview` is `Option`, so v0.2.0 readers ignore it.
- All deep-mode fields keep their existing serialization. Overview-mode runs leave them empty/None and stamp `Partial` provenance so consumers cannot mistake "absent" for "zero".

### 5.3 Provenance contract

Every overview-mode response carries at least:

```text
ProvenanceMarker {
    kind: ProvenanceKind::Partial,
    detail: Some("Overview mode: streaming triage; no dominator tree, retained sizes, or leak suspects."),
}
```

Per-leak / per-suspect provenance is unaffected because overview mode does not emit leaks. Reports must render an explicit "Deep analysis not run — overview mode" banner whenever this marker is present (see §6.7).

## 6. M7-1 — Streaming Overview Mode (deep design)

### 6.1 Goal

Produce a class-resolved, bounded-memory summary of any HPROF dump in a single streaming pass:

- **Throughput target:** ≥ 1.5 GiB/s sustained (today's record-scan is 2.25 GiB/s; class resolution adds bookkeeping).
- **Memory target:** < 1 GiB RSS on a 10 GB dump regardless of object count.
- **Latency target:** 10 GB dump processed in < 60 s on the M7-5 reference workstation.
- **Honesty target:** every overview output is unambiguously labeled `Partial`; no field that requires the graph is fabricated.

### 6.2 Architecture

```
File → BufReader (existing)
      ↓
   Streaming HPROF record loop
      ↓
   Tag dispatch (subset of binary_parser, no ObjectGraph build)
      ├── TAG_STRING_IN_UTF8     → bounded string-id → name table (interned)
      ├── TAG_LOAD_CLASS         → class-id → string-id mapping
      ├── TAG_HEAP_DUMP[_SEGMENT] sub-records:
      │     ├── INSTANCE_DUMP    → resolve class-id → ClassAccumulator.add(size)
      │     ├── OBJECT_ARRAY_DUMP→ resolve element class-id → ClassAccumulator.add(size)
      │     ├── PRIMITIVE_ARRAY_DUMP → primitive bucket (per element type)
      │     └── *_GC_ROOT_*      → bounded GcRootCounter (count only, no objects)
      ├── TAG_STACK_FRAME / TAG_STACK_TRACE → bounded thread-frame accumulator
      └── all other tags         → skip_bytes (existing helper)
      ↓
   Bounded top-N extraction
      ↓
   OverviewSummary
```

Critical properties:

1. **No `ObjectGraph` is constructed.** Per-object data is consumed and discarded. Only aggregate counters survive.
2. **String / class tables are bounded by *number of declared classes*, not number of objects.** A 10 GB dump with 500 M objects typically has < 100 K loaded classes; this fits in tens of MB.
3. **Top-N accumulators use a min-heap.** Memory cost is O(N), not O(classes), so a `top_n = 1000` setting costs ~tens of KB.
4. **Reuse, don't fork.** The overview parser lives next to the existing scanner and shares helpers (`skip_bytes`, `tag_name`, the tag constants in `core::hprof::tags`). It does **not** import or call `binary_parser::parse_hprof_reader`.

### 6.3 New module layout

```
core/src/hprof/
    overview.rs        ← NEW: streaming overview parser, OverviewSummary, accumulators
    parser.rs          ← unchanged record-scanner (kept; used by `parse_heap`)
    binary_parser.rs   ← unchanged deep parser (kept; used by `parse_hprof_file_with_options`)
    mod.rs             ← re-export OverviewSummary, parse_hprof_overview, parse_hprof_overview_file
```

Rationale for `core/src/hprof/overview.rs` (parser-adjacent) over `core/src/analysis/overview.rs`:

- Overview mode is fundamentally a **parser variant**, not an analyzer.
- Placing it next to `binary_parser.rs` and `parser.rs` keeps tag-dispatch code colocated.
- `core::analysis` continues to operate on data structures, not raw HPROF bytes.

### 6.4 Core types

```rust
// core/src/hprof/overview.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewSummary {
    pub heap_path: String,
    pub header: HprofHeader,
    pub total_size_bytes: u64,
    pub elapsed_ms: u64,

    // Aggregates
    pub total_instances: u64,
    pub total_object_arrays: u64,
    pub total_primitive_array_bytes: u64,
    pub gc_root_counts: GcRootCounts,
    pub loaded_class_count: u64,

    // Bounded top-N
    pub top_classes_by_bytes: Vec<OverviewClassStat>,
    pub top_classes_by_instances: Vec<OverviewClassStat>,
    pub thread_frames: Vec<OverviewStackFrame>,

    // Honest limits
    pub truncated: bool,
    pub options: OverviewOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewClassStat {
    pub class_name: String,
    pub instances: u64,
    pub approx_shallow_bytes: u64, // sum of INSTANCE_DUMP / array record payloads
    pub percentage_of_total: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GcRootCounts {
    pub jni_global: u64,
    pub jni_local: u64,
    pub java_frame: u64,
    pub native_stack: u64,
    pub sticky_class: u64,
    pub thread_block: u64,
    pub monitor_used: u64,
    pub thread_object: u64,
    pub other: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewStackFrame {
    pub thread_serial: u32,
    pub class_name: String,
    pub method_name: String,
    pub source_file: String,
    pub line_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewOptions {
    pub top_n: usize,                  // default 100
    pub max_thread_frames: usize,      // default 256
    pub max_loaded_classes_tracked: usize, // hard ceiling, default 200_000
}

impl Default for OverviewOptions {
    fn default() -> Self {
        Self { top_n: 100, max_thread_frames: 256, max_loaded_classes_tracked: 200_000 }
    }
}

pub fn parse_hprof_overview_file(
    path: &str,
    options: OverviewOptions,
) -> CoreResult<OverviewSummary> { /* ... */ }

pub fn parse_hprof_overview<R: Read>(
    reader: &mut R,
    options: OverviewOptions,
    heap_path: &str,
    total_size_bytes: u64,
) -> CoreResult<OverviewSummary> { /* ... */ }
```

`approx_shallow_bytes` is documented as **HPROF record payload bytes attributed to that class**, not JVM-faithful shallow size. This is the same accuracy contract hprof-slurp publishes; it is honest and benchmark-comparable.

### 6.5 Bounded accumulator design

```rust
// core/src/hprof/overview.rs (internal)
struct ClassAccumulator {
    // class-id (u64) → (instances, bytes)
    by_id: HashMap<u64, (u64, u64)>,
}
```

Per-class state is `(u64, u64)` — 16 bytes plus hash-table overhead. With ≤ 200 K classes the upper bound is ~10 MB. If a pathological dump exceeds `max_loaded_classes_tracked`, set `truncated = true` and stop adding new classes; existing classes continue accumulating. **Never silently drop data without flipping `truncated`.**

Top-N extraction at the end uses a single `BinaryHeap<Reverse<...>>` pass, O(C log N) where C = class count, N = top_n.

String/class name resolution:

- `TAG_STRING_IN_UTF8` records are stored in a `HashMap<u64 /* string_id */, Arc<str>>`.
- `TAG_LOAD_CLASS` maps `class_id → string_id`.
- At top-N extraction time, unresolved class-ids fall back to the synthetic name `<unresolved class id 0x…>` and contribute a `Partial` provenance detail.

GC root counts are bumped on every `*_GC_ROOT_*` sub-record without storing the object ids. Thread frames are bounded by `max_thread_frames` (FIFO drop with `truncated = true`).

### 6.6 CLI surface

Add `--mode` to `parse` and `analyze`. **Flag is additive; existing invocations behave identically.**

```text
mnemosyne parse <heap> [--mode auto|deep|overview] [--top-n N]
mnemosyne analyze <heap> [--mode auto|deep|overview] [--top-n N] ... (existing flags)
```

```rust
// cli/src/main.rs (additive)
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum ModeArg {
    #[default]
    Auto,
    Deep,
    Overview,
}

#[derive(Debug, Parser)]
struct ParseArgs {
    heap: PathBuf,
    #[arg(long, value_enum, default_value_t = ModeArg::Auto)]
    mode: ModeArg,
    #[arg(long, default_value_t = 100)]
    top_n: usize,
}
```

Resolution rules:

| User passes | Effective mode |
|---|---|
| (nothing) | `Auto` → resolved by file size at the boundary |
| `--mode deep` | `Deep` always |
| `--mode overview` | `Overview` always (even on a 100 MB dump — useful for benchmarks) |
| `--mode auto` | same as default |

Existing `parse` continues to return today's `HeapSummary` shape when mode resolves to `Deep`. When mode resolves to `Overview`, `parse` emits a new `OverviewSummary` rendering. CLI text/markdown/JSON renderers branch on the mode.

`analyze` in overview mode:

- Skips `analyze_heap` entirely.
- Wraps the `OverviewSummary` into `AnalyzeResponse { mode: Overview, overview: Some(_), summary: <synthesized HeapSummary stub from overview totals>, leaks: vec![], graph: GraphMetrics::default(), provenance: vec![partial_marker], .. }`.
- The synthesized `HeapSummary` is necessary because downstream renderers expect it; it carries its own `ProvenanceKind::Synthetic` marker indicating it was derived from streaming aggregates.

### 6.7 Reporting

All five formats (`text`, `markdown`, `html`, `toon`, `json`) get an overview-mode branch. Hard requirements:

1. **Banner first.** Every renderer leads with a "⚠ Overview mode — streaming triage; deep analysis not run" notice (text equivalent for non-graphical formats).
2. **Show what we have:** total bytes, class count, top-N tables, GC root counts, thread frames.
3. **Explicitly mark what we don't have:** "Retained sizes — not available in overview mode", "Leak suspects — not available in overview mode", "Dominator tree — not available in overview mode".
4. **JSON / TOON keep all deep-mode keys present but null/empty.** This preserves schema stability for CI consumers.

### 6.8 MCP integration

`parse_heap` and `analyze_heap` MCP methods accept an optional `mode` parameter:

```jsonc
// parse_heap params (additive)
{
  "heap_path": "/path/to/dump.hprof",
  "mode": "auto" | "deep" | "overview",   // optional, defaults to "auto"
  "top_n": 100                            // optional, overview-mode only
}
```

- Missing `mode` → `Auto` (current behavior preserved on small dumps).
- Schema for `parse_heap` / `analyze_heap` in `list_tools` gains the `mode` enum and a description note that overview mode returns partial data.
- The response carries `mode` and (for overview) `overview` directly inside the existing JSON envelope. MCP consumers that ignore unknown fields keep working.

### 6.9 Differentiator continuity

Where overview-mode data permits, Mnemosyne's differentiators continue to function:

| Differentiator | In overview mode |
|---|---|
| Provenance markers | ✅ Always present, `Partial` at minimum |
| Structured output (JSON/TOON) | ✅ Same envelope, partial fields |
| MCP tool surface | ✅ Same methods, additive `mode` param |
| AI insights | ⚠️ Disabled by default in overview mode (not enough graph data); CLI prints a notice if `--ai` is passed with `--mode overview`. Future M8 work may add an overview-aware AI prompt. |
| CI policies (M7-2) | ✅ Will accept overview-mode inputs for top-N and total-bytes thresholds; retained-size policies error-out with a clear message. |
| Flame graphs (M7-3) | ⚠️ Not supported in overview mode (require dominator data). M7-3 must error cleanly when fed overview data. |

### 6.10 Test plan (TDD-cycle compatible)

All tests are written **before** implementation per `.github/skills/tdd-cycle`.

#### Unit tests (`core/src/hprof/overview.rs` `#[cfg(test)]`)

| Test | Asserts |
|---|---|
| `bounded_topn_keeps_largest` | Min-heap of size N preserves top-N from a > N-element sequence. |
| `class_accumulator_aggregates_instances_and_bytes` | Repeated `add(class_id, size)` produces correct totals. |
| `truncation_flag_set_when_class_ceiling_hit` | Adding `max_loaded_classes_tracked + 1` distinct classes flips `truncated = true`. |
| `unresolved_class_id_falls_back_to_synthetic_name` | Class with no matching `LOAD_CLASS` produces `<unresolved class id 0x…>` and contributes provenance detail. |
| `gc_root_counts_increment_per_subrecord` | One of each `*_GC_ROOT_*` tag yields counts of 1. |
| `thread_frame_buffer_drops_oldest_at_capacity` | Beyond `max_thread_frames`, FIFO drop, `truncated = true`. |

#### Integration tests (`core/tests/`)

Use existing fixtures in `core/src/hprof/test_fixtures.rs` and `resources/test-fixtures/`.

| Test | Asserts |
|---|---|
| `overview_mode_on_small_fixture_matches_deep_mode_class_set` | Top-10 class names from overview mode are a subset of deep-mode histogram class names. (Sizes are *approximate* shallow, so equality is not asserted; subset + ordering correlation is.) |
| `overview_mode_emits_partial_provenance` | `OverviewSummary`-bearing `AnalyzeResponse` carries `ProvenanceKind::Partial` with the canonical detail string. |
| `overview_mode_skips_object_graph` | Memory probe (or, where probe unavailable, a sentinel `unsafe`-free check) confirms no `ObjectGraph` is constructed in the overview path. |
| `auto_mode_below_threshold_picks_deep` | A small fixture under threshold runs deep mode end-to-end. |
| `auto_mode_above_threshold_picks_overview` | Threshold override via env var forces overview path on a small fixture. |

#### Regression tests

| Test | Asserts |
|---|---|
| Existing `cargo test --workspace` baseline | All v0.2.0 tests pass unchanged. Deep-mode codepath is not modified. |
| `analyze_response_json_back_compat` | Deserializing a v0.2.0 captured `AnalyzeResponse` JSON (no `mode`, no `overview`) succeeds and yields `mode == Deep`, `overview == None`. |
| `mcp_parse_heap_no_mode_param_back_compat` | MCP `parse_heap` request without `mode` returns the same shape as v0.2.0 on existing fixtures. |

#### Performance gate (deferred to M7-5)

A `cargo bench` entry `overview_bench` measures throughput on a synthetic dump from `scripts/generate_synthetic_heap.sh`. The 10 GB / 60 s / 1 GB RSS target is **validated in M7-5**, not in M7-1, but M7-1 lands the bench harness so the gate is mechanically runnable.

### 6.11 Slice breakdown for implementation

M7-1 is broken into 5 small TDD-friendly slices. Each slice ends with `cargo {check,test,clippy,fmt}` clean and the next slice depends on the previous.

#### Slice M7-1.A — `AnalysisMode` enum + boundary plumbing ✅ shipped (`7e27e0e`)

- **Files affected:**
  - `core/src/analysis/mode.rs` (new)
  - `core/src/analysis/mod.rs` (re-export)
  - `core/src/analysis/engine.rs` (`AnalyzeResponse` gains `mode`, `overview` fields, both default-deserialize to back-compat values)
  - `core/src/lib.rs` (re-export `AnalysisMode`)
- **Test gate:** new unit test `analyze_response_json_back_compat` (regression) + `mode_default_is_auto` + `mode_resolves_by_size`.
- **Target size:** ~150 LOC + tests. No behavior change — defaults make every existing path stay deep.

#### Slice M7-1.B — Bounded accumulators (no parser yet) ✅ shipped (`8ceb43b`)

- **Files affected:**
  - `core/src/hprof/overview.rs` (new — types + accumulator structs + top-N extraction; **no I/O, no parsing**)
  - `core/src/hprof/mod.rs` (re-export `OverviewSummary`, `OverviewOptions`)
- **Test gate:** all unit tests from §6.10 (bounded top-N, class accumulator, truncation, fallback name, gc-root counts, thread-frame buffer).
- **Target size:** ~250 LOC + ~150 LOC tests. Pure data structures, easy to drive from synthetic input.

#### Slice M7-1.C — Streaming overview parser ✅ shipped (`c128131`)

- **Files affected:**
  - `core/src/hprof/overview.rs` (add `parse_hprof_overview` + `parse_hprof_overview_file`)
  - reuses `core/src/hprof/tags.rs` (no change)
  - reuses `skip_bytes` helper (lift to `pub(super)` if needed; no other parser change)
- **Test gate:** integration tests `overview_mode_on_small_fixture_matches_deep_mode_class_set` + `overview_mode_skips_object_graph`. Existing parser tests untouched.
- **Target size:** ~400 LOC + ~150 LOC tests. The bulk of M7-1 effort lives here.

#### Slice M7-1.D — CLI `--mode` wiring ✅ shipped (`995f92b`, `8534f37`)

- **Files affected:**
  - `cli/src/main.rs` (add `ModeArg`, plumb into `ParseArgs` and `AnalyzeArgs`, branch in handlers)
  - `cli/tests/integration.rs` (new tests for `--mode overview` on a small fixture)
  - `core/src/report/*.rs` (overview-mode renderers for all 5 formats; banner + honest "not available" markers)
- **Test gate:** CLI integration tests for `parse --mode overview`, `analyze --mode overview`, and `--mode auto` with threshold env var. Snapshot tests for each renderer's overview-mode output.
- **Target size:** ~300 LOC + ~250 LOC tests. Touches reports — coordinate with Documentation Sync at end.

#### Slice M7-1.E — MCP `mode` parameter + `list_tools` schema ✅ shipped (`09ee9c5`, `c64b4a1` doc sync)

- **Files affected:**
  - `core/src/mcp/server.rs` (parse `mode` from params for `parse_heap` and `analyze_heap`; update `list_tools` schemas; route to overview path when resolved)
  - MCP test cases in same file
- **Test gate:** MCP tests `parse_heap_mode_overview_returns_overview_summary`, `analyze_heap_mode_overview_carries_partial_provenance`, `mcp_parse_heap_no_mode_param_back_compat`.
- **Target size:** ~200 LOC + ~150 LOC tests.

After Slice E, M7-1 is complete and ready for benchmark validation in M7-5.

### 6.12 Risks and open questions

| Risk | Mitigation |
|---|---|
| Approximate shallow size diverges materially from MAT shallow size | Document the definition explicitly; M7-5 publishes both numbers side-by-side with the methodology. |
| `Auto` threshold of 4 GiB is wrong in practice | Threshold is one `pub const` + env override; revisit after M7-5 evidence. |
| Class-id table grows unboundedly on adversarial dumps | Hard ceiling `max_loaded_classes_tracked` + `truncated` flag. |
| Overview mode users assume retained sizes are zero rather than absent | Renderers must show "not available" text; JSON keeps `null`/empty with provenance detail. |
| AI insights expectation in overview mode | CLI prints a one-line notice and skips AI; documented in user-guide. |
| Parser drift between deep and overview when new HPROF tags appear | Both parsers share `core::hprof::tags` constants; new tags must be added there first, then handled (or skipped) in both parsers. Add a CI assertion that the tag enum count matches handled-tag count in each parser. |

### 6.13 Open questions deferred to follow-up batches

- Should overview mode emit a class-name → class-id manifest for cross-correlation with deep-mode runs on the same dump? (Defer; only useful once we have heap-diff in M8-1.)
- Should `mnemosyne diff` accept overview inputs? (Defer to M7-2 / M8-1 — overview×overview diff is cheap, overview×deep diff is misleading.)

## 7. M7-2 — CI Regression Policies (framing only)

**Goal:** `mnemosyne ci-check <heap.hprof> --policy policy.toml` exits non-zero on threshold violations; emits structured JSON suitable for GitHub Actions / Jenkins consumers.

**Scope sketch:**
- New CLI subcommand `ci-check`.
- New `core::policy` module: `Policy` schema (TOML), evaluator, `PolicyResult`.
- Initial predicates: total bytes, total instances, top-N class instances/bytes, leak count by severity, retained-size thresholds (deep mode only), provenance gates ("fail if any leak is `Synthetic`").
- Overview-mode-compatible predicates explicitly enumerated; deep-only predicates fail with a clear "requires deep mode" error rather than silently passing.
- GitHub Action and Jenkinsfile snippets in `docs/integrations/`.

**Dependencies:** M7-1 (mode awareness in policy evaluator).
**Detailed design:** addendum doc when picked up.

## 8. M7-3 — Allocation-Site Flame Graphs (framing only)

**Goal:** `mnemosyne flamegraph <heap.hprof> -o flame.svg` produces a retained-size flame graph rooted in stack-trace / allocation-site data.

**Scope sketch:**
- New CLI subcommand `flamegraph`.
- New `core::report::flamegraph` module producing folded-stack output and an SVG renderer (consider `inferno` crate).
- Requires deep-mode data (allocation sites + dominator-derived retained sizes).
- Errors cleanly when fed an overview-mode input.

**Dependencies:** none beyond v0.2.0 deep-mode data; M7-1 only for clean error path.
**Detailed design:** addendum doc when picked up.

## 9. M7-4 — OQL Targeted Expansion (framing only)

**Goal:** Close the highest-value MAT OQL gaps without committing to the full grammar.

**Candidate predicates (prioritized, final list TBD by addendum):**
1. `WHERE @toString LIKE '...'` — string match against synthetic toString projection.
2. `WHERE @retainedSize > N` / `< N` — numeric comparison on retained size (deep mode only).
3. `WHERE @gcRootPath CONTAINS 'classname'` — path-based filter.
4. `OBJECTS x.field` — single-hop projection across an instance field.
5. `SELECT *, @retainedSize FROM ...` — projection of pseudo-attributes.
6. `WHERE x INSTANCEOF y AND x.field IS NULL` — boolean composition (already partially supported).

**Dependencies:** existing query engine + M3 dominator tree.
**Detailed design:** addendum doc when picked up.

## 10. M7-5 — Comparative Benchmarks (framing only)

**Goal:** Reproducible head-to-head benchmarks vs Eclipse MAT and hprof-slurp on shared fixtures.

**Scope sketch:**
- Reference workstation spec captured in `docs/benchmarks.md`.
- Fixture matrix: 156 MB (existing), 1 GB, 4 GB, 10 GB synthetic dumps from `scripts/generate_synthetic_heap.sh`.
- Metrics: wall time, max RSS, peak file-cache, output equivalence (top-10 class set overlap, parser correctness).
- Tools: `hyperfine` for timing, `heaptrack` / `/usr/bin/time -v` for RSS, scripted MAT/hprof-slurp invocations.
- Publish results as `docs/benchmarks-vs-competitors.md` + raw CSV under `docs/performance/`.

**Dependencies:** M7-1 lands the overview path that makes the 10 GB tier feasible; M7-2 / M7-3 should land first so the comparison includes the differentiated workflows.
**Detailed design:** addendum doc when picked up.

## 11. M7-6 — v0.3.0 Release (framing only)

**Goal:** Cut `v0.3.0` across all v0.2.0 release channels (GitHub release, crates.io, Docker, Homebrew) with release notes that lead with M7's three credibility wins: 10 GB scale, CI policies, flame graphs.

**Scope sketch:**
- Driven by `.github/prompts/release-prep.md` and `.github/skills/finishing-a-development-branch/SKILL.md`.
- CHANGELOG / STATUS / roadmap updated by Documentation Sync against the actual shipped scope.
- Comparative benchmark publication (M7-5) is a hard release gate.

**Dependencies:** all preceding M7 slices.

## 12. Validation strategy (milestone-level)

| Gate | Owner | Trigger |
|---|---|---|
| `cargo check --workspace` clean | Implementation Agent | every slice |
| `cargo test --workspace --all-targets` clean | Testing Agent | every slice |
| `cargo clippy --workspace --all-targets -- -D warnings` clean | Static Analysis Agent | every slice |
| `cargo fmt --all -- --check` clean | Static Analysis Agent | every slice |
| Test count ≥ 260 by end of M7 | Testing Agent | M7-6 release gate |
| Comparative benchmark doc published | Documentation Sync | M7-6 release gate |
| Provenance honesty review | Architecture Review Agent | end of each milestone slice that touches output |
| Documentation drift sweep | Documentation Sync | end of each batch |

## 13. Rollout phases (milestone-level)

1. **Phase 1 — Foundation:** M7-1 (slices A → E) lands streaming overview mode.
2. **Phase 2 — Differentiation:** M7-2 and M7-3 in parallel (different code paths, no file overlap).
3. **Phase 3 — Depth:** M7-4 OQL predicates, picked one at a time.
4. **Phase 4 — Validation & release:** M7-5 benchmark publication, then M7-6 v0.3.0 release.

## 14. Implementation readiness verdict

**READY AFTER DOC UPDATE** — this document was created during this design pass. M7-1 is specified at slice depth and the Implementation Agent may proceed with **Slice M7-1.A (`AnalysisMode` enum + boundary plumbing)** as the first task. M7-2 through M7-6 require their own addendum design docs before their respective implementation begins; framing in §§7–11 is sufficient for roadmap alignment but **not** sufficient for coding.

### 14.1 Post-M7-1 update (2026-04-26)

M7-1 is **complete end-to-end**. All five slices (A–E) shipped via commits `7e27e0e`, `8ceb43b`, `c128131`, `995f92b`, `8534f37`, `09ee9c5`, with documentation sync in `c64b4a1`. The workspace test count stands at **268** with `cargo check`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check` all clean. Streaming overview mode is now reachable from CLI (`parse|analyze --mode {auto,deep,overview}`), MCP (`parse_heap` / `analyze_heap` `mode` parameter), and core (`core::hprof::overview`), with byte-identical deep-mode JSON preserved by default.

**M7-2 design is now in progress.** The detailed addendum lives at [milestone-7-2-ci-regression-policies.md](milestone-7-2-ci-regression-policies.md) and supersedes the framing in §7 of this document for implementation purposes. Once that addendum returns `READY`, the Implementation Agent may begin with **Slice M7-2.A (`core::policy` skeleton + TOML parser)**. M7-3 through M7-6 still require their own addendum design docs before coding begins.

### 14.2 Post-M7-2 update (2026-04-26)

M7-2 is **complete end-to-end**. All five slices (A–E) shipped via commits `2cbd4cc`, `2c9ff65`, `bdc6b5f`, `c6812a5`, `3c81006`, `5b147a0`, `846be08`. `mnemosyne-cli ci-check` is reachable with policy TOML, severity-aware exit codes, and text/JSON/JUnit/GitHub-Actions renderers. Workspace `cargo {check, test --workspace --all-targets, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` are clean and the test count has advanced from 268 to 330. Roadmap and STATUS reflect the shipped scope.

**M7-3 design is now in progress.** The detailed addendum lives at [milestone-7-3-allocation-site-flame-graphs.md](milestone-7-3-allocation-site-flame-graphs.md) and supersedes the framing in §8 of this document for implementation purposes. Once that addendum returns `READY`, the Implementation Agent may begin with **Slice M7-3.A (folded-stack format types + dominator collapser)**. M7-4 through M7-6 still require their own addendum design docs before coding begins.

### 14.3 Post-M7-3 update (2026-04-26)

M7-3 is **complete end-to-end**. All slices shipped via commits `4539f1a`, `b2f32c0`, `cfe36f0`, `e6e7458`, `8b15fad`, `adaf46c`. `mnemosyne-cli flamegraph` is reachable with class-hierarchy, dominator, and gc-root-path collapse strategies plus folded-stack and SVG renderers; the corresponding MCP tool is wired and overview-mode partial-result semantics are preserved. Workspace `cargo {check, test --workspace --all-targets, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` are clean. Roadmap and STATUS reflect the shipped scope.

**M7-4 design is now in progress.** The detailed addendum lives at [milestone-7-4-oql-targeted-expansion.md](milestone-7-4-oql-targeted-expansion.md) and supersedes the framing in §9 of this document for implementation purposes. Once that addendum returns `READY`, the Implementation Agent may begin with **Slice M7-4.A (pseudo-attribute infrastructure: real `@toString`, `@gcRootPath` field, lexer/AST hooks for `CONTAINS`/`OBJECTS`/`IS NULL`)**. M7-5 and M7-6 still require their own addendum design docs before coding begins.

### 14.4 Post-M7-4 update (2026-04-26)

M7-4 is **complete end-to-end**. All slices shipped via commits `2311d5c`, `4afd062`, `7be4799`, `ea8421b`, `fd4c787`, `2f029bb`, `bfa4e19`. The targeted OQL expansion delivers `@retainedSize`, `@toString` (real `String` contents), `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS`, and `IS NULL` / `IS NOT NULL` while keeping full MAT OQL parity deferred to M8-2. Workspace `cargo {check, test --workspace --all-targets, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` are clean. Roadmap and STATUS reflect the shipped scope.

**M7-5 design is now in progress.** The detailed addendum lives at [milestone-7-5-comparative-benchmarks.md](milestone-7-5-comparative-benchmarks.md) and supersedes the framing in §10 of this document for implementation purposes. M7-5 differs from prior slices: most of the work is **methodology, scripting, and measurement**, and the actual benchmark execution (4 tools × 4 fixtures × N=5 runs, including a 10 GB heap dump) cannot run inside an agent session — it requires hardware access and time on the reference workstation. The agent ships slices A, B, and D; **slice C (run benchmarks and capture raw data) is the user's responsibility** on real hardware, with the agent producing the harness and the analysis-template. Once the M7-5 addendum returns `READY`, the Implementation Agent may begin with **Slice M7-5.A (fixture generation + reference workstation spec + tool installation guide)**. M7-6 still requires its own addendum design doc before release work begins.
