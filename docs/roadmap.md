# Mnemosyne Roadmap & Milestones

> **Last updated:** 2026-03-07
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

Mnemosyne is today an **alpha-stage Rust-based JVM heap analysis tool**. It can stream-parse HPROF files to produce class histograms and heap summaries, run deterministic leak-detection heuristics with package and severity filters, trace GC root paths with budget-aware synthetic fallback, generate template-based fix suggestions, and render results in five output formats (Text, Markdown, HTML, TOON, JSON) — all backed by a provenance system that labels every synthetic, partial, fallback, or placeholder data surface. A stdio MCP server exposes six JSON-RPC handlers (`parse_heap`, `detect_leaks`, `map_to_code`, `find_gc_path`, `explain_leak`, `propose_fix`), making the tool available inside VS Code, Cursor, Zed, JetBrains, and ChatGPT Desktop. The parser reads HPROF headers and record-level tags to derive statistics, but **does not parse individual object references or field data** — which means there is no object graph, no retained-size computation, and no real dominator tree today. The AI module (`generate_ai_insights`) is **fully stubbed**: it returns deterministic template text with zero LLM calls and zero HTTP client dependencies.

Mnemosyne has the foundations to become **the first Rust-native, AI-assisted heap analysis platform** that rivals Eclipse MAT in analysis depth while offering capabilities no existing tool provides: provenance-tracked outputs that distinguish real analysis from heuristic guesses, MCP-native IDE integration for copilot-style workflows, CI/CD-friendly automation via structured JSON and TOON output, and an AI-native architecture designed from day one for LLM integration. The Rust core means multi-GB heap dumps can be processed with predictable memory usage and no GC pauses — a meaningful advantage over Java-based tools like MAT and VisualVM for production incident response.

Five properties position Mnemosyne to stand out in a crowded JVM tooling ecosystem: **(1)** Rust performance enabling streaming analysis of heap dumps that exceed host RAM; **(2)** a provenance system unique among heap analyzers, giving users and automation confidence in result trustworthiness; **(3)** MCP-first architecture that makes heap analysis a conversation in the developer's IDE rather than a separate tool; **(4)** AI-native design with well-shaped type contracts (`AiInsights`, `AiWireExchange`, config plumbing) ready for LLM wiring; and **(5)** automation-friendly structured output (JSON, TOON) enabling CI regression detection with machine-readable leak signals.

Honest assessment: **significant work remains** to deliver on this vision. The most critical gap is the absence of a real object reference graph — without it, retained-size computation, dominator trees, accurate leak suspects, and meaningful GC path tracing are all impossible. The second critical gap is AI wiring: every "AI-powered" claim in the README is aspirational today. Testing is minimal (~12 unit tests, zero integration tests), there is no CI pipeline, no release binaries, and no sample heap dumps. The architecture is sound, the module boundaries are clean, and the provenance/reporting layers are genuinely well-built — but the core analysis engine needs to be rebuilt on top of a real object graph before the tool can deliver trustworthy results.

---

## Section 2 — Current State Assessment

### Core Capabilities

| Capability | Status | Honest Assessment |
|---|---|---|
| HPROF streaming parser | ✅ Implemented | Reads headers + record tags via `byteorder`-based binary parsing. Derives class histograms from record-type stats (INSTANCE_DUMP, OBJ_ARRAY_DUMP, etc.). Does **NOT** parse individual object fields, references, or string content. Cannot build an object reference graph. |
| Leak detection heuristics | ✅ Implemented | `detect_leaks()` and `analyze_heap()` in `core::analysis` use class histogram data + package/severity filters + explicit leak-kind selection. Deterministic. Falls back to synthetic entries when histograms are empty. Not backed by retained-size data — leak candidates are ranked by shallow aggregate size, not true retention. |
| Graph / dominator preview | ⚠️ Partial | `core::graph::summarize_graph()` builds a small `petgraph` from the top 12 class or record entries and runs `simple_fast` dominator computation on this synthetic tree. This is a **reporting visualization**, not a real dominator tree. No retained-size computation. |
| GC root path tracing | ✅ Implemented | `core::gc_path` is among the most complete features. Parses real GC root sub-records (15 root types), CLASS_DUMP, INSTANCE_DUMP, OBJ_ARRAY_DUMP. Builds an in-memory edge map with configurable budget limits (`DEFAULT_MAX_INSTANCES=32768`, `MAX_ROOTS=8192`). BFS from roots to target. Falls back to synthetic path with `ProvenanceKind::Fallback` when data is insufficient. |
| AI / LLM insights | ❌ Stubbed | `core::ai::generate_ai_insights()` returns deterministic template text. No HTTP client in `Cargo.toml`, no API calls, no LLM SDK. Config plumbing exists (`AiConfig` with provider/model/temperature fields) but terminates at the stub. The "AI-powered" claim in README is entirely aspirational. |
| Fix suggestions | ⚠️ Template only | `core::fix::propose_fix()` generates template patches in three styles (Minimal, Defensive, Comprehensive). No AI involvement, no code analysis. Useful scaffolding with provenance markers. |
| Source mapping | ✅ Implemented | `core::mapper::map_to_code()` scans project dirs for `.java`/`.kt` files, runs `git blame` for metadata. Basic but functional for local projects. |
| Reporting | ✅ Implemented | `core::report` renders 5 formats (Text, Markdown, HTML, TOON, JSON). HTML output uses `escape_html()` for XSS prevention. TOON uses `escape_toon_value()` for control characters. Provenance markers rendered in all non-JSON formats. One of the most polished subsystems. |
| MCP server | ✅ Wired | `core::mcp::serve()` runs a stdio JSON-RPC loop with async Tokio I/O. Handles 6 methods. Works end-to-end but backed by the same stubs/heuristics as CLI. |
| Config system | ✅ Implemented | `cli::config_loader` reads TOML files from 5 locations + env vars + CLI flags. `core::config` defines `AppConfig`, `AiConfig`, `ParserConfig`, `AnalysisConfig`. Clean, well-layered. |
| Provenance system | ✅ Implemented | `ProvenanceKind` (Synthetic, Partial, Fallback, Placeholder) + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in all report formats and CLI output. Unique feature in the heap-analysis space. |

