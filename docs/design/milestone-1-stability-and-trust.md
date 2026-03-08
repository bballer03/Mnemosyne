# Milestone 1 — Stability & Trust

> **Status:** ✅ COMPLETE — Real-world-validated via M1.5  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Build the foundational object-graph pipeline that makes Mnemosyne's analysis credible: a real object graph populated from HPROF binary data, a dominator tree with retained-size computation, graph-backed leak detection, and GC root path tracing over the full object graph.

## Context

Without a real object graph and retained sizes, Mnemosyne cannot make credible claims about memory analysis. This milestone delivers the analytical foundation that all downstream features (leak suspects, histograms, diffing, AI-assisted explanations) depend on. Before M1, Mnemosyne could only produce summary-level record histograms from streaming parsing — no per-object analysis, no dominator tree, no retained sizes.

## Scope

- **Object graph data model** — `ObjectGraph`, `HeapObject`, `ClassInfo`, `FieldDescriptor`, `GcRoot`, `GcRootType`, `LoadedClass`, `ObjectKind`
- **Binary HPROF parser** — parse STRING_IN_UTF8, LOAD_CLASS, GC roots, INSTANCE_DUMP, OBJ_ARRAY_DUMP, PRIM_ARRAY_DUMP sub-records into ObjectGraph
- **Dominator tree** — Lengauer-Tarjan via `petgraph::algo::dominators::simple_fast` with virtual super-root connected to all GC roots
- **Retained size computation** — post-order traversal accumulation over the dominator tree
- **Graph-backed analysis** — `analyze_heap()` attempts graph path first, falls back with `ProvenanceKind::Fallback`
- **Unified leak detection** — `detect_leaks()` uses the same graph-backed path
- **GC root path tracing** — `ObjectGraph` BFS first, budget-limited `GcGraph` second, synthetic third
- **Object graph navigation API** — `get_object(id)`, `get_references(id)`, `get_referrers(id)`
- **Synthetic HPROF test fixtures** — deterministic binary builders for unit and integration testing
- **CI pipeline** — GitHub Actions for build + test + clippy + fmt
- **Integration tests** — 23 CLI E2E tests covering all major commands

## Non-scope

- Real-world HPROF validation (deferred to M1.5)
- AI/LLM wiring
- CLI UX polish (M2)
- MAT-style advanced analysis features (M3)
- Thread inspection, ClassLoader analysis, OQL (M3)
- Report format changes beyond provenance marker rendering

## Architecture Overview

```
                        ┌─────────────────────┐
                        │   HPROF File (.hprof)│
                        └──────────┬──────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                     │
              ▼                    ▼                     ▼
   ┌──────────────────┐ ┌──────────────────┐  ┌──────────────────┐
   │   parser.rs      │ │ binary_parser.rs │  │ test_fixtures.rs │
   │   (streaming     │ │ (full object     │  │ (synthetic HPROF │
   │    summary)      │ │  graph builder)  │  │  builders)       │
   └────────┬─────────┘ └────────┬─────────┘  └──────────────────┘
            │                    │
            ▼                    ▼
   ┌──────────────────┐ ┌──────────────────┐
   │   HeapSummary    │ │   ObjectGraph    │
   │   (record stats) │ │ (objects, refs,  │
   └──────────────────┘ │  classes, roots) │
                        └────────┬─────────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
                    ▼            ▼            ▼
          ┌──────────────┐ ┌─────────┐ ┌──────────┐
          │ dominator.rs │ │gc_path  │ │engine.rs │
          │ (dom tree +  │ │.rs (BFS │ │(analyze  │
          │  retained sz)│ │+ fallbk)│ │+ leaks)  │
          └──────────────┘ └─────────┘ └──────────┘
```

## Module/File Impact

| File | Change | Status |
|---|---|---|
| `core/src/hprof/object_graph.rs` | New: canonical graph model types | ✅ Delivered |
| `core/src/hprof/binary_parser.rs` | New: binary HPROF → ObjectGraph parser | ✅ Delivered (⚠️ tag bug) |
| `core/src/hprof/test_fixtures.rs` | New: synthetic HPROF fixture builders | ✅ Delivered |
| `core/src/graph/dominator.rs` | New: Lengauer-Tarjan dominator tree + retained sizes | ✅ Delivered |
| `core/src/graph/gc_path.rs` | Rewritten: ObjectGraph BFS + triple fallback | ✅ Delivered |
| `core/src/graph/metrics.rs` | Updated: graph metrics from dominator data | ✅ Delivered |
| `core/src/analysis/engine.rs` | Updated: graph-backed analyze + detect_leaks | ✅ Delivered |
| `core/src/lib.rs` | Updated: new public API re-exports | ✅ Delivered |
| `.github/workflows/ci.yml` | New: CI pipeline | ✅ Delivered |
| `cli/tests/integration.rs` | New: 23 CLI E2E tests | ✅ Delivered |

