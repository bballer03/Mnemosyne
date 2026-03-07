# Functional Status

_Last updated: 2026-03-07_

This document captures where the current alpha build stands versus the roadmap described in `README.md` and `ARCHITECTURE.md`. Use it to see what already works, what is partially there, and which gaps remain before we can call the heap analyzer "functionally complete".

## Snapshot
- ✅ **HPROF parser** streams headers/record stats and produces a class histogram + heap summary without loading entire dumps into RAM.
- ✅ **Object-graph pipeline** now includes shared core types (`core::object_graph`) plus a binary HPROF parser (`core::hprof_parser`) that populates objects, classes, references, and GC roots.
- ✅ **CLI surface** (`parse`, `leaks`, `analyze`, `diff`, `map`, `fix`, `gc-path`, `serve`) all call into the shared core. Reports are emitted via stdout or `--output-file`.
- ✅ **Leak detection** is now unified: `detect_leaks()` attempts object-graph → dominator → retained-size analysis first, then falls back to heuristics with explicit `ProvenanceKind::Fallback` markers.
- ✅ **Graph/dominator view** now supports real retained sizes in both `analyze_heap()` and `detect_leaks()`, with the lightweight summary preview retained only as fallback.
- ✅ **GC path finder** now prefers full `ObjectGraph` BFS first, then falls back to a budget-limited `GcGraph`, then synthetic paths when the heap lacks enough detail.
- ✅ **Object-graph navigation API** now exposes `get_object(id)`, `get_references(id)`, and `get_referrers(id)` for programmatic heap exploration.
- ⚠️ **AI insights** are deterministic stubs; the configurable LLM-backed task runner is still to be wired up.
- ✅ **Report/export** supports Text/Markdown/HTML/TOON/JSON with `--output-file`. HTML output is XSS-hardened; TOON values are properly escaped. Provenance markers are rendered in all non-JSON formats.
- ✅ **Provenance system** labels synthetic, partial, fallback, and placeholder data across analysis responses, leak insights, GC paths, and fix suggestions. CLI and report renderers surface these markers to consumers.
- ✅ **Output hardening** — HTML escaping prevents XSS in report output; TOON escaping handles control characters and backslashes correctly.
- ✅ **Development workflow** now includes GitHub Actions CI, the `test-fixtures` cargo feature for canonical fixture reuse, and 80 passing tests including 16 CLI integration tests.

## Capability Checklist
| Area | Status | Notes | Next Step |
| --- | --- | --- | --- |
| Parser streaming + histogram | ✅ | `core::heap` parses headers + record stats and derives class histograms for fast summary-level commands. | Keep summary parsing aligned with the graph-backed path. |
| Object-graph foundation | ✅ | `core::object_graph` defines the canonical model and `core::hprof_parser` now populates it from binary HPROF records. | Reuse the same graph across more analysis surfaces. |
| Leak detection | ✅ | `detect_leaks()` now shares the graph-backed object-graph → dominator → retained-size path used by `analyze_heap()`, with heuristic fallback labeled via provenance markers. | Extend retained-size-backed suspect ranking into diffing and richer comparison flows. |
| Dominators / retained size | ✅ | `core::dominator` computes real retained sizes, and both `analysis::analyze_heap()` and `detect_leaks()` consume them when parsing succeeds; summary preview remains as fallback only. | Reuse dominator-backed data in diffing and future explorer surfaces. |
| GC root path | ✅ | `core::gc_path` now attempts full `ObjectGraph` BFS first, then falls back to a budget-limited `GcGraph`, then synthetic paths when necessary. | Keep edge labeling and fallback behavior aligned as more traversal surfaces land. |
| AI/LLM integration | ⚠️ | `generate_ai_insights` returns placeholder text; config fields exist. | Wire prompts/tasks to an actual LLM backend (or local model) with structured output. |
| Provenance | ✅ | `ProvenanceKind` + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in Text/Markdown/HTML/TOON and surfaced in CLI `leaks`/`gc-path`/`fix` output. | No immediate next step; provenance coverage expands as new response surfaces land. |
| Output hardening | ✅ | `escape_html` prevents XSS in HTML reports; `escape_toon_value` handles control chars in TOON. Clippy range-pattern and iterator warnings resolved. | Maintain as new renderers are added. |
| Reporting / exports | ✅ | Text/Markdown/HTML/TOON/JSON all available, with provenance markers and `--output-file` support. | Add richer diff visualizations / GUI output (still future). |
| Test fixtures / CI | ✅ | `core::test_fixtures` now ships behind `feature = "test-fixtures"` for integration-test reuse, `build_graph_fixture()` expands canonical heap shapes, `cli/tests/integration.rs` adds 16 CLI E2E tests, and the workspace is validated by 80 passing tests plus clean check/clippy/fmt runs. | Add more real-world heap fixtures and benchmark coverage in future milestones. |
| MCP server | ⚠️ | Command handlers exist but rely on the same placeholder AI/GC implementations. | Revisit once core analysis stack is feature-complete. |
| Documentation | ✅ | README/ARCHITECTURE/QUICKSTART/STATUS describe the current pipeline with status callouts. | Keep docs in sync as features land. |

## Remaining Must-Haves Before "Functionally Complete"
Graph-backed leak detection unification and regression-quality integration coverage are now complete for Milestone 1. The remaining must-haves are:

1. **Configurable AI task runner** (YAML- or TOML-driven) that can call an LLM or rule engine for higher-fidelity insights.
2. **Richer exporters/visualizations** (e.g., protobuf or flame-graphs) layered atop the JSON writer and `--output-file` plumbing.

## Recently Completed
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
