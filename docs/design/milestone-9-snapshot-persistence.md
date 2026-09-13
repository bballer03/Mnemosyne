# Milestone 9 — Snapshot Persistence & Parse-Once-Query-Many

> **Status:** ✅ **Shipped** — Slices 9.A–9.D (implementation) and 9.E (this doc-sync pass) all complete. See §16 for the closeout note.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M9
> **Predecessors:** M8 (Reachability & References Deep Dive) ✅ shipped — this milestone's on-disk snapshot format must embed M8's new analyzer outputs (`ReferrerReport`, thread `FrameLocal`s, dominator context for `inspect`) so the format doesn't need re-versioning immediately after M9 ships, per roadmap.md's explicit sequencing rationale.
> **Last updated:** 2026-04-27

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M9 |
| Type | Parity-closing (biggest single UX gap vs MAT's `.index` files for repeat-triage workflows) |
| Touched crates | `core`, `cli`. No `tauri`/`ui` change (UI surfacing deferred). |
| Test count target | +30 to +40 net new Rust tests (workspace currently at 566 → target ≥ 600). |
| Memory budget | Snapshot save/load must not exceed the peak RSS of a normal deep-mode `analyze` run — serialization is a streaming write, not a second full copy held in memory alongside the live graph. |

## 2. Objective

After M9, a Mnemosyne user who has already parsed a heap dump once does not pay the parse cost again for the next five commands they run against it:

1. **"Re-open this heap instantly instead of re-parsing it."**
   `mnemosyne analyze --snapshot <path-or-hash>` (or auto-discovered by HPROF SHA-256) skips the binary-parser pass entirely and deserializes a cached `ObjectGraph` + `DominatorTree` in well under a second, versus 10+ seconds of cold binary parsing on a multi-hundred-MB dump.
2. **"Explicitly manage my snapshot cache."**
   `mnemosyne snapshot save|load|list|rm` gives the user direct control instead of relying on implicit auto-caching alone.
3. **"Know when a cached snapshot is stale or incompatible, loudly."**
   A schema-version mismatch or a heap file that no longer matches its cached SHA-256 fails with a clear, structured error and a suggested fix (`--refresh` / `snapshot rm`), never a silent re-parse that hides the fact the cache was cold, and never a silent load of stale/wrong data.

Today, `mnemosyne analyze`, `gc-path`, `inspect`, `analyze --by-referrer`, and every other deep-mode command re-parses the full HPROF binary from scratch on every single invocation, even when the same file was just analyzed seconds ago. This is the single biggest repeat-workflow UX gap versus Eclipse MAT's `.index` artifacts, called out explicitly in roadmap.md's MAT parity matrix (§2) and confirmed in ARCHITECTURE.md's "Future Extension Points" as still-open. M9 closes it.

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [core/src/hprof/object_graph.rs](../../core/src/hprof/object_graph.rs) — `ObjectGraph` and every type it's built from (`HeapObject`, `ClassInfo`, `GcRoot`, `StackTrace`, `StackFrame`, field descriptors) **already derive `Serialize, Deserialize`** (confirmed: 10+ `#[derive(..., Serialize, Deserialize)]` occurrences across the file). This means the single most expensive artifact to reconstruct — the parsed object graph — is already serde-round-trippable with zero new derive work. This was not obviously true going in and materially reduces M9's risk.
- [core/src/graph/dominator.rs](../../core/src/graph/dominator.rs) — `DominatorTree` does **not** currently derive `Serialize`/`Deserialize`. Its three private fields (`immediate_dominators: HashMap<ObjectId, ObjectId>`, `dominated_children: HashMap<ObjectId, Vec<ObjectId>>`, `retained_sizes: HashMap<ObjectId, u64>`) are all serde-friendly primitive/collection types — adding the derives (or a small serializable mirror struct if the fields must stay private) is mechanical, not a redesign. Scoped as Slice 9.A's first concrete task.
- [core/src/mcp/session.rs](../../core/src/mcp/session.rs) — `McpSessionStore` already implements the exact persistence shape M9 needs: a `root: PathBuf` directory, `save()`/`load()` via `serde_json::to_vec_pretty`/`from_slice`, and — critically — an **atomic write pattern**: write to `<id>.tmp`, then `replace_session_file(&temp, &target)` (a rename-based swap) so a crash mid-write never corrupts the target file. M9's `SnapshotStore` reuses this exact pattern rather than inventing a new one. `PersistedAiSession` also demonstrates the established `session_version: u32` schema-versioning convention M9's `SnapshotManifest` follows.
- [core/src/mcp/server.rs:611](../../core/src/mcp/server.rs) and [cli/src/config_loader.rs:2](../../cli/src/config_loader.rs) — `dirs::data_local_dir()` / `dirs::config_dir()` are already the established cross-platform base-directory pattern in this codebase (the `dirs` crate is already a workspace dependency per `Cargo.toml`). M9's default cache root (`~/.cache/mnemosyne/` per roadmap.md's M9 scope text) resolves via `dirs::cache_dir()` (same crate, correct semantic — `cache_dir` not `data_local_dir`, since a snapshot cache is reconstructible/disposable, unlike AI session history) with a documented override.
- [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `analyze_heap_with_graph()` (added in M7-3) already gives deep-mode callers a stable way to retrieve `(AnalyzeResponse, ObjectGraph, DominatorTree)` together without widening the serialized `AnalyzeResponse` contract — this is the exact seam M9's snapshot-save path hooks into: save happens *after* this call succeeds, serializing the `ObjectGraph`/`DominatorTree` it returned rather than re-deriving them.
- [core/src/hprof/binary_parser.rs](../../core/src/hprof/binary_parser.rs) — the parse entry point every deep-mode command currently calls unconditionally. M9 does not change this function; it adds a *earlier* short-circuit (check snapshot cache first) in each CLI/MCP call site, following the same shape M7-1's `auto|deep|overview` mode resolution already established as a pre-parse decision point.
- No existing hashing utility for heap-file identity was found reused elsewhere for this exact purpose (`sha2` is already a workspace dependency, used for hashed audit logging in `core::llm`'s privacy path) — M9 reuses the same `sha2` dependency, not a new one.

### 3.2 What MAT does

Eclipse MAT's `ParseHeapDump.sh` batch-indexes a `.hprof` into a set of `.index` files (class index, object index, dominator index, GC-roots index, etc.) stored alongside the original dump. Re-opening the same dump in the MAT UI is near-instant because it loads those indexes instead of re-parsing. MAT does not version its index format explicitly across releases in a way third-party tools rely on — this is arguably a weakness M9 improves on by making schema-version mismatches structured and loud rather than an opaque MAT re-index prompt.

M9 does not replicate MAT's *multiple specialized index files per dump* architecture. It ships **one versioned snapshot file per dump** containing the serialized `ObjectGraph` + `DominatorTree` (+ optionally precomputed M8 analyzer outputs, see §6.2) — simpler, and sufficient for Mnemosyne's actual re-open bottleneck (the binary parse + dominator computation, not per-query index lookups, since `get_references`/`get_referrers`/`immediate_dominator` are already O(1) once the graph is in memory).

### 3.3 The central technical decision: what exactly gets cached, and keyed by what

**Cache key:** SHA-256 of the heap dump file's bytes (reuses `sha2`, already a dependency). Not the file path — the same logical dump copied to a different path must hit the same cache entry; a different dump that happens to share a stale path must not. This mirrors roadmap.md's M9 scope text (`~/.cache/mnemosyne/<heap-sha256>/`) exactly.

**Cache invalidation triggers (all loud, none silent):**
1. Schema version mismatch (`SnapshotManifest.schema_version` older/newer than the running binary's `SNAPSHOT_SCHEMA_VERSION`) → structured `snapshot_schema_mismatch` error with the cached version, the running version, and a hint to `snapshot rm` or re-run with `--refresh`.
2. Heap file's current SHA-256 no longer matches the snapshot manifest's recorded hash (the file changed on disk since the snapshot was taken) → structured `snapshot_stale_source` error, same hint.
3. Snapshot file missing/corrupt (`serde_json`/bincode deserialize failure) → structured `snapshot_corrupt` error, same hint. Never falls back to a silent re-parse — the user must explicitly ask for that via `--refresh` or by not passing `--snapshot` at all.

**What is NOT cached:** `AiInsights` (AI output is a function of live provider state, not the heap; caching it risks staleness the user can't reason about), `ProvenanceMarker`s from a *specific* analysis run (regenerated fresh from the loaded graph so provenance always reflects the current invocation's actual code path, not a frozen historical one), MCP AI session state (already has its own persistence in `core::mcp::session`, unrelated concern).

### 3.4 Serialization format choice

JSON (`serde_json`), not a binary format (`bincode`/similar), for Slice 9.A–9.C, matching every other persistence surface already in this codebase (`McpSessionStore`, all report renderers). Rationale: this codebase has zero existing binary-serialization precedent, JSON keeps the snapshot format debuggable/inspectable (a user can `jq` a snapshot file to sanity-check it, which materially helps support/debugging), and the re-open win is dominated by *skipping the HPROF binary parse + dominator Lengauer-Tarjan pass*, not by JSON-vs-binary deserialization speed — `serde_json` deserializing a few hundred MB of already-structured data is still an order of magnitude faster than re-parsing raw HPROF binary records and rebuilding the dominator tree from scratch. If Slice 9.D's benchmark (§12) shows JSON deserialization itself becomes the bottleneck on very large graphs, a binary-format follow-up is scoped as out-of-milestone future work (§13), not retrofitted here — no premature optimization against an unmeasured target.

## 4. Scope

In:

1. New top-level module `core::snapshot` (sibling of `analysis`, `diff`, `policy`, `report`, matching this crate's existing domain-module layout). Contains `SnapshotManifest`, `SnapshotStore`, `save_snapshot()`, `load_snapshot()`, `list_snapshots()`, `remove_snapshot()`.
2. `SnapshotManifest { schema_version: u32, heap_sha256: String, heap_path: String, created_at: String, mnemosyne_version: String, object_count: usize, has_referrer_report: bool, has_field_data: bool }` — small header, cheap to read without deserializing the full payload (used by `snapshot list`).
3. Snapshot payload: `SnapshotPayload { manifest: SnapshotManifest, object_graph: ObjectGraph, dominator_tree: DominatorTree }`. `DominatorTree` gains `Serialize, Deserialize` derives (Slice 9.A prerequisite, mechanical per §3.1).
4. `core::graph::dominator` — `DominatorTree` derives extended; no algorithmic change.
5. CLI: `mnemosyne snapshot save <heap> [--output <dir>]`, `mnemosyne snapshot load <hash-or-path>`, `mnemosyne snapshot list`, `mnemosyne snapshot rm <hash>`. Additive `--snapshot <hash-or-path>` flag on `analyze`, `leaks`, `gc-path`, `inspect`, `query` (every command that currently calls the binary parser) — when passed, skip the parse and deserialize instead; when a bare heap path is given without `--snapshot` and a fresh matching snapshot already exists in the default cache dir, **auto-use it** (this is the actual parse-once-query-many win — explicit `--snapshot` is for power users pointing at a specific saved file/location, auto-discovery is the default ergonomic path most users get for free). `--refresh` forces a re-parse and overwrites the cache entry even when a valid snapshot is found.
6. MCP: `open_snapshot` (returns manifest + confirms it's loaded for the session), `list_snapshots`. Existing tools (`analyze_heap`, `parse_heap`, `gc_root_path`, `inspect_object`, `query_heap`) gain an additive `snapshot: string` param mirroring the CLI `--snapshot` flag.
7. Cache root resolution: `dirs::cache_dir()` (e.g. `~/.cache/mnemosyne/` on Linux, `~/Library/Caches/mnemosyne/` on macOS, `%LOCALAPPDATA%\mnemosyne\cache\` on Windows) joined with `<heap-sha256>/snapshot.json`, overridable via `MNEMOSYNE_SNAPSHOT_DIR` env var (mirrors the existing `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` env-override convention from M7-1) and/or a `[snapshot].directory` config key (mirrors `[ai.sessions].directory`'s existing override pattern in `core::mcp::session`).
8. Validation: round-trip tests (`ObjectGraph`/`DominatorTree` serialize → deserialize → identical query results for `get_references`/`get_referrers`/`retained_size`/etc.), cache-invalidation tests for all three triggers in §3.3, a benchmark comparing cold-parse time vs. snapshot-load time on the existing `medium` synthetic fixture.

Out:

- **No cross-machine snapshot interchange.** A snapshot is tied to the machine/Mnemosyne-version that produced it via `schema_version` + `mnemosyne_version` (recorded, not currently enforced beyond schema_version — see §11 R3). Portable snapshot exchange is explicitly deferred (not a goal even in future milestones per roadmap.md M9 scope).
- **No multi-snapshot session state beyond explicit `open_snapshot`.** M10-territory (M10 is already shipped and didn't need this; a future multi-snapshot workflow session belongs to M11's workflow suite, not here).
- **No AI insight caching.** Regenerated fresh every time, per §3.3.
- **No UI/Tauri surfacing.** CLI/MCP only, matching the M8/M10 precedent of shipping the backend surface first.
- **No privacy/redaction changes to the snapshot format itself in this milestone** — however, see §11 R2: snapshots can contain retained string contents when `retain_field_data` was used, so `[ai.privacy]`-style redaction awareness is a **documented caveat**, not new code, in Slice 9.A (full redaction-on-snapshot is out of scope; users handling sensitive dumps should be told snapshots inherit the same sensitivity as the source HPROF file and should be protected/deleted accordingly).
- **No streaming/overview-mode snapshot support.** Overview mode never builds an `ObjectGraph` (that's the entire point of M7-1); there's nothing to snapshot. `mnemosyne snapshot save` on an overview-resolved heap returns the same `feature_unavailable_in_overview_mode` structured error already established.

## 5. Architecture overview

```
 mnemosyne analyze <heap> [--snapshot <hash-or-path>] [--refresh]
                          │
                          ▼
        core::snapshot::resolve(heap_path, snapshot_arg, refresh)
          - explicit --snapshot: load that entry, error loudly if invalid
          - no --snapshot, not --refresh: hash the heap file, check cache dir
            for a fresh matching entry; load it if present, else fall through
          - --refresh or no cache hit: fall through to normal binary parse
                          │                              │
              (cache hit) │                              │ (cache miss / refresh)
                          ▼                              ▼
              SnapshotStore::load()          binary_parser::parse (existing, unchanged)
              → (ObjectGraph, DominatorTree)  → build_dominator_tree (existing, unchanged)
                          │                              │
                          │                   SnapshotStore::save() (write-through,
                          │                   best-effort: failure to write cache
                          │                   never fails the user's actual command)
                          │                              │
                          └──────────────┬───────────────┘
                                         ▼
                         analyze_heap_with_graph() / gc-path / inspect / query
                         (existing M7/M8/M10 code, unchanged — this is the seam,
                          not a new analysis path)


 mnemosyne snapshot save|load|list|rm
                          │
                          ▼
                core::snapshot::{save_snapshot, load_snapshot,
                                 list_snapshots, remove_snapshot}
                          │
                          ▼
                SnapshotStore (root: PathBuf, same atomic-write shape as
                McpSessionStore: write to .tmp, rename into place)
```

Module placement rationale: `core::snapshot` is a new top-level sibling, not nested inside `hprof` (it's a cross-cutting persistence concern over the *result* of parsing, not part of parsing itself) and not inside `mcp` (it must be usable from the CLI without any MCP session context, unlike `core::mcp::session`). This mirrors how `core::diff` and `core::policy` are top-level siblings rather than nested under `analysis`.

## 6. Data model

```rust
// core/src/snapshot/mod.rs

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub schema_version: u32,
    pub heap_sha256: String,
    pub heap_path: String,        // recorded for `snapshot list` display only; never trusted for identity, hash is
    pub created_at: String,       // RFC3339, mirrors PersistedAiSession's created_at convention
    pub mnemosyne_version: String,
    pub object_count: usize,
    pub has_field_data: bool,     // true when the graph was parsed with ParseOptions { retain_field_data: true }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub manifest: SnapshotManifest,
    pub object_graph: ObjectGraph,
    pub dominator_tree: DominatorTree,
}

pub struct SnapshotStore {
    root: PathBuf,   // same shape as McpSessionStore
}

impl SnapshotStore {
    pub fn new(root: PathBuf) -> Self { /* … */ }
    pub fn ensure_root(&self) -> CoreResult<()> { /* … */ }
    pub fn save(&self, heap_path: &str, graph: &ObjectGraph, dominator: &DominatorTree) -> CoreResult<SnapshotManifest> { /* … */ }
    pub fn load(&self, key: &str) -> CoreResult<SnapshotPayload> { /* key: sha256 hash OR direct file path to a .json snapshot */ }
    pub fn list(&self) -> CoreResult<Vec<SnapshotManifest>> { /* … */ }
    pub fn remove(&self, key: &str) -> CoreResult<()> { /* … */ }
    pub fn find_fresh_for_heap(&self, heap_path: &str) -> CoreResult<Option<SnapshotPayload>> {
        /* hash heap_path, check for a matching, schema-current entry; None (not an error) when absent */
    }
}
```

```rust
// core/src/graph/dominator.rs (extended — mechanical, no algorithm change)

#[derive(Debug, Clone, Serialize, Deserialize)]   // ADDED
pub struct DominatorTree {
    immediate_dominators: HashMap<ObjectId, ObjectId>,
    dominated_children: HashMap<ObjectId, Vec<ObjectId>>,
    retained_sizes: HashMap<ObjectId, u64>,
}
```

No changes to `AnalyzeResponse`, `HeapDiff`, `ObjectInspection`, or any other existing wire type — snapshot save/load is entirely a pre-analysis seam, invisible to every downstream contract already shipped in M1–M10. This is the cleanest possible integration: zero risk of the byte-identical-output regressions every prior milestone had to guard against, because nothing about *what* gets computed changes, only *whether the binary parse runs first*.

### 6.1 Error envelope

Reuses the established `error_details` pattern:

```json
{ "error": "snapshot_schema_mismatch",
  "detail": "cached snapshot schema_version=1, running binary expects schema_version=2",
  "hint": "run with --refresh, or `mnemosyne snapshot rm <hash>`" }
{ "error": "snapshot_stale_source",
  "detail": "heap file sha256 no longer matches the cached snapshot",
  "hint": "run with --refresh to re-parse and update the cache" }
{ "error": "snapshot_corrupt",
  "detail": "<serde error>",
  "hint": "run `mnemosyne snapshot rm <hash>` and re-run without --snapshot" }
{ "error": "snapshot_not_found",
  "detail": "no snapshot found for key '<key>'",
  "hint": "run `mnemosyne snapshot save <heap>` first, or `mnemosyne snapshot list`" }
```

### 6.2 Explicit non-goal within scope: precomputed M8 analyzer outputs

The predecessor note (top of doc) says the format "must embed M8's new analyzer outputs... so the format doesn't need re-versioning immediately." On inspection, this is **already satisfied without extra work**: `ReferrerReport`, `ObjectInspection`, and per-frame `FrameLocal`s are all *cheap, on-demand computations* over an already-loaded `(ObjectGraph, DominatorTree)` pair (§3.1 confirms `get_referrers`/`get_references` are O(1) lookups, `inspect_object` is O(1) plus O(F) field reads, `analyze_by_referrer` is a single O(objects) pass). None of them need to be *pre*-computed and stored in the snapshot payload — loading the graph is the expensive part M9 solves; re-deriving these reports from a loaded graph is already fast. Storing them anyway would bloat the snapshot file and create staleness risk (a stored `ReferrerReport` computed with `top_n=10` would need re-computation anyway if the user asks for `top_n=50` later). **Decision: `SnapshotPayload` stores only `ObjectGraph` + `DominatorTree`; every M8/M10 analyzer re-runs on top of the loaded graph exactly as it does today.** This is simpler than the roadmap's original phrasing implied and should be called out in Slice 9.E docs as the resolved design question.

## 7. CLI surface

```
mnemosyne snapshot save <HEAP>
    [--output <dir>]                   # override the default cache root for this save only

mnemosyne snapshot load <HASH_OR_PATH>
    # prints the manifest (schema version, object count, created_at, etc.) and confirms load succeeded;
    # does not itself run any analysis -- pairs with --snapshot on other commands

mnemosyne snapshot list
    # table: hash (short), heap path, object count, created_at, schema version

mnemosyne snapshot rm <HASH>

# Additive on analyze / leaks / gc-path / inspect / query:
mnemosyne analyze <HEAP> [--snapshot <HASH_OR_PATH>] [--refresh]
    # no --snapshot, no --refresh: auto-uses a fresh matching cache entry if present, else parses + writes cache
    # --snapshot <x>: load exactly that entry, error loudly if invalid (does not silently fall back to parsing)
    # --refresh: always re-parse, always overwrite the cache entry
```

Exit codes (extension of existing mapping, next free codes after M8's 8/9):

| Code | Meaning |
|---|---|
| 10 | `snapshot_not_found` — explicit `--snapshot <key>` / `snapshot load` key doesn't exist |
| 11 | `snapshot_schema_mismatch` |
| 12 | `snapshot_stale_source` |
| 13 | `snapshot_corrupt` |

Auto-discovery cache misses are **not** an error (codes 10-13 only apply to explicit `--snapshot`/`snapshot load` — auto-discovery silently falls through to a normal parse on any miss, since "no cache yet" is the expected first-run state, not a failure).

## 8. MCP surface

New tools:

```jsonc
{ "name": "open_snapshot",
  "input": [{ "name": "key", "type": "string", "required": true, "description": "SHA-256 hash or direct snapshot file path." }],
  "output_schema": "SnapshotManifest" }
{ "name": "list_snapshots",
  "input": [],
  "output_schema": "Vec<SnapshotManifest>" }
```

Existing tools gain an additive `snapshot: string` param (`analyze_heap`, `parse_heap`, `gc_root_path`, `inspect_object`, `query_heap`) — same additive-param precedent M8 Slice 8.A used for `gc_root_path`'s `all_paths`/`by_class`/`max_paths`, and M10 used for `diff_heaps`'s `mode`.

## 9. Sub-slice plan

All slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean, following the discipline every M7/M8/M10 slice already proved out.

### Slice 9.A — Serializable `DominatorTree` + `core::snapshot` core (save/load)

- **Scope:** Add `Serialize`/`Deserialize` to `DominatorTree`. New `core::snapshot` module: `SnapshotManifest`, `SnapshotPayload`, `SnapshotStore::{new, ensure_root, save, load}` (list/remove/find_fresh_for_heap deferred to 9.B). No CLI/MCP wiring yet — pure core round-trip.
- **Files owned:** `core/src/graph/dominator.rs` (derive only), `core/src/snapshot/mod.rs` (new), `core/tests/snapshot_round_trip.rs` (new).
- **Validation gates:** save → load round-trip produces byte-identical `get_references`/`get_referrers`/`retained_size`/`immediate_dominator` results to the pre-save graph on the `medium` synthetic fixture. Corrupt-file load returns `snapshot_corrupt`, not a panic. Existing `DominatorTree` tests (`core/src/graph/dominator.rs`'s own test module) pass unchanged.
- **Target size:** ~350 LOC + ~300 LOC tests.

### Slice 9.B — Cache-key resolution, listing, removal, staleness detection

- **Scope:** `SnapshotStore::{list, remove, find_fresh_for_heap}`. SHA-256 heap-file hashing (reuse `sha2`, already a dependency). Schema-version and source-hash staleness checks (§3.3, §6.1 error envelope).
- **Files owned:** `core/src/snapshot/mod.rs` (extend), `core/src/snapshot/hash.rs` (new, small — file-hashing helper), `core/tests/snapshot_staleness.rs` (new).
- **Validation gates:** three staleness triggers (§3.3) each produce their distinct structured error, none silently fall back. `find_fresh_for_heap` returns `None` (not an error) on first-run cache miss.
- **Target size:** ~250 LOC + ~250 LOC tests.

### Slice 9.C — CLI integration (`snapshot save|load|list|rm` + `--snapshot`/`--refresh` on existing commands)

- **Scope:** New `mnemosyne snapshot` subcommand family. `--snapshot`/`--refresh` flags on `analyze`, `leaks`, `gc-path`, `inspect`, `query`. Exit codes 10-13. Auto-discovery wiring (default cache root via `dirs::cache_dir()`, `MNEMOSYNE_SNAPSHOT_DIR` override).
- **Files owned:** `cli/src/main.rs` (new `Commands::Snapshot`, extend `AnalyzeArgs`/`LeaksArgs`/`GcPathArgs`/`InspectArgs`/`QueryArgs`), `cli/tests/snapshot_cli.rs` (new).
- **Validation gates:** `analyze <heap>` (no flags) on a fresh heap writes a cache entry; a second `analyze <heap>` run auto-uses it and completes without re-parsing (assert via a timing/marker test, not wall-clock — e.g. inject a way to assert the binary-parser codepath was NOT invoked). `--refresh` always re-parses. `--snapshot <bad-key>` returns exit 10, not a silent parse fallback.
- **Target size:** ~350 LOC + ~350 LOC tests.

### Slice 9.D — MCP integration + benchmark validation

- **Scope:** `open_snapshot`/`list_snapshots` MCP tools. Additive `snapshot` param on `analyze_heap`/`parse_heap`/`gc_root_path`/`inspect_object`/`query_heap`. Criterion bench comparing cold-parse vs. snapshot-load on the `medium` fixture (§12 budget).
- **Files owned:** `core/src/mcp/server.rs` (extend), `core/benches/snapshot_load.rs` (new).
- **Validation gates:** MCP round-trip test (save via CLI, load via MCP, confirm identical manifest). Bench confirms snapshot-load is materially faster than cold parse on `medium` (documented ratio, not a hard-coded pass/fail threshold — same "directional, host-local" caveat M10's Slice 8-1.G baseline doc used).
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 9.E — Documentation sync

- **Scope:** `docs/roadmap.md` (mark M9 shipped, parity matrix, scorecard, design-doc index), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md`, `docs/user-guide.md` (new `snapshot` section, `--snapshot`/`--refresh` on existing command docs), including the §6.2 resolved-design-question call-out (analyzer outputs are NOT pre-cached, computed on demand).
- **Files owned:** Documentation files only.
- **Validation gates:** Full-workspace `cargo {check,test,clippy,fmt}` still green. Docs cross-reference the design doc and each other consistently, matching the M8 Slice 8.E / M10 Slice H precedent.
- **Target size:** Documentation-only.

## 10. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | Snapshot write failure (disk full, permissions) silently degrades the user's actual command | Save is write-through and best-effort: a save failure is logged (`tracing::warn`) but never fails the enclosing `analyze`/`leaks`/etc. invocation — the user still gets their result, just without a cache entry written. |
| R2 | Snapshots inherit the same sensitive-data exposure as the source HPROF (retained strings, field values when `retain_field_data` was used) | Documented caveat (§4 "Out"), not new redaction code — `docs/user-guide.md` explicitly tells users to treat `~/.cache/mnemosyne/` with the same sensitivity as their heap dumps and to `snapshot rm` when done with a sensitive dump. |
| R3 | `mnemosyne_version` recorded but not enforced — a snapshot saved by v0.4.0 could be silently loaded by a v0.5.0 binary with a compatible `schema_version` but subtly different serialized field semantics | `schema_version` is the enforced contract, not `mnemosyne_version` (informational only) — any wire-incompatible change to `ObjectGraph`/`DominatorTree`'s serialized shape MUST bump `SNAPSHOT_SCHEMA_VERSION`, documented as a hard rule in the module's doc comment, same discipline the M9-triggering roadmap risk register already calls out ("Land M9 after M8 so the format includes the new analyzers from day one" — the corollary is: bump the version number aggressively rather than trying to keep old snapshots forever-compatible). |
| R4 | Disk-quota / unbounded cache growth (every distinct heap ever analyzed accumulates a cache entry forever) | Out of scope for this milestone (§4) but flagged: `snapshot list`/`snapshot rm` give the user manual control today; automatic LRU eviction is a documented future-work item, not silently implemented here (silent eviction would itself violate the honesty-first pattern this project holds everywhere else). |
| R5 | `find_fresh_for_heap`'s SHA-256 hashing adds overhead on every auto-discovery check, partially offsetting the win on very large dumps | Hashing is I/O-bound and still far cheaper than a full HPROF binary parse + dominator computation (streaming SHA-256 vs. full structural parse); if Slice 9.D's benchmark shows this assumption wrong on some dump shape, that's exactly what the benchmark is for — no premature optimization here. |

## 11. Test strategy

- Round-trip fidelity: `ObjectGraph`/`DominatorTree` serialize → deserialize → every existing query method (`get_object`, `get_references`, `get_referrers`, `immediate_dominator`, `dominated_by`, `retained_size`) returns identical results to the pre-serialize graph, on the existing `medium` synthetic fixture (reuse, not reinvent — same fixture M10's Slice 8-1.G baseline used).
- All three staleness triggers (§3.3) produce their distinct structured error and none silently falls back to a stale read.
- Regression boundary: every existing `analyze`/`leaks`/`gc-path`/`inspect`/`query` test (no `--snapshot` flag passed) must produce byte-identical output before/after this milestone — enforced the same way M8/M10 slices enforced it, since this milestone's entire design intent (§6) is "zero change to any downstream wire contract."
- Negative tests: corrupt snapshot file → `snapshot_corrupt`, not panic. Schema mismatch → `snapshot_schema_mismatch`. Modified source heap → `snapshot_stale_source`. Missing key → `snapshot_not_found` (exit 10).

## 12. Performance budget

Targets, measured on the existing `medium` synthetic fixture (~512 MB heap, ~3M objects), same measurement-host-caveat discipline as M10's Slice 8-1.G baseline doc (directional, host-local, not authoritative cross-host numbers):

| Metric | Cold parse (today) | Snapshot save (additional) | Snapshot load |
|---|---|---|---|
| Wall-clock | T₀ | ≤ 0.3 × T₀ (one streaming serialize pass) | documented, target ≤ 0.2 × T₀ |
| Peak RSS | R₀ | ≤ R₀ (streaming write, not a second in-memory copy) | ≤ R₀ (same graph size, just built from JSON instead of HPROF) |

No hard CI-red threshold in Slice 9.D — same "capture and document, don't gate on an unmeasured number" approach M10 used, with a follow-up to tighten into a hard gate once real numbers exist.

## 13. Out-of-scope (explicit non-goals)

- Cross-machine / cross-version snapshot portability guarantees beyond the recorded (but not fully enforced) `mnemosyne_version` field.
- Automatic cache eviction / disk-quota management (R4) — manual `snapshot rm` only.
- Binary (non-JSON) snapshot format — revisit only if Slice 9.D's benchmark shows JSON deserialization itself is the bottleneck (§3.4).
- Precomputed/cached M8 analyzer outputs inside the snapshot payload — resolved as unnecessary, see §6.2.
- Multi-snapshot session workflows (M11 territory).
- UI/Tauri surfacing.

## 14. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M9 backlog entry.
- Sibling design (slice-breakdown template, additive-only-seam discipline): [milestone-8-reachability-references.md](milestone-8-reachability-references.md) (M8), [milestone-8-1-object-level-diff.md](milestone-8-1-object-level-diff.md) (M10).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 9.E.
- Existing persistence precedent (atomic write, schema versioning): [core/src/mcp/session.rs](../../core/src/mcp/session.rs) (`McpSessionStore`, `PersistedAiSession`).
- Existing serializable graph: [core/src/hprof/object_graph.rs](../../core/src/hprof/object_graph.rs).
- Existing non-serializable target: [core/src/graph/dominator.rs](../../core/src/graph/dominator.rs) (`DominatorTree`).
- M7-3 seam this milestone reuses: [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) (`analyze_heap_with_graph()`).

## 15. Implementation readiness verdict

**READY** — this design doc is implementation-depth. The Implementation Agent may proceed with **Slice 9.A** (serializable `DominatorTree` + core save/load) as the first task. Slices 9.B → 9.D are sequential (each builds on the prior slice's `core::snapshot` surface; 9.C and 9.D both touch files 9.B doesn't, so once 9.B lands they *could* run in parallel, but 9.D's MCP wiring is small enough that sequential is simpler and lower-risk given both eventually touch shared infrastructure). Slice 9.E is gated behind 9.A–9.D landing.

## 16. Slice 9.E closeout (documentation sync)

Slices 9.A–9.D landed as: `b64f1fa` (9.A, serializable `DominatorTree` + core `SnapshotStore::{new,ensure_root,save,load}`), `165d2f4` (9.B, `SnapshotStore::{list,remove,find_fresh_for_heap}` + SHA-256 staleness detection), `31564a1` (9.C, `mnemosyne snapshot save|load|list|rm` CLI + `--snapshot`/`--refresh` on `analyze`/`leaks`/`gc-path`/`inspect`/`query` + exit codes 10-13, plus the new `SnapshotStore::load_checked` method for loud explicit-key staleness), and `369265d` (9.D, MCP `open_snapshot`/`list_snapshots` + additive `snapshot` param on `analyze_heap`/`parse_heap`/`find_gc_path`/`inspect_object`/`query_heap`). Slice 9.E (this pass) updated: `docs/roadmap.md` (M9 rows marked shipped across the MAT parity matrix §2, the M9 milestone entry §5, the scorecard §7, and the design-doc index §9), `STATUS.md` (new M9 snapshot bullet plus two capability-checklist rows for the cache core and the MCP surface), `CHANGELOG.md` (`[Unreleased]` entry below the existing M8/M10 entries), `README.md` (Key Features bullet, CLI command list, MCP method list — also reconciling a pre-existing gap where `diff_heaps` had never been added to README's/the user guide's MCP method lists during M10's own doc-sync), `ARCHITECTURE.md` (new `core::snapshot` module in the project-structure tree and "Shipped today" bullets, `core::graph::dominator`'s new derives, `core::mcp::session` reuse note), and `docs/user-guide.md` (new `snapshot` subcommand reference section; `--snapshot`/`--refresh` and exit codes 10-13 added to the `analyze`/`leaks`/`gc-path`/`inspect`/`query` sections; MCP section extended with `open_snapshot`/`list_snapshots`/`diff_heaps`).

**Scope drift found during doc-sync (this design doc's own speculative text vs. what actually shipped):**

- **On-disk layout is flat, not nested.** §4 point 7 and §7's architecture-overview prose describe `~/.cache/mnemosyne/<heap-sha256>/snapshot.json` (a per-hash subdirectory). What shipped (`core/src/snapshot/mod.rs`'s `SnapshotStore::path_for`) is a flat `<cache-root>/<heap-sha256>.json` file directly under the store root — simpler, and equally sufficient since the manifest is embedded in the same file rather than split out. Every other design-doc claim about the cache root itself (`dirs::cache_dir()/mnemosyne`, `MNEMOSYNE_SNAPSHOT_DIR` override) shipped exactly as specified.
- **No `[snapshot].directory` config-key override.** §4 point 7 speculated an env var *and* a config-key override (mirroring `[ai.sessions].directory`). Only the `MNEMOSYNE_SNAPSHOT_DIR` environment variable shipped; there is no `[snapshot]` TOML config section. A config-key override remains a plausible, low-risk future-work item if evidence justifies it.
- **No `core/src/snapshot/hash.rs`.** §9's Slice 9.B "Files owned" list speculated a dedicated file-hashing module. The SHA-256 helper (`sha256_hex`) shipped instead as a small private function directly inside `core/src/snapshot/mod.rs` — one file's worth of logic didn't justify a second module.
- **`created_at` is epoch-seconds, not RFC3339.** The §6 data-model code comment claims `created_at` is `"RFC3339, mirrors PersistedAiSession's created_at convention"`. What actually shipped reuses `crate::mcp::session::timestamp_now()` verbatim, which returns Unix epoch seconds as a string — the *same* convention `PersistedAiSession::created_at` already uses, just not RFC3339 (this workspace has no RFC3339-formatting dependency, and Slice 9.A correctly declined to add one just for this field). The design doc's inline comment was simply wrong about the format; the module doc comment in `core/src/snapshot/mod.rs` documents the actual, correct rationale.
- **`SnapshotStore::load_checked` is a new method beyond the original §6 sketch.** The original data model showed only `new`/`ensure_root`/`save`/`load`/`list`/`remove`/`find_fresh_for_heap`. Slice 9.C added `load_checked(key, heap_path)` to hold the *loud* staleness checks (`snapshot_schema_mismatch`/`snapshot_stale_source`) needed by explicit `--snapshot <key>`/MCP `snapshot` param call sites, kept separate from silent `load()` (corruption/not-found only) and silent `find_fresh_for_heap()` (any staleness = `Ok(None)`). This is the resolution of an open question the module's own doc comments flag explicitly as a Slice 9.B-vs-9.C design decision, not an oversight — see `core/src/snapshot/mod.rs`'s module-level "Design note: where staleness checking lives" comment.
- **`parse_heap`'s snapshot response is a distinct partial shape, not a `HeapSummary`.** Neither §6 nor §8 spelled this out explicitly, but it follows directly from §6.2's decision to cache only `ObjectGraph`/`DominatorTree`: `parse_heap`'s normal `HeapSummary` is derived from a raw byte-level HPROF record-tag scan that a cached snapshot doesn't retain. Slice 9.D's shipped resolution (`core/src/mcp/server.rs::serialize_snapshot_summary`) returns a distinctly-shaped object — manifest fields plus an honestly-computed `total_shallow_size_bytes`, carrying a `ProvenanceKind::Partial` marker — rather than either re-reading the source file (defeating the point of `--snapshot`) or faking a `HeapSummary`-shaped response from different underlying data. This is exactly the "parse_heap-in-snapshot-mode shape decision" the module's own doc comments call out for Slice 9.E to document; §6.2's core precedent (analyzer outputs stay on-demand, not precomputed) was upheld unchanged.
- **The Slice 9.D benchmark did not ship.** §9's Slice 9.D scope and §12's performance budget both call for `core/benches/snapshot_load.rs`, a Criterion bench comparing cold-parse vs. snapshot-load wall-clock. No such file exists in the shipped tree (`core/benches/` has `diff_object.rs`, `dominator_bench.rs`, `graph_bench.rs`, `parser_bench.rs` — no `snapshot_load.rs`). Round-trip *correctness* is fully covered (including via the MCP path, per Slice 9.D's validation gate), but the §12 performance claims (snapshot-load ≤ 0.2× cold-parse wall-clock) remain directional and unmeasured, not benchmark-confirmed. Flagged here as open follow-up rather than silently claimed as met.
- Everything else — the CLI subcommand/flag names, MCP tool/param names, the four structured error codes and CLI exit codes 10-13, the `SnapshotManifest`/`SnapshotPayload` field shapes (`has_referrer_report` was correctly *not* shipped, consistent with §6.2's resolution), and the §6.2 decision itself that M8 analyzer outputs are never precomputed into the snapshot — shipped exactly as this design doc specified.

Full-workspace verification at closeout: `cargo check --workspace --all-targets` clean, `cargo test --workspace` 687 passed / 0 failed, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean.