### Technical Strengths

- **Rust performance model**: streaming parser with `BufReader`, no GC, predictable memory. Can handle files larger than RAM in principle.
- **Clean module separation**: 11 focused modules in `core` (`ai`, `analysis`, `config`, `errors`, `fix`, `gc_path`, `graph`, `heap`, `mapper`, `mcp`, `report`) with clear single-responsibility boundaries.
- **Streaming design**: parser processes HPROF records sequentially without loading the full dump. Foundation for scaling to multi-GB files.
- **Provenance system**: genuinely novel for a heap analyzer. Labels every synthetic/heuristic output surface so consumers know what to trust.
- **Multi-format output**: 5 report formats with consistent provenance rendering. HTML is XSS-hardened. TOON enables compact CI consumption.
- **Config hierarchy**: TOML + env vars + CLI flags with clear precedence. Production-ready design pattern.
- **MCP integration**: stdio JSON-RPC server with 6 handlers. First-mover for heap analysis in the MCP ecosystem.
- **Graph infrastructure**: `petgraph` dependency already present. The crate supports Lengauer-Tarjan dominators — the same algorithm needed for real retained-size computation.
- **Type contracts**: well-shaped request/response types (`AnalyzeRequest`, `AnalyzeResponse`, `GcPathResult`, `FixResponse`, etc.) that establish stable contracts between CLI, MCP, and core.

### Major Weaknesses

- **No object graph**: the parser reads record tags and byte lengths but does **not** parse object fields, references, or class field descriptors. This means every downstream analysis (retained sizes, dominators, leak suspects, GC paths beyond the budget-limited best-effort) is operating on aggregate statistics, not real object relationships. This is the single biggest gap.
- **AI is 100% stubbed**: `generate_ai_insights()` returns hardcoded template strings. There are zero HTTP client dependencies in `Cargo.toml`. The `AiConfig` fields (provider, model, temperature, API key) exist but connect to nothing. Every "AI-powered" claim in documentation is marketing ahead of implementation.
- **Minimal test coverage**: approximately 12 unit tests total (provenance rendering, HTML/TOON escaping, a few analysis tests). Zero integration tests, zero end-to-end tests, no test fixtures or sample heap dumps.
- **No CI/CD**: no GitHub Actions, no pre-commit hooks, no automated lint/test/build pipeline. No release workflow, no binary distribution.
- **No release packaging**: build-from-source only. No `cargo install` publication, no pre-built binaries, no Homebrew formula, no Docker image.
- **No sample data**: no example `.hprof` files for testing, tutorials, or development. Contributors cannot exercise the tool without generating their own dumps.
- **Documentation claims outpace implementation**: README describes "retained size", "dominator tree computation", "AI-generated code fixes", and progress bars that don't exist. QUICKSTART shows example output that the tool cannot actually produce today.
- **Graph module is misleading**: `summarize_graph()` builds a synthetic tree from top-12 entries, not a real dominator tree. The name and output suggest more than is delivered.
- **Leak detection has no ground truth**: without retained sizes, leak candidates are ranked by aggregate class sizes from record tags. This misses cases entirely (e.g., a small class holding a reference chain to a large byte array).

### Maturity Assessment

