# Mnemosyne Roadmap & Milestones

> **Last updated:** 2026-04-13 (roadmap/design alignment for shipped M3 and approved-scope M5 state)
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

Mnemosyne is an **alpha-stage Rust-based JVM heap analysis tool** with a validated analytical foundation. It stream-parses HPROF files to produce class histograms and heap summaries, parses binary HPROF records into a full object reference graph (`core::hprof::binary_parser` → `core::hprof::object_graph`), computes a real dominator tree via Lengauer-Tarjan (`core::graph::dominator`), derives retained sizes from post-order subtree accumulation, runs graph-backed analysis in both `analyze_heap()` and `detect_leaks()` with automatic fallback to heuristics when parsing fails, traces GC root paths with `ObjectGraph` BFS first plus layered fallbacks, exposes object navigation via `get_object(id)`, `get_references(id)`, and `get_referrers(id)`, generates heuristic fix suggestions, and renders results in five output formats (Text, Markdown, HTML, TOON, JSON) — all backed by a provenance system that labels every synthetic, partial, fallback, or placeholder data surface. A stdio MCP server now exposes fourteen live methods (`list_tools`, `parse_heap`, `detect_leaks`, `analyze_heap`, `query_heap`, `map_to_code`, `find_gc_path`, `create_ai_session`, `resume_ai_session`, `get_ai_session`, `close_ai_session`, `chat_session`, `explain_leak`, `propose_fix`) and preserves the legacy string `error` field while attaching machine-readable `error_details` on failures. The AI module is no longer fully stubbed: `analysis::generate_ai_insights` now supports `rules`, `stub`, and `provider` modes, with provider-backed execution verified for OpenAI-compatible, local, and Anthropic endpoints in this branch.

Mnemosyne has the foundations to become **the first Rust-native, AI-assisted heap analysis platform** that rivals Eclipse MAT in analysis depth while offering capabilities no existing tool provides: provenance-tracked outputs that distinguish real analysis from heuristic guesses, MCP-native IDE integration for copilot-style workflows, CI/CD-friendly automation via structured JSON and TOON output, and an AI-native architecture designed from day one for LLM integration. The Rust core means multi-GB heap dumps can be processed with predictable memory usage and no GC pauses — a meaningful advantage over Java-based tools like MAT and VisualVM for production incident response.

Five properties position Mnemosyne to stand out in a crowded JVM tooling ecosystem: **(1)** Rust performance enabling streaming analysis of heap dumps that exceed host RAM; **(2)** a provenance system unique among heap analyzers, giving users and automation confidence in result trustworthiness; **(3)** MCP-first architecture that makes heap analysis a conversation in the developer's IDE rather than a separate tool; **(4)** AI-native design with well-shaped type contracts (`AiInsights`, `AiWireExchange`, config plumbing) ready for LLM wiring; and **(5)** automation-friendly structured output (JSON, TOON) enabling CI regression detection with machine-readable leak signals.

**v0.2.0 update (2026-03-08, refreshed 2026-04-12): v0.2.0 is deployed to all channels and real-world validated.** v0.2.0 is published on GitHub Releases (5 binary targets), GHCR Docker (`ghcr.io/bballer03/mnemosyne:0.2.0`), crates.io (`mnemosyne-core` 0.2.0 + `mnemosyne-cli` 0.2.0), and Homebrew (formula updated with correct SHA256 digests). A comprehensive real-world validation against a 150MB Spring Boot production heap dump plus M3 follow-through confirmed: the graph-backed pipeline, investigation features, classloader/query/profile features, completed Step 11 dense multi-tier validation, and a new configurable rule-based AI task runner. The benchmark baseline plus dense Step 11 validation now show streaming parser efficiency plus stable large-tier default-path RSS around 2.87x-2.90x and investigation-path RSS around 3.89x-3.92x through the ~2 GB tier.

Honest assessment: **the analytical foundation is strong, real-world-validated, and shipping to users — but significant feature work remains** to deliver on the full vision. The core pipeline — object graph, dominator tree, retained sizes, histogram grouping, MAT-style suspects, unreachable objects, class-level diff, thread inspection, string analysis, collection inspection, top-instance ranking, unified leak detection, GC paths, provenance system, ClassLoader analysis, the shipped OQL/query slice, analyze profiles, optional `hyperfine` / `heaptrack` wrapper automation, a configurable AI task runner, provider-backed AI execution, persisted MCP AI sessions, and evidence-first MCP transport hardening — all work correctly on production data. The distribution story is excellent: v0.2.0 is live across five channels. Benchmark data is now published, Step 11 large-dump validation is complete, and the final approved-scope M3 closeout batch is landed and verified. **The immediate priorities are: (1) executing M4 as the next full open milestone, (2) treating any further OQL/query or scale work as evidence-driven follow-on rather than M3 blockers, and (3) treating any further M5 work as narrower post-milestone follow-on rather than a broadly pending milestone.**

---

## Section 2 — Current State Assessment

### Core Capabilities

| Capability | Status | Honest Assessment |
|---|---|---|
| HPROF streaming parser | ✅ Validated | Two-tier parsing: `core::hprof::parser` streams headers + record tags at 2.25 GiB/s for fast class histograms. `core::hprof::binary_parser` parses binary HPROF records into `ObjectGraph` at 90 MiB/s. Tag constants corrected in M1.5 — both `HEAP_DUMP` (0x0C) and `HEAP_DUMP_SEGMENT` (0x1C) records are now parsed correctly. Validated on real-world Kotlin+Spring Boot dumps (150MB). First benchmark baseline published. |
| Leak detection | ✅ Graph-backed + heuristic fallback | `detect_leaks()` attempts the graph-backed path first (ObjectGraph → dominator → retained sizes), then falls back to heuristics with provenance markers. MAT-style suspect ranking with retained/shallow ratio, accumulation-point detection, dominated-count context, and composite scoring delivered in M3-P1-B2. Both paths validated on real-world data. Leak-ID validation enforced. |
| Graph / dominator tree | ✅ Real-world validated + benchmarked | `core::graph::dominator::build_dominator_tree()` runs Lengauer–Tarjan over the full object reference graph with virtual super-root. Validated on both synthetic fixtures and real-world JVM dumps. Produces meaningful retained sizes. Dominator tree builds in 1.85s on the 156 MB dump. Step 11 is complete and now adds dense synthetic validation at roughly 500 MB / 1 GB / 2 GB with the lean default path at 2.87x-2.90x RSS:dump and the opt-in investigation path at 3.89x-3.92x. |
| GC root path tracing | ✅ Real-world validated | `core::graph::gc_path` tries `ObjectGraph` BFS first, then budget-limited `GcGraph`, then synthetic paths. Primary BFS path activates on real-world dumps. Provenance labels honestly indicate data quality. |
| Histogram grouping | ✅ Delivered | Graph-backed grouping by class, package, classloader with instance counts, shallow sizes, and retained sizes. CLI `analyze --group-by` renders grouped output. |
| Unreachable objects | ✅ Delivered | BFS/DFS from GC roots identifies unreachable objects with total count/shallow size and per-class breakdown. |
| Enhanced heap diff | ✅ Delivered | Record-level diff preserved; class-level deltas (instance, shallow-byte, retained-byte) added when both snapshots build object graphs. |
| AI / LLM insights | ✅ Delivered | `core::analysis::generate_ai_insights()` now supports `rules`, `stub`, and `provider` modes with ordered task definitions from `AiConfig`, preserving CLI/MCP/report contracts. OpenAI-compatible, local, and Anthropic provider execution now have targeted verification coverage. Step `14(d)` covers prompt redaction, hashed audit logging, and a minimal prompt-budget guard. Step `14(e)` now includes a CLI-first chat slice with bounded in-process history, persisted MCP AI sessions for resumed `chat_session` / `explain_leak` / `propose_fix` follow-up, an AI-backed one-file / one-snippet fix-generation slice, and black-box `serve` coverage for delayed responses, larger payloads, and session-backed follow-up. |
| Fix suggestions | ⚠️ Partial | `core::fix::propose_fix()` preserves the legacy one-argument entrypoint, while `core::fix::propose_fix_with_config()` attempts provider-backed fix generation when `AiMode::Provider` is active and `project_root` yields one mapped source file plus a small local snippet. The response contract stays stable, successful AI-backed fixes return the same `FixSuggestion` shape, and Mnemosyne falls back to heuristic placeholder patches with explicit provenance when provider-backed generation is unavailable or fails validation. |
| Source mapping | ✅ Implemented | `core::mapper::map_to_code()` scans project dirs for `.java`/`.kt` files, runs `git blame` for metadata. Basic but functional for local projects. |
| Reporting | ✅ Implemented | `core::report` renders 5 formats (Text, Markdown, HTML, TOON, JSON). HTML output uses `escape_html()` for XSS prevention. TOON uses `escape_toon_value()` for control characters. Provenance markers rendered in all non-JSON formats. One of the most polished subsystems. |
| MCP server | ✅ Wired | `core::mcp::serve()` runs a stdio line-delimited JSON RPC-like loop over stdio with async Tokio I/O. It now handles 14 live methods, including explicit AI session lifecycle operations plus `list_tools` for discovery, and attaches machine-readable `error_details` while preserving the legacy `error` string. Analysis quality is backed by real graph-based results on real dumps. AI insights and `propose_fix` both flow through the shared AI pipeline, with fix generation falling back to heuristic guidance plus explicit provenance when provider-backed generation is unavailable. |
| Config system | ✅ Implemented | `cli::config_loader` reads TOML files from 5 locations + env vars + CLI flags. `core::config` defines `AppConfig`, `AiConfig`, `ParserConfig`, `AnalysisConfig`. Clean, well-layered. |
| Provenance system | ✅ Implemented | `ProvenanceKind` (Synthetic, Partial, Fallback, Placeholder) + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in all report formats and CLI output. Unique feature in the heap-analysis space. |
| Distribution | ✅ Full deployment | v0.2.0 live on GitHub Releases (5 targets), GHCR Docker, crates.io (both crates), Homebrew. Docker build validation in CI. |
| Benchmarking | ✅ Baseline published | Criterion benchmarks for parser, graph, dominator. RSS measurements. Scaling projections. Published in `docs/performance/memory-scaling.md`. |

