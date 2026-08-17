# Mnemosyne Roadmap — Path to MAT

> **Last updated:** 2026-04-26 (post-v0.3.0 Tech PM refresh)
> **Owner:** Tech PM Agent  
> **Goal:** Reach Eclipse MAT-level analysis depth while extending Mnemosyne's structural differentiators (provenance, streaming overview, MCP, ci-check, single-binary distribution)
> **Historical archive:** [roadmap-archive.md](roadmap-archive.md)

Mnemosyne has closed M1 through M7 and shipped `v0.3.0` (2026-04-26) across GitHub Releases, GHCR, and Homebrew. The active roadmap is now a Tech PM **post-v0.3.0 refresh**: an honest MAT parity matrix, an explicit differentiator inventory, a backlog of candidate M8+ milestones, and a recommended next milestone for orchestration to schedule.

---

## Roadmap-Wide Invariants (Differentiating Factors — Non-Negotiable)

Every milestone proposal in this document MUST either close a parity gap that is justified by the MAT comparison in §3, or extend one of the differentiators below. These invariants survive across milestones and override scope-only parity arguments.

1. **Provenance-first analysis** — every output declares whether it is overview / partial / deep, with structured `ProvenanceMarker` data. The honesty contract is enforced everywhere — including new surfaces.
2. **Streaming overview mode** — sub-linear memory triage on multi-GB heaps; no full `ObjectGraph` allocation. Any new analyzer must consider an overview-compatible path or fail loudly with a structured `feature_unavailable_in_overview_mode` error.
3. **MCP-first integration** — every user-facing capability is reachable from the MCP server with structured JSON output, so AI agents and IDEs are first-class consumers, not an afterthought.
4. **CLI-first, automation-first** — JSON / TOON / JUnit / GitHub Actions output is a baseline; `ci-check` is the primary CI surface; no GUI dependency.
5. **Conventional Commits + verifiable releases** — every shipped feature traceable to commits, validation evidence, and reproducible benchmark artifacts.
6. **Open-core, Rust-native** — single binary, no JVM dependency, low overhead, fast startup. New dependencies must justify themselves.

---

## 1. Where We Are (v0.3.0 — Shipped 2026-04-26)

`v0.3.0` is the current published release. The core parser, object-graph analysis, dominator tree, UI, MCP server, AI integration, CI policy gate, allocation flame graphs, and targeted OQL slice are all shipped. The remaining question is no longer whether Mnemosyne works, but which gaps matter most on the path to a credible MAT alternative without compromising the differentiator invariants above.

| Area | Status | Key Metric |
|---|---|---|
| Parser | ✅ Production | ~2.25 GiB/s streaming, ~90 MiB/s binary |
| Analysis (deep) | ✅ Production | Dominator tree, retained sizes, MAT-style suspects, 8 analyzer surfaces |
| Streaming overview mode | ✅ Shipped | Bounded-memory triage; survives 6.47 GB WSL fixture in published partial run |
| CI regression policies | ✅ Shipped | `ci-check`: 10 predicates, 4 output formats, 5 exit codes |
| Allocation flame graphs | ✅ Shipped | `flamegraph`: 3 rooting strategies, SVG / folded-stack / JSON |
| OQL | 🟡 Targeted | `@retainedSize` / `@toString` / `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS`, `IS NULL` — narrow vs full MAT OQL |
| AI | ✅ Shipped | Rules / stub / provider modes, CLI `chat`, persisted MCP sessions, redaction + audit |
| MCP | ✅ Shipped | 14 methods, `list_tools`, `error_details`, session lifecycle, mode-aware |
| UI | ✅ Shipped | Browser-first React: triage / artifact / heap / leak workspace |
| Desktop | ⚠️ Scaffold | Tauri shell with native commands; no signed release artifacts |
| Distribution | ✅ Full | GitHub Releases (5 targets), GHCR, Homebrew, source |
| Testing | ✅ Solid | 448 Rust tests + UI suite |
| Scale credibility | 🟡 Partial | Deep validated ~2 GB; overview survives 6.47 GB on WSL; **native-Linux + MAT + 10 GiB rerun still pending (M7-5)** |

**Active roadmap rule:** M1–M6 are archived. M7 is shipped (6/6). M7-5 reference-workstation rerun is preserved as future credibility work. M8+ proposals below are **candidates** until orchestration commits a next active milestone.

## 2. MAT Parity Matrix (v0.3.0)

Honest comparison against Eclipse MAT capability dimensions. **MAT support:** ✅ Full / 🟡 Partial / ❌ N/A. **Mnemosyne v0.3.0:** ✅ Full / 🟡 Partial / ⏳ Designed / ❌ Missing.