| Subsystem | Maturity | Rationale |
|---|---|---|
| Parser | Alpha | Functional for record-level stats. Missing object-level parsing needed for real analysis. |
| Leak detection | Alpha | Heuristic-only. Useful for surface-level signals but not graph-backed or retained-size aware. |
| Graph / Dominator | Pre-alpha | Preview visualization only. Not a real dominator tree. Would mislead users who expect MAT-like results. |
| AI | Pre-alpha | Fully stubbed. Returns deterministic text. Not wired to any model. |
| GC root paths | Alpha | Real parsing of roots/instances within budget. Best-effort with fallback. Among the strongest features. |
| Fix suggestions | Alpha | Template-based scaffolding. No code analysis or AI involvement. |
| Source mapping | Alpha | Works for basic cases. No IDE integration beyond file scanning. |
| Reporting | Beta | 5 formats, XSS hardening, provenance rendering, well-tested. Ready for use. |
| MCP server | Alpha | Wired and functional but outputs depend on stubs/heuristics. |
| Config | Beta | Clean hierarchy, env + TOML + CLI. Production-ready pattern. |
| Provenance | Beta | Unique, well-integrated across all surfaces. Novel in the space. |
| Testing | Pre-alpha | ~12 unit tests. No integration, E2E, or fixture coverage. |
| CI/CD | Not started | No pipeline, no automation, no release workflow. |

---

## Section 3 — Gap Analysis

### 3.1 Correctness & Trust Gaps

**No object reference graph.** The HPROF parser in `core::heap::parse_heap()` reads record headers (tag byte + length) and counts INSTANCE_DUMP, OBJ_ARRAY_DUMP, etc. occurrences but does **not** parse instance field values, object references within instances, class field descriptors (LOAD_CLASS → CLASS_DUMP field sections), or string table entries (STRING_IN_UTF8). Without this data:

- **No retained-size computation.** Retained size requires knowing which objects are exclusively reachable through a given object. This requires a full reference graph and dominator tree.
- **No real dominator tree.** The `graph::summarize_graph()` function builds a synthetic tree from the top 12 entries. Real dominator analysis requires Lengauer-Tarjan (or equivalent) over the full object reference graph. `petgraph::algo::dominators::simple_fast` is already in the dependency tree and could be applied — but only after the graph exists.
- **Leak detection is heuristic-only.** Current leak candidates are classes with high aggregate sizes from record-tag histograms. A class with 100K small instances that each hold a reference to a large `byte[]` will not be flagged correctly. MAT-style leak suspects (objects whose retained size is disproportionate to their shallow size) are impossible without retained sizes.
- **GC paths are best-effort.** The `gc_path` module parses real instances within budget (`DEFAULT_MAX_INSTANCES=32768`), which means paths through larger graphs will miss edges or fall back to synthetic data. With a full object graph, paths would be exact.
- **Diff is record-level, not object-level.** `diff_heaps()` compares aggregate record/class statistics between two snapshots. It cannot track individual object migration, new allocation sites, or reference chain changes.

**Provenance partially compensates** — the system correctly labels synthetic/fallback data — but the fundamental analysis results are not trustworthy for production incident response.

### 3.2 Testing & CI Gaps

- **~12 unit tests** across the core crate. Tests cover provenance rendering (text, HTML, TOON), escape functions, and a few analysis paths.
- **Zero integration tests.** No tests parse a real `.hprof` file end-to-end. No tests exercise the CLI binary. No tests verify MCP request/response cycles.
- **Zero end-to-end tests.** No tests run `mnemosyne parse <file>` and validate stdout output.
- **No CI pipeline.** No GitHub Actions workflow. No automated `cargo test`, `cargo clippy`, `cargo fmt --check`, or `cargo audit` on pull requests.
- **No test fixtures.** No sample `.hprof` files in the repository. Generating valid HPROF files requires a JVM, which complicates contributor onboarding. Small synthetic HPROF files could be generated programmatically.
- **No coverage tracking.** No `cargo-tarpaulin` or `cargo-llvm-cov` integration. Unknown actual coverage percentage.
- **No property-based testing.** Parser binary handling is a prime candidate for `proptest` or `quickcheck` fuzzing.
- **No benchmarks.** No `criterion` benchmarks for parser throughput, graph construction, or report rendering. Cannot track performance regressions.

### 3.3 Documentation & Onboarding Gaps

- **README claims outpace implementation.** "Advanced object graph & dominator analysis", "AI-generated explanations", "Memory-mapped I/O (zero-copy parsing)", and progress bar output are described but not implemented.
- **QUICKSTART shows aspirational output.** Example output includes progress bars (`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100% • 3.2s`) and "Retained Size" values that the tool cannot produce.
- **No API documentation.** `docs/api.md` exists but is a placeholder. `core` module public API has doc-comments on some items but no generated rustdoc site.
- **No tutorial or cookbook.** No guided walkthrough of a real analysis session. No examples of interpreting output or acting on leak candidates.
- **No troubleshooting guide.** No documentation for common errors, unsupported HPROF variants, or limitations.
- **Architecture doc is partially stale.** Describes "Current Implementation Snapshot (March 2026)" but still references November 2025 status callouts in component sections.

