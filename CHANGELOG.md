# Changelog

All notable changes to Mnemosyne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Graph-backed investigation analyzers for thread inspection, string analysis, collection inspection, and top-N largest instances in `core::analysis`
- Retained instance field bytes, selective `byte[]` / `char[]` payload retention, parsed `STACK_TRACE` / `STACK_FRAME` records, and typed field readers (`FieldValue`, `read_field()`, `read_all_fields()`) in `core::hprof::object_graph`
- CLI `analyze` flags `--threads`, `--strings`, `--collections`, `--top-instances`, `--top-n`, and `--min-capacity`, plus text-mode rendering for the new analyzer sections
- `ParseOptions` with `retain_field_data`, plus `parse_hprof_file_with_options()` and `parse_hprof_with_options()` in `core::hprof::binary_parser`
- Enhanced `scripts/measure_rss.sh` with multi-command profiling, `/proc/PID/status` VmHWM fallback sampling, automatic RSS:dump ratio computation, and pass/warn/fail markers
- Additional integration coverage for zero-leak confirmation and M3 Phase 2 analyze flags, bringing the validated workspace total to 129 tests

### Changed
- `AnalyzeRequest` and `AnalyzeResponse` now carry optional investigation-feature enable flags and result sections for thread, string, collection, and top-instance reports
- `core::fix::generator` and `core::mcp::server` now construct the expanded `AnalyzeRequest` contract used by the shared analysis engine
- Default binary HPROF parsing no longer retains instance `field_data` or primitive `byte[]` / `char[]` content unless callers opt in through `ParseOptions { retain_field_data: true }`
- `analyze_heap()` now enables field-data retention only when string, collection, or thread analysis is requested; default `analyze`, `leaks`, and `gc-path` runs now use the lean parser path

### Fixed
- `mnemosyne leaks` now prints `No leak suspects detected.` when analysis completes without any leak candidates instead of exiting successfully with silent output
- Collection oversized-threshold handling now uses inclusive `<= 0.25` fill ratio instead of strict `< 0.25`
- String analysis now reports logical character count for `char[]`-backed strings instead of raw UTF-16 byte count

## [0.2.0] - 2026-03-08

### Added
- `validate_leak_id()` for strict leak-ID matching; `explain`, `fix`, and MCP `explain_leak` now return errors for unknown or invalid leak IDs instead of silently falling back
- `build_segment_fixture()` test fixture builder for `HEAP_DUMP_SEGMENT` (tag `0x1C`) coverage
- 14 new tests (5 in M1.5-B1, 9 in M1.5-B2), bringing the workspace total to 101 tests including 30 CLI integration tests
- Real-world HPROF integration tests validating the `parse`, `analyze`, `leaks`, and `gc-path` pipeline end to end against actual JVM heap dumps
- Heuristic fallback validation test confirming `synthesize_leaks()` still produces candidates when graph-backed analysis results are filtered away
- `core::hprof::tags` as the shared source of truth for HPROF top-level record tags, heap-dump sub-record tags, and `tag_name()` mappings
- Criterion benchmark targets in `core/benches/` covering parser throughput, object-graph construction, and dominator-tree computation
- Initial `scripts/measure_rss.sh` support for max-RSS capture during CLI parse runs, plus `docs/design/memory-scaling.md` as the memory-scaling decision template
- Graph-backed histogram grouping by class, package, and classloader via `HistogramEntry`, `HistogramGroupBy`, and `HistogramResult`
- MAT-style leak suspect ranking via `LeakSuspect`, including retained/shallow ratio, accumulation-point detection, short reference-chain context, and composite score-based ordering
- Unreachable-object summaries via `UnreachableSet` and `UnreachableClassEntry`, with per-class counts and shallow-size totals
- Class-level heap diff output via optional `HeapDiff::class_diff` / `ClassLevelDelta` plus CLI table rendering
- `AnalysisConfig.accumulation_threshold` and CLI `analyze --group-by class|package|classloader`
- 9 new tests for histogram grouping, suspect scoring, unreachable objects, and enhanced diff, bringing the workspace total to 110 passing tests
- Published first benchmark baseline with Criterion throughput data and RSS measurements in `docs/performance/memory-scaling.md`; updated `docs/design/memory-scaling.md` with real measured data and architectural decision

### Changed
- `explain --leak-id`, `fix --leak-id`, and MCP `explain_leak` with `leak_id` now fail fast with a descriptive error when the specified leak identifier does not match any detected leak, instead of silently falling back to the full leak set
- `core::hprof::parser`, `core::hprof::binary_parser`, `core::hprof::test_fixtures`, and `core::graph::gc_path` now import shared HPROF tag constants instead of maintaining duplicated local values
- `analyze_heap()` now attaches optional histogram and unreachable-object sections, and `LeakInsight` gained optional `shallow_size_bytes` / `suspect_score` fields with `skip_serializing_if` for backward compatibility
- `diff_heaps()` now preserves the existing record-level diff and adds graph-backed class-level deltas when both snapshots parse into object graphs

