# Milestone 7-3 — Allocation-Site Flame Graphs (`mnemosyne flamegraph`)

> **Status:** 🟡 Pending — design complete, implementation not started
> **Predecessors:** M7-1 streaming overview mode ✅ shipped; M7-2 CI regression policies ✅ shipped (`2cbd4cc`, `2c9ff65`, `bdc6b5f`, `c6812a5`, `3c81006`, `5b147a0`, `846be08`)
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Parent design:** [milestone-7-production-readiness.md](milestone-7-production-readiness.md) §8
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Last updated:** 2026-04-26

This addendum is the implementation-depth design for M7-3. It supersedes the framing in §8 of the parent M7 design doc for the purpose of coding. Slices defined here (M7-3.A through M7-3.D) are gated by the `READY` verdict in §18.

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone slice | M7-3 |
| Type | Differentiation |
| Predecessors | M7-1 (streaming overview mode), M7-2 (CI regression policies) |
| Required prerequisites | `core::analysis::AnalysisMode`, deep-mode `AnalyzeResponse` with `GraphMetrics { dominators, … }`, `core::graph::dominator::DominatorTree`, `core::hprof::ObjectGraph`, `core::hprof::LoadedClass` (all present on `main`) |
| New crates | `inferno` (runtime, `core` only — see §10 and §16) |
| Cargo.toml deltas (anticipated) | `inferno = { version = "0.11", default-features = false, features = ["nameattr"] }` runtime in `core`; no MSRV impact expected (validated as part of Slice C) |
| Test count target | +12 to +15 net new tests (push workspace from 330 → ≥ 342) |

## 2. Objective

`mnemosyne flamegraph <heap.hprof> -o flame.svg` reads a heap dump in deep mode, walks the dominator tree (or class hierarchy, or shortest GC-root paths) into folded-stack frames weighted by retained or shallow bytes, and emits an interactive SVG flame graph that can be opened in any browser. The same subcommand can also emit the intermediate folded-stack text format and a structured JSON envelope so the data is consumable by external tools (`flamegraph.pl`, `speedscope`) and Mnemosyne's own MCP/AI pipelines.

The differentiation Mnemosyne is claiming with this command is precisely scoped: **flame graphs from a post-mortem heap dump, weighted by retained size, exportable as a self-contained SVG**. No live profiling required.

## 3. Context

The competitive landscape for heap-dump flame graphs is shallow:

- **Eclipse MAT** ships a "thread overview" view that approximates a stack-rooted retention picture but offers no proper SVG export and no class-hierarchy or GC-root-path roots.
- **hprof-slurp** has no flame-graph output at all; it is a streaming summarizer.
- **JFR + async-profiler** ship excellent flame graphs, but for **CPU samples or live allocation events** captured during a recording session — not from a post-mortem `.hprof` file. They cannot help an operator who only has the dump.
- **YourKit / JProfiler** are GUI-first commercial tools; their flame-graph-style views are not scriptable, not CI-shaped, and not exportable as standalone SVG.

Mnemosyne's differentiation, restated:

1. Inputs are post-mortem heap dumps (HPROF), not live recordings.
2. Weights are **retained sizes** (computed from the dominator tree), not shallow sizes alone — answering "what holds memory" instead of just "what is allocated".
3. Output is a self-contained, browser-openable SVG produced from the CLI in one shot, plus a folded-stack text format that other ecosystems already consume.