### 3.4 Packaging & Release Gaps

- **Build from source only.** No published crate on crates.io, no pre-built binaries.
- **No `cargo install` support.** The `cli` crate doesn't publish a binary target to crates.io.
- **No release binaries.** No GitHub Releases, no CI-built artifacts for Linux/macOS/Windows.
- **No Homebrew formula.** No tap for macOS users.
- **No Docker image.** No container for CI/CD pipeline integration or incident response toolkits.
- **No version tags.** Git history has no version tags. `Cargo.toml` shows `0.1.0` but no tagged release.
- **No changelog automation.** `CHANGELOG.md` exists but is manually maintained with a single `[Unreleased]` section.

### 3.5 Feature Parity Gaps vs Eclipse MAT

Eclipse MAT is the de-facto standard for JVM heap analysis. Mnemosyne is missing essentially **all core MAT analysis features**:

- **No dominator tree**: MAT's primary analysis view. Requires full object graph.
- **No retained sizes**: MAT computes retained sizes for every object. The foundation for all accurate analysis.
- **No OQL**: MAT provides Object Query Language for ad-hoc heap exploration.
- **No thread inspection**: MAT links thread stack traces to retained objects.
- **No classloader analysis**: MAT detects classloader leaks by analyzing the classloader hierarchy.
- **No collection inspection**: MAT inspects `HashMap`, `ArrayList`, etc. fill ratios and waste.
- **No unreachable object reporting**: MAT identifies objects not reachable from any GC root.
- **No histogram grouping**: MAT groups histograms by package, classloader, or superclass.
- **No object-level comparison**: MAT diffs two dumps at object/class granularity.

The gap is substantial. However, Mnemosyne does not need full MAT parity to be useful — it needs to nail the top 3-4 MAT features (dominator tree, retained sizes, leak suspects, GC paths) and combine them with capabilities MAT lacks (AI, provenance, MCP, CI automation).

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
| Dominator tree | ❌ Missing | No object graph → no dominators | Build object reference graph from HPROF INSTANCE_DUMP/OBJ_ARRAY_DUMP field data, then run Lengauer-Tarjan | Very High | Critical | M1 |
| Retained size | ❌ Missing | No retained-size computation | Compute from dominator tree using accumulated subtree sizes | High | Critical | M1 |
| Object graph traversal | ❌ Missing | Can't browse individual objects | Build indexed object store during parsing, expose navigation API | Very High | Critical | M1 |
| Shortest path to GC roots | ⚠️ Partial | Budget-limited, best-effort within 32K instances | Improve with full object graph; use BFS from target to any GC root | Medium | High | M1 |
| Leak suspects report | ⚠️ Partial | Heuristic-only, not graph-backed | MAT-style: find objects with disproportionate retained size relative to shallow size | High | Critical | M3 |
| Histogram by class/package/classloader | ⚠️ Partial | Record-level histogram only, no classloader or package grouping | Parse per-object data, group by fully-qualified class name, classloader, package | Medium | High | M3 |
| OQL / query language | ❌ Missing | No query capability | Design mini-query language or embed existing (e.g., SQL-like over object model) | Very High | High | M3 |
| Thread inspection | ❌ Missing | Not implemented | Parse HPROF STACK_TRACE + STACK_FRAME records, link threads to retained objects | High | Medium | M3 |
| ClassLoader analysis | ❌ Missing | Not implemented | Parse classloader hierarchy from CLASS_DUMP records, detect leaks per classloader | High | Medium | M3 |
| Collection inspection | ❌ Missing | Not implemented | Detect known collection types (`HashMap`, `ArrayList`, etc.), inspect fill ratio, size, waste | Medium | Medium | M3 |
| Export / reporting | ✅ Implemented | Good for current scope | Already strong: 5 formats, provenance, XSS hardening. Add CSV, protobuf, flamegraph later | Low | Medium | M2 |
| UI-based exploration | ❌ Missing | CLI only | Phase from TUI → static HTML → web UI → full explorer | Very High | High | M4 |
| Large dump performance | ⚠️ Partial | Streaming works but no object graph storage yet | Optimize object graph to use memory-mapped storage or chunked processing for multi-GB dumps | High | High | M1 |
| Heap snapshot comparison | ⚠️ Partial | Record-level diff only | Diff at object/class level once object graph exists | Medium | Medium | M3 |
| Unreachable objects | ❌ Missing | Not implemented | After building reachability from GC roots, report unreachable set and sizes | Medium | Medium | M3 |

### Detailed Analysis per Feature