## API/CLI/Reporting Impact

- `analyze_heap()` and `detect_leaks()` now attempt graph-backed analysis first
- `find_gc_path()` uses ObjectGraph BFS as primary path
- New navigation API: `get_object()`, `get_references()`, `get_referrers()`
- All responses carry `ProvenanceKind` markers distinguishing real vs fallback vs synthetic data
- CLI `leaks`, `gc-path`, `fix` commands surface provenance markers
- All 5 report formats render provenance markers

## Data Model Changes

### New Types (core::hprof::object_graph)
- `ObjectGraph` — central graph container with objects, classes, references, GC roots, string table
- `HeapObject` — individual heap object (class ref, size, field values, object kind)
- `ClassInfo` — class metadata (name, super class, fields, instance size)
- `FieldDescriptor` — field name + type
- `GcRoot` — GC root entry (object ID, root type)
- `ObjectKind` — Instance | ObjectArray | PrimitiveArray

### New Types (core::graph::dominator)
- `DominatorTree` — wrapper around computed dominator relationships with retained sizes
- `DominatorNode` — node with object ID, class name, shallow size, retained size

### Updated Types
- `AnalyzeResponse` — now carries `provenance: Vec<ProvenanceMarker>` and graph metrics
- `LeakInsight` — now carries per-leak `provenance: Option<ProvenanceMarker>`
- `GcPathResult` — now carries `provenance: Option<ProvenanceMarker>`
- `FixResponse` — now carries `provenance: Option<ProvenanceMarker>`

## Validation/Testing Strategy

- **59 core unit tests** — parser, graph, dominator, retained sizes, provenance, reporting, escaping
- **5 CLI unit tests** — argument parsing, config loading
- **23 CLI integration tests** — E2E subprocess tests covering parse, leaks, analyze, gc-path, diff, fix, report, config
- **Synthetic HPROF fixtures** — `build_simple_fixture()` and `build_graph_fixture()` behind the `test-fixtures` cargo feature
- **CI** — GitHub Actions: cargo check, test, clippy (-D warnings), fmt --check

### Known Testing Gap
All 87 tests use synthetic HPROF data with `HEAP_DUMP` (tag 0x0C). No tests exercise `HEAP_DUMP_SEGMENT` (tag 0x1C) which is used by all real-world JVM dumps. This gap directly allowed the tag-constant bug to ship undetected. See M1.5 for remediation.

## Rollout/Implementation Phases

All phases delivered in order:
1. ✅ M1-B1: Sample HPROF test fixtures
2. ✅ M1-B2: Object graph data structures
3. ✅ M1-B3: Binary HPROF parser
4. ✅ M1-B4: Dominator tree algorithm
5. ✅ M1-B5: Retained size integration into analyze_heap()
6. ✅ M1-B6: Wire graph into detect_leaks() + gc_path + navigation API
7. ✅ M1-B7: Integration tests + test-fixtures feature

## Risks and Open Questions

| Risk | Status | Impact |
|---|---|---|
| **HPROF tag-constant bug (P0)** | ✅ RESOLVED | Fixed in M1.5 Phase 1: `TAG_HEAP_DUMP_SEGMENT` corrected to `0x1C`, `CPU_SAMPLES` to `0x0D` in binary_parser.rs, parser.rs, gc_path.rs |
| Graph-backed path validated only on synthetic data | ✅ RESOLVED | M1.5 Phase 3 added 4 real-world integration tests against optional heap.hprof; pipeline validated end-to-end |
| Heuristic fallback returns zero candidates on real data | ✅ RESOLVED | M1.5 Phase 4 validated heuristic fallback with nonexistent-package filter test |
| In-memory ObjectGraph scaling for large dumps | ⚠️ Open | Tested with small real-world dumps; large-dump (>1GB) memory behavior deferred to M3 benchmark evaluation |

### Critical Finding
M1 is architecturally sound. The algorithms, data structures, and pipeline design are correct. The failure is a single tag-constant error in the HPROF binary parser that causes it to look for heap data at the wrong tag offset. Fix is tracked in M1.5.