### Fixed
- **Critical HPROF tag-constant bug:** corrected `TAG_HEAP_DUMP_SEGMENT` from `0x0D` to `0x1C` across `binary_parser.rs`, `parser.rs`, and `gc_path.rs`; real-world JVM heap dumps using `HEAP_DUMP_SEGMENT` records are now parsed correctly
- Corrected `tag_name()` mappings: `0x0D` → `CPU_SAMPLES`, `0x0E` → `CONTROL_SETTINGS`, `0x1C` → `HEAP_DUMP_SEGMENT`, `0x2C` → `HEAP_DUMP_END`
- Preserved incoming field labels when reconstructing GC paths from `ObjectGraph` BFS parents, fixing the edge-label regression uncovered during tag-centralization work

## [0.1.1] - 2026-03-08

### Changed
- **Core module restructure:** Reorganized flat `core/src/` layout into grouped module directories (`hprof/`, `graph/`, `analysis/`, `mapper/`, `report/`, `fix/`, `mcp/`). Public API re-exports preserved for backward compatibility.
- `heap` module renamed to `hprof::parser`; HPROF binary parser, object graph model, and test fixtures grouped under `hprof/`.
- `dominator`, `gc_path`, and graph metrics grouped under `graph/`.
- `analysis` engine and AI insights grouped under `analysis/`.
- Fix generation, code mapping, report rendering, and MCP server each have their own module directory.
- CLI imports updated to use new module paths (`hprof::`, `graph::`).

## [0.1.0] - 2026-03-08