**Dominator Tree.**
*Current Status:* `core::graph::summarize_graph()` builds a petgraph from the top 12 class/record entries and computes dominators over this miniature tree. This is a **visualization aid**, not a real dominator tree.
*Gap:* A real dominator tree requires a complete object reference graph where nodes are heap objects and edges are reference fields. The parser does not extract this data today.
*Recommended Approach:* Extend `core::heap` to parse INSTANCE_DUMP field values (using CLASS_DUMP field descriptors to know which fields are references), OBJ_ARRAY_DUMP elements, and build an adjacency list. Then apply `petgraph::algo::dominators::simple_fast` (Lengauer-Tarjan) over the full graph with a virtual GC root node connected to all GC roots.
*Milestone:* M1 — this is the foundational piece everything else depends on.

**Retained Size.**
*Current Status:* Not computed. `LeakInsight.retained_size_bytes` is populated with aggregate class-level byte counts from record tags, not actual retained sizes.
*Gap:* Retained size = sum of shallow sizes of all objects in the subtree rooted at a node in the dominator tree. Requires the dominator tree.
*Recommended Approach:* Post-order traversal of the dominator tree, accumulating shallow sizes upward. Store retained size on each node. Expose via `HeapSummary` or a new `ObjectDetail` type.
*Milestone:* M1 — directly depends on dominator tree.

**Object Graph Traversal.**
*Current Status:* No object-level data model. The parser produces aggregate `ClassStat` and `RecordStat` entries.
*Gap:* Users cannot inspect individual objects, explore reference chains, or understand why a specific object is retained.
*Recommended Approach:* During parsing, build an indexed object store (HashMap or arena-allocated) mapping object IDs to their class, shallow size, and outgoing references. Expose a navigation API with `get_object(id)`, `get_referrers(id)`, `get_references(id)`.
*Milestone:* M1 — co-delivered with the object graph.

**Shortest Path to GC Roots.**
*Current Status:* `core::gc_path` is genuinely well-implemented for its scope. Parses 15 GC root types, CLASS_DUMP, INSTANCE_DUMP, builds edges within budget limits (32K instances, 8K roots). BFS from roots. Falls back to synthetic path with provenance marker.
*Gap:* Budget limits mean paths through large graphs may be incomplete. With a full object graph, paths would be exact and unlimited.
*Recommended Approach:* Once the object graph exists, rewrite path-finding as BFS over the complete graph. Keep the budget-limited mode as a fast-path option. Remove synthetic fallback for the graph-backed path.
*Milestone:* M1 — improves after object graph lands.

**Leak Suspects Report.**
*Current Status:* `detect_leaks()` produces candidates from class histograms ranked by aggregate size. No retained-size signal.
*Gap:* MAT's leak suspect report finds objects whose retained size is disproportionately large relative to their shallow size — indicating they hold large reference chains. This is impossible without retained sizes.
*Recommended Approach:* After M1 delivers retained sizes, implement a leak suspect algorithm: (1) find objects where retained_size >> shallow_size, (2) identify accumulation points (objects holding many references to the same class), (3) generate suspect reports with reference chain context.
*Milestone:* M3 — depends on M1 retained sizes.

**Histogram by Class/Package/ClassLoader.**
*Current Status:* `HeapSummary.classes` contains `ClassStat` entries derived from record tags. No classloader or package-level grouping.
*Gap:* MAT groups histograms by package prefix, classloader identity, and superclass — enabling users to quickly scope analysis to their own code.
*Recommended Approach:* With per-object data from M1, group by FQN prefix (package), by classloader object ID (from CLASS_DUMP), and by superclass chain. Expose as query parameters on the histogram API.
*Milestone:* M3 — depends on M1 object graph for classloader data.

**OQL / Query Language.**
*Current Status:* Not implemented. No query capability of any kind.
*Gap:* MAT's OQL allows `SELECT * FROM java.lang.String WHERE toString().length() > 1000` style queries. Extremely powerful for ad-hoc investigation.
*Recommended Approach:* Design a minimal query language (e.g., `SELECT class, retained_size FROM objects WHERE class LIKE 'com.example.%' AND retained_size > 1MB ORDER BY retained_size DESC`). Implement as a parser → AST → evaluator over the object store. Start with class/size filters, then expand to field access and predicates.
*Milestone:* M3 — requires M1 object graph and M3 histogram improvements as prerequisites.

**Thread Inspection.**
*Current Status:* Not implemented. HPROF STACK_TRACE and STACK_FRAME records are skipped during parsing.
*Gap:* MAT links threads to their retained objects, showing which threads hold memory and through what call stack.
*Recommended Approach:* Parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT records. Link thread objects to their stack traces and to objects reachable from thread-local roots.
*Milestone:* M3 — depends on M1 object graph for object-to-thread linkage.

**ClassLoader Analysis.**
*Current Status:* Not implemented. CLASS_DUMP records are partially parsed in `gc_path` but classloader IDs are not stored or analyzed.
*Gap:* ClassLoader leaks (common in application servers and OSGi containers) cannot be detected without tracking the classloader hierarchy.
*Recommended Approach:* During CLASS_DUMP parsing, record the classloader reference for each class. Build a classloader tree. Detect leaks by finding classloaders that retain surprising amounts of memory.
*Milestone:* M3 — depends on M1 object graph.

