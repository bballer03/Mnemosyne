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
| Class loader explorer (per-loader histogram, unique classes) | ✅ | ✅ | None | — | `analyze --classloaders` ships `ClassLoaderReport { loaders, potential_leaks, duplicate_classes }` — **two independent leak signals that coexist, neither replacing the other**: (1) `potential_leaks`, a single-loader "retains a lot, loads almost nothing else" heuristic (M3 Phase 3, CLI-rendered + tested as of M13's grounding fix), and (2) `duplicate_classes` (new, M13), MAT's actual "Duplicate Classes" cross-loader signal — a class name loaded by ≥2 distinct classloaders, the classic Tomcat/Jetty/Spring hot-redeploy pattern. Each `ClassLoaderInfo` also gains `unique_class_count` (computed, not yet CLI-rendered — see M13 design doc closeout) and `ancestor_chain` (bounded parent-loader walk, rendered as an "Ancestors" column). `ci-check classloader_leak_count` predicate and MCP `detect_classloader_leaks` tool both ship. Shipped M13 Slices 13.A–13.C (13.D is this doc-sync). Browser UI: `ClassloaderExplorerPanel` in Artifact Explorer surfaces both signals plus the ancestor chain. Shipped M14 Slice 14.C. |
| Duplicate strings / arrays detection | ✅ | 🟡 | Strings yes; arrays no | Low | `analyze_strings()` reports duplicate groups + dedup waste. **No** equivalent for primitive arrays or boxed array dedup. MAT has both. |
| Thread overview + frame-locals + stack | ✅ | ✅ | None | — | `inspect_threads()` now additionally cross-references `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` GC roots into per-frame `FrameLocal { variable_slot, object_id, class_name, root_kind }` entries, printed as `local:`/`jni-local:` lines under each stack frame in `analyze --threads` text output. `variable_slot` honestly reuses the HPROF frame number — HPROF frame-local roots carry no genuine bytecode slot index. Shipped M8 Slice 8.D. Browser UI: new `/heap-explorer/threads` route (`ThreadExplorerPanel`) renders per-thread stacks with per-frame local tables. Shipped M14 Slice 14.C. |
| Inspector — object field-level browse, refs in/out | ✅ | ✅ | None | — | `mnemosyne inspect <heap> --object-id <id> [--retain-field-data] [--format text\|json\|toon]` and MCP `inspect_object` ship a focused single-object view: shallow/retained size, dominator parent/children, references out, referrers in (all as structured `ObjectRef { object_id, class_name }`, not baked strings), and opt-in typed field values. Shipped M8 Slice 8.C. Browser UI: `ObjectInspectorPanel` renders refs/dominator parent-children as clickable navigation chips via a new optional `inspectObject` bridge method. Shipped M14 Slice 14.B. |
| Group by class / classloader / package / superclass | ✅ | 🟡 | Class / package / classloader yes; superclass no | Low | `--group-by class\|package\|classloader`. Missing `--group-by superclass` (and the related "group by class -> superclass tree"). |
| **Group by referrer** (incoming references analysis) | ✅ | ✅ | None | — | `mnemosyne analyze --by-referrer` and MCP `analyze_heap` `by_referrer: boolean` param rank objects by incoming-reference count (tiebreak retained size), with top referrer classes per entry. Shipped as a `ReferrerReport` optional field on `AnalyzeResponse` / an additive param on the existing `analyze_heap` tool — not a separate `analyze_by_referrer` tool (roadmap text below in §5 originally speculated otherwise). Shipped M8 Slice 8.B. Browser UI: new `ReferrerPanel` in Artifact Explorer renders the ranked table directly from the artifact, no live bridge call needed. Shipped M14 Slice 14.C. |
| Reachable / unreachable objects analysis | ✅ | ✅ | None | — | `find_unreachable_objects()` walks from GC roots and reports per-class counts + shallow size. |
| **Compare two heap dumps** (object-level diff) | ✅ | ✅ | None | — | `mnemosyne diff --mode object` ships fingerprint-based per-object identity (`class+retained` / `class+dominator` / `full-fingerprint`), added/removed/retained_changed sections, `MatchQuality`, and MCP `diff_heaps` mode. `ci-check --baseline` + `object_growth_threshold` predicate and `--cross-reference-leaks` leak-progression cross-reference shipped in M10-B. Browser UI: new `/compare` route (comparison basket) loads a precomputed diff report or runs one live via a new dedicated bridge, rendering added/removed/retained-changed tables with a match-quality badge. Shipped M14 Slice 14.A. |
| Allocation-site flame graphs | 🟡 | ✅ | **Mnemosyne ahead** | — | MAT has no native flame-graph export; users typically pipe to async-profiler. Mnemosyne ships SVG / folded-stack / JSON natively. **Differentiator.** |
| Custom inspector views / extensions | ✅ | ❌ | Not shipped | Defer | MAT plugins (`org.eclipse.mat.api.IQuery`) are widely used. Mnemosyne has a plugin design doc (M6) but no runtime extension surface. Defer until adoption justifies. |
| **Index files / persistent snapshot** (parse-once, query-many) | ✅ | ✅ | None | — | `mnemosyne snapshot save\|load\|list\|rm` plus additive `--snapshot <hash-or-path>`/`--refresh` on `analyze`/`leaks`/`gc-path`/`inspect`/`query` cache the parsed `ObjectGraph` + `DominatorTree` keyed by heap-file SHA-256 under `dirs::cache_dir()/mnemosyne` (override via `MNEMOSYNE_SNAPSHOT_DIR`); auto-discovery re-opens a fresh matching cache entry with no flags at all. Schema/staleness mismatches on an *explicit* `--snapshot`/`snapshot load` fail loudly with exit codes `10`-`13`; auto-discovery misses fall through to a normal parse, never silently. MCP gets `open_snapshot`/`list_snapshots` plus an additive `snapshot` param on `analyze_heap`/`parse_heap`/`find_gc_path`/`inspect_object`/`query_heap`. Shipped M9 Slices 9.A-9.D (9.E is this doc-sync). Browser UI: `RecentHeapsList` on the AI-guided landing lists cached snapshots via a new `listSnapshots` bridge method, omitting the section entirely when unavailable. Shipped M14 Slice 14.D. |
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
- **Out of scope:** Full MAT OQL `inbounds` / `outbounds` traversal (defer to M9-2). Custom IQuery plugin runtime. UI surfaces beyond what the existing leak workspace already covers (**since resolved:** M14 Slices 14.B/14.C shipped the object-inspector chip navigation, GC-path multi-view, referrer panel, and thread view this note deferred).
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
- **Out of scope:** Cross-machine snapshot interchange (defer). Multi-snapshot in a single MCP session beyond explicit `open` (M11 territory). No UI/Tauri surfacing (**since resolved for the browser UI:** M14 Slice 14.D's `RecentHeapsList` surfaces `list_snapshots` on the guided landing; Tauri native-command wiring remains unaddressed, deferred to M16 per that milestone's own scope).
- **Why now / strategic rationale:** This is the single biggest UX gap vs MAT for repeat workflows. It is also a force multiplier for M10 (heap diff) and M11 (MCP workflow suite), both of which need cheap snapshot re-open. Landed **after** M8 so the on-disk format could include M8's new analyzer context from day one (moot in practice, since §6.2 resolved that those outputs don't need precomputing at all).
- **Success criteria (met):** Round-trip fidelity (`get_references`/`get_referrers`/`retained_size`/`immediate_dominator` identical before/after save+load) and all three staleness triggers each produce their distinct structured error, verified in `core/tests/snapshot_round_trip.rs` and `core/tests/snapshot_staleness.rs`. CLI (`cli/tests/snapshot_cli.rs`) and MCP (`core/src/mcp/server.rs` test module) both cover save/load/list/rm and the additive-flag/param paths on every wired command. Every pre-M9 `analyze`/`leaks`/`gc-path`/`inspect`/`query` invocation without `--snapshot` stays byte-identical.
- **Risks / dependencies:** Format stability — `SNAPSHOT_SCHEMA_VERSION` is the enforced compatibility contract; `mnemosyne_version` is recorded but informational only. Disk-quota / unbounded cache growth is explicitly out of scope this milestone (manual `snapshot rm` only). Privacy: snapshots inherit the same sensitivity as the source HPROF file (documented caveat, not new redaction code) — treat `~/.cache/mnemosyne/` accordingly.
- **Estimated slice count:** 5 shipped (9.A–9.D implementation + 9.E doc-sync).

