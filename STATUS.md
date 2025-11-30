# Functional Status

_Last updated: 2025-11-30_

This document captures where the current alpha build stands versus the roadmap described in `README.md` and `ARCHITECTURE.md`. Use it to see what already works, what is partially there, and which gaps remain before we can call the heap analyzer "functionally complete".

## Snapshot
- ✅ **HPROF parser** streams headers/record stats and produces a class histogram + heap summary without loading entire dumps into RAM.
- ✅ **CLI surface** (`parse`, `leaks`, `analyze`, `diff`, `map`, `fix`, `gc-path`, `serve`) all call into the shared core. Reports are emitted via stdout for redirection/`tee`.
- ✅ **Leak heuristics** leverage parsed class stats, package filters, and severity thresholds before falling back to synthetic entries.
- ⚠️ **Graph/dominator view** is a lightweight preview of the top classes/records rather than a true retained-size computation.
- ✅ **GC path finder** now parses real GC roots + instance dumps (within configurable budgets) and only falls back to synthetic data when the heap lacks enough detail.
- ⚠️ **AI insights** are deterministic stubs; the configurable LLM-backed task runner is still to be wired up.
- ⚠️ **Report/export** now supports Text/Markdown/HTML/TOON/JSON plus the `--output-file` writer, but richer visualizations remain on the backlog.

## Capability Checklist
| Area | Status | Notes | Next Step |
| --- | --- | --- | --- |
| Parser streaming + histogram | ✅ | `core::heap` parses headers + record stats and derives class histograms. | Extend to retain per-object graph for retained-size math. |
| Leak detection | ✅ | `analysis::synthesize_leaks` works off real class stats with package + severity filters. | Incorporate dominator/retained-size info when available. |
| Dominators / retained size | ⚠️ | `core::graph` builds a small petgraph from top classes only. | Implement true object-graph traversal + retained-size calculations. |
| GC root path | ✅ | `core::gc_path` parses HPROF roots/instance data to trace best-effort paths, falling back only when dumps exceed budgets. | Expand coverage as retained-size graph lands. |
| AI/LLM integration | ⚠️ | `generate_ai_insights` returns placeholder text; config fields exist. | Wire prompts/tasks to an actual LLM backend (or local model) with structured output. |
| Reporting / exports | ⚠️ | Text/Markdown/HTML/TOON/JSON all available, and `--output-file` writes artifacts directly. | Add richer diff visualizations / GUI output (still future). |
| MCP server | ⚠️ | Command handlers exist but rely on the same placeholder AI/GC implementations. | Revisit once core analysis stack is feature-complete. |
| Documentation | ✅ | README/ARCHITECTURE/QUICKSTART describe the current pipeline with status callouts. | Keep STATUS.md + docs in sync as features land. |

## Remaining Must-Haves Before "Functionally Complete"
1. **Real retained-size + dominator computation** so leak reporting, diffs, and graphs are grounded in the actual object graph.
2. **Configurable AI task runner** (YAML- or TOML-driven) that can call an LLM or rule engine for higher-fidelity insights.
3. **Richer exporters/visualizations** (e.g., protobuf or flame-graphs) layered atop the new JSON writer and `--output-file` plumbing.
4. **Regression-quality tests + fixtures** that cover the graph/AI paths and document expected outputs for representative dumps.

When each item above turns green, we can flip the README/architecture messaging from "roadmap" to "shipped" and retire the synthetic stand-ins currently in use.
