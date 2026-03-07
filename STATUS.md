# Functional Status

_Last updated: 2026-03-07_

This document captures where the current alpha build stands versus the roadmap described in `README.md` and `ARCHITECTURE.md`. Use it to see what already works, what is partially there, and which gaps remain before we can call the heap analyzer "functionally complete".

## Snapshot
- ✅ **HPROF parser** streams headers/record stats and produces a record-category histogram + heap summary without loading entire dumps into RAM.
- ✅ **Object-graph pipeline** now includes shared core types (`core::object_graph`) plus a binary HPROF parser (`core::hprof_parser`) that populates objects, classes, references, and GC roots.
- ✅ **CLI surface** (`parse`, `leaks`, `analyze`, `diff`, `map`, `fix`, `gc-path`, `serve`) all call into the shared core. `parse` summary sections and `leaks` output now render aligned terminal tables at the CLI boundary, with follow-up disclosure sections when width-bounded cells truncate record-category names, leak IDs, or class names.
- ✅ **Error handling** now uses structured `CoreError` variants plus CLI `hint:` lines for missing heap dumps, wrong file types, HPROF header parse failures, and invalid config files.
- ✅ **Leak detection** is now unified: `detect_leaks()` attempts object-graph → dominator → retained-size analysis first, then falls back to heuristics with explicit `ProvenanceKind::Fallback` markers.
- ✅ **Graph/dominator view** now supports real retained sizes in both `analyze_heap()` and `detect_leaks()`, with the lightweight summary preview retained only as fallback.
- ✅ **GC path finder** now prefers full `ObjectGraph` BFS first, then falls back to a budget-limited `GcGraph`, then synthetic paths when the heap lacks enough detail.
- ✅ **Object-graph navigation API** now exposes `get_object(id)`, `get_references(id)`, and `get_referrers(id)` for programmatic heap exploration.
- ⚠️ **AI insights** are deterministic stubs; the configurable LLM-backed task runner is still to be wired up.
- ✅ **Report/export** supports Text/Markdown/HTML/TOON/JSON with `--output-file`. HTML output is XSS-hardened; TOON values are properly escaped. Provenance markers are rendered in all non-JSON formats.
- ✅ **Provenance system** labels synthetic, partial, fallback, and placeholder data across analysis responses, leak insights, GC paths, and fix suggestions. CLI and report renderers surface these markers to consumers.
- ✅ **Output hardening** — HTML escaping prevents XSS in report output; TOON escaping handles control characters and backslashes correctly.
- ✅ **Development workflow** now includes GitHub Actions CI, the `test-fixtures` cargo feature for canonical fixture reuse, and 87 passing tests including 23 CLI integration tests.
- ✅ **Release automation** now validates `v*` tags against the workspace version, cross-compiles `mnemosyne-cli` for five targets, packages archives, publishes GitHub Releases with attached binaries, and builds/pushes the GHCR Docker image on tagged releases.
- ✅ **Packaging metadata + Homebrew scaffolding** now cover crates.io-ready workspace metadata across `Cargo.toml`, `core/Cargo.toml`, and `cli/Cargo.toml`, plus a macOS Homebrew formula for both Apple Silicon and Intel release archives.