**Collection Inspection.**
*Current Status:* Not implemented.
*Gap:* MAT detects under-utilized collections (e.g., `HashMap` with 16 buckets and 1 entry, or `ArrayList` with capacity 1000 and 2 elements). These waste significant memory at scale.
*Recommended Approach:* Identify known collection class names during object graph traversal. Inspect internal fields (e.g., `HashMap.table.length` vs `HashMap.size`) to compute fill ratio. Report collections with low fill ratios or excessive capacity.
*Milestone:* M3 — depends on M1 object graph and field-value parsing.

**Large Dump Performance.**
*Current Status:* The streaming parser handles arbitrarily large files at the record level. But building a full object graph for a multi-GB dump may require more memory than is available.
*Gap:* A 4GB heap dump may contain 50M+ objects, each with multiple references. An in-memory adjacency list could require 10-20GB of RAM.
*Recommended Approach:* (1) Start with in-memory graph for dumps up to ~1GB. (2) For larger dumps, implement a two-pass strategy: first pass indexes object positions, second pass resolves references using memory-mapped access. (3) Consider disk-backed storage (e.g., `sled` or `redb` for an on-disk object store) for very large dumps.
*Milestone:* M1 (basic) → M3 (optimized for large dumps).

**Heap Snapshot Comparison.**
*Current Status:* `diff_heaps()` compares two `HeapSummary` values at the record/class-stat level. Reports delta bytes, delta objects, and changed classes.
*Gap:* MAT can diff at the object level, showing new objects, freed objects, and reference chain changes between snapshots.
*Recommended Approach:* With M1 object graphs, diff object sets by class and ID. Identify newly allocated objects, freed objects, and changed reference patterns. Report delta retained sizes per class.
*Milestone:* M3 — depends on M1 object graph.

**Unreachable Objects.**
*Current Status:* Not implemented.
*Gap:* MAT reports objects not reachable from any GC root, which helps understand phantom memory and finalizer pressure.
*Recommended Approach:* After building the object graph and GC root set, mark all reachable objects via BFS/DFS. Report unmarked objects with their classes and sizes.
*Milestone:* M3 — depends on M1 object graph and GC root parsing.

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

**Why it matters:** Without a real object graph and retained sizes, Mnemosyne cannot make credible claims about memory analysis. This is the single biggest gap between current state and a useful tool.

**Key Deliverables:**
1. Full object graph parser — parse INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP records into an indexed object store with reference edges
2. Retained size computation — compute retained sizes via dominator tree subtree accumulation
3. Real dominator tree — implement Lengauer-Tarjan or Cooper et al. algorithm on the object graph
4. Upgrade leak detection — back LeakInsight with retained-size data instead of heuristics only
5. Upgrade GC path finder — use full object graph for accurate shortest-path computation
6. Comprehensive test suite — unit tests for each module, integration tests with sample HPROF fixtures
7. Sample HPROF test fixtures — small, well-known dumps for deterministic testing
8. CI pipeline — GitHub Actions for build + test + clippy + fmt

**Dependencies:** None (this is the foundation)

**Modules/files affected:** `core/src/heap.rs`, `core/src/graph.rs`, `core/src/analysis.rs`, `core/src/gc_path.rs`, new test fixtures, `.github/workflows/`

**Complexity:** Very High — this is the hardest milestone with the most new code. The object graph implementation alone is substantial.

**Implementation order within milestone:**
1. Sample HPROF fixtures (unblocks everything else)
2. Object graph parser (extend heap.rs)
3. Dominator tree algorithm (extend graph.rs)
4. Retained size computation (extend graph.rs)
5. Upgrade leak detection (update analysis.rs)
6. Upgrade GC path (update gc_path.rs)
7. Integration tests
8. CI pipeline

**Definition of done:**
- Can parse a real HPROF dump into a full object graph with reference edges
- Can compute retained sizes for any object
- Can produce a real dominator tree
- Leak detection uses retained-size data when available
- GC path uses full object graph when available
- At least 50 unit tests + 5 integration tests pass
- CI runs on every PR

---

### Milestone 2 — Packaging, Releases, and DX
**Objective:** Make Mnemosyne easy to install, use, and contribute to.

**Why it matters:** No one adopts a tool they can't easily install. Developer experience is the gateway to open-source adoption.

**Key Deliverables:**
1. GitHub Actions CI/CD — build, test, clippy, fmt, release automation
2. Release binaries — Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
3. `cargo install mnemosyne-cli` support
4. Homebrew formula
5. Docker image
6. CLI UX improvements — progress bars (indicatif), colored output, better error messages
7. Versioned releases with changelog automation
8. Updated install/quickstart docs
9. Issue templates and PR template

**Dependencies:** M1 CI pipeline