### M10 — Compare Two Heaps (Object-Level Diff) (Parity-Closing + Differentiator-Extending)

- **Status:** ✅ **Shipped**, landed out of sequence as design doc [milestone-8-1-object-level-diff.md](design/milestone-8-1-object-level-diff.md) (slices A–G merged via PR #38 and PR #41), with the `ci-check object_growth_threshold` predicate and leak-progression cross-reference closed out by the M10-B follow-up ([milestone-10-b-object-growth-policy.md](design/milestone-10-b-object-growth-policy.md)). The design doc used internal slice ids `8-1.A`–`8-1.H` before this roadmap refresh existed; those slices are this M10 milestone, not a sub-slice of M8. Treat `milestone-8-1-object-level-diff.md` as the M10 design doc going forward.
- **Theme:** Stable per-object identity tracking + leak-progression detection across snapshots.
- **Goal:** Replace class-level-only `diff_heaps()` with stable per-object identity heuristics so users can detect "this exact object grew", "this collection accumulated K new entries", and "this leak suspect is now M× larger" across two snapshots.
- **Delivered scope (as shipped, differs from original text below in naming only — see note):**
  - Object-identity heuristics: `class+retained`, `class+dominator` (default), `full-fingerprint` — class name + log-bucketed retained size + immediate-dominator class chain + optional field-shape/outbound-reference signature. `MatchQuality` (collision rate, false-match/false-split risk) reported per diff instead of `ProvenanceKind::Partial`.
  - `core::diff::run_diff()` / `core::diff::object::engine` returning `ObjectDiffReport` with `added` / `removed` / `retained_changed` per-object delta records (shipped as `ObjectDelta`, not a bare `diff_objects()` free function).
  - `mnemosyne diff before.hprof after.hprof --mode object [--identity-strategy ...]` CLI (shipped flag is `--mode object`, not `--object-level`) + MCP tool `diff_heaps` with `mode: "object"` (shipped as a mode on the existing `diff_heaps` tool, not a separate `diff_objects` tool).
  - Growth-suspect ranking: top-N (`--top`, default 50) objects by retained-size delta, both `added`/`removed`/`retained_changed`.
  - Text/JSON/TOON renderers under `core::report::diff`.
  - **(M10-B)** `ci-check --baseline <BEFORE_HEAP>` + `Predicate::ObjectGrowthThreshold` (`object_growth_threshold`), a class-scoped, two-heap `core::policy` predicate that fails CI when a tracked object (or class of objects) grows beyond a per-class retained-size limit. Missing `--baseline` with such a rule present is a loud, structured `object_growth_threshold_requires_baseline` error (exit code 2), never a silent skip.
  - **(M10-B)** `mnemosyne diff --mode object --cross-reference-leaks` + `ObjectDelta.leak_severity` + `core::diff::object::annotate_leak_progression()`: cross-references `added`/`retained_changed` deltas against `detect_leaks()` suspects on the after-heap, annotating text output with a `[LEAK: <severity>]` suffix. Opt-in, default off.
- **Out of scope:** 3+ snapshot trend analysis (defer to M14). Cross-machine snapshot diff. Time-series database backend. Persistent fingerprint indexes (M8-8/M9 territory). MCP wiring for `--baseline`/`--cross-reference-leaks` (CLI-first for M10-B; deferred to a future slice if usage justifies it).
- **Why now / strategic rationale:** Closes a top-3 MAT gap **and** opens a category MAT does not own — heap-regression CI gating at object granularity. Landed ahead of M9 rather than after; M9 (snapshot persistence) would still make repeat diffs cheaper but was not a hard blocker for this slice of work.
- **Success criteria:** `diff --mode object` identifies grown/added/removed objects in synthetic two-snapshot fixtures (`pure-add`, `pure-remove`, `retained-grow`, collision fixtures — shipped). `ci-check object_growth_threshold` — **shipped, M10-B**. False-positive rate documented via `MatchQuality.collision_rate` (shipped).
- **Risks / dependencies:** Object identity is fundamentally heuristic without write-barrier instrumentation; honesty contract upheld via `MatchQuality` + structured `feature_unavailable_*` errors rather than silent fallback. Memory cost mitigated with `--object-diff-min-retained` floor + `MAX_OBJECT_DIFF_FINGERPRINTS` hard cap (see design doc §6.2).
- **Estimated slice count:** 5–10 slices (8 shipped: A–G implementation + validation, H doc-sync; M10-B follow-up landed as a single additional slice).

### M11 — MCP Workflow Suite (Pure Differentiator) — ✅ Shipped

- **Status:** ✅ **Shipped** — design doc [milestone-11-mcp-workflow-suite.md](design/milestone-11-mcp-workflow-suite.md), slices 11.A–11.D (implementation) + 11.E (documentation sync, this pass).
- **Theme:** Pre-canned MCP tool sets for common AI-agent triage workflows.
- **Goal:** Move the MCP server from a flat tool list to a curated workflow surface where an AI agent can complete an entire triage session — leak triage, GC-root retention review, object-graph traversal, or two-snapshot comparison — without bespoke prompting to sequence the underlying calls itself.
- **Delivered scope (as shipped; matches the design doc closely, with one config-surface difference from its own speculative text — see note):**
  - New top-level `core::workflow` module (sibling of `mcp`/`snapshot`/`diff`/`policy`), all four `WorkflowKind` variants fully implemented: `TriageMemoryLeak` (`detect` → `investigate_suspect` → `explain` → `propose_fix` → `complete`, Slice 11.A), `TuneGc` (`root_kind_breakdown` → `thread_local_review` → `top_retainers` → `complete`) and `TraverseObjectGraph` (`inspect` ⇄ `choose_direction` → `complete`, the one kind with a real caller-driven branch point — Slice 11.B), and `CompareSnapshots` (`resolve_snapshots` → `diff` → `complete`, composing `SnapshotStore` + M10's `run_diff`/`DiffMode::Object` — Slice 11.C).
  - `WorkflowState`/`StepRecord`/`WorkflowDescription`/`WorkflowStore` exactly as designed in §6: `WorkflowStore` mirrors `McpSessionStore`/`SnapshotStore`'s atomic-write + `schema_version: u32` shape (the third consumer of that pattern, as the design doc itself anticipated). No `WorkflowStore::list()` shipped — the design doc explicitly scoped this as optional ("add only if trivially cheap"), and it wasn't needed: `get_workflow` on a known id is sufficient, exactly as anticipated, not a gap.
  - Five MCP tools registered in `core::mcp::server` (Slice 11.D): `describe_workflow(kind)`, `start_workflow(kind, params)`, `next_step(workflow_id, step_input)`, `get_workflow(workflow_id)`, `close_workflow(workflow_id)` — same lifecycle shape as the existing `create_ai_session`/`resume_ai_session`/`get_ai_session`/`close_ai_session`/`chat_session` precedent, per the design doc's own framing. Four structured error codes (`workflow_not_found`, `workflow_corrupt`, `workflow_step_input_mismatch`, `workflow_already_complete`) flow through the same `error_details` envelope every other MCP error uses. `compare_snapshots`'s `start_workflow` params additionally accept `before_snapshot_key`/`after_snapshot_key` (M9's `--snapshot`-flag-style key format), since M9 Slices 9.C/9.D had already landed by Slice 11.D — the design doc's own conditional (§8 Slice 11.D) resolved in the "yes" branch.
  - Companion `docs/mcp-workflows.md` with one real, captured (not hand-written) request/response transcript per workflow kind, produced by driving the actual `core::mcp::server::handle_request` dispatcher against synthetic HPROF fixtures — the "reproducible AI-agent transcripts" bar this milestone's own success criteria set.
  - Contract tests (design doc §6.1, itself mandated by this milestone's own roadmap-stated R1 risk below) verifying each workflow kind's `WorkflowDescription` step-name sequence matches the actual runtime `current_step` transitions observed running the workflow to completion, for all four kinds.
- **One config-surface difference from the design doc's own speculative text:** §4 point 3 sketched a `[workflow].directory` TOML config-key override alongside an env var. Only the env var shipped — `MNEMOSYNE_WORKFLOW_DIR`, mirroring `MNEMOSYNE_SNAPSHOT_DIR`'s own precedent from M9 (which shipped the same env-only pattern, no config key, for the same reason). `core::mcp::server`'s `default_workflow_dir()` documents this explicitly in its own doc comment rather than silently diverging.
- **Out of scope (unchanged from original text):** New analyzers (M8/M13 already own that surface; M11 adds zero new heap-analysis logic — every step calls an existing, already-tested primitive). Server-side AI inference (provider mode already covers this). General-purpose/user-defined workflows — four fixed kinds only. Live-JVM GC tuning — `tune_gc` is diagnostic data only; its own `describe_workflow` output says so explicitly. Classloader-leak workflow — deferred past this milestone even though M13 (classloader explorer) has since shipped; no fifth workflow kind was added in 11.A–11.E, so this remains open future work, not silently dropped. UI/Tauri surfacing.
- **Why now / strategic rationale:** Pure differentiator — MAT has no agent-facing workflow surface at all, GUI-driven wizards excepted, and even those are presentation-layer report generators, not stateful resumable sessions. Multiplies the value of every prior milestone (M8's referrer/inspector analyzers, M9's snapshot store, M10's object diff) with no new architectural risk, since every workflow step is orchestration over already-tested code.
- **Success criteria (met):** All four workflow kinds ship with a passing full-run (`start` → `next_step`* → `complete`) integration test, each asserting intermediate step data matches what calling the underlying primitive directly would return. `docs/mcp-workflows.md` carries one real captured transcript per kind. `describe_workflow`/`start_workflow`/`next_step`/`get_workflow`/`close_workflow` are all registered in `list_tools` with the schemas the design doc's §7 specifies. Negative-path coverage exists for all four structured error codes; `TraverseObjectGraph`'s branch-point validation (rejecting a `choose_direction` input naming an id that wasn't actually offered) has dedicated tests per the design doc's §10.
- **Risks / dependencies:** R1 (workflow drift if an underlying analyzer's output shape changes without the workflow contract updating to match) — mitigated by the §6.1 contract tests, which is this milestone's one piece of process beyond normal coverage, specifically because the roadmap named this exact risk. R2 (`tune_gc`'s name implying live GC tuning that doesn't exist) — mitigated in the user/agent-facing `describe_workflow("tune_gc")` text itself, not just this doc. R5 (unbounded on-disk workflow-state growth) — same answer as M9's own R4: manual `close_workflow` only; automatic eviction remains documented future work.
- **Estimated slice count:** 5–10 slices (5 shipped: 11.A–11.D implementation + 11.E doc-sync).

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
- **Not shipped (explicit non-goal, per design doc §4/§12):** UI leak-workspace panel (downgraded to documented future work, matching the established M8/M9/M10/M11 backend-before-UI pattern — **since resolved:** M14 Slice 14.C shipped `ClassloaderExplorerPanel` in Artifact Explorer, not the leak workspace specifically, but the same `duplicate_classes`/`ancestor_chain` data now has a browser surface). Group-by-superclass histograms. Live classloader unloading. Custom plugin runtime for user-defined heuristics.
- **Why now / strategic rationale:** Closes a real MAT capability that matters specifically for JVM webapp / app-server users — a non-trivial slice of Mnemosyne's target audience. Builds cleanly on the M3 classloader report, and — per §3.3 of the design doc — deliberately keeps both leak signals alive rather than treating the new one as a replacement.
- **Success criteria (met):** A deliberately-duplicated "redeployed webapp" fixture (same class name, two distinct loaders) produces exactly one `DuplicateClassGroup`; a class loaded twice by the *same* loader never appears in any group; a three-loader mixed fixture confirms `unique_class_count` and `duplicate_classes` agree (derived from one grouping pass, not two); a three-level ancestor chain resolves in ascending generation order and a self-referential / two-node-cycle chain terminates without hanging; the `ci-check` predicate fires on a duplicated fixture, stays clean on a non-duplicated one, and skips (not errors) on overview-mode input; the pre-existing `potential_leaks` tests continue passing unchanged, proving the two-signals-coexist claim as a regression gate, not just a design note.
- **Risks / dependencies:** Parent-loader chain walk on adversarial/cyclic HPROF data — mitigated with a bounded depth (16) plus a visited-set cycle guard that terminates well before the depth bound, same discipline as M8 Slice 8.A's path-enumeration budget caps. False positives on legitimately re-loaded framework classes — mitigated by reporting the raw signal without a baked-in severity judgment, same "give the operator the data" philosophy as `MatchQuality` (M10) and `potential_leaks` itself.
- **Estimated slice count:** 2–5 slices (4 shipped: 13.A–13.C implementation + 13.D doc-sync).

### M14 — UI Backend-Parity & AI-Native Redesign (Usability, Parity-Closing) — ✅ Shipped

- **Status:** ✅ **Shipped** — design doc [milestone-14-ui-parity-ai-native.md](design/milestone-14-ui-parity-ai-native.md), slices 14.A–14.D (implementation) + 14.E (visual-consistency pass + documentation sync, this pass). The first UI-track milestone in this roadmap's M8+ series — every prior milestone in this series (M8/M9/M10/M11/M13) deliberately deferred UI work, and this is that accumulated backlog.
- **Theme:** Close the gap between what core/CLI/MCP can already do (M8–M13) and what the browser UI can show, while restructuring navigation around an AI-guided default with the full MAT-equivalent power surface kept undiminished underneath.
- **Goal:** Every M8–M13 backend capability gets a UI surface. Navigation gets an AI-guided landing layer (triage/chat/NL-query, backed by M11 workflows) as the default entry point, with today's power views (dominator tree, object inspector, query console, leak workspace) fully intact and one click away — accelerator, not replacement.
- **Delivered scope (as shipped; four dedicated host bridges resolved the design doc's own §6.1 open question — see note):**
  - **Comparison basket UI (Slice 14.A):** new `/compare` route, `ui/src/features/comparison/` (`ComparisonPicker`, `MatchQualityBadge`, `ObjectDeltaTable`, `comparison-store.ts`). Loads a precomputed `mnemosyne diff --mode object --format json` report or runs a live diff via a brand-new, dedicated `__MNEMOSYNE_COMPARISON_BRIDGE__.diffObjects()` — not an extension of either existing bridge, since neither is scoped to a before/after snapshot pair. Real Rust wire-shape verified against actual CLI output (built the binary, ran `diff --mode object --format json` against synthetic fixtures): JSON keys are snake_case as-shipped everywhere in this codebase (no `#[serde(rename_all = "camelCase")]` anywhere), enum variants serialize as Rust PascalCase (`"ClassDominator"`, `"RetainedChanged"`) — confirming the existing `analysis-types.ts` convention rather than inventing a new one.
  - **Object Inspector + GC-path extensions (Slice 14.B):** `HeapExplorerHostBridge.inspectObject(objectId, retainFieldData?)` — `ObjectInspectorPanel` renders dominator parent/children as clickable chips navigating to `/heap-explorer/object-inspector?objectId=...`, reusing the exact `Link` pattern the existing references/referrers sections already established. `LeakWorkspaceHostBridge.findAllGcPaths(objectId, maxPaths?)` — `LeakGcPathPage` gains a path-count selector (1/3/5/10/20, default 5) rendering every enumerated path plus a truncation notice, with the original single-path branch left completely untouched and gated behind `!findAllAvailable` — a bridge lacking either new method produces byte-identical output to pre-14.B.
  - **Referrer, classloader, and thread panels (Slice 14.C):** `ReferrerPanel`/`ClassloaderExplorerPanel` (new, in `ArtifactExplorerPage`) and `ThreadExplorerPanel` (new `/heap-explorer/threads` route, `HeapThreadsPage`) — all three purely artifact-backed (`AnalysisArtifact.referrerReport`/`.classloaderReport`/`.threadReport`), no new bridge calls. Real backend JSON shape verified against a hand-built HPROF fixture (`analyze --by-referrer --classloaders --threads --format json`): the response fields are `thread_report`/`classloader_report`/`referrer_report` (not a bare `threads` field as the design doc's illustrative §6 sketch had it), and object-id encoding is inconsistent *within* `thread_report` itself — `ReferrerEntry.objectId`/`FrameLocal.objectId` are hex strings (`"0x..."`) while `ThreadInfo.objectId` and every classloader-side object id are raw numeric `u64` — faithfully preserved as found, not normalized away.
  - **AI-guided landing + workflow cards + persistent nav (Slice 14.D):** a fourth dedicated bridge, `__MNEMOSYNE_WORKFLOW_BRIDGE__` (`describeWorkflow`/`startWorkflow`/`nextStep`/`listSnapshots`) — not an extension of any of the other three, since workflow orchestration and snapshot-cache introspection don't fit a single-heap, single-leak, or before/after-pair shape. `TriageSummaryCard` drives M11's `triage_memory_leak` end-to-end through the shared generic `WorkflowCard`; `NaturalLanguageInputBar` routes OQL-shaped input (leading `SELECT`, or an OQL-only operator like `=~`) to the existing query-console execution path and everything else to `start_workflow` with a small keyword table (`natural-language-router.ts`), defaulting to `triage_memory_leak` when nothing matches; `WorkflowCards` surfaces `tune_gc`/`traverse_object_graph` directly and deep-links `compare_snapshots` to the Slice 14.A `/compare` route instead of driving `start_workflow` itself (that workflow's first step needs a before/after pair the landing page has no natural source for); `RecentHeapsList` (`list_snapshots`) renders nothing at all when unavailable, per the design doc's own explicit scope note. `TopNav` (persistent navigation to every power route) is rendered directly inside `ArtifactLoaderPage`'s `/` route rather than as a shared layout wrapping every route in `router.tsx` — a shared-layout wrapper was tried first and reverted because it collided with several pre-existing pages' own in-page navigation links sharing the same accessible names ("Dashboard", "Artifact Explorer", "Heap Explorer"), breaking those pages' own pre-existing tests. `ArtifactLoaderPage`'s existing drop-flow markup, copy, and auto-navigate-to-`/dashboard` behavior are completely unchanged — `GuidedLanding` is appended below it, not swapped in.
  - **Visual-consistency pass + documentation sync (Slice 14.E, this pass):** reviewed all ~15 new/extended Slice 14.A–14.D components against `ui/src/app/globals.css`'s tokens and against each other. Found and fixed one small drift: `NaturalLanguageInputBar` and `RecentHeapsList` each rendered their own `<h3>` nested directly under `GuidedLanding`'s own `<h3>` section heading (an a11y heading-hierarchy violation, and inconsistent with `TriageSummaryCard`/`WorkflowCards`' sibling content, which sits at `<h4>` under that same parent) — both demoted to `<h4>`. Everything else already matched the established panel/card/table/badge conventions from the pre-existing M4-era `DashboardPage`/`ArtifactExplorerPage` (border `#1e293b`, panel radius 24 with the `rgba(15,23,42,.96)→rgba(2,6,23,.96)` gradient, card radius 16 on `rgba(2,6,23,.75)`, `#38bdf8` uppercase eyebrow labels, `#94a3b8` body text, identical table header/row/badge-pill styling) — confirmed, not assumed, by reading every touched file. One apparent inconsistency was investigated and confirmed to be a **pre-existing, internally-consistent sub-convention, not drift**: the entire `leak-workspace` subpage family (`LeakExplainPage`/`LeakFixPage`/`LeakSourceMapPage`/`LeakGcPathPage`) uses a different card-border color (`#334155`) than the rest of the app, consistently, predating M14 — left untouched rather than "fixed" into a mismatch with its own siblings.
- **Out of scope (unchanged from original text):** New backend capability (this milestone surfaces what already exists — zero `core`/`cli` files changed across 14.A–14.E). MAT backend-parity gaps (OQL depth, duplicate arrays, group-by-superclass, plugin runtime) — M15. Desktop packaging — M16. Tauri native-command wiring for any of the four new/extended bridges (`diffObjects`, `inspectObject`, `findAllGcPaths`, the workflow bridge's four methods) — `tauri/src/bridge.ts`/`commands.rs` inject none of them yet; every new UI surface degrades to an explicit unavailable state without a live bridge, exactly like every pre-existing optional-capability method.
- **Why now / strategic rationale:** User-requested, brainstormed 2026-08-20. Every prior M8–M13 milestone deliberately deferred UI work ("backend-before-UI" pattern) — this milestone is the accumulated backlog from that pattern, not new scope invention. Highest usability leverage per unit of work: the backend risk was already retired: this was presentation-layer work only. Also resolves three specific UI-deferral notes those milestones' own sections carried: M8's "UI surfaces beyond what the existing leak workspace already covers" (Slices 14.B/14.C shipped exactly that), M9's "No UI/Tauri surfacing" (Slice 14.D's `RecentHeapsList` shipped the browser snapshot picker), and M13's "UI leak-workspace panel (downgraded to documented future work)" (Slice 14.C shipped `ClassloaderExplorerPanel` in Artifact Explorer, not the leak workspace specifically, but the same underlying `duplicate_classes`/`ancestor_chain` data now has a browser surface).
- **Success criteria (met):** Every listed backend surface has a working UI page verified in-browser and by its own test file. Guided landing and power views are both reachable within one click of each other at all times (`TopNav`'s `POWER_ROUTES`). No regression to existing M4–M6 UI routes — `bun run test` stayed green across all four implementation slices, and stands at 285 passing tests (112+85+45+43 across four batches) at this Slice 14.E closeout, with `tsc --noEmit` clean throughout.
- **Risks / dependencies:** UI work has no existing test-suite precedent as heavy as the Rust side in this repo — mitigated by keeping `bun run test`/`tsc --noEmit`/in-browser verification as a hard gate per slice, same discipline as `cargo test`/clippy/fmt on the Rust side; held for all five slices. Scope creep into a second design language — mitigated by treating `globals.css`'s existing tokens as the source of truth, extended not replaced; verified directly in Slice 14.E rather than assumed.
- **Estimated slice count:** 5–8 slices (5 shipped: 14.A–14.D implementation + 14.E visual-consistency-and-doc-sync).

### M15 — MAT Backend Parity Completion (Parity-Closing)

- **Status:** 🔲 Pending — design doc to be authored.
- **Theme:** Close the remaining backend/analysis gaps against Eclipse MAT identified in the parity matrix (§2): OQL depth, duplicate primitive-array detection, group-by-superclass, and a custom plugin/extension runtime.
- **Goal:** No MAT analysis capability left un-mirrored in core/CLI/MCP, absorbing the deferred B1/B9 backlog items below as this milestone's actual scope rather than indefinite deferral.
- **Scope:**
  - OQL depth: subqueries, `UNION`, multi-hop traversal, full predicate functions, `eval(...)`, regex `=~`, `dominators(...)`, `outbounds`/`inbounds` traversal — closing the remaining ~70% of MAT's OQL surface beyond M7-4's targeted slice.
  - Duplicate primitive-array / boxed-array detection, mirroring the existing `analyze_strings()` duplicate-group shape.
  - `--group-by superclass` (and the related "group by class → superclass tree").
  - Custom plugin/extension runtime (MAT's `IQuery` equivalent) — builds on the existing design reference `docs/design/m6-plugin-extension-system.md`.
- **Out of scope:** UI surfacing of any of the above (that's a follow-on to M14's pattern, scoped when this milestone's backend lands). Live JVM interaction (unrelated to OQL/grouping/plugins).
- **Why now / strategic rationale:** These are the last four rows in the MAT parity matrix (§2) still marked 🟡/❌ that are pure analysis-capability gaps (M7-5's benchmark rerun, tracked separately as M12, is a credibility/evidence gap, not a capability gap). Closing them retires the "Path to MAT" framing's remaining honest caveats.
- **Success criteria:** Each of the four scope items has core+CLI+MCP+test+doc coverage matching the M8–M13 bar (design doc, TDD, `cargo {check,test,clippy,fmt}` clean, doc sync).
- **Risks / dependencies:** OQL depth is explicitly flagged in this roadmap's own risk register as "treadmill risk" (§8) — mitigate by shipping it as a bounded, named slice list (not an open-ended "improve OQL" task) the same way M7-4's targeted slice was scoped. Plugin runtime is the highest-risk/highest-effort item in this milestone — consider sequencing it last within M15, or splitting it into its own M15-B if scope proves too large once designed.
- **Estimated slice count:** 6–10 slices.

### M16 — Desktop Packaging & Distribution (Adoption)

- **Status:** 🔲 Pending — design doc to be authored.
- **Theme:** Ship Mnemosyne as a downloadable, installable desktop application (parity with how users obtain and run Eclipse MAT today), not just a CLI binary + optional dev-mode Tauri shell.
- **Goal:** A user can download one installer per platform (Windows/macOS/Linux), run it, and get the full M14 GUI without installing Rust, Node, or building from source — absorbing the deferred B7 backlog item as this milestone's actual scope.
- **Scope:**
  - Harden the existing `tauri/` scaffold (already wraps the shared `ui/` build and injects host bridges) into a release-grade build: signed installers where platform tooling requires it, auto-update story (or an explicit documented decision to skip auto-update for v1), and inclusion in the tagged-release pipeline alongside the existing 5-target CLI archives / GHCR / Homebrew channels.
  - Bundle the M14 UI (this milestone depends on M14 having shipped a GUI worth bundling — sequencing note, not a hard blocker on individual M14 slices).
- **Out of scope:** New native commands beyond what `tauri/src/commands.rs` already exposes (`load_heap`, `query_heap`, `get_references`/`get_referrers`, `explain_leak`, `find_gc_path`, `map_to_code`, `propose_fix` — extend only if M14 UI work surfaces a gap). Auto-update infrastructure unless scoped in explicitly during design.
- **Why now / strategic rationale:** User-requested for adoption — "for easy adoption we could also package and make this downloadable with gui like eclipse mat." The scaffold already exists (M6); this is release-hardening, not new architecture, matching the "adoption-data dependent" B7 backlog note this roadmap already carried.
- **Success criteria:** A signed (or documented-as-unsigned-with-rationale) installer exists per target platform, downloadable from GitHub Releases alongside the CLI archives, launches the M14 GUI, and loads/analyzes a real heap dump end-to-end without any local Rust/Node toolchain.
- **Risks / dependencies:** Code-signing requires platform-specific credentials/certificates this environment may not have — flag early in design rather than discovering it mid-slice, same "check the environment before promising the milestone" discipline M12 already established when it turned out to be blocked here. Depends on M14 shipping a GUI worth packaging.
- **Estimated slice count:** 4–6 slices.

### Other backlog items (lower priority — not proposed as standalone M8+)

| # | Item | Origin | Priority | Notes |
|---|---|---|---|---|
| B2 | Property-based parser testing | M7 deferred | P2 | `proptest` for binary parser robustness — fold into M8 hardening slice |
| B3 | Byte-accurate progress bars | M7 deferred | P3 | Polish — fold into M8 |
| B4 | Incremental leak tracking (3+ snapshots) | D3 | P2 | Depends on M9 + M10; consider M17 |
| B5 | IDE-native memory annotations (LSP / VS Code) | D4 | P3 | Differentiator; consider M17+ |
| B6 | Smart heap reduction advisor | D5 | P3 | Builds on M8 + M10 |
| B8 | Streaming responses (MCP) | M5 follow-on | P3 | Only if evidence shows need |

B1 (full OQL expansion) and B9 (custom plugin/extension runtime) are promoted from this backlog into **M15**'s actual scope, above. B7 (Tauri desktop release, signed) is promoted into **M16**.

---

## 6. Recommended Next Milestone — **M14 UI Backend-Parity & AI-Native Redesign**

**Status update (2026-09-13):** M8, M9, M10, M10-B, M11, M13, and now **M14** are all shipped as of this update — the sequencing below this note is now history, preserved for the record. M12 remains blocked (no native-Linux + Eclipse MAT reference workstation available in the executing environment). With M14's UI-backfill closed out, the active recommendation moves to M15 (MAT backend parity completion), followed by M16 (desktop packaging, which depends on M14 having shipped a GUI worth bundling — it now has).

**Recommendation (historical, at the time M14 was scheduled):** Schedule **M14 — UI Backend-Parity & AI-Native Redesign** as the active next milestone, followed by M15 (MAT backend parity completion) and M16 (desktop packaging).

**Justification:**

1. **Retired backend risk, presentation-only work left:** every M8–M13 milestone deliberately deferred UI work under a "backend-before-UI" pattern (see each milestone's own "Not shipped" notes above). That backlog is now the single highest-leverage piece of remaining work — the analysis capability already exists and is tested; M14 is surfacing it, not inventing it.
2. **User-requested, explicitly scoped via brainstorming session (2026-08-20):** AI-guided default navigation with the full MAT-equivalent power surface kept undiminished underneath — see [milestone-14-ui-parity-ai-native.md](design/milestone-14-ui-parity-ai-native.md).
3. **M15 (remaining OQL/array/superclass/plugin gaps) and M16 (desktop packaging) sequenced after M14** because M16 explicitly depends on M14 having shipped a GUI worth packaging, and M15's backend gaps don't block M14's UI-backfill scope (M14 surfaces what's already shipped, not what M15 will add).

**Recommended sequencing (current):**

1. **M14** — UI Backend-Parity & AI-Native Redesign (active)
2. **M15** — MAT Backend Parity Completion (independent of M14; can run in parallel in a separate worktree track if capacity allows)
3. **M16** — Desktop Packaging & Distribution (depends on M14 shipping a GUI)

**Historical sequencing (M8–M13 era, completed):**

1. ~~M8~~ — Reachability & References Deep Dive — ✅ shipped
2. ~~M12~~ — Reference-workstation rerun — blocked (environment)
3. ~~M9~~ — Snapshot persistence — ✅ shipped
4. ~~M10~~ — Object-level diff — ✅ shipped (+ M10-B follow-up)
5. ~~M11~~ — MCP workflow suite — ✅ shipped
6. ~~M13~~ — Classloader explorer — ✅ shipped
7. ~~M14~~ — UI backend-parity & AI-native redesign — ✅ shipped

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
| **MCP / IDE integration** | ✅ MAT has none | Extended in **M11** ✅ shipped (5 workflow-lifecycle tools, 4 workflow kinds) |
| MCP workflow suite (`triage_memory_leak`/`tune_gc`/`traverse_object_graph`/`compare_snapshots`) | ✅ MAT has no agent-facing workflow surface at all | **M11** ✅ shipped |
| **Streaming overview mode** | ✅ MAT has none | M7-1 ✅ |
| **Allocation flame graphs** | ✅ MAT has none natively | M7-3 ✅ |
| Browser UI surface for M8/M9/M10/M13 backend capability (referrer/classloader/thread panels, object-inspector chip navigation, GC-path multi-view, comparison basket) | ✅ | **M14** ✅ shipped (Slices 14.A–14.C) |
| AI-guided workflow landing (triage summary card, NL-query router, workflow cards, snapshot picker, persistent power-route nav) | ✅ MAT has no equivalent | **M14** ✅ shipped (Slice 14.D) |

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
| **M10 Object-Level Diff** | [design/milestone-8-1-object-level-diff.md](design/milestone-8-1-object-level-diff.md), [design/milestone-10-b-object-growth-policy.md](design/milestone-10-b-object-growth-policy.md) | ✅ Shipped (slices A–G implementation, H doc-sync, M10-B `ci-check`/leak-progression follow-up) |
| **M11 MCP Workflow Suite** | [design/milestone-11-mcp-workflow-suite.md](design/milestone-11-mcp-workflow-suite.md) | ✅ Shipped (slices 11.A–11.D implementation, 11.E doc-sync) |
| **M12 Reference-Workstation Re-run** | reuse [design/milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md) | ⏳ Pending |
| **M13 Classloader Explorer** | [design/milestone-13-classloader-explorer.md](design/milestone-13-classloader-explorer.md) | ✅ Shipped (slices 13.A–13.C implementation, 13.D doc-sync) |
| **M14 UI Backend-Parity & AI-Native Redesign** | [design/milestone-14-ui-parity-ai-native.md](design/milestone-14-ui-parity-ai-native.md) | ✅ Shipped (slices 14.A–14.D implementation, 14.E visual-consistency + doc-sync) |
| **M15 MAT Backend Parity Completion** | _to be authored_ | ⏳ Pending |
| **M16 Desktop Packaging & Distribution** | _to be authored_ | ⏳ Pending |

---

For milestone history, completed-batch detail, prior backlog tables, the original M7 proposal, and the full v0.2.0 state snapshot, see [roadmap-archive.md](roadmap-archive.md).
