# Functional Status

_Last updated: 2026-03-07_

This document captures where the current alpha build stands versus the roadmap described in `README.md` and `ARCHITECTURE.md`. Use it to see what already works, what is partially there, and which gaps remain before we can call the heap analyzer "functionally complete".

## Snapshot
- ✅ **HPROF parser** streams headers/record stats and produces a class histogram + heap summary without loading entire dumps into RAM.
- ⚠️ **Object-graph foundation** now exists as shared core types (`core::object_graph`) for objects, classes, field descriptors, and GC roots, but HPROF parsing is not wired into it yet.
- ✅ **CLI surface** (`parse`, `leaks`, `analyze`, `diff`, `map`, `fix`, `gc-path`, `serve`) all call into the shared core. Reports are emitted via stdout or `--output-file`.
- ✅ **Leak heuristics** leverage parsed class stats, package filters, and severity thresholds before falling back to synthetic entries.
- ⚠️ **Graph/dominator view** is a lightweight preview of the top classes/records rather than a true retained-size computation.
- ✅ **GC path finder** now parses real GC roots + instance dumps (within configurable budgets) and only falls back to synthetic data when the heap lacks enough detail.
- ⚠️ **AI insights** are deterministic stubs; the configurable LLM-backed task runner is still to be wired up.
- ✅ **Report/export** supports Text/Markdown/HTML/TOON/JSON with `--output-file`. HTML output is XSS-hardened; TOON values are properly escaped. Provenance markers are rendered in all non-JSON formats.
- ✅ **Provenance system** labels synthetic, partial, fallback, and placeholder data across analysis responses, leak insights, GC paths, and fix suggestions. CLI and report renderers surface these markers to consumers.
- ✅ **Output hardening** — HTML escaping prevents XSS in report output; TOON escaping handles control characters and backslashes correctly.
- ✅ **Development workflow** now includes a GitHub Actions CI pipeline plus synthetic HPROF test-fixture builders for deterministic parser/graph tests.

## Capability Checklist
| Area | Status | Notes | Next Step |
| --- | --- | --- | --- |
| Parser streaming + histogram | ✅ | `core::heap` parses headers + record stats and derives class histograms. | Extend to retain per-object graph for retained-size math. |
| Object-graph foundation | ⚠️ | `core::object_graph` defines heap-object, class, field, and GC-root types, but the parser still returns summary-only data. | Wire real HPROF records into the object graph, then feed dominator/retained-size work from it. |
| Leak detection | ✅ | `analysis::synthesize_leaks` works off real class stats with package + severity filters. | Incorporate dominator/retained-size info when available. |
| Dominators / retained size | ⚠️ | `core::graph` builds a small petgraph from top classes only. | Implement true object-graph traversal + retained-size calculations. |
| GC root path | ✅ | `core::gc_path` parses HPROF roots/instance data to trace best-effort paths, falling back only when dumps exceed budgets. | Expand coverage as retained-size graph lands. |
| AI/LLM integration | ⚠️ | `generate_ai_insights` returns placeholder text; config fields exist. | Wire prompts/tasks to an actual LLM backend (or local model) with structured output. |
| Provenance | ✅ | `ProvenanceKind` + `ProvenanceMarker` on `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, `FixResponse`. Rendered in Text/Markdown/HTML/TOON and surfaced in CLI `leaks`/`gc-path`/`fix` output. | No immediate next step; provenance coverage expands as new response surfaces land. |
| Output hardening | ✅ | `escape_html` prevents XSS in HTML reports; `escape_toon_value` handles control chars in TOON. Clippy range-pattern and iterator warnings resolved. | Maintain as new renderers are added. |
| Reporting / exports | ✅ | Text/Markdown/HTML/TOON/JSON all available, with provenance markers and `--output-file` support. | Add richer diff visualizations / GUI output (still future). |
| Test fixtures / CI | ✅ | `core::test_fixtures` builds deterministic HPROF binaries in tests, `resources/test-fixtures/README.md` documents them, and `.github/workflows/ci.yml` runs workspace check/test/clippy/fmt in GitHub Actions. | Expand integration coverage once object-graph parsing lands. |
| MCP server | ⚠️ | Command handlers exist but rely on the same placeholder AI/GC implementations. | Revisit once core analysis stack is feature-complete. |
| Documentation | ✅ | README/ARCHITECTURE/QUICKSTART/STATUS describe the current pipeline with status callouts. | Keep docs in sync as features land. |

## Remaining Must-Haves Before "Functionally Complete"
1. **Real retained-size + dominator computation** so leak reporting, diffs, and graphs are grounded in the actual object graph.
2. **Configurable AI task runner** (YAML- or TOML-driven) that can call an LLM or rule engine for higher-fidelity insights.
3. **Richer exporters/visualizations** (e.g., protobuf or flame-graphs) layered atop the JSON writer and `--output-file` plumbing.
4. **Regression-quality tests + fixtures** that cover the graph/AI paths and document expected outputs for representative dumps.

## Recently Completed
- **M1 foundations batch** — Added `core::object_graph` with shared object/class/field/root data structures so upcoming parser, dominator, retained-size, and GC-path work can converge on one canonical graph model.
- **Synthetic HPROF fixtures** — Added `core::test_fixtures` plus `resources/test-fixtures/README.md` so parser and graph code can exercise small deterministic heap shapes without committing large binaries.
- **CI workflow** — Added `.github/workflows/ci.yml` to run `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` on pushes and pull requests.
- **Provenance batch** — `ProvenanceKind` enum (`Synthetic`, `Partial`, `Fallback`, `Placeholder`) and `ProvenanceMarker` struct integrated into `AnalyzeResponse`, `LeakInsight`, `GcPathResult`, and `FixResponse`. Synthetic paths and fix suggestions are labeled automatically.
- **Output hardening** — `escape_html()` added to all user-controlled data in HTML reports (prevents XSS). `escape_toon_value()` added for TOON key-value output (handles backslashes, newlines, carriage returns).
- **Clippy cleanup** — Resolved range-pattern warnings in `heap.rs` and iterator warning in `mapper.rs`.
- **Provenance rendering** — All non-JSON report formats (Text, Markdown, HTML, TOON) now render per-leak and response-level provenance markers. Three dedicated tests cover text, TOON, and HTML provenance output.
- **CLI provenance display** — `leaks`, `gc-path`, and `fix` CLI commands now print provenance markers when present.

When each must-have item above turns green, we can flip the README/architecture messaging from "roadmap" to "shipped" and retire the synthetic stand-ins currently in use.
