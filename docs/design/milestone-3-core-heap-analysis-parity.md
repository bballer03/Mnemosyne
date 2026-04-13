# Milestone 3 — Core Heap Analysis Parity

> **Status:** ✅ Complete for the approved scope — future evidence-driven follow-on remains  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-04-13

---

## Objective

Document the M3 milestone that made Mnemosyne a credible CLI-first alternative to Eclipse MAT for core heap investigation, and identify the narrower future follow-on work beyond the shipped approved scope.

## Context

Eclipse MAT remains the de-facto standard for JVM heap analysis. M3 was the milestone that closed Mnemosyne's approved core parity gap: graph-backed suspect ranking, grouped histograms, unreachable-object reporting, class-level diffing, thread inspection, string analysis, collection inspection, top-instance reporting, classloader reporting, the shipped OQL/query slice, and analyze profiles are now shipped in the live codebase.

hprof-slurp still provides useful scale and throughput reference points, but M3 is no longer waiting on a broad "overview mode vs deep mode" decision. Step 11 is complete, the current in-memory architecture cleared the active roadmap gate, and streaming/threaded-I/O/`nom` work is now an evidence-driven future scale path rather than a prerequisite for claiming M3 delivery.

**Current milestone truth:** the approved M3 scope is complete. The final closeout batch landed the optional `hyperfine` / `heaptrack` wrappers with graceful skip behavior plus a deeper query slice with retained instance-field projection/filtering on the CLI/MCP query paths and hierarchy-aware `INSTANCEOF`. Remaining work is future follow-on only: richer OQL/query depth beyond the shipped slice, larger-tier validation only where still useful, and evidence-driven scale levers.

## Scope

### Shipped in the approved M3 scope
1. **MAT-style leak suspects** — retained/shallow ratio, accumulation-point detection, dominated-count context, and short reference-chain context
2. **Histogram improvements** — grouping by class, package, and classloader
3. **Thread inspection** — stack-trace parsing plus retained-memory context
4. **ClassLoader analysis** — report-oriented classloader summaries in `analyze_heap()` and shared renderers
5. **Collection inspection** — waste/fill-ratio inspection for the highest-value collection types
6. **Unreachable objects** — GC-root reachability summaries
7. **Enhanced heap diff** — class-level graph-backed comparison layered onto the existing diff surface
8. **Top-N largest instances** — graph-backed retained-size ranking
9. **String analysis** — duplicate detection, dedup-waste estimation, and top strings by size
10. **Configurable analysis profiles** — `overview`, `incident-response`, and `ci-regression`
11. **Initial OQL/query surface** — the shipped built-in-field query/parser/executor slice
12. **Benchmark baseline and scaling validation** — Criterion plus completed Step 11 dense synthetic validation through roughly the 2 GB tier
13. **Final closeout batch** — optional `hyperfine` / `heaptrack` wrappers with graceful skip behavior plus deeper retained-field query/filter support and hierarchy-aware `INSTANCEOF`

### Future evidence-driven follow-on
1. **Richer OQL/query depth** — broader predicates, deeper field access, and explorer ergonomics beyond the shipped slice
2. **Additional larger-tier validation only where justified** — especially if future real-world large dumps reveal new scaling pressure
3. **Streaming overview mode / threaded I/O / `nom` evaluation only if evidence supports them** — these are scale levers, not milestone gates

## Non-scope

- Web UI (M4)
- AI/LLM wiring (M5)
- Plugin/extension system (M6)
- Real-world HPROF tag fix (M1.5 — prerequisite)
- Cross-platform distribution changes
- MCP server protocol changes (beyond new tool handlers for new features)

## Historical Note

The sections below preserve the original implementation design and phased breakdown that drove M3 delivery. Read them as historical planning context for shipped work, not as a current pending implementation checklist. Where the doc discusses overview mode, parser prerequisites, or future-tense module additions, the live codebase is the runtime truth: most deep graph-backed parity work is already shipped, and the true remaining work is the narrower follow-through listed above.

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

### Historical Module Structure (Shipped Foundation + Superseded Proposals)

The outline below captures the intended M3 shape at design time. Parts of it shipped, but some command/module proposals were superseded by the current implementation. Treat this section as historical context, not the current runtime contract.

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

### CLI / MCP / Reporting Impact (Historical Design vs Shipped Surface)

Shipped M3 surface:
- `mnemosyne query <heap.hprof> "..."` provides the initial built-in-field query surface
- `mnemosyne analyze --threads --strings --collections --classloaders --top-instances` exposes the shipped investigation features
- `mnemosyne analyze --group-by class|package|classloader` exposes grouped histograms
- MCP ships `query_heap` and the existing `analyze_heap` flags rather than dedicated `inspect_thread` / `get_histogram` handlers

Historical proposals below did not all ship as-is and should not be treated as current commands:
- `mnemosyne threads`
- `mnemosyne dominators`
- `mnemosyne histogram ...`
- `--mode overview|deep|auto`
- dedicated `inspect_thread` and `get_histogram` MCP handlers

### New CLI Flags
- Shipped: `--profile ci-regression|incident-response|overview`
- Shipped: `--top-n <N>`
- Remaining evidence-driven follow-on only: `--mode overview|deep|auto`

### New MCP Handlers
- Shipped: `query_heap`
- Not part of the shipped surface: dedicated `inspect_thread` / `get_histogram` handlers

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
- `HeapDiff` — historical proposal included deeper diff expansion; shipped M3 work added class-level retained/shallow/instance deltas rather than object-level lifecycle fields

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

### Historical delivered phases
1. **Phase 1 — Foundation:** tag centralization, Criterion benchmark setup, and memory measurement groundwork shipped
2. **Phase 1 Batch 2:** histogram grouping, MAT-style suspects, unreachable objects, and class-level diff shipped
3. **Phase 2 / 3 / 5 outcome:** thread inspection, classloader analysis, collection inspection, string analysis, top instances, and analysis profiles shipped
4. **Phase 4 first slice outcome:** the initial OQL/query engine shipped as a built-in-field surface rather than full MAT-equivalent OQL

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

### Future follow-on after shipped M3 work
1. **OQL/query follow-through** — deepen the shipped query slice beyond retained instance-field projection/filtering and hierarchy-aware `INSTANCEOF`
2. **Scale levers only if needed** — revisit overview mode, dual-mode auto-selection, performance tuning, threaded I/O, or parser strategy only if future evidence shows the current architecture is insufficient

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
- **Blocked by:** Nothing — this is now a shipped-plus-follow-through milestone record
- **Supports:** M4 UI work, remaining benchmark publication, and later M6 examples/benchmark comparisons
