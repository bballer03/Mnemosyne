# Milestone 3 — Core Heap Analysis Parity

> **Status:** ⚬ READY FOR IMPLEMENTATION — M1.5 prerequisite complete  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Close the feature gap with Eclipse MAT on core analysis capabilities, making Mnemosyne a credible alternative for developers who currently rely on MAT for heap investigation. Additionally, adopt performance and scalability patterns from hprof-slurp to handle multi-GB production dumps efficiently.

## Context

Eclipse MAT is the de-facto standard for JVM heap analysis. Users choose heap analysis tools based on what questions they can answer. Mnemosyne's M1 pipeline (object graph, dominator tree, retained sizes) provides the foundation, but significant feature gaps remain: no MAT-style suspect ranking, no histogram grouping, no OQL, no thread inspection, no ClassLoader analysis, no collection inspection, no unreachable object reporting, and only record-level diffing.

hprof-slurp demonstrates that a Rust HPROF parser can achieve ~2GB/s throughput with ~500MB memory — by trading analysis depth for speed. M3 should deliver both: a streaming "overview" mode for fast triage and deep graph-backed analysis for MAT-class investigation.

**Critical dependency:** M1.5 (tag-constant fix + real-world validation) is ✅ COMPLETE. The graph-backed pipeline is validated on real-world HPROF files. M3 implementation may begin.

## Scope

### Analysis Features
1. **MAT-style leak suspects** — objects where retained_size >> shallow_size; accumulation-point detection; reference chain context
2. **Histogram improvements** — group by fully-qualified class, package prefix, ClassLoader
3. **OQL-like query engine** — simple query language for ad-hoc object inspection
4. **Thread inspection** — parse STACK_TRACE + STACK_FRAME + ROOT_THREAD_OBJECT; link threads to retained objects
5. **ClassLoader analysis** — parse classloader hierarchy from CLASS_DUMP; per-loader stats; classloader leak detection
6. **Collection inspection** — detect known collection types (HashMap, ArrayList, etc.); fill ratio; size anomalies
7. **Unreachable objects** — report objects not reachable from any GC root; sizes and classes
8. **Enhanced heap diff** — object/class-level comparison (not just record-level)

### Performance & Scalability
9. **Streaming "overview" mode** — bounded-memory class/instance stats without full graph (inspired by hprof-slurp)
10. **Benchmark infrastructure** — `criterion` micro-benchmarks, `hyperfine` CLI timing, `heaptrack` memory profiling
11. **Thread stack trace extraction** — parse STACK_TRACE + STACK_FRAME records (inspired by hprof-slurp)
12. **Memory-bounded object store evaluation** — measure RSS at various dump sizes; evaluate alternatives if >4× ratio

### Supporting Features
13. **Top-N largest instances** — per-class largest single instance size
14. **String analysis** — list strings, detect duplicates, quantify dedup savings
15. **Configurable analysis profiles** — `--mode overview|deep`, `--profile ci-regression|incident-response`

## Non-scope

- Web UI (M4)
- AI/LLM wiring (M5)
- Plugin/extension system (M6)
- Real-world HPROF tag fix (M1.5 — prerequisite)
- Cross-platform distribution changes
- MCP server protocol changes (beyond new tool handlers for new features)

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                         CLI / MCP                                    │
│                                                                      │
│  --mode overview ───→ Streaming Parser (bounded memory, ~2GB/s)     │
│  --mode deep ───────→ Full Graph Pipeline (in-memory ObjectGraph)   │
│  --mode auto ───────→ overview if >1GB, deep otherwise              │
└──────────────┬───────────────────────┬───────────────────────────────┘
               │                       │
     ┌─────────▼──────────┐   ┌────────▼──────────┐
     │  Overview Mode     │   │  Deep Mode         │
     │                    │   │                    │
     │  Class histogram   │   │  ObjectGraph       │
     │  Top-N instances   │   │  Dominator tree    │
     │  Thread stacks     │   │  Retained sizes    │
     │  String stats      │   │  Leak suspects     │
     │  ──── bounded ──── │   │  GC paths          │
     │  memory (~500MB)   │   │  OQL queries       │
     └────────────────────┘   │  Collection stats  │
                              │  ClassLoader tree  │
                              │  Unreachable set   │
                              │  Object-level diff │
                              └────────────────────┘
