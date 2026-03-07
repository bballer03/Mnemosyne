# Mnemosyne Roadmap & Milestones

> **Last updated:** 2026-03-07 (post M1 completion — all batches delivered)
> **Owner:** Tech PM Agent
> **Status:** Living document — updated after each major implementation batch

---

## Table of Contents

1. [Executive Summary](#section-1--executive-summary)
2. [Current State Assessment](#section-2--current-state-assessment)
3. [Gap Analysis](#section-3--gap-analysis)
4. [Eclipse MAT Feature Parity Analysis](#section-4--eclipse-mat-feature-parity-analysis)
5. [UI / Product Experience Strategy](#section-5--ui--product-experience-strategy)
6. [Differentiation Opportunities](#section-6--differentiation-opportunities)
7. [Feature Proposals](#section-7--feature-proposals)
8. [Roadmap](#section-8--roadmap)
9. [Milestones Detail](#section-9--milestones-detail)
10. [Suggested Implementation Order](#section-10--suggested-implementation-order)
11. [Risk Register & Open Questions](#section-11--risk-register--open-questions)

---

## Section 1 — Executive Summary

Mnemosyne is today an **alpha-stage Rust-based JVM heap analysis tool**. It can stream-parse HPROF files to produce class histograms and heap summaries, parse binary HPROF records into a full object reference graph (`core::hprof_parser` → `core::object_graph`), compute a real dominator tree via Lengauer-Tarjan (`core::dominator`), derive retained sizes from post-order subtree accumulation, run graph-backed analysis in both `analyze_heap()` and `detect_leaks()` with automatic fallback to heuristics when parsing fails, trace GC root paths with `ObjectGraph` BFS first plus layered fallbacks, expose object navigation via `get_object(id)`, `get_references(id)`, and `get_referrers(id)`, generate template-based fix suggestions, and render results in five output formats (Text, Markdown, HTML, TOON, JSON) — all backed by a provenance system that labels every synthetic, partial, fallback, or placeholder data surface. A stdio MCP server exposes six JSON-RPC handlers (`parse_heap`, `detect_leaks`, `map_to_code`, `find_gc_path`, `explain_leak`, `propose_fix`), making the tool available inside VS Code, Cursor, Zed, JetBrains, and ChatGPT Desktop. The AI module (`generate_ai_insights`) is **fully stubbed**: it returns deterministic template text with zero LLM calls and zero HTTP client dependencies.

Mnemosyne has the foundations to become **the first Rust-native, AI-assisted heap analysis platform** that rivals Eclipse MAT in analysis depth while offering capabilities no existing tool provides: provenance-tracked outputs that distinguish real analysis from heuristic guesses, MCP-native IDE integration for copilot-style workflows, CI/CD-friendly automation via structured JSON and TOON output, and an AI-native architecture designed from day one for LLM integration. The Rust core means multi-GB heap dumps can be processed with predictable memory usage and no GC pauses — a meaningful advantage over Java-based tools like MAT and VisualVM for production incident response.

Five properties position Mnemosyne to stand out in a crowded JVM tooling ecosystem: **(1)** Rust performance enabling streaming analysis of heap dumps that exceed host RAM; **(2)** a provenance system unique among heap analyzers, giving users and automation confidence in result trustworthiness; **(3)** MCP-first architecture that makes heap analysis a conversation in the developer's IDE rather than a separate tool; **(4)** AI-native design with well-shaped type contracts (`AiInsights`, `AiWireExchange`, config plumbing) ready for LLM wiring; and **(5)** automation-friendly structured output (JSON, TOON) enabling CI regression detection with machine-readable leak signals.

Honest assessment: **significant work remains** to deliver on this vision, but Milestone 1 is now complete and the foundational analysis engine is in place. The object graph, dominator tree, retained-size computation, unified `detect_leaks()` path, GC-path rewrite, and object navigation API are all delivered. The most critical remaining gap is AI wiring: every "AI-powered" claim in the README is aspirational today. An 80-test suite (59 core + 5 CLI unit + 16 CLI integration) now runs clean in GitHub Actions CI, and the `test-fixtures` feature keeps canonical HPROF builders reusable across unit and integration coverage. There are no release binaries, no sample real-world heap dumps, and no benchmarks. The architecture is sound, the core analysis pipeline is graph-backed across the primary analysis surfaces, and the provenance/reporting layers are well-built — the next priorities are packaging, UX, and wiring real LLM calls.

---

## Section 2 — Current State Assessment

### Core Capabilities

| Capability | Status | Honest Assessment |
|---|---|---|
| HPROF streaming parser | ✅ Implemented | Two-tier parsing: `core::heap` streams headers + record tags for fast class histograms. `core::hprof_parser` now parses binary HPROF records (STRING_IN_UTF8, LOAD_CLASS, CLASS_DUMP, INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP, 8 GC root types) into a full `core::object_graph::ObjectGraph` with objects, references, classes, and GC roots. Uses `byteorder` for big-endian binary parsing. Both 4-byte and 8-byte identifier sizes supported. |
| Leak detection heuristics | ✅ Unified | `detect_leaks()` now attempts the same object-graph → dominator → retained-size path as `analyze_heap()`, then falls back to heuristics with `ProvenanceKind::Fallback` when graph parsing fails or filters exclude all candidates. Direct leak detection is no longer split from the graph-backed pipeline. |
| Graph / dominator tree | ✅ Complete | `core::dominator::build_dominator_tree()` runs Lengauer–Tarjan (`petgraph::algo::dominators::simple_fast`) over the full object reference graph with a virtual super-root connected to all GC roots. Computes retained sizes via post-order subtree accumulation. Exposed through `analyze_heap()` and `detect_leaks()`. `ObjectGraph` now also exposes `get_object(id)`, `get_references(id)`, and `get_referrers(id)` for navigable exploration. |
| GC root path tracing | ✅ Implemented | `core::gc_path` now tries full `ObjectGraph` BFS first, then falls back to a budget-limited `GcGraph`, then synthetic paths when heap data is insufficient. Field-name edge labels are preserved via `get_field_names_for_class()`. |
| AI / LLM insights | ❌ Stubbed | `core::ai::generate_ai_insights()` returns deterministic template text. No HTTP client in `Cargo.toml`, no API calls, no LLM SDK. Config plumbing exists (`AiConfig` with provider/model/temperature fields) but terminates at the stub. The "AI-powered" claim in README is entirely aspirational. |
| Fix suggestions | ⚠️ Template only | `core::fix::propose_fix()` generates template patches in three styles (Minimal, Defensive, Comprehensive). No AI involvement, no code analysis. Useful scaffolding with provenance markers. |
| Source mapping | ✅ Implemented | `core::mapper::map_to_code()` scans project dirs for `.java`/`.kt` files, runs `git blame` for metadata. Basic but functional for local projects. |
| Reporting | ✅ Implemented | `core::report` renders 5 formats (Text, Markdown, HTML, TOON, JSON). HTML output uses `escape_html()` for XSS prevention. TOON uses `escape_toon_value()` for control characters. Provenance markers rendered in all non-JSON formats. One of the most polished subsystems. |
| MCP server | ✅ Wired | `core::mcp::serve()` runs a stdio JSON-RPC loop with async Tokio I/O. Handles 6 methods. Works end-to-end but backed by the same stubs/heuristics as CLI. |
| Config system | ✅ Implemented | `cli::config_loader` reads TOML files from 5 locations + env vars + CLI flags. `core::config` defines `AppConfig`, `AiConfig`, `ParserConfig`, `AnalysisConfig`. Clean, well-layered. |
| Provenance system | ✅ Implemented | `ProvenanceKind` (Synthetic, Partial, Fallback, Placeholder) + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in all report formats and CLI output. Unique feature in the heap-analysis space. |

### Technical Strengths

- **Rust performance model**: streaming parser with `BufReader`, no GC, predictable memory. Can handle files larger than RAM in principle.
- **Clean module separation**: 13 focused modules in `core` (`ai`, `analysis`, `config`, `dominator`, `errors`, `fix`, `gc_path`, `graph`, `heap`, `hprof_parser`, `mapper`, `mcp`, `object_graph`, `report`, `test_fixtures`) with clear single-responsibility boundaries.
- **Real object graph**: `core::hprof_parser` parses binary HPROF records into an `ObjectGraph` with objects, reference edges, class metadata, and GC roots — the foundation for all graph-backed analysis.
- **Real dominator tree**: `core::dominator` implements Lengauer–Tarjan over the full object graph with virtual super-root. Computes retained sizes via post-order accumulation.
- **Graph-backed analysis pipeline**: `analyze_heap()` attempts object-graph → dominator-tree → retained-size analysis first, with automatic fallback to heuristics. Provenance markers distinguish real from heuristic results.
- **Streaming design**: `core::heap` parser processes HPROF records sequentially without loading the full dump. Foundation for scaling to multi-GB files.
- **Provenance system**: genuinely novel for a heap analyzer. Labels every synthetic/heuristic output surface so consumers know what to trust.
- **Multi-format output**: 5 report formats with consistent provenance rendering. HTML is XSS-hardened. TOON enables compact CI consumption.
- **80-test suite with CI**: 59 core + 5 CLI unit + 16 CLI integration tests running in GitHub Actions. Synthetic HPROF test fixtures plus the `test-fixtures` cargo feature enable deterministic parser, graph, and end-to-end CLI testing.
- **Config hierarchy**: TOML + env vars + CLI flags with clear precedence. Production-ready design pattern.
- **MCP integration**: stdio JSON-RPC server with 6 handlers. First-mover for heap analysis in the MCP ecosystem.
- **Type contracts**: well-shaped request/response types (`AnalyzeRequest`, `AnalyzeResponse`, `GcPathResult`, `FixResponse`, etc.) that establish stable contracts between CLI, MCP, and core.

### Major Weaknesses

- **AI is 100% stubbed**: `generate_ai_insights()` returns hardcoded template strings. There are zero HTTP client dependencies in `Cargo.toml`. The `AiConfig` fields (provider, model, temperature, API key) exist but connect to nothing. Every "AI-powered" claim in documentation is marketing ahead of implementation.
- **No benchmarks or performance data**: no `criterion` benchmarks for parser throughput, graph construction, dominator computation, or report rendering. Cannot track performance regressions or compare against MAT/VisualVM.
- **No release packaging**: build-from-source only. No `cargo install` publication, no pre-built binaries, no Homebrew formula, no Docker image.
- **No sample real-world data**: synthetic test fixtures exist for deterministic testing, but no example real `.hprof` files for tutorials or development.
- **Diff is record-level, not object-level**: `diff_heaps()` compares aggregate record/class statistics. It cannot track individual object migration or reference chain changes.
- **Graph module naming is misleading**: `summarize_graph()` still exists as a lightweight fallback that builds a synthetic tree from top-12 entries. Its name suggests more than it delivers, though the real dominator tree now exists alongside it.

### Maturity Assessment

| Subsystem | Maturity | Rationale |
|---|---|---|
| Parser | Alpha+ | `core::heap` handles record-level stats. `core::hprof_parser` parses object-level binary HPROF into a full `ObjectGraph`. Both 4-byte and 8-byte ID sizes supported. |
| Leak detection | Alpha+ | `detect_leaks()` and `analyze_heap()` now share the graph-backed retained-size pipeline with explicit heuristic fallback provenance. |
| Graph / Dominator | Alpha+ | Real Lengauer–Tarjan dominator tree implemented in `core::dominator`, retained sizes surfaced in both primary analysis paths, and `ObjectGraph` now exposes a navigation API. |
| AI | Pre-alpha | Fully stubbed. Returns deterministic text. Not wired to any model. |
| GC root paths | Alpha | Real parsing of roots/instances within budget. Best-effort with fallback. Among the strongest features. |
| Fix suggestions | Alpha | Template-based scaffolding. No code analysis or AI involvement. |
| Source mapping | Alpha | Works for basic cases. No IDE integration beyond file scanning. |
| Reporting | Beta | 5 formats, XSS hardening, provenance rendering, well-tested. Ready for use. |
| MCP server | Alpha | Wired and functional but outputs depend on stubs/heuristics. |
| Config | Beta | Clean hierarchy, env + TOML + CLI. Production-ready pattern. |
| Provenance | Beta | Unique, well-integrated across all surfaces. Novel in the space. |
| Testing | Alpha+ | 80 tests (59 core + 5 CLI unit + 16 CLI integration). Synthetic HPROF test fixtures, reusable `test-fixtures` feature, and GitHub Actions CI. No property-based testing or benchmarks yet. |
| CI/CD | Alpha | GitHub Actions CI runs `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check` on pushes and PRs. No release automation, no nightly builds. |

---

## Section 3 — Gap Analysis

### 3.1 Correctness & Trust Gaps

**Object reference graph: implemented and now used across the primary analysis surfaces.** `core::hprof_parser` parses INSTANCE_DUMP field values (using CLASS_DUMP field descriptors), OBJ_ARRAY_DUMP elements, PRIM_ARRAY_DUMP, and GC root records into a full `ObjectGraph` with reference edges. `core::dominator` computes a real dominator tree via Lengauer–Tarjan and derives retained sizes via post-order accumulation. `analyze_heap()` and `detect_leaks()` both use this pipeline, `gc_path` now prefers `ObjectGraph` BFS first, and `ObjectGraph` exposes `get_object(id)`, `get_referrers(id)`, and `get_references(id)` for exploration. The main remaining correctness gap is elsewhere:

- **Diff is record-level, not object-level.** `diff_heaps()` compares aggregate record/class statistics between two snapshots. It cannot track individual object migration, new allocation sites, or reference chain changes.

**Provenance correctly labels data quality** — the system labels graph-backed results with no provenance marker (clean data) and heuristic/fallback results with `ProvenanceKind::Fallback` or `ProvenanceKind::Partial`, so consumers know what to trust.

### 3.2 Testing & CI Gaps

- **80 tests** across the workspace (59 core + 5 CLI unit + 16 CLI integration). Tests cover provenance rendering, escape functions, analysis paths, HPROF parsing, object graph construction, dominator tree correctness, retained-size computation, CLI argument handling, and end-to-end command execution.
- **Synthetic HPROF test fixtures** exist in `core::test_fixtures`. Small deterministic binary HPROF files exercise the parser and graph pipeline without requiring a JVM or committing large binaries.
- **`test-fixtures` cargo feature** exposes canonical fixture builders to integration tests without widening the builder API surface.
- **CI pipeline running.** GitHub Actions (`.github/workflows/ci.yml`) runs `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and PRs.
- **16 end-to-end CLI integration tests.** `cli/tests/integration.rs` runs `parse`, `leaks`, `analyze`, `gc-path`, `diff`, `fix`, `report`, and `config` as subprocesses against synthetic HPROF fixtures and validates success/output behavior.
- **No integration tests against real `.hprof` files.** Tests use synthetic fixtures only. Real-world heap dumps from production JVMs are not tested.
- **No coverage tracking.** No `cargo-tarpaulin` or `cargo-llvm-cov` integration. Unknown actual coverage percentage.
- **No property-based testing.** Parser binary handling is a prime candidate for `proptest` or `quickcheck` fuzzing.
- **No benchmarks.** No `criterion` benchmarks for parser throughput, graph construction, dominator computation, or report rendering. Cannot track performance regressions.

### 3.3 Documentation & Onboarding Gaps

- **README claims partially accurate now.** "Advanced object graph & dominator analysis" and "retained size" are now implemented in `analyze_heap()`. "AI-generated explanations" remain aspirational. Progress bar output still does not exist.
- **QUICKSTART shows aspirational output.** Example output includes progress bars (`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100% • 3.2s`) that the tool cannot produce.
- **No API documentation.** `docs/api.md` exists but is a placeholder. `core` module public API has doc-comments on some items but no generated rustdoc site.
- **No tutorial or cookbook.** No guided walkthrough of a real analysis session. No examples of interpreting output or acting on leak candidates.
- **No troubleshooting guide.** No documentation for common errors, unsupported HPROF variants, or limitations.
- **No performance benchmarks published.** No data comparing Mnemosyne against MAT, VisualVM, or other tools.

### 3.4 Packaging & Release Gaps

- **Build from source only.** No published crate on crates.io, no pre-built binaries.
- **No `cargo install` support.** The `cli` crate doesn't publish a binary target to crates.io.
- **No release binaries.** No GitHub Releases, no CI-built artifacts for Linux/macOS/Windows.
- **No Homebrew formula.** No tap for macOS users.
- **No Docker image.** No container for CI/CD pipeline integration or incident response toolkits.
- **No version tags.** Git history has no version tags. `Cargo.toml` shows `0.1.0` but no tagged release.
- **No changelog automation.** `CHANGELOG.md` exists but is manually maintained with a single `[Unreleased]` section.

### 3.5 Feature Parity Gaps vs Eclipse MAT

Eclipse MAT is the de-facto standard for JVM heap analysis. With M1-B3/B4/B5 delivered, Mnemosyne now has the foundational analysis features (object graph, dominator tree, retained sizes) but is still missing many advanced MAT capabilities:

- **No browsable dominator view**: real dominator tree exists but is not exposed as an interactive explorer.
- **No OQL**: MAT provides Object Query Language for ad-hoc heap exploration.
- **No thread inspection**: MAT links thread stack traces to retained objects.
- **No classloader analysis**: MAT detects classloader leaks by analyzing the classloader hierarchy.
- **No collection inspection**: MAT inspects `HashMap`, `ArrayList`, etc. fill ratios and waste.
- **No unreachable object reporting**: MAT identifies objects not reachable from any GC root.
- **No histogram grouping**: MAT groups histograms by package, classloader, or superclass.
- **No object-level comparison**: MAT diffs two dumps at object/class granularity.

The gap is narrowing. With the object graph, dominator tree, retained sizes, unified leak detection, and navigation API delivered, Mnemosyne now has the analytical foundation. The next priorities are exposing these capabilities through richer surfaces (browsable views, query language) and closing the remaining feature gaps (MAT-style suspect ranking, histogram grouping, thread inspection).

### 3.6 UX & Usability Gaps

- **No progress bars.** Parsing large dumps shows no progress indication. The `anstream` dependency is present but unused for progress output. `indicatif` is not in the dependency tree.
- **Limited error messages.** Error types in `core::errors` use `thiserror` with basic messages. No suggestions for common mistakes (wrong file format, missing config, etc.).
- **No interactive mode.** No REPL or interactive exploration of results.
- **No color output.** `anstream` is a dependency but output is plain text. No color highlighting for severity levels, provenance markers, or key metrics.
- **No summary dashboards.** CLI output is sequential text. No at-a-glance summary view.
- **No table formatting.** Histograms and leak lists are printed as text. No aligned table output.

### 3.7 Ecosystem & Community Gaps

- **No issue templates.** GitHub repo has no `.github/ISSUE_TEMPLATE/` directory.
- **No PR templates.** No `.github/PULL_REQUEST_TEMPLATE.md`.
- **No CODE_OF_CONDUCT.** `CONTRIBUTING.md` exists but no code of conduct.
- **No security policy.** No `SECURITY.md` for vulnerability reporting.
- **No contributor ladder.** No documented path from first contribution to maintainer.
- **No example projects.** `docs/examples/README.md` exists but is a placeholder.
- **No benchmarks.** No performance comparison data against MAT, VisualVM, or YourKit.
- **No community infrastructure.** No Discord, Discussions, or mailing list.

---

## Section 4 — Eclipse MAT Feature Parity Analysis

| MAT Feature | Mnemosyne Status | Gap | Implementation Approach | Difficulty | Strategic Importance | Milestone |
|---|---|---|---|---|---|---|
| Dominator tree | ✅ Basic | Real Lengauer-Tarjan implemented; not exposed as browsable view | Expose via CLI subcommand + MCP handler + navigation API | Medium | Critical | M1 ✅ |
| Retained size | ✅ Basic | Computed from dominator tree, surfaced in `analyze_heap()` | Expose in more surfaces (`detect_leaks`, diff, histogram) | Medium | Critical | M1 ✅ |
| Object graph traversal | ✅ Basic | Object graph exists (`core::object_graph` + `core::hprof_parser`) and now exposes `get_object(id)`, `get_referrers(id)`, `get_references(id)`; no dedicated CLI/MCP browser yet | Expose navigation through richer CLI, MCP, and UI explorer surfaces | Medium | Critical | M1 ✅ |
| Shortest path to GC roots | ✅ Basic | `gc_path` now prefers full `ObjectGraph` BFS, then falls back to a budget-limited graph and synthetic path when necessary | Improve explorer surfaces and reduce fallback use on extreme dumps | Medium | High | M1 ✅ |
| Leak suspects report | ⚠️ Partial | Direct leak detection is now graph-backed, but the richer MAT-style suspect algorithm is not implemented | Find objects with disproportionate retained size relative to shallow size and accumulation patterns | High | Critical | M3 |
| Histogram by class/package/classloader | ⚠️ Partial | Record-level histogram only, no classloader or package grouping | Parse per-object data, group by fully-qualified class name, classloader, package | Medium | High | M3 |
| OQL / query language | ❌ Missing | No query capability | Design mini-query language or embed existing (e.g., SQL-like over object model) | Very High | High | M3 |
| Thread inspection | ❌ Missing | Not implemented | Parse HPROF STACK_TRACE + STACK_FRAME records, link threads to retained objects | High | Medium | M3 |
| ClassLoader analysis | ❌ Missing | Not implemented | Parse classloader hierarchy from CLASS_DUMP records, detect leaks per classloader | High | Medium | M3 |
| Collection inspection | ❌ Missing | Not implemented | Detect known collection types (`HashMap`, `ArrayList`, etc.), inspect fill ratio, size, waste | Medium | Medium | M3 |
| Export / reporting | ✅ Implemented | Good for current scope | Already strong: 5 formats, provenance, XSS hardening. Add CSV, protobuf, flamegraph later | Low | Medium | M2 |
| UI-based exploration | ❌ Missing | CLI only | Phase from TUI → static HTML → web UI → full explorer | Very High | High | M4 |
| Large dump performance | ⚠️ Partial | Streaming parser handles any size; in-memory object graph works but may require significant RAM for multi-GB dumps | Optimize with memory-mapped storage or chunked processing for very large dumps | High | High | M1 ⚠️ |
| Heap snapshot comparison | ⚠️ Partial | Record-level diff only | Diff at object/class level once object graph exists | Medium | Medium | M3 |
| Unreachable objects | ❌ Missing | Not implemented | After building reachability from GC roots, report unreachable set and sizes | Medium | Medium | M3 |

### Detailed Analysis per Feature

**Dominator Tree.**
*Current Status:* ✅ Implemented. `core::dominator::build_dominator_tree()` runs `petgraph::algo::dominators::simple_fast` (Lengauer–Tarjan) over the full object reference graph built by `core::hprof_parser`. A virtual super-root node connects to all GC roots. `core::graph::summarize_graph()` remains as a lightweight fallback for when the full graph is unavailable.
*Remaining Gap:* The dominator tree is consumed by `analyze_heap()` but is not exposed as a standalone CLI subcommand, MCP handler, or browsable view. Users cannot navigate the tree interactively.
*Next Steps:* Add a `mnemosyne dominators` CLI command and MCP handler. Expose `top_retained(n)`, tree-browsing queries, and integrate into the future web UI.
*Milestone:* Core algorithm delivered in M1-B4. Browsable view is M4.

**Retained Size.**
*Current Status:* ✅ Implemented. `core::dominator::build_dominator_tree()` computes retained sizes via post-order traversal, accumulating shallow sizes upward through the dominated subtree. `core::graph::build_graph_metrics_from_dominator()` populates `DominatorNode.retained_size` with real values. Both `analyze_heap()` and `detect_leaks()` now surface retained-size-backed leak insights when graph parsing succeeds.
*Remaining Gap:* Diff, histogram, and MCP surfaces still do not expose retained sizes as richly as the main analysis flows.
*Next Steps:* Expose retained sizes in `diff_heaps()` output, histogram views, and future explorer surfaces.
*Milestone:* Core computation and primary-surface integration delivered in M1. Broader surface integration moves to later milestones.

**Object Graph Traversal.**
*Current Status:* ✅ Basic. `core::hprof_parser` parses binary HPROF records into `core::object_graph::ObjectGraph` with `HeapObject` nodes (instances, object arrays, primitive arrays), `ClassInfo` metadata, `GcRoot` entries, and reference edges. The graph is consumed by `core::dominator` and `analyze_heap()`, and `ObjectGraph` now exposes `get_object(id)`, `get_referrers(id)`, and `get_references(id)`.
*Remaining Gap:* Navigation is available programmatically but not yet surfaced as a dedicated CLI, MCP, or UI explorer experience.
*Next Steps:* Expose the existing navigation API through richer CLI and MCP browsing surfaces. Foundation for the future web UI object inspector.
*Milestone:* Graph data structures and base navigation API delivered in M1. Richer explorer surfaces remain future work.

**Shortest Path to GC Roots.**
*Current Status:* ✅ Basic. `core::gc_path` now tries BFS over the complete `ObjectGraph` first, preserving richer edge labels, then falls back to a budget-limited graph and finally a synthetic path with provenance markers when heap data is insufficient.
*Remaining Gap:* Extreme dumps can still fall into the fallback tiers, and there is no dedicated visual exploration surface for path traversal yet.
*Next Steps:* Improve explorer surfaces and keep reducing fallback pressure on very large heaps.
*Milestone:* Core graph-backed path-finding delivered in M1.

**Leak Suspects Report.**
*Current Status:* `detect_leaks()` and `analyze_heap()` now produce graph-backed leak insights with retained-size data when graph parsing succeeds, with heuristic fallback when it does not.
*Remaining Gap:* MAT's leak suspect report goes further by finding objects whose retained size is disproportionately large relative to their shallow size and by identifying accumulation points with stronger explanatory evidence. That MAT-style ranking algorithm is still not implemented.
*Recommended Approach:* Build on the delivered retained-size pipeline to (1) find objects where retained_size >> shallow_size, (2) identify accumulation points (objects holding many references to the same class), and (3) generate suspect reports with reference chain context.
*Milestone:* Base graph-backed leak detection delivered in M1; advanced suspect ranking belongs to M3.

**Histogram by Class/Package/ClassLoader.**
*Current Status:* `HeapSummary.classes` contains `ClassStat` entries derived from record tags. No classloader or package-level grouping.
*Gap:* MAT groups histograms by package prefix, classloader identity, and superclass — enabling users to quickly scope analysis to their own code.
*Recommended Approach:* With per-object data from M1 (✅ delivered), group by FQN prefix (package), by classloader object ID (from CLASS_DUMP), and by superclass chain. Expose as query parameters on the histogram API.
*Milestone:* M3 — uses M1 object graph for classloader data.

**OQL / Query Language.**
*Current Status:* Not implemented. No query capability of any kind.
*Gap:* MAT's OQL allows `SELECT * FROM java.lang.String WHERE toString().length() > 1000` style queries. Extremely powerful for ad-hoc investigation.
*Recommended Approach:* Design a minimal query language (e.g., `SELECT class, retained_size FROM objects WHERE class LIKE 'com.example.%' AND retained_size > 1MB ORDER BY retained_size DESC`). Implement as a parser → AST → evaluator over the object store. Start with class/size filters, then expand to field access and predicates.
*Milestone:* M3 — requires M1 object graph (✅ delivered) and M3 histogram improvements as prerequisites.

**Thread Inspection.**
*Current Status:* Not implemented. HPROF STACK_TRACE and STACK_FRAME records are skipped during parsing.
*Gap:* MAT links threads to their retained objects, showing which threads hold memory and through what call stack.
*Recommended Approach:* Parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT records. Link thread objects to their stack traces and to objects reachable from thread-local roots.
*Milestone:* M3 — uses M1 object graph (✅ delivered) for object-to-thread linkage.

**ClassLoader Analysis.**
*Current Status:* Not implemented. CLASS_DUMP records are partially parsed in `gc_path` but classloader IDs are not stored or analyzed.
*Gap:* ClassLoader leaks (common in application servers and OSGi containers) cannot be detected without tracking the classloader hierarchy.
*Recommended Approach:* During CLASS_DUMP parsing, record the classloader reference for each class. Build a classloader tree. Detect leaks by finding classloaders that retain surprising amounts of memory.
*Milestone:* M3 — uses M1 object graph (✅ delivered).

**Collection Inspection.**
*Current Status:* Not implemented.
*Gap:* MAT detects under-utilized collections (e.g., `HashMap` with 16 buckets and 1 entry, or `ArrayList` with capacity 1000 and 2 elements). These waste significant memory at scale.
*Recommended Approach:* Identify known collection class names during object graph traversal. Inspect internal fields (e.g., `HashMap.table.length` vs `HashMap.size`) to compute fill ratio. Report collections with low fill ratios or excessive capacity.
*Milestone:* M3 — uses M1 object graph (✅ delivered) and field-value parsing.

**Large Dump Performance.**
*Current Status:* The streaming parser handles arbitrarily large files at the record level. The full object graph parser (`core::hprof_parser`) loads all objects into memory, which works well for dumps up to ~1GB but may require significant RAM for larger dumps.
*Gap:* A 4GB heap dump may contain 50M+ objects, each with multiple references. An in-memory adjacency list could require 10-20GB of RAM.
*Recommended Approach:* (1) Current in-memory graph works for dumps up to ~1GB. (2) For larger dumps, implement a two-pass strategy: first pass indexes object positions, second pass resolves references using memory-mapped access. (3) Consider disk-backed storage (e.g., `sled` or `redb` for an on-disk object store) for very large dumps.
*Milestone:* Basic in-memory graph delivered in M1. Optimized for large dumps in M3.

**Heap Snapshot Comparison.**
*Current Status:* `diff_heaps()` compares two `HeapSummary` values at the record/class-stat level. Reports delta bytes, delta objects, and changed classes.
*Gap:* MAT can diff at the object level, showing new objects, freed objects, and reference chain changes between snapshots.
*Recommended Approach:* With M1 object graphs now available, diff object sets by class and ID. Identify newly allocated objects, freed objects, and changed reference patterns. Report delta retained sizes per class.
*Milestone:* M3 — uses M1 object graph (✅ delivered).

**Unreachable Objects.**
*Current Status:* Not implemented.
*Gap:* MAT reports objects not reachable from any GC root, which helps understand phantom memory and finalizer pressure.
*Recommended Approach:* After building the object graph and GC root set, mark all reachable objects via BFS/DFS. Report unmarked objects with their classes and sizes.
*Milestone:* M3 — uses M1 object graph (✅ delivered) and GC root parsing.

---

## Section 5 — UI / Product Experience Strategy

### 5.1 Why a UI Matters

Heap analysis is a fundamentally **visual and exploratory task**. Developers investigating memory leaks need to navigate dominator trees, inspect reference chains, compare object counts, and drill into specific classes — activities that map poorly to sequential text output. A tree view of the dominator hierarchy, a searchable histogram, and a clickable reference chain are not nice-to-haves; they are how practitioners actually work. Eclipse MAT's success is inseparable from its Swing-based tree explorers and table views. For Mnemosyne to compete for adoption, it must eventually offer interactive exploration — but it must do so in a way that preserves the CLI-first, automation-friendly foundation.

### 5.2 Target Users

- **Primary: Java/Kotlin application developers** debugging memory issues during development or after production incidents. Need fast time-to-insight, clear explanations, and actionable fix suggestions. May use Mnemosyne through IDE (MCP) or directly via CLI.
- **Secondary: SREs and performance engineers** responding to production OOMs or memory regressions. Need CLI/CI-friendly tooling, structured output for automation, comparison between snapshots, and the ability to analyze multi-GB dumps efficiently under pressure.
- **Tertiary: CI/CD pipelines** running automated regression detection. Need JSON/TOON output with stable schemas, exit codes for threshold violations, and zero-interaction execution.

### 5.3 CLI-First Positioning

The CLI is the **stable foundation layer**. Every analysis capability must be accessible via CLI commands and programmatic API before it appears in any UI. The MCP server is a CLI-adjacent interface (same core, different transport). UIs are consumer layers that call the same core APIs. This ordering is non-negotiable:

1. Core library API (Rust)
2. CLI commands + MCP handlers
3. JSON/TOON structured output
4. Interactive UIs (TUI, web, desktop)

This ensures automation, testing, and integration are never second-class citizens.

### 5.4 Technology Tradeoffs

| Approach | Pros | Cons | Recommendation |
|---|---|---|---|
| TUI (ratatui) | Native Rust, SSH-friendly, fast, no browser required | Complex graph/tree navigation, limited visual fidelity, steep learning curve for rich views | Good for Phase UI-1: explorer mode |
| Static HTML | Zero server, shareable, portable, already partially implemented | No interactivity for large datasets, hard to do search/filter without JS | Phase UI-2: enhanced reports |
| Web UI (local) | Rich interactivity, modern UX, familiar tooling | Requires web framework dependency, more complex build, frontend skill needed | Phase UI-3: primary interactive explorer |
| Desktop (Tauri) | Native feel, full Rust backend, cross-platform | Complex build pipeline, platform-specific issues, distribution challenges | Phase UI-4 alternative |
| Web app (hosted) | Collaboration, no install, team sharing | Heap data contains sensitive application internals — security and privacy concerns | Future/optional, enterprise only |

### 5.5 Phased UI Plan

**Phase UI-1: CLI UX Improvements** (Milestone 2)
- Add progress bars for parsing and analysis with `indicatif` crate
- Colorized output using existing `anstream` dependency for severity levels, provenance markers, and key metrics
- Better error messages with suggestions and context (e.g., "file not found — did you mean heap.hprof?")
- Formatted table output for histograms and leak lists using `comfy-table` or similar
- Summary section at top of analysis output showing key metrics at a glance
- Stack: `indicatif`, `console`/`comfy-table`, existing `anstream`

**Phase UI-2: Static Interactive HTML Reports** (Milestone 4)
- Self-contained HTML file with embedded minified JavaScript
- Collapsible sections for leak details, object trees, and reference chains
- Search/filter within the report (client-side JS)
- Sortable tables for histograms and leak lists
- Provenance badges with color-coded severity (green=real, yellow=partial, orange=fallback, red=synthetic)
- Object graph mini-visualization using D3.js or similar (embedded)
- Stack: HTML template generation in Rust, embedded minified JS/CSS

**Phase UI-3: Lightweight Web UI** (Milestone 4)
- Local web server using `axum` (already familiar Tokio ecosystem)
- Upload or select heap dump file
- Real-time parsing progress via WebSocket or SSE
- Interactive dominator tree browser with drill-down
- Object graph explorer: click through reference chains
- Histogram explorer with group-by controls (class, package, classloader)
- Query interface when OQL lands
- Key screens:
  - **Dashboard**: heap size, object count, top consumers, leak suspect count
  - **Dominator Tree**: expandable tree with retained sizes, search, filter
  - **Object Inspector**: selected object detail with fields, references, referrers
  - **Leak Report**: ranked suspects with evidence chains
  - **GC Path Viewer**: visual path from object to GC root
  - **Query Console**: OQL input with results table
- Stack: `axum` + `htmx` (progressive enhancement) or React SPA, served from the Rust binary

**Phase UI-4: Full Interactive Heap Investigation** (Milestone 6+)
- Full object reference graph visualization (graph layout engine)
- Side-by-side dump comparison with visual diff
- AI conversation panel: ask questions about the heap in natural language
- Save and share analysis sessions
- Plugin architecture for custom analysis views
- Stack: Tauri desktop app or full web application

### 5.6 Key UI Views

Each of these views corresponds to a core analysis capability and should be designed together with its underlying API:

1. **Dashboard / Summary**: at-a-glance heap metrics, top consumers, leak count, provenance quality score
2. **Dominator Tree**: expandable tree with node = object/class, columns = shallow size, retained size, percentage, class name
3. **Histogram Explorer**: sortable/filterable table of classes with instance count, shallow size, retained size; group-by package/classloader toggle
4. **Leak Suspects**: ranked list of suspect objects with evidence (retained size, reference chains, accumulation patterns)
5. **GC Path Tracer**: visual path from target object back to GC root, with node types and field names on edges
6. **Object Inspector**: detail view for a single object: class, size, fields, outgoing references, incoming references (referrers)
7. **Diff View**: side-by-side comparison of two snapshots with delta highlighting
8. **Query Console**: text input for OQL with tabular results and export
9. **AI Insights Panel**: natural-language analysis, conversation thread, suggested actions

### 5.7 Usability Goals

- **Fast time-to-insight**: parse → top leak suspect visible in under 10 seconds for a 1GB dump
- **Obvious navigation**: clear visual hierarchy — dashboard → drill into leak → see reference chain → inspect object
- **Provenance always visible**: every data surface shows its provenance badge so users never mistake a heuristic for a fact
- **Responsive with large dumps**: UI must remain interactive even with millions of objects (virtual scrolling, lazy loading, server-side pagination)
- **Keyboard navigation**: power users should never need a mouse
- **Accessible colors**: severity and provenance indicators must use patterns/shapes in addition to color for accessibility

---

## Section 6 — Milestone Roadmap

### Milestone 1 — Stability & Trust
**Objective:** Make the core analysis trustworthy by building a real object graph, retained size computation, and dominator tree — the foundation everything else depends on.

**Why it matters:** Without a real object graph and retained sizes, Mnemosyne cannot make credible claims about memory analysis. This milestone delivers the analytical foundation.

**Status: ✅ COMPLETE — All batches delivered.**

**Delivered:**
1. ✅ Sample HPROF test fixtures — `core::test_fixtures` builds synthetic HPROF binaries for deterministic testing
2. ✅ Object graph data structures — `core::object_graph` defines `ObjectGraph`, `HeapObject`, `ClassInfo`, `GcRoot`, `FieldDescriptor`, etc.
3. ✅ Full object graph parser — `core::hprof_parser` parses INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP, CLASS_DUMP, GC root records into the indexed object store with reference edges
4. ✅ Real dominator tree — `core::dominator::build_dominator_tree()` implements Lengauer-Tarjan via `petgraph::algo::dominators::simple_fast` with virtual super-root
5. ✅ Retained size computation — post-order subtree accumulation in the dominator tree
6. ✅ Graph-backed analysis in `analyze_heap()` — attempts object-graph → dominator → retained-size pipeline first, falls back to heuristics with provenance markers
7. ✅ CI pipeline — GitHub Actions for build + test + clippy + fmt
8. ✅ Unified `detect_leaks()` onto the graph-backed path — attempts object graph + dominator analysis first, then falls back with explicit provenance
9. ✅ Rewrote GC path finder over the full object graph — `ObjectGraph` BFS first, then budget-limited `GcGraph`, then synthetic fallback
10. ✅ Added object graph navigation API — `get_object(id)`, `get_referrers(id)`, `get_references(id)`
11. ✅ Added 16 CLI integration tests plus reusable `test-fixtures` feature — 80 total passing tests across the workspace

**Dependencies:** None (this is the foundation)

**Modules/files affected:** `core/src/heap.rs`, `core/src/hprof_parser.rs`, `core/src/object_graph.rs`, `core/src/dominator.rs`, `core/src/graph.rs`, `core/src/analysis.rs`, `core/src/gc_path.rs`, `core/src/test_fixtures.rs`, `.github/workflows/ci.yml`

**Complexity:** Very High — this was the hardest milestone with the most new code. It is now fully delivered.

**Definition of done:**
- ✅ Can parse a real HPROF dump into a full object graph with reference edges
- ✅ Can compute retained sizes for any object
- ✅ Can produce a real dominator tree
- ✅ Leak detection uses retained-size data when available across both `analyze_heap()` and `detect_leaks()`
- ✅ GC path uses full object graph when available, with explicit layered fallback when it is not
- ✅ 80 tests pass (59 core + 5 CLI unit + 16 CLI integration)
- ✅ CI runs on every PR

---

### Milestone 2 — Packaging, Releases, and DX
**Objective:** Make Mnemosyne easy to install, use, and contribute to.

**Why it matters:** No one adopts a tool they can't easily install. Developer experience is the gateway to open-source adoption.

**Key Deliverables:**
1. Release automation — extend existing CI with cross-compile + publish workflow
2. Release binaries — Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
3. `cargo install mnemosyne-cli` support
4. Homebrew formula
5. Docker image
6. CLI UX improvements — progress bars (indicatif), colored output, better error messages
7. Versioned releases with changelog automation
8. Updated install/quickstart docs
9. Issue templates and PR template

**Dependencies:** M1 CI pipeline (✅ delivered)

**Modules/files affected:** `.github/workflows/`, `Cargo.toml`, `cli/`, `docs/`, `.github/ISSUE_TEMPLATE/`, `Dockerfile`

**Complexity:** Medium

**Implementation order:**
1. Release automation (cross-compile + publish, extending existing CI)
2. cargo install setup
3. CLI UX (progress bars, colors, errors)
4. Homebrew formula
5. Docker image
6. Issue/PR templates
7. Documentation updates

**Definition of done:**
- `cargo install mnemosyne-cli` works from crates.io
- Pre-built binaries for 5 platform targets on GitHub Releases
- CI passes with build + test + clippy + fmt
- `mnemosyne parse` shows a progress bar
- Error messages include context and suggestions
- Issue and PR templates exist

---

### Milestone 3 — Core Heap Analysis Parity
**Objective:** Close the feature gap with Eclipse MAT on core analysis capabilities.

**Why it matters:** Users choose heap analysis tools based on what they can answer. MAT is the benchmark. Mnemosyne needs to answer the same questions, better.

**Key Deliverables:**
1. MAT-style leak suspects algorithm — objects with disproportionate retained vs shallow size
2. Histogram improvements — group by fully-qualified class, package, classloader
3. OQL-like query engine — simple query language for object inspection
4. Thread inspection — parse thread records + stack traces, link to objects
5. ClassLoader analysis — hierarchy parsing, per-loader stats, leak detection
6. Collection inspection — detect known collections, fill ratio, size anomalies
7. Unreachable objects analysis — report unreachable set after GC root reachability
8. Enhanced heap diff — object/class-level comparison (not just record-level)

**Dependencies:** M1 (object graph, retained sizes, dominator tree) — ✅ core delivered

**Modules/files affected:** `core/src/analysis.rs`, `core/src/heap.rs`, `core/src/graph.rs`, new `core/src/query.rs`, new `core/src/thread.rs`, new `core/src/collections.rs`

**Complexity:** Very High

**Implementation order:**
1. Histogram improvements (uses object graph from M1)
2. Leak suspects algorithm
3. Unreachable objects
4. Enhanced heap diff
5. Thread inspection
6. ClassLoader analysis
7. Collection inspection
8. OQL query engine

**Definition of done:**
- `mnemosyne leaks` produces MAT-comparable leak suspect rankings
- Histograms group by class, package, and classloader
- `mnemosyne query "SELECT * FROM com.example.* WHERE retained_size > 1MB"` or equivalent works
- Thread inspection reports objects held per thread stack
- All features have unit and integration tests

---

### Milestone 4 — UI & Usability
**Objective:** Make Mnemosyne visually accessible to developers who prefer graphical exploration.

**Why it matters:** Most memory analysis is inherently visual — tree browsing, graph navigation, pattern recognition. A UI dramatically widens the user base.

**Key Deliverables:**
1. Static interactive HTML reports — self-contained, JS-enabled, collapsible, searchable
2. Local web UI — axum-based server for interactive exploration
3. Dominator tree browser — expandable tree view with retained size bars
4. Object inspector — click any object to see fields, references, size
5. Leak report dashboard — visual summary with drill-down
6. GC path visualizer — interactive path from object to GC root
7. Search and filter across all views

**Dependencies:** M1 (object graph), M3 (analysis features)

**Modules/files affected:** `core/src/report.rs`, new `core/src/web.rs` or `web/` crate, HTML templates, static assets

**Complexity:** High

**Implementation order:**
1. Enhanced HTML reports (Phase UI-2)
2. Local web server scaffolding
3. Dashboard view
4. Dominator tree browser
5. Object inspector
6. Leak report view
7. GC path visualizer
8. Query console (if M3 OQL exists)

**Definition of done:**
- `mnemosyne serve --web` opens a browser with interactive heap exploration
- Dominator tree is navigable with expand/collapse
- Can click any object to see its fields and references
- Provenance badges are visible throughout
- Works with dumps up to 2GB without browser crashes

---

### Milestone 5 — AI / MCP / Differentiation
**Objective:** Wire real AI capabilities and make MCP integration production-ready.

**Why it matters:** AI-assisted analysis is the key differentiator. Without real LLM calls, the "AI-powered" promise is hollow.

**Key Deliverables:**
1. LLM integration — wire generate_ai_insights to real OpenAI/Anthropic/local model calls
2. Configurable prompt/task runner — YAML-defined prompts with selective context injection
3. AI-driven leak explanations — pass retained-size data + object graph context to LLM
4. AI-driven fix suggestions — use LLM to generate context-aware patches
5. Conversation mode — interactive Q&A about a heap dump via CLI or MCP
6. MCP protocol improvements — proper tool descriptions, streaming, error contracts
7. Privacy controls — configurable data redaction before sending to LLM
8. Local model support — llama.cpp or similar for offline use

**Dependencies:** M1 (meaningful data to send to AI) — ✅ core delivered, M3 (richer analysis context)

**Modules/files affected:** `core/src/ai.rs`, `core/src/mcp.rs`, `core/src/config.rs`, new `core/src/llm.rs`, new `core/src/prompts/` directory

**Complexity:** High

**Implementation order:**
1. HTTP client + LLM abstraction layer
2. OpenAI backend implementation
3. Configurable prompt templates (YAML)
4. Wire AI insights to real calls
5. AI-driven explanations
6. AI-driven fix generation
7. MCP protocol hardening
8. Conversation mode
9. Privacy controls
10. Local model support

**Definition of done:**
- `mnemosyne analyze heap.hprof --ai` calls a real LLM and returns meaningful analysis
- Prompts are configurable via YAML
- MCP explain_leak returns LLM-generated explanation
- Privacy controls documented and configurable
- Works with at least 2 LLM providers (OpenAI + one local)

---

### Milestone 6 — Ecosystem & Community
**Objective:** Build the community and ecosystem that makes Mnemosyne self-sustaining.

**Why it matters:** Open-source success requires more than good code. It requires documentation, examples, community infrastructure, and ongoing engagement.

**Key Deliverables:**
1. Comprehensive documentation — API docs (rustdoc), user guide, tutorials
2. Example projects — sample Java apps with known memory issues + heap dumps
3. Benchmark suite — reproducible perf benchmarks vs MAT and other tools
4. Plugin/extension system — custom analyzers, output formats, LLM backends
5. Community infrastructure — Discord/Slack, contributor guide, office hours
6. Integration examples — GitHub Actions workflow, Jenkins pipeline, GitLab CI
7. Case studies — real-world usage stories
8. Conference talks / blog posts

**Dependencies:** M1-M5 (need a mature tool to evangelize)

**Modules/files affected:** `docs/`, `examples/`, `benches/`, `.github/`

**Complexity:** Medium (mostly documentation and content)

**Implementation order:**
1. API docs (rustdoc)
2. Example projects + sample dumps
3. Benchmark suite
4. Integration examples
5. User guide / tutorials
6. Plugin system design
7. Community channels
8. Content creation

**Definition of done:**
- rustdoc published
- 3+ example projects with heap dumps
- Benchmarks show parsing performance vs alternatives
- CI integration guide exists for GitHub Actions + Jenkins
- Active community channel with 50+ members

---

## Section 7 — Actionable Implementation Batches

### Milestone 1 Batches

#### M1-B1: Sample HPROF Test Fixtures ✅
- **Status:** Delivered
- **Outcome:** `core::test_fixtures` builds small deterministic HPROF binaries that exercise all record types needed for parser and graph testing. `resources/test-fixtures/README.md` documents them.

#### M1-B2: Object Graph Parser — Data Structures ✅
- **Status:** Delivered
- **Outcome:** `core::object_graph` defines `ObjectGraph`, `HeapObject`, `ClassInfo`, `FieldDescriptor`, `GcRoot`, `GcRootType`, `LoadedClass`, `ObjectKind`, and the string/class lookup tables. Canonical model used by parser, dominator, and analysis.

#### M1-B3: Object Graph Parser — HPROF Parsing ✅
- **Status:** Delivered
- **Outcome:** `core::hprof_parser` parses binary HPROF strings, classes, GC roots, INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP into `ObjectGraph`. Uses `byteorder` for big-endian binary parsing. API: `parse_hprof(data: &[u8])`, `parse_hprof_file(path: &str)`.

#### M1-B4: Dominator Tree Algorithm ✅
- **Status:** Delivered
- **Outcome:** `core::dominator::build_dominator_tree()` runs `petgraph::algo::dominators::simple_fast` (Lengauer-Tarjan) over the full object reference graph with a virtual super-root connected to all GC roots. Computes retained sizes via post-order accumulation. API: `top_retained(n)`, `retained_size(id)`, `immediate_dominator(id)`, `dominated_by(id)`, `node_count()`.

#### M1-B5: Retained Size Computation ✅
- **Status:** Delivered
- **Outcome:** `analysis::analyze_heap()` attempts graph-backed analysis first: `parse_hprof_file()` → `build_dominator_tree()` → graph-backed leak insights + dominator metrics. Falls back to heuristics with `ProvenanceKind::Fallback` / `ProvenanceKind::Partial` when parsing or filters prevent graph-backed results. `graph::build_graph_metrics_from_dominator()` populates `DominatorNode.retained_size` with real values.

#### M1-B6: Wire Graph Into Remaining Analysis Surfaces ✅
- **Status:** Delivered
- **Outcome:** Unified `detect_leaks()` onto the graph-backed path, rewrote the GC path finder to prefer `ObjectGraph` BFS with triple fallback (`ObjectGraph` BFS → budget-limited `GcGraph` → synthetic), and added navigation APIs on `ObjectGraph` (`get_object`, `get_references`, `get_referrers`).

#### M1-B7: Integration Tests ✅
- **Status:** Delivered
- **Outcome:** Added 16 CLI integration tests in `cli/tests/integration.rs` covering parse, leaks, analyze, gc-path, diff, fix, report, and config. The `test-fixtures` cargo feature eliminates fixture duplication and brings the total suite to 80 passing tests.

### Milestone 2 Batches

#### M2-B1: CLI UX — Progress Bars and Colors
- **Goal:** Add progress bars (indicatif) for long-running operations and colorized output
- **Files/modules affected:** `cli/src/main.rs`, `cli/Cargo.toml` (add indicatif dep)
- **Expected agent owner:** Implementation Agent
- **Validation:** `mnemosyne parse large.hprof` shows a progress bar; errors are red, warnings yellow; tests pass
- **Risk notes:** Progress reporting requires parser to emit progress callbacks — may need parser interface change
- **Non-scope:** Do not change core analysis or report logic

#### M2-B2: Release Automation
- **Goal:** Set up GitHub Actions to cross-compile and publish release binaries for Linux/macOS/Windows on tag push
- **Files/modules affected:** `.github/workflows/release.yml` (new), `Cargo.toml` (version metadata)
- **Expected agent owner:** Implementation Agent
- **Validation:** Pushing a version tag produces GitHub Release with binaries
- **Risk notes:** Cross-compilation can be tricky; consider cross-rs or cargo-zigbuild
- **Non-scope:** Do not set up Homebrew or Docker yet

#### M2-B3: Packaging — cargo install + Homebrew
- **Goal:** Publish mnemosyne-cli to crates.io; create Homebrew formula
- **Files/modules affected:** `Cargo.toml` metadata, new `Formula/` or homebrew-tap repo
- **Expected agent owner:** Implementation Agent
- **Validation:** `cargo install mnemosyne-cli` works; `brew install mnemosyne` works
- **Risk notes:** crates.io requires unique name; may need to check availability
- **Non-scope:** Do not set up Docker yet

---

## Section 8 — Differentiation Strategy

### 8.1 Provenance-Aware Analysis (Unique)
No other heap analysis tool tags its output with data provenance. Mnemosyne's ProvenanceKind system (Synthetic, Partial, Fallback, Placeholder) gives users and automation clear signals about data trust. This is genuinely novel and should be highlighted in all marketing.

### 8.2 MCP-Native Workflows
Eclipse MAT has no IDE integration path. Mnemosyne's MCP server makes it a memory debugging copilot inside VS Code, Cursor, Zed, and JetBrains. This is a first-mover advantage in the AI-assisted development tool space.

### 8.3 AI-Assisted Diagnosis
Once wired to real LLMs, Mnemosyne can explain memory issues in plain language, suggest fixes with code patches, and provide interactive Q&A about heap dumps. No other heap analyzer does this. The architecture is ready; the wiring is needed.

### 8.4 CI/CD Regression Detection
JSON and TOON output formats make Mnemosyne automation-friendly from day one. A CI pipeline can parse results, track trends, and alert on regressions. MAT is desktop-only with no CI story.

### 8.5 Rust Performance
Rust gives genuine advantages for large-dump analysis: no GC pauses, low memory overhead, predictable performance. For multi-GB dumps that choke Java-based tools, Mnemosyne should be measurably faster.

### 8.6 Modern Developer Experience
Clean CLI, multiple output formats, configuration hierarchy, provenance tracking, IDE integration — this is what modern developers expect. MAT's Eclipse-era UI feels dated. Mnemosyne can win on DX alone.

### 8.7 Scriptability & API-First
Every capability exposed via CLI is also available as a Rust library and via MCP. This API-first design enables integrations that aren't possible with GUI-only tools.

---

## Section 9 — Open Source Success Strategy

### 9.1 Documentation
- README with real examples, honest status badges, quick start
- ARCHITECTURE.md with up-to-date diagrams
- API docs via rustdoc, published to docs.rs
- User guide with tutorials for common scenarios
- Troubleshooting guide for common errors

### 9.2 Examples & Sample Data
- 3-5 sample heap dumps with known issues (leak, large cache, duplicate strings, classloader leak, thread leak)
- Example Java projects that generate these dumps
- Walk-through tutorials using each sample

### 9.3 CI/CD
- Build + test + clippy + fmt on every PR
- Nightly builds for development branch
- Status badges in README
- Test coverage tracking (tarpaulin or similar)

### 9.4 Releases & Packaging
- Semantic versioning
- Pre-built binaries for 5 platform targets
- `cargo install mnemosyne-cli`
- Homebrew formula
- Docker image for CI use
- Changelog automation (git-cliff or similar)

### 9.5 Community Files
- CODE_OF_CONDUCT.md
- SECURITY.md (responsible disclosure)
- Issue templates (bug report, feature request, heap dump compatibility)
- PR template with checklist
- CONTRIBUTING.md (already exists — enhance with architecture overview)
- Good first issues labeled and maintained

### 9.6 Contributor Onboarding
- Architecture walkthrough in docs
- "Good first issue" labels
- Module ownership/contact guide
- Development setup guide (build, test, lint)
- Agent workflow documentation (already exists)

### 9.7 Benchmark Transparency
- Published benchmarks for parsing speed vs file size
- Memory usage benchmarks
- Comparison with MAT and hprof-slurp where fair
- Reproducible benchmark scripts in repo

### 9.8 Success Metrics
| Metric | 6-month target | 12-month target |
|---|---|---|
| GitHub stars | 500 | 2,000 |
| Monthly downloads (crates.io) | 200 | 1,000 |
| Contributors | 5 | 15 |
| Open issues (healthy) | 20 | 50 |
| Passing CI | 100% | 100% |
| Test count | 100 | 300 |
| Doc coverage | 80% | 95% |

---

## Section 10 — Prioritized Backlog

| # | Item | Priority | Impact | Effort | Dependencies | Milestone | Status |
|---|---|---|---|---|---|---|---|
| 1 | Object graph parser | P0 | High | XL | None | M1 | ✅ Done |
| 2 | Dominator tree algorithm | P0 | High | L | Object graph | M1 | ✅ Done |
| 3 | Retained size computation | P0 | High | M | Dominator tree | M1 | ✅ Done |
| 4 | Sample HPROF test fixtures | P0 | High | M | None | M1 | ✅ Done |
| 5 | CI pipeline (GitHub Actions) | P0 | High | M | None | M1 | ✅ Done |
| 6 | Unify `detect_leaks()` onto graph path | P0 | High | L | Object graph + retained sizes | M1 | ✅ Done |
| 7 | Rewrite GC path over full object graph | P0 | High | M | Object graph | M1 | ✅ Done |
| 8 | Object graph navigation API | P0 | High | M | Object graph | M1 | ✅ Done |
| 9 | Integration tests via reusable synthetic HPROF fixtures | P0 | High | L | Test fixtures + CI | M1 | ✅ Done |
| 10 | Release binaries | P1 | High | M | CI pipeline (✅) | M2 | ⚬ Pending |
| 11 | cargo install support | P1 | High | S | Release setup | M2 | ⚬ Pending |
| 12 | CLI progress bars + colors | P1 | Medium | S | None | M2 | ⚬ Pending |
| 13 | MAT-style leak suspects | P1 | High | L | Retained sizes (✅) | M3 | ⚬ Pending |
| 14 | Histogram by class/package/classloader | P1 | High | M | Object graph (✅) | M3 | ⚬ Pending |
| 15 | Homebrew formula | P1 | Medium | S | Release binaries | M2 | ⚬ Pending |
| 16 | LLM integration (real API calls) | P1 | High | L | Meaningful data (✅ M1) | M5 | ⚬ Pending |
| 17 | Enhanced heap diff | P1 | Medium | M | Object graph (✅) | M3 | ⚬ Pending |
| 18 | Static interactive HTML reports | P2 | High | L | Reporting exists | M4 | ⚬ Pending |
| 19 | OQL query engine | P2 | High | XL | Object graph (✅) | M3 | ⚬ Pending |
| 20 | Thread inspection | P2 | Medium | L | Object graph (✅) | M3 | ⚬ Pending |
| 21 | ClassLoader analysis | P2 | Medium | L | Object graph (✅) | M3 | ⚬ Pending |
| 22 | Local web UI | P2 | High | XL | HTML reports | M4 | ⚬ Pending |
| 23 | Collection inspection | P2 | Medium | M | Object graph (✅) | M3 | ⚬ Pending |
| 24 | Unreachable objects | P2 | Medium | M | Object graph (✅) | M3 | ⚬ Pending |
| 25 | Configurable prompt/task runner | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 26 | AI conversation mode | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 27 | Docker image | P2 | Medium | S | Release automation | M2 | ⚬ Pending |
| 28 | Example projects + sample dumps | P2 | Medium | M | Test fixtures (✅) | M6 | ⚬ Pending |
| 29 | Benchmark suite | P2 | Medium | M | Object graph (✅) | M6 | ⚬ Pending |
| 30 | Plugin/extension system | P3 | Medium | XL | Stable APIs (M3+) | M6 | ⚬ Pending |
| 31 | Full interactive heap browser | P3 | High | XL | Web UI + OQL | M4 | ⚬ Pending |
| 32 | Local LLM support | P3 | Medium | L | LLM integration | M5 | ⚬ Pending |

---

## Section 11 — Recommended Immediate Next Steps

Milestone 1 is delivered and validated (80 tests, clippy clean, fmt clean). The immediate next steps now begin Milestone 2:

### Step 1: M2-B1 — CLI UX Improvements
**Why next:** Quick win for developer experience. Progress bars, colored output, better error messages. Low risk.
**Owner:** Implementation Agent
**Effort:** Small

### Step 2: M2-B2 — Release Automation
**Why next:** Get binaries out so people can actually install and use the tool without building from source.
**Owner:** Implementation Agent
**Effort:** Medium

### Step 3: M2-B3 — Packaging (cargo install + Homebrew)
**Why next:** Complete the distribution story for the broadest reach.
**Owner:** Implementation Agent
**Effort:** Small

---

*This roadmap is a living document. Update it after each major batch completion.*
*Next review: after the first Milestone 2 batch lands.*