### Technical Strengths

- **Rust performance model**: streaming parser at 2.25 GiB/s with `BufReader`, no GC, predictable memory. The 156 MB real-world fixture remains a useful regression sentinel at 4.23x on the lean default path, while completed Step 11 dense synthetic validation shows the default path stabilizing around 2.87x-2.90x and the investigation path around 3.89x-3.92x through the ~2 GB tier.
- **Published benchmark baseline**: Criterion benchmarks + RSS measurements against the 156 MB real-world fixture plus completed Step 11 dense synthetic validation at roughly 500 MB / 1 GB / 2 GB tiers.
- **Clean module separation**: grouped implementation domains in `core/src/` (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`) plus shared `config.rs`, `errors.rs`, and `lib.rs` re-exports.
- **Real object graph (real-world validated)**: `core::hprof::binary_parser` parses binary HPROF records into an `ObjectGraph` with objects, reference edges, class metadata, and GC roots. Validated on real-world Kotlin+Spring Boot dumps (150MB).
- **Real dominator tree (benchmarked)**: `core::graph::dominator` implements Lengauer–Tarjan over the full object graph with virtual super-root. Computes retained sizes via post-order accumulation. 1.85s on 156 MB fixture.
- **Graph-backed analysis pipeline (production-validated)**: `analyze_heap()` and `detect_leaks()` both attempt object-graph → dominator-tree → retained-size analysis first, with automatic fallback to heuristics and provenance markers. Pipeline activates and produces meaningful results on real-world production dumps.
- **MAT-style suspect ranking**: Retained/shallow ratio, accumulation-point detection, dominated-count context, short reference chains, and composite scoring.
- **Histogram grouping**: Graph-backed grouping by class, package prefix, and classloader with instance, shallow-size, and retained-size totals.
- **Unreachable object analysis**: GC-root reachability traversal with per-class unreachable count and shallow-size breakdown.
- **Class-level heap diff**: Instance, shallow-byte, and retained-byte deltas between snapshots when both build object graphs.
- **Streaming design**: `core::hprof::parser` processes HPROF records sequentially without loading the full dump. Foundation for scaling to multi-GB files.
- **Provenance system**: genuinely novel for a heap analyzer. Labels every synthetic/heuristic output surface so consumers know what to trust.
- **Multi-format output**: 5 report formats with consistent provenance rendering. HTML is XSS-hardened. TOON enables compact CI consumption.
- **Workspace test suite with CI**: the repo carries broad core + CLI coverage, including real-world HPROF validation tests. Synthetic and segment HPROF test fixtures plus the `test-fixtures` cargo feature enable deterministic parser, graph, end-to-end CLI testing, and targeted error-path coverage.
- **Config hierarchy**: TOML + env vars + CLI flags with clear precedence. Production-ready design pattern.
- **MCP integration**: stdio server with 14 live methods, self-described tool metadata via `list_tools`, persisted AI session lifecycle support, and shared core contracts. First-mover for heap analysis in the MCP ecosystem.
- **Type contracts**: well-shaped request/response types (`AnalyzeRequest`, `AnalyzeResponse`, `GcPathResult`, `FixResponse`, etc.) that establish stable contracts between CLI, MCP, and core.
- **Full distribution**: v0.2.0 published on crates.io, GitHub Releases (5 targets), Homebrew, Docker (GHCR). Docker build validation in CI. All channels functional and validated.

### Major Weaknesses

- **Post-M5 AI follow-on is now narrower than the shipped milestone scope**: provider-backed execution is verified for OpenAI-compatible, local, and Anthropic endpoints; prompt templates, provider-mode prompt redaction, provider-mode hashed audit logging, a minimal `max_tokens`-driven prompt-budget guard, MCP `error_details` / `list_tools` discovery, a CLI-first chat slice, and a one-file / one-snippet AI-backed fix-generation slice are in place. The remaining gap is broader conversation/exploration semantics, native local-provider transports beyond OpenAI-compatible endpoints, and any further transport work only if later evidence justifies it.
- **Real-world large-dump validation is still lighter than synthetic large-tier coverage**: Step 11 cleared the roadmap gate with dense synthetic ~500 MB / ~1 GB / ~2 GB tiers, but additional real-world large-heap fixtures would further de-risk the architecture.
- **MAT parity gap is now narrower**: M3 shipped ClassLoader analysis, the approved query slice now includes retained instance-field projection/filtering plus hierarchy-aware `INSTANCEOF`, and analyze profiles. The remaining explorer gap is depth and breadth: richer OQL predicates, deeper query ergonomics, and more interactive browsing surfaces.
- **`fix` produces output when `analyze` reports 0 leaks**: the fix surface operates on dominator analysis independently of the leak filter, which can confuse users. Fix output is correctly labeled `[SYNTHETIC] [PLACEHOLDER]` but the workflow disconnect is a UX issue.
- **Diff is class-level, not object-level**: `diff_heaps()` now shows class-level deltas (instance, shallow, retained) but cannot track individual object migration or reference chain changes between snapshots.
- **Graph module naming is misleading**: `summarize_graph()` still exists as a lightweight fallback that builds a synthetic tree from top-12 entries. Its name suggests more than it delivers, though the real dominator tree now exists alongside it.
- **No sample real-world data for tutorials**: real-world validation exists in CI (optional fixture), but no example `.hprof` files for documentation or onboarding.
- **Documentation drift requires ongoing discipline**: docs have been brought back in line with the live CLI/MCP/API surface, but this repo changes quickly and the sync work must continue alongside feature work.
- **`docs/examples/` remains intentionally lightweight**: it now points users to live examples, but it is not yet a full cookbook.

### Maturity Assessment

| Subsystem | Maturity | Rationale |
|---|---|---|
| Parser | Beta- | Both streaming (2.25 GiB/s) and binary (90 MiB/s) parsers validated on real-world HPROF files. First benchmark baseline published, and dense synthetic validation now extends through roughly the 2 GB tier. Broader very-large real-world heap coverage remains future follow-on. |
| Leak detection | Beta- | Graph-backed + heuristic fallback both validated on real data. MAT-style suspect ranking with composite scoring. Leak-ID validation enforced. Zero-result runs now report `No leak suspects detected.` explicitly. |
| Graph / Dominator | Beta- | Lengauer–Tarjan validated on real-world object graphs. Retained sizes correct. Benchmarked at 1.85s on 156 MB. Histogram grouping and unreachable-object analysis delivered. Main remaining gap: browsable dominator/explorer view. |
| AI | Alpha+ | Configurable rule-based task runner is real and exercised by CLI/MCP/report flows. Provider-backed execution is verified for OpenAI-compatible, local, and Anthropic endpoints, persisted MCP AI sessions are shipped, and the current stdio request/response transport has direct black-box coverage for delayed AI-backed responses and larger single-response payloads. |
| GC root paths | Alpha+ | `ObjectGraph` BFS activates on real dumps. Triple fallback with honest provenance. |
| Fix suggestions | Alpha | Heuristic fallback remains the safe baseline, but provider-backed one-file / one-snippet fix generation is now shipped when source context is available. The response contract stays stable and falls back with explicit provenance when AI-backed generation cannot run. |
| Source mapping | Alpha | Works for basic cases. No IDE integration beyond file scanning. |
| Reporting | Beta | 5 formats, XSS hardening, provenance rendering, well-tested. Ready for use. |
| MCP server | Beta- | Wired, functional, and backed by real graph-based analysis on real dumps. `list_tools` now exposes tool descriptions and parameter metadata, failures include machine-readable `error_details`, provider-backed AI calls share the outbound prompt-redaction layer, persisted heap-bound AI sessions now support resumed `chat_session` / `explain_leak` / `propose_fix` follow-up, and direct `serve` coverage verifies delayed AI-backed responses plus larger single-response payloads. Streaming remains conditional rather than part of the current contract. |
| Config | Beta | Clean hierarchy, env + TOML + CLI. Production-ready pattern. |
| Provenance | Beta | Unique, well-integrated across all surfaces. Novel in the space. |
| Testing | Beta- | Current verified worktree runs passed `cargo check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check`. `cargo test` currently reports 228 passing Rust tests (9 CLI unit + 69 CLI integration + 139 core unit + 3 classloader + 5 query_executor + 3 query_parser; doctests 0). Criterion benchmarks are published, and focused wrapper tests also cover the optional `hyperfine` / `heaptrack` scripts. No property-based testing or coverage tracking. |
| CI/CD | Beta | GitHub Actions CI runs check + test + clippy + fmt. Release workflow cross-compiles 5 targets + Docker + crates.io. Docker build validation added. Nightly builds still absent. |
| Distribution | Beta | v0.2.0 live on GitHub Releases, crates.io, GHCR Docker, Homebrew. All channels validated. |
| Benchmarking | Alpha+ | Baseline published with Criterion data + RSS measurements, plus completed Step 11 dense synthetic validation through roughly the 2 GB tier. Still lacks comparative benchmarks vs MAT/hprof-slurp and more real-world large-dump data. |

---

## Section 3 — Gap Analysis

### 3.1 Correctness & Trust Gaps

**✅ RESOLVED (M1.5): HPROF tag constant mislabeling.** Both parsers now use the correct tag constants: `HEAP_DUMP_SEGMENT = 0x1C`, `CPU_SAMPLES = 0x0D`, `HEAP_DUMP_END = 0x2C`, `CONTROL_SETTINGS = 0x0E`. The streaming parser's `tag_name()` function also returns correct labels. Validated on real-world Kotlin+Spring Boot dumps (~110MB, ~150MB).

**✅ RESOLVED (M1.5): Object reference graph validated on real-world data.** The full pipeline — `binary_parser` → `ObjectGraph` → `dominator` → retained sizes → leak detection — now activates on real JVM dumps. The binary parser correctly parses `HEAP_DUMP_SEGMENT` (0x1C) records, producing a populated object graph. Dominator tree computes meaningful retained sizes from real object data. Graph-backed analysis produces real results on production Kotlin+Spring Boot dumps.

**✅ RESOLVED (M1.5): Leak detection produces results on real-world data.** The graph-backed path activates and finds leak candidates on real dumps. Heuristic fallback also works with provenance markers. Leak-ID validation now enforced — unknown IDs return errors.

**✅ RESOLVED (M1.5): explain/fix commands validate leak IDs.** Unknown leak-IDs now return explicit errors instead of generic responses. Fix command no longer generates hardcoded patches for fabricated IDs.

- **Diff is record-level, not object-level.** `diff_heaps()` compares aggregate record/class statistics between two snapshots. It cannot track individual object migration, new allocation sites, or reference chain changes. (Note: the diff command itself works well and is one of the most useful features — the "delta" summary is accurate at the record level.) Object-level diff remains future follow-on work rather than part of the approved M3 scope.

**Provenance correctly labels data quality** — the system labels graph-backed results with no provenance marker (clean data) and heuristic/fallback results with `ProvenanceKind::Fallback` or `ProvenanceKind::Partial`, so consumers know what to trust. The provenance system worked as designed during real-world testing: `[PARTIAL]` labels were honestly displayed.

### 3.2 Testing & CI Gaps

- **228 passing Rust tests** are currently reported by `cargo test` in this worktree. Coverage includes provenance rendering, escape functions, analysis paths, HPROF parsing, object graph construction, dominator tree correctness, retained-size computation, histogram grouping, suspect scoring, unreachable-object analysis, enhanced diffing, thread inspection, string analysis, collection inspection, top-instance reporting, CLI argument handling, end-to-end command execution, targeted failure-path UX, and real-world HPROF validation.
- **Synthetic HPROF test fixtures** exist in `core::test_fixtures`. Small deterministic binary HPROF files exercise the parser and graph pipeline without requiring a JVM or committing large binaries. Includes `build_simple_fixture()` (0x0C), `build_graph_fixture()` (0x0C), and `build_segment_fixture()` (0x1C).
- **`test-fixtures` cargo feature** exposes canonical fixture builders to integration tests without widening the builder API surface.
- **Real-world HPROF validation** added in M1.5: 4 tests validate against real Kotlin+Spring Boot production dumps. Binary parser, object graph population, dominator tree construction, and retained-size computation are all tested on real data (gated behind optional fixture path).
- **CI pipeline running.** GitHub Actions (`.github/workflows/ci.yml`) runs `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and PRs.
- **30 end-to-end CLI integration tests.** `cli/tests/integration.rs` runs `parse`, `leaks`, `analyze`, `gc-path`, `diff`, `fix`, `report`, and `config` as subprocesses against synthetic HPROF fixtures and validates key error-path guidance.
- **No coverage tracking.** No `cargo-tarpaulin` or `cargo-llvm-cov` integration. Unknown actual coverage percentage.
- **No property-based testing.** Parser binary handling is a prime candidate for `proptest` or `quickcheck` fuzzing.
- **Benchmarking is now published, but comparative baselines are still missing.** Criterion and RSS numbers are documented, and Step 11 completed dense synthetic large-tier validation. The remaining gap is external comparison and more real-world large-heap fixtures.

### 3.3 Documentation & Onboarding Gaps

- **README and QUICKSTART now reflect shipped behavior.** Output examples in both files match the actual CLI table-based presentation and the live `mnemosyne-cli` command surface.
- **`docs/api.md` now documents the live MCP wire contract.** It covers the current stdio envelope, fourteen live methods, request/response shapes, and serialization details that matter to consumers, including `error_details`.
- **`docs/examples/` is still lightweight, not a full cookbook.** It now points to live examples and includes truthful command/config snippets, but richer scenario-driven walkthroughs are still a documentation opportunity.
- **README badge qualifier gap is closed.** The README now uses a version-qualified badge.
- **No tutorial or cookbook.** No guided walkthrough of a real analysis session. No examples of interpreting output or acting on leak candidates.
- **No troubleshooting guide.** No documentation for common errors, unsupported HPROF variants, or limitations.
- **No comparative performance benchmarks published yet.** The first Mnemosyne baseline is published, but there is still no committed comparison against MAT, VisualVM, or other tools.

### 3.4 Packaging & Release Gaps

- **Release distribution is live for v0.2.0.** `.github/workflows/release.yml` cross-compiles and packages `mnemosyne-cli` for five targets, publishes tagged GitHub releases, builds/pushes `ghcr.io/<owner>/mnemosyne` on tagged releases, and the current production release is now shipped across those channels.
- **✅ crates.io published** (`mnemosyne-core 0.2.0` + `mnemosyne-cli 0.2.0`).
- **✅ `cargo install mnemosyne-cli` is live.**
- **Docker delivery is now in place.** A multi-stage `Dockerfile` builds `mnemosyne-cli` into a non-root `debian:bookworm-slim` runtime image, and tagged releases publish `ghcr.io/<owner>/mnemosyne` with semver plus `latest` tags.
- **✅ SHA256 values filled for v0.2.0.** `HomebrewFormula/mnemosyne.rb` now contains release checksums for the tagged archives.
- **✅ CHANGELOG.md has `[0.2.0] - 2026-03-08` section.** Changelog updates are still manual.

### 3.5 Feature Parity Gaps vs Eclipse MAT

Eclipse MAT is the de-facto standard for JVM heap analysis. With M1 and M1.5 complete, Mnemosyne now has the foundational analysis features (object graph, dominator tree, retained sizes) validated on real-world data. The remaining gaps are in advanced MAT capabilities:

- **No browsable dominator view**: real dominator tree exists and is validated but is not exposed as an interactive explorer.
- **Only a minimal OQL/query surface**: the built-in-field query engine and CLI `query` command landed, but richer predicates, field access, and explorer-style querying still remain.
- **Classloader analysis is present but still narrow**: Mnemosyne can report classloader candidates, but deeper hierarchy/drill-down and broader explorer surfaces remain future work.
- **No object-level comparison**: MAT diffs two dumps at object granularity with object identity and reference-chain changes. Mnemosyne now covers class-level deltas, but not per-object identity tracking.

The gap remains significant but the architectural path is clear. The object graph model, dominator tree algorithm, retained sizes computation, unified leak detection pipeline, and navigation API are all implemented and validated on real-world data. **M3 is the milestone that closes the MAT parity gap for core analysis features.**

### 3.6 UX & Usability Gaps

- **Progress indicators are present, but not yet byte-accurate.** CLI commands now use `indicatif` spinners, but long-running parses still lack a true progress bar tied to bytes or records processed.
- **Error messaging is materially better, but troubleshooting docs still lag.** `CoreError` now carries structured variants for missing files, non-HPROF inputs, HPROF header parse failures, and config errors, and the CLI prints `hint:` lines with suggestions. The remaining gap is documentation for unsupported dump variants and deeper troubleshooting scenarios.
- **No general interactive explorer.** The CLI-first leak-focused chat REPL and persisted MCP session follow-up are shipped, but there is still no broader interactive heap explorer or wider conversation/exploration surface.
- **Output styling is now solid for a CLI tool.** Spinners, colorized labels, and comfy-table aligned ASCII tables with truncation disclosure are shipped. Richer presentation (summary dashboards, interactive TUI) remains future work.

### 3.7 Ecosystem & Community Gaps

- **Community baseline files now exist, but contributor pathways are still thin.** Issue templates, a PR template, `CODE_OF_CONDUCT.md`, and `SECURITY.md` are now in place, but there is still no documented contributor ladder or maintainer path.
- **Examples remain intentionally lightweight.** `docs/examples/` now ships real examples, but it is not yet a full scenario-driven cookbook or sample-project suite.
- **Comparative benchmark publication still remains.** Criterion baselines and RSS/scaling measurements are now published, but fair comparison data against MAT, VisualVM, or hprof-slurp is still missing.
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
| OQL / query language | ⚠️ Partial | The approved-scope M3 query slice is shipped, including retained instance-field projection/filtering and hierarchy-aware `INSTANCEOF`; richer predicates and broader OQL semantics remain future follow-on | Extend the current parser/executor toward fuller OQL over the object model as future follow-on | Very High | High | M3 complete for approved scope |
| Thread inspection | ✅ Delivered | M3 Phase 2 parses `STACK_TRACE` / `STACK_FRAME`, correlates thread roots, and reports per-thread retained bytes plus stack frames | Extend into MCP and future explorer surfaces | High | Medium | M3 |
| ClassLoader analysis | ✅ Delivered | Optional classloader reports now ship in `analyze_heap()`, CLI `analyze --classloaders`, shared report renderers, and MCP `analyze_heap` | Expand duplicate-class / hierarchy-depth analysis if needed | High | Medium | M3 |
| Collection inspection | ✅ Delivered | M3 Phase 2 inspects `HashMap`, `HashSet`, `ArrayList`, and `ConcurrentHashMap` fill ratio and waste | Expand type coverage and reuse the summary in explorer surfaces | Medium | Medium | M3 |
| Export / reporting | ✅ Implemented | Good for current scope | Already strong: 5 formats, provenance, XSS hardening. Add CSV, protobuf, flamegraph later | Low | Medium | M2 |
| UI-based exploration | ❌ Missing | CLI only | Phase from TUI → static HTML → web UI → full explorer | Very High | High | M4 |
| Large dump performance | ⚠️ Partial | Streaming parser handles any size; the current in-memory object graph cleared the approved M3 gate with dense synthetic validation through roughly the 2 GB tier, while broader large-tier follow-on remains evidence-driven | Extend validation and consider alternative storage only if future profiling or real-world heaps justify it | High | High | Post-M3 evidence-driven follow-on |
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
*Remaining Gap:* Not yet exposed in all MCP and richer explorer surfaces.
*Next Steps:* Reuse retained sizes in future MCP and explorer views where they add value.
*Milestone:* Core computation delivered in M1. Real-world validation completed in M1.5. Diff/histogram surface integration shipped in M3.

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
*Current Status:* ⚠️ Partial. Mnemosyne now ships the approved M3 query slice through the CLI `query` command and MCP `query_heap`, including retained instance-field projection/filtering and hierarchy-aware `INSTANCEOF`, but it is still much smaller than MAT's OQL.
*Gap:* MAT's OQL allows `SELECT * FROM java.lang.String WHERE toString().length() > 1000` style queries. Mnemosyne still lacks richer predicates, deeper object-query ergonomics, and broader OQL semantics.
*Recommended Approach:* Extend the current parser → AST → evaluator over the object store instead of replacing it. Grow from the shipped slice toward richer predicates and deeper field/query ergonomics if future evidence justifies it.
*Milestone:* Approved-scope slice delivered in M3; broader OQL semantics remain future M4+ follow-on.

**Thread Inspection.**
*Current Status:* ✅ Delivered in M3 Phase 2. HPROF `STACK_TRACE` and `STACK_FRAME` records are parsed into `ObjectGraph`, `ROOT_THREAD_OBJECT` roots are correlated back to `java.lang.Thread` instances, and the analyzer reports per-thread retained bytes, thread-local counts, and stack frames.
*Remaining Gap:* The current report is text/report-oriented; there is not yet a dedicated MCP or interactive explorer surface for browsing thread-retained subgraphs.
*Recommended Approach:* Reuse the shipped stack-trace and retained-size data for future MCP/API and explorer views rather than introducing a second thread-analysis model.
*Milestone:* Delivered in M3 Phase 2.

**ClassLoader Analysis.**
*Current Status:* ✅ Delivered. Mnemosyne now ships optional classloader reporting in `analyze_heap()`, shared renderers, CLI `analyze --classloaders`, and MCP `analyze_heap` integration.
*Gap:* The current classloader surface is still report-oriented. Deeper hierarchy exploration, duplicate-class analysis, and richer drill-down remain future work.
*Recommended Approach:* Reuse the shipped classloader data model and reporting path for future explorer/UI work rather than adding a second classloader-analysis path.
*Milestone:* Delivered in M3 Phase 3.

**Collection Inspection.**
*Current Status:* ✅ Delivered in M3 Phase 2. The current analyzer recognizes `HashMap`, `HashSet`, `ArrayList`, and `ConcurrentHashMap`, inspects backing-array capacity through retained field data, and reports fill ratio, empty collections, oversized collections, and waste totals.
*Remaining Gap:* Coverage is intentionally focused on the highest-value collection types; broader JDK/framework collection support is future work.
*Recommended Approach:* Keep extending the existing field-data-driven analyzer instead of adding a parallel collection-inspection path.
*Milestone:* Delivered in M3 Phase 2.

**Large Dump Performance.**
*Current Status:* The streaming parser handles arbitrarily large files at the record level. The full object graph parser (`core::hprof::binary_parser`) is validated on populated real-world graphs for ~110MB and ~150MB dumps, Criterion benchmark targets are published, and Step 11 now includes completed dense multi-tier validation at ~500 MB / ~1 GB / ~2 GB with default-path RSS staying under 3.0x and the investigation path staying under 4.0x.
*Gap:* Broader very-large real-world heap coverage is still lighter than the completed dense synthetic ~500 MB / ~1 GB / ~2 GB validation tiers, so additional larger-tier follow-through remains evidence-driven rather than required for approved-scope M3 completion.
*Recommended Approach:* Keep the current in-memory architecture as the shipped baseline. Only extend larger-tier real-world validation or evaluate alternative storage/indexing approaches if future profiling or production heaps show the current path is insufficient.
*Milestone:* Step 11 and the approved M3 gate are complete. Any additional scale work is post-M3 future follow-on.

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

| Dimension | hprof-slurp | Eclipse MAT | draftcode/hprof-parser | Mnemosyne |
|---|---|---|---|---|
| **Language** | Rust | Java (SWT) | Go | Rust |
| **Architecture** | Streaming single-pass, multithreaded pipeline | Index-based, in-memory with disk indexes | Streaming parser → LevelDB persistent index | Sequential `BufReader`, in-memory `ObjectGraph` |
| **Parser** | `nom` combinator library, handles incomplete inputs from chunked streaming | Custom Java parser with parallel indexing (v1.16.0+) | `encoding/binary` + `bufio.Reader`, sequential | `byteorder` crate, sequential `Read` trait |
| **Memory model** | ~500MB flat for 34GB dumps (streaming, no intermediary results stored) | High (Java heap + disk indexes; requires heap proportional to dump size) | Disk-backed (LevelDB); RAM usage bounded by index cache | Unknown at scale (in-memory `HashMap`-backed `ObjectGraph`), but validated on 150MB-class real data and currently benchmarked at 4.23x RSS:dump on the default path and 4.78x on the investigation path |
| **Throughput** | ~2GB/s on 4+ cores (34GB in ~34s) | Slow initial parse, fast re-queries via indexes | Unknown (no benchmarks published) | Initial baseline published: 2.25 GiB/s streaming, 90.5 MiB/s binary parser on a 156 MB fixture |
| **Threading** | Multithreaded: file reader → parser → stats recorder via channels; 3×64MB prefetch buffer | Parallel indexing in recent versions | Single-threaded | Single-threaded parser; Tokio runtime exists but parser is synchronous |
| **Analysis depth** | Shallow: top-N classes, top-N instances, strings, thread stacks. No graph, no retained sizes, no leak detection | Deep: full dominator tree, retained sizes, OQL, leak suspects, collection inspection, thread analysis, classloader analysis | None — parser + index only, zero analysis logic | Medium-high: real dominator tree, retained sizes, leak detection, GC paths, thread inspection, string analysis, collection inspection, top-instance ranking, ClassLoader analysis, and a minimal OQL/query surface are validated; deeper explorer semantics remain future work |
| **Output** | Text + JSON | GUI + batch HTML/CSV reports | None (library only, no CLI or reporting) | Text + Markdown + HTML + TOON + JSON (5 formats) |
| **IDE/AI integration** | None | Eclipse plugin only | None | MCP server (14 live methods, `list_tools`, persisted AI sessions, structured error details), shared AI pipeline |
| **Provenance** | None | None | None | Full provenance system (unique) |
| **CI/CD story** | JSON output for scripting | Batch mode (limited) | None | JSON + TOON structured output, designed for CI |
| **Real-world validation** | Tested on 34GB Spring Boot production dumps | Industry standard, decades of production use | Unknown (author's own note: incomplete record-type coverage) | ✅ Validated on real 150MB Spring Boot heap dumps; dense Step 11 validation also cleared ~500 MB / ~1 GB / ~2 GB synthetic tiers |
| **Maintenance** | Active (26 releases) | Active (Eclipse foundation) | Dead (last commit ~2019) | Active |
| **Stars/community** | ~140 stars, single developer, 26 releases | ~225 stars (GitHub mirror), 11+ contributors, decades of Eclipse ecosystem | 46 stars, 2 contributors, unmaintained | Early stage, <10 stars |

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

  **Mnemosyne comparison:** Mnemosyne now has real-world fixtures, published Criterion + RSS baseline data, completed dense synthetic validation through roughly the 2 GB tier, and optional `hyperfine` / `heaptrack` wrappers with graceful skip behavior. The remaining gaps are comparative external baselines and broader very-large real-world fixture coverage.

#### Feature Gaps Identified

| hprof-slurp Feature | Mnemosyne Status | Priority | Notes |
|---|---|---|---|
| Top-N allocated classes (size, count, largest instance) | ⚠️ Partial (record-level histogram) | P1 | Mnemosyne has class stats from streaming parser but not per-instance largest-instance tracking |
| Top-N largest instances | ✅ Delivered | P2 | M3 Phase 2 ranks the heaviest retained objects directly from the object graph |
| String listing | ✅ Delivered | P2 | M3 Phase 2 decodes `java.lang.String` objects, reports duplicate groups, and quantifies dedup waste |
| Thread stack trace display | ✅ Delivered | P1 | M3 Phase 2 stitches `STACK_TRACE` + `STACK_FRAME` into per-thread reports with retained-memory context |
| JSON output for automation | ✅ Implemented | — | Mnemosyne has JSON + 4 other formats |
| ~2GB/s streaming throughput | ⚠️ Partial | P1 | Mnemosyne now publishes a 2.25 GiB/s streaming-parser baseline on the 156 MB fixture, but still lacks apples-to-apples external comparisons on the same workloads |
| ~500MB memory for 34GB dumps | ❌ Unlikely | P1 | In-memory `ObjectGraph` will balloon for large dumps. Need a bounded "overview mode" |
| Real-world 34GB dump validation | ⚠️ Partial | P0 | Dense synthetic validation now covers roughly 500 MB / 1 GB / 2 GB tiers, but very-large real-world fixture coverage is still missing |
| `hyperfine` + `heaptrack` benchmarking | ✅ Delivered | P1 | Optional wrappers now exist with graceful skip behavior; broader comparative benchmarking remains future work |
| IntelliJ stacktrace compatibility | ✅ Delivered | P3 | Thread-stack output now includes IntelliJ-style compatibility for "Analyze stacktrace" workflows |

#### Key Takeaway

hprof-slurp proves that a Rust HPROF parser can achieve **2GB/s throughput with 500MB memory** — but only by trading away analysis depth. Mnemosyne can still learn from that streaming-performance model, but any dual-mode architecture is now a future scale option rather than an M3 completion requirement.

### 4.5.3 Eclipse MAT Lessons (Supplementary to Section 4)

Section 4 provides the detailed feature-by-feature MAT parity analysis. This subsection adds architectural and strategic lessons from MAT's design:

1. **Index-based architecture for fast re-queries.** MAT builds disk-backed index files during the initial (slow) parse. Subsequent queries are fast because they read from indexes, not the raw dump. Mnemosyne should consider a similar pattern for the "deep analysis" mode — parse once, write an index, and serve queries from the index. This would enable a `mnemosyne serve --web` experience where the initial parse is slow but subsequent exploration is instant.

2. **Batch mode for CI.** MAT can run predefined report templates without the GUI. This validates Mnemosyne's CI/CD automation story — but Mnemosyne should go further with configurable analysis profiles (e.g., `--profile ci-regression` vs `--profile incident-response`).

3. **Historical security vulnerabilities.** MAT had XSS in HTML reports (CVE-2019-17634) and deserialization issues in index files (CVE-2019-17635). Mnemosyne's `escape_html()` hardening already addresses the XSS class. If/when Mnemosyne adds index files, deserialization safety must be designed in from the start.

4. **MAT's weakness is its strength.** MAT's Eclipse/SWT GUI is simultaneously its moat (rich interactive exploration) and its liability (dated UI, no CLI-first workflow, no CI story, no AI integration, no MCP). Mnemosyne should target the same analysis depth through modern interfaces.

### 4.5.3a Other Prior Art — draftcode/hprof-parser (Go)

**Project:** [draftcode/hprof-parser](https://github.com/draftcode/hprof-parser) — Apache 2.0, Go, 46 stars, 2 contributors. Last commit ~2019, unmaintained. A Google 20% project by Masaya Suzuki, self-described as "written by a single person in a day" with incomplete record-type support.

**What it is:** A Go library (not a CLI tool) with three packages: a streaming HPROF parser, protobuf-defined data structures for HPROF record types, and a LevelDB-backed persistent index for random-access queries. It parses a dump into LevelDB once, then serves lookups by object ID, class ID, or GC root type via prefix-scanned iteration. It has zero analysis logic — no dominator tree, no retained sizes, no leak detection, no reporting.

**Why it's noted here:** It independently validates the parse-once-index-many architecture pattern that Eclipse MAT uses and that Mnemosyne's backlog #48 (index/cache file for fast re-queries) targets. Its design choices provide a concrete reference for the eventual index format discussion:

- **Protobuf as the serialization format** for indexed records. For Mnemosyne, Rust-native options (`rkyv`, `bincode`, `redb`) are likely better fits than protobuf, but the concept of a structured, versioned serialization layer is validated.
- **Prefix-based key encoding** (e.g., `string-<hex_id>`, `class-<hex_id>`, `instance-<hex_id>`) for type-specific scans in a KV store. Portable to any embedded database.
- **Batch writes** (100K record batches) during initial indexing for efficiency. Standard best practice.

**Strategic relevance to Mnemosyne:** Minimal. Mnemosyne already surpasses this project in every functional dimension — parser completeness, analysis depth, CLI, reporting, MCP, provenance, testing, and maintenance. The main takeaway is architectural confirmation: when Mnemosyne reaches the index/cache design phase (backlog #48, likely M4-M6), this project's key-encoding scheme and protobuf model are worth reviewing alongside MAT's custom index format and Rust-native serialization options. No priority or roadmap changes result from this analysis.

### 4.5.4 Positioning Matrix

| Capability | hprof-slurp | Eclipse MAT | draftcode/hprof-parser | Mnemosyne (Current) | Mnemosyne (Target) |
|---|---|---|---|---|---|
| Parse 34GB dump | ✅ 34s, 500MB | ⚠️ Slow, high memory | ⚠️ Incomplete record support | ❌ Untested | ✅ Fast overview (<60s) + deep mode |
| Dominator tree | ❌ | ✅ Full | ❌ | ✅ Real-world validated | ✅ Real-world validated |
| Retained sizes | ❌ | ✅ Full | ❌ | ✅ Real-world validated | ✅ Real-world validated |
| Leak detection | ❌ | ✅ Advanced | ❌ | ✅ Graph-backed + heuristic fallback | ✅ Graph-backed + AI-assisted |
| Thread stacks | ✅ | ✅ With object linkage | ❌ | ✅ With object linkage | ✅ With object linkage |
| OQL / queries | ❌ | ✅ Full OQL | ⚠️ KV lookups by ID only | ⚠️ Shipped query slice | ✅ Richer OQL follow-on |
| String analysis | ✅ List | ⚠️ Manual | ❌ | ✅ Duplicate detection + stats | ✅ Duplicate detection + stats |
| Collection inspection | ❌ | ✅ | ❌ | ✅ Fill ratio analysis | ✅ Fill ratio analysis |
| Persistent index | ❌ | ✅ Custom disk indexes | ✅ LevelDB + protobuf | ❌ | ✅ Planned (backlog #48) |
| AI/LLM integration | ❌ | ❌ | ❌ | ⚠️ Stubbed | ✅ Real LLM-backed |
| MCP/IDE integration | ❌ | ❌ | ❌ | ✅ | ✅ Production-ready |
| Provenance tracking | ❌ | ❌ | ❌ | ✅ | ✅ |
| CI/CD automation | ⚠️ JSON only | ⚠️ Batch mode | ❌ | ✅ JSON + TOON | ✅ Profiles + thresholds |
| Performance benchmarks | ✅ Published | ❌ | ❌ | ✅ Baseline published | ✅ Published + comparative |
| CLI / end-user tool | ✅ | ✅ (GUI) | ❌ (library only) | ✅ | ✅ |
| Active maintenance | ✅ | ✅ | ❌ (dead since 2019) | ✅ | ✅ |

### 4.5.5 Strategic Recommendations from Competitor Analysis

1. **Dual-mode parser architecture (P1, post-M1.5).** Add a "fast overview" mode that streams through the dump accumulating class statistics, top-N instances, and thread stacks WITHOUT building the full object graph. This mode should target hprof-slurp-class throughput (~1-2GB/s) and bounded memory (~500MB-1GB). The existing "deep analysis" mode (binary_parser → ObjectGraph → dominator) remains for full graph-backed analysis. Users select via `--mode overview` vs `--mode deep` (default: auto-select based on file size).

2. **Threaded I/O pipeline (P2, future evidence-driven follow-on).** Adopt hprof-slurp's prefetch reader pattern only if future profiling shows the current parser path is insufficient. A dedicated I/O thread could read 64MB chunks ahead of the parser to decouple I/O latency from parse computation.

3. **Benchmark infrastructure (P1, M1.5/M3 shipped plus follow-on).** Mnemosyne now has `criterion`, RSS tooling, and optional `hyperfine` / `heaptrack` wrappers. Future work is comparative publication and any stricter regression gating that evidence justifies.

4. **Thread stack trace extraction (P1, M3 — elevated from P2).** Delivered in M3 Phase 2. Future work is formatting thread dumps for IntelliJ's "Analyze stacktrace" and exposing the same data over richer APIs.

5. **String analysis (P2, M3).** Delivered in M3 Phase 2. Future work is broadening string-decoding coverage and exporter/UI surfaces rather than inventing a second string-analysis path.

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
> | M3 — Core Heap Analysis Parity | [milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md) | ✅ Complete for the approved scope |
> | M3-P1-B2 — Core Analysis Features | [m3-p1-b2-core-analysis-features.md](design/m3-p1-b2-core-analysis-features.md) | ✅ Complete |
> | M3-P2 — Advanced Analysis | [M3-phase2-analysis.md](design/M3-phase2-analysis.md) | ✅ Complete |
> | M4 — UI & Usability | [milestone-4-ui-and-usability.md](design/milestone-4-ui-and-usability.md) | ⚬ Pending |
> | M5 — AI / MCP / Differentiation | [milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md) | ✅ Complete for the approved milestone scope |
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

**Design addenda:** [docs/superpowers/specs/2026-04-13-m3-a-small-closeout-design.md](superpowers/specs/2026-04-13-m3-a-small-closeout-design.md), [docs/superpowers/specs/2026-04-13-m3-b-docker-cve-triage-design.md](superpowers/specs/2026-04-13-m3-b-docker-cve-triage-design.md)

**Objective:** Close the feature gap with Eclipse MAT on core analysis capabilities.

**Why it matters:** Users choose heap analysis tools based on what they can answer. MAT is the benchmark. Mnemosyne needs to answer the same questions, better.

**Status:** ✅ Complete for the approved scope.

**Shipped deliverables:**
1. ✅ MAT-style leak suspects algorithm — objects with disproportionate retained vs shallow size
2. ✅ Histogram improvements — grouping by fully-qualified class, package, and classloader
3. ✅ Thread inspection — stack-trace parsing plus retained-object context
4. ✅ Top-N largest instances — retained-size-backed triage ranking
5. ✅ String analysis — duplicate detection and dedup-savings reporting
6. ✅ Collection inspection — known collection fill-ratio and waste analysis
7. ✅ Unreachable objects analysis — unreachable-set summaries after GC-root traversal
8. ✅ Enhanced heap diff — class-level graph-backed comparison layered onto record-level diffing
9. ✅ Initial OQL/query surface — built-in-field query execution through CLI `query` and MCP `query_heap`
10. ✅ ClassLoader analysis — report-oriented classloader summaries and leak candidates
11. ✅ Configurable analysis profiles — `overview`, `incident-response`, and `ci-regression`
12. ✅ Benchmark baseline plus Step 11 validation — Criterion and dense synthetic validation through roughly the 2 GB tier
13. ✅ Final closeout batch — optional `hyperfine` / `heaptrack` benchmark wrappers with graceful skip behavior, deeper retained instance-field projection/filtering on CLI/MCP query paths, and hierarchy-aware `INSTANCEOF`

**Future follow-on only:**
1. Richer OQL depth beyond the shipped query slice
2. Additional real-world large-dump follow-through only where still justified
3. Streaming overview mode / threaded I/O / `nom` evaluation only if profiling evidence warrants them

**Dependencies:** M1 (object graph, retained sizes, dominator tree) — ✅ delivered and real-world validated. M1.5 (real-world hardening) — ✅ complete.

**Modules/files affected:** `core/src/analysis/engine.rs`, `core/src/analysis/thread.rs`, `core/src/analysis/string_analysis.rs`, `core/src/analysis/collection.rs`, `core/src/analysis/top_instances.rs`, `core/src/analysis/classloader.rs`, `core/src/hprof/binary_parser.rs`, `core/src/hprof/object_graph.rs`, `core/src/query/`, `core/benches/`, `scripts/measure_rss.sh`

**Complexity:** Very High

**Future follow-on order if evidence warrants more work:**
1. Query follow-through beyond the shipped slice
2. Additional scale work only if future profiling justifies it

**Definition of done for M3 documentation closeout:**
- roadmap/docs treat M3 as complete for the approved scope rather than broadly pending
- any deeper query or scale work is explicitly evidence-driven future follow-on
- no shipped M3 feature is still documented as future implementation

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

**Objective:** Record the shipped M5 milestone scope accurately and isolate the narrower post-M5 follow-on work.

**Why it matters:** AI-assisted analysis is the key differentiator, and the approved M5 milestone is now shipped. The remaining work is narrower follow-on, not a broadly pending milestone.

**Dependencies note:** M1.5 must be complete before wiring AI to analysis results — sending empty/heuristic data to an LLM produces misleading output.

**Shipped deliverables:**
1. ✅ LLM integration — `generate_ai_insights()` wired to real provider-backed calls
2. ✅ Configurable prompt/task runner — YAML-backed prompt/template control plus selective context injection
3. ✅ AI-driven leak explanations — retained-size and graph context flow into real AI explanation paths
4. ✅ First AI-backed fix-generation slice — provider-backed one-file / one-snippet patch generation with heuristic fallback
5. ✅ Conversation mode — CLI-first chat plus persisted MCP AI session follow-up
6. ✅ MCP protocol hardening — tool descriptions, structured errors, persisted sessions, and evidence-first request/response validation
7. ✅ Privacy controls — configurable prompt redaction, hashed audit logging, and the shipped prompt-budget guard
8. ✅ Local endpoint support in approved scope — OpenAI-compatible local endpoints are supported today

**Post-M5 follow-on only:**
1. Broader conversation/exploration semantics beyond the shipped leak-focused/session-backed surfaces
2. Native local-provider transports beyond OpenAI-compatible local endpoints
3. Response streaming only if future validation proves the current request/response contract insufficient

**Dependencies:** M1 (meaningful data to send to AI) — ✅ core delivered, M3 (richer analysis context)

**Modules/files affected:** `core/src/analysis/ai.rs`, `core/src/mcp/server.rs`, `core/src/mcp/session.rs`, `core/src/config.rs`, `core/src/llm.rs`, `core/src/fix/generator.rs`, `cli/src/main.rs`, `cli/src/config_loader.rs`

**Complexity:** High

**Remaining implementation order:**
1. Broader conversation/exploration semantics only if product evidence supports more scope
2. Native local-provider transports only if OpenAI-compatible local endpoints are insufficient
3. Response streaming only if the current request/response transport proves inadequate

**Definition of done for post-M5 follow-on:**
- no approved-scope M5 feature is still documented as broadly pending
- follow-on work is explicitly narrower than the shipped milestone
- request/response MCP transport remains the documented shipped contract unless evidence later justifies streaming

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
Mnemosyne now ships real AI-assisted diagnosis for the approved scope: `generate_ai_insights` dispatches `rules`, `stub`, or provider mode; provider mode supports OpenAI-compatible cloud and local endpoints plus Anthropic; `mnemosyne-cli chat` delivers a bounded CLI-first follow-up flow; and the stdio MCP server ships persisted AI sessions for resumed `chat_session`, `explain_leak`, and `propose_fix` requests. Follow-on work is narrower: broader exploration semantics, native non-OpenAI-compatible local transports, and streaming only if later evidence shows the current request/response path is insufficient.

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
| 1 | Object graph parser | P0 | High | XL | None | M1 | ✅ Done |
| 2 | Dominator tree algorithm | P0 | High | L | Object graph | M1 | ✅ Done |
| 3 | Retained size computation | P0 | High | M | Dominator tree | M1 | ✅ Done |
| 4 | Sample HPROF test fixtures | P0 | High | M | None | M1 | ✅ Done |
| 5 | CI pipeline (GitHub Actions) | P0 | High | M | None | M1 | ✅ Done |
| 6 | Unify `detect_leaks()` onto graph path | P0 | High | L | Object graph + retained sizes | M1 | ✅ Done |
| 7 | Rewrite GC path over full object graph | P0 | High | M | Object graph | M1 | ✅ Done |
| 8 | Object graph navigation API | P0 | High | M | Object graph | M1 | ✅ Done |
| 9 | Integration tests via reusable synthetic HPROF fixtures | P0 | High | L | Test fixtures + CI | M1 | ✅ Done |
| 9a | Fix HPROF tag constants (0x0D/0x1C swap) | P0 | Critical | S | None | M1.5 | ✅ Done |
| 9b | Add HEAP_DUMP_SEGMENT (0x1C) parsing support | P0 | Critical | M | Tag fix (9a) | M1.5 | ✅ Done |
| 9c | Real-world HPROF test fixture + validation tests | P0 | Critical | M | Tag fix (9a) | M1.5 | ✅ Done |
| 9d | End-to-end pipeline validation on real dumps | P0 | High | M | 9a + 9b + 9c | M1.5 | ✅ Done |
| 9e | Investigate heuristic fallback zero-results on real data | P1 | High | M | Tag fix (9a) | M1.5 | ✅ Done |
| 9f | Leak-ID validation for explain/fix commands | P1 | Medium | S | None | M1.5 | ✅ Done |
| 10 | Release binaries | P1 | High | M | CI pipeline (✅) | M2 | ✅ Done |
| 11 | cargo install support | P1 | High | S | Release setup | M2 | ✅ Done |
| 12 | CLI progress bars + colors | P1 | Medium | S | None | M2 | ✅ Done |
| 12a | Table-formatted CLI output | P1 | Medium | S | CLI UX (✅) | M2 | ✅ Done |
| 13 | MAT-style leak suspects | P1 | High | L | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 14 | Histogram by class/package/classloader | P1 | High | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 15 | Homebrew formula | P1 | Medium | S | Release binaries | M2 | ✅ Done |
| 16 | LLM integration (real API calls) | P1 | High | L | M3 analysis context | M5 | ✅ Done — approved M5 scope shipped: provider mode, YAML prompt templates, prompt redaction, audit logging, prompt-budget guard, MCP tool/error hardening, persisted MCP AI sessions, CLI-first chat, and the first AI-backed fix-generation slice landed |
| 17 | Enhanced heap diff | P1 | Medium | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 18 | Static interactive HTML reports | P2 | High | L | Reporting exists | M4 | ⚬ Pending |
| 19 | OQL query engine | P2 | High | XL | M3 Phase 2 data model | M3 | ✅ Done for approved scope — shipped query slice includes retained instance-field projection/filtering and hierarchy-aware `INSTANCEOF`; richer OQL remains future follow-on |
| 20 | Thread inspection | P1 | High | L | M1.5 ✅ | M3 | ✅ Done (M3 Phase 2) |
| 21 | ClassLoader analysis | P2 | Medium | L | M1.5 ✅ | M3 | ✅ Done |
| 22 | Local web UI | P2 | High | XL | HTML reports | M4 | ⚬ Pending |
| 23 | Collection inspection | P1 | High | M | M1.5 ✅ | M3 | ✅ Done (M3 Phase 2) |
| 24 | Unreachable objects | P2 | Medium | M | M1.5 ✅ | M3 | ✅ Done (M3-P1-B2) |
| 25 | Configurable prompt/task runner | P2 | Medium | L | LLM integration | M5 | ✅ Done — rule-based task runner plus YAML prompt-template loading landed |
| 26 | AI conversation mode | P2 | Medium | L | LLM integration | M5 | ⚠️ Follow-on only — CLI-first leak-focused chat and persisted MCP AI sessions are shipped; broader conversation/exploration semantics remain |
| 27 | Docker image | P2 | Medium | S | Release automation | M2 | ✅ Done |
| 28 | Example projects + sample dumps | P2 | Medium | M | Test fixtures (✅) | M6 | ⚬ Pending |
| 29 | Benchmark suite (`criterion`) | P1 | Medium | M | M1.5 ✅ | M3 | ✅ Done — baseline published in `docs/performance/memory-scaling.md` |
| 30 | Plugin/extension system | P3 | Medium | XL | Stable APIs (M3+) | M6 | ⚬ Pending |
| 31 | Full interactive heap browser | P3 | High | XL | Web UI + OQL | M4 | ⚬ Pending |
| 32 | Local LLM support | P3 | Medium | L | LLM integration | M5 | ⚠️ Partial — OpenAI-compatible local endpoints are supported today; native local-provider transports remain future work |
| 33 | Real MCP API documentation for `docs/api.md` | P2 | Medium | M | MCP server (✅) | M3 | ✅ Done |
| 34 | Real usage examples in `docs/examples/` | P2 | Medium | M | M1.5 ✅ | M3/M6 | ✅ Done — real examples now ship; broader cookbook/sample-project work remains M6-scale follow-on |
| 35 | README badge version qualifier (`v0.2.0-alpha`) | P3 | Low | S | None | M3 | ✅ Done |
| 36 | Dockerfile base image CVE triage | P2 | Medium | S | None | M3 | ✅ Done — runtime scan found 0 critical and 16 high findings; all runtime highs triaged as `wont-fix`, with no safe minimal in-family remediation identified |
| 37 | Streaming "overview" mode — bounded-memory class/instance stats without full graph | P2 | High | L | Scaling validation (11) | Future follow-on | ⚬ Pending — not an M3 completion gate after Step 11; only pursue if future profiling or real-world scale evidence justifies it |
| 38 | Thread stack trace extraction — STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT | P1 | High | L | M1.5 ✅ | M3 | ✅ Done (M3 Phase 2) |
| 39 | Benchmark infrastructure — `hyperfine` CLI timing + `heaptrack` memory profiling | P2 | Medium | M | Criterion baseline (✅) | M3 | ✅ Done — optional wrappers now ship with graceful skip behavior |
| 40 | Top-N largest instances report — per-class largest single instance size | P1 | Medium | S | Object graph ✅ | M3 | ✅ Done (M3 Phase 2) |
| 41 | String analysis — list strings, detect duplicates, quantify dedup savings | P1 | High | M | Object graph ✅ | M3 | ✅ Done (M3 Phase 2) |
| 42 | Threaded I/O pipeline — prefetch reader with 64MB chunked read-ahead | P2 | Medium | L | Scaling validation (11) | Future follow-on | ⚬ Pending — only pursue if future profiling shows the current parser path is insufficient |
| 43 | Memory-bounded object store evaluation — measure RSS at 1GB+ | P1 | High | M | Benchmark baseline (✅) | M3 | ✅ Done — dense synthetic validation now covers ~1 GB and ~2 GB tiers |
| 44 | Large-dump validation program — dumps at 500MB/1GB/2GB/5GB tiers | P1 | High | M | M1.5 ✅ | Future follow-on | ⚠️ Partial — ~500 MB / ~1 GB / ~2 GB tiers are complete; larger follow-through remains available if needed |
| 45 | `nom` parser evaluation — prototype, compare throughput | P3 | Medium | L | Scaling validation (11) | Future follow-on | ⚬ Pending — only pursue if benchmark evidence points to a parser bottleneck |
| 46 | Configurable analysis profiles — `--profile ci-regression|incident-response|overview` | P2 | Medium | M | M3 Phase 3 | M3/M5 | ✅ Done |
| 47 | IntelliJ stacktrace format compatibility | P3 | Low | S | Thread stacks (38) | M3 | ✅ Done |
| 48 | Index/cache file for fast re-queries | P3 | Medium | XL | M3 analysis features | M4/M6 | ⚬ Pending |
| **49** | **`leaks` command: explicit "No leaks detected" message when zero leaks** | **P1** | **Medium** | **S** | **None** | **M3** | **✅ Done — `leaks` now prints `No leak suspects detected.`** |
| **50** | **`fix` command: clearer UX when no leaks detected** | **P2** | **Low** | **S** | **None** | **M3** | **✅ Done — `fix` already prints `No fix suggestions available for the provided criteria.`** |
| **51** | **Docker build validation in CI** | **P1** | **Medium** | **S** | **None** | **M2** | **✅ Done — added to CI pipeline** |
| **52** | **v0.2.0 multi-channel deployment** | **P0** | **High** | **M** | **M3-P1 + benchmarks** | **M2** | **✅ Done — GitHub Releases, GHCR Docker, crates.io, Homebrew** |

---

## Section 11 — Recommended Immediate Next Steps

**✅ M1, M1.5, M2, M3 core parity work, M5 AI/MCP differentiation, and Step 11 large-dump validation are all COMPLETE in this branch.** v0.2.0 is deployed to all channels (GitHub Releases, GHCR Docker, crates.io, Homebrew). The graph-backed pipeline is production-validated, benchmark data is published, and the remaining differentiation work now centers on deeper post-M3 product work.

### Previously Completed Steps
1. ✅ Fix HPROF tag constants — `TAG_HEAP_DUMP_SEGMENT` corrected to `0x1C`
2. ✅ Add HEAP_DUMP_SEGMENT (0x1C) to binary parser dispatch
3. ✅ Real-world HPROF test fixture + validation — 4 integration tests against real JVM dumps
4. ✅ Investigate heuristic fallback zero-results — validated with nonexistent-package filter test
5. ✅ Leak-ID validation for explain/fix — `validate_leak_id()` wired into CLI and MCP
6. ✅ HEAP_DUMP_SEGMENT unit tests — `build_segment_fixture()` + dedicated parser tests
7. ✅ v0.2.0 release + benchmark baseline — all M1.5+M3-P1 fixes released, Criterion benchmarks published, RSS measurements documented, scaling projections written
8. ✅ M3 Phase 1 — histogram grouping, MAT-style suspects, unreachable objects, class-level diff

### Step 9: Quick UX Wins from v0.2.0 Validation
**Status:** ✅ Complete.
**Actions:**
  - (a) ✅ **`leaks` command now prints `No leak suspects detected.` when zero leaks are found**
  - (b) ✅ **`fix` command already returns a clear no-results message instead of empty output**
**Files:** `cli/src/main.rs`
**Owner:** Implementation Agent
**Effort:** Small (< 1 batch)
**Dependencies:** None

### Step 10: M3 Phase 2 — Investigation Features
**Status:** ✅ Complete.
**Actions:**
  - (a) ✅ **Thread inspection** (backlog #20/#38) — parses `STACK_TRACE` + `STACK_FRAME` + `ROOT_THREAD_OBJECT`, links threads to retained objects, and reports per-thread retained memory plus stack frames
  - (b) ✅ **Top-N largest instances** (backlog #40) — ranks the heaviest retained objects for quick triage
  - (c) ✅ **String analysis** (backlog #41) — detects duplicates, quantifies dedup savings, and reports top strings by size
  - (d) ✅ **Collection inspection** (backlog #23) — inspects `HashMap` / `HashSet` / `ArrayList` / `ConcurrentHashMap` fill ratio and waste
  - (e) ✅ **CLI wiring + polish** — `analyze` now exposes `--threads --strings --collections --top-instances --top-n --min-capacity` and renders text-mode tables for all four analyzers
**Files:** new `core/src/analysis/thread.rs`, new `core/src/analysis/string_analysis.rs`, new `core/src/analysis/collection.rs`, `core/src/hprof/binary_parser.rs` (STACK_TRACE/STACK_FRAME parsing), `core/src/hprof/object_graph.rs` (StackTrace/StackFrame types), `core/src/analysis/engine.rs`, `cli/src/main.rs`
**Owner:** Implementation Agent
**Effort:** Large (2-3 batches)
**Dependencies:** M3 Phase 1 (✅ delivered) — histogram/suspects infrastructure

### Step 11 (parallel with Step 10): Large-Dump Scaling Validation
**Why this step existed:** the original v0.2.0 baseline was 3.56x RSS:dump on 156 MB, but the post-Phase-2 re-baseline jumped to 4.78x and triggered remediation. That validation gate is now closed: dense synthetic follow-through covered roughly 500 MB / 1 GB / 2 GB tiers and cleared the current in-memory architecture for the active roadmap scope.
**Design doc:** [memory-scaling.md](design/memory-scaling.md) (Step 11 validation protocol section + remediation design)
**Actions:**
  - (a) ✅ **Re-baseline (done):** Post-Phase-2 re-baseline measured 4.78x RSS:dump on the 156 MB fixture and triggered the remediation path
  - (b) ✅ **field_data remediation (done):** conditional `field_data` retention via `ParseOptions.retain_field_data` is implemented. See [memory-scaling.md § Remediation](design/memory-scaling.md). Default `analyze`/`leaks` now measure 4.23x; investigation flags (`--threads`/`--strings`/`--collections`) remain 4.78x when opted in.
  - (c) ✅ **Post-remediation validation (done):** 156 MB re-measurement confirmed the new default path at 4.23x and the opt-in investigation path at 4.78x
  - (d) ✅ **Large-dump validation (done)** — dense synthetic heaps now cover ~500 MB / ~1 GB / ~2 GB tiers with stable object/reference-heavy shapes and decision-quality RSS measurements
  - (e) ✅ **Memory-bounded evaluation (done)** — dense multi-tier RSS profiling kept the default path at 2.87x-2.90x and the investigation path at 3.89x-3.92x
  - (f) ✅ **Decision gate (done)** — current in-memory architecture cleared the Step 11 gate, so streaming overview mode / memmap2 remain future scale levers rather than active blockers
**Files:** `core/src/hprof/binary_parser.rs`, `core/src/analysis/engine.rs`, `core/src/graph/gc_path.rs`, `scripts/`, `docs/performance/memory-scaling.md`
**Owner:** Implementation Agent + Testing Agent
**Effort:** Medium
**Dependencies:** Benchmark baseline (✅ published), Phase 2 re-baseline (✅ measured)

### Step 12: M3 Phase 3 — Advanced Query + ClassLoader
**Status:** ✅ Complete for the approved M3 scope, with richer OQL depth still open as follow-through.
**Why after Phase 2:** OQL requires the data model enrichments from thread + collection + string analysis. ClassLoader analysis completes the MAT parity story.
**Actions:**
  - (a) ✅ **ClassLoader analysis** (backlog #21) — shipped in `analyze_heap()`, report renderers, CLI `analyze --classloaders`, and MCP `analyze_heap`
  - (b) ✅ **OQL/query first slice** (backlog #19) — a minimal built-in-field query surface shipped; richer predicates, field access, and explorer-style depth remain follow-through
  - (c) ✅ **Configurable analysis profiles** (backlog #46) — `--profile ci-regression|incident-response|overview`
**Owner:** Implementation Agent
**Effort:** Very Large (OQL alone is XL)
**Dependencies:** Steps 10 (data model) + 11 (scaling validation)

### Step 13: M5 Phase 1 — External Provider AI Execution
**Status:** ✅ First slice complete.
**Actions:**
  - (a) ✅ **OpenAI-compatible provider mode** — `AiMode::Provider` now calls real chat-completions endpoints via a small transport layer in `core::llm`
  - (b) ✅ **Provider runtime config** — `endpoint`, `api_key_env`, `max_tokens`, and `timeout_secs` now flow through `[ai]` / `[llm]` config and matching `MNEMOSYNE_AI_*` env overrides
  - (c) ✅ **Honest provider failures** — missing API keys, unsupported providers, transport failures, and malformed TOON responses now return explicit errors instead of silently falling back
  - (d) ✅ **Async boundary fix** — provider execution is isolated with `spawn_blocking` at async CLI/MCP call sites so the blocking transport does not panic inside the Tokio runtime
  - (e) ✅ **Verification** — provider-mode AI unit coverage plus end-to-end CLI JSON regression coverage are in place
**Files:** `core/src/analysis/ai.rs`, `core/src/llm.rs`, `core/src/config.rs`, `cli/src/config_loader.rs`, `core/src/analysis/engine.rs`, `core/src/mcp/server.rs`, `cli/src/main.rs`, `cli/tests/integration.rs`, `core/Cargo.toml`
**Owner:** Implementation Agent
**Effort:** Medium
**Dependencies:** configurable AI task runner (✅ delivered)

### Step 14: M5 Phase 2 — Prompt/Provider Hardening
**Design addendum:** `docs/superpowers/specs/2026-04-12-ai-prompt-templates-design.md`
**Design addendum:** `docs/superpowers/specs/2026-04-12-ai-anthropic-transport-design.md`
**Design addendum:** `docs/superpowers/specs/2026-04-12-ai-cli-chat-design.md`
**Why next:** This was the hardening phase that closed the approved M5 scope after the first provider slice made the AI path real.
**Actions:**
  - (a) ✅ **Configurable prompt templates** (backlog #25) — provider-mode instruction sections now load from YAML via an embedded default plus optional `[ai.prompts].template_dir` override directory
  - (b) ✅ **Anthropic transport** — transport code plus targeted core and CLI verification are in place
  - (c) ✅ **MCP protocol improvements** — `list_tools` discovery plus backward-compatible structured `error_details` are in place; keep streaming as future follow-on only if the current transport proves insufficient
  - (d) ✅ **Privacy controls** — provider-mode prompt redaction and hashed audit logging are now in place under `[ai.privacy]` (`redact_heap_path`, `redact_patterns`, `audit_log`), and `max_tokens` now provides a minimal prompt-budget guard that trims leak context while preserving instructions
  - (e) ✅ **Conversation mode follow-through** — `mnemosyne-cli chat <heap.hprof>` remains the CLI-first slice, and MCP now persists explicit heap-bound AI sessions via `create_ai_session`, `resume_ai_session`, `get_ai_session`, `close_ai_session`, and `chat_session`, with session-backed `explain_leak` / `propose_fix` follow-up
**Owner:** Implementation Agent
**Effort:** Large
**Dependencies:** Step 13

### Step 15: v0.3.0 Release — MAT-Parity Analysis
**Why here:** After M3 Phases 1-3 complete, Mnemosyne will have core MAT-parity analysis features. This is a significant milestone worth a named release because it transitions Mnemosyne from "interesting alpha" to "credible MAT alternative."
**Actions:**
  - Tag and release v0.3.0 with all M3 features
  - Update CHANGELOG, release notes, README feature list
  - Publish comparative analysis: what Mnemosyne can do vs MAT in table form
**Owner:** Implementation Agent + Documentation Sync
**Effort:** Small
**Dependencies:** Steps 10-12

### Remaining Roadmap Order
The remaining roadmap should now execute in this order:
1. **Small remaining M3 closeout work** — README badge qualifier, real usage examples, IntelliJ stacktrace compatibility, and benchmark/query follow-through only where still justified
2. **M4 (UI & Usability)** — the next full open milestone: interactive HTML reports and the local web UI on top of shipped M3/M5 capabilities
3. **M5 follow-on only where evidence supports it** — broader conversation/exploration semantics, native local-provider transports beyond OpenAI-compatible endpoints, and transport streaming only if the current request/response contract proves insufficient
4. **M6 (Ecosystem & Community)** — docs, examples, benchmarks, integrations, and community infrastructure after M4 and any justified M5 follow-on

**Why this order:**
- M3 is no longer a broadly open milestone; the remaining work is narrow and should be finished before opening larger milestone work
- M4 is the next genuinely open product milestone and now builds on already shipped analysis and AI surfaces
- M5 is not reopened as a fully pending milestone; only narrower follow-on work remains
- M6 benefits from showcasing the post-M4 product rather than competing with major UI work for attention

### Documentation Debt (track alongside feature work)
- **`docs/api.md`** — now documents the live MCP wire contract. Keep it synchronized as any post-M5 follow-on evolves.
- **`docs/examples/`** — needs real CLI/MCP usage examples (backlog #34). This is now a small M3 closeout item that also contributes to later M6 work.
- **Dockerfile base image CVEs** — triaged in M3-B: Docker Scout was attempted first but blocked by missing authentication in this environment, so the recorded result comes from fallback Grype scans of saved runtime and builder-stage images. The shipped runtime scan showed no criticals and only `wont-fix` high findings, while the builder-stage scan was much noisier but non-shipping, so keep backlog `#36` open for future Debian refresh windows or a later evidence-backed base-image change.
- **README version badge** — update to `v0.2.0-alpha` (backlog #35). Low priority.

---

## Section 12 — Risk Register & Lessons Learned

### Active Risks

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **In-memory ObjectGraph may still regress on future real-world large dumps** | Medium | Medium | Dense Step 11 validation cleared ~500 MB / ~1 GB / ~2 GB tiers at 2.87x-2.90x default-path RSS and 3.89x-3.92x investigation-path RSS, but additional real-world large-dump fixtures would further de-risk the architecture. Streaming overview mode (backlog #37) remains the fallback architecture if future real-world data regresses. |
| **Dockerfile base image (`debian:bookworm-slim`) carries distro-level CVE noise** | Medium | Medium | M3-B triage attempted Docker Scout first but fell back to saved-image Grype scans when Scout auth was unavailable. The shipped runtime image had no critical findings and only `wont-fix` high findings, while the builder-stage scan was substantially noisier but did not affect the shipped runtime contract, so no safe minimal in-family remediation was justified in this environment. Reassess on future Debian refresh windows or with stronger scanner evidence before switching base families. |
| **M3 Phase 2-3 scope is large** | Medium | High | Phase 2 has 4 investigation features; Phase 3 has OQL + ClassLoader + profiles. Phased approach ensures incremental delivery. Each phase gate checks that previous phase is shipping and tested before committing to the next. |
| **External AI execution may prove harder than the new rule-runner slice suggests** | Medium | Medium | The first provider-backed slice now works for OpenAI-compatible endpoints, and Step `14(d)` now covers prompt redaction, hashed audit logging, and a minimal prompt-budget guard, but broader prompt iteration and provider support still need careful incremental follow-through. Keep each M5 slice narrow. |
| **AI integration quality may drift across providers** | Medium | Medium | Provider mode is now real and verified for strict TOON parsing, but prompt quality and provider-specific response shaping still need iteration. Mitigation: keep the TOON contract strict, add provider-specific tests, and expand one provider at a time. |
| **Other HPROF sub-record types may have parsing bugs** | Medium | Medium | Real-world dumps may contain sub-record types not covered by current fixtures. M3 Phase 2 (thread inspection) will exercise STACK_TRACE/STACK_FRAME records, expanding coverage. Large-dump validation further mitigates. |
| **hprof-slurp may add analysis depth features** | Low | Low | hprof-slurp is actively maintained. If it adds dominator tree/retained sizes, Mnemosyne's "analysis depth" differentiator narrows. Mitigation: deliver M3 investigation features + AI integration to establish the depth+AI moat. |
| **Eclipse MAT may modernize its CLI/API** | Low | Low | MAT recently moved to GitHub and shipped parallelism improvements. If MAT adds a modern CLI or AI features, Mnemosyne's DX differentiator narrows. Mitigation: move quickly on MCP, AI, and modern CLI to establish the DX moat. |

### Resolved Risks

| Risk | Resolution |
|---|---|
| **Tag-constant bug (P0 correctness)** | ✅ Fixed in M1.5-B1: `TAG_HEAP_DUMP_SEGMENT` corrected to `0x1C`. Validated end-to-end on real-world data. |
| **Heuristic fallback tuning** | ✅ Validated in M1.5-B2: fallback path produces candidates when graph results are filtered. |
| **Leak-ID commands return generic responses** | ✅ Fixed in M1.5-B2: `validate_leak_id()` returns `CoreError::InvalidInput` for unknown IDs. |
| **No benchmark data (can't detect regressions)** | ✅ Resolved in v0.2.0 and extended in Step 11: Criterion benchmarks + RSS measurements published. Current contract: 2.25 GiB/s streaming, 90 MiB/s binary parse, 1.85s dominator, 4.23x RSS:dump on the 156 MB real fixture, and dense multi-tier validation at 2.87x-2.90x default path / 3.89x-3.92x investigation path through ~2 GB. |
| **v0.1.1 tag bug shipped to users** | ✅ Resolved: v0.2.0 released with tag fix to all channels. No breaking API changes. |
| **Distribution gaps** | ✅ Resolved: v0.2.0 fully deployed to GitHub Releases (5 targets), GHCR Docker, crates.io, Homebrew. Docker build validation added to CI. |

### Lessons Learned (v0.2.0 Release + Validation)

1. **Synthetic-only test coverage creates false confidence.** All 87 tests passed, clippy was clean, CI was green — but the tool produced incorrect output on every real-world dump. Lesson: real-world HPROF test fixtures are mandatory, not nice-to-have.
2. **Tag constant errors are insidious.** The HPROF spec uses sequential hex values (0x0C, 0x0D, 0x0E) for unrelated record types and then jumps to 0x1C/0x2C for segment/end. This is a spec design that invites off-by-one style errors. Multiple independent sources map these tags differently. Lesson: verify tag constants against the authoritative JDK source (`hprof_b_spec.h`), not third-party reference docs.
3. **Silent fallback can mask critical bugs.** The provenance system correctly labeled outputs as `[PARTIAL]` and `[FALLBACK]`, but the user experience was "analyze works, just with limited data" rather than "the parser is completely failing on your dump." Lesson: consider adding a warning when the graph-backed path fails entirely and ALL results are fallback.
4. **The features that work well are genuinely good.** `parse`, `diff`, `config`, reporting formats, error handling, and the provenance system all performed correctly on real-world data. The issue is specifically in the HPROF binary parser’s tag dispatch, not in the overall architecture or downstream pipeline.
5. **Cross-platform builds work.** 5-target cross-compilation producing working binaries demonstrates the Rust cross-compilation story is mature.
6. **Multi-channel deployment validates confidence.** v0.2.0 being live on 5 channels (GitHub Releases, Docker, crates.io, Homebrew, CI) means early users immediately benefit from the critical tag fix. Lesson: invest in release automation early.
7. **Benchmark baselines prevent regression creep.** Publishing the first Criterion + RSS baseline and then re-baselining after Step 11 exposed a real regression that would otherwise have gone unnoticed. Lesson: measure early, then re-measure after each memory-heavy feature batch.
8. **Zero-output on zero-results is a UX anti-pattern.** v0.2.0 validation revealed that `leaks` exits 0 but prints nothing when no leaks are found. Users cannot tell success from a broken pipe. Lesson: always print confirmation messages.
9. **Fix/analyze workflow disconnect needs attention.** `fix` produces suggestions even when `analyze` reports zero leaks because it operates on dominator data, not leak filter output. Labels are correct but the workflow is confusing.

---

*This roadmap is a living document. Update it after each major batch completion.*
*Last review: roadmap/design alignment for shipped M3 and approved-scope M5 state (2026-04-13).*
*Next review: after the small remaining M3 closeout batch or the first M4 slice lands.*