### Added
- Comfy-table-based aligned terminal tables for `mnemosyne parse` summary sections and `mnemosyne leaks`, plus follow-up disclosure sections that print full record-category names, leak IDs, and leak class names when table cells are width-bounded
- Structured error handling with contextual hints for missing heap dumps, common non-HPROF inputs (`.jar`, `.class`, `.log`, `.txt`, `.csv`), HPROF header parse failures, and invalid config files; the CLI now prints colored `hint:` lines from `CoreError` suggestions
- Initial project structure
- Documentation (README, ARCHITECTURE, CONTRIBUTING)
- Copilot instructions for fun commit messages
- Architecture diagrams (SVG)
- GitHub Actions release automation in `.github/workflows/release.yml`: validate `v*` tags against `[workspace.package].version`, cross-compile `mnemosyne-cli` for x86_64 Linux, aarch64 Linux, x86_64 macOS, aarch64 macOS, and x86_64 Windows, package tar.gz/zip archives, and publish GitHub Releases with generated notes plus attached binaries
- Docker image distribution: multi-stage `Dockerfile` + `.dockerignore`, non-root `debian:bookworm-slim` runtime with `ENTRYPOINT ["mnemosyne-cli"]` and `WORKDIR /data`, plus a GHCR publish job in `.github/workflows/release.yml` that pushes `ghcr.io/<owner>/mnemosyne` with `<version>`, `<major>.<minor>`, and `latest` tags on tagged releases
- Homebrew formula in `HomebrewFormula/mnemosyne.rb` for macOS release archives, with Intel/Apple Silicon selection via `Hardware::CPU.arm?` and release SHA placeholders to fill on the first tagged release
- Rust workspace scaffolding (`mnemosyne-core` + `mnemosyne-cli`) with stub CLI commands and core APIs
- Basic HPROF header parsing with CLI wiring for `parse`, `leaks`, and `analyze`
- Record-level HPROF scanning with CLI summaries (top tags, record counts, heuristics-driven leak severity)
- Functional MCP stdio server that handles `parse_heap` and `detect_leaks` requests
- Graph module with synthetic dominator summaries included in analysis reports
- Source mapping module with `mnemosyne map` CLI command, MCP `map_to_code` handler, and leak identifiers surfaced in reports
- GC path tracing scaffolding with CLI `gc-path` subcommand and MCP `find_gc_path` endpoint
- AI Insights heuristics powering `--ai` analysis output with model/confidence metadata in CLI, reports, and JSON responses
- `mnemosyne explain` CLI command plus MCP `explain_leak` handler that reuse AI insights for targeted leak narratives
- `mnemosyne fix` CLI command and MCP `propose_fix` shim that craft placeholder patches in MINIMAL/DEFENSIVE/COMPREHENSIVE styles
- TOON (Token-Oriented Outline Notation) report format exposed via `--format toon`, replacing the former JSON output path for CI/CD integrations
- Config loader that reads `.mnemosyne.toml`, `$MNEMOSYNE_CONFIG`, and `--config` overrides (plus environment variables) so CLI/MCP surfaces share the same defaults
- `[analysis]` configuration now powers CLI defaults (severity, package filters, leak kinds) with matching `MNEMOSYNE_MIN_SEVERITY`, `MNEMOSYNE_PACKAGES`, and `MNEMOSYNE_LEAK_TYPES` environment overrides and updated docs
- `mnemosyne leaks/analyze/explain` accept a repeatable `--leak-kind` flag, leak synthesis emits one record per requested kind, and `min_severity` now drops lower-confidence candidates instead of merely renaming them
- `[analysis].packages` now flow through untouched: leak synthesis rotates through the entire list, and `--package` became repeatable (and comma-friendly) across `leaks`, `analyze`, and `explain`
- `mnemosyne serve` now honors the shared configuration loader, so MCP requests reuse the same `[analysis]`, AI, and parser defaults as the CLI (including `--config` and `$MNEMOSYNE_CONFIG` precedence)
- `mnemosyne diff` now parses both snapshots, reports delta size/object counts, and lists the largest class/record shifts with friendlier CLI output plus refreshed docs/examples
- Authentic GC root tracing: `core::gc_path` parses real roots/class dumps/instance dumps to build best-effort paths (with a graceful synthetic fallback) and comes with updated docs + fixtures
- Provenance system: `ProvenanceKind` enum (`Synthetic`, `Partial`, `Fallback`, `Placeholder`) and `ProvenanceMarker` struct integrated into `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`; synthetic paths, fix suggestions, and fallback data are labeled automatically
- Provenance rendering across all non-JSON report formats (Text, Markdown, HTML, TOON) with per-leak and response-level markers; three dedicated tests cover text, TOON, and HTML provenance output
- CLI provenance display: `leaks`, `gc-path`, and `fix` subcommands now surface provenance markers when present
- Output hardening: `escape_html()` prevents XSS in HTML reports; `escape_toon_value()` handles control characters and backslashes in TOON key-value output; two dedicated tests validate escaping behavior
- Clippy cleanup: resolved range-pattern warnings in `heap.rs` and iterator warning in `mapper.rs`
- Milestone 1 foundations: new `core::object_graph` module defines canonical heap-object, class, field, and GC-root types for upcoming retained-size and dominator work
- Synthetic HPROF fixture builders: new `core::test_fixtures` module plus `resources/test-fixtures/README.md` document deterministic heap shapes for parser/graph tests
- GitHub Actions CI workflow: `.github/workflows/ci.yml` now runs workspace `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and pull requests
- Binary HPROF object-graph parser: new `core::hprof_parser` reads strings, classes, roots, instances, and arrays into `core::object_graph`
- Real dominator tree support: new `core::dominator` computes immediate dominators, dominated children, and retained sizes from the object graph
- Graph-backed retained-size integration: `analysis::analyze_heap()` now prefers object-graph + dominator analysis, surfaces retained sizes in graph metrics/report output, and falls back with explicit provenance when full graph-backed results are unavailable
- Unified `detect_leaks()` onto the graph-backed path: attempts object-graph → dominator → retained-size analysis first, then falls back to heuristics with provenance markers
- GC path finder rewrite with triple fallback: `ObjectGraph` BFS → budget-limited `GcGraph` → synthetic path, with edge labels enriched by `get_field_names_for_class()`
- Object graph navigation API: `get_object(id)`, `get_references(id)`, and `get_referrers(id)` on `ObjectGraph` for programmatic heap exploration
- 16 CLI integration tests in `cli/tests/integration.rs` covering `parse`, `leaks`, `analyze`, `gc-path`, `diff`, `fix`, `report`, and `config` against synthetic HPROF fixtures
- `test-fixtures` cargo feature on `mnemosyne-core` so integration tests can import canonical HPROF fixture builders without inlining them
- Narrowed `test_fixtures` public API: only `build_simple_fixture()` and `build_graph_fixture()` remain externally visible; builders are `pub(crate)`

### Changed
- Unified maintainer identity to `bballer03` across package metadata and release-facing docs; GitHub repository and GHCR references remain on `bballer03`
- Clarified `mnemosyne parse` summary wording so the CLI now describes heap record-category aggregate bytes/share/entries instead of implying class-level retained-size semantics; added 4 CLI regressions for parse/leak table output and truncation disclosure, bringing the validated workspace total to 87 tests including 23 CLI integration tests
- Updated `docs/QUICKSTART.md` output examples to match the shipped table-based CLI presentation for `parse`, `leaks`, and `diff` commands; replaced aspirational progress bar and bullet-style examples with actual spinner messages, aligned ASCII tables, per-leak detail blocks, provenance markers, and inline diff output
- Updated `README.md` output examples for `parse` and `leaks` to match the shipped spinner-based table presentation; replaced aspirational progress bar and emoji-prefixed examples with actual CLI output format
- Updated `docs/roadmap.md` Section 11 recommended next steps to reflect completed M2 table output and doc passes; refreshed gap analysis sections (3.3, 3.6), M2 milestone status, backlog table, and added M2-B7 batch entry
- Added a workspace-level `description` field in `Cargo.toml` so release metadata is complete for packaged artifacts
- Completed crates.io packaging metadata across workspace/core/cli manifests by inheriting `description`, `readme`, `homepage`, `keywords`, and `categories`, and pinned the `mnemosyne-core` path dependency in `cli/Cargo.toml` to `version = "0.1.0"` for publish/install compatibility