**Modules/files affected:** `.github/workflows/`, `Cargo.toml`, `cli/`, `docs/`, `.github/ISSUE_TEMPLATE/`, `Dockerfile`

**Complexity:** Medium

**Implementation order:**
1. CI pipeline polish (extend from M1)
2. Release automation (cross-compile + publish)
3. cargo install setup
4. CLI UX (progress bars, colors, errors)
5. Homebrew formula
6. Docker image
7. Issue/PR templates
8. Documentation updates

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

**Dependencies:** M1 (object graph, retained sizes, dominator tree)

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

**Dependencies:** M1 (meaningful data to send to AI), M3 (richer analysis context)

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

#### M1-B1: Sample HPROF Test Fixtures
- **Goal:** Create small, deterministic HPROF files that exercise all record types needed for testing
- **Files/modules affected:** new `resources/test-fixtures/` directory, new fixture generation script or Rust helper
- **Expected agent owner:** Implementation Agent
- **Validation:** Fixtures parse successfully with current parser; file sizes are small (<1MB each)
- **Risk notes:** Need to ensure fixtures cover INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP, CLASS_DUMP, GC root records
- **Non-scope:** Do not modify existing parser behavior; do not touch report or CLI code

#### M1-B2: Object Graph Parser — Data Structures
- **Goal:** Define the indexed object store, reference edge types, and class metadata structures needed to represent a full object graph
- **Files/modules affected:** `core/src/heap.rs` (new types), possibly new `core/src/object_graph.rs`
- **Expected agent owner:** Implementation Agent (with Architecture Review)
- **Validation:** Types compile; existing tests pass; no regressions
- **Risk notes:** API surface design is critical — get it right or downstream work suffers. Consider memory efficiency for large dumps.
- **Non-scope:** Do not implement parsing logic yet; do not change existing HeapSummary or ClassStat

#### M1-B3: Object Graph Parser — HPROF Parsing
- **Goal:** Extend the streaming parser to read INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP, CLASS_DUMP records and populate the object graph data structures
- **Files/modules affected:** `core/src/heap.rs`
- **Expected agent owner:** Implementation Agent
- **Validation:** Can parse test fixtures into a populated object graph; unit tests verify correct object count, reference count, class assignments
- **Risk notes:** Major complexity — HPROF binary format has intricate field layouts. Memory usage must stay reasonable for large dumps.
- **Non-scope:** Do not implement dominator tree or retained sizes yet; do not change CLI/MCP interfaces

#### M1-B4: Dominator Tree Algorithm
- **Goal:** Implement a dominator tree algorithm (Lengauer-Tarjan or Cooper et al.) on the object graph using petgraph
- **Files/modules affected:** `core/src/graph.rs`
- **Expected agent owner:** Implementation Agent
- **Validation:** Dominator tree matches known-correct results for test fixtures; unit tests verify parent-child relationships and dominance frontiers
- **Risk notes:** Algorithmic complexity — need to handle cycles, multiple GC roots (virtual super-root). Performance critical for large graphs.
- **Non-scope:** Do not change report formatting or CLI output yet

#### M1-B5: Retained Size Computation
- **Goal:** Compute retained sizes by accumulating subtree sizes in the dominator tree
- **Files/modules affected:** `core/src/graph.rs`, `core/src/analysis.rs` (wire into LeakInsight)
- **Expected agent owner:** Implementation Agent
- **Validation:** Retained sizes match expected values for test fixtures; sum of retained sizes under root equals total heap; unit tests
- **Risk notes:** Must handle shared objects correctly (retained by domination, not just reachability)
- **Non-scope:** Do not change report or CLI formatting yet

#### M1-B6: Wire Graph Into Analysis Pipeline
- **Goal:** Update analyze_heap, detect_leaks, and reporting to use real object graph + retained sizes when available, with provenance fallback to current heuristics when not
- **Files/modules affected:** `core/src/analysis.rs`, `core/src/report.rs`, `core/src/gc_path.rs`
- **Expected agent owner:** Implementation Agent
- **Validation:** analyze output changes when parsing a fixture with full object graph; provenance markers correctly label graph-backed vs heuristic results; all existing tests still pass
- **Risk notes:** Must maintain backward compatibility — if object graph parsing fails or file is too large, fall back gracefully to current heuristics
- **Non-scope:** Do not change CLI interface or MCP protocol