```

### New Module Structure

```
core/src/
├── query/           ← NEW: OQL-like query engine
│   ├── mod.rs
│   ├── parser.rs    (query language parser)
│   └── evaluator.rs (query execution over ObjectGraph)
├── thread/          ← NEW: thread inspection
│   ├── mod.rs
│   └── inspector.rs (stack trace parsing + object linkage)
├── collections/     ← NEW: collection inspection
│   ├── mod.rs
│   └── inspector.rs (known-type detection + fill ratio)
├── hprof/
│   ├── tags.rs           ← NEW: centralized HPROF tag constants (dedup)
│   └── binary_parser.rs  (enhanced: overview mode, STACK_TRACE parsing)
├── graph/
│   └── metrics.rs        (enhanced: unreachable objects, histogram grouping)
├── analysis/
│   └── engine.rs         (enhanced: MAT-style suspects, enhanced diff)
└── benches/         ← NEW: criterion benchmarks
    ├── parser_bench.rs
    ├── graph_bench.rs
    └── dominator_bench.rs
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `core/src/hprof/tags.rs` | New | Centralized HPROF tag constants — single source of truth for top-level record tags and heap-dump sub-tags, replacing duplicated constants in `binary_parser.rs`, `test_fixtures.rs`, `gc_path.rs`, and inline hex in `parser.rs` |
| `core/src/hprof/mod.rs` | Updated | Add `pub mod tags;` and re-export |
| `core/src/analysis/engine.rs` | Enhanced | MAT-style suspect ranking, enhanced diff, profile selection |
| `core/src/hprof/binary_parser.rs` | Enhanced | Overview mode (bounded), STACK_TRACE/STACK_FRAME parsing; use `tags::*` instead of local constants |
| `core/src/hprof/parser.rs` | Enhanced | Stream-mode class stats with bounded memory; `tag_name()` uses `tags::*` instead of inline hex |
| `core/src/hprof/test_fixtures.rs` | Updated | Use `tags::*` instead of local constants |
| `core/src/graph/gc_path.rs` | Updated | Use `hprof::tags::*` instead of local constants |
| `core/src/graph/metrics.rs` | Enhanced | Histogram grouping, unreachable objects, collection stats |
| `core/src/query/mod.rs` | New | OQL query language module |
| `core/src/query/parser.rs` | New | Query parser (SQL-like syntax) |
| `core/src/query/evaluator.rs` | New | Query execution over ObjectGraph |
| `core/src/thread/mod.rs` | New | Thread inspection module |
| `core/src/thread/inspector.rs` | New | Stack trace parsing + object linkage |
| `core/src/collections/mod.rs` | New | Collection inspection module |
| `core/src/collections/inspector.rs` | New | Known-type detection + fill ratio |
| `core/src/lib.rs` | Updated | New public API re-exports |
| `cli/src/main.rs` | Updated | New subcommands, --mode flag, --profile flag |
| `benches/` | New | Criterion benchmark suite |

## API/CLI/Reporting Impact

### New CLI Commands
- `mnemosyne query "SELECT class, retained_size FROM objects WHERE ..."` — OQL queries
- `mnemosyne threads` — thread inspection with stack traces
- `mnemosyne dominators` — standalone dominator tree view
- `mnemosyne histogram --group-by package|classloader` — grouped histograms

### New CLI Flags
- `--mode overview|deep|auto` — analysis depth selection
- `--profile ci-regression|incident-response` — preconfigured analysis profiles
- `--top-n <N>` — control top-N display count

### New MCP Handlers
- `query_heap` — OQL queries via MCP
- `inspect_thread` — thread inspection via MCP
- `get_histogram` — grouped histogram via MCP

### Report Changes
- Leak suspects section with MAT-style ranking (retained/shallow ratio, accumulation points)
- Thread analysis section
- Collection waste analysis section
- Unreachable objects section

## Data Model Changes

