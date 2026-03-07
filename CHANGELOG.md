# Changelog

All notable changes to Mnemosyne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