The reference SVG renderer is the [`inferno`](https://crates.io/crates/inferno) crate, which is the Rust port of Brendan Gregg's `flamegraph.pl`. It accepts folded-stack text and emits the same interactive SVG (search, zoom, hover) that profiling toolchains have standardized on.

This is pure differentiation territory. M7-3 widens the moat opened by M5/M6 (structured outputs) and M7-2 (CI workflow) into a first-class visualization surface.

## 4. Scope

In:

- New CLI subcommand `mnemosyne flamegraph` with the surface defined in §8.
- New `core::report::flamegraph` module (sibling of `core::report::renderer`):
  - Folded-stack format types and emitter (§9).
  - Three rooting strategies / collapsers (§7).
  - SVG renderer wrapping `inferno::flamegraph` (§10).
  - JSON envelope renderer for programmatic consumption (§11).
- Mode-compatibility enforcement: deep mode required (§12).
- Frame-budget controls: `--min-fraction` folding and `--max-frames` cap (§§7.4, 9.2, 17/R3).
- CLI integration tests covering each `--root` value, each `--format` value, the size cap, and the mode-mismatch error path.

Out:

- **No live profiling.** No JVMTI, no JFR ingestion, no allocation-event recording. Those are non-M7 ideas tracked in the v0.4+ backlog if at all.
- **No diff between two flame graphs.** That is M8-1 (object-level diff) territory.
- **No Tauri UI integration.** Embedding the SVG into the desktop UI is M8-9.
- **No MCP exposure** in this milestone. A `flamegraph` MCP tool is a follow-up batch and is explicitly deferred (§13). The CLI surface is the only public entry point in M7-3.
- **No allocation-site recording** based on HPROF stack-trace records. HPROF stack frames *are* present and parsed (`core::hprof::StackTrace`/`StackFrame`), but the dominant allocation-site signal in a typical dump is too sparse to drive a flame graph; the three rooting strategies in §7 are graph-derived, not allocation-site-derived. The name "allocation-site flame graphs" in the roadmap and parent doc is preserved as the umbrella term, but the actual rootings are dominator-, class-hierarchy-, and GC-root-path-based.
- **No breaking changes** to `parse`, `analyze`, `leaks`, `diff`, `query`, `map`, `gc-path`, `explain`, `chat`, `fix`, `serve`, or `ci-check`.

## 5. Architecture overview

```
┌──────────────────────┐
│ mnemosyne flamegraph │  CLI entry: cli/src/main.rs
│  args parse (clap)   │
└──────────┬───────────┘
           │ FlameGraphArgs
           ▼
┌──────────────────────┐
│ Mode resolution      │  core::analysis::AnalysisMode (existing)
│ (auto/deep/overview) │  + flame-graph mode-compatibility check (§12)
└──────────┬───────────┘
           │ AnalysisMode (resolved → must be Deep)
           ▼
┌──────────────────────┐
│ Deep heap analysis   │  core::analysis::analyze_heap → AnalyzeResponse
│  + dominator tree    │  internally retains ObjectGraph + DominatorTree handle
│                      │  (see §6 — the public path needs a small addition)
└──────────┬───────────┘
           │ FlameGraphInput { graph, dominator, classes, response }
           ▼
┌──────────────────────┐
│ Collapser            │  core::report::flamegraph::collapse::{
│  (per --root)        │      dominator,
│                      │      class_hierarchy,
│                      │      gc_root_path,
│                      │  } → FoldedStacks
└──────────┬───────────┘
           │ FoldedStacks
           ▼
┌──────────────────────┐
│ Frame-budget pass    │  fold-by-min-fraction; truncate to max_frames
│  (§§7.4, 9.2)        │
└──────────┬───────────┘
           │ FoldedStacks (bounded)
           ▼
┌──────────────────────┐
│ Renderer             │  --format svg            → core::report::flamegraph::render::svg
│  (--format)          │  --format folded-stack   → core::report::flamegraph::render::folded
│                      │  --format json           → core::report::flamegraph::render::json
└──────────┬───────────┘
           │ bytes
           ▼
┌──────────────────────┐
│ Output writer        │  --output <path>  (mandatory; flame graphs are not stdout-friendly)
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Exit code mapping    │  ok → 0; mode mismatch → 5; bad heap → 3; IO → 2
└──────────────────────┘
```

Module placement rationale:

- `core::report::flamegraph` lives under `core::report` because the artifact (an SVG, a folded-stack file, or a JSON envelope) is a **report** rendered from analysis output. It is the same shape as `core::report::renderer` (which already owns text/JSON renderers for `AnalyzeResponse`).
- The collapser logic is **not** part of `core::analysis` because it does not derive new analytical insight; it is a projection of existing insight (`DominatorTree`, `ObjectGraph`, `LoadedClass` chain) into a different shape.
- The collapser is **not** part of `core::graph` because it is consumer-facing presentation logic, not a graph algorithm.

## 6. Public API addition (small)

The current `analyze_heap` returns an `AnalyzeResponse` whose `graph: GraphMetrics` field exposes `dominators: Vec<DominatorNode>` and node/edge counts, but the underlying `ObjectGraph` and `DominatorTree` are dropped at the end of the function (`try_build_dominator` returns them locally and they go out of scope). Flame-graph collapsing needs both the full `ObjectGraph` (for class names, references, GC roots) and the full `DominatorTree` (for parent/child traversal and retained sizes), not just the post-aggregation `GraphMetrics`.

Two acceptable options; this addendum picks **option B**:

- **Option A — extend `AnalyzeResponse`.** Attach `Option<Arc<ObjectGraph>>` and `Option<Arc<DominatorTree>>`. Pro: one analysis pass produces everything. Con: bloats the JSON-serialized response (must be marked `#[serde(skip)]`), and pollutes a CI-shaped artifact with internal handles. Also forces every caller to think about reference cycles.
- **Option B — separate entry point for flame-graph callers (chosen).** Add `core::analysis::analyze_heap_with_graph(request) -> CoreResult<(AnalyzeResponse, ObjectGraph, DominatorTree)>`. The existing `analyze_heap` continues to return only `AnalyzeResponse` and is byte-identical. The flame-graph CLI handler calls `analyze_heap_with_graph` exclusively. **No serialization surface changes.**

Option B is implemented in Slice M7-3.A as a thin wrapper that runs `try_build_dominator` once, builds the response from those two values, and returns the trio. The existing `analyze_heap` is reimplemented as `analyze_heap_with_graph(req).map(|(r, _, _)| r)` to keep a single source of truth.

## 7. Rooting strategies

All three strategies produce the same in-memory shape (`FoldedStacks`, §9.1) and are therefore renderer-agnostic. Selection is via `--root <type>`.

### 7.1 `dominator` (default)

Frames are the dominator chain from the virtual super-root → GC root → … → leaf object. Weight per leaf is the leaf's **shallow** size; weight per internal frame is the sum of its descendants' shallow sizes (which equals its retained size by construction of the dominator tree).

Algorithm:

1. Start at `VIRTUAL_ROOT_ID` (`core::graph::dominator::VIRTUAL_ROOT_ID`).
2. Iterate `dom.dominated_by(id)` recursively in a stable order (sort children by retained size descending, then by class name ascending — see §17/R2 for determinism).
3. For each path from root to a leaf, emit one folded-stack line `frame0;frame1;…;frameN  shallow_bytes`.
4. Frame name is the class name of the dominated object (or `<gc-root>` for the virtual root, or `<unknown class id=…>` when the class id is not in `graph.loaded_classes`).
5. Frame elision: if a sub-tree's total weight is below `min_fraction * total_size`, the whole subtree collapses into a single synthetic `<other:N classes>` frame. The frame budget pass (§9.2) enforces this after collection.

This is the most useful strategy for "what holds memory". It is the default.

### 7.2 `class-hierarchy`

Frames are the Java class **inheritance** chain (`java.lang.Object → java.lang.Throwable → java.lang.Exception → com.example.MyException`). Weight is the **total shallow bytes of all instances of that class**.

Algorithm:

1. Group `graph.objects` by `class_id`. Sum shallow size per class to get `bytes_per_class`.
2. For each `LoadedClass` in `graph.loaded_classes`, walk `super_class_id` upward to root, emitting a stack `Object;…;LeafClass  bytes_per_class`.
3. Sibling order: deterministic by class name (lexical).
4. Frame elision: same `min_fraction` policy as §7.1.

This is the most useful strategy for "where did all the `SQLException` come from / which classes dominate this dump". It does **not** require the dominator tree; it works from `ObjectGraph` alone. (It still requires deep mode because overview mode does not retain per-object class id mappings.)

### 7.3 `gc-root-path`

Frames are the **shortest path from any GC root to each leak suspect or top-N retained class**. Weight is retained bytes.

Algorithm:

1. Seed set: union of (a) every `LeakInsight.suspect_object_id` in `AnalyzeResponse.leaks`, and (b) every object id in `AnalyzeResponse.graph.dominators` whose retained size is in the top `max(50, ceil(max_frames / 32))` (the cap keeps the BFS tractable on huge graphs).
2. Build the **reverse** edge index from `graph.objects[*].references` (a `HashMap<ObjectId, Vec<ObjectId>>`). Cache on `FlameGraphInput` so multi-strategy invocations only pay this cost once.
3. For each seed, BFS from the seed through the reverse edges until a GC root is reached. Stop at the first GC root encountered (shortest path). Reverse the path so frames read root → seed.
4. Emit one folded-stack line per seed: `<gc-root:KIND>;ClassA;ClassB;…;SeedClass  retained_bytes`.
5. Cycle handling: BFS already ignores revisits; an explicit `visited: HashSet<ObjectId>` per seed guarantees termination.
6. Depth cap: hard cap of `64` frames per path. Paths longer than 64 are truncated to the first 32 frames + `<…elided N…>` + the last 31 frames. (Discussed in §17/R4.)

This is the most useful strategy for "why is this not collected".

### 7.4 Frame-budget interaction across strategies

After a collapser produces its raw `FoldedStacks`, a single budget pass (§9.2) enforces:

- `--min-fraction`: any frame (or subtree, for `dominator`) whose weight is `< min_fraction * total_weight` folds into `<other>` at its current depth.
- `--max-frames`: if frame count after fraction folding still exceeds the cap, drop the lowest-weight frames first (stable tie-break by name) and aggregate them into the same `<other>` bucket. **The `<other>` bucket itself is never dropped**, even if it is small — it preserves the total weight invariant (§9.3).

## 8. CLI surface

Exact `clap` derive structure to be added to `cli/src/main.rs` (additive — does not touch existing `Commands` enum variants):

```rust
#[derive(Subcommand, Debug)]
enum Commands {
    // ...existing variants unchanged, including CiCheck(CiCheckArgs)...
    /// Render a flame graph from a heap dump (deep mode required).
    Flamegraph(FlameGraphArgs),
}

#[derive(Args, Debug)]
struct FlameGraphArgs {
    /// Heap dump to analyze.
    heap: PathBuf,

    /// Output file (SVG, folded-stack text, or JSON depending on --format).
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Rooting strategy.
    #[arg(long, value_enum, default_value_t = FlameRoot::Dominator)]
    root: FlameRoot,

    /// Output format.
    #[arg(long, value_enum, default_value_t = FlameFormat::Svg)]
    format: FlameFormat,

    /// Analysis mode. Flame graphs require deep; auto fails loud on overview-sized inputs.
    #[arg(long, value_enum, default_value_t = ModeArg::Auto)]
    mode: ModeArg,

    /// Frames smaller than this fraction of total weight are folded into "<other>".
    #[arg(long, default_value_t = 0.001)]
    min_fraction: f64,

    /// SVG title (defaults to "Mnemosyne flame graph — <heap basename>").
    #[arg(long)]
    title: Option<String>,

    /// Hard cap on total frame count after fraction folding.
    #[arg(long, default_value_t = 5000)]
    max_frames: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FlameRoot {
    Dominator,
    ClassHierarchy,
    GcRootPath,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FlameFormat {
    Svg,
    FoldedStack,
    Json,
}
```

`ModeArg` already exists from M7-1. `FlameRoot` and `FlameFormat` are new and live alongside the other CLI enums in `cli/src/main.rs`. They map 1:1 to `core::report::flamegraph::FlameRoot` and `core::report::flamegraph::FlameFormat` via `From` impls (same pattern as `ModeArg → AnalysisMode`).

Help-text shape (rendered by clap from doc-comments):

```text
mnemosyne flamegraph <HEAP> -o <PATH> [--root dominator|class-hierarchy|gc-root-path]
                                       [--format svg|folded-stack|json]
                                       [--mode auto|deep|overview]
                                       [--min-fraction <FLOAT>]
                                       [--title <STRING>]
                                       [--max-frames <UINT>]
```

`--output` is **mandatory**: flame graphs are binary-ish (SVG can be 100s of KB) and the JSON envelope is large; piping to stdout is not a workflow we support.

## 9. Folded-stack format

### 9.1 In-memory shape

```rust
// core/src/report/flamegraph/types.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FoldedStacks {
    /// Total weight across all stacks (bytes). Invariant in the budget pass (§9.3).
    pub total_weight: u64,
    /// Each stack: ordered frames root → leaf, plus the leaf's weight.
    pub stacks: Vec<FoldedStack>,
    /// Strategy that produced this set (informational; round-tripped to JSON).
    pub root: FlameRoot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FoldedStack {
    pub frames: Vec<String>, // sanitized per §9.4
    pub weight: u64,         // bytes
}
```

### 9.2 Text format

One stack per line:

```
frame0;frame1;frame2 1234
```

- Field separator: ASCII `;` (matches Brendan Gregg / `inferno`).
- Frame/weight separator: a single ASCII space.
- Weight: integer, base 10, **bytes** (no `KB`/`MB` suffixes — keeps numeric tools simple).
- Line terminator: `\n` (LF), regardless of host OS.
- The order of lines is sorted: descending by weight, then ascending by joined frame string. This makes the output deterministic.

Example (truncated, `--root dominator`):

```
<gc-root:System Class>;java.lang.Class;java.util.HashMap;java.util.HashMap$Node 8388608
<gc-root:Java Stack Frame>;java.lang.String;[B 4194304
<gc-root:System Class>;<other:42 classes> 1048576
```

### 9.3 Total-weight invariant

After fraction folding and max-frame truncation, `sum(stacks.iter().map(|s| s.weight)) == total_weight` must hold. The `<other>` bucket exists precisely to absorb the residue from elision and truncation. Tests in §14 lock this invariant.

### 9.4 Frame-name sanitization

Class names in the JVM include `;` (field descriptors), `[` (array prefixes), and `<` (synthetic markers). The folded-stack format uses `;` as the field separator, so `;` in a frame name corrupts parsing.

Sanitization rules applied at emit time:

1. Replace any `;` inside a frame with `,`.
2. Leave `[` and `<` and `>` as-is — `inferno` accepts them, and they are part of standard JVM type strings (`[B`, `<init>`, `<clinit>`).
3. Replace any `\n`, `\r`, or `\t` with a single space.
4. UTF-8 is preserved as-is; both `inferno` and `flamegraph.pl` accept UTF-8 frame names.

A standalone `sanitize_frame(name: &str) -> Cow<'_, str>` helper lives in `core::report::flamegraph::types` and is the only path frame names take to the renderer.

## 10. SVG renderer choice

**Recommendation: depend on `inferno` (default).**

Justification:

- License: **CDDL-1.0** for `inferno` itself. CDDL is generally considered compatible with both Apache-2.0 and MIT in distribution scenarios because it is file-based and reciprocal only on modified `inferno` source. We do not modify `inferno`; we link to it. (Confirm during Slice C with `cargo deny` or a manual scan of the resolved tree; Mnemosyne is dual MIT / Apache-2.0.) Action item logged as R1 in §17.
- Maintenance: actively maintained Rust port of `flamegraph.pl`, used widely (`cargo flamegraph`, `flamegraph` cargo subcommand).
- API: stable `from_lines` / `from_reader` interface accepting folded-stack input and writing SVG to any `io::Write`. This is exactly the shape we need.
- Output parity: produces the same interactive SVG (search, zoom, hover) that profiler users already know.

Fallback plan: if Slice C uncovers a license-tree problem, an MSRV regression, or a binary-size regression that the team rejects, the fallback is a hand-rolled minimal SVG renderer (one `<rect>` + one `<text>` per frame, rectangles laid out left-to-right, depth-stacked top-to-bottom, no embedded JS). Limitations of the fallback:

- No interactive zoom (the `<script>` block in `inferno`'s template is what powers zoom/search).
- No hover tooltip (would require additional `<title>` children — feasible but more code).
- No search box.
- Output size and visual quality are still acceptable for a static deliverable.

The hand-rolled fallback is fully specified in §10.1 of the implementation Slice C handoff so it can be activated without further design.

### 10.1 Minimal hand-rolled SVG (fallback specification)

If used, the renderer:

1. Computes total width = max(1200, frame_count * 8) and total height = (max_depth + 2) * 16 px.
2. For each frame, draws a `<rect>` whose width is `(weight / total_weight) * width`, x-offset is the cumulative offset within its parent, y-offset is `depth * 16`, fill is a stable hash of the frame name (HSL palette, same as inferno's "hot" palette).
3. Draws a `<text>` child sized to fit (font-size 12, font-family "monospace"); truncates with ellipsis at runtime via `<title>` for the hover tooltip.
4. Wraps the whole thing in `<svg xmlns="http://www.w3.org/2000/svg" viewBox=…>` and emits no `<script>` or `<style>` (keeps the SVG pure, browser-renderable, and CSP-safe).

## 11. Output formats

| Format | Extension | Producer | Consumer expectation |
|---|---|---|---|
| `svg` (default) | `.svg` | `inferno::flamegraph::from_lines` (§10) | Open in any browser; interactive zoom/search/hover. |
| `folded-stack` | `.txt` (convention) | direct serialize of §9.2 | `flamegraph.pl`, `speedscope`, custom tooling. |
| `json` | `.json` | `serde_json::to_writer_pretty` over §11.1 | Mnemosyne MCP / AI / programmatic. |

### 11.1 JSON envelope

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameGraphEnvelope {
    pub schema_version: u32,            // 1
    pub generator: String,              // "mnemosyne <version>"
    pub heap_path: String,
    pub mode: AnalysisMode,             // always Deep in M7-3
    pub root: FlameRoot,
    pub generated_at: String,           // RFC3339, UTC
    pub total_weight_bytes: u64,
    pub frame_count: usize,
    pub min_fraction: f64,
    pub max_frames: usize,
    pub stacks: Vec<FoldedStack>,       // post-budget, deterministic order
    pub provenance: Vec<ProvenanceMarker>, // copied from AnalyzeResponse + flame-graph markers
}
```

`schema_version` starts at `1`. Any breaking change bumps the version and is announced in `CHANGELOG.md`.

## 12. Mode interaction

Flame graphs require deep mode. The CLI handler enforces this **before** running analysis to avoid wasted work:

1. Resolve `--mode`:
   - `--mode deep` → proceed regardless of size; if file size > 4 GiB, emit a `tracing::warn!` "deep-mode flame graph on a large dump may exhaust host memory".
   - `--mode overview` → fail immediately with **exit code 5** and stderr message: `flame graph requires deep mode; received --mode overview`.
   - `--mode auto` → resolve via `core::analysis::resolve_mode(file_size)`. If the resolution is `Overview`, fail with **exit code 5** and stderr message: `flame graph requires deep mode; current heap size triggers auto-overview; pass --mode deep to override`. If the resolution is `Deep`, proceed.

2. After the deep analysis runs, an additional sanity check confirms `response.mode == AnalysisMode::Deep` and aborts with exit code 5 otherwise (defense in depth — this should be unreachable).

Exit-code table:

| Code | Reason |
|---|---|
| 0 | Flame graph rendered. |
| 2 | IO error reading heap or writing output. |
| 3 | Heap parsing failed (corrupt or unreadable HPROF). |
| 5 | Mode mismatch (overview where deep required) or empty graph (no frames to render). |

The codes intentionally do not collide with `ci-check`'s `--fail-on` (0/1/4) or schema/IO codes (2/3) from M7-2. Code 5 is new in M7-3 and is reserved for "input shape incompatible with this command".

## 13. Differentiator continuity

- **Provenance markers.** The flame-graph output carries the same `provenance` vector as `AnalyzeResponse` (copied) plus a marker of its own:
  ```text
  ProvenanceMarker {
      kind: ProvenanceKind::Generated,
      detail: Some("flamegraph: root=<strategy>, total_weight=<bytes>, frames=<count>, min_fraction=<float>"),
  }
  ```
  Embedded in the JSON envelope. For the SVG format, the same string is rendered into the SVG `<desc>` element (machine-readable) and into the bottom-right corner of the SVG (human-readable).

- **Structured output.** The `json` format makes flame graphs first-class structured data, not an opaque image. This is the same pattern `analyze`, `leaks`, `query`, and `ci-check` already follow.

- **MCP exposure.** Deferred to a follow-up batch (out of M7-3 scope per §4). The data shape (`FlameGraphEnvelope`) is designed to be returnable as an MCP tool result without changes; the deferral is purely about CLI/MCP wiring, not data design.

## 14. Test plan

Test count target: **+12 to +15** new tests. The plan targets the lower bound; the upper bound covers margin for snapshot tests added during slice review.

### Unit tests (collapsers) — `core/src/report/flamegraph/collapse/*`

| Test | Asserts |
|---|---|
| `collapse_dominator_simple_chain` | Linear root → A → B graph produces one stack, weight equals B's shallow size, frames are `[<gc-root:…>, A, B]`. |
| `collapse_dominator_branching_preserves_total` | Branching tree's stack weights sum to the dominator-tree total retained at the virtual root. |
| `collapse_dominator_unknown_class_id_uses_placeholder` | An object with an unmapped class id produces frame `<unknown class id=N>` and does not panic. |
| `collapse_class_hierarchy_walks_super_chain` | A class with a 4-deep ancestor chain emits exactly one stack of length 4. |
| `collapse_class_hierarchy_groups_by_class` | Two instances of the same class merge into one stack with summed weight. |
| `collapse_gc_root_path_shortest_wins` | When two GC roots can reach the same seed at depths 2 and 5, the depth-2 path is selected. |
| `collapse_gc_root_path_cycle_terminates` | A graph with a `A → B → A` cycle does not loop forever and does not duplicate frames. |

### Unit tests (frame budget + format) — `core/src/report/flamegraph/types.rs`

| Test | Asserts |
|---|---|
| `min_fraction_folds_small_subtrees_into_other` | Frames below threshold collapse into `<other:N classes>`; total weight invariant holds. |
| `max_frames_truncates_lowest_weight_first` | Truncation drops smallest weights first; `<other>` bucket absorbs them. |
| `sanitize_frame_replaces_semicolons_with_commas` | A frame containing `Foo;Bar` becomes `Foo,Bar`. |
| `folded_text_is_deterministic` | Two runs over the same input produce byte-identical folded-stack text. |

### Unit tests (renderers) — `core/src/report/flamegraph/render/*`

| Test | Asserts |
|---|---|
| `svg_smoke_is_valid_xml` | Output starts with `<?xml` or `<svg`, parses with a minimal XML reader, contains at least one `<rect>` per emitted frame. |
| `svg_byte_size_in_expected_range` | For a known 100-frame fixture, output size is within `[5_000, 250_000]` bytes. |
| `json_envelope_round_trips` | `serde_json::to_string` then `from_str` reproduces the same `FlameGraphEnvelope`. |

### CLI integration tests — `cli/tests/integration.rs`

| Test | Asserts |
|---|---|
| `flamegraph_dominator_svg_smoke` | Default invocation on a tiny synthetic dump exits 0 and writes a non-empty SVG file. |
| `flamegraph_class_hierarchy_folded_stack_smoke` | `--root class-hierarchy --format folded-stack` produces a file matching the format in §9.2. |
| `flamegraph_gc_root_path_json_smoke` | `--root gc-root-path --format json` produces a `FlameGraphEnvelope` with non-empty stacks and matching `root` field. |
| `flamegraph_explicit_overview_exits_five` | `--mode overview` exits with code 5 and stderr contains "flame graph requires deep mode". |
| `flamegraph_auto_mode_on_large_dump_exits_five` | A fixture that resolves to overview under auto-mode exits with code 5. |
| `flamegraph_max_frames_cap_respected` | `--max-frames 10` produces a file with ≤ 10 distinct frames (counted by parsing the folded output). |

### Regression

| Test | Asserts |
|---|---|
| Existing 330-test workspace baseline | All pre-M7-3 tests pass unchanged. |
| `analyze`, `parse`, `ci-check`, `leaks`, `query`, `gc-path`, `serve` outputs unchanged | M7-3 is purely additive; no shared-mutable state, no change to existing renderers, no change to `AnalyzeResponse` serialization. |

## 15. Slice breakdown

Four TDD-friendly slices, each ending with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice M7-3.A — Folded-stack types + dominator collapser + tests

- **Files affected:**
  - `core/src/report/flamegraph/mod.rs` (new — module root, re-exports)
  - `core/src/report/flamegraph/types.rs` (new — `FoldedStacks`, `FoldedStack`, `FlameRoot`, `FlameFormat`, `sanitize_frame`)
  - `core/src/report/flamegraph/collapse/mod.rs` (new — `FlameGraphInput`, dispatcher)
  - `core/src/report/flamegraph/collapse/dominator.rs` (new)
  - `core/src/report/flamegraph/budget.rs` (new — `apply_budget(stacks, min_fraction, max_frames) -> FoldedStacks`)
  - `core/src/analysis/engine.rs` (add `analyze_heap_with_graph`, refactor `analyze_heap` to delegate; no behavior change)
  - `core/src/report/mod.rs` (add `pub mod flamegraph;`)
- **Tests:** `collapse_dominator_simple_chain`, `collapse_dominator_branching_preserves_total`, `collapse_dominator_unknown_class_id_uses_placeholder`, `min_fraction_folds_small_subtrees_into_other`, `max_frames_truncates_lowest_weight_first`, `sanitize_frame_replaces_semicolons_with_commas`, `folded_text_is_deterministic`.
- **Out of scope:** other collapsers, SVG, JSON, CLI.
- **Target size:** ~350 LOC + ~250 LOC tests.

### Slice M7-3.B — Class-hierarchy and gc-root-path collapsers + tests

- **Files affected:**
  - `core/src/report/flamegraph/collapse/class_hierarchy.rs` (new)
  - `core/src/report/flamegraph/collapse/gc_root_path.rs` (new — includes reverse-edge index)
  - `core/src/report/flamegraph/collapse/mod.rs` (extend dispatcher)
- **Tests:** `collapse_class_hierarchy_walks_super_chain`, `collapse_class_hierarchy_groups_by_class`, `collapse_gc_root_path_shortest_wins`, `collapse_gc_root_path_cycle_terminates`.
- **Out of scope:** SVG, JSON, CLI.
- **Target size:** ~300 LOC + ~200 LOC tests.

### Slice M7-3.C — SVG renderer (via `inferno`) + JSON envelope + folded-stack output to file

- **Files affected:**
  - `core/Cargo.toml` (add `inferno` runtime dep — see §10; verify license/MSRV during this slice)
  - `core/src/report/flamegraph/render/mod.rs` (new)
  - `core/src/report/flamegraph/render/svg.rs` (new — `inferno` wrapper; behind `inferno` dep)
  - `core/src/report/flamegraph/render/folded.rs` (new — text emitter per §9.2)
  - `core/src/report/flamegraph/render/json.rs` (new — `FlameGraphEnvelope`)
- **Tests:** `svg_smoke_is_valid_xml`, `svg_byte_size_in_expected_range`, `json_envelope_round_trips`, plus a folded-stack snapshot test.
- **Out of scope:** CLI.
- **Target size:** ~250 LOC + ~150 LOC tests + Cargo.toml diff.

### Slice M7-3.D — CLI `flamegraph` subcommand + integration tests + mode enforcement

- **Files affected:**
  - `cli/src/main.rs` (new `Flamegraph(FlameGraphArgs)` variant, `FlameRoot` and `FlameFormat` CLI enums, handler that wires mode resolution → `analyze_heap_with_graph` → collapser → budget → renderer → file write → exit code)
  - `cli/tests/integration.rs` (add the six CLI tests in §14)
- **Tests:** all CLI integration tests in §14.
- **Out of scope:** none — this slice closes M7-3.
- **Target size:** ~250 LOC + ~250 LOC tests.

After Slice D, M7-3 is complete. Documentation Sync should run an impact-driven pass against `STATUS.md`, `CHANGELOG.md`, `docs/roadmap.md` (mark M7-3 complete; advance next-action to M7-4), `README.md` (CLI surface table), and `docs/user-guide.md` (new `flamegraph` section).

## 16. Dependencies

`inferno` is the one new runtime dependency. Slice C must verify and document:

1. **License tree.** Run `cargo tree --duplicates` and `cargo deny check licenses` (if configured; otherwise manual scan). Confirm CDDL-1.0 plus transitive licenses are all on Mnemosyne's allowlist (Apache-2.0 / MIT / BSD-2/3 / ISC / Zlib / Unicode-DFS-2016 / CDDL-1.0).
2. **MSRV.** Mnemosyne's MSRV is the Rust workspace `rust-version` value. `inferno`'s current MSRV is 1.70 at the time of writing; if the workspace MSRV is newer or older, `cargo check --locked` will surface the conflict.
3. **Binary size.** `cargo build --release -p mnemosyne-cli` size delta is measured before/after; >10% growth must be flagged in the PR description and approved.
4. **Default features.** Pin to `default-features = false, features = ["nameattr"]` to avoid pulling in CLI-binary helpers we do not need.

If any of (1)–(3) fails, switch to the hand-rolled fallback (§10.1) and document the reason in `CHANGELOG.md`.

## 17. Risks and open questions

| # | Risk / question | Mitigation / current answer |
|---|---|---|
| R1 | `inferno` license tree (CDDL-1.0 + transitive deps) incompatible with Mnemosyne's MIT/Apache-2.0 dual license | Slice C adds explicit license verification (§16). If a real conflict surfaces, switch to the hand-rolled SVG renderer (§10.1); the renderer interface is already abstracted so this is a one-file change. |
| R2 | Flame-graph determinism (frame order, sibling sort) | All collapsers sort children by `(retained_size desc, name asc)`; folded-stack text sorts lines by `(weight desc, joined frames asc)`; JSON envelope serializes `stacks` in the same order. Tests `folded_text_is_deterministic` and the snapshot test in Slice C lock this. |
| R3 | `--max-frames` truncation policy: which frames are dropped first | Lowest-weight frames first, stable tie-break by joined frame string. The truncated weight is **always** preserved by aggregating into the `<other>` bucket so the SVG total area equals `total_weight`. Documented in §7.4 and §9.3, tested in `max_frames_truncates_lowest_weight_first`. |
| R4 | `gc-root-path` strategy on very deep dominator trees / cycles | Per-seed `visited: HashSet<ObjectId>` guarantees BFS termination; hard depth cap of 64 frames per path with `<…elided N…>` filler keeps even pathological inputs renderable. Documented in §7.3, tested in `collapse_gc_root_path_cycle_terminates`. |
| R5 | Provenance footer: should the SVG itself include a machine-readable provenance block? | **Yes.** Embed in `<desc>` (machine) + bottom-right corner text (human). Same string in JSON envelope's `provenance` vector. Implemented in Slice C; spec in §13. |
| Q1 | Should flame graphs be emitted from MCP as well as CLI? | **Defer to a follow-up batch.** The data shape (`FlameGraphEnvelope`) is MCP-ready; only wiring is missing. Tracked as a v0.3.x improvement, not a v0.3.0 release blocker. |
| Q2 | Should the `class-hierarchy` strategy weight by **retained** bytes instead of shallow bytes when the dominator tree is available? | **No** for v1. Class-hierarchy retained-size is ill-defined (a class is not an object that retains anything; only its instances do). Sticking to "sum of shallow bytes of instances" keeps the semantic clean. The dominator strategy is the place to ask retention questions. |
| Q3 | Should `--root all` produce three SVGs in one invocation? | **Defer.** Easy to add later as a wrapper; not worth the CLI surface bloat in v1. |
| Q4 | Should the `gc-root-path` seed set be configurable (`--top-n`, `--leak-only`)? | **Defer.** v1 uses the policy in §7.3 (leaks ∪ top-N retained, capped). If real users push back, expose a flag in v0.3.x without breaking the schema. |

## 18. Cross-references

- Parent design: [milestone-7-production-readiness.md](milestone-7-production-readiness.md) §8 (framing) and §6 (M7-1 deep-mode requirements that this addendum builds on).
- Sibling addendum: [milestone-7-2-ci-regression-policies.md](milestone-7-2-ci-regression-policies.md) (M7-2 — establishes the structured-output and exit-code patterns reused here).
- Roadmap: [docs/roadmap.md §4](../roadmap.md) (M7-3 row).
- Existing analysis types consumed by collapsers and renderers:
  - [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `AnalyzeResponse`, `analyze_heap`, the new `analyze_heap_with_graph` (§6).
  - [core/src/analysis/mode.rs](../../core/src/analysis/mode.rs) — `AnalysisMode`, `resolve_mode`.
  - [core/src/graph/dominator.rs](../../core/src/graph/dominator.rs) — `DominatorTree`, `VIRTUAL_ROOT_ID`, `build_dominator_tree`, `dominated_by`, `retained_size`.
  - `core/src/hprof/mod.rs` — `ObjectGraph`, `HeapObject`, `LoadedClass` (super-chain), `GcRoot`.
  - `core/src/report/mod.rs` — to be extended with `pub mod flamegraph;`.
- CLI integration target: [cli/src/main.rs](../../cli/src/main.rs).
- External dependency: [`inferno`](https://crates.io/crates/inferno) — folded-stack → SVG renderer.

## 19. Implementation readiness verdict

**READY** — this addendum is implementation-depth. The Implementation Agent may proceed with **Slice M7-3.A (folded-stack format types + dominator collapser + tests)** as the first task. Slices B → D are gated behind their predecessors and must each end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean before handing off to the next slice.