| MAT capability | MAT | Mnemosyne v0.3.0 | Gap | Priority | Notes |
|---|---|---|---|---|---|
| HPROF parsing (1.0.1 / 1.0.2, classic + binary) | ✅ | ✅ | None | — | Mnemosyne's `core::hprof` parser handles classic + binary HPROF including `HEAP_DUMP_SEGMENT` (M1.5 fix). Real-world fixture validation passes. |
| Histogram (per class, instances + shallow + retained) | ✅ | ✅ | None | — | `analyze_heap()` ships per-class histograms with retained sizes when graph-backed; overview mode emits class-resolved shallow approximations with honest provenance labels. |
| Dominator tree (full + partial, sorted by retained) | ✅ | ✅ | None | — | `core::graph::dominator` (Lengauer-Tarjan) is the source of retained sizes. Available via `analyze_heap()`, `flamegraph`, and the new public `analyze_heap_with_graph()` entry point. |
| Path to GC roots — shortest path | ✅ | ✅ | None | — | `mnemosyne gc-path` uses `ObjectGraph` BFS first, then `GcGraph` budget fallback, then synthetic. |
| Path to GC roots — **all paths** / by class | ✅ | ✅ | None | — | `mnemosyne gc-path --all-paths [--by-class <name>] [--max-paths <n>]` enumerates every GC root path up to a shared `max_paths` budget (default 20), with an honest `truncated: bool` flag when the budget caps enumeration. Exit codes `8` (`--object-id` not found) / `9` (`--by-class` matches zero live instances). Shipped M8 Slice 8.A. |
| Leak suspects report (heuristic) | ✅ | ✅ | None | — | `detect_leaks()` ships graph-backed retained-size + accumulation-point ranking with heuristic fallback labeled via `ProvenanceKind::Fallback`. **Differentiator:** structured provenance markers vs MAT's opaque suspect text. |
| OQL — full operator set | ✅ | 🟡 | M7-4 covers ~30% of MAT OQL surface | Medium (M8 / M9) | Shipped: `@retainedSize`, `@toString`, `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS x.field`, `IS NULL`. Missing: subqueries, `UNION`, multi-hop traversal, full predicate functions, `eval(...)`, regex `=~`, `dominators(...)`, `outbounds`/`inbounds` traversal. |
| Top consumers report | ✅ | ✅ | None | — | `find_top_instances()` + analyze report top-N largest instances by retained or shallow size. |
| Class loader explorer (per-loader histogram, unique classes) | ✅ | ✅ | None | — | `analyze --classloaders` ships `ClassLoaderReport { loaders, potential_leaks, duplicate_classes }` — **two independent leak signals that coexist, neither replacing the other**: (1) `potential_leaks`, a single-loader "retains a lot, loads almost nothing else" heuristic (M3 Phase 3, CLI-rendered + tested as of M13's grounding fix), and (2) `duplicate_classes` (new, M13), MAT's actual "Duplicate Classes" cross-loader signal — a class name loaded by ≥2 distinct classloaders, the classic Tomcat/Jetty/Spring hot-redeploy pattern. Each `ClassLoaderInfo` also gains `unique_class_count` (computed, not yet CLI-rendered — see M13 design doc closeout) and `ancestor_chain` (bounded parent-loader walk, rendered as an "Ancestors" column). `ci-check classloader_leak_count` predicate and MCP `detect_classloader_leaks` tool both ship. Shipped M13 Slices 13.A–13.C (13.D is this doc-sync). |
| Duplicate strings / arrays detection | ✅ | 🟡 | Strings yes; arrays no | Low (M11) | `analyze_strings()` reports duplicate groups + dedup waste. **No** equivalent for primitive arrays or boxed array dedup. MAT has both. |
| Thread overview + frame-locals + stack | ✅ | ✅ | None | — | `inspect_threads()` now additionally cross-references `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` GC roots into per-frame `FrameLocal { variable_slot, object_id, class_name, root_kind }` entries, printed as `local:`/`jni-local:` lines under each stack frame in `analyze --threads` text output. `variable_slot` honestly reuses the HPROF frame number — HPROF frame-local roots carry no genuine bytecode slot index. Shipped M8 Slice 8.D. |
| Inspector — object field-level browse, refs in/out | ✅ | ✅ | None | — | `mnemosyne inspect <heap> --object-id <id> [--retain-field-data] [--format text\|json\|toon]` and MCP `inspect_object` ship a focused single-object view: shallow/retained size, dominator parent/children, references out, referrers in (all as structured `ObjectRef { object_id, class_name }`, not baked strings), and opt-in typed field values. Shipped M8 Slice 8.C. |
| Group by class / classloader / package / superclass | ✅ | 🟡 | Class / package / classloader yes; superclass no | Low (M11) | `--group-by class\|package\|classloader`. Missing `--group-by superclass` (and the related "group by class -> superclass tree"). |
| **Group by referrer** (incoming references analysis) | ✅ | ✅ | None | — | `mnemosyne analyze --by-referrer` and MCP `analyze_heap` `by_referrer: boolean` param rank objects by incoming-reference count (tiebreak retained size), with top referrer classes per entry. Shipped as a `ReferrerReport` optional field on `AnalyzeResponse` / an additive param on the existing `analyze_heap` tool — not a separate `analyze_by_referrer` tool (roadmap text below in §5 originally speculated otherwise). Shipped M8 Slice 8.B. |
| Reachable / unreachable objects analysis | ✅ | ✅ | None | — | `find_unreachable_objects()` walks from GC roots and reports per-class counts + shallow size. |
| **Compare two heap dumps** (object-level diff) | ✅ | 🟡 | `ci-check object_growth_threshold` predicate + leak-progression cross-reference missing | Low (M10-B) | `mnemosyne diff --mode object` ships fingerprint-based per-object identity (`class+retained` / `class+dominator` / `full-fingerprint`), added/removed/retained_changed sections, `MatchQuality`, and MCP `diff_heaps` mode. Missing: `ci-check object_growth_threshold` predicate, leak-progression cross-reference with `detect_leaks()`. |
| Allocation-site flame graphs | 🟡 | ✅ | **Mnemosyne ahead** | — | MAT has no native flame-graph export; users typically pipe to async-profiler. Mnemosyne ships SVG / folded-stack / JSON natively. **Differentiator.** |
| Custom inspector views / extensions | ✅ | ❌ | Not shipped | Defer | MAT plugins (`org.eclipse.mat.api.IQuery`) are widely used. Mnemosyne has a plugin design doc (M6) but no runtime extension surface. Defer until adoption justifies. |
| **Index files / persistent snapshot** (parse-once, query-many) | ✅ | ✅ | None | — | `mnemosyne snapshot save\|load\|list\|rm` plus additive `--snapshot <hash-or-path>`/`--refresh` on `analyze`/`leaks`/`gc-path`/`inspect`/`query` cache the parsed `ObjectGraph` + `DominatorTree` keyed by heap-file SHA-256 under `dirs::cache_dir()/mnemosyne` (override via `MNEMOSYNE_SNAPSHOT_DIR`); auto-discovery re-opens a fresh matching cache entry with no flags at all. Schema/staleness mismatches on an *explicit* `--snapshot`/`snapshot load` fail loudly with exit codes `10`-`13`; auto-discovery misses fall through to a normal parse, never silently. MCP gets `open_snapshot`/`list_snapshots` plus an additive `snapshot` param on `analyze_heap`/`parse_heap`/`find_gc_path`/`inspect_object`/`query_heap`. Shipped M9 Slices 9.A-9.D (9.E is this doc-sync). |
| **CI/CD-native automation** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has no first-class CI gate. `ci-check` + JSON / JUnit / GitHub Actions output is unique. **Differentiator.** |
| **Streaming bounded-memory mode** | 🟡 | ✅ | **Mnemosyne ahead** | — | MAT has `ParseHeapDump.sh` for batch indexing, but no truly streaming bounded-RSS triage on multi-GB dumps. **Differentiator.** |
| **AI-assisted diagnosis** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has none. Mnemosyne ships rules / stub / provider modes, prompt redaction, audit log, CLI `chat`, persisted MCP sessions. **Differentiator.** |
| **MCP-native IDE integration** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT is Eclipse-only. Mnemosyne ships 14 MCP methods with structured errors and session lifecycle. **Differentiator.** |
| **Provenance contract** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has no equivalent. `ProvenanceKind { Synthetic, Partial, Fallback, Placeholder }` rendered across all non-JSON formats. **Differentiator.** |
| **Single-binary distribution, no JVM** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT requires JVM + Eclipse RCP. Mnemosyne ships static binaries via 4 channels. **Differentiator.** |

### Parity matrix summary

- **Mnemosyne ≥ MAT:** allocation flame graphs, CI/CD automation, streaming bounded-memory mode, AI-assisted diagnosis, MCP/IDE integration, provenance, distribution.
- **MAT ≈ Mnemosyne (parity closed):** persistent indexes / parse-once-query-many, all-paths-to-GC-roots / by-class, group-by-referrer, object-level heap diff, and classloader leak detection (both the pre-existing single-loader heuristic and the new cross-loader duplicate-class signal) — closed in M9/M8/M10/M13, see below.
- **MAT ≥ Mnemosyne (medium priority):** full OQL depth.
- **MAT ≥ Mnemosyne (low priority / defer):** duplicate-arrays, group-by-superclass, custom plugin runtime.

---

## 3. Differentiator Inventory

Capabilities Mnemosyne has that MAT does not (or where MAT's offering is materially weaker). These are the strategic moat — every M8+ milestone is judged in part by whether it preserves or extends them.

| Mnemosyne capability | MAT equivalent | Differentiation | Strategic value |
|---|---|---|---|
| Streaming overview mode (sub-linear RSS) | `ParseHeapDump.sh` batch indexer | Mnemosyne emits useful triage output without ever building the object graph; bounded-memory by construction | **Critical.** Enables real-time CI use, container-friendly footprint, multi-GB triage on developer laptops |
| `ci-check` policy DSL | None | TOML-backed policy with 10 predicates, 4 output formats, 5 exit codes, mode-aware skip semantics | **Critical.** Defines a category MAT does not own (heap-regression-as-CI-gate) |
| MCP server (14 methods, sessions) | None | First-class structured tooling for AI agents and IDEs; `list_tools`, `error_details`, persisted sessions | **Critical.** Positions Mnemosyne as the canonical heap-analysis backend for AI-assisted triage |
| Provenance markers | None | Every output declares synthetic / partial / fallback / placeholder status; rendered in Text / Markdown / HTML / TOON | **High.** Honesty contract; differentiates from "trust-me" triage tools |
| AI integration (rules / stub / provider) | None | Provider mode with prompt redaction, hashed audit, prompt-budget guard, YAML templates, CLI `chat`, MCP sessions | **High.** AI-assisted triage with verifiable safety guarantees |
| Native CI/automation outputs (JSON / TOON / JUnit / GH Actions) | Limited | First-class structured outputs across analyze, ci-check, query, flamegraph | **High.** Automation moat; works with any CI/observability stack |
| Single-binary, no-JVM distribution | None (MAT requires Eclipse RCP) | Static binaries on 5 targets via GitHub Releases / GHCR / Homebrew | **High.** Container-friendly; serverless-friendly; CI-friendly |
| Allocation-site flame graphs (native SVG export) | None natively | `flamegraph` with 3 rooting strategies (dominator, class-hierarchy, gc-root-path), 3 formats | **Medium.** Categorically novel; pairs with profiler workflows |
| Reproducible benchmark artifacts | Partial | `scripts/bench/` harness publishes raw CSVs alongside reports; reference-spec discipline | **Medium.** Credibility moat for performance claims |
| Conventional Commits + verifiable releases | None | Every shipped feature traceable; release automation validates tag/version alignment | **Medium.** Auditability; supply-chain hygiene |

---

## 4. M7 — Production Readiness & Scale (✅ Shipped 2026-04-26)

**Design references:** [design/milestone-7-production-readiness.md](design/milestone-7-production-readiness.md), [design/milestone-7-1-streaming-overview-mode.md](design/milestone-7-1-streaming-overview-mode.md), [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md), [design/milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md), [design/milestone-7-4-oql-targeted-expansion.md](design/milestone-7-4-oql-targeted-expansion.md), [design/milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md), [design/milestone-7-6-v0-3-0-release.md](design/milestone-7-6-v0-3-0-release.md)

**Status:** ✅ **6 of 6 slices closed for `v0.3.0` release** (2026-04-26).

| # | Item | Status | Notes |
|---|---|---|---|
| M7-1 | Streaming overview mode | ✅ Shipped | `auto\|deep\|overview` across CLI/MCP/core; 4 GiB auto threshold |
| M7-2 | CI regression policies | ✅ Shipped | `ci-check`: 10 predicates, 4 formats, 5 exit codes |
| M7-3 | Allocation-site flame graphs | ✅ Shipped | `flamegraph`: SVG / folded-stack / JSON, 3 rooting strategies |
| M7-4 | OQL targeted expansion | ✅ Shipped | 6-feature parity slice; full MAT OQL deferred to M8/M9 |
| M7-5 | Comparative benchmarks | 🟡 Partial — preserved as future work | Published partial WSL run for `mnemo-overview` vs `hprof-slurp` on `small/medium/large`. **Pending:** native-Linux reference-workstation rerun with Eclipse MAT, `mnemo-deep`, equivalence, and the `10 GiB` fixture |
| M7-6 | v0.3.0 release | ✅ Shipped | Tag `v0.3.0`, 5 platform archives, GHCR `0.3.0/0.3/latest`, Homebrew SHA-256 bumped |

The M7-5 reference-workstation rerun is **preserved** as a credibility follow-up under M12 (see §5). It is no longer a release blocker but remains the primary unfilled credibility item for full MAT-comparison claims.

---

## 5. M8+ Candidate Milestones (Post-v0.3.0)

These are **candidates** for orchestration to schedule. Each closes a parity gap from §2 or extends a differentiator from §3 (or both). The recommended immediate next milestone is identified in §6.

### M8 — Reachability & References Deep Dive (Parity-Closing + Differentiator-Extending) — ✅ Shipped

- **Status:** ✅ **Shipped** — design doc [milestone-8-reachability-references.md](design/milestone-8-reachability-references.md), slices 8.A–8.D (implementation) + 8.E (documentation sync, this pass). All four target workflows landed.
- **Theme:** Close the highest-value MAT investigation gaps that the existing graph already supports.
- **Goal:** Ship the three top MAT-only investigation workflows — all-paths-to-GC-roots / by-class, group-by-referrer (incoming references), and a focused object inspector surface — using the existing `ObjectGraph` + dominator infrastructure with provenance and overview-aware fallbacks.
- **Delivered scope (as shipped, two naming/mechanism differences from the original text below — see note):**
  - `mnemosyne gc-path --all-paths [--by-class <class>] [--max-paths <n>, default 20]`: path enumeration bounded by a **shared** `max_paths` budget across the whole request, surfaced via a plain `truncated: bool` flag on `GcPathResult` (**not** `ProvenanceKind::Partial` markers as originally speculated below — the design doc's own §6 had already corrected this before implementation). New optional `all_paths: Option<Vec<Vec<GcPathNode>>>` field; the existing `path` field is untouched, so today's `gc-path` (no new flags) stays byte-identical. New CLI exit codes `8` (object id not found) / `9` (`--by-class` matches zero live instances).
  - New `core::analysis::referrers` module (`ReferrerEntry`, `ReferrerReport`, `analyze_by_referrer()`) + `mnemosyne analyze --by-referrer` + MCP `by_referrer: boolean` param **on the existing `analyze_heap` tool** (**not** a separate `analyze_by_referrer` tool as originally speculated below — same additive-param-not-new-tool pattern M10 used for `diff_heaps`). Ranks objects by incoming-reference count, tiebreak retained size, with up to 5 top referrer classes per entry.
  - `mnemosyne inspect <heap> --object-id <id> [--retain-field-data] [--format text|json|toon]` + new MCP tool `inspect_object`, backed by new `core::analysis::inspector` (`ObjectInspection`, `inspect_object()`) and a new `core::report::inspect` renderer family (mirrors `core::report::diff` placement). `ObjectInspection`'s `references_out` / `referrers_in` / `dominator_parent` / `dominator_children` ship as structured `ObjectRef { object_id, class_name }`, not baked `"{id} ({class})"` strings — changed during Slice 8.C review specifically so MCP/AI-agent consumers can chain calls without string-parsing.
  - Thread frame-locals: `core::analysis::thread` cross-references `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` GC roots into `FrameLocal { variable_slot, object_id, class_name, root_kind }` entries on each stack frame, printed as `local:` / `jni-local:` lines under each frame in `analyze --threads` text output. `variable_slot` honestly reuses the raw HPROF frame number rather than pretending to be a real bytecode local-variable slot index (HPROF frame-local roots carry no such index).
- **Out of scope:** Full MAT OQL `inbounds` / `outbounds` traversal (defer to M9-2). Custom IQuery plugin runtime. UI surfaces beyond what the existing leak workspace already covers.
- **Why now / strategic rationale:** These are top-3 MAT investigation workflows that Mnemosyne currently lacks despite having the graph primitives. They reuse M1 / M3 graph infrastructure with no new architectural risk. They directly improve leak-triage credibility, which is the project's core mission.
- **Success criteria (met):** `gc-path --all-paths` enumerates all paths under budget with honest truncation. `analyze --by-referrer` ranks referrers with regression coverage. `inspect <id>` exposes field values + refs in/out for any reachable object, with structured refs. Frame-locals appear on synthetic thread fixtures and are regression-gated against pre-M8 `analyze --threads` output. Slices 8.A–8.D landed with full-workspace `cargo {check,test,clippy,fmt}` clean at each gate; 566 tests passing at Slice 8.E closeout.
- **Risks / dependencies:** Path enumeration needs careful budget caps (combinatorial explosion) — mitigated with a shared `max_paths` budget, not per-path. Frame-locals depend on `STACK_TRACE` records being present in the dump. Overview-mode behavior surfaces a structured `feature_unavailable_in_overview_mode` error (deep-mode-only, matching M7-3/M7-4/M10 precedent).
- **Estimated slice count:** 5–10 slices (5 shipped: 8.A–8.D implementation + 8.E doc-sync).

### M9 — Snapshot Persistence & Parse-Once-Query-Many (Parity-Closing) — ✅ Shipped

- **Status:** ✅ **Shipped** — design doc [milestone-9-snapshot-persistence.md](design/milestone-9-snapshot-persistence.md), slices 9.A–9.D (implementation) + 9.E (documentation sync, this pass).
- **Theme:** Eliminate re-parse latency for repeat triage workflows.
- **Goal:** Persist a verified, versioned, on-disk snapshot index after the first parse so subsequent commands re-open instantly. Mnemosyne's first answer to MAT's `.index` artifacts.
- **Delivered scope (as shipped, differs from the original text below in several particulars — see note):**
  - New top-level `core::snapshot` module: `SnapshotManifest`, `SnapshotPayload { manifest, object_graph, dominator_tree }`, `SnapshotStore::{new, ensure_root, save, load, load_checked, list, remove, find_fresh_for_heap}`. `DominatorTree` gained `Serialize`/`Deserialize` derives (mechanical, no algorithm change). Cache entries are keyed by heap-file SHA-256 and stored as flat `<sha256>.json` files under the store root (**not** nested under `<heap-sha256>/snapshot.json` as originally speculated below — a flat layout was simpler and equally sufficient). **Only** `ObjectGraph` + `DominatorTree` are cached — M8's analyzer outputs (`ReferrerReport`, `ObjectInspection`, `FrameLocal`s) are deliberately **not** precomputed/stored; they stay cheap on-demand computations over the loaded graph, resolving the open design question the predecessor note below originally flagged (see the design doc's §6.2).
  - `mnemosyne snapshot save <heap> [--output <dir>] | load <hash-or-path> | list | rm <hash>` CLI surface, plus additive `--snapshot <hash-or-path>` / `--refresh` flags on `analyze`, `leaks`, `gc-path`, `inspect`, and `query` (every command that calls the binary parser). No flags at all auto-discovers a fresh matching cache entry silently; an explicit `--snapshot <key>` loads exactly that entry and fails loudly (never silently re-parses) on a stale, mismatched, corrupt, or missing key. New CLI exit codes `10` (`snapshot_not_found`), `11` (`snapshot_schema_mismatch`), `12` (`snapshot_stale_source`), `13` (`snapshot_corrupt`) — auto-discovery misses are not errors, only explicit `--snapshot`/`snapshot load` usage surfaces them.
  - MCP `open_snapshot` (key -> manifest) and `list_snapshots`, plus an additive `snapshot: string` param on `analyze_heap`, `parse_heap`, `find_gc_path`, `inspect_object`, and `query_heap` — same additive-param-not-new-tool pattern M8/M10 used. `parse_heap`'s `snapshot` param cannot reconstruct a real `HeapSummary` (that requires a raw HPROF record-tag scan a cached `ObjectGraph`/`DominatorTree` doesn't retain), so it returns a distinctly-shaped partial response (manifest fields + an honestly-computed `total_shallow_size_bytes`) carrying a `ProvenanceKind::Partial` marker instead — a shape decision the module's own doc comments flag explicitly, not an oversight.
  - Cache root: `dirs::cache_dir()/mnemosyne`, overridable via `MNEMOSYNE_SNAPSHOT_DIR` only (**no** `[snapshot].directory` config-key override shipped, unlike the two-mechanism override originally speculated below).
- **Not yet shipped (open follow-up):** the Slice 9.D criterion benchmark comparing cold-parse vs. snapshot-load wall-clock (`core/benches/snapshot_load.rs`) was scoped but not written — the §12 performance budget in the design doc remains directional/unmeasured rather than benchmark-confirmed. Round-trip correctness (including via the MCP path) is fully covered by tests; only the timing claim is unverified.
- **Out of scope:** Cross-machine snapshot interchange (defer). Multi-snapshot in a single MCP session beyond explicit `open` (M11 territory). No UI/Tauri surfacing.
- **Why now / strategic rationale:** This is the single biggest UX gap vs MAT for repeat workflows. It is also a force multiplier for M10 (heap diff) and M11 (MCP workflow suite), both of which need cheap snapshot re-open. Landed **after** M8 so the on-disk format could include M8's new analyzer context from day one (moot in practice, since §6.2 resolved that those outputs don't need precomputing at all).
- **Success criteria (met):** Round-trip fidelity (`get_references`/`get_referrers`/`retained_size`/`immediate_dominator` identical before/after save+load) and all three staleness triggers each produce their distinct structured error, verified in `core/tests/snapshot_round_trip.rs` and `core/tests/snapshot_staleness.rs`. CLI (`cli/tests/snapshot_cli.rs`) and MCP (`core/src/mcp/server.rs` test module) both cover save/load/list/rm and the additive-flag/param paths on every wired command. Every pre-M9 `analyze`/`leaks`/`gc-path`/`inspect`/`query` invocation without `--snapshot` stays byte-identical.
- **Risks / dependencies:** Format stability — `SNAPSHOT_SCHEMA_VERSION` is the enforced compatibility contract; `mnemosyne_version` is recorded but informational only. Disk-quota / unbounded cache growth is explicitly out of scope this milestone (manual `snapshot rm` only). Privacy: snapshots inherit the same sensitivity as the source HPROF file (documented caveat, not new redaction code) — treat `~/.cache/mnemosyne/` accordingly.
- **Estimated slice count:** 5 shipped (9.A–9.D implementation + 9.E doc-sync).