### New Types
- `LeakSuspect` — retained_size, shallow_size, ratio, accumulation_point, reference_chain_summary
- `QueryResult` — rows of matched objects from OQL
- `ThreadInfo` — thread name, stack frames, retained objects, retained size
- `CollectionStats` — collection class, capacity, size, fill_ratio, wasted_bytes
- `UnreachableSet` — total count, total size, by-class breakdown
- `HistogramGroup` — grouped class stats by package or classloader
- `AnalysisProfile` — enum for preconfigured analysis configurations

### Updated Types
- `AnalyzeResponse` — add thread_info, collection_stats, unreachable_set, suspect_ranking fields
- `HeapSummary` — add histogram_groups, string_stats fields
- `HeapDiff` — add object-level diff fields (new_objects, freed_objects, retained_delta_by_class)

## Validation/Testing Strategy

### Benchmark Suite
- `criterion` benchmarks for: parser throughput (bytes/sec), graph construction (objects/sec), dominator computation, retained-size accumulation, OQL query evaluation
- `hyperfine` scripts for end-to-end CLI timing at different dump sizes
- `heaptrack` integration for memory profiling (RSS at 10MB/100MB/1GB tiers)

### Unit Tests
- OQL parser: valid/invalid query syntax, edge cases
- OQL evaluator: query execution against synthetic graphs
- Thread inspector: STACK_TRACE/STACK_FRAME parsing
- Collection inspector: known-type detection, fill-ratio computation
- Histogram grouping: package/classloader grouping logic
- Unreachable objects: reachability computation accuracy
- MAT-style suspects: ranking algorithm correctness

### Integration Tests
- CLI E2E tests for new commands and flags
- Real-world dump validation for all new features
- Profile-based analysis configuration

### Performance Gates
- Parser throughput: establish baseline, prevent >10% regression
- Memory usage: establish RSS baseline per dump size tier

## Rollout/Implementation Phases

### Phase 1 — Foundation (post-M1.5)
0. **HPROF tag centralization** — extract duplicated tag constants from `binary_parser.rs`, `test_fixtures.rs`, `gc_path.rs`, and inline hex in `parser.rs` into a single `core/src/hprof/tags.rs` module. This is a pre-requisite cleanup that eliminates a class of future drift bugs before Phase 2+ feature work adds more tag consumers.
1. Histogram improvements — group by class, package, classloader
2. Benchmark infrastructure setup (criterion + hyperfine)
3. Memory measurement at different dump sizes

#### Phase 1 Batch 1 (M3-P1-B1): Benchmark Infrastructure + RSS Measurement + Tag Centralization

**Scope:**
- Centralize HPROF tag constants into `core/src/hprof/tags.rs` (dedup from `binary_parser.rs`, `parser.rs`, `test_fixtures.rs`, `gc_path.rs`)
- Add `criterion` dev-dependency to `core/Cargo.toml`
- Add criterion benchmarks: `core/benches/parser_bench.rs` (parse_hprof), `core/benches/graph_bench.rs` (parse_hprof + object_graph construction), `core/benches/dominator_bench.rs` (build_dominator_tree)
- Add RSS measurement tooling/scripts for real 110-150MB dumps
- Document memory scaling decision: in-memory viable to X GB?

**Files impacted:**
| File | Change |
|---|---|
| `core/src/hprof/tags.rs` | NEW — centralized tag constants |
| `core/src/hprof/mod.rs` | Add `pub mod tags;` |
| `core/src/hprof/binary_parser.rs` | Replace local `TAG_*` consts with `tags::*` |
| `core/src/hprof/parser.rs` | Replace inline hex in `tag_name()` with `tags::*` |
| `core/src/hprof/test_fixtures.rs` | Replace local `TAG_*` consts with `tags::*` |
| `core/src/graph/gc_path.rs` | Replace local `HEAP_DUMP_TAG` etc. with `hprof::tags::*` |
| `core/Cargo.toml` | Add `criterion` dev-dependency, `[[bench]]` entries |
| `core/benches/parser_bench.rs` | NEW — parser throughput benchmark |
| `core/benches/graph_bench.rs` | NEW — graph construction benchmark |
| `core/benches/dominator_bench.rs` | NEW — dominator tree benchmark |

**Success criteria:**
- Zero duplicated HPROF tag constants across the codebase
- `cargo bench` runs criterion benchmarks for parser, graph, and dominator
- RSS baseline documented for synthetic fixtures and 110-150MB real dumps
- Memory scaling recommendation documented (in-memory viable to X GB?)
- All 101 existing tests continue to pass

