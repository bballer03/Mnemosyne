# Milestone 8 — Reachability & References Deep Dive

> **Status:** 🔲 Pending — design authored 2026-04-27, awaiting Implementation Agent pickup of Slice 8.A.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M8, "Recommended Next Milestone" per §6
> **Predecessors:** M7 (shipped, all graph/dominator/policy infra); M10 object-level diff (🟡 mostly shipped — see [milestone-8-1-object-level-diff.md](milestone-8-1-object-level-diff.md)), landed out of sequence but does not block M8.
> **Last updated:** 2026-04-27

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M8 |
| Type | Parity-closing (3 top MAT gaps) + differentiator-extending (every surface ships provenance/MCP/overview-aware errors) |
| Touched crates | `core`, `cli`. No `tauri` / `ui` change (UI surfacing deferred; see §11). |
| Test count target | +35 to +45 net new Rust tests (workspace currently at 466 → target ≥ 505). |
| Memory budget | All-paths enumeration and referrer aggregation must respect the same deep-mode-only, budget-capped discipline as `gc-path` and `flamegraph` (M7-3/M7-4 precedent). No new unbounded traversal. |

## 2. Objective

After M8, a Mnemosyne user can answer three questions MAT users take for granted and Mnemosyne users currently cannot:

1. **"Show me every path from this object back to a GC root, not just the shortest one."**
   `mnemosyne gc-path <heap> --object-id <id> --all-paths [--max-paths N]` — today's `gc-path` only returns the single shortest BFS path.
2. **"Show me every path from any live instance of class X back to a GC root."**
   `mnemosyne gc-path <heap> --by-class <class-name> [--max-paths N]` — today there is no by-class projection at all; the user must already know a specific object id.
3. **"Which objects hold the most incoming references, and from where?"**
   `mnemosyne analyze <heap> --by-referrer [--top-n N]` — MAT's "Group by referrer" / "Show objects by incoming references" is a top-3 MAT investigation workflow. `ObjectGraph::get_referrers()` already exists as a primitive; there is no aggregating analyzer or CLI/MCP surface on top of it.
4. **"Give me one focused view of a single object: fields, refs in/out, dominator context."**
   `mnemosyne inspect <heap> --object-id <id>` — the UI's Object Inspector already does this; there is no CLI/MCP equivalent for scripting or AI-agent use.
5. **"What local variables does this thread frame hold?"**
   `inspect_threads()` already reports stacks and per-thread retained bytes; it does not cross-reference `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` roots into per-frame local-variable listings.

