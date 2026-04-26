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
| Path to GC roots — **all paths** / by class | ✅ | ❌ | Only shortest path is exposed | **High (M8)** | MAT supports "merge shortest paths to GC roots from class X". Mnemosyne has the graph primitives but no all-paths or by-class projection. |
| Leak suspects report (heuristic) | ✅ | ✅ | None | — | `detect_leaks()` ships graph-backed retained-size + accumulation-point ranking with heuristic fallback labeled via `ProvenanceKind::Fallback`. **Differentiator:** structured provenance markers vs MAT's opaque suspect text. |
| OQL — full operator set | ✅ | 🟡 | M7-4 covers ~30% of MAT OQL surface | Medium (M8 / M9) | Shipped: `@retainedSize`, `@toString`, `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS x.field`, `IS NULL`. Missing: subqueries, `UNION`, multi-hop traversal, full predicate functions, `eval(...)`, regex `=~`, `dominators(...)`, `outbounds`/`inbounds` traversal. |
| Top consumers report | ✅ | ✅ | None | — | `find_top_instances()` + analyze report top-N largest instances by retained or shallow size. |
| Class loader explorer (per-loader histogram, unique classes) | ✅ | 🟡 | `analyze --classloaders` exists; no dedicated leak-detection or unique-classes-per-loader explorer view | Medium (M11) | Per-loader retained-size aggregation is shipped. Missing: classloader leak detection (multiple loaders for same class — the classic Tomcat/webapp leak), unique-classes-per-loader histograms, parent-loader tree drill-down. |
| Duplicate strings / arrays detection | ✅ | 🟡 | Strings yes; arrays no | Low (M11) | `analyze_strings()` reports duplicate groups + dedup waste. **No** equivalent for primitive arrays or boxed array dedup. MAT has both. |
| Thread overview + frame-locals + stack | ✅ | 🟡 | Stacks + thread-local counts shipped; no per-frame local-variable detail | Medium (M8) | `inspect_threads()` parses `STACK_TRACE` / `STACK_FRAME`, correlates `ROOT_THREAD_OBJECT`, and reports retained bytes. Missing: per-frame **local variable values** (MAT's "frame-as-pseudo-object" feature requires `ROOT_JNI_LOCAL` / `ROOT_JAVA_FRAME` cross-referencing). |
| Inspector — object field-level browse, refs in/out | ✅ | 🟡 | API + UI shipped; no dedicated CLI surface | Low | `ObjectGraph::get_object/get_references/get_referrers` and the React Object Inspector cover this. Missing: a focused `mnemosyne inspect <object-id>` CLI + MCP method. |
| Group by class / classloader / package / superclass | ✅ | 🟡 | Class / package / classloader yes; superclass no | Low (M11) | `--group-by class\|package\|classloader`. Missing `--group-by superclass` (and the related "group by class -> superclass tree"). |
| **Group by referrer** (incoming references analysis) | ✅ | ❌ | Not shipped | **High (M8)** | MAT's "Show objects by incoming references" / "Group by referrer" is a top-3 MAT investigation workflow. Mnemosyne has `get_referrers()` but no aggregating analyzer or CLI surface. |
| Reachable / unreachable objects analysis | ✅ | ✅ | None | — | `find_unreachable_objects()` walks from GC roots and reports per-class counts + shallow size. |
| **Compare two heap dumps** (object-level diff) | ✅ | 🟡 | Class-level diff only | **High (M10)** | `diff_heaps()` ships record-level + class-level deltas with retained-size diffs. Missing: stable per-object identity tracking, growth-suspect ranking, leak-progression detection across snapshots. |
| Allocation-site flame graphs | 🟡 | ✅ | **Mnemosyne ahead** | — | MAT has no native flame-graph export; users typically pipe to async-profiler. Mnemosyne ships SVG / folded-stack / JSON natively. **Differentiator.** |
| Custom inspector views / extensions | ✅ | ❌ | Not shipped | Defer | MAT plugins (`org.eclipse.mat.api.IQuery`) are widely used. Mnemosyne has a plugin design doc (M6) but no runtime extension surface. Defer until adoption justifies. |
| **Index files / persistent snapshot** (parse-once, query-many) | ✅ | ❌ | Not shipped | **High (M9)** | MAT's `.index` artifacts make re-open near-instant. Mnemosyne re-parses on every invocation. This is the single biggest UX gap for repeat triage workflows. |
| **CI/CD-native automation** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has no first-class CI gate. `ci-check` + JSON / JUnit / GitHub Actions output is unique. **Differentiator.** |
| **Streaming bounded-memory mode** | 🟡 | ✅ | **Mnemosyne ahead** | — | MAT has `ParseHeapDump.sh` for batch indexing, but no truly streaming bounded-RSS triage on multi-GB dumps. **Differentiator.** |
| **AI-assisted diagnosis** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has none. Mnemosyne ships rules / stub / provider modes, prompt redaction, audit log, CLI `chat`, persisted MCP sessions. **Differentiator.** |
| **MCP-native IDE integration** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT is Eclipse-only. Mnemosyne ships 14 MCP methods with structured errors and session lifecycle. **Differentiator.** |
| **Provenance contract** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT has no equivalent. `ProvenanceKind { Synthetic, Partial, Fallback, Placeholder }` rendered across all non-JSON formats. **Differentiator.** |
| **Single-binary distribution, no JVM** | ❌ | ✅ | **Mnemosyne ahead** | — | MAT requires JVM + Eclipse RCP. Mnemosyne ships static binaries via 4 channels. **Differentiator.** |

### Parity matrix summary

- **Mnemosyne ≥ MAT:** allocation flame graphs, CI/CD automation, streaming bounded-memory mode, AI-assisted diagnosis, MCP/IDE integration, provenance, distribution.
- **MAT ≥ Mnemosyne (high priority):** all-paths-to-GC-roots / by-class, group-by-referrer, object-level heap diff, persistent indexes / parse-once-query-many.
- **MAT ≥ Mnemosyne (medium priority):** full OQL depth, classloader leak detection, frame-locals on threads.
- **MAT ≥ Mnemosyne (low priority / defer):** duplicate-arrays, group-by-superclass, custom plugin runtime, dedicated CLI inspector.

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

### M8 — Reachability & References Deep Dive (Parity-Closing + Differentiator-Extending)

- **Theme:** Close the highest-value MAT investigation gaps that the existing graph already supports.
- **Goal:** Ship the three top MAT-only investigation workflows — all-paths-to-GC-roots / by-class, group-by-referrer (incoming references), and a focused object inspector surface — using the existing `ObjectGraph` + dominator infrastructure with provenance and overview-aware fallbacks.
- **Scope:**
  - `mnemosyne gc-path --all-paths` and `--by-class <class>` (path enumeration with budget caps and `ProvenanceKind::Partial` truncation markers).
  - New `analysis::referrers` analyzer + `mnemosyne analyze --by-referrer` and MCP `analyze_by_referrer` (incoming-reference aggregation, ranked by retained size).
  - `mnemosyne inspect <object-id>` CLI surface + MCP `inspect_object` (field values, refs in/out, dominator parent/children).
  - Thread frame-locals: cross-reference `ROOT_JAVA_FRAME` / `ROOT_JNI_LOCAL` into `inspect_threads()` output.
- **Out of scope:** Full MAT OQL `inbounds` / `outbounds` traversal (defer to M9-2). Custom IQuery plugin runtime. UI surfaces beyond what the existing leak workspace already covers.
- **Why now / strategic rationale:** These are top-3 MAT investigation workflows that Mnemosyne currently lacks despite having the graph primitives. They reuse M1 / M3 graph infrastructure with no new architectural risk. They directly improve leak-triage credibility, which is the project's core mission.
- **Success criteria:** `gc-path --all-paths` enumerates ≥10 paths under budget for the real-world fixture. `analyze --by-referrer` ranks the top-N retained-size referrers and survives golden-output regression tests. `inspect <id>` exposes field values + refs in/out for any reachable object. Frame-locals appear on at least the synthetic thread fixture. ≥30 new tests; clippy + fmt clean.
- **Risks / dependencies:** Path enumeration needs careful budget caps (combinatorial explosion). Frame-locals depend on `STACK_TRACE` records being present in the dump. Overview-mode behavior must surface a structured `feature_unavailable_in_overview_mode` error.
- **Estimated slice count:** 5–10 slices.

### M9 — Snapshot Persistence & Parse-Once-Query-Many (Parity-Closing)

- **Theme:** Eliminate re-parse latency for repeat triage workflows.
- **Goal:** Persist a verified, versioned, on-disk snapshot index after the first parse so subsequent commands re-open instantly. Mnemosyne's first answer to MAT's `.index` artifacts.
- **Scope:**
  - `core::snapshot` subsystem: serialize `ObjectGraph` (and selected analyzers' precomputed outputs) to disk under `~/.cache/mnemosyne/<heap-sha256>/` with version + schema check.
  - `mnemosyne snapshot save | load | list | rm` CLI surface.
  - All commands accept `--snapshot <path>` (or auto-discover by HPROF SHA-256).
  - MCP `open_snapshot` / `list_snapshots`.
  - Cache invalidation on schema-version mismatch (loud, with provenance).
- **Out of scope:** Cross-machine snapshot interchange (defer). Multi-snapshot in a single MCP session beyond explicit `open` (M10 territory).
- **Why now / strategic rationale:** This is the single biggest UX gap vs MAT for repeat workflows. It is also a force multiplier for M10 (heap diff) and M11 (MCP workflow suite), both of which need cheap snapshot re-open. Should land **after** M8 because the on-disk format must include the new analyzers M8 adds, otherwise we ship a snapshot format we'll have to re-version immediately.
- **Success criteria:** Re-open of a 1 GiB snapshot completes in <1s vs >10s re-parse. Schema-version mismatch fails loudly with a hint. Round-trip equality test for graph + analyzer outputs. ≥20 new tests.
- **Risks / dependencies:** Format stability — must version every embedded schema. Disk-quota awareness. Privacy: snapshots may contain string contents — apply same `[ai.privacy]` redaction option. Depends on M8 analyzer surfaces being stable.
- **Estimated slice count:** 5–10 slices.

### M10 — Compare Two Heaps (Object-Level Diff) (Parity-Closing + Differentiator-Extending)

- **Theme:** Stable per-object identity tracking + leak-progression detection across snapshots.
- **Goal:** Replace class-level-only `diff_heaps()` with stable per-object identity heuristics so users can detect "this exact object grew", "this collection accumulated K new entries", and "this leak suspect is now M× larger" across two snapshots.
- **Scope:**
  - Object-identity heuristics: GC-root path + class + dominator-chain hash; with explicit `ProvenanceKind::Partial` when the heuristic is uncertain.
  - `core::analysis::diff_objects()` returning per-object delta records.
  - `mnemosyne diff --object-level <a.hprof> <b.hprof>` CLI + MCP `diff_objects`.
  - Growth-suspect ranking: top-N objects by retained-size delta.
  - Leak-progression: cross-reference object-level diff with `detect_leaks()` outputs.
  - **Differentiator extension:** `ci-check` `object_growth_threshold` predicate — fail CI when a tracked object grows beyond a per-class limit between two snapshots.
- **Out of scope:** 3+ snapshot trend analysis (defer to M14). Cross-machine snapshot diff. Time-series database backend.
- **Why now / strategic rationale:** Closes a top-3 MAT gap **and** opens a category MAT does not own — heap-regression CI gating at object granularity. Pairs naturally with M9 (snapshot persistence is the prerequisite for cheap repeat diffs).
- **Success criteria:** `diff --object-level` identifies grown objects in a synthetic two-snapshot pair. `ci-check object_growth_threshold` fails on a deliberately leaking pair. False-positive rate documented and bounded with provenance.
- **Risks / dependencies:** Object identity is fundamentally heuristic without write-barrier instrumentation; honesty contract is critical. Depends on M9 for cheap snapshot re-open. Memory cost of two graphs in RAM (mitigate with overview-mode diff path).
- **Estimated slice count:** 5–10 slices.

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

### M13 — Classloader Explorer & Leak Detection (Parity-Closing)

- **Theme:** First-class classloader-leak detection (the Tomcat / Jetty / Spring webapp leak).
- **Goal:** Detect the classic "same class loaded by N classloaders" leak pattern and provide drill-down into per-loader uniqueness.
- **Scope:**
  - `analysis::classloader::detect_classloader_leaks()` returning duplicate-class signals with retained-size attribution.
  - `mnemosyne analyze --classloader-leaks` + MCP `detect_classloader_leaks`.
  - Per-loader unique-classes histogram + parent-loader tree drill-down.
  - `ci-check` `classloader_leak_count` predicate.
  - UI: classloader leak panel in the leak workspace.
- **Out of scope:** Custom plugin runtime. Group-by-superclass histograms (defer).
- **Why now / strategic rationale:** Closes a real MAT capability that matters specifically for JVM webapp / app-server users — a non-trivial slice of Mnemosyne's target audience. Builds cleanly on the M3 classloader report.
- **Success criteria:** Detects a deliberately-leaked classloader on a synthetic webapp fixture. Per-loader histograms render in CLI + UI. `ci-check` predicate works.
- **Risks / dependencies:** Requires representative webapp-leak fixtures (build under `examples/` or `resources/test-fixtures/`).
- **Estimated slice count:** 2–5 slices.

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
| GC root paths — all paths / by class | ❌ | **M8** |
| Group by referrer | ❌ | **M8** |
| Object inspector (CLI/MCP) | 🟡 (UI only) | **M8** |
| Thread frame-locals | 🟡 | **M8** |
| Object-level heap diff | 🟡 (class-level only) | **M10** |
| Persistent indexes / parse-once-query-many | ❌ | **M9** |
| Classloader leak detection | 🟡 (per-loader histogram only) | **M13** |
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
| **M8 Reachability & References** | _to be authored by Design Consulting_ | ⏳ Pending |
| **M9 Snapshot Persistence** | _to be authored by Design Consulting_ | ⏳ Pending |
| **M10 Object-Level Diff** | _to be authored by Design Consulting_ | ⏳ Pending |
| **M11 MCP Workflow Suite** | _to be authored by Design Consulting_ | ⏳ Pending |
| **M12 Reference-Workstation Re-run** | reuse [design/milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md) | ⏳ Pending |
| **M13 Classloader Explorer** | _to be authored by Design Consulting_ | ⏳ Pending |

---

For milestone history, completed-batch detail, prior backlog tables, the original M7 proposal, and the full v0.2.0 state snapshot, see [roadmap-archive.md](roadmap-archive.md).
