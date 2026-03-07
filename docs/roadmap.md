# Mnemosyne Roadmap & Milestones

> **Last updated:** 2026-03-08 (post v0.1.1 real-world validation)
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
11. [Recommended Immediate Next Steps](#section-11--recommended-immediate-next-steps)
12. [Risk Register & Lessons Learned](#section-12--risk-register--lessons-learned)

---

## Section 1 — Executive Summary

Mnemosyne is today an **alpha-stage Rust-based JVM heap analysis tool**. It can stream-parse HPROF files to produce class histograms and heap summaries, parse binary HPROF records into a full object reference graph (`core::hprof::binary_parser` → `core::hprof::object_graph`), compute a real dominator tree via Lengauer-Tarjan (`core::graph::dominator`), derive retained sizes from post-order subtree accumulation, run graph-backed analysis in both `analyze_heap()` and `detect_leaks()` with automatic fallback to heuristics when parsing fails, trace GC root paths with `ObjectGraph` BFS first plus layered fallbacks, expose object navigation via `get_object(id)`, `get_references(id)`, and `get_referrers(id)`, generate template-based fix suggestions, and render results in five output formats (Text, Markdown, HTML, TOON, JSON) — all backed by a provenance system that labels every synthetic, partial, fallback, or placeholder data surface. A stdio MCP server exposes seven JSON-RPC handlers (`parse_heap`, `detect_leaks`, `map_to_code`, `find_gc_path`, `explain_leak`, `propose_fix`, `apply_fix`), making the tool available inside VS Code, Cursor, Zed, JetBrains, and ChatGPT Desktop. The AI module (`analysis::generate_ai_insights`) is **fully stubbed**: it returns deterministic template text with zero LLM calls and zero HTTP client dependencies.

Mnemosyne has the foundations to become **the first Rust-native, AI-assisted heap analysis platform** that rivals Eclipse MAT in analysis depth while offering capabilities no existing tool provides: provenance-tracked outputs that distinguish real analysis from heuristic guesses, MCP-native IDE integration for copilot-style workflows, CI/CD-friendly automation via structured JSON and TOON output, and an AI-native architecture designed from day one for LLM integration. The Rust core means multi-GB heap dumps can be processed with predictable memory usage and no GC pauses — a meaningful advantage over Java-based tools like MAT and VisualVM for production incident response.

Five properties position Mnemosyne to stand out in a crowded JVM tooling ecosystem: **(1)** Rust performance enabling streaming analysis of heap dumps that exceed host RAM; **(2)** a provenance system unique among heap analyzers, giving users and automation confidence in result trustworthiness; **(3)** MCP-first architecture that makes heap analysis a conversation in the developer's IDE rather than a separate tool; **(4)** AI-native design with well-shaped type contracts (`AiInsights`, `AiWireExchange`, config plumbing) ready for LLM wiring; and **(5)** automation-friendly structured output (JSON, TOON) enabling CI regression detection with machine-readable leak signals.

**Critical update (2026-03-08): Real-world validation against two Kotlin + Spring Boot heap dumps (~110MB and ~150MB) has revealed that the graph-backed analysis pipeline does NOT activate on production HPROF files.** Root cause: the HPROF tag constants in both `parser.rs` and `binary_parser.rs` are incorrect — tag `0x0D` is mapped as `HEAP_DUMP_SEGMENT` when the HPROF spec defines it as `CPU_SAMPLES`, and tag `0x1C` (the real `HEAP_DUMP_SEGMENT`) is mapped as `CPU_SAMPLES`. Since virtually all modern JVM heap dumps store object data in `HEAP_DUMP_SEGMENT` (0x1C) records, the binary parser silently skips all real heap data on production dumps and the system falls back to heuristic-only mode. This explains: zero leak candidates, record-tag-level-only dominators (7 graph nodes for ~314K records), synthetic GC paths, and the mislabeled parse output. **Milestone 1 is reopened.** The graph-backed pipeline works on synthetic test fixtures (which use tag `0x0C` HEAP_DUMP) but NOT on real-world dumps (which use tag `0x1C` HEAP_DUMP_SEGMENT). M1 cannot be considered complete until the pipeline is validated end-to-end against real HPROF files.

Honest assessment: **significant work remains** to deliver on this vision. The architectural foundations — object graph model, dominator tree algorithm, retained-size computation, unified pipeline design, provenance system — are sound and well-implemented. However, a critical tag-constant bug means the graph-backed pipeline has only been validated against synthetic test fixtures, not real-world heap dumps. v0.1.1 completed the internal `core/src/` restructure from flat files into grouped module directories (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`) while preserving public API re-exports in `lib.rs`. The AI module remains 100% stubbed. An 87-test suite (59 core + 5 CLI unit + 23 CLI integration) now runs clean in GitHub Actions CI, the `test-fixtures` feature keeps canonical HPROF builders reusable across unit and integration coverage, tagged GitHub releases publish prebuilt binaries for five targets, v0.1.1 is the current release baseline, tagged releases publish a GHCR Docker image, and the CLI now emits structured suggestions for common file/config mistakes. Sample real-world heap dumps and benchmarks are still missing. **The immediate priority is fixing the tag-constant bug, validating the graph-backed pipeline against real-world HPROF files, and closing the M1.5 hardening milestone before any M3 work begins.** The architecture is sound; the implementation needs real-world hardening.

---

## Section 2 — Current State Assessment

### Core Capabilities

| Capability | Status | Honest Assessment |
|---|---|---|
| HPROF streaming parser | ⚠️ Tag bug | Two-tier parsing: `core::hprof::parser` streams headers + record tags for fast class histograms. `core::hprof::binary_parser` parses binary HPROF records into `ObjectGraph`. **CRITICAL BUG:** Both parsers have incorrect HPROF tag constants — `0x0D` is mapped as `HEAP_DUMP_SEGMENT` (spec: `CPU_SAMPLES`) and `0x1C` is mapped as `CPU_SAMPLES` (spec: `HEAP_DUMP_SEGMENT`). This means the binary parser looks for heap data at the wrong tag and silently skips real heap segments in production dumps. The streaming parser also mislabels the output (showing ~93-135MB of real heap data as "CPU_SAMPLES"). Works on synthetic fixtures (which use 0x0C HEAP_DUMP) but fails on real JVM dumps (which use 0x1C HEAP_DUMP_SEGMENT). |
| Leak detection heuristics | ⚠️ Fallback-only on real dumps | `detect_leaks()` attempts the graph-backed path first, then falls back to heuristics. **On real-world Kotlin+Spring dumps, the graph-backed path silently fails (due to the tag bug) and heuristic fallback produces zero candidates.** All 6 `leaks` invocations against real dumps returned empty results. Pipeline design is correct but requires the tag fix to activate on production data. |
| Graph / dominator tree | ⚠️ Synthetic-only validated | `core::graph::dominator::build_dominator_tree()` runs Lengauer–Tarjan over the full object reference graph with virtual super-root. Algorithm is correct on synthetic fixtures. **On real-world dumps, the object graph is empty (due to the tag bug), so the dominator tree operates on zero real objects.** `analyze` output shows "Graph Nodes: 7" (matching record *tag types*, not objects) confirming the graph is populated with summary metadata, not real heap objects. The algorithm itself is sound; it needs a correctly populated object graph to produce real results. |
| GC root path tracing | ⚠️ Fallback-only on real dumps | `core::graph::gc_path` tries `ObjectGraph` BFS first, then budget-limited `GcGraph`, then synthetic paths. On real-world dumps the ObjectGraph is empty (tag bug), so ALL paths are synthetic/fallback. The layered fallback design is correct and the provenance labels are honest, but the primary path never activates on production data. |
| AI / LLM insights | ❌ Stubbed | `core::analysis::generate_ai_insights()` returns deterministic template text. No HTTP client in `Cargo.toml`, no API calls, no LLM SDK. Config plumbing exists (`AiConfig` with provider/model/temperature fields) but terminates at the stub. The "AI-powered" claim in README is entirely aspirational. |
| Fix suggestions | ⚠️ Template only | `core::fix::propose_fix()` generates template patches in three styles (Minimal, Defensive, Comprehensive). No AI involvement, no code analysis. Useful scaffolding with provenance markers. |
| Source mapping | ✅ Implemented | `core::mapper::map_to_code()` scans project dirs for `.java`/`.kt` files, runs `git blame` for metadata. Basic but functional for local projects. |
| Reporting | ✅ Implemented | `core::report` renders 5 formats (Text, Markdown, HTML, TOON, JSON). HTML output uses `escape_html()` for XSS prevention. TOON uses `escape_toon_value()` for control characters. Provenance markers rendered in all non-JSON formats. One of the most polished subsystems. |
| MCP server | ✅ Wired | `core::mcp::serve()` runs a stdio JSON-RPC loop with async Tokio I/O. Handles 7 methods. Works end-to-end but backed by the same stubs/heuristics as CLI. |
| Config system | ✅ Implemented | `cli::config_loader` reads TOML files from 5 locations + env vars + CLI flags. `core::config` defines `AppConfig`, `AiConfig`, `ParserConfig`, `AnalysisConfig`. Clean, well-layered. |
| Provenance system | ✅ Implemented | `ProvenanceKind` (Synthetic, Partial, Fallback, Placeholder) + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in all report formats and CLI output. Unique feature in the heap-analysis space. |

### Technical Strengths

- **Rust performance model**: streaming parser with `BufReader`, no GC, predictable memory. Can handle files larger than RAM in principle.
- **Clean module separation**: grouped implementation domains in `core/src/` (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`) plus shared `config.rs`, `errors.rs`, and `lib.rs` re-exports.
- **Real object graph (synthetic-validated)**: `core::hprof::binary_parser` parses binary HPROF records into an `ObjectGraph` with objects, reference edges, class metadata, and GC roots. Works on synthetic fixtures using `HEAP_DUMP` (0x0C) records, but does not parse `HEAP_DUMP_SEGMENT` (0x1C) records used by real JVM dumps.
- **Real dominator tree (algorithm correct)**: `core::graph::dominator` implements Lengauer–Tarjan over the full object graph with virtual super-root. Computes retained sizes via post-order accumulation. Algorithm validated on synthetic data.
- **Graph-backed analysis pipeline (architecture sound, not yet real-world-validated)**: `analyze_heap()` attempts object-graph → dominator-tree → retained-size analysis first, with automatic fallback to heuristics. Provenance markers distinguish real from heuristic results. On real-world dumps, the pipeline currently always falls back due to the tag bug.
- **Streaming design**: `core::hprof::parser` processes HPROF records sequentially without loading the full dump. Foundation for scaling to multi-GB files.
- **Provenance system**: genuinely novel for a heap analyzer. Labels every synthetic/heuristic output surface so consumers know what to trust.
- **Multi-format output**: 5 report formats with consistent provenance rendering. HTML is XSS-hardened. TOON enables compact CI consumption.
- **87-test suite with CI**: 59 core + 5 CLI unit + 23 CLI integration tests running in GitHub Actions. Synthetic HPROF test fixtures plus the `test-fixtures` cargo feature enable deterministic parser, graph, end-to-end CLI testing, and targeted error-path coverage.
- **Config hierarchy**: TOML + env vars + CLI flags with clear precedence. Production-ready design pattern.
- **MCP integration**: stdio JSON-RPC server with 7 handlers. First-mover for heap analysis in the MCP ecosystem.
- **Type contracts**: well-shaped request/response types (`AnalyzeRequest`, `AnalyzeResponse`, `GcPathResult`, `FixResponse`, etc.) that establish stable contracts between CLI, MCP, and core.

### Major Weaknesses

- **CRITICAL: HPROF tag constants are wrong — graph-backed pipeline fails on real-world dumps.** `binary_parser.rs` defines `TAG_HEAP_DUMP_SEGMENT = 0x0D` but the HPROF spec says `0x0D = CPU_SAMPLES` and `0x1C = HEAP_DUMP_SEGMENT`. The streaming parser's `tag_name()` function has the same swap. Since modern JVMs write all heap object data into `HEAP_DUMP_SEGMENT` (0x1C) records, the binary parser silently skips all real heap data and the entire graph-backed pipeline (dominator tree, retained sizes, leak detection, GC paths) falls back to heuristics. This is a **P0 correctness bug** that undermines all analysis output on real data.
- **Graph-backed pipeline is synthetic-only validated.** All 87 tests use synthetic HPROF fixtures that store heap data in `HEAP_DUMP` (0x0C) records. No tests exercise `HEAP_DUMP_SEGMENT` (0x1C), which is the tag used by virtually all real JVM heap dumps. The pipeline architecture is sound but has never been proven on production data.
- **Leak detection returns zero results on real dumps.** All 6 leak detection invocations against real Kotlin+Spring Boot dumps (~110MB, ~150MB) returned empty — no table, no candidates, no output at all. The heuristic fallback path does not find candidates either, suggesting the heuristic thresholds or filters may also need tuning for real-world data.
- **AI is 100% stubbed**: `generate_ai_insights()` returns hardcoded template strings. There are zero HTTP client dependencies in `Cargo.toml`. The `AiConfig` fields (provider, model, temperature, API key) exist but connect to nothing. Every "AI-powered" claim in documentation is marketing ahead of implementation.
- **explain/fix commands ignore provided leak IDs.** `explain` with a fabricated leak-id doesn't error — returns generic response. `fix` generates patches for `com.example.CacheLeak` regardless of input. These commands don't validate leak-ids against actual heap data.
- **No benchmarks or performance data**: no `criterion` benchmarks for parser throughput, graph construction, dominator computation, or report rendering. Cannot track performance regressions or compare against MAT/VisualVM.
- **No sample real-world data**: synthetic test fixtures exist for deterministic testing, but no example real `.hprof` files for tutorials or development. This gap directly enabled the tag bug to go undetected.
- **Diff is record-level, not object-level**: `diff_heaps()` compares aggregate record/class statistics. It cannot track individual object migration or reference chain changes.
- **Graph module naming is misleading**: `summarize_graph()` still exists as a lightweight fallback that builds a synthetic tree from top-12 entries. Its name suggests more than it delivers, though the real dominator tree now exists alongside it.

### Maturity Assessment

| Subsystem | Maturity | Rationale |
|---|---|---|
| Parser | ⚠️ Pre-alpha (real-world) | `core::hprof::parser` handles record-level stats but mislabels tag 0x1C (HEAP_DUMP_SEGMENT) as CPU_SAMPLES. `core::hprof::binary_parser` parses synthetic HPROF correctly but has wrong tag constant for HEAP_DUMP_SEGMENT (0x0D instead of 0x1C), causing it to skip all heap data in real JVM dumps. Downgraded from Alpha+ until tag fix is validated. |
| Leak detection | ⚠️ Pre-alpha (real-world) | Pipeline design is correct but produces zero results on real Kotlin+Spring dumps due to tag bug. Heuristic fallback also returns empty. Downgraded until validated against real data. |
| Graph / Dominator | Alpha (synthetic-only) | Lengauer–Tarjan algorithm is implemented and correct on synthetic data. Not yet validated on real-world object graphs because the parser tag bug prevents real objects from entering the graph. |
| AI | Pre-alpha | Fully stubbed. Returns deterministic text. Not wired to any model. |
| GC root paths | Alpha | Real parsing of roots/instances within budget. Best-effort with fallback. Among the strongest features. |
| Fix suggestions | Alpha | Template-based scaffolding. No code analysis or AI involvement. |
| Source mapping | Alpha | Works for basic cases. No IDE integration beyond file scanning. |
| Reporting | Beta | 5 formats, XSS hardening, provenance rendering, well-tested. Ready for use. |
| MCP server | Alpha | Wired and functional but outputs depend on stubs/heuristics. |
| Config | Beta | Clean hierarchy, env + TOML + CLI. Production-ready pattern. |
| Provenance | Beta | Unique, well-integrated across all surfaces. Novel in the space. |
| Testing | Alpha+ | 87 tests (59 core + 5 CLI unit + 23 CLI integration). Synthetic HPROF test fixtures, reusable `test-fixtures` feature, and GitHub Actions CI. No property-based testing or benchmarks yet. |
| CI/CD | Alpha+ | GitHub Actions CI runs `cargo check`, `cargo test`, `cargo clippy`, and `cargo fmt --check` on pushes and PRs, and tagged releases now run a separate workflow that validates the tag version, cross-compiles `mnemosyne-cli` for five targets, packages archives, and publishes a GitHub Release. Nightly builds are still absent. |

---

## Section 3 — Gap Analysis

### 3.1 Correctness & Trust Gaps

**CRITICAL: HPROF tag constant mislabeling (P0).** Both `core::hprof::parser` and `core::hprof::binary_parser` have incorrect tag-to-name mappings:
- Tag `0x0D` is mapped as `HEAP_DUMP_SEGMENT` — HPROF spec says it is `CPU_SAMPLES`
- Tag `0x1C` is mapped as `CPU_SAMPLES` — HPROF spec says it is `HEAP_DUMP_SEGMENT`
- Tag `0x0E` is mapped as `HEAP_DUMP_END` — HPROF spec says it is `CONTROL_SETTINGS`
- Tag `0x2C` is mapped as `HEAP_DUMP_SEGMENT_EXT` — HPROF spec says it is `HEAP_DUMP_END`

This causes the binary parser to look for heap data at tag `0x0D` (which contains CPU sample data, usually absent) and ignore tag `0x1C` (which contains all real heap object data in modern JVM dumps). The result: the `ObjectGraph` is empty on real-world HPROF files, and the entire graph-backed pipeline silently falls back to heuristics.

**Object reference graph: implemented but not yet validated on real-world data.** The pipeline architecture is sound — `binary_parser` → `ObjectGraph` → `dominator` → retained sizes → leak detection — but the tag constant bug means it has only been exercised on synthetic HPROF fixtures (which use `HEAP_DUMP` tag 0x0C, not `HEAP_DUMP_SEGMENT` tag 0x1C). On two real-world Kotlin+Spring Boot dumps (~110MB, ~150MB), the binary parser produced an empty graph, the dominator tree showed 7 nodes (matching record tag types, not objects), leak detection returned zero candidates, and GC paths were all synthetic.

**Leak detection returns zero results on real-world data.** All 6 `leaks` invocations (default, `--min-severity medium`, `--leak-kind cache,thread`, `--package com.example`) against real dumps produced no output. Root cause is the empty object graph, but the heuristic fallback path also needs investigation — for dumps with ~314K records and 93-135MB of heap data, heuristics should find at least some candidates.

**explain/fix commands don't validate leak IDs.** `explain` with a fabricated leak-id returns a generic response instead of erroring. `fix` generates hardcoded patches for `com.example.CacheLeak` regardless of input. These commands need to validate leak-ids against actual heap data or return explicit "not found" errors.

- **Diff is record-level, not object-level.** `diff_heaps()` compares aggregate record/class statistics between two snapshots. It cannot track individual object migration, new allocation sites, or reference chain changes. (Note: the diff command itself works well and is one of the most useful features — the "delta" summary is accurate at the record level.)

**Provenance correctly labels data quality** — the system labels graph-backed results with no provenance marker (clean data) and heuristic/fallback results with `ProvenanceKind::Fallback` or `ProvenanceKind::Partial`, so consumers know what to trust. The provenance system worked as designed during real-world testing: `[PARTIAL]` labels were honestly displayed.

### 3.2 Testing & CI Gaps

- **87 tests** across the workspace (59 core + 5 CLI unit + 23 CLI integration). Tests cover provenance rendering, escape functions, analysis paths, HPROF parsing, object graph construction, dominator tree correctness, retained-size computation, CLI argument handling, end-to-end command execution, and targeted failure-path UX.
- **Synthetic HPROF test fixtures** exist in `core::test_fixtures`. Small deterministic binary HPROF files exercise the parser and graph pipeline without requiring a JVM or committing large binaries.
- **`test-fixtures` cargo feature** exposes canonical fixture builders to integration tests without widening the builder API surface.
- **CI pipeline running.** GitHub Actions (`.github/workflows/ci.yml`) runs `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and PRs.
- **23 end-to-end CLI integration tests.** `cli/tests/integration.rs` runs `parse`, `leaks`, `analyze`, `gc-path`, `diff`, `fix`, `report`, and `config` as subprocesses against synthetic HPROF fixtures and now also validates key error-path guidance.
- **No integration tests against real `.hprof` files.** Tests use synthetic fixtures only. Real-world heap dumps from production JVMs are not tested. **This gap directly allowed the tag-constant bug to ship undetected** — synthetic fixtures use `HEAP_DUMP` (0x0C) which works, while real JVM dumps use `HEAP_DUMP_SEGMENT` (0x1C) which the parser ignores.
- **No coverage tracking.** No `cargo-tarpaulin` or `cargo-llvm-cov` integration. Unknown actual coverage percentage.
- **No property-based testing.** Parser binary handling is a prime candidate for `proptest` or `quickcheck` fuzzing.
- **No benchmarks.** No `criterion` benchmarks for parser throughput, graph construction, dominator computation, or report rendering. Cannot track performance regressions.

### 3.3 Documentation & Onboarding Gaps

- **README and QUICKSTART now reflect shipped behavior.** Output examples in both files match the actual CLI table-based presentation. "AI-generated explanations" remain aspirational — AI is 100% stubbed.
- **`docs/api.md` is still placeholder scaffolding.** The file exists but contains no real MCP API documentation — no JSON-RPC method signatures, no request/response schemas, no usage examples. This is documentation debt that misleads contributors who expect an API reference. Needs real content covering all 7 MCP handlers with wire-format examples.
- **`docs/examples/` is still placeholder.** `docs/examples/README.md` exists but has no real usage examples. Needs real CLI workflow examples, sample analysis sessions, and MCP integration examples. Currently a dead end for anyone following the docs.
- **README badge still says `status-alpha-yellow`.** The badge does not include a version qualifier. Optionally update to a version-qualified badge (e.g., `v0.1.1-alpha`) for better clarity. Low priority but noted.
- **No tutorial or cookbook.** No guided walkthrough of a real analysis session. No examples of interpreting output or acting on leak candidates.
- **No troubleshooting guide.** No documentation for common errors, unsupported HPROF variants, or limitations.
- **No performance benchmarks published.** No data comparing Mnemosyne against MAT, VisualVM, or other tools. No `criterion` benchmark suite exists in the repository. Planned for M3/M6 but should be tracked explicitly as a prerequisite or parallel item to M3 analysis work.

### 3.4 Packaging & Release Gaps

- **Release distribution is live for v0.1.1.** `.github/workflows/release.yml` cross-compiles and packages `mnemosyne-cli` for five targets, publishes tagged GitHub releases, builds/pushes `ghcr.io/<owner>/mnemosyne` on tagged releases, and the current production release is now shipped across those channels.
- **✅ crates.io published** (`mnemosyne-core 0.1.1` + `mnemosyne-cli 0.1.1`).
- **✅ `cargo install mnemosyne-cli` is live.**
- **Docker delivery is now in place.** A multi-stage `Dockerfile` builds `mnemosyne-cli` into a non-root `debian:bookworm-slim` runtime image, and tagged releases publish `ghcr.io/<owner>/mnemosyne` with semver plus `latest` tags.
- **✅ SHA256 values filled for v0.1.1.** `HomebrewFormula/mnemosyne.rb` now contains release checksums for the tagged archives.
- **✅ CHANGELOG.md has `[0.1.1] - 2026-03-08` section.** Changelog updates are still manual.

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

The gap remains significant but the architectural path is clear. The object graph model, dominator tree algorithm, retained sizes computation, unified leak detection pipeline, and navigation API are all implemented — but the HPROF tag-constant bug means they have only been validated on synthetic fixtures, not real-world dumps. **M1.5 (tag fix + real-world hardening) must complete before the MAT parity gap can meaningfully close.** After M1.5, the next priorities are MAT-style suspect ranking, histogram grouping, and thread inspection.

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
| Dominator tree | ⚠️ Synthetic-only | Algorithm correct on synthetic data; produces tag-level-only nodes on real dumps due to tag bug | Fix tag constants (M1.5), then expose via CLI subcommand + MCP handler | Medium | Critical | M1 ⚠️ / M1.5 |
| Retained size | ⚠️ Synthetic-only | Algorithm correct on synthetic data; no real objects enter graph on production dumps | Fix tag constants (M1.5), then expose in more surfaces | Medium | Critical | M1 ⚠️ / M1.5 |
| Object graph traversal | ⚠️ Synthetic-only | Object graph model and API exist but binary parser skips HEAP_DUMP_SEGMENT records on real dumps | Fix tag constants (M1.5), validate on real data, then expose richer surfaces | Medium | Critical | M1 ⚠️ / M1.5 |
| Shortest path to GC roots | ⚠️ Fallback-only on real dumps | Falls back to synthetic paths on all real dumps because ObjectGraph is empty | Fix tag constants (M1.5), validate non-synthetic paths on real data | Medium | High | M1 ⚠️ / M1.5 |
| Leak suspects report | ⚠️ Partial | Pipeline design is graph-backed, but returns zero candidates on real dumps (tag bug). MAT-style suspect ranking not yet implemented | Fix M1.5 first, then implement accumulation-pattern analysis | High | Critical | M1.5 → M3 |
| Histogram by class/package/classloader | ⚠️ Partial | Record-level histogram only, no classloader or package grouping | Parse per-object data, group by fully-qualified class name, classloader, package | Medium | High | M3 |
| OQL / query language | ❌ Missing | No query capability | Design mini-query language or embed existing (e.g., SQL-like over object model) | Very High | High | M3 |
| Thread inspection | ❌ Missing | Not implemented | Parse HPROF STACK_TRACE + STACK_FRAME records, link threads to retained objects | High | Medium | M3 |
| ClassLoader analysis | ❌ Missing | Not implemented | Parse classloader hierarchy from CLASS_DUMP records, detect leaks per classloader | High | Medium | M3 |
| Collection inspection | ❌ Missing | Not implemented | Detect known collection types (`HashMap`, `ArrayList`, etc.), inspect fill ratio, size, waste | Medium | Medium | M3 |
| Export / reporting | ✅ Implemented | Good for current scope | Already strong: 5 formats, provenance, XSS hardening. Add CSV, protobuf, flamegraph later | Low | Medium | M2 |
| UI-based exploration | ❌ Missing | CLI only | Phase from TUI → static HTML → web UI → full explorer | Very High | High | M4 |
| Large dump performance | ⚠️ Partial | Streaming parser handles any size; in-memory object graph has not been tested on real dumps (tag bug). ~110MB and ~150MB dumps parse cleanly at the streaming level. | Fix M1.5 tag bug first, then assess real memory usage with populated graphs | High | High | M1.5 → M3 |
| Heap snapshot comparison | ⚠️ Partial | Record-level diff only | Diff at object/class level once object graph exists | Medium | Medium | M3 |
| Unreachable objects | ❌ Missing | Not implemented | After building reachability from GC roots, report unreachable set and sizes | Medium | Medium | M3 |

### Detailed Analysis per Feature

**Dominator Tree.**
*Current Status:* ⚠️ Algorithm implemented and correct on synthetic data; not validated on real-world dumps. `core::graph::dominator::build_dominator_tree()` runs `petgraph::algo::dominators::simple_fast` (Lengauer–Tarjan) over the full object reference graph with a virtual super-root. On real-world dumps, the object graph is empty due to the tag-constant bug, so the dominator tree produces only tag-level summary nodes.
*Remaining Gap:* Fix the tag-constant bug (M1.5) and validate on real data. After that, the dominator tree should be exposed as a standalone CLI subcommand, MCP handler, and browsable view.
*Next Steps:* Complete M1.5 first. Then add a `mnemosyne dominators` CLI command and MCP handler. Expose `top_retained(n)`, tree-browsing queries, and integrate into the future web UI.
*Milestone:* Core algorithm delivered in M1. Real-world validation in M1.5. Browsable view is M4.

**Retained Size.**
*Current Status:* ⚠️ Algorithm implemented and correct on synthetic data. `core::graph::dominator::build_dominator_tree()` computes retained sizes via post-order traversal. On real-world dumps, the computation produces no meaningful results because the object graph is empty (tag bug).
*Remaining Gap:* Fix tag-constant bug (M1.5), validate retained sizes on real data. Then expose in diff, histogram, and MCP surfaces.
*Next Steps:* Complete M1.5 first. Then expose retained sizes in `diff_heaps()` output, histogram views, and future explorer surfaces.
*Milestone:* Core computation delivered in M1. Real-world validation in M1.5. Broader surface integration in later milestones.

**Object Graph Traversal.**
*Current Status:* ⚠️ Basic architecture in place. `core::hprof::binary_parser` parses binary HPROF records into `core::hprof::object_graph::ObjectGraph` and the graph exposes `get_object(id)`, `get_referrers(id)`, and `get_references(id)`. However, the parser's `TAG_HEAP_DUMP_SEGMENT` constant is wrong (0x0D instead of 0x1C), so the graph is empty on real-world HPROF dumps.
*Remaining Gap:* Fix tag constant (M1.5), validate graph population on real data. Then surface navigation through richer CLI and MCP browsing experiences.
*Next Steps:* Complete M1.5. Then expose the existing navigation API through richer CLI and MCP browsing surfaces.
*Milestone:* Graph data structures and base navigation API delivered in M1. Real-world validation in M1.5. Richer explorer surfaces remain future work.

**Shortest Path to GC Roots.**
*Current Status:* ⚠️ Architecture correct with layered fallback. On real-world dumps, the `ObjectGraph` is empty (tag bug), so ALL paths are synthetic/fallback. The provenance labels are honest.
*Remaining Gap:* Fix tag bug (M1.5) so the primary `ObjectGraph` BFS path activates on real data.
*Next Steps:* Complete M1.5. Validate non-synthetic paths on real dumps.
*Milestone:* Core graph-backed path-finding architecture delivered in M1. Real-world activation in M1.5.

**Leak Suspects Report.**
*Current Status:* `detect_leaks()` and `analyze_heap()` are designed to produce graph-backed leak insights with retained-size data, with heuristic fallback when graph parsing fails. On real-world Kotlin+Spring Boot dumps, the graph-backed path silently fails (tag bug) and the heuristic fallback produces zero candidates.
*Remaining Gap:* (1) Fix tag bug so graph-backed path activates (M1.5). (2) Investigate why heuristic fallback returns zero candidates on real dumps. (3) After M1.5, implement MAT-style ranking: objects where retained_size >> shallow_size, accumulation point detection, reference chain context.
*Recommended Approach:* M1.5 first for tag fix + real-world validation. Then build on the delivered retained-size pipeline for MAT-style suspect ranking.
*Milestone:* Base pipeline delivered in M1 (synthetic). Fix and validation in M1.5. Advanced suspect ranking in M3.

**Histogram by Class/Package/ClassLoader.**
*Current Status:* `HeapSummary.classes` contains `ClassStat` entries derived from record tags. No classloader or package-level grouping.
*Gap:* MAT groups histograms by package prefix, classloader identity, and superclass — enabling users to quickly scope analysis to their own code.
*Recommended Approach:* With per-object data from M1 (⚠️ requires M1.5 tag fix), group by FQN prefix (package), by classloader object ID (from CLASS_DUMP), and by superclass chain. Expose as query parameters on the histogram API.
*Milestone:* M3 — uses M1 object graph for classloader data. **Blocked on M1.5.**

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
*Current Status:* The streaming parser handles arbitrarily large files at the record level. The full object graph parser (`core::hprof::binary_parser`) is designed to load all objects into memory, but due to the tag-constant bug, it has never been tested with a fully populated graph from real data. Two real-world dumps (~110MB, ~150MB) parse cleanly at the streaming level.
*Gap:* Unknown real-world memory usage. A populated graph from a 150MB dump may require significant RAM. A 4GB heap dump may contain 50M+ objects requiring 10-20GB of RAM for an in-memory adjacency list.
*Recommended Approach:* (1) Fix tag bug (M1.5) and measure actual memory usage with populated graphs from real dumps. (2) If RSS is acceptable, proceed with in-memory approach. (3) If memory is excessive, implement two-pass indexing or disk-backed storage.
*Milestone:* Memory measurement in M1.5. Optimization if needed in M3.

**Heap Snapshot Comparison.**
*Current Status:* `diff_heaps()` compares two `HeapSummary` values at the record/class-stat level. Reports delta bytes, delta objects, and changed classes.
*Gap:* MAT can diff at the object level, showing new objects, freed objects, and reference chain changes between snapshots.
*Recommended Approach:* With M1 object graphs now available, diff object sets by class and ID. Identify newly allocated objects, freed objects, and changed reference patterns. Report delta retained sizes per class.
*Milestone:* M3 — uses M1 object graph (⚠️ M1.5 tag fix required).

**Unreachable Objects.**
*Current Status:* Not implemented.
*Gap:* MAT reports objects not reachable from any GC root, which helps understand phantom memory and finalizer pressure.
*Recommended Approach:* After building the object graph and GC root set, mark all reachable objects via BFS/DFS. Report unmarked objects with their classes and sizes.
*Milestone:* M3 — uses M1 object graph (⚠️ M1.5 tag fix required) and GC root parsing.

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

**Status: ⚠️ REOPENED — Synthetic-validated but NOT real-world-validated.**

All M1 batches were delivered and pass on synthetic HPROF test fixtures. However, **real-world validation against Kotlin+Spring Boot heap dumps revealed a critical HPROF tag-constant bug** that prevents the graph-backed pipeline from activating on production data. M1 cannot be closed until the pipeline is validated end-to-end against real HPROF files. See **Milestone 1.5** for the hardening plan.

**Delivered (synthetic-validated):**
1. ✅ Sample HPROF test fixtures — `core::test_fixtures` builds synthetic HPROF binaries for deterministic testing
2. ✅ Object graph data structures — `core::hprof::object_graph` defines `ObjectGraph`, `HeapObject`, `ClassInfo`, `GcRoot`, `FieldDescriptor`, etc.
3. ⚠️ Full object graph parser — parses binary HPROF records into `ObjectGraph`, but `TAG_HEAP_DUMP_SEGMENT` is set to `0x0D` instead of the correct `0x1C`, causing all `HEAP_DUMP_SEGMENT` data in real dumps to be skipped
4. ✅ Real dominator tree — algorithm is correct on synthetic data; awaiting real-world validation
5. ✅ Retained size computation — algorithm is correct on synthetic data; awaiting real-world validation
6. ✅ Graph-backed analysis in `analyze_heap()` — pipeline design is correct but falls back to heuristics on all real dumps due to tag bug
7. ✅ CI pipeline — GitHub Actions for build + test + clippy + fmt
8. ✅ Unified `detect_leaks()` onto the graph-backed path — pipeline design correct but produces zero results on real dumps
9. ✅ Rewrote GC path finder over the full object graph — falls back to synthetic paths on all real dumps
10. ✅ Added object graph navigation API — `get_object(id)`, `get_referrers(id)`, `get_references(id)`
11. ✅ Added 16 CLI integration tests plus reusable `test-fixtures` feature — later expanded to 23 integration tests and 87 total passing tests (all synthetic-only)

**Dependencies:** None (this is the foundation)

**Modules/files affected:** `core/src/hprof/parser.rs`, `core/src/hprof/binary_parser.rs`, `core/src/hprof/object_graph.rs`, `core/src/graph/dominator.rs`, `core/src/graph/metrics.rs`, `core/src/analysis/engine.rs`, `core/src/graph/gc_path.rs`, `core/src/hprof/test_fixtures.rs`, `.github/workflows/ci.yml`

**Complexity:** Very High — this was the hardest milestone with the most new code.

**Definition of done (REVISED — original criteria were met on synthetic data only):**
- ⚠️ Can parse a real HPROF dump into a full object graph with reference edges — **FAILS on real dumps due to tag bug**
- ⚠️ Can compute retained sizes for any object — **algorithm correct but no real objects enter the graph**
- ⚠️ Can produce a real dominator tree — **algorithm correct but produces tag-level-only nodes on real dumps**
- ⚠️ Leak detection uses retained-size data — **falls back to heuristics and returns zero candidates on real dumps**
- ⚠️ GC path uses full object graph — **falls back to synthetic paths on real dumps**
- ✅ 87 tests pass (59 core + 5 CLI unit + 23 CLI integration) — all synthetic-only
- ✅ CI runs on every PR

**Blocking issue for M1 closure:** Tag-constant fix + real-world HPROF validation (see M1.5)

---

### Milestone 2 — Packaging, Releases, and DX
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
**Objective:** Fix the critical tag-constant bug, validate the graph-backed pipeline end-to-end against real-world HPROF files, and ensure the M1 foundation actually works on production data.

**Why it matters:** M1 delivered the architecture, data structures, and algorithms — but only validated them against synthetic HPROF test fixtures that use `HEAP_DUMP` (0x0C) records. Real-world JVM dumps use `HEAP_DUMP_SEGMENT` (0x1C), which the parser currently skips. Without this fix, ALL downstream analysis features (M3, M4, M5) are built on a foundation that does not work on real data. This is the highest-priority work in the project.

**Status:** 🔴 NOT STARTED — P0 PRIORITY

**Key Deliverables:**
1. **P0: Fix HPROF tag constants** — Correct `TAG_HEAP_DUMP_SEGMENT` from `0x0D` to `0x1C` in `binary_parser.rs`. Fix the `tag_name()` function in `parser.rs` to correctly label all tags per the HPROF spec (0x0D=CPU_SAMPLES, 0x0E=CONTROL_SETTINGS, 0x1C=HEAP_DUMP_SEGMENT, 0x2C=HEAP_DUMP_END). Fix `HEAP_DUMP_SEGMENT_TAG` in `gc_path.rs`.
2. **P0: Add HEAP_DUMP_SEGMENT parsing** — Ensure `binary_parser.rs` processes tag `0x1C` records through the same sub-record parsing path as `HEAP_DUMP` (0x0C). Verify both tags work.
3. **P0: Real-world HPROF test fixture** — Create or source a small (~5-10MB) real JVM heap dump that uses `HEAP_DUMP_SEGMENT` records. Add integration tests that validate: object graph population, dominator tree construction with real objects, non-zero leak candidates, non-synthetic GC paths.
4. **P1: Validate graph-backed pipeline end-to-end** — Run the full `analyze_heap()` → `detect_leaks()` → `gc_path` pipeline against real dumps and verify: graph nodes >> 7, retained sizes are meaningful, leak candidates are non-empty, GC paths use ObjectGraph BFS (not fallback).
5. **P1: Investigate heuristic fallback on real data** — If the heuristic threshold / filters are also broken for real data (zero candidates even in fallback mode), fix the heuristic path too.
6. **P1: Leak-ID validation** — `explain` and `fix` commands should validate provided leak-ids against actual heap data and return clear errors for unknown IDs instead of silently returning generic responses.
7. **P2: Add HEAP_DUMP_SEGMENT unit tests** — Add parser unit tests specifically for tag 0x1C processing to prevent regression.

**Dependencies:** None — this unblocks everything else.

**Modules/files affected:** `core/src/hprof/parser.rs`, `core/src/hprof/binary_parser.rs`, `core/src/graph/gc_path.rs`, `core/src/analysis/engine.rs`, `cli/tests/integration.rs`, `core/src/hprof/test_fixtures.rs`

**Complexity:** Medium — the fix itself is likely small (correct constants + add 0x1C to the tag match), but validation and testing against real-world data is the bulk of the work.

**Definition of done:**
- `mnemosyne parse` correctly labels tag 0x1C as HEAP_DUMP_SEGMENT and 0x0D as CPU_SAMPLES
- `binary_parser::parse_hprof_file()` produces a non-empty `ObjectGraph` from a real JVM heap dump
- `analyze_heap()` on a real dump shows object-level dominators (not record-tag-level)
- `detect_leaks()` on a real dump returns ≥1 leak candidate
- `gc-path` on a real dump returns a non-synthetic path at least some of the time
- `explain` and `fix` with an invalid leak-id return an error, not a generic response
- All existing 87 tests continue to pass
- At least 5 new tests exercise HEAP_DUMP_SEGMENT parsing and real-world validation
- CI runs clean

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

**Dependencies:** M1 (object graph, retained sizes, dominator tree) — ⚠️ architecture delivered, **M1.5 (real-world hardening) MUST be complete first**

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
| 1 | Object graph parser | P0 | High | XL | None | M1 | ✅ Done (synthetic) |
| 2 | Dominator tree algorithm | P0 | High | L | Object graph | M1 | ✅ Done (synthetic) |
| 3 | Retained size computation | P0 | High | M | Dominator tree | M1 | ✅ Done (synthetic) |
| 4 | Sample HPROF test fixtures | P0 | High | M | None | M1 | ✅ Done |
| 5 | CI pipeline (GitHub Actions) | P0 | High | M | None | M1 | ✅ Done |
| 6 | Unify `detect_leaks()` onto graph path | P0 | High | L | Object graph + retained sizes | M1 | ✅ Done (synthetic) |
| 7 | Rewrite GC path over full object graph | P0 | High | M | Object graph | M1 | ✅ Done (synthetic) |
| 8 | Object graph navigation API | P0 | High | M | Object graph | M1 | ✅ Done |
| 9 | Integration tests via reusable synthetic HPROF fixtures | P0 | High | L | Test fixtures + CI | M1 | ✅ Done |
| **9a** | **Fix HPROF tag constants (0x0D/0x1C swap)** | **P0** | **Critical** | **S** | **None** | **M1.5** | **🔴 Blocked** |
| **9b** | **Add HEAP_DUMP_SEGMENT (0x1C) parsing support** | **P0** | **Critical** | **M** | **Tag fix (9a)** | **M1.5** | **🔴 Blocked** |
| **9c** | **Real-world HPROF test fixture + validation tests** (all 87 tests are synthetic-only — this gap directly allowed the tag bug to ship) | **P0** | **Critical** | **M** | **Tag fix (9a)** | **M1.5** | **🔴 Blocked** |
| **9d** | **End-to-end pipeline validation on real dumps** | **P0** | **High** | **M** | **9a + 9b + 9c** | **M1.5** | **🔴 Blocked** |
| **9e** | **Investigate heuristic fallback zero-results on real data** | **P1** | **High** | **M** | **Tag fix (9a)** | **M1.5** | **🔴 Blocked** |
| **9f** | **Leak-ID validation for explain/fix commands** | **P1** | **Medium** | **S** | **None** | **M1.5** | **🔴 Blocked** |
| 10 | Release binaries | P1 | High | M | CI pipeline (✅) | M2 | ✅ Done |
| 11 | cargo install support | P1 | High | S | Release setup | M2 | ✅ Done |
| 12 | CLI progress bars + colors | P1 | Medium | S | None | M2 | ✅ Done |
| 12a | Table-formatted CLI output | P1 | Medium | S | CLI UX (✅) | M2 | ✅ Done |
| 13 | MAT-style leak suspects | P1 | High | L | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 14 | Histogram by class/package/classloader | P1 | High | M | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 15 | Homebrew formula | P1 | Medium | S | Release binaries | M2 | ✅ Done |
| 16 | LLM integration (real API calls) | P1 | High | L | M1.5 + meaningful real data | M5 | ⚬ Pending |
| 17 | Enhanced heap diff | P1 | Medium | M | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 18 | Static interactive HTML reports | P2 | High | L | Reporting exists | M4 | ⚬ Pending |
| 19 | OQL query engine | P2 | High | XL | **M1.5 + M3** | M3 | ⚬ Pending |
| 20 | Thread inspection | P2 | Medium | L | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 21 | ClassLoader analysis | P2 | Medium | L | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 22 | Local web UI | P2 | High | XL | HTML reports | M4 | ⚬ Pending |
| 23 | Collection inspection | P2 | Medium | M | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 24 | Unreachable objects | P2 | Medium | M | **M1.5 real-world validation (🔴)** | M3 | ⚬ Pending |
| 25 | Configurable prompt/task runner | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 26 | AI conversation mode | P2 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 27 | Docker image | P2 | Medium | S | Release automation | M2 | ✅ Done |
| 28 | Example projects + sample dumps | P2 | Medium | M | Test fixtures (✅) | M6 | ⚬ Pending |
| 29 | Benchmark suite (`criterion`) — no benchmarks exist today | P2 | Medium | M | **M1.5 real-world validation (🔴)** | M3/M6 | ⚬ Pending |
| 30 | Plugin/extension system | P3 | Medium | XL | Stable APIs (M3+) | M6 | ⚬ Pending |
| 31 | Full interactive heap browser | P3 | High | XL | Web UI + OQL | M4 | ⚬ Pending |
| 32 | Local LLM support | P3 | Medium | L | LLM integration | M5 | ⚬ Pending |
| 33 | Real MCP API documentation for `docs/api.md` (JSON-RPC signatures, schemas, examples) | P2 | Medium | M | MCP server (✅) | M1.5/M3 | ⚬ Pending |
| 34 | Real usage examples in `docs/examples/` (CLI workflows, analysis sessions, MCP integration) | P2 | Medium | M | M1.5 real-world validation | M3/M6 | ⚬ Pending |
| 35 | README badge version qualifier (`v0.1.1-alpha`) | P3 | Low | S | None | M3 | ⚬ Pending |
| 36 | Dockerfile base image CVE triage (`debian:bookworm-slim` known vulnerabilities) | P2 | Medium | S | None | M1.5/M3 | ⚬ Pending — route to Security Agent |

---

## Section 11 — Recommended Immediate Next Steps

**⚠️ CRITICAL: M1.5 Real-World Hardening must complete before any M3 work begins.**

Milestone 2 is complete. Milestone 1 was believed complete but real-world validation against Kotlin+Spring Boot heap dumps has revealed that the graph-backed pipeline does NOT activate on production HPROF files. The root cause is a HPROF tag-constant bug: `HEAP_DUMP_SEGMENT` (0x1C) is mislabeled as `CPU_SAMPLES` in both parsers, and the binary parser looks for heap data at `0x0D` (actually `CPU_SAMPLES`) instead of `0x1C`. This means all M1 features (dominator tree, retained sizes, graph-backed leak detection, GC paths) only work on synthetic test fixtures, not real JVM dumps.

### Step 1 (P0): Fix HPROF tag constants
**Why first:** This is a one-line-per-file fix that unblocks everything else. The tag swap in `binary_parser.rs`, `parser.rs`, and `gc_path.rs` is the root cause of all real-world failures.
**Files:** `core/src/hprof/binary_parser.rs`, `core/src/hprof/parser.rs`, `core/src/graph/gc_path.rs`
**Owner:** Implementation Agent
**Effort:** Small
**Dependencies:** None

### Step 2 (P0): Add HEAP_DUMP_SEGMENT (0x1C) to binary parser dispatch
**Why next:** After fixing the constant, the binary parser's top-level record-tag match must dispatch `0x1C` records to the same sub-record parser used for `HEAP_DUMP` (0x0C). Without this, the constant fix alone may not be sufficient.
**Files:** `core/src/hprof/binary_parser.rs`
**Owner:** Implementation Agent
**Effort:** Small–Medium
**Dependencies:** Step 1

### Step 3 (P0): Real-world HPROF test fixture + validation
**Why next:** Source or generate a small real JVM heap dump (~5-10MB) that uses `HEAP_DUMP_SEGMENT`. Add integration tests that verify: non-empty object graph, meaningful dominator tree, non-zero leak candidates, and non-synthetic GC paths. This prevents the tag bug class from recurring.
**Files:** `resources/test-fixtures/`, `core/src/hprof/test_fixtures.rs`, `cli/tests/integration.rs`
**Owner:** Implementation Agent + Testing Agent
**Effort:** Medium
**Dependencies:** Steps 1–2

### Step 4 (P1): Investigate heuristic fallback zero-results
**Why next:** Even in heuristic fallback mode, `leaks` returned zero candidates on ~314K-record dumps. The fallback thresholds or filters may need tuning for real-world data. After the tag fix enables graph-backed detection, verify that the fallback path also produces reasonable candidates when the graph path is artificially disabled.
**Files:** `core/src/analysis/engine.rs`
**Owner:** Implementation Agent
**Effort:** Medium
**Dependencies:** Step 1 (to understand whether the issue was just the empty graph or also the heuristics)

### Step 5 (P1): Leak-ID validation for explain/fix
**Why next:** `explain` and `fix` should return errors for unknown/invalid leak IDs instead of silently returning generic responses. This is a trust and usability issue.
**Files:** `core/src/analysis/engine.rs`, `core/src/fix/generator.rs`
**Owner:** Implementation Agent
**Effort:** Small
**Dependencies:** None (can be parallelized with Steps 1–4)

### Step 6: Resume M3 work (only after M1.5 is validated)
**Why after:** M3 features (MAT-style suspects, histogram grouping, enhanced diff) all depend on a working object graph populated with real data. Starting M3 before M1.5 is validated would repeat the same synthetic-only validation gap.
**First M3 item:** MAT-style leak suspects algorithm
**Owner:** Implementation Agent
**Effort:** Large
**Dependencies:** M1.5 complete and validated

### Pre-M3 Debt Items (tracked)
The following items were identified during the real-world validation pass and should be addressed before or alongside M3 work:
- **`docs/api.md` placeholder scaffolding** — needs real MCP API documentation (backlog #33)
- **`docs/examples/` placeholder** — needs real CLI/MCP usage examples (backlog #34)
- **README badge version qualifier** — low priority cosmetic item (backlog #35)
- **Real-world HPROF test fixtures** — already tracked as M1.5 deliverable #3 / backlog #9c; all 87 tests are synthetic-only
- **No `criterion` benchmark suite** — tracked in backlog #29; milestone updated to M3/M6
- **Dockerfile base image CVEs** — `debian:bookworm-slim` has known vulnerabilities; routed to Security Agent for triage (backlog #36, risk register R-NEW-1)

---

## Section 12 — Risk Register & Lessons Learned

### Active Risks

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **Tag-constant bug may have deeper parser issues** | High | Medium | After fixing 0x0D/0x1C swap, run comprehensive validation against multiple real-world dumps from different JVM versions (OpenJDK, GraalVM, Azul) and frameworks (Spring Boot, Quarkus, plain Java) |
| **Dockerfile base image (`debian:bookworm-slim`) has known CVEs** | Medium | High | Base image carries known vulnerabilities per container scanning. Not blocking M3 but should be triaged by Security Agent. Options: update to latest bookworm-slim, switch to distroless, or pin a patched digest. Route to Security Agent for assessment and remediation recommendation. |
| **Real-world object graphs may exceed memory on moderate machines** | High | Medium | After tag fix, measure actual memory usage when parsing ~150MB dumps into ObjectGraph. If >4GB RSS, evaluate memory-mapped or chunked strategies before M3 |
| **Other HPROF sub-record types may have parsing bugs** | Medium | Medium | Real-world dumps may contain sub-record types or field layouts not covered by synthetic fixtures. Add broader sub-record coverage in M1.5 validation |
| **Heuristic fallback may need separate tuning** | Medium | High | Even after tag fix, the heuristic path needs its own validation pass — the thresholds were developed without real-world data |
| **AI integration is entirely unvalidated** | Medium | Low (deferred) | No urgency until M1.5 and M3 provide meaningful analysis data to send to an LLM |

### Lessons Learned (v0.1.1 Real-World Validation)

1. **Synthetic-only test coverage creates false confidence.** All 87 tests passed, clippy was clean, CI was green — but the tool produced incorrect output on every real-world dump. Lesson: real-world HPROF test fixtures are mandatory, not nice-to-have.
2. **Tag constant errors are insidious.** The HPROF spec uses sequential hex values (0x0C, 0x0D, 0x0E) for unrelated record types and then jumps to 0x1C/0x2C for segment/end. This is a spec design that invites off-by-one style errors. Multiple independent sources map these tags differently. Lesson: verify tag constants against the authoritative JDK source (`hprof_b_spec.h`), not third-party reference docs.
3. **Silent fallback can mask critical bugs.** The provenance system correctly labeled outputs as `[PARTIAL]` and `[FALLBACK]`, but the user experience was "analyze works, just with limited data" rather than "the parser is completely failing on your dump." Lesson: consider adding a warning when the graph-backed path fails entirely and ALL results are fallback.
4. **The features that work well are genuinely good.** `parse`, `diff`, `config`, reporting formats, error handling, and the provenance system all performed correctly on real-world data. The issue is specifically in the HPROF binary parser’s tag dispatch, not in the overall architecture or downstream pipeline.
5. **Cross-platform builds work.** Windows binary confirmed functional on real Kotlin+Spring Boot dumps. This is a meaningful achievement for a Rust CLI tool targeting JVM developers.

---

*This roadmap is a living document. Update it after each major batch completion.*
*Next review: after M1.5 tag fix lands and real-world validation is complete.*