## Capability Checklist
| Area | Status | Notes | Next Step |
| --- | --- | --- | --- |
| Parser streaming + histogram | ✅ | `core::heap` parses headers + record stats and derives record-category histograms for fast summary-level commands; CLI parse tables now label aggregate bytes/share/entries accurately and disclose full category names when truncation occurs. | Keep summary parsing aligned with the graph-backed path. |
| Object-graph foundation | ✅ | `core::object_graph` defines the canonical model and `core::hprof_parser` now populates it from binary HPROF records. | Reuse the same graph across more analysis surfaces. |
| Leak detection | ✅ | `detect_leaks()` now shares the graph-backed object-graph → dominator → retained-size path used by `analyze_heap()`, with heuristic fallback labeled via provenance markers. | Extend retained-size-backed suspect ranking into diffing and richer comparison flows. |
| Dominators / retained size | ✅ | `core::dominator` computes real retained sizes, and both `analysis::analyze_heap()` and `detect_leaks()` consume them when parsing succeeds; summary preview remains as fallback only. | Reuse dominator-backed data in diffing and future explorer surfaces. |
| GC root path | ✅ | `core::gc_path` now attempts full `ObjectGraph` BFS first, then falls back to a budget-limited `GcGraph`, then synthetic paths when necessary. | Keep edge labeling and fallback behavior aligned as more traversal surfaces land. |
| AI/LLM integration | ⚠️ | `generate_ai_insights` returns placeholder text; config fields exist. | Wire prompts/tasks to an actual LLM backend (or local model) with structured output. |
| Provenance | ✅ | `ProvenanceKind` + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in Text/Markdown/HTML/TOON and surfaced in CLI `leaks`/`gc-path`/`fix` output. | No immediate next step; provenance coverage expands as new response surfaces land. |
| Output hardening | ✅ | `escape_html` prevents XSS in HTML reports; `escape_toon_value` handles control chars in TOON. Clippy range-pattern and iterator warnings resolved. | Maintain as new renderers are added. |
| Reporting / exports | ✅ | Text/Markdown/HTML/TOON/JSON all available, with provenance markers and `--output-file` support. | Add richer diff visualizations / GUI output (still future). |
| Test fixtures / CI | ✅ | `core::test_fixtures` now ships behind `feature = "test-fixtures"` for integration-test reuse, `build_graph_fixture()` expands canonical heap shapes, `cli/tests/integration.rs` now covers 23 CLI E2E cases including table-output and truncation regressions, and the workspace is validated by 87 passing tests plus clean check/clippy/fmt runs. | Add more real-world heap fixtures and benchmark coverage in future milestones. |
| Release automation + packaging | ✅ | `.github/workflows/release.yml` validates `v*` tags against `[workspace.package].version`, builds `mnemosyne-cli` for Linux/macOS/Windows across five targets, packages tar.gz/zip archives, creates a GitHub Release with generated notes plus attached binaries, and builds/pushes `ghcr.io/<owner>/mnemosyne` on tagged releases with `<version>`, `<major>.<minor>`, and `latest` tags. Workspace/core/cli manifests now carry crates.io metadata, `cli` pins `mnemosyne-core = "0.1.0"` for publish/install compatibility, and `HomebrewFormula/mnemosyne.rb` adds macOS install scaffolding for Intel + Apple Silicon releases. | Publish the first crates.io release and replace Homebrew SHA256 placeholders on the first tagged release. |
| MCP server | ⚠️ | Command handlers exist but rely on the same placeholder AI/GC implementations. | Revisit once core analysis stack is feature-complete. |
| Documentation | ✅ | README/ARCHITECTURE/QUICKSTART/STATUS describe the current pipeline with status callouts. | Keep docs in sync as features land. |

## Remaining Must-Haves Before "Functionally Complete"
Graph-backed leak detection unification and regression-quality integration coverage are now complete for Milestone 1. The remaining must-haves are:

1. **Configurable AI task runner** (YAML- or TOML-driven) that can call an LLM or rule engine for higher-fidelity insights.
2. **Richer exporters/visualizations** (e.g., protobuf or flame-graphs) layered atop the JSON writer and `--output-file` plumbing.