### M10 — Compare Two Heaps (Object-Level Diff) (Parity-Closing + Differentiator-Extending)

- **Status:** 🟡 **Mostly shipped**, landed out of sequence as design doc [milestone-8-1-object-level-diff.md](design/milestone-8-1-object-level-diff.md) (slices A–G merged via PR #38 and PR #41). The design doc used internal slice ids `8-1.A`–`8-1.H` before this roadmap refresh existed; those slices are this M10 milestone, not a sub-slice of M8. Treat `milestone-8-1-object-level-diff.md` as the M10 design doc going forward.
- **Theme:** Stable per-object identity tracking + leak-progression detection across snapshots.
- **Goal:** Replace class-level-only `diff_heaps()` with stable per-object identity heuristics so users can detect "this exact object grew", "this collection accumulated K new entries", and "this leak suspect is now M× larger" across two snapshots.
- **Delivered scope (as shipped, differs from original text below in naming only — see note):**
  - Object-identity heuristics: `class+retained`, `class+dominator` (default), `full-fingerprint` — class name + log-bucketed retained size + immediate-dominator class chain + optional field-shape/outbound-reference signature. `MatchQuality` (collision rate, false-match/false-split risk) reported per diff instead of `ProvenanceKind::Partial`.
  - `core::diff::run_diff()` / `core::diff::object::engine` returning `ObjectDiffReport` with `added` / `removed` / `retained_changed` per-object delta records (shipped as `ObjectDelta`, not a bare `diff_objects()` free function).
  - `mnemosyne diff before.hprof after.hprof --mode object [--identity-strategy ...]` CLI (shipped flag is `--mode object`, not `--object-level`) + MCP tool `diff_heaps` with `mode: "object"` (shipped as a mode on the existing `diff_heaps` tool, not a separate `diff_objects` tool).
  - Growth-suspect ranking: top-N (`--top`, default 50) objects by retained-size delta, both `added`/`removed`/`retained_changed`.
  - Text/JSON/TOON renderers under `core::report::diff`.
- **Not yet shipped (open follow-up, tracked as M10-B):**
  - **`ci-check object_growth_threshold` predicate** — the differentiator extension into `core::policy` was not part of slices A–G and has no design coverage yet.
  - Leak-progression cross-reference with `detect_leaks()` output.
- **Out of scope:** 3+ snapshot trend analysis (defer to M14). Cross-machine snapshot diff. Time-series database backend. Persistent fingerprint indexes (M8-8/M9 territory).
- **Why now / strategic rationale:** Closes a top-3 MAT gap **and** opens a category MAT does not own — heap-regression CI gating at object granularity. Landed ahead of M9 rather than after; M9 (snapshot persistence) would still make repeat diffs cheaper but was not a hard blocker for this slice of work.
- **Success criteria:** `diff --mode object` identifies grown/added/removed objects in synthetic two-snapshot fixtures (`pure-add`, `pure-remove`, `retained-grow`, collision fixtures — shipped). `ci-check object_growth_threshold` — **pending, M10-B**. False-positive rate documented via `MatchQuality.collision_rate` (shipped).
- **Risks / dependencies:** Object identity is fundamentally heuristic without write-barrier instrumentation; honesty contract upheld via `MatchQuality` + structured `feature_unavailable_*` errors rather than silent fallback. Memory cost mitigated with `--object-diff-min-retained` floor + `MAX_OBJECT_DIFF_FINGERPRINTS` hard cap (see design doc §6.2).
- **Estimated slice count:** 5–10 slices (8 shipped: A–G implementation + validation; H doc-sync landing now).

### M11 — MCP Workflow Suite (Pure Differentiator)

- **Theme:** Pre-canned MCP tool sets for common AI-agent triage workflows.
- **Goal:** Move the MCP server from "14 generic tools" to a curated workflow surface where an AI agent can complete an entire triage session — leak triage, GC-root tuning, object-graph traversal — without bespoke prompting.
- **Scope:**
  - Workflow tool sets: `triage_memory_leak`, `tune_gc`, `traverse_object_graph`, `compare_snapshots` (composing existing primitives).
  - Workflow state machines with `ProvenanceKind`-aware step transitions.
  - MCP `describe_workflow` / `start_workflow` / `next_step` lifecycle.
  - Companion documentation: `docs/mcp-workflows.md` with prompt-engineering examples.
  - Optional: classloader-leak workflow (parity with MAT's classic Tomcat-leak hunt) once classloader explorer ships in M13.
- **Out of scope:** New analyzers (those go in M8 / M13). Server-side AI inference (provider mode already covers this).
- **Why now / strategic rationale:** Pure differentiator — MAT has no equivalent. Multiplies the value of every prior milestone with no architectural risk. Best leverage of M5 + the AI investments already made.
- **Success criteria:** ≥4 workflows shipped, each with a passing scripted-agent integration test. Docs include reproducible AI-agent transcripts. MCP lifecycle methods stable.
- **Risks / dependencies:** Workflow drift if underlying analyzers change without updating workflow contracts. Mitigate with contract tests.
- **Estimated slice count:** 5–10 slices.

### M12 — Reference-Workstation Benchmark Re-Run (Credibility / M7-5 Closeout)

- **Theme:** Close the published-evidence credibility gap.
- **Goal:** Execute the M7-5 reference-spec methodology on a native-Linux reference workstation: Eclipse MAT, `mnemo-deep`, `mnemo-overview`, `hprof-slurp`, all four fixtures including the `10 GiB` tier, equivalence (Jaccard), and full RSS table with GNU `/usr/bin/time -v`.
- **Scope:**
  - Re-run `scripts/bench/run_comparative.sh` per [milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md) on the reference workstation in [docs/benchmarks/reference-spec.md](benchmarks/reference-spec.md).
  - Publish updated `docs/benchmarks/comparative-v0.3.x.md` (or `v0.4.0.md`) with full methodology, raw artifacts, RSS table, and equivalence numbers.
  - Update `README.md`, `STATUS.md`, and roadmap to retire the "partial" caveat.
- **Out of scope:** New tools or features. Architectural change.
- **Why now / strategic rationale:** Required to retire the M7-5 partial caveat. Smallest milestone in this list. Can run in parallel with M8 because it does not touch source code.
- **Success criteria:** Published full report; partial caveat removed from `STATUS.md` and `README.md`; RSS comparison table published.
- **Risks / dependencies:** Requires access to a native-Linux reference workstation with Eclipse MAT installed and the `10 GiB` fixture. Hardware-dependent timing.
- **Estimated slice count:** 2–5 slices.

### M13 — Classloader Explorer & Leak Detection (Parity-Closing) — ✅ Shipped

- **Status:** ✅ **Shipped** — design doc [milestone-13-classloader-explorer.md](design/milestone-13-classloader-explorer.md), slices 13.A–13.C (implementation) + 13.D (documentation sync, this pass).
- **Theme:** First-class classloader-leak detection (the Tomcat / Jetty / Spring webapp leak).
- **Goal:** Detect the classic "same class loaded by N classloaders" leak pattern and provide drill-down into per-loader uniqueness.
- **Grounding correction (found during design, binding for this milestone):** roadmap's original framing ("🟡 per-loader histogram only") understated what M3 Phase 3 already shipped. `analyze_classloaders()` already computed `potential_leaks` — a **single-loader** heuristic ("retains a lot, loads almost nothing else") — before this milestone; that heuristic's CLI rendering and test coverage were closed as a grounding fix immediately ahead of M13's own implementation. M13 does **not** replace `potential_leaks` — it ships a second, independent signal alongside it. Both stay permanently; a loader can trip one, the other, both, or neither.
- **Delivered scope (as shipped; one naming difference from the original text below — see note):**
  - `core::analysis::classloader::detect_duplicate_classes(graph) -> Vec<DuplicateClassGroup>` — the actual new logic, MAT's "Duplicate Classes" report shape: groups `graph.classes` by normalized class name and keeps only names declared by ≥2 distinct `class_loader_id` values (the same class loaded twice by the *same* loader is correctly excluded). `ClassLoaderReport` gains `duplicate_classes: Vec<DuplicateClassGroup>` as a new field alongside the untouched `potential_leaks`.
  - `ClassLoaderInfo` gains two new fields computed from the same single grouping pass (no second scan): `unique_class_count` (classes loaded by this loader and no other) and `ancestor_chain: Vec<ObjectId>` (bounded parent-loader walk via new `resolve_loader_chain()`, default cap 16, cycle-guarded so adversarial/malformed HPROF data cannot hang the walk).
  - CLI: `mnemosyne analyze --classloaders` (no new flag — additive to the existing flag) gains a "Duplicate classes across loaders" section, printed only when non-empty, same convention as the pre-existing "Potential classloader leaks" section. The per-loader table gains an "Ancestors" column (**not** a "Unique" column as originally speculated below — `unique_class_count` is computed and serialized but not yet CLI-rendered; see the design doc's closeout for this scope-drift note).
  - `ci-check classloader_leak_count` predicate (threshold on `duplicate_classes.len()`, deep-only, same skip convention as `leak_count`/`retained_size`/`dominator_root_count`) in `core::policy`. A follow-up fix landed in the same slice after code review: `ci-check`'s handler had hardcoded `enable_classloaders: false`, so the predicate's own evaluator logic was correct but never received real data outside its unit tests — now `enable_classloaders` is derived from whether the loaded policy actually declares the rule.
  - MCP `detect_classloader_leaks` tool (`heap_path` in, `Vec<DuplicateClassGroup>` out) — a focused, cheaper single-purpose call, same rationale as `diff_heaps` existing standalone. `analyze_heap`'s existing `enable_classloaders` param needed no new param; the extended `ClassLoaderReport`'s new fields are additive and appear automatically.
- **Not shipped (explicit non-goal, per design doc §4/§12):** UI leak-workspace panel (downgraded to documented future work, matching the established M8/M9/M10/M11 backend-before-UI pattern). Group-by-superclass histograms. Live classloader unloading. Custom plugin runtime for user-defined heuristics.
- **Why now / strategic rationale:** Closes a real MAT capability that matters specifically for JVM webapp / app-server users — a non-trivial slice of Mnemosyne's target audience. Builds cleanly on the M3 classloader report, and — per §3.3 of the design doc — deliberately keeps both leak signals alive rather than treating the new one as a replacement.
- **Success criteria (met):** A deliberately-duplicated "redeployed webapp" fixture (same class name, two distinct loaders) produces exactly one `DuplicateClassGroup`; a class loaded twice by the *same* loader never appears in any group; a three-loader mixed fixture confirms `unique_class_count` and `duplicate_classes` agree (derived from one grouping pass, not two); a three-level ancestor chain resolves in ascending generation order and a self-referential / two-node-cycle chain terminates without hanging; the `ci-check` predicate fires on a duplicated fixture, stays clean on a non-duplicated one, and skips (not errors) on overview-mode input; the pre-existing `potential_leaks` tests continue passing unchanged, proving the two-signals-coexist claim as a regression gate, not just a design note.
- **Risks / dependencies:** Parent-loader chain walk on adversarial/cyclic HPROF data — mitigated with a bounded depth (16) plus a visited-set cycle guard that terminates well before the depth bound, same discipline as M8 Slice 8.A's path-enumeration budget caps. False positives on legitimately re-loaded framework classes — mitigated by reporting the raw signal without a baked-in severity judgment, same "give the operator the data" philosophy as `MatchQuality` (M10) and `potential_leaks` itself.
- **Estimated slice count:** 2–5 slices (4 shipped: 13.A–13.C implementation + 13.D doc-sync).

### Other backlog items (lower priority — not proposed as standalone M8+)

| # | Item | Origin | Priority | Notes |
|---|---|---|---|---|
| B1 | Full OQL expansion (subqueries, multi-hop, regex, `eval`) | M7-4 deferred | P2 | Treadmill risk; absorb into M8 / M9 incrementally rather than as standalone |
| B2 | Property-based parser testing | M7 deferred | P2 | `proptest` for binary parser robustness — fold into M8 hardening slice |
| B3 | Byte-accurate progress bars | M7 deferred | P3 | Polish — fold into M8 |
| B4 | Incremental leak tracking (3+ snapshots) | D3 | P2 | Depends on M9 + M10; consider M14 |
| B5 | IDE-native memory annotations (LSP / VS Code) | D4 | P3 | Differentiator; consider M14+ |
| B6 | Smart heap reduction advisor | D5 | P3 | Builds on M8 + M10 |
| B7 | Tauri desktop release (signed) | M6 follow-on | P3 | Adoption-data dependent |
| B8 | Streaming responses (MCP) | M5 follow-on | P3 | Only if evidence shows need |
| B9 | Custom plugin / extension runtime | MAT parity | P3 | Defer until adoption justifies |

---

## 6. Recommended Next Milestone — **M8 Reachability & References Deep Dive**

**Recommendation:** Schedule **M8 — Reachability & References Deep Dive** as the active next milestone.

**Justification (4–6 sentences):**

1. **Highest user value among parity-closing options:** all-paths-to-GC-roots, group-by-referrer, and object inspector are top-3 MAT investigation workflows that Mnemosyne users explicitly lack today, and they directly serve the project's core mission (leak triage credibility). M9 (persistence) and M10 (object diff) have higher infrastructure value but lower per-day user value.
2. **Lowest design risk:** the `ObjectGraph`, dominator, and `get_referrers()` primitives all exist; M8 is composition + new CLI/MCP surfaces, not new graph algorithms. M9 and M10 both require new on-disk schemas and identity heuristics with non-trivial design risk.
3. **Best leverage of existing M1–M7 architecture:** the analyzers reuse existing graph traversal, the deep-mode-only constraint pattern is established (M7-3 / M7-4), and provenance markers, exit codes, and overview-aware error paths are already in place to copy.
4. **Strategic positioning — both parity-closing and differentiator-extending:** every M8 surface ships with structured provenance, MCP first-class exposure, JSON / TOON / GH Actions output, and overview-aware errors. M8 closes a MAT gap **while widening** the provenance / MCP / automation moat. Pure differentiator milestones (M11) are valuable but should follow at least one parity push to keep the MAT-comparison story credible.
5. **Fit with Mnemosyne identity:** the project's stated goal is "Path to MAT" — closing three top-3 MAT investigation workflows in one milestone is the most direct expression of that goal. It also lays groundwork for M9 (snapshots must include the new analyzers' precomputed outputs) and M11 (MCP workflows can compose the new tools).
6. **Parallelizable with M12:** M12 is documentation + benchmark execution only and does not touch source code, so it can run in parallel with M8 without ownership conflicts.

**Recommended sequencing:**

1. **M8** — Reachability & References Deep Dive (active)
2. **M12** — Reference-workstation rerun (parallel with M8; no source changes)
3. **M9** — Snapshot persistence (after M8 stabilizes the new analyzers)
4. **M10** — Object-level diff (after M9 lands cheap re-open)
5. **M11** — MCP workflow suite (after M8 + M10 give it richer primitives to compose)
6. **M13** — Classloader explorer (parallel-eligible with M11; independent surface)

---

## 7. MAT Feature Parity Scorecard (post-v0.3.0)

Updated to reflect the parity matrix in §2.

| MAT Feature | Mnemosyne Status | Closes In |
|---|---|---|
| Dominator tree | ✅ | — |
| Retained sizes | ✅ | — |
| Leak suspects | ✅ | — |
| GC root paths (shortest) | ✅ | — |
| Histogram grouping (class / package / classloader) | ✅ | — |
| Thread inspection (stacks + retained) | ✅ | — |
| String analysis (duplicate detection) | ✅ | — |
| Collection inspection | ✅ | — |
| Reachable / unreachable analysis | ✅ | — |
| Top consumers | ✅ | — |
| OQL — targeted subset | 🟡 ~30% | M7-4 ✅ → B1 (incremental) |
| GC root paths — all paths / by class | ✅ (`gc-path --all-paths`/`--by-class`) | **M8** ✅ shipped |
| Group by referrer | ✅ (`analyze --by-referrer`) | **M8** ✅ shipped |
| Object inspector (CLI/MCP) | ✅ (`mnemosyne inspect`, MCP `inspect_object`) | **M8** ✅ shipped |
| Thread frame-locals | ✅ (`analyze --threads` `local:`/`jni-local:` lines) | **M8** ✅ shipped |
| Object-level heap diff | ✅ (`--mode object`; `ci-check` predicate pending) | **M10** ✅ mostly shipped |
| Persistent indexes / parse-once-query-many | ✅ (`snapshot save\|load\|list\|rm`, `--snapshot`/`--refresh`) | **M9** ✅ shipped |
| Classloader leak detection | ✅ (`potential_leaks` single-loader heuristic + `duplicate_classes` cross-loader signal, both permanent) | **M13** ✅ shipped |
| Group by superclass | ❌ | B-list |
| Duplicate primitive arrays | ❌ | B-list |
| Custom plugin runtime | ❌ | Defer (B9) |
| Large-dump handling — full reference benchmark | 🟡 partial WSL | **M12** |
| **CI/CD automation** | ✅ Better than MAT | M7-2 ✅ — extended in M10 |
| **AI-assisted diagnosis** | ✅ MAT has none | Differentiator |
| **Provenance tracking** | ✅ MAT has none | Differentiator |
| **MCP / IDE integration** | ✅ MAT has none | Extended in M11 |
| **Streaming overview mode** | ✅ MAT has none | M7-1 ✅ |
| **Allocation flame graphs** | ✅ MAT has none natively | M7-3 ✅ |

## 8. Risk Register

Active risks only. Resolved risks live in [roadmap-archive.md](roadmap-archive.md).

| Risk | Why it matters | Mitigation |
|---|---|---|
| M7-5 reference-workstation rerun still pending | Performance claims are only as good as published evidence | Schedule **M12** in parallel with M8 |
| OQL scope creep | Full MAT parity is an endless tail | Absorb full OQL into M8 / M9 incrementally; never as a standalone milestone |
| Object-identity heuristics in M10 will be approximate | Honesty contract risk if growth-detection looks more authoritative than it is | Mandatory `ProvenanceKind::Partial` markers; documented false-positive bounds |
| M9 snapshot format churn | Re-versioning a shipped on-disk format is expensive | Land M9 **after** M8 so the format includes the new analyzers from day one |
| Provider-specific AI quality drift | Multi-provider UX can erode trust | Strict wire contracts; provider tests; rules fallback always available |
| Desktop hardening could consume bandwidth | Native packaging is not the credibility blocker | Keep B7 deferred until adoption data justifies |
| Differentiator dilution | Adding parity-only features without preserving provenance / MCP / overview-aware error paths would erode the moat | **Roadmap-wide invariant** in §0; every M8+ milestone must extend or preserve at least one differentiator |
| Rare HPROF edge cases on new real-world heaps | Parser correctness risk grows with broader fixture variety | Property-based testing (B2) folded into M8 hardening |

## 9. Design Documents Index

| Scope | Design Doc | Status |
|---|---|---|
| M1 — Stability & Trust | [design/milestone-1-stability-and-trust.md](design/milestone-1-stability-and-trust.md) | ✅ |
| M1.5 — Real-World Hardening | [design/milestone-1.5-real-world-hardening.md](design/milestone-1.5-real-world-hardening.md) | ✅ |
| M2 — Packaging, Releases, DX | [design/milestone-2-packaging-releases-dx.md](design/milestone-2-packaging-releases-dx.md) | ✅ |
| M3 — Core Heap Analysis Parity | [design/milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md) | ✅ |
| M4 — UI & Usability | [design/milestone-4-ui-and-usability.md](design/milestone-4-ui-and-usability.md) | ✅ |
| M5 — AI / MCP / Differentiation | [design/milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md) | ✅ |
| M6 — Ecosystem and Community | [design/milestone-6-ecosystem-and-community.md](design/milestone-6-ecosystem-and-community.md) | ✅ |
| M7 parent | [design/milestone-7-production-readiness.md](design/milestone-7-production-readiness.md) | ✅ Shipped |
| M7-1 streaming overview | [design/milestone-7-1-streaming-overview-mode.md](design/milestone-7-1-streaming-overview-mode.md) | ✅ |
| M7-2 CI policies | [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md) | ✅ |
| M7-3 flame graphs | [design/milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md) | ✅ |
| M7-4 OQL targeted | [design/milestone-7-4-oql-targeted-expansion.md](design/milestone-7-4-oql-targeted-expansion.md) | ✅ |
| M7-5 comparative benchmarks | [design/milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md) | 🟡 partial |
| M7-6 v0.3.0 release | [design/milestone-7-6-v0-3-0-release.md](design/milestone-7-6-v0-3-0-release.md) | ✅ Shipped |
| Scaling support | [design/memory-scaling.md](design/memory-scaling.md) | ✅ |
| **M8 Reachability & References** | [design/milestone-8-reachability-references.md](design/milestone-8-reachability-references.md) | ✅ Shipped (slices 8.A–8.D implementation, 8.E doc-sync) |
| **M9 Snapshot Persistence** | [design/milestone-9-snapshot-persistence.md](design/milestone-9-snapshot-persistence.md) | ✅ Shipped (slices 9.A–9.D implementation, 9.E doc-sync) |
| **M10 Object-Level Diff** | [design/milestone-8-1-object-level-diff.md](design/milestone-8-1-object-level-diff.md) | 🟡 Mostly shipped (slices A–G; `ci-check` predicate = M10-B) |
| **M11 MCP Workflow Suite** | _to be authored by Design Consulting_ | ⏳ Pending |
| **M12 Reference-Workstation Re-run** | reuse [design/milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md) | ⏳ Pending |
| **M13 Classloader Explorer** | [design/milestone-13-classloader-explorer.md](design/milestone-13-classloader-explorer.md) | ✅ Shipped (slices 13.A–13.C implementation, 13.D doc-sync) |

---

For milestone history, completed-batch detail, prior backlog tables, the original M7 proposal, and the full v0.2.0 state snapshot, see [roadmap-archive.md](roadmap-archive.md).