Today none of these five are answerable. M8 closes all five using graph primitives that already exist (`ObjectGraph`, `DominatorTree`, `get_references`/`get_referrers`/`get_object`) — this is composition and new CLI/MCP surface area, not new graph algorithms, matching the "lowest design risk" rationale in roadmap.md §6.

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [core/src/graph/gc_path.rs](../../core/src/graph/gc_path.rs) — `GcPathRequest { heap_path, object_id, max_depth: Option<u32> }`, `GcPathNode`, `GcPathResult`, `find_gc_path()`. Triple fallback: full `ObjectGraph` BFS → budget-limited `GcGraph` → synthetic. Returns exactly one (shortest) path today.
- [core/src/graph/dominator.rs](../../core/src/graph/dominator.rs) — `DominatorTree::{immediate_dominator, dominated_by, retained_size, node_count, top_retained}`. No incoming-reference aggregation.
- [core/src/hprof/object_graph.rs](../../core/src/hprof/object_graph.rs) — `ObjectGraph::{get_object(id), get_references(id) -> Vec<ObjectId>, get_referrers(id) -> Vec<ObjectId>}` (lines 403/408/417). `get_referrers` is O(1) lookup against a precomputed reverse-reference index built during parse — confirmed cheap enough to aggregate over the whole graph without a second graph pass.
- [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `AnalyzeRequest` / `AnalyzeResponse` already follow an additive-optional-field pattern (`histogram`, `unreachable`, `thread_report`, `classloader_report`, `collection_report`, `string_report`, `top_instances`, each `#[serde(default, skip_serializing_if = "Option::is_none")]`). `build_reference_chain(dom, graph, obj_id)` (line 696) already exists and is reused by both leak-suspect ranking and M10's object-diff reference chains — M8 reuses it a third time rather than duplicating.
- [core/src/analysis/thread.rs](../../core/src/analysis/thread.rs) — `ThreadInfo`, `ThreadReport`, `inspect_threads()`. Stack frames are parsed from `STACK_TRACE`/`STACK_FRAME` records; no `ROOT_JAVA_FRAME`/`ROOT_JNI_LOCAL` cross-reference into per-frame locals yet.
- [cli/src/main.rs](../../cli/src/main.rs) — `GcPathArgs { heap, object_id, max_depth: Option<u32> }` (line 228, additive extension point). `AnalyzeArgs` already has a large additive flag set (`--group-by`, `--threads`, `--strings`, `--collections`, `--top-instances`, `--top-n`, `--min-capacity`) that `--by-referrer` slots into the same pattern.
- [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `list_tools` registry (additive; M10 added `diff_heaps` here the same way M8 adds `inspect_object`).

### 3.2 What MAT does

- **Merge shortest paths to GC roots** — per-class aggregation of *all* paths from every instance of a class, merged into one tree, so common ancestry collapses instead of repeating per-instance.
- **Group by referrer / "list objects" → "show objects by incoming references"** — inverts the usual outgoing-reference browse; ranks by retained size of the referring set.
- **Inspector** — field-level browse pane: values, refs in/out, dominator parent/children. GUI-only in MAT; Mnemosyne's UI already has this, M8 adds CLI/MCP.
- **Thread details → local variables** — MAT resolves `JNI local` and `Java frame` GC roots against the live stack to show which local variable in which frame holds a reference.

M8 explicitly does **not** replicate MAT's tree-merge UI (no interactive merge — same non-goal pattern as M10 §13); it ships the data (`all_paths`, `by_class` projection, `by_referrer` ranking) in text/JSON/TOON, matching the shipped-parity-not-interactive-parity pattern already established for M7-4 OQL and M10 diff.

## 4. Scope

In:

1. **`core::graph::gc_path` — all-paths + by-class.** New `AllPathsRequest { heap_path, object_id: Option<ObjectId>, by_class: Option<String>, max_paths: usize, max_depth: Option<u32> }` and `find_all_gc_paths()` reusing the existing `ObjectGraph` BFS machinery but enumerating a bounded frontier instead of stopping at the first hit. `by_class` resolves every live instance of the named class first, then runs all-paths per instance up to the same `max_paths` budget shared across the whole class (not per-instance — see §6 budget note).
2. **New module `core::analysis::referrers`.** `ReferrerEntry { object_id, class_name, retained_size, referrer_count, top_referrer_classes: Vec<(String, u64)> }`, `ReferrerReport`, `analyze_by_referrer()`. Ranks objects by `referrer_count` (primary) and retained size (tiebreak), reusing `get_referrers()` and the existing dominator retained-size lookup.
3. **New module `core::analysis::inspector`.** `ObjectInspection { object_id, class_name, shallow_size, retained_size, fields: Vec<FieldValueEntry>, references_out: Vec<ObjectId>, referrers_in: Vec<ObjectId>, dominator_parent: Option<ObjectId>, dominator_children: Vec<ObjectId> }`, `inspect_object()`. Thin composition over existing `ObjectGraph`/`DominatorTree` accessors plus the existing `read_all_fields()` typed field reader (requires `retain_field_data: true`, same opt-in pattern as string/collection analyzers).
4. **`core::analysis::thread` extension.** Add `frame_locals: Vec<FrameLocal>` to the existing per-frame type by cross-referencing `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` GC roots (already parsed into `ObjectGraph::gc_roots`, confirm during Slice 8.D) against the thread's stack frames by frame-number/thread-serial correlation.
5. **CLI additive flags:**
   - `gc-path` gains `--all-paths`, `--by-class <name>`, `--max-paths <n>` (default `20`).
   - `analyze` gains `--by-referrer` (reuses existing `--top-n`).
   - New subcommand `inspect <heap> --object-id <id> [--retain-field-data]`.
   - `analyze --threads` output gains frame-locals automatically when present (no new flag — additive field on existing report, matching the additive-field convention used everywhere else in this codebase).
6. **MCP additions:** `gc_root_path` gains optional `all_paths` / `by_class` params (existing tool, additive params — not a new tool, mirroring how M10 added `mode` to `diff_heaps` rather than a new tool name). New tool `inspect_object`.
7. **Reporting:** all-paths/by-class extend the existing `gc-path` text/JSON/TOON renderers with a `paths: Vec<...>` shape instead of a single `path`. `--by-referrer` extends the existing `analyze` renderers (same additive-section pattern as `--threads`/`--strings`). `inspect` gets its own small renderer family under `core::report::inspect` (mirrors `core::report::diff` and `core::report::flamegraph` placement precedent).
8. **Validation:** synthetic fixtures for known multi-path graphs (diamond-shaped reference graphs where two paths exist to the same GC root), a referrer-heavy fixture (one object with N referrers), and `retain_field_data`-gated inspector tests.

Out:

- **No custom IQuery-style plugin runtime.** Not M8 territory (roadmap B9, deferred).
- **No UI surfacing.** JSON envelopes are UI-ready (same commitment M10 made) but no `ui/` or `tauri/` code changes in this milestone.
- **No streaming/overview-mode support for any of the five features.** All five require deep mode on both/either dump; overview-mode calls return the established `feature_unavailable_in_overview_mode` structured error, same as M7-3/M7-4/M10.
- **No interactive path-tree merge visualization.** Data only.
- **No changes to `AnalyzeResponse`'s existing optional fields' semantics.** All additions are new optional fields, `#[serde(default, skip_serializing_if = ...)]`, so `--mode class`-equivalent default behavior (no new flags passed) stays byte-identical to pre-M8 output — same non-negotiable compatibility bar M10 held for `--mode class`.

## 5. Architecture overview

```
 mnemosyne gc-path <heap> --object-id <id> [--all-paths|--by-class <c>] [--max-paths N]
                          │
                          ▼
        core::graph::gc_path::find_all_gc_paths(AllPathsRequest)
          - by_class=Some → resolve live instances of class first
          - object_id=Some → single-object frontier
          - reuses existing ObjectGraph BFS; frontier capped at max_paths
          - falls back to today's find_gc_path() when neither flag set (unchanged path)
                          │
                          ▼
                 GcPathResult { paths: Vec<GcPathNode chain> }   (extends existing shape)


 mnemosyne analyze <heap> --by-referrer [--top-n N]
                          │
                          ▼
        core::analysis::referrers::analyze_by_referrer(graph, dom, top_n)
          - iterate object ids, get_referrers(id).len() → rank
          - top_referrer_classes: resolve referrer object → class name, group+count
                          │
                          ▼
                 ReferrerReport   (new optional field on AnalyzeResponse)


 mnemosyne inspect <heap> --object-id <id> [--retain-field-data]
                          │
                          ▼
        core::analysis::inspector::inspect_object(graph, dom, id, retain_field_data)
          - get_object / get_references / get_referrers / dominator lookups
          - read_all_fields() when retain_field_data
                          │
                          ▼
                 ObjectInspection   (new report type; core::report::inspect renderers)


 analyze --threads  (existing)
                          │
                          ▼
        core::analysis::thread::inspect_threads()  [extended]
          - cross-reference ROOT_JAVA_FRAME / ROOT_JNI_LOCAL against stack frames
                          │
                          ▼
                 ThreadReport.threads[].frames[].locals: Vec<FrameLocal>   (new field)
```

Module placement rationale: `core::analysis::referrers` and `core::analysis::inspector` are new siblings of `core::analysis::{thread, string_analysis, collection, top_instances}` — same domain, same additive-analyzer pattern used four times already (M3 Phase 2). `core::graph::gc_path` gets extended in place (not a new module) because all-paths/by-class are the same algorithm family as today's shortest-path, just with a wider frontier — splitting it would duplicate the BFS/fallback machinery for no benefit.

## 6. Data model

```rust
// core/src/graph/gc_path.rs (extended)

pub struct AllPathsRequest {
    pub heap_path: String,
    pub object_id: Option<String>,   // mutually exclusive with by_class
    pub by_class: Option<String>,
    pub max_paths: usize,            // default 20, shared budget across the whole request
    pub max_depth: Option<u32>,
}

pub struct GcPathResult {
    pub target: String,                    // unchanged field, now describes the query (object id or class name)
    pub paths: Vec<Vec<GcPathNode>>,        // CHANGED: was `path: Vec<GcPathNode>`; see migration note below
    pub truncated: bool,                    // true when max_paths capped enumeration
    pub provenance: Option<ProvenanceMarker>,
}
```

**Breaking-change note (flagged for Slice 8.A review):** today's `GcPathResult` has a single `path: Vec<GcPathNode>` field consumed by CLI, MCP `find_gc_path`, and any existing JSON/TOON consumers. Widening to `paths: Vec<Vec<GcPathNode>>` is a breaking rename, not additive — this violates the "no breaking change to existing defaults" bar every other M7/M10 surface held. **Resolution:** keep `path: Vec<GcPathNode>` untouched (always the first/shortest path, byte-identical to today) and add a new optional field `all_paths: Option<Vec<Vec<GcPathNode>>>` populated only when `--all-paths`/`--by-class` is requested. This is the same additive-option discipline as every other M8/M10 field. Slice 8.A must implement it this way, not the naive rename sketched in the architecture diagram above (diagram simplified for readability; §6 data model here is authoritative).

```rust
// core/src/analysis/referrers.rs (new)

pub struct ReferrerEntry {
    pub object_id: String,
    pub class_name: String,
    pub retained_size: Option<u64>,        // None when dominator tree unavailable
    pub referrer_count: usize,
    pub top_referrer_classes: Vec<(String, usize)>,  // up to 5, sorted by count desc
}

pub struct ReferrerReport {
    pub entries: Vec<ReferrerEntry>,       // top-N by referrer_count, tiebreak retained_size desc
    pub total_objects_considered: usize,
}
```

```rust
// core/src/analysis/inspector.rs (new)

pub struct FieldValueEntry {
    pub name: String,
    pub type_name: String,
    pub value: String,          // rendered via existing FieldValue Display, same as query engine's @toString path
}

pub struct ObjectInspection {
    pub object_id: String,
    pub class_name: String,
    pub shallow_size: u64,
    pub retained_size: Option<u64>,
    pub fields: Option<Vec<FieldValueEntry>>,   // None unless retain_field_data
    pub references_out: Vec<String>,
    pub referrers_in: Vec<String>,
    pub dominator_parent: Option<String>,
    pub dominator_children: Vec<String>,
}
```

```rust
// core/src/analysis/thread.rs (extended)

pub struct FrameLocal {
    pub variable_slot: u32,        // JVM local-variable slot index; no name available without debug info
    pub object_id: String,
    pub class_name: String,
    pub root_kind: FrameLocalRootKind,  // JavaFrame | JniLocal
}

// ThreadInfo.frames[i] gains: pub locals: Vec<FrameLocal>  (empty Vec, not Option — matches existing Vec-typed report fields)
```

`AnalyzeResponse` gains exactly one new optional field:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub referrer_report: Option<ReferrerReport>,        // NEW (Slice 8.B)
```

`inspect` is a standalone CLI subcommand / MCP tool, not folded into `AnalyzeResponse` — it targets one object, not a whole-heap report, matching the separation already established for `gc-path` and `query` (neither is nested inside `AnalyzeResponse` either).

### 6.1 Budget discipline

- All-paths enumeration is bounded by `max_paths` (default 20) as a **shared** budget across the whole request, not per-path-length — a request that finds 20 paths at depth 3 stops before exploring depth 4, and `truncated: true` is set. This mirrors M10's `MAX_OBJECT_DIFF_FINGERPRINTS` hard-cap-with-honest-flag pattern rather than silent truncation.
- `by_class` resolves candidate instances via a single graph scan (`O(objects)`, already paid by `ObjectGraph` construction) before running the same bounded BFS per instance, stopping the moment the shared `max_paths` budget is exhausted across all instances combined.
- `analyze_by_referrer()` is `O(objects)` — one pass to build the count, using the referrer index that `get_referrers()` already relies on (no new index construction). No additional memory budget concern; documented in Slice 8.B validation as within the existing `analyze_heap()` RSS envelope (Step 11 ratios).
- `inspect_object()` is `O(1)` graph lookups plus `O(F)` field reads when `retain_field_data` — same cost profile as the existing UI Object Inspector bridge calls (`get_references`/`get_referrers` are already used there).
- Frame-locals cross-reference is bounded by existing thread/stack-frame counts already parsed by `inspect_threads()` — no new unbounded structure.

## 7. CLI surface

```
mnemosyne gc-path <HEAP> --object-id <ID>
    [--all-paths]                      # NEW: populate all_paths instead of (in addition to) path
    [--by-class <CLASS_NAME>]          # NEW: mutually exclusive with --object-id as the target selector
    [--max-paths <n>]                  # NEW: default 20
    [--max-depth <n>]                  # existing

mnemosyne analyze <HEAP>
    [--by-referrer]                    # NEW: populate referrer_report
    # existing --group-by / --threads / --strings / --collections / --top-instances / --top-n / --min-capacity unchanged

mnemosyne inspect <HEAP> --object-id <ID>      # NEW subcommand
    [--retain-field-data]              # opt-in field values, mirrors diff's --retain-field-data flag naming
    [--format {text|json|toon}]        # default text, same additive pattern as diff
```

Exit codes (extension of today's mapping, next free codes after M10's 5/6/7):

| Code | Meaning |
|---|---|
| 0 | Success |
| 3 | Heap parse error |
| 5 | Overview-mode mismatch (existing code, reused — same semantic as M7-3/M7-4/M10) |
| 8 | `--object-id` not found in the heap (new — `inspect`, `gc-path --object-id`) |
| 9 | `--by-class` matches zero live instances (new — `gc-path --by-class`) |

Codes 8/9 are new and additive; they do not collide with 0/2/3/5/6/7 already in use.

## 8. MCP surface

`gc_root_path` (existing tool) gains optional params — **not a new tool**, matching the M10 precedent of adding `mode` to `diff_heaps` rather than registering `diff_objects`:

```jsonc
{
  "name": "gc_root_path",
  "input": [
    // ...existing params unchanged...
    { "name": "all_paths", "type": "boolean", "required": false,
      "description": "Return all paths (bounded by max_paths) instead of only the shortest." },
    { "name": "by_class", "type": "string", "required": false,
      "description": "Find all-paths for every live instance of this class instead of a single object_id." },
    { "name": "max_paths", "type": "number", "required": false, "description": "Default 20." }
  ]
}
```

New tool `inspect_object`:

```jsonc
{
  "name": "inspect_object",
  "description": "Field-level inspection of a single heap object: values, refs in/out, dominator context.",
  "input": [
    { "name": "heap_path", "type": "string", "required": true },
    { "name": "object_id", "type": "string", "required": true },
    { "name": "retain_field_data", "type": "boolean", "required": false }
  ],
  "output_schema": "ObjectInspection"
}
```

`analyze_heap` MCP method gains optional `by_referrer: boolean` param, populating `referrer_report` the same way `enable_threads`/`enable_strings` already work.

Error envelopes reuse the established `error_details` pattern: `object_id_not_found`, `class_has_no_live_instances`, `feature_unavailable_in_overview_mode`.

## 9. Output formats

`gc-path --all-paths` text output extends today's single-path print with a numbered list:

```
GC root paths for <object-id> (3 of 3 found, not truncated):
  Path 1 (depth 4): <root> -> Server.pool -> Pool.cache -> Cache.entries[12] -> <object-id>
  Path 2 (depth 5): <root> -> Registry.services -> ServiceA.handler -> ... -> <object-id>
  Path 3 (depth 6): ...
```

`analyze --by-referrer` extends the existing analyze report with a new section, same table style as `--group-by`:

```
Top referenced objects (by incoming reference count):
  Object              Class                    Referrers  Retained    Top referrer classes
  0x7f2a...            com.example.SharedCache  184        512.00 MB   ConnectionPool(120), RequestHandler(64)
```

`inspect` gets its own compact report (new, small renderer family):

```
Object <id>  (com.example.CacheEntry)
  Shallow: 48 B   Retained: 1.20 MB
  Dominator parent: 0x7f2a... (com.example.Cache)
  Dominator children: 3
  References out (2): 0x7f3b... (java.lang.String), 0x7f3c... (com.example.Key)
  Referrers in (1): 0x7f2a... (com.example.Cache)
  Fields (--retain-field-data only):
    key: com.example.Key = 0x7f3c...
    value: java.lang.String = "session-42a1"
```

JSON/TOON follow the same additive-field, deterministic-ordering discipline established in M10 §10.2/§10.3/§15.4 (sort keys documented per-slice; reuse `core::report::diff`'s TOON section-per-field convention as the template).

## 10. Sub-slice plan

All slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean, following the same discipline M10 slices A–G already proved out.

### Slice 8.A — All-paths + by-class GC root path

- **Scope:** Extend `core::graph::gc_path` with `AllPathsRequest`, `find_all_gc_paths()`, the additive `all_paths: Option<Vec<Vec<GcPathNode>>>` field on `GcPathResult` (existing `path` field untouched — see §6 migration note). CLI `--all-paths`/`--by-class`/`--max-paths` on `gc-path`. Exit codes 8/9.
- **Files owned:** `core/src/graph/gc_path.rs`, `cli/src/main.rs` (`GcPathArgs` extension + `handle_gc_path`), `core/tests/gc_path_all_paths.rs` (new), `cli/tests/gc_path_all_paths_cli.rs` (new).
- **Validation gates:** diamond-shaped synthetic fixture (two distinct paths to the same root) returns both paths; `--by-class` on a class with 3 live instances returns paths for all 3 (budget permitting); `max_paths` truncation sets `truncated: true` and does not panic; today's `gc-path` (no new flags) output is byte-identical to pre-M8.
- **Target size:** ~350 LOC + ~300 LOC tests.

### Slice 8.B — Group-by-referrer analyzer

- **Scope:** New `core::analysis::referrers` module, `ReferrerReport` on `AnalyzeResponse`, CLI `analyze --by-referrer`, MCP `analyze_heap` `by_referrer` param, text/JSON/TOON rendering via the existing `render_report` extension points.
- **Files owned:** `core/src/analysis/referrers.rs` (new), `core/src/analysis/engine.rs` (wire into `analyze_heap`), `core/src/analysis/mod.rs`, `core/src/report/renderer.rs` (extend), `cli/src/main.rs` (`--by-referrer` flag), `core/src/mcp/server.rs` (`by_referrer` param), `core/tests/analyze_by_referrer.rs` (new).
- **Validation gates:** referrer-heavy synthetic fixture (one object with N=50 referrers across 3 classes) ranks correctly; `top_referrer_classes` grouping matches an independently computed reference; `analyze` with no `--by-referrer` flag produces byte-identical output to pre-M8.
- **Target size:** ~300 LOC + ~250 LOC tests.

### Slice 8.C — Object inspector (CLI + MCP)

- **Scope:** New `core::analysis::inspector` module, `ObjectInspection` type, new `mnemosyne inspect` CLI subcommand, new MCP tool `inspect_object`, new `core::report::inspect` renderer family (text/JSON/TOON).
- **Files owned:** `core/src/analysis/inspector.rs` (new), `core/src/report/inspect/mod.rs` + `text.rs`/`json.rs`/`toon.rs` (new, mirrors `core/src/report/diff/` layout), `cli/src/main.rs` (`InspectArgs` + `handle_inspect` + `Commands::Inspect`), `core/src/mcp/server.rs` (`inspect_object` registration + handler), `core/tests/inspect_object.rs` (new), `cli/tests/inspect_cli.rs` (new).
- **Validation gates:** inspecting a known synthetic object returns correct shallow/retained size, refs in/out, dominator parent/children; `--retain-field-data` populates `fields`, absence leaves it `None`; unknown `--object-id` returns exit code 8 / `object_id_not_found`.
- **Target size:** ~400 LOC + ~350 LOC tests.

### Slice 8.D — Thread frame-locals

- **Scope:** Confirm `ROOT_JAVA_FRAME`/`ROOT_JNI_LOCAL` are present in `ObjectGraph::gc_roots` (verify during this slice — if the binary parser does not yet retain thread-serial/frame-number correlation metadata on those roots, this slice must add that first as a prerequisite, scoped here rather than deferred, since frame-locals is meaningless without it). Extend `ThreadInfo`/stack-frame types with `locals: Vec<FrameLocal>`, wire correlation logic into `inspect_threads()`.
- **Files owned:** `core/src/analysis/thread.rs` (extend), `core/src/hprof/object_graph.rs` (only if root-correlation metadata needs adding — verify first, may be a no-op), `core/tests/thread_frame_locals.rs` (new).
- **Validation gates:** synthetic thread fixture with a known local reference resolves to the correct `FrameLocal`; threads with no frame-local roots produce empty `Vec`, not an error; `analyze --threads` output for fixtures with no locals is unchanged from pre-M8 (regression check).
- **Target size:** ~250–400 LOC depending on root-correlation prerequisite + ~250 LOC tests.

### Slice 8.E — Documentation sync

- **Scope:** Update `docs/roadmap.md` (mark M8 shipped, MAT parity matrix, scorecard, design-doc index — same pattern as M10's Slice H), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md`, `docs/user-guide.md` (new `gc-path --all-paths`, `analyze --by-referrer`, `inspect` sections).
- **Files owned:** Documentation files only.
- **Validation gates:** Full-workspace `cargo {check,test,clippy,fmt}` still green after all slices; docs cross-reference the design doc and each other consistently.
- **Target size:** Documentation-only.

## 11. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | Widening `GcPathResult` breaks existing consumers | Additive `all_paths: Option<...>` field only; `path` untouched (§6 migration note is binding). |
| R2 | All-paths enumeration combinatorially explodes on cyclic/dense graphs | Shared `max_paths` budget across the whole request, not per-path; BFS frontier cap, not DFS with backtracking; `truncated: true` surfaced honestly. |
| R3 | `by_class` on a common class (e.g. `java.lang.String`) returns a flood of near-duplicate paths | Same `max_paths` budget applies across all instances combined; documented in CLI help that `by_class` on high-cardinality classes should be paired with a lower `--max-paths`. |
| R4 | Frame-locals prerequisite (root-correlation metadata) turns out to be missing from the binary parser, ballooning Slice 8.D scope | Verification step is first action in Slice 8.D, before any report-type work; if missing, the parser fix is scoped inside 8.D rather than silently deferred, keeping the milestone honest about total cost. |
| R5 | `inspect` duplicates logic already in the UI bridge / MCP `query_heap` `OBJECTS` projection | `inspect_object()` is intentionally a thin composition over the same `ObjectGraph`/`DominatorTree` accessors the UI bridge and query engine already call — no new graph-walking logic, only a new aggregation/formatting layer. |
| R6 | `referrer_report` field bloats `AnalyzeResponse` JSON size on heaps with millions of objects | `--top-n` cap (reuses existing default, e.g. 10) applies to `ReferrerReport.entries`; `total_objects_considered` is a count, not a full dump. |

## 12. Test strategy

- Diamond-shaped synthetic fixture (shared ancestor, two divergent paths, reconverge at target) for Slice 8.A.
- Referrer-heavy fixture (one hot object, N referrers across ≥3 classes) for Slice 8.B.
- Standard graph fixture reused from M1/M3 fixtures for Slice 8.C inspector correctness (field values, refs, dominator context already exercised by existing `object_graph` tests — reuse, don't reinvent).
- Thread fixture with at least one `ROOT_JAVA_FRAME`/`ROOT_JNI_LOCAL` for Slice 8.D.
- Regression boundary: existing `gc-path` (no new flags), `analyze` (no `--by-referrer`), and `analyze --threads` (pre-locals) tests must pass unchanged — enforced as exit criteria on Slices 8.A/8.B/8.D respectively, same discipline as M10 §15.2.

## 13. Out-of-scope (explicit non-goals)

- UI/Tauri surfacing (JSON is UI-ready; no `ui/`/`tauri/` code here).
- Overview-mode support for any of the five features (deep-mode-only, matching M7-3/M7-4/M10 precedent).
- Interactive path-tree merge visualization (MAT GUI-only feature; data-only here).
- Full OQL `inbounds`/`outbounds` traversal (deferred to M9-2 per roadmap.md §5 M8 scope note).
- Custom IQuery plugin runtime (B9, deferred).

## 14. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M8 backlog entry; §6 "Recommended Next Milestone" justification.
- Sibling design (slice-breakdown template, additive-field discipline): [milestone-8-1-object-level-diff.md](milestone-8-1-object-level-diff.md) (M10).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 8.E.
- Existing primitives: [core/src/graph/gc_path.rs](../../core/src/graph/gc_path.rs), [core/src/graph/dominator.rs](../../core/src/graph/dominator.rs), [core/src/hprof/object_graph.rs](../../core/src/hprof/object_graph.rs) (`get_object`/`get_references`/`get_referrers`), [core/src/analysis/thread.rs](../../core/src/analysis/thread.rs), [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) (`build_reference_chain`).
- MCP surface: [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `list_tools` registry.

## 15. Implementation readiness verdict

**READY** — this design doc is implementation-depth. The Implementation Agent may proceed with **Slice 8.A (all-paths + by-class GC root path)** as the first task. Slices 8.B–8.D are independent of each other and may run in parallel by different subagents (no shared-file overlap: 8.A touches `graph/gc_path.rs`, 8.B touches `analysis/referrers.rs` + `engine.rs`, 8.C touches `analysis/inspector.rs` + new `report/inspect/`, 8.D touches `analysis/thread.rs` — the only shared file is `cli/src/main.rs` and `core/src/mcp/server.rs`, which must be edited sequentially, one slice at a time, to avoid two writing agents on the same file per the repo's forbidden-parallel-edit rule). Slice 8.E is gated behind 8.A–8.D landing.
