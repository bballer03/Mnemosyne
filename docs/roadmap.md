# Mnemosyne Roadmap & Milestones

> **Last updated:** 2026-03-08 (post M3-P1-B2 — histogram grouping, suspect ranking, unreachable objects, and class-level diff landed)
> **Owner:** Tech PM Agent
> **Status:** Living document — updated after each major implementation batch

---

## Table of Contents

1. [Executive Summary](#section-1--executive-summary)
2. [Current State Assessment](#section-2--current-state-assessment)
3. [Gap Analysis](#section-3--gap-analysis)
4. [Eclipse MAT Feature Parity Analysis](#section-4--eclipse-mat-feature-parity-analysis)
4.5. [Predecessor & Competitor Analysis](#section-45--predecessor--competitor-analysis)
5. [UI / Product Experience Strategy](#section-5--ui--product-experience-strategy)
6. [Differentiation Opportunities](#section-6--differentiation-opportunities)
7. [Feature Proposals](#section-7--feature-proposals)
8. [Roadmap](#section-8--roadmap)
9. [Milestones Detail](#section-9--milestones-detail)
10. [Suggested Implementation Order](#section-10--suggested-implementation-order)
11. [Recommended Immediate Next Steps](#section-11--recommended-immediate-next-steps)
12. [Risk Register & Lessons Learned](#section-12--risk-register--lessons-learned)

---

## Section 1 — Executive Summary

Mnemosyne is an **alpha-stage Rust-based JVM heap analysis tool** with a validated analytical foundation. It stream-parses HPROF files to produce class histograms and heap summaries, parses binary HPROF records into a full object reference graph (`core::hprof::binary_parser` → `core::hprof::object_graph`), computes a real dominator tree via Lengauer-Tarjan (`core::graph::dominator`), derives retained sizes from post-order subtree accumulation, runs graph-backed analysis in both `analyze_heap()` and `detect_leaks()` with automatic fallback to heuristics when parsing fails, traces GC root paths with `ObjectGraph` BFS first plus layered fallbacks, exposes object navigation via `get_object(id)`, `get_references(id)`, and `get_referrers(id)`, generates template-based fix suggestions, and renders results in five output formats (Text, Markdown, HTML, TOON, JSON) — all backed by a provenance system that labels every synthetic, partial, fallback, or placeholder data surface. A stdio MCP server exposes seven JSON-RPC handlers (`parse_heap`, `detect_leaks`, `map_to_code`, `find_gc_path`, `explain_leak`, `propose_fix`, `apply_fix`), making the tool available inside VS Code, Cursor, Zed, JetBrains, and ChatGPT Desktop. The AI module (`analysis::generate_ai_insights`) is **fully stubbed**: it returns deterministic template text with zero LLM calls and zero HTTP client dependencies.

Mnemosyne has the foundations to become **the first Rust-native, AI-assisted heap analysis platform** that rivals Eclipse MAT in analysis depth while offering capabilities no existing tool provides: provenance-tracked outputs that distinguish real analysis from heuristic guesses, MCP-native IDE integration for copilot-style workflows, CI/CD-friendly automation via structured JSON and TOON output, and an AI-native architecture designed from day one for LLM integration. The Rust core means multi-GB heap dumps can be processed with predictable memory usage and no GC pauses — a meaningful advantage over Java-based tools like MAT and VisualVM for production incident response.

Five properties position Mnemosyne to stand out in a crowded JVM tooling ecosystem: **(1)** Rust performance enabling streaming analysis of heap dumps that exceed host RAM; **(2)** a provenance system unique among heap analyzers, giving users and automation confidence in result trustworthiness; **(3)** MCP-first architecture that makes heap analysis a conversation in the developer's IDE rather than a separate tool; **(4)** AI-native design with well-shaped type contracts (`AiInsights`, `AiWireExchange`, config plumbing) ready for LLM wiring; and **(5)** automation-friendly structured output (JSON, TOON) enabling CI regression detection with machine-readable leak signals.

**M1.5 update (2026-03-08): The critical HPROF tag-constant bug has been fixed and the graph-backed pipeline is now validated on real-world data.** The tag constants in `binary_parser.rs`, `parser.rs`, and `gc_path.rs` have been corrected (`TAG_HEAP_DUMP_SEGMENT` = `0x1C`). Real-world Kotlin + Spring Boot heap dumps (~110MB, ~150MB) now parse correctly into populated object graphs, produce meaningful dominator trees with real retained sizes, and generate non-synthetic GC paths. Leak-ID validation has been added to `explain`, `fix`, and MCP `explain_leak`. The workspace now carries 101 passing tests (66 core + 5 CLI unit + 30 CLI integration), including 4 real-world HPROF integration tests that validate the end-to-end pipeline against actual JVM dumps.

Honest assessment: **the analytical foundation is sound and real-world-validated, but significant feature work remains** to deliver on the full vision. The core pipeline — object graph, dominator tree, retained sizes, unified leak detection, GC paths, provenance system — works correctly on production data. The distribution story is solid: v0.1.1 is published on crates.io, GitHub Releases, Homebrew, and Docker. The AI module remains 100% stubbed. v0.1.1 completed the internal `core/src/` restructure from flat files into grouped module directories (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`) while preserving public API re-exports in `lib.rs`. Benchmarks, performance data, and analysis feature parity with Eclipse MAT are the primary remaining gaps. **The immediate priority is shipping a v0.2.0 correctness release, establishing a benchmark baseline, and then delivering M3 core analysis features (MAT-style suspects, histogram grouping, enhanced diff, thread inspection) that make Mnemosyne a credible MAT alternative.**

---

## Section 2 — Current State Assessment

### Core Capabilities

| Capability | Status | Honest Assessment |
|---|---|---|
| HPROF streaming parser | ✅ Validated | Two-tier parsing: `core::hprof::parser` streams headers + record tags for fast class histograms. `core::hprof::binary_parser` parses binary HPROF records into `ObjectGraph`. Tag constants corrected in M1.5 — both `HEAP_DUMP` (0x0C) and `HEAP_DUMP_SEGMENT` (0x1C) records are now parsed correctly. Validated on real-world Kotlin+Spring Boot dumps (~110MB, ~150MB). |
| Leak detection | ✅ Graph-backed + heuristic fallback | `detect_leaks()` attempts the graph-backed path first (ObjectGraph → dominator → retained sizes), then falls back to heuristics with provenance markers. Both paths validated on real-world data. Leak-ID validation added in M1.5 — unknown IDs now return errors. |
| Graph / dominator tree | ✅ Real-world validated | `core::graph::dominator::build_dominator_tree()` runs Lengauer–Tarjan over the full object reference graph with virtual super-root. Validated on both synthetic fixtures and real-world JVM dumps. Produces meaningful retained sizes from real object data. |
| GC root path tracing | ✅ Real-world validated | `core::graph::gc_path` tries `ObjectGraph` BFS first, then budget-limited `GcGraph`, then synthetic paths. Primary BFS path activates on real-world dumps. Provenance labels honestly indicate data quality. |
| AI / LLM insights | ❌ Stubbed | `core::analysis::generate_ai_insights()` returns deterministic template text. No HTTP client in `Cargo.toml`, no API calls, no LLM SDK. Config plumbing exists (`AiConfig` with provider/model/temperature fields) but terminates at the stub. The "AI-powered" claim in README is aspirational until M5. |
| Fix suggestions | ⚠️ Template only | `core::fix::propose_fix()` generates template patches in three styles (Minimal, Defensive, Comprehensive). No AI involvement, no code analysis. Useful scaffolding with provenance markers. Leak-ID validation now enforced. |
| Source mapping | ✅ Implemented | `core::mapper::map_to_code()` scans project dirs for `.java`/`.kt` files, runs `git blame` for metadata. Basic but functional for local projects. |
| Reporting | ✅ Implemented | `core::report` renders 5 formats (Text, Markdown, HTML, TOON, JSON). HTML output uses `escape_html()` for XSS prevention. TOON uses `escape_toon_value()` for control characters. Provenance markers rendered in all non-JSON formats. One of the most polished subsystems. |
| MCP server | ✅ Wired | `core::mcp::serve()` runs a stdio JSON-RPC loop with async Tokio I/O. Handles 7 methods. Works end-to-end; analysis quality now backed by real graph-based results on real dumps. AI insights remain stubbed. |
| Config system | ✅ Implemented | `cli::config_loader` reads TOML files from 5 locations + env vars + CLI flags. `core::config` defines `AppConfig`, `AiConfig`, `ParserConfig`, `AnalysisConfig`. Clean, well-layered. |
| Provenance system | ✅ Implemented | `ProvenanceKind` (Synthetic, Partial, Fallback, Placeholder) + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in all report formats and CLI output. Unique feature in the heap-analysis space. |

### Technical Strengths

- **Rust performance model**: streaming parser with `BufReader`, no GC, predictable memory. Can handle files larger than RAM in principle.
- **Clean module separation**: grouped implementation domains in `core/src/` (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`) plus shared `config.rs`, `errors.rs`, and `lib.rs` re-exports.
- **Real object graph (real-world validated)**: `core::hprof::binary_parser` parses binary HPROF records into an `ObjectGraph` with objects, reference edges, class metadata, and GC roots. Validated on both synthetic fixtures (0x0C) and real-world Kotlin+Spring Boot dumps (0x1C).
- **Real dominator tree (real-world validated)**: `core::graph::dominator` implements Lengauer–Tarjan over the full object graph with virtual super-root. Computes retained sizes via post-order accumulation. Validated on real data.
- **Graph-backed analysis pipeline (real-world validated)**: `analyze_heap()` and `detect_leaks()` both attempt object-graph → dominator-tree → retained-size analysis first, with automatic fallback to heuristics and provenance markers. Pipeline activates and produces meaningful results on real-world dumps.
- **Streaming design**: `core::hprof::parser` processes HPROF records sequentially without loading the full dump. Foundation for scaling to multi-GB files.
- **Provenance system**: genuinely novel for a heap analyzer. Labels every synthetic/heuristic output surface so consumers know what to trust.
- **Multi-format output**: 5 report formats with consistent provenance rendering. HTML is XSS-hardened. TOON enables compact CI consumption.
- **101-test suite with CI**: 66 core + 5 CLI unit + 30 CLI integration tests running in GitHub Actions, including 4 real-world HPROF validation tests. Synthetic and segment HPROF test fixtures plus the `test-fixtures` cargo feature enable deterministic parser, graph, end-to-end CLI testing, and targeted error-path coverage.
- **Config hierarchy**: TOML + env vars + CLI flags with clear precedence. Production-ready design pattern.
- **MCP integration**: stdio JSON-RPC server with 7 handlers. First-mover for heap analysis in the MCP ecosystem.
- **Type contracts**: well-shaped request/response types (`AnalyzeRequest`, `AnalyzeResponse`, `GcPathResult`, `FixResponse`, etc.) that establish stable contracts between CLI, MCP, and core.
- **Distribution**: v0.1.1 published on crates.io, GitHub Releases (5 targets), Homebrew, Docker (GHCR). All channels functional.

### Major Weaknesses

- **AI is 100% stubbed**: `generate_ai_insights()` returns hardcoded template strings. There are zero HTTP client dependencies in `Cargo.toml`. The `AiConfig` fields (provider, model, temperature, API key) exist but connect to nothing. Every "AI-powered" claim in documentation is marketing ahead of implementation.
- **Benchmark infrastructure exists, but no published baseline exists yet**: `criterion` benches now cover parser throughput, graph construction, and dominator computation, and `scripts/measure_rss.sh` can capture CLI parse RSS. Throughput and RSS at scale are still unknown because the initial baseline has not been collected or published.
- **Memory scaling unknown at 1GB+**: validated on 110-150MB dumps. In-memory `ObjectGraph` may blow up on multi-GB dumps. No data yet.
- **MAT parity gap remains**: thread inspection, collection inspection, OQL, deeper ClassLoader analysis, and large-dump ergonomics are still missing. M3-P1-B2 closed histogram grouping, MAT-style suspect ranking, unreachable-object reporting, and class-level diff.
- **Diff is record-level, not object-level**: `diff_heaps()` compares aggregate record/class statistics. It cannot track individual object migration or reference chain changes.
- **Graph module naming is misleading**: `summarize_graph()` still exists as a lightweight fallback that builds a synthetic tree from top-12 entries. Its name suggests more than it delivers, though the real dominator tree now exists alongside it.
- **No sample real-world data for tutorials**: real-world validation exists in CI (optional fixture), but no example `.hprof` files for documentation or onboarding.

### Maturity Assessment

| Subsystem | Maturity | Rationale |
|---|---|---|
| Parser | Alpha+ | Both streaming (record-level stats) and binary (full object graph) parsers validated on real-world HPROF files. Tag constants correct. Lacks benchmarks, threading, and multi-GB validation. |
| Leak detection | Alpha+ | Graph-backed + heuristic fallback both validated on real data. Leak-ID validation enforced. MAT-style suspect ranking, accumulation-point detection, and short reference-chain context landed in M3-P1-B2. |
| Graph / Dominator | Alpha+ | Lengauer–Tarjan validated on real-world object graphs. Retained sizes correct. M3-P1-B2 added histogram grouping and unreachable-object analysis; the main remaining gap is a browsable dominator/explorer view. |
| AI | Pre-alpha | Fully stubbed. Returns deterministic text. Not wired to any model. |
| GC root paths | Alpha+ | `ObjectGraph` BFS activates on real dumps. Triple fallback with honest provenance. |
| Fix suggestions | Alpha | Template-based scaffolding with leak-ID validation. No code analysis or AI. |
| Source mapping | Alpha | Works for basic cases. No IDE integration beyond file scanning. |
| Reporting | Beta | 5 formats, XSS hardening, provenance rendering, well-tested. Ready for use. |
| MCP server | Alpha+ | Wired, functional, and backed by real graph-based analysis on real dumps. AI insights remain stubbed. |
| Config | Beta | Clean hierarchy, env + TOML + CLI. Production-ready pattern. |
| Provenance | Beta | Unique, well-integrated across all surfaces. Novel in the space. |
| Testing | Beta- | 110 tests across the workspace, including real-world HPROF validation plus dedicated histogram/suspect/unreachable/diff coverage. Reusable `test-fixtures` feature, GitHub Actions CI, and Criterion benchmark targets for parser/graph/dominator workloads. No property-based testing and no published performance baseline yet. |
| CI/CD | Beta- | GitHub Actions CI runs check + test + clippy + fmt. Tagged release workflow cross-compiles for 5 targets + Docker. Nightly builds still absent. |

---

## Section 3 — Gap Analysis

### 3.1 Correctness & Trust Gaps

**✅ RESOLVED (M1.5): HPROF tag constant mislabeling.** Both parsers now use the correct tag constants: `HEAP_DUMP_SEGMENT = 0x1C`, `CPU_SAMPLES = 0x0D`, `HEAP_DUMP_END = 0x2C`, `CONTROL_SETTINGS = 0x0E`. The streaming parser's `tag_name()` function also returns correct labels. Validated on real-world Kotlin+Spring Boot dumps (~110MB, ~150MB).

**✅ RESOLVED (M1.5): Object reference graph validated on real-world data.** The full pipeline — `binary_parser` → `ObjectGraph` → `dominator` → retained sizes → leak detection — now activates on real JVM dumps. The binary parser correctly parses `HEAP_DUMP_SEGMENT` (0x1C) records, producing a populated object graph. Dominator tree computes meaningful retained sizes from real object data. Graph-backed analysis produces real results on production Kotlin+Spring Boot dumps.

**✅ RESOLVED (M1.5): Leak detection produces results on real-world data.** The graph-backed path activates and finds leak candidates on real dumps. Heuristic fallback also works with provenance markers. Leak-ID validation now enforced — unknown IDs return errors.

**✅ RESOLVED (M1.5): explain/fix commands validate leak IDs.** Unknown leak-IDs now return explicit errors instead of generic responses. Fix command no longer generates hardcoded patches for fabricated IDs.

- **Diff is record-level, not object-level.** `diff_heaps()` compares aggregate record/class statistics between two snapshots. It cannot track individual object migration, new allocation sites, or reference chain changes. (Note: the diff command itself works well and is one of the most useful features — the "delta" summary is accurate at the record level.) Object-level diff planned for M3.

**Provenance correctly labels data quality** — the system labels graph-backed results with no provenance marker (clean data) and heuristic/fallback results with `ProvenanceKind::Fallback` or `ProvenanceKind::Partial`, so consumers know what to trust. The provenance system worked as designed during real-world testing: `[PARTIAL]` labels were honestly displayed.

### 3.2 Testing & CI Gaps

- **110 tests** across the workspace. Tests cover provenance rendering, escape functions, analysis paths, HPROF parsing, object graph construction, dominator tree correctness, retained-size computation, histogram grouping, suspect scoring, unreachable-object analysis, enhanced diffing, CLI argument handling, end-to-end command execution, targeted failure-path UX, and real-world HPROF validation.
- **Synthetic HPROF test fixtures** exist in `core::test_fixtures`. Small deterministic binary HPROF files exercise the parser and graph pipeline without requiring a JVM or committing large binaries. Includes `build_simple_fixture()` (0x0C), `build_graph_fixture()` (0x0C), and `build_segment_fixture()` (0x1C).
- **`test-fixtures` cargo feature** exposes canonical fixture builders to integration tests without widening the builder API surface.
- **Real-world HPROF validation** added in M1.5: 4 tests validate against real Kotlin+Spring Boot production dumps. Binary parser, object graph population, dominator tree construction, and retained-size computation are all tested on real data (gated behind optional fixture path).
- **CI pipeline running.** GitHub Actions (`.github/workflows/ci.yml`) runs `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and PRs.
- **30 end-to-end CLI integration tests.** `cli/tests/integration.rs` runs `parse`, `leaks`, `analyze`, `gc-path`, `diff`, `fix`, `report`, and `config` as subprocesses against synthetic HPROF fixtures and validates key error-path guidance.
- **No coverage tracking.** No `cargo-tarpaulin` or `cargo-llvm-cov` integration. Unknown actual coverage percentage.
- **No property-based testing.** Parser binary handling is a prime candidate for `proptest` or `quickcheck` fuzzing.
- **Benchmark results are still missing.** Criterion benchmark targets for parser throughput, graph construction, and dominator computation now exist, but no baseline numbers or regression thresholds have been published yet. This remains the highest-priority testing gap.

### 3.3 Documentation & Onboarding Gaps

- **README and QUICKSTART now reflect shipped behavior.** Output examples in both files match the actual CLI table-based presentation. "AI-generated explanations" remain aspirational — AI is 100% stubbed.
- **`docs/api.md` is still placeholder scaffolding.** The file exists but contains no real MCP API documentation — no JSON-RPC method signatures, no request/response schemas, no usage examples. This is documentation debt that misleads contributors who expect an API reference. Needs real content covering all 7 MCP handlers with wire-format examples.
- **`docs/examples/` is still placeholder.** `docs/examples/README.md` exists but has no real usage examples. Needs real CLI workflow examples, sample analysis sessions, and MCP integration examples. Currently a dead end for anyone following the docs.
- **README badge still says `status-alpha-yellow`.** The badge does not include a version qualifier. Optionally update to a version-qualified badge (e.g., `v0.1.1-alpha`) for better clarity. Low priority but noted.
- **No tutorial or cookbook.** No guided walkthrough of a real analysis session. No examples of interpreting output or acting on leak candidates.
- **No troubleshooting guide.** No documentation for common errors, unsupported HPROF variants, or limitations.
- **No performance benchmarks published yet.** No committed data compares Mnemosyne against MAT, VisualVM, or other tools. The `criterion` suite and RSS tooling now exist in the repository, but the first baseline and comparison write-up still need to be produced.

### 3.4 Packaging & Release Gaps

- **Release distribution is live for v0.1.1.** `.github/workflows/release.yml` cross-compiles and packages `mnemosyne-cli` for five targets, publishes tagged GitHub releases, builds/pushes `ghcr.io/<owner>/mnemosyne` on tagged releases, and the current production release is now shipped across those channels.
- **✅ crates.io published** (`mnemosyne-core 0.1.1` + `mnemosyne-cli 0.1.1`).
- **✅ `cargo install mnemosyne-cli` is live.**
- **Docker delivery is now in place.** A multi-stage `Dockerfile` builds `mnemosyne-cli` into a non-root `debian:bookworm-slim` runtime image, and tagged releases publish `ghcr.io/<owner>/mnemosyne` with semver plus `latest` tags.
- **✅ SHA256 values filled for v0.1.1.** `HomebrewFormula/mnemosyne.rb` now contains release checksums for the tagged archives.
- **✅ CHANGELOG.md has `[0.1.1] - 2026-03-08` section.** Changelog updates are still manual.

### 3.5 Feature Parity Gaps vs Eclipse MAT

Eclipse MAT is the de-facto standard for JVM heap analysis. With M1 and M1.5 complete, Mnemosyne now has the foundational analysis features (object graph, dominator tree, retained sizes) validated on real-world data. The remaining gaps are in advanced MAT capabilities:

- **No browsable dominator view**: real dominator tree exists and is validated but is not exposed as an interactive explorer.
- **No OQL**: MAT provides Object Query Language for ad-hoc heap exploration.
- **No thread inspection**: MAT links thread stack traces to retained objects.
- **No classloader analysis**: MAT detects classloader leaks by analyzing the classloader hierarchy.
- **No collection inspection**: MAT inspects `HashMap`, `ArrayList`, etc. fill ratios and waste.
- **No object-level comparison**: MAT diffs two dumps at object granularity with object identity and reference-chain changes. Mnemosyne now covers class-level deltas, but not per-object identity tracking.

The gap remains significant but the architectural path is clear. The object graph model, dominator tree algorithm, retained sizes computation, unified leak detection pipeline, and navigation API are all implemented and validated on real-world data. **M3 is the milestone that closes the MAT parity gap for core analysis features.**

### 3.6 UX & Usability Gaps

- **Progress indicators are present, but not yet byte-accurate.** CLI commands now use `indicatif` spinners, but long-running parses still lack a true progress bar tied to bytes or records processed.
- **Error messaging is materially better, but troubleshooting docs still lag.** `CoreError` now carries structured variants for missing files, non-HPROF inputs, HPROF header parse failures, and config errors, and the CLI prints `hint:` lines with suggestions. The remaining gap is documentation for unsupported dump variants and deeper troubleshooting scenarios.
- **No interactive mode.** No REPL or interactive exploration of results.
- **Output styling is now solid for a CLI tool.** Spinners, colorized labels, and comfy-table aligned ASCII tables with truncation disclosure are shipped. Richer presentation (summary dashboards, interactive TUI) remains future work.

### 3.7 Ecosystem & Community Gaps

- **Community baseline files now exist, but contributor pathways are still thin.** Issue templates, a PR template, `CODE_OF_CONDUCT.md`, and `SECURITY.md` are now in place, but there is still no documented contributor ladder or maintainer path.
- **No example projects.** `docs/examples/README.md` exists but is a placeholder. No real CLI workflow examples, sample analysis sessions, or MCP integration examples.
- **No benchmarks.** No `criterion` benchmark suite and no performance comparison data against MAT, VisualVM, or YourKit. Must be addressed before or alongside M3.
- **No community infrastructure.** No Discord, Discussions, or mailing list.

---

## Section 4 — Eclipse MAT Feature Parity Analysis

| MAT Feature | Mnemosyne Status | Gap | Implementation Approach | Difficulty | Strategic Importance | Milestone |
|---|---|---|---|---|---|---|
| Dominator tree | ✅ Real-world validated | Algorithm correct, validated on real dumps. Not yet exposed as a browsable view | Expose via CLI subcommand + MCP handler + future web UI | Medium | Critical | M1 ✅ |
| Retained size | ✅ Real-world validated | Algorithm correct, validated on real dumps. Not yet exposed in all surfaces | Expose in diff, histogram, and MCP surfaces | Medium | Critical | M1 ✅ |
| Object graph traversal | ✅ Real-world validated | Object graph model and API exist, binary parser populates fully from real dumps | Expose richer navigation surfaces | Medium | Critical | M1 ✅ |
| Shortest path to GC roots | ✅ Real-world validated | Primary BFS path activates on real dumps with honest provenance | Improve path quality with priority queuing and path deduplication | Medium | High | M1 ✅ |
| Leak suspects report | ✅ Delivered | M3-P1-B2 ranks suspects by retained/shallow ratio, accumulation-point detection, dominated-count context, short reference chains, and composite score | Extend the same scoring into explorer and future MCP surfaces | High | Critical | M3 |
| Histogram by class/package/classloader | ✅ Delivered | M3-P1-B2 adds graph-backed grouping by class, package prefix, and classloader plus CLI `analyze --group-by` output | Reuse grouped histogram data in dedicated MCP and UI surfaces | Medium | High | M3 |
| OQL / query language | ❌ Missing | No query capability | Design mini-query language or embed existing (e.g., SQL-like over object model) | Very High | High | M3 |
| Thread inspection | ❌ Missing | Not implemented | Parse HPROF STACK_TRACE + STACK_FRAME records, link threads to retained objects | High | Medium | M3 |
| ClassLoader analysis | ❌ Missing | Not implemented | Parse classloader hierarchy from CLASS_DUMP records, detect leaks per classloader | High | Medium | M3 |
| Collection inspection | ❌ Missing | Not implemented | Detect known collection types (`HashMap`, `ArrayList`, etc.), inspect fill ratio, size, waste | Medium | Medium | M3 |
| Export / reporting | ✅ Implemented | Good for current scope | Already strong: 5 formats, provenance, XSS hardening. Add CSV, protobuf, flamegraph later | Low | Medium | M2 |
| UI-based exploration | ❌ Missing | CLI only | Phase from TUI → static HTML → web UI → full explorer | Very High | High | M4 |
| Large dump performance | ⚠️ Partial | Streaming parser handles any size; in-memory object graph validated on 110-150MB dumps. Memory behavior at 1GB+ unknown | Benchmark current RSS, consider disk-backed store if needed | High | High | M3 |
| Heap snapshot comparison | ⚠️ Partial | Record-level diff plus class-level instance/shallow/retained deltas landed in M3-P1-B2; object-identity and reference-chain diffing are still missing | Extend to object-level diff if stable identity/indexing is added later | Medium | Medium | M3 |
| Unreachable objects | ✅ Delivered | M3-P1-B2 reports unreachable-object count, shallow size, and per-class breakdown from GC-root reachability traversal | Add richer drill-down and explorer/report views | Medium | Medium | M3 |

### Detailed Analysis per Feature

**Dominator Tree.**
*Current Status:* ✅ Algorithm implemented, correct, and validated on real-world dumps. `core::graph::dominator::build_dominator_tree()` runs `petgraph::algo::dominators::simple_fast` (Lengauer–Tarjan) over the full object reference graph with a virtual super-root. Produces meaningful retained sizes from real JVM heap data.
*Remaining Gap:* Not yet exposed as a standalone CLI subcommand or browsable view. Currently only visible through `analyze` output.
*Next Steps:* Add a `mnemosyne dominators` CLI command and MCP handler. Expose `top_retained(n)`, tree-browsing queries, and integrate into the future web UI.
*Milestone:* Core algorithm delivered in M1. Real-world validation completed in M1.5. Browsable view is M4.

**Retained Size.**
*Current Status:* ✅ Algorithm implemented, correct, and validated on real-world dumps. `core::graph::dominator::build_dominator_tree()` computes retained sizes via post-order traversal. Produces accurate retained sizes from real object data.
*Remaining Gap:* Not yet exposed in diff, histogram, or all MCP surfaces.
*Next Steps:* Expose retained sizes in `diff_heaps()` output, histogram views, and future explorer surfaces.
*Milestone:* Core computation delivered in M1. Real-world validation completed in M1.5. Broader surface integration in M3.

**Object Graph Traversal.**
*Current Status:* ✅ Implemented and validated on real-world data. `core::hprof::binary_parser` parses binary HPROF records into `core::hprof::object_graph::ObjectGraph` and the graph exposes `get_object(id)`, `get_referrers(id)`, and `get_references(id)`. Correctly parses both `HEAP_DUMP` (0x0C) and `HEAP_DUMP_SEGMENT` (0x1C) records.
*Remaining Gap:* Navigation API exists but is not surfaced through richer CLI or MCP browsing experiences.
*Next Steps:* Expose the existing navigation API through richer CLI and MCP browsing surfaces. Add object inspection commands.
*Milestone:* Graph data structures and navigation API delivered in M1. Real-world validation completed in M1.5. Richer explorer surfaces in M3/M4.

**Shortest Path to GC Roots.**
*Current Status:* ✅ Architecture correct with layered fallback, validated on real-world data. On real dumps, the `ObjectGraph` BFS path activates and finds real GC root paths. The provenance labels honestly distinguish real paths from fallback paths.
*Remaining Gap:* Path quality could be improved with priority queuing and path deduplication.
*Next Steps:* Improve path quality for M3 analysis features. Add path visualization in M4.
*Milestone:* Core graph-backed path-finding delivered in M1. Real-world activation validated in M1.5.

**Leak Suspects Report.**
*Current Status:* ✅ M3-P1-B2 added MAT-style suspect ranking to the graph-backed path. `detect_leaks()` and `analyze_heap()` now score suspects using retained/shallow ratio, accumulation-point detection, dominated-object counts, short reference chains, and a composite score.
*Remaining Gap:* The current output is intentionally compact. Dedicated explorer views, richer path visualization, and broader MCP exposure remain future work.
*Recommended Approach:* Reuse the existing `LeakSuspect` scoring model in future explorer/MCP surfaces rather than introducing a second ranking system.
*Milestone:* Base pipeline delivered in M1, validated in M1.5, advanced suspect ranking delivered in M3-P1-B2.

**Histogram by Class/Package/ClassLoader.**
*Current Status:* ✅ M3-P1-B2 added graph-backed histogram grouping by fully-qualified class, package prefix, and classloader. `mnemosyne analyze --group-by` now renders grouped histogram output directly in the CLI.
*Remaining Gap:* Superclass grouping and a dedicated histogram MCP/API surface are still future work.
*Recommended Approach:* Treat the current grouped histogram as the shared source for future explorer and MCP histogram views.
*Milestone:* Delivered in M3-P1-B2.

**OQL / Query Language.**
*Current Status:* Not implemented. No query capability of any kind.
*Gap:* MAT's OQL allows `SELECT * FROM java.lang.String WHERE toString().length() > 1000` style queries. Extremely powerful for ad-hoc investigation.
*Recommended Approach:* Design a minimal query language (e.g., `SELECT class, retained_size FROM objects WHERE class LIKE 'com.example.%' AND retained_size > 1MB ORDER BY retained_size DESC`). Implement as a parser → AST → evaluator over the object store. Start with class/size filters, then expand to field access and predicates.
*Milestone:* M3 — requires M1 object graph (⚠️ M1.5 tag fix required) and M3 histogram improvements as prerequisites.

**Thread Inspection.**
*Current Status:* Not implemented. HPROF STACK_TRACE and STACK_FRAME records are skipped during parsing.
*Gap:* MAT links threads to their retained objects, showing which threads hold memory and through what call stack.
*Recommended Approach:* Parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT records. Link thread objects to their stack traces and to objects reachable from thread-local roots.
*Milestone:* M3 — uses M1 object graph (⚠️ M1.5 tag fix required) for object-to-thread linkage.

**ClassLoader Analysis.**
*Current Status:* Not implemented. CLASS_DUMP records are partially parsed in `gc_path` but classloader IDs are not stored or analyzed.
*Gap:* ClassLoader leaks (common in application servers and OSGi containers) cannot be detected without tracking the classloader hierarchy.
*Recommended Approach:* During CLASS_DUMP parsing, record the classloader reference for each class. Build a classloader tree. Detect leaks by finding classloaders that retain surprising amounts of memory.
*Milestone:* M3 — uses M1 object graph (⚠️ M1.5 tag fix required).

**Collection Inspection.**
*Current Status:* Not implemented.
*Gap:* MAT detects under-utilized collections (e.g., `HashMap` with 16 buckets and 1 entry, or `ArrayList` with capacity 1000 and 2 elements). These waste significant memory at scale.
*Recommended Approach:* Identify known collection class names during object graph traversal. Inspect internal fields (e.g., `HashMap.table.length` vs `HashMap.size`) to compute fill ratio. Report collections with low fill ratios or excessive capacity.
*Milestone:* M3 — uses M1 object graph (⚠️ M1.5 tag fix required) and field-value parsing.

**Large Dump Performance.**
*Current Status:* The streaming parser handles arbitrarily large files at the record level. The full object graph parser (`core::hprof::binary_parser`) is validated on populated real-world graphs for ~110MB and ~150MB dumps, and M3-P1-B1 added Criterion benchmark targets plus `scripts/measure_rss.sh` so throughput and RSS can now be measured consistently. The actual baseline data is still pending.
*Gap:* Unknown real-world memory usage. A populated graph from a 150MB dump may require significant RAM. A 4GB heap dump may contain 50M+ objects requiring 10-20GB of RAM for an in-memory adjacency list.
*Recommended Approach:* (1) Fix tag bug (M1.5) and measure actual memory usage with populated graphs from real dumps. (2) If RSS is acceptable, proceed with in-memory approach. (3) If memory is excessive, implement two-pass indexing or disk-backed storage.
*Milestone:* Memory measurement in M1.5. Optimization if needed in M3.

**Heap Snapshot Comparison.**
*Current Status:* `diff_heaps()` now preserves the existing record/class-stat view and, when both snapshots build object graphs, adds class-level instance, shallow-byte, and retained-byte deltas.
*Gap:* MAT can still go deeper with object-identity and reference-chain changes between snapshots.
*Recommended Approach:* Keep the current class-level graph diff as the default high-signal comparison and only pursue object-level identity tracking if a stable indexing layer is introduced.
*Milestone:* Class-level diff delivered in M3-P1-B2; object-level diff remains future work.

**Unreachable Objects.**
*Current Status:* ✅ M3-P1-B2 now reports objects not reachable from any GC root, including total unreachable count/shallow size plus per-class breakdown.
*Remaining Gap:* The current summary is aggregate-only; per-object drill-down and dedicated report views remain future work.
*Recommended Approach:* Keep BFS/DFS reachability from GC roots as the canonical implementation and reuse its output across report/UI surfaces.
*Milestone:* Delivered in M3-P1-B2.

---

## Section 4.5 — Predecessor & Competitor Analysis

This section provides a structured competitive analysis of Mnemosyne against two key predecessor/competitor projects in the JVM heap analysis space: **hprof-slurp** (Rust CLI) and **Eclipse MAT** (Java desktop, already analyzed in Section 4). The goal is to extract concrete lessons, identify feature gaps, and inform roadmap priorities.

### 4.5.1 Competitor Landscape Overview

| Dimension | hprof-slurp | Eclipse MAT | Mnemosyne |
|---|---|---|---|
| **Language** | Rust | Java (SWT) | Rust |
| **Architecture** | Streaming single-pass, multithreaded pipeline | Index-based, in-memory with disk indexes | Sequential `BufReader`, in-memory `ObjectGraph` |
| **Parser** | `nom` combinator library, handles incomplete inputs from chunked streaming | Custom Java parser with parallel indexing (v1.16.0+) | `byteorder` crate, sequential `Read` trait |
| **Memory model** | ~500MB flat for 34GB dumps (streaming, no intermediary results stored) | High (Java heap + disk indexes; requires heap proportional to dump size) | Unknown at scale (in-memory `HashMap`-backed `ObjectGraph`; untested on real data due to tag bug) |
| **Throughput** | ~2GB/s on 4+ cores (34GB in ~34s) | Slow initial parse, fast re-queries via indexes | Unknown (Criterion infrastructure landed in M3-P1-B1; results not yet published) |
| **Threading** | Multithreaded: file reader → parser → stats recorder via channels; 3×64MB prefetch buffer | Parallel indexing in recent versions | Single-threaded parser; Tokio runtime exists but parser is synchronous |
| **Analysis depth** | Shallow: top-N classes, top-N instances, strings, thread stacks. No graph, no retained sizes, no leak detection | Deep: full dominator tree, retained sizes, OQL, leak suspects, collection inspection, thread analysis, classloader analysis | Medium (architecture for depth exists but not yet validated on real data): dominator tree, retained sizes, leak detection, GC paths — all synthetic-only |
| **Output** | Text + JSON | GUI + batch HTML/CSV reports | Text + Markdown + HTML + TOON + JSON (5 formats) |
| **IDE/AI integration** | None | Eclipse plugin only | MCP server (7 handlers), AI stub architecture |
| **Provenance** | None | None | Full provenance system (unique) |
| **CI/CD story** | JSON output for scripting | Batch mode (limited) | JSON + TOON structured output, designed for CI |
| **Real-world validation** | Tested on 34GB Spring Boot production dumps | Industry standard, decades of production use | ⚠️ NOT validated on real data (tag bug) |
| **Stars/community** | ~140 stars, single developer, 26 releases | ~225 stars (GitHub mirror), 11+ contributors, decades of Eclipse ecosystem | Early stage, <10 stars |

### 4.5.2 hprof-slurp Deep Analysis

**Project:** [hprof-slurp](https://github.com/agourlay/hprof-slurp) — Apache 2.0, Rust, single developer (Arnaud Gourlay), v0.6.2 current, 26 releases.

**Design philosophy:** "Does not replace MAT/VisualVM — provides extremely fast overview to decide if deeper analysis is worthwhile." This is explicitly a triage tool, not a deep analyzer. It trades analysis depth for extreme speed and minimal memory usage.

#### Architecture Patterns Worth Learning From

1. **Streaming single-pass with `nom` parser combinators.** hprof-slurp uses `nom` to parse binary HPROF data in a streaming fashion. The `nom` library natively handles incomplete input — when a chunk boundary falls mid-record, `nom` returns `Incomplete` and the next chunk continues. This is materially better than Mnemosyne's current `Read`-based sequential approach for large files, because:
   - It naturally handles chunked I/O without manual buffer management
   - Parser code is declarative and composable
   - Error recovery is built into the combinator model
   - The same parser handles both complete and streaming inputs

   **Mnemosyne comparison:** Mnemosyne's `binary_parser.rs` uses `byteorder::ReadBytesExt` for field-by-field reading from a `BufReader`. This works but is (a) single-threaded, (b) cannot prefetch, and (c) requires the `Read` stream to always have complete records available. For multi-GB dumps, this will be a throughput bottleneck.

2. **Threaded pipeline with channel-based data flow.** hprof-slurp uses a producer-consumer pipeline: a file reader thread prefetches 64MB chunks, a parser thread consumes chunks and produces parsed records, and a statistics recorder thread aggregates results. The 3×64MB prefetch buffer (192MB) ensures the parser is never waiting on I/O.

   **Mnemosyne comparison:** Mnemosyne's parser is entirely single-threaded. The `parse_hprof_reader()` function reads and processes records synchronously. For summary-level parsing (`parser.rs`), this may be acceptable. For full graph construction (`binary_parser.rs`), which must process every sub-record, the lack of I/O prefetching and parallel processing will significantly limit throughput on large dumps.

3. **Fixed memory ceiling.** hprof-slurp maintains ~500MB RSS even for 34GB dumps because it never builds an in-memory representation of the full heap. It accumulates per-class statistics and top-N lists in bounded data structures.

   **Mnemosyne comparison:** Mnemosyne's `ObjectGraph` stores all objects, classes, references, and GC roots in `HashMap`s and `Vec`s. For a 34GB dump with potentially ~500M objects, this in-memory model would require tens of GB of RAM — likely exceeding host memory. Mnemosyne needs either (a) a memory-bounded "overview mode" similar to hprof-slurp for triage, or (b) a disk-backed/memory-mapped object store for deep analysis.

4. **Real-world validation methodology.** hprof-slurp is tested against real 34GB Spring Boot heap dumps from production scenarios. The author publishes benchmarks using `hyperfine` (timing) and `heaptrack` (memory profiling) against each release. Performance improvements are tracked release-over-release (70%+ improvement from v0.1.0 to v0.4.7).

   **Mnemosyne comparison:** Mnemosyne has zero benchmarks, zero real-world test fixtures, and the tag-constant bug went undetected because all 87 tests use synthetic HPROF data. Adopting hprof-slurp's validation discipline — real dumps + `hyperfine` + `heaptrack` — is a high-priority gap.

#### Feature Gaps Identified

| hprof-slurp Feature | Mnemosyne Status | Priority | Notes |
|---|---|---|---|
| Top-N allocated classes (size, count, largest instance) | ⚠️ Partial (record-level histogram) | P1 | Mnemosyne has class stats from streaming parser but not per-instance largest-instance tracking |
| Top-N largest instances | ❌ Missing | P2 | Useful for quick triage: "which single object is eating 2GB?" |
| String listing | ❌ Missing | P2 | hprof-slurp lists all String objects; useful for duplicate string detection |
| Thread stack trace display | ❌ Missing | P1 | hprof-slurp stitches STACK_TRACE + STACK_FRAME into coherent thread dumps. Already planned for M3 but should be elevated given hprof-slurp's success with this feature |
| JSON output for automation | ✅ Implemented | — | Mnemosyne has JSON + 4 other formats |
| ~2GB/s streaming throughput | ❌ Unknown | P1 | Criterion benchmark targets now exist, but no published numbers yet. Current single-threaded `byteorder` approach is likely significantly slower |
| ~500MB memory for 34GB dumps | ❌ Unlikely | P1 | In-memory `ObjectGraph` will balloon for large dumps. Need a bounded "overview mode" |
| Real-world 34GB dump validation | ❌ Missing | P0 | Mnemosyne has never been tested on dumps >150MB |
| `hyperfine` + `heaptrack` benchmarking | ❌ Missing | P1 | No performance measurement infrastructure exists |
| IntelliJ stacktrace compatibility | ❌ Missing | P3 | Nice-to-have: format thread stacks for IntelliJ's "Analyze stacktrace" |

#### Key Takeaway

hprof-slurp proves that a Rust HPROF parser can achieve **2GB/s throughput with 500MB memory** — but only by trading away analysis depth. Mnemosyne's strategic response should be: **(a)** adopt hprof-slurp's streaming performance model for a "fast overview" mode that matches its speed, **(b)** add a second "deep analysis" mode that builds the full object graph for MAT-class analysis depth, and **(c)** let the user choose based on their needs. This dual-mode architecture lets Mnemosyne serve as both a fast triage tool (replacing hprof-slurp) and a deep analyzer (replacing MAT).

### 4.5.3 Eclipse MAT Lessons (Supplementary to Section 4)

Section 4 provides the detailed feature-by-feature MAT parity analysis. This subsection adds architectural and strategic lessons from MAT's design:

1. **Index-based architecture for fast re-queries.** MAT builds disk-backed index files during the initial (slow) parse. Subsequent queries are fast because they read from indexes, not the raw dump. Mnemosyne should consider a similar pattern for the "deep analysis" mode — parse once, write an index, and serve queries from the index. This would enable a `mnemosyne serve --web` experience where the initial parse is slow but subsequent exploration is instant.

2. **Batch mode for CI.** MAT can run predefined report templates without the GUI. This validates Mnemosyne's CI/CD automation story — but Mnemosyne should go further with configurable analysis profiles (e.g., `--profile ci-regression` vs `--profile incident-response`).

3. **Historical security vulnerabilities.** MAT had XSS in HTML reports (CVE-2019-17634) and deserialization issues in index files (CVE-2019-17635). Mnemosyne's `escape_html()` hardening already addresses the XSS class. If/when Mnemosyne adds index files, deserialization safety must be designed in from the start.

4. **MAT's weakness is its strength.** MAT's Eclipse/SWT GUI is simultaneously its moat (rich interactive exploration) and its liability (dated UI, no CLI-first workflow, no CI story, no AI integration, no MCP). Mnemosyne should target the same analysis depth through modern interfaces.

### 4.5.4 Positioning Matrix

| Capability | hprof-slurp | Eclipse MAT | Mnemosyne (Current) | Mnemosyne (Target) |
|---|---|---|---|---|
| Parse 34GB dump | ✅ 34s, 500MB | ⚠️ Slow, high memory | ❌ Untested | ✅ Fast overview (<60s) + deep mode |
| Dominator tree | ❌ | ✅ Full | ⚠️ Synthetic-only | ✅ Real-world validated |
| Retained sizes | ❌ | ✅ Full | ⚠️ Synthetic-only | ✅ Real-world validated |
| Leak detection | ❌ | ✅ Advanced | ⚠️ Heuristic fallback only | ✅ Graph-backed + AI-assisted |
| Thread stacks | ✅ | ✅ With object linkage | ❌ | ✅ With object linkage |
| OQL / queries | ❌ | ✅ Full OQL | ❌ | ✅ Mini-query language |
| String analysis | ✅ List | ⚠️ Manual | ❌ | ✅ Duplicate detection + stats |
| Collection inspection | ❌ | ✅ | ❌ | ✅ Fill ratio analysis |
| AI/LLM integration | ❌ | ❌ | ⚠️ Stubbed | ✅ Real LLM-backed |
| MCP/IDE integration | ❌ | ❌ | ✅ | ✅ Production-ready |
| Provenance tracking | ❌ | ❌ | ✅ | ✅ |
| CI/CD automation | ⚠️ JSON only | ⚠️ Batch mode | ✅ JSON + TOON | ✅ Profiles + thresholds |
| Performance benchmarks | ✅ Published | ❌ | ❌ | ✅ Published + comparative |

### 4.5.5 Strategic Recommendations from Competitor Analysis

1. **Dual-mode parser architecture (P1, post-M1.5).** Add a "fast overview" mode that streams through the dump accumulating class statistics, top-N instances, and thread stacks WITHOUT building the full object graph. This mode should target hprof-slurp-class throughput (~1-2GB/s) and bounded memory (~500MB-1GB). The existing "deep analysis" mode (binary_parser → ObjectGraph → dominator) remains for full graph-backed analysis. Users select via `--mode overview` vs `--mode deep` (default: auto-select based on file size).

2. **Threaded I/O pipeline (P2, M3).** Adopt hprof-slurp's prefetch reader pattern: a dedicated I/O thread reads 64MB chunks ahead of the parser. This decouples I/O latency from parse computation and is a straightforward win for large-file throughput. Can be implemented with `std::sync::mpsc` channels or `crossbeam` channels.

3. **Benchmark infrastructure (P1, M1.5/M3).** Adopt hprof-slurp's benchmarking discipline: `criterion` for micro-benchmarks (parser throughput, graph construction, dominator computation), `hyperfine` scripts for end-to-end CLI timing, and `heaptrack` integration for memory profiling. Publish results in README and track regressions in CI.

4. **Thread stack trace extraction (P1, M3 — elevated from P2).** hprof-slurp demonstrates this is a high-value, moderate-effort feature. Parse `STACK_TRACE` (0x05) + `STACK_FRAME` (0x04) + `ROOT_THREAD_OBJECT` (0x08) records. Display thread dumps in a format compatible with IntelliJ's "Analyze stacktrace." This bridges the gap between hprof-slurp's triage utility and MAT's thread-to-object linkage.

5. **String analysis (P2, M3).** Parse String objects from the heap, detect duplicates, report top-N by count and total memory waste. hprof-slurp lists all strings; Mnemosyne should go further by quantifying the memory savings from string deduplication (interning).

6. **Memory-bounded object store evaluation (P1, M3).** After M1.5 fixes the tag bug, measure actual RSS when parsing real dumps of increasing size (10MB, 100MB, 1GB, 10GB). If the in-memory `ObjectGraph` exceeds 4× dump size in RSS, evaluate alternatives: memory-mapped storage (`memmap2`), disk-backed index (inspired by MAT), or a two-pass architecture (streaming stats first, then selective graph construction).

7. **Large-dump validation program (P1, M1.5+).** Source or generate heap dumps at multiple size tiers (10MB, 100MB, 1GB, 10GB, 30GB+) from diverse JVM environments (OpenJDK 11/17/21, GraalVM, Azul Zulu, different frameworks). Use these for validation, benchmarking, and regression testing. hprof-slurp's 34GB Spring Boot dump is the performance bar to beat.

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

> **Design Documents Index:**
> All milestone design documents live under [`docs/design/`](design/):
>
> | Milestone | Design Doc | Status |
> |---|---|---|
> | M1 — Stability & Trust | [milestone-1-stability-and-trust.md](design/milestone-1-stability-and-trust.md) | ✅ Complete (real-world validated via M1.5) |
> | M1.5 — Real-World Hardening | [milestone-1.5-real-world-hardening.md](design/milestone-1.5-real-world-hardening.md) | ✅ Complete |
> | M2 — Packaging, Releases, DX | [milestone-2-packaging-releases-dx.md](design/milestone-2-packaging-releases-dx.md) | ✅ Complete |
> | M3 — Core Heap Analysis Parity | [milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md) | ⚬ In Progress (M3-P1-B1 and M3-P1-B2 complete) |
> | M3-P1-B2 — Core Analysis Features | [m3-p1-b2-core-analysis-features.md](design/m3-p1-b2-core-analysis-features.md) | ✅ Complete |
> | M4 — UI & Usability | [milestone-4-ui-and-usability.md](design/milestone-4-ui-and-usability.md) | ⚬ Pending |
> | M5 — AI / MCP / Differentiation | [milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md) | ⚬ Pending |
> | M6 — Ecosystem & Community | [milestone-6-ecosystem-and-community.md](design/milestone-6-ecosystem-and-community.md) | ⚬ Pending |

### Milestone 1 — Stability & Trust

> **Design Reference:** [docs/design/milestone-1-stability-and-trust.md](design/milestone-1-stability-and-trust.md)

**Objective:** Make the core analysis trustworthy by building a real object graph, retained size computation, and dominator tree — the foundation everything else depends on.

**Why it matters:** Without a real object graph and retained sizes, Mnemosyne cannot make credible claims about memory analysis. This milestone delivers the analytical foundation.

**Status: ✅ COMPLETE — Real-world validated via M1.5.**

All M1 batches were delivered and validated. Initial synthetic-only validation revealed a critical HPROF tag-constant bug that prevented the graph-backed pipeline from activating on production data. M1.5 resolved this: tag constants were corrected, 0x1C dispatch was added, and the pipeline was validated end-to-end against real-world HPROF files with 101/101 tests passing (including 4 real-world integration tests).

**Delivered (real-world validated via M1.5):**
1. ✅ Sample HPROF test fixtures — `core::test_fixtures` builds synthetic and `HEAP_DUMP_SEGMENT` HPROF binaries for deterministic testing
2. ✅ Object graph data structures — `core::hprof::object_graph` defines `ObjectGraph`, `HeapObject`, `ClassInfo`, `GcRoot`, `FieldDescriptor`, etc.
3. ✅ Full object graph parser — parses binary HPROF records into `ObjectGraph`, including both `HEAP_DUMP` (0x0C) and `HEAP_DUMP_SEGMENT` (0x1C) records (tag fix landed in M1.5)
4. ✅ Real dominator tree — Lengauer-Tarjan algorithm validated on both synthetic and real-world data
5. ✅ Retained size computation — post-order accumulation validated on real-world data
6. ✅ Graph-backed analysis in `analyze_heap()` — pipeline activates on real-world dumps with meaningful results
7. ✅ CI pipeline — GitHub Actions for build + test + clippy + fmt
8. ✅ Unified `detect_leaks()` onto the graph-backed path — produces graph-backed results on real dumps
9. ✅ Rewrote GC path finder over the full object graph — `ObjectGraph` BFS activates on real-world dumps
10. ✅ Added object graph navigation API — `get_object(id)`, `get_referrers(id)`, `get_references(id)`
11. ✅ 101 passing tests (66 core + 5 CLI unit + 30 CLI integration) including real-world HPROF validation and leak-ID validation

**Dependencies:** None (this is the foundation)

**Modules/files affected:** `core/src/hprof/parser.rs`, `core/src/hprof/binary_parser.rs`, `core/src/hprof/object_graph.rs`, `core/src/graph/dominator.rs`, `core/src/graph/metrics.rs`, `core/src/analysis/engine.rs`, `core/src/graph/gc_path.rs`, `core/src/hprof/test_fixtures.rs`, `.github/workflows/ci.yml`

**Complexity:** Very High — this was the hardest milestone with the most new code.

**Definition of done (REVISED — original criteria were met on synthetic data only):**
- ✅ Can parse a real HPROF dump into a full object graph with reference edges — validated via M1.5
- ✅ Can compute retained sizes for any object — validated on real-world data
- ✅ Can produce a real dominator tree — validated with real objects on real-world dumps
- ✅ Leak detection uses retained-size data — graph-backed path activates on real dumps
- ✅ GC path uses full object graph — non-synthetic paths confirmed on real dumps
- ✅ 101 tests pass (71 core + 30 CLI integration) — includes real-world HPROF validation
- ✅ CI runs on every PR

**Closure:** M1.5 resolved the tag-constant bug and validated the full pipeline on real-world HPROF data.

---

### Milestone 2 — Packaging, Releases, and DX

> **Design Reference:** [docs/design/milestone-2-packaging-releases-dx.md](design/milestone-2-packaging-releases-dx.md)

**Objective:** Make Mnemosyne easy to install, use, and contribute to.

**Status:** ✅ Complete. Release automation, packaging metadata, crates.io publication, Homebrew formula checksums, CLI UX (spinners, colors, aligned comfy-table output with truncation disclosure), Docker image distribution, community files, contextual error handling, and documentation consistency passes are all shipped.

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

### Milestone 1.5 — Real-World Hardening

> **Design Reference:** [docs/design/milestone-1.5-real-world-hardening.md](design/milestone-1.5-real-world-hardening.md)

**Objective:** Fix the critical tag-constant bug, validate the graph-backed pipeline end-to-end against real-world HPROF files, and ensure the M1 foundation actually works on production data.

**Why it matters:** M1 delivered the architecture, data structures, and algorithms — but only validated them against synthetic HPROF test fixtures that use `HEAP_DUMP` (0x0C) records. Real-world JVM dumps use `HEAP_DUMP_SEGMENT` (0x1C), which the parser currently skips. Without this fix, ALL downstream analysis features (M3, M4, M5) are built on a foundation that does not work on real data. This is the highest-priority work in the project.

**Status:** ✅ COMPLETE — All deliverables shipped, 101/101 tests passing, real-world validated.

**Delivered:**
1. ✅ **P0: Fixed HPROF tag constants** — Corrected `TAG_HEAP_DUMP_SEGMENT` from `0x0D` to `0x1C` in `binary_parser.rs`. Fixed `tag_name()` in `parser.rs` (0x0D=CPU_SAMPLES, 0x0E=CONTROL_SETTINGS, 0x1C=HEAP_DUMP_SEGMENT, 0x2C=HEAP_DUMP_END). Fixed `gc_path.rs`.
2. ✅ **P0: Added HEAP_DUMP_SEGMENT parsing** — `binary_parser.rs` now processes tag `0x1C` records through the same sub-record parser as `HEAP_DUMP` (0x0C). Both tags work.
3. ✅ **P0: Real-world HPROF validation** — 4 real-world integration tests validate `parse`, `analyze`, `leaks`, and `gc-path` against actual JVM heap dumps. Tests skip gracefully when the optional `resources/test-fixtures/heap.hprof` fixture is absent.
4. ✅ **P1: End-to-end pipeline validation on real dumps** — Graph nodes >> 7, retained sizes are meaningful, GC paths use ObjectGraph BFS on real data.
5. ✅ **P1: Heuristic fallback validation** — `synthesize_leaks()` confirmed to produce candidates when graph-backed results are filtered away.
6. ✅ **P1: Leak-ID validation** — `validate_leak_id()` wired into `explain`, `fix`, and MCP `explain_leak`. Unknown IDs return `CoreError::InvalidInput`.
7. ✅ **P2: HEAP_DUMP_SEGMENT unit tests** — `build_segment_fixture()` added; dedicated segment-parsing tests prevent regression.

**Tests added:** 14 new tests (5 in M1.5-B1, 9 in M1.5-B2), bringing workspace total from 87 → 101.

**Dependencies:** None — this was the critical unblocker for all downstream work.

**Modules/files affected:** `core/src/hprof/parser.rs`, `core/src/hprof/binary_parser.rs`, `core/src/graph/gc_path.rs`, `core/src/analysis/engine.rs`, `cli/tests/integration.rs`, `core/src/hprof/test_fixtures.rs`

**Complexity:** Medium — tag fix was small; validation and testing was the bulk.

**Definition of done (ALL MET):**
- ✅ `mnemosyne parse` correctly labels tag 0x1C as HEAP_DUMP_SEGMENT and 0x0D as CPU_SAMPLES
- ✅ `binary_parser::parse_hprof_file()` produces a non-empty `ObjectGraph` from a real JVM heap dump
- ✅ `analyze_heap()` on a real dump shows object-level dominators (not record-tag-level)
- ✅ `detect_leaks()` on a real dump returns ≥1 leak candidate
- ✅ `gc-path` on a real dump returns a non-synthetic path at least some of the time
- ✅ `explain` and `fix` with an invalid leak-id return an error, not a generic response
- ✅ All existing 87 tests continue to pass + 14 new tests = 101 total
- ✅ CI runs clean

---

### Milestone 3 — Core Heap Analysis Parity

> **Design Reference:** [docs/design/milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md)

**Objective:** Close the feature gap with Eclipse MAT on core analysis capabilities.

**Why it matters:** Users choose heap analysis tools based on what they can answer. MAT is the benchmark. Mnemosyne needs to answer the same questions, better.

**Status:** ⚬ In Progress — Phase 1 batches M3-P1-B1 and M3-P1-B2 are complete. Benchmark scaffolding, RSS tooling, histogram grouping, MAT-style suspect ranking, unreachable-object analysis, and class-level diff are all landed. Remaining M3 work centers on thread inspection, ClassLoader analysis, collection inspection, OQL, and large-dump scaling.

**Key Deliverables:**
1. MAT-style leak suspects algorithm — objects with disproportionate retained vs shallow size
2. Histogram improvements — group by fully-qualified class, package, classloader
3. OQL-like query engine — simple query language for object inspection
4. Thread inspection — parse thread records + stack traces, link to objects
5. ClassLoader analysis — hierarchy parsing, per-loader stats, leak detection
6. Collection inspection — detect known collections, fill ratio, size anomalies
7. Unreachable objects analysis — report unreachable set after GC root reachability
8. Enhanced heap diff — object/class-level comparison (not just record-level)

**Dependencies:** M1 (object graph, retained sizes, dominator tree) — ✅ delivered and real-world validated. M1.5 (real-world hardening) — ✅ complete.

**Modules/files affected:** `core/src/analysis/engine.rs`, `core/src/hprof/parser.rs`, `core/src/graph/metrics.rs`, new `core/src/query.rs`, new `core/src/thread.rs`, new `core/src/collections.rs`

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

> **Design Reference:** [docs/design/milestone-4-ui-and-usability.md](design/milestone-4-ui-and-usability.md)

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

**Modules/files affected:** `core/src/report/renderer.rs`, new `core/src/web.rs` or `web/` crate, HTML templates, static assets

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

> **Design Reference:** [docs/design/milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md)

**Objective:** Wire real AI capabilities and make MCP integration production-ready.

**Why it matters:** AI-assisted analysis is the key differentiator. Without real LLM calls, the "AI-powered" promise is hollow.

**Dependencies note:** M1.5 must be complete before wiring AI to analysis results — sending empty/heuristic data to an LLM produces misleading output.

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

**Modules/files affected:** `core/src/analysis/ai.rs`, `core/src/mcp/server.rs`, `core/src/config.rs`, new `core/src/llm.rs`, new `core/src/prompts/` directory

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

> **Design Reference:** [docs/design/milestone-6-ecosystem-and-community.md](design/milestone-6-ecosystem-and-community.md)

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
- **Outcome:** `core::hprof::object_graph` defines `ObjectGraph`, `HeapObject`, `ClassInfo`, `FieldDescriptor`, `GcRoot`, `GcRootType`, `LoadedClass`, `ObjectKind`, and the string/class lookup tables. Canonical model used by parser, dominator, and analysis.

#### M1-B3: Object Graph Parser — HPROF Parsing ✅
- **Status:** Delivered
- **Outcome:** `core::hprof::binary_parser` parses binary HPROF strings, classes, GC roots, INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP into `ObjectGraph`. Uses `byteorder` for big-endian binary parsing. API: `parse_hprof(data: &[u8])`, `parse_hprof_file(path: &str)`.

#### M1-B4: Dominator Tree Algorithm ✅
- **Status:** Delivered
- **Outcome:** `core::graph::dominator::build_dominator_tree()` runs `petgraph::algo::dominators::simple_fast` (Lengauer-Tarjan) over the full object reference graph with a virtual super-root connected to all GC roots. Computes retained sizes via post-order accumulation. API: `top_retained(n)`, `retained_size(id)`, `immediate_dominator(id)`, `dominated_by(id)`, `node_count()`.

#### M1-B5: Retained Size Computation ✅
- **Status:** Delivered
- **Outcome:** `analysis::analyze_heap()` attempts graph-backed analysis first: `parse_hprof_file()` → `build_dominator_tree()` → graph-backed leak insights + dominator metrics. Falls back to heuristics with `ProvenanceKind::Fallback` / `ProvenanceKind::Partial` when parsing or filters prevent graph-backed results. `graph::build_graph_metrics_from_dominator()` populates `DominatorNode.retained_size` with real values.

#### M1-B6: Wire Graph Into Remaining Analysis Surfaces ✅
- **Status:** Delivered
- **Outcome:** Unified `detect_leaks()` onto the graph-backed path, rewrote the GC path finder to prefer `ObjectGraph` BFS with triple fallback (`ObjectGraph` BFS → budget-limited `GcGraph` → synthetic), and added navigation APIs on `ObjectGraph` (`get_object`, `get_references`, `get_referrers`).

#### M1-B7: Integration Tests ✅
- **Status:** Delivered
- **Outcome:** Added 16 CLI integration tests in `cli/tests/integration.rs` covering parse, leaks, analyze, gc-path, diff, fix, report, and config. The `test-fixtures` cargo feature eliminates fixture duplication; later M2 work expanded this to 23 integration tests and 87 passing tests overall.

### Milestone 2 Batches

#### M2-B1: CLI UX — Progress Bars and Colors
- **Goal:** Add progress bars (indicatif) for long-running operations and colorized output
- **Files/modules affected:** `cli/src/main.rs`, `cli/Cargo.toml` (add indicatif dep)
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered (spinners, colorized labels, and severity/provenance styling are now wired into the CLI)
- **Validation:** `mnemosyne parse large.hprof` shows a progress bar; errors are red, warnings yellow; tests pass
- **Risk notes:** Progress reporting requires parser to emit progress callbacks — may need parser interface change
- **Non-scope:** Do not change core analysis or report logic

#### M2-B2: Release Automation
- **Goal:** Set up GitHub Actions to cross-compile and publish release binaries for Linux/macOS/Windows on tag push
- **Files/modules affected:** `.github/workflows/release.yml` (new), `Cargo.toml` (version metadata)
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered (workflow added; tagged release path is now automated)
- **Validation:** Pushing a version tag produces a GitHub Release with binaries; the workflow also validates the tag version against `[workspace.package].version`
- **Risk notes:** Cross-compilation can be tricky; consider cross-rs or cargo-zigbuild
- **Non-scope:** Do not set up Homebrew or Docker yet

#### M2-B3: Packaging — cargo install + Homebrew
- **Goal:** Publish mnemosyne-cli to crates.io; create Homebrew formula
- **Files/modules affected:** `Cargo.toml` metadata, new `Formula/` or homebrew-tap repo
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered (crates.io publish is live and the Homebrew formula carries real SHA256 values for v0.1.1)
- **Validation:** `cargo install mnemosyne-cli` works; `brew install mnemosyne` works
- **Risk notes:** crates.io requires unique name; may need to check availability
- **Non-scope:** Do not set up Docker yet

#### M2-B5: Community Files and Templates
- **Goal:** Add issue templates, PR template, and core community files
- **Files/modules affected:** `.github/ISSUE_TEMPLATE/`, `.github/PULL_REQUEST_TEMPLATE.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered
- **Validation:** GitHub presents bug/feature issue templates and the repository includes contributor conduct and security reporting guidance
- **Risk notes:** Keep templates aligned with actual support expectations
- **Non-scope:** Do not expand into contributor-program documentation yet

#### M2-B6: Better Error Messages with Suggestions/Context
- **Goal:** Replace generic CLI/core failure paths with structured errors, actionable hints, and stronger validation for common user mistakes
- **Files/modules affected:** `core/src/errors.rs`, `core/src/hprof/parser.rs`, `cli/src/main.rs`, `cli/tests/integration.rs`, `core/src/lib.rs`
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered
- **Validation:** Missing-file errors suggest nearby `.hprof` paths, common wrong extensions produce targeted HPROF guidance, invalid config loads surface fix hints, and 3 new CLI integration tests cover the new error paths
- **Risk notes:** Error quality still depends on the parser surfacing enough phase/detail context for deeper failures
- **Non-scope:** Do not add table formatting or broader troubleshooting docs in this batch

#### M2-B7: Table-Formatted CLI Output
- **Goal:** Add comfy-table for aligned table output in CLI parse summaries and leak listings
- **Files/modules affected:** `cli/src/main.rs`, `cli/Cargo.toml`, `cli/tests/integration.rs`
- **Expected agent owner:** Implementation Agent
- **Status:** ✅ Delivered (comfy-table aligned tables, truncation-safe disclosure with row-stable identifiers, corrected parse summary wording)
- **Validation:** 87 tests passing (59 core + 5 CLI unit + 23 CLI integration), clippy clean
- **Risk notes:** None — shipped cleanly after 5 review cycles
- **Non-scope:** Core analysis, report rendering, MCP output unchanged

---

## Section 8 — Differentiation Strategy

### 8.1 Provenance-Aware Analysis (Unique)
No other heap analysis tool tags its output with data provenance. Mnemosyne's ProvenanceKind system (Synthetic, Partial, Fallback, Placeholder) gives users and automation clear signals about data trust. This is genuinely novel and should be highlighted in all marketing.

### 8.2 MCP-Native Workflows
Eclipse MAT has no IDE integration path. hprof-slurp has no IDE integration. Mnemosyne's MCP server makes it a memory debugging copilot inside VS Code, Cursor, Zed, and JetBrains. This is a first-mover advantage in the AI-assisted development tool space.

### 8.3 AI-Assisted Diagnosis
Once wired to real LLMs, Mnemosyne can explain memory issues in plain language, suggest fixes with code patches, and provide interactive Q&A about heap dumps. No other heap analyzer does this. The architecture is ready; the wiring is needed.

### 8.4 CI/CD Regression Detection
JSON and TOON output formats make Mnemosyne automation-friendly from day one. A CI pipeline can parse results, track trends, and alert on regressions. MAT is desktop-only with no CI story. hprof-slurp offers JSON but no regression-detection workflow or threshold configuration.

### 8.5 Rust Performance
Rust gives genuine advantages for large-dump analysis: no GC pauses, low memory overhead, predictable performance. For multi-GB dumps that choke Java-based tools, Mnemosyne should be measurably faster.

### 8.6 Modern Developer Experience
Clean CLI, multiple output formats, configuration hierarchy, provenance tracking, IDE integration — this is what modern developers expect. MAT's Eclipse-era UI feels dated. hprof-slurp is CLI-only with minimal UX polish. Mnemosyne can win on DX alone.

### 8.7 Scriptability & API-First
Every capability exposed via CLI is also available as a Rust library and via MCP. This API-first design enables integrations that aren't possible with GUI-only tools.

### 8.8 Dual-Mode Analysis: Triage + Deep (Planned)
Neither hprof-slurp nor Eclipse MAT offers both fast triage AND deep analysis in a single tool. hprof-slurp is fast-triage-only (no graph, no retained sizes). MAT is deep-analysis-only (slow initial parse, heavy memory). Mnemosyne's target architecture — a streaming "overview" mode for hprof-slurp-class speed plus a "deep" mode for MAT-class analysis — would be a unique dual-mode capability. Users would get fast triage in seconds, then drill into deep analysis when needed, all in one tool. This eliminates the current workflow of "run hprof-slurp first to decide if MAT is worth the wait."

### 8.9 Analysis Depth Beyond Both Predecessors (Planned)
hprof-slurp explicitly does not attempt leak detection, dominator trees, or retained sizes. MAT lacks AI integration, MCP workflows, and provenance tracking. Mnemosyne's target is the intersection: hprof-slurp's speed characteristics + MAT's analysis depth + AI-native insights + provenance trust signals + MCP IDE integration. No existing tool combines all five.

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
- Memory usage benchmarks (RSS at 10MB/100MB/1GB/10GB tiers — per Section 4.5 competitor analysis)
- Comparison with MAT, hprof-slurp, and VisualVM where fair
- Reproducible benchmark scripts in repo (`criterion` + `hyperfine` + `heaptrack` — per hprof-slurp's validation discipline)
- Performance bar: hprof-slurp achieves ~2GB/s streaming at ~500MB RSS for 34GB dumps; Mnemosyne's "overview" mode should target comparable throughput

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
| 1 | Object graph parser | P0 | High | XL | None | M1 | ✅ Done (synthetic) |
| 2 | Dominator tree algorithm | P0 | High | L | Object graph | M1 | ✅ Done (synthetic) |
| 3 | Retained size computation | P0 | High | M | Dominator tree | M1 | ✅ Done (synthetic) |
| 4 | Sample HPROF test fixtures | P0 | High | M | None | M1 | ✅ Done |
| 5 | CI pipeline (GitHub Actions) | P0 | High | M | None | M1 | ✅ Done |
| 6 | Unify `detect_leaks()` onto graph path | P0 | High | L | Object graph + retained sizes | M1 | ✅ Done (synthetic) |
| 7 | Rewrite GC path over full object graph | P0 | High | M | Object graph | M1 | ✅ Done (synthetic) |
| 8 | Object graph navigation API | P0 | High | M | Object graph | M1 | ✅ Done |
| 9 | Integration tests via reusable synthetic HPROF fixtures | P0 | High | L | Test fixtures + CI | M1 | ✅ Done |
| **9a** | **Fix HPROF tag constants (0x0D/0x1C swap)** | **P0** | **Critical** | **S** | **None** | **M1.5** | **✅ Done** |
| **9b** | **Add HEAP_DUMP_SEGMENT (0x1C) parsing support** | **P0** | **Critical** | **M** | **Tag fix (9a)** | **M1.5** | **✅ Done** |
| **9c** | **Real-world HPROF test fixture + validation tests** (all 87 tests are synthetic-only — this gap directly allowed the tag bug to ship) | **P0** | **Critical** | **M** | **Tag fix (9a)** | **M1.5** | **✅ Done** |
| **9d** | **End-to-end pipeline validation on real dumps** | **P0** | **High** | **M** | **9a + 9b + 9c** | **M1.5** | **✅ Done** |
| **9e** | **Investigate heuristic fallback zero-results on real data** | **P1** | **High** | **M** | **Tag fix (9a)** | **M1.5** | **✅ Done** |
| **9f** | **Leak-ID validation for explain/fix commands** | **P1** | **Medium** | **S** | **None** | **M1.5** | **✅ Done** |
| 10 | Release binaries | P1 | High | M | CI pipeline (✅) | M2 | ✅ Done |
| 11 | cargo install support | P1 | High | S | Release setup | M2 | ✅ Done |
| 12 | CLI progress bars + colors | P1 | Medium | S | None | M2 | ✅ Done |
| 12a | Table-formatted CLI output | P1 | Medium | S | CLI UX (✅) | M2 | ✅ Done |
| 13 | MAT-style leak suspects | P1 | High | L | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 14 | Histogram by class/package/classloader | P1 | High | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 15 | Homebrew formula | P1 | Medium | S | Release binaries | M2 | ✅ Done |
| 16 | LLM integration (real API calls) | P1 | High | L | M1.5 ✅ + M3 analysis context | M5 | ⚬ Pending |
| 17 | Enhanced heap diff | P1 | Medium | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 18 | Static interactive HTML reports | P2 | High | L | Reporting exists | M4 | ⚬ Pending |
| 19 | OQL query engine | P2 | High | XL | M3 histogram/suspects | M3 | ⚬ Pending |
| 20 | Thread inspection | P2 | Medium | L | M1.5 ✅ | M3 | ⚬ Pending |
| 21 | ClassLoader analysis | P2 | Medium | L | M1.5 ✅ | M3 | ⚬ Pending |
| 22 | Local web UI | P2 | High | XL | HTML reports | M4 | ⚬ Pending |
| 23 | Collection inspection | P2 | Medium | M | M1.5 ✅ | M3 | ⚬ Pending |
| 24 | Unreachable objects | P2 | Medium | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 25 | Configurable prompt/task runner | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 26 | AI conversation mode | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 27 | Docker image | P2 | Medium | S | Release automation | M2 | ✅ Done |
| 28 | Example projects + sample dumps | P2 | Medium | M | Test fixtures (✅) | M6 | ⚬ Pending |
| 29 | Benchmark suite (`criterion`) | P1 | Medium | M | M1.5 ✅ | M3 | ⚠️ Partial — Criterion bench targets landed in M3-P1-B1 |
| 30 | Plugin/extension system | P3 | Medium | XL | Stable APIs (M3+) | M6 | ⚬ Pending |
| 31 | Full interactive heap browser | P3 | High | XL | Web UI + OQL | M4 | ⚬ Pending |
| 32 | Local LLM support | P3 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 33 | Real MCP API documentation for `docs/api.md` (JSON-RPC signatures, schemas, examples) | P2 | Medium | M | MCP server (✅) | M3 | ⚬ Pending |
| 34 | Real usage examples in `docs/examples/` (CLI workflows, analysis sessions, MCP integration) | P2 | Medium | M | M1.5 ✅ | M3/M6 | ⚬ Pending |
| 35 | README badge version qualifier (`v0.2.0-alpha`) | P3 | Low | S | None | M3 | ⚬ Pending |
| 36 | Dockerfile base image CVE triage (`debian:bookworm-slim` known vulnerabilities) | P2 | Medium | S | None | M3 | ⚬ Pending — route to Security Agent |
| **37** | **Streaming "overview" mode — bounded-memory class/instance stats without full graph (inspired by hprof-slurp)** | **P2** | **High** | **L** | **M1.5 ✅** | **M3** | **⚬ Pending** |
| **38** | **Thread stack trace extraction — parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT (inspired by hprof-slurp)** | **P1** | **High** | **L** | **M1.5 ✅** | **M3** | **⚬ Pending** |
| **39** | **Benchmark infrastructure — `criterion` micro-benchmarks + `hyperfine` CLI timing + `heaptrack` memory profiling** | **P1** | **High** | **M** | **M1.5 ✅** | **M3** | **⚠️ Partial — Criterion benches landed; `hyperfine`/`heaptrack` still pending** |
| **40** | **Top-N largest instances report — per-class largest single instance size** | **P2** | **Medium** | **S** | **Object graph ✅** | **M3** | **⚬ Pending** |
| **41** | **String analysis — list strings, detect duplicates, quantify dedup savings** | **P2** | **Medium** | **M** | **Object graph ✅** | **M3** | **⚬ Pending** |
| **42** | **Threaded I/O pipeline — prefetch reader with 64MB chunked read-ahead via channels** | **P2** | **Medium** | **L** | **Benchmark baseline (39)** | **M3** | **⚬ Pending** |
| **43** | **Memory-bounded object store evaluation — measure RSS at various dump sizes, evaluate memmap2 or disk-backed index if >4× ratio** | **P1** | **High** | **M** | **M1.5 ✅** | **M3** | **⚠️ Partial — RSS script + decision template landed; baseline data pending** |
| **44** | **Large-dump validation program — source/generate dumps at multiple size tiers from diverse JVMs** | **P1** | **High** | **M** | **M1.5 ✅** | **M3/M6** | **⚬ Pending** |
| **45** | **`nom` parser evaluation — prototype `nom`-based binary parser, compare throughput to current `byteorder` approach** | **P2** | **Medium** | **L** | **Benchmark baseline (39)** | **M3** | **⚬ Pending** |
| **46** | **Configurable analysis profiles — `--profile ci-regression` vs `--profile incident-response` vs `--profile overview`** | **P2** | **Medium** | **M** | **Dual-mode parser (37)** | **M3/M5** | **⚬ Pending** |
| **47** | **IntelliJ stacktrace format compatibility — format thread stack output for IntelliJ's "Analyze stacktrace"** | **P3** | **Low** | **S** | **Thread stacks (38)** | **M3** | **⚬ Pending** |
| **48** | **Index/cache file for fast re-queries — write a disk-backed index during initial parse** | **P3** | **Medium** | **XL** | **M3 analysis features** | **M4/M6** | **⚬ Pending** |

---

## Section 11 — Recommended Immediate Next Steps

**✅ M1, M1.5, and M2 are all COMPLETE.** The graph-backed pipeline is validated on real-world HPROF files, distribution is live, and M3 is fully unblocked. The workspace carries 101 passing tests (66 core + 5 CLI unit + 30 CLI integration).

### Previously Completed Steps (M1.5)
1. ✅ Fix HPROF tag constants — `TAG_HEAP_DUMP_SEGMENT` corrected to `0x1C`
2. ✅ Add HEAP_DUMP_SEGMENT (0x1C) to binary parser dispatch
3. ✅ Real-world HPROF test fixture + validation — 4 integration tests against real JVM dumps
4. ✅ Investigate heuristic fallback zero-results — validated with nonexistent-package filter test
5. ✅ Leak-ID validation for explain/fix — `validate_leak_id()` wired into CLI and MCP
6. ✅ HEAP_DUMP_SEGMENT unit tests — `build_segment_fixture()` + dedicated parser tests

### Step 6 (NEXT): v0.2.0 Correctness Release + Benchmark Baseline
**Why first:** v0.1.1 users who tried real dumps got broken output due to the tag bug. The M1.5 fixes must be released. A lightweight benchmark baseline prevents future regressions as M3 features land.
**Actions:**
  - (a) Tag and release v0.2.0 containing all M1.5 fixes (tag correction, leak-ID validation, real-world tests)
  - (b) Run the newly added Criterion benchmarks for parser throughput, graph construction, and dominator computation, then publish the first baseline (backlog #29/#39)
  - (c) Use `scripts/measure_rss.sh` to measure RSS on the existing 110-150MB real-world test dumps and fill in `docs/design/memory-scaling.md` (backlog #43 — partial)
  - (d) Update CHANGELOG and metadata for v0.2.0
**Files:** `Cargo.toml` (version), `CHANGELOG.md`, new `benches/` directory
**Owner:** Implementation Agent
**Effort:** Small
**Dependencies:** None — all M1.5 work is merged

### Step 7: M3 Phase 1 — Core Analysis Features
**Status:** ✅ Complete in M3-P1-B2.
**Delivered:**
  - (a) **MAT-style leak suspects** (backlog #13) — ranking by retained/shallow size ratio, accumulation-point detection, dominated-count context, short reference-chain context, and composite suspect score
  - (b) **Histogram improvements** (backlog #14) — graph-backed grouping by FQN class, package prefix, and classloader via `mnemosyne analyze --group-by class|package|classloader`
  - (c) **Enhanced heap diff** (backlog #17) — class-level instance/shallow/retained deltas layered on top of the existing record-level comparison
  - (d) **Unreachable objects** (backlog #24) — reachability traversal from GC roots with total count/shallow size and per-class breakdown
**Files:** `core/src/analysis/engine.rs`, `core/src/graph/metrics.rs`, `core/src/hprof/parser.rs`, `core/src/config.rs`, `core/src/report/renderer.rs`, `core/src/mcp/server.rs`, `cli/src/main.rs`
**Owner:** Implementation Agent
**Effort:** Completed across one implementation batch
**Dependencies:** Satisfied by M1.5 graph-backed parsing and M3-P1-B1 benchmark/test scaffolding

### Step 8: M3 Phase 2 — Investigation Features
**Why next:** These features complete the investigative toolkit, bridging the gap with both hprof-slurp (thread stacks, top-N) and MAT (collection inspection, string analysis).
**Actions:**
  - (a) **Thread inspection** (backlog #20/#38) — parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT, link threads to retained objects, new `mnemosyne threads` command
  - (b) **Top-N largest instances** (backlog #40) — per-class largest single instance; useful for quick triage
  - (c) **String analysis** (backlog #41) — list string objects, detect duplicates, quantify dedup savings
  - (d) **Collection inspection** (backlog #23) — detect HashMap/ArrayList/etc., compute fill ratio, report waste
**Files:** new `core/src/thread/` module, `core/src/hprof/binary_parser.rs` (STACK_TRACE parsing), `core/src/graph/metrics.rs`, `cli/src/main.rs`
**Owner:** Implementation Agent
**Effort:** Large (2-3 batches)
**Dependencies:** Step 7 (Phase 1 provides histogram/suspects infrastructure); can partially overlap

### Step 9 (parallel with Step 8): Performance & Scalability Assessment
**Why here:** After M3 Phase 1 adds more analysis features, we need to confirm the pipeline scales. Before building OQL or the web UI, we must know memory behavior on 1GB+ dumps.
**Actions:**
  - (a) **Large-dump validation** (backlog #44) — source/generate dumps at 100MB/500MB/1GB/5GB tiers
  - (b) **Memory-bounded evaluation** (backlog #43 — full) — RSS profiling at each tier; if >4× ratio, evaluate alternatives
  - (c) **Streaming overview mode** (backlog #37) — implement bounded-memory "overview" mode if large-dump testing reveals scaling issues
  - (d) Decision point: if RSS is acceptable up to 1-2GB dumps, defer streaming overview to post-M3; if RSS blows up, address before M3 Phase 2
**Files:** `benches/`, `core/src/hprof/binary_parser.rs`, potentially `core/src/hprof/parser.rs`
**Owner:** Implementation Agent + Testing Agent
**Effort:** Medium-Large
**Dependencies:** Step 6 (benchmark baseline)

### Step 10: M3 Phase 3 — Advanced Features (after Phase 1 + 2)
**Actions:**
  - (a) **ClassLoader analysis** (backlog #21) — parse classloader hierarchy, per-loader stats, leak detection
  - (b) **OQL query engine** (backlog #19) — mini-query language over the object graph
  - (c) **Configurable analysis profiles** (backlog #46) — `--profile ci-regression|incident-response`
**Owner:** Implementation Agent
**Effort:** Very Large (OQL alone is XL)
**Dependencies:** Steps 7-8 (M3 Phase 1 + 2 provide the data model and analysis surfaces)

### Post-M3 Sequence
After M3 completes, the recommended order is:
1. **v0.3.0 release** — first release with MAT-parity analysis features
2. **M5 (AI/MCP Differentiation)** — wire real LLM calls to the now-meaningful analysis data
3. **M4 (UI & Usability)** — build interactive HTML reports and web UI on top of rich M3 analysis
4. **M6 (Ecosystem & Community)** — docs, examples, benchmarks, community infrastructure

**Note on M4/M5 ordering:** M5 (AI) is recommended before M4 (web UI) because:
- AI integration adds value to existing CLI/MCP surfaces immediately (no new UI needed)
- The web UI benefits from showing AI-generated insights alongside traditional analysis
- MCP handler output quality improves dramatically once AI is wired, improving the IDE copilot experience
- M4 is the largest remaining milestone; M5 can deliver value while M4 is designed

### Pre-M3 Debt Items (tracked)
The following items should be addressed alongside or shortly after M3 Phase 1:
- **`docs/api.md` placeholder scaffolding** — needs real MCP API documentation (backlog #33)
- **`docs/examples/` placeholder** — needs real CLI/MCP usage examples (backlog #34)
- **Dockerfile base image CVEs** — `debian:bookworm-slim` has known vulnerabilities; route to Security Agent (backlog #36)

---

## Section 12 — Risk Register & Lessons Learned

### Active Risks

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **In-memory ObjectGraph may not scale to 1GB+ dumps** | High | High | M1.5 validated on 110-150MB dumps. A 10GB dump with 100M+ objects could require 20-40GB RSS. Benchmark baseline (Step 6) measures current RSS. If >4× ratio at 500MB, evaluate memmap2/disk-backed alternatives before M3 Phase 2. Streaming overview mode (backlog #37) is the fallback architecture. |
| **Dockerfile base image (`debian:bookworm-slim`) has known CVEs** | Medium | High | Base image carries known vulnerabilities per container scanning. Route to Security Agent for assessment. Options: update to latest bookworm-slim, switch to distroless, or pin a patched digest. |
| **M3 scope is very large (15 features across 3 phases)** | Medium | High | Phased approach mitigates: Phase 1 (4 features) delivers core value before committing to Phases 2-3. Strict scope discipline required — no scope expansion without orchestration approval. |
| **Parser throughput may be significantly below hprof-slurp baseline** | Medium | High | hprof-slurp achieves ~2GB/s with multithreaded `nom`-based parsing. Mnemosyne's single-threaded `byteorder` approach is likely 2-5× slower. Step 6 benchmark baseline will quantify the gap. Threaded I/O (backlog #42) and `nom` evaluation (backlog #45) are mitigation paths. |
| **Other HPROF sub-record types may have parsing bugs** | Medium | Medium | Real-world dumps may contain sub-record types not covered by current fixtures. M3 Phase 2 (thread inspection) will exercise STACK_TRACE/STACK_FRAME records, expanding coverage. Large-dump validation (Step 9) further mitigates. |
| **hprof-slurp may add analysis depth features** | Low | Low | hprof-slurp is actively maintained. If it adds dominator tree/retained sizes, Mnemosyne's "analysis depth" differentiator narrows. Mitigation: deliver M3 analysis features + AI integration to establish the depth+AI moat. |
| **Eclipse MAT may modernize its CLI/API** | Low | Low | MAT recently moved to GitHub and shipped parallelism improvements. If MAT adds a modern CLI or AI features, Mnemosyne's DX differentiator narrows. Mitigation: move quickly on MCP, AI, and modern CLI to establish the DX moat. |
| **AI integration is entirely unvalidated** | Medium | Low (deferred) | No urgency until M3 provides meaningful analysis data to send to an LLM. M5 is sequenced after M3 specifically for this reason. |

### Resolved Risks

| Risk | Resolution |
|---|---|
| **Tag-constant bug (P0 correctness)** | ✅ Fixed in M1.5-B1: `TAG_HEAP_DUMP_SEGMENT` corrected to `0x1C`. `tag_name()` mappings corrected. 4 real-world integration tests prevent regression. |
| **Heuristic fallback tuning** | ✅ Validated in M1.5-B2: fallback path produces candidates when graph results are filtered. Tested with nonexistent-package filter. |
| **Leak-ID commands return generic responses** | ✅ Fixed in M1.5-B2: `validate_leak_id()` returns `CoreError::InvalidInput` for unknown IDs. 2 integration tests + 2 unit tests. |

### Lessons Learned (v0.1.1 Real-World Validation)

1. **Synthetic-only test coverage creates false confidence.** All 87 tests passed, clippy was clean, CI was green — but the tool produced incorrect output on every real-world dump. Lesson: real-world HPROF test fixtures are mandatory, not nice-to-have.
2. **Tag constant errors are insidious.** The HPROF spec uses sequential hex values (0x0C, 0x0D, 0x0E) for unrelated record types and then jumps to 0x1C/0x2C for segment/end. This is a spec design that invites off-by-one style errors. Multiple independent sources map these tags differently. Lesson: verify tag constants against the authoritative JDK source (`hprof_b_spec.h`), not third-party reference docs.
3. **Silent fallback can mask critical bugs.** The provenance system correctly labeled outputs as `[PARTIAL]` and `[FALLBACK]`, but the user experience was "analyze works, just with limited data" rather than "the parser is completely failing on your dump." Lesson: consider adding a warning when the graph-backed path fails entirely and ALL results are fallback.
4. **The features that work well are genuinely good.** `parse`, `diff`, `config`, reporting formats, error handling, and the provenance system all performed correctly on real-world data. The issue is specifically in the HPROF binary parser’s tag dispatch, not in the overall architecture or downstream pipeline.
5. **Cross-platform builds work.** Windows binary confirmed functional on real Kotlin+Spring Boot dumps. This is a meaningful achievement for a Rust CLI tool targeting JVM developers.

---

*This roadmap is a living document. Update it after each major batch completion.*
*Last review: post-M3-P1-B1 documentation sync — benchmark scaffolding, RSS tooling, and tag-centralization status aligned (2026-03-08).*
*Next review: after v0.2.0 release + first published benchmark/RSS baseline.*