### Phase 1 Batch 2 (M3-P1-B2): Core Analysis Features

**Scope:**
- Histogram grouping (by fully-qualified class, package prefix, ClassLoader)
- MAT-style leak suspects (retained/shallow ratio, accumulation-point detection, reference chain context)
- Unreachable objects (report objects not reachable from any GC root; sizes and classes)
- Enhanced heap diff (object/class-level comparison, not just record-level)

**Design addendum:** [docs/design/m3-p1-b2-core-analysis-features.md](m3-p1-b2-core-analysis-features.md)

**Files impacted:**
| File | Change |
|---|---|
| `core/src/graph/metrics.rs` | Enhanced — add histogram grouping, unreachable objects computation |
| `core/src/analysis/engine.rs` | Enhanced — add MAT-style suspect ranking, enhanced diff, new response fields |
| `core/src/hprof/object_graph.rs` | Enhanced — add reachability helpers |
| `core/src/graph/dominator.rs` | Enhanced — expose `shallow_size` alongside `retained_size` for suspect ranking |
| `core/src/hprof/parser.rs` | Enhanced — add new types (`HistogramGroup`, `UnreachableSet`), extend `HeapDiff` |
| `core/src/lib.rs` | Updated — new public API re-exports |
| `cli/src/main.rs` | Updated — new CLI flags for histogram grouping |

**Success criteria:**
- `analyze` output includes MAT-style suspect ranking with retained/shallow ratio
- `--group-by` flag on histogram output groups by package or classloader
- Unreachable objects section appears in analysis reports
- `diff` command produces class-level comparison using object graph data
- All existing 101 tests continue to pass
- New unit tests for each feature

### Phase 2 — Core Analysis
4. MAT-style leak suspects algorithm (✅ moved to M3-P1-B2)
5. Unreachable objects analysis (✅ moved to M3-P1-B2)
6. Enhanced heap diff (object/class level) (✅ moved to M3-P1-B2)

### Phase 3 — New Capabilities
7. Thread inspection (STACK_TRACE + STACK_FRAME parsing)
8. ClassLoader analysis
9. Collection inspection
10. String analysis (duplicates, dedup savings)

### Phase 4 — Advanced Features
11. OQL query engine (parser + evaluator)
12. Streaming "overview" mode (bounded memory)
13. Top-N largest instances

### Phase 5 — Polish
14. Analysis profiles (--profile flag)
15. Dual-mode auto-selection (auto: overview if >1GB)
16. Performance optimization based on benchmark data

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **M1.5 complete** | Resolved | N/A | M1.5 delivered all 5 phases with 101/101 tests passing; graph pipeline validated on real-world data |
| In-memory ObjectGraph may not scale to multi-GB dumps | High | High | RSS measurement in Phase 1; if >4× dump size, evaluate memmap2/disk-backed storage before Phase 2 |
| OQL complexity may exceed budget | Medium | Medium | Start with minimal query language (class/size filters only), expand incrementally |
| Thread record parsing may have JVM-version-specific edge cases | Medium | Medium | Test against multiple JVM versions (OpenJDK 11/17/21, GraalVM) |
| Benchmark infrastructure may reveal uncompetitive parser throughput | High | Medium | hprof-slurp achieves ~2GB/s; if Mnemosyne is <500MB/s, evaluate nom migration or threaded I/O before feature work |
| Feature scope is very large | High | Medium | Phase implementation strictly; each phase is independently valuable |

### Open Questions
1. Should OQL be SQL-like or use a custom syntax? (Recommendation: SQL-like subset for familiarity)
2. Should the overview mode be a separate binary or a flag? (Recommendation: flag `--mode overview`)
3. At what dump size should auto-mode switch from deep to overview? (Recommendation: 1GB default, configurable)
4. Should collection inspection require Java stdlib version metadata? (Risk: version-specific field names)

### Dependencies
- **Blocked by:** Nothing — M1.5 (real-world hardening) is complete
- **Blocks:** M4 (UI needs analysis features to display), M5 (AI needs real analysis data), M6 (benchmarks needed for comparisons)