## Recently Completed
- **M2 CLI table output polish** — Added `comfy-table`-based aligned terminal tables for `mnemosyne parse` summary sections and `mnemosyne leaks`, kept formatting at the CLI boundary, corrected parse-summary wording to describe heap record-category aggregate bytes/entries instead of class-level retained-size semantics, and added truncation-safe disclosure sections so long leak IDs and class names remain fully visible. Added 4 CLI integration regressions for parse tables, leak tables, long-value disclosure, and colliding truncated leak IDs, bringing the workspace total to 87 tests including 23 CLI integration cases. This completes the Milestone 2 UX polish item for table-formatted summary output ahead of first-release follow-through.
- **M2-B6 better error messages** — Added structured `CoreError` variants for missing files, non-HPROF inputs, HPROF parse failures, and config errors; `validate_heap_file()` now suggests nearby `.hprof` files and flags common wrong extensions; the CLI top-level error handler prints colored `hint:` lines; and the suite now includes 3 new error-path integration tests, bringing the workspace total to 83 tests.
- **M2-B4 Docker image** — Added a multi-stage `Dockerfile` and `.dockerignore` for a Rust builder plus `debian:bookworm-slim` runtime image, running `mnemosyne-cli` as a non-root user with `WORKDIR /data`, OCI labels, and `ENTRYPOINT ["mnemosyne-cli"]`. `.github/workflows/release.yml` now also builds and pushes `ghcr.io/<owner>/mnemosyne` on tagged releases with `<version>`, `<major>.<minor>`, and `latest` tags.
- **M2-B3 packaging** — Added crates.io-ready metadata across the workspace manifests, pinned `mnemosyne-core` versioned path dependencies in `cli/Cargo.toml` for publish/install compatibility, and added `HomebrewFormula/mnemosyne.rb` for Intel + Apple Silicon macOS release archives. `cargo publish --dry-run --allow-dirty -p mnemosyne-core` now succeeds; Homebrew SHA256 values will be filled after the first tagged release.
- **M2-B2 release automation** — Added `.github/workflows/release.yml` to validate tag/version alignment, cross-compile `mnemosyne-cli` for five targets, package tar.gz/zip artifacts, and publish GitHub Releases with generated notes and attached binaries.
- **M1 foundations batch** — Added `core::object_graph` with shared object/class/field/root data structures so upcoming parser, dominator, retained-size, and GC-path work can converge on one canonical graph model.
- **M1-B3 object-graph parser** — Added `core::hprof_parser` to read binary HPROF strings, classes, roots, instances, and arrays into the shared object graph.
- **M1-B4 dominator tree** — Added `core::dominator` with Lengauer-Tarjan immediate dominators, dominated-child lookup, and retained-size accumulation.
- **M1-B5 retained-size integration** — `analysis::analyze_heap()` now attempts graph-backed analysis first, emits real retained sizes and dominator metrics when available, and falls back with explicit provenance when filters or parsing prevent full graph-backed results.
- **M1-B6 graph-backed analysis completion** — `detect_leaks()` now follows the graph-backed path first, `gc_path` was rewritten with `ObjectGraph` BFS plus triple fallback, and `ObjectGraph` now exposes `get_object(id)`, `get_references(id)`, and `get_referrers(id)`.
- **M1-B7 integration coverage** — Added 16 CLI integration tests in `cli/tests/integration.rs` covering parse, leaks, analyze, gc-path, diff, fix, report, and config across synthetic HPROF fixtures.
- **Test-fixtures feature** — Added the `test-fixtures` cargo feature so integration tests can reuse canonical fixture builders such as `build_simple_fixture()` and `build_graph_fixture()` without widening the builder API surface.
- **Synthetic HPROF fixtures** — Added `core::test_fixtures` plus `resources/test-fixtures/README.md` so parser and graph code can exercise small deterministic heap shapes without committing large binaries.
- **CI workflow** — Added `.github/workflows/ci.yml` to run `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and pull requests.
- **Provenance batch** — `ProvenanceKind` enum (`Synthetic`, `Partial`, `Fallback`, `Placeholder`) and `ProvenanceMarker` struct integrated into `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, and `FixResponse`. Synthetic paths and fix suggestions are labeled automatically.
- **Output hardening** — `escape_html()` added to all user-controlled data in HTML reports (prevents XSS). `escape_toon_value()` added for TOON key-value output (handles backslashes, newlines, carriage returns).
- **Clippy cleanup** — Resolved range-pattern warnings in `heap.rs` and iterator warning in `mapper.rs`.
- **Provenance rendering** — All non-JSON report formats (Text, Markdown, HTML, TOON) now render per-leak and response-level provenance markers. Three dedicated tests cover text, TOON, and HTML provenance output.
- **CLI provenance display** — `leaks`, `gc-path`, and `fix` CLI commands now print provenance markers when present.

When each must-have item above turns green, we can flip the README/architecture messaging from "roadmap" to "shipped" and retire the synthetic stand-ins currently in use.