#### M1-B7: Integration Tests + CI Pipeline
- **Goal:** Create integration tests that run full CLI commands against test fixtures and verify outputs; set up GitHub Actions CI
- **Files/modules affected:** `tests/` directory (new), `.github/workflows/ci.yml` (new)
- **Expected agent owner:** Testing Agent (tests), Implementation Agent (CI)
- **Validation:** CI passes on push and PR; integration tests cover parse, leaks, analyze, gc-path, diff; at least 50 total tests
- **Risk notes:** CI may need Rust toolchain caching for speed
- **Non-scope:** Do not add release automation yet (that's M2)

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

| # | Item | Priority | Impact | Effort | Dependencies | Milestone |
|---|---|---|---|---|---|---|
| 1 | Object graph parser | P0 | High | XL | None | M1 |
| 2 | Dominator tree algorithm | P0 | High | L | Object graph | M1 |
| 3 | Retained size computation | P0 | High | M | Dominator tree | M1 |
| 4 | Sample HPROF test fixtures | P0 | High | M | None | M1 |
| 5 | CI pipeline (GitHub Actions) | P0 | High | M | None | M1 |
| 6 | Wire graph into analysis pipeline | P0 | High | L | Object graph + retained sizes | M1 |
| 7 | Integration tests | P0 | High | L | Test fixtures + CI | M1 |
| 8 | Release binaries | P1 | High | M | CI pipeline | M2 |
| 9 | cargo install support | P1 | High | S | Release setup | M2 |
| 10 | CLI progress bars + colors | P1 | Medium | S | None | M2 |
| 11 | MAT-style leak suspects | P1 | High | L | Retained sizes | M3 |
| 12 | Histogram by class/package/classloader | P1 | High | M | Object graph | M3 |
| 13 | Homebrew formula | P1 | Medium | S | Release binaries | M2 |
| 14 | LLM integration (real API calls) | P1 | High | L | Meaningful data (M1) | M5 |
| 15 | Enhanced heap diff | P1 | Medium | M | Object graph | M3 |
| 16 | Static interactive HTML reports | P2 | High | L | Reporting exists | M4 |
| 17 | OQL query engine | P2 | High | XL | Object graph | M3 |
| 18 | Thread inspection | P2 | Medium | L | Object graph | M3 |
| 19 | ClassLoader analysis | P2 | Medium | L | Object graph | M3 |
| 20 | Local web UI | P2 | High | XL | HTML reports | M4 |
| 21 | Collection inspection | P2 | Medium | M | Object graph | M3 |
| 22 | Unreachable objects | P2 | Medium | M | Object graph | M3 |
| 23 | Configurable prompt/task runner | P2 | Medium | L | LLM integration | M5 |
| 24 | AI conversation mode | P2 | Medium | L | LLM integration | M5 |
| 25 | Docker image | P2 | Medium | S | Release automation | M2 |
| 26 | Example projects + sample dumps | P2 | Medium | M | Test fixtures | M6 |
| 27 | Benchmark suite | P2 | Medium | M | Object graph | M6 |
| 28 | Plugin/extension system | P3 | Medium | XL | Stable APIs (M3+) | M6 |
| 29 | Full interactive heap browser | P3 | High | XL | Web UI + OQL | M4 |
| 30 | Local LLM support | P3 | Medium | L | LLM integration | M5 |

---

## Section 11 — Recommended Immediate Next Steps

Execute these batches in order:

### Step 1: M1-B1 — Sample HPROF Test Fixtures
**Why first:** Everything else needs test data. Can't validate any analysis improvement without known-good heap dumps.
**Owner:** Implementation Agent
**Effort:** Medium

### Step 2: M1-B7 (partial) — CI Pipeline Setup
**Why now:** Get CI running early so all subsequent work is validated automatically.
**Owner:** Implementation Agent
**Effort:** Medium

### Step 3: M1-B2 — Object Graph Data Structures
**Why next:** Define the types before implementing the parser. Get Architecture Review feedback on the API surface.
**Owner:** Implementation Agent (with Architecture Review)
**Effort:** Medium

### Step 4: M1-B3 — Object Graph HPROF Parsing
**Why next:** The single most impactful piece of work. Unlocks retained sizes, dominators, and all of M3.
**Owner:** Implementation Agent
**Effort:** Extra Large

### Step 5: M1-B4 — Dominator Tree Algorithm
**Why next:** Directly depends on object graph. Unlocks retained sizes.
**Owner:** Implementation Agent
**Effort:** Large

### Step 6: M1-B5 — Retained Size Computation
**Why next:** Unlocks real leak detection quality.
**Owner:** Implementation Agent
**Effort:** Medium

### Step 7: M1-B6 — Wire Graph Into Analysis Pipeline
**Why next:** Make all the new data flow into existing outputs. Users see the improvement.
**Owner:** Implementation Agent
**Effort:** Large

### Step 8: M1-B7 (complete) — Integration Tests
**Why next:** Lock in all M1 behavior with comprehensive tests.
**Owner:** Testing Agent
**Effort:** Large

### Step 9: M2-B1 — CLI UX Improvements
**Why next:** Quick win for developer experience. Low risk.
**Owner:** Implementation Agent
**Effort:** Small

### Step 10: M2-B2 — Release Automation
**Why next:** Get binaries out so people can actually use the tool.
**Owner:** Implementation Agent
**Effort:** Medium

---

*This roadmap is a living document. Update it after each major batch completion.*
*Next review: after Milestone 1 completion.*
