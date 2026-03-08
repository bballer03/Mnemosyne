# Milestone 1.5 — Real-World Hardening

> **Status:** ✅ COMPLETE — All 5 phases delivered, 101/101 tests passing  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Fix the critical HPROF tag-constant bug, validate the graph-backed analysis pipeline end-to-end against real-world HPROF files, and close the gap between synthetic-only validation and production readiness. This unblocks all downstream milestones (M3–M6).

## Context

M1 delivered the architecture, data structures, and algorithms for graph-backed heap analysis — but only validated them against synthetic test fixtures that use `HEAP_DUMP` (tag 0x0C). Real-world JVM dumps use `HEAP_DUMP_SEGMENT` (tag 0x1C), which the binary parser currently skips due to an incorrect tag constant mapping. The result: on production Kotlin+Spring Boot heap dumps (~110MB, ~150MB), the ObjectGraph is empty, the dominator tree shows 7 nodes (record tag types, not objects), leak detection returns zero candidates, and all GC paths are synthetic.

This is the highest-priority work in the project. **Every downstream feature depends on a correctly populated object graph from real-world data.**

### Root Cause Detail

Both `parser.rs` and `binary_parser.rs` have incorrect HPROF tag-to-name mappings:
- Tag `0x0D` → mapped as `HEAP_DUMP_SEGMENT` (HPROF spec: `CPU_SAMPLES`)
- Tag `0x1C` → mapped as `CPU_SAMPLES` (HPROF spec: `HEAP_DUMP_SEGMENT`)
- Tag `0x0E` → mapped as `HEAP_DUMP_END` (HPROF spec: `CONTROL_SETTINGS`)
- Tag `0x2C` → mapped as `HEAP_DUMP_SEGMENT_EXT` (HPROF spec: `HEAP_DUMP_END`)

`gc_path.rs` also references `HEAP_DUMP_SEGMENT_TAG` with the wrong value.

## Scope

### P0 — Must complete before any M3 work
1. **Fix HPROF tag constants** in `binary_parser.rs`, `parser.rs`, and `gc_path.rs`
2. **Add HEAP_DUMP_SEGMENT (0x1C) parsing** — dispatch 0x1C records through the same sub-record parser as HEAP_DUMP (0x0C)
3. **Real-world HPROF test fixture** — source/generate a small (~5-10MB) real JVM heap dump using HEAP_DUMP_SEGMENT records; add to test suite
4. **End-to-end pipeline validation** — verify against real dumps: non-empty ObjectGraph, meaningful dominator tree, non-zero leak candidates, non-synthetic GC paths

### P1 — Complete shortly after P0
5. **Investigate heuristic fallback zero-results** — verify fallback produces reasonable candidates when graph path is artificially disabled on real data
6. **Leak-ID validation** — `explain` and `fix` commands should error on unknown IDs instead of returning generic responses
7. **HEAP_DUMP_SEGMENT unit tests** — dedicated parser tests for tag 0x1C to prevent regression

## Non-scope

- MAT-style leak suspect ranking (M3)
- Histogram grouping (M3)
- OQL query engine (M3)
- Thread inspection (M3)
- ClassLoader analysis (M3)
- AI/LLM wiring (M5)
- Performance benchmarking (post-M1.5, M3)
- Streaming "overview" mode (M3)
- Any changes to report formats or CLI UX

## Architecture Overview

No architectural changes. M1.5 is a correctness fix within the existing pipeline:

```
                 HPROF File (real-world, tag 0x1C)
                              │
                              ▼
                   ┌──────────────────────┐
                   │   binary_parser.rs   │
                   │                      │
                   │  Tag dispatch:       │
                   │  0x0C HEAP_DUMP ──┐  │
                   │  0x1C HEAP_DUMP   │  │  ← FIX: add 0x1C dispatch
                   │       _SEGMENT ───┤  │
                   │                   ▼  │
                   │  parse_sub_records() │
                   │                      │
                   └──────────┬───────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │     ObjectGraph      │
                   │  (now populated with │
                   │   real objects!)      │
                   └──────────┬───────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
              ▼               ▼               ▼
       dominator.rs     gc_path.rs      engine.rs
       (real retained   (real BFS       (real graph-
        sizes)           paths)          backed leaks)
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `core/src/hprof/binary_parser.rs` | Bug fix | Correct `TAG_HEAP_DUMP_SEGMENT` from `0x0D` to `0x1C`; add 0x1C to tag dispatch |
| `core/src/hprof/parser.rs` | Bug fix | Correct `tag_name()` mappings for 0x0D, 0x0E, 0x1C, 0x2C |
| `core/src/graph/gc_path.rs` | Bug fix | Correct `HEAP_DUMP_SEGMENT_TAG` constant |
| `core/src/analysis/engine.rs` | Investigation | Verify heuristic fallback thresholds on real data |
| `core/src/fix/generator.rs` | Enhancement | Add leak-ID validation |
| `core/src/hprof/test_fixtures.rs` | Enhancement | Add HEAP_DUMP_SEGMENT fixture builder |
| `cli/tests/integration.rs` | Enhancement | Add real-world validation integration tests |
| `resources/test-fixtures/` | New fixture | Small real-world HPROF file |

## API/CLI/Reporting Impact

- No API changes — same public types and function signatures
- `parse` output will correctly label tag 0x1C as `HEAP_DUMP_SEGMENT` and 0x0D as `CPU_SAMPLES`
- `analyze`, `leaks`, `gc-path` will produce real graph-backed results on real dumps instead of falling back
- `explain` and `fix` will return errors for invalid/unknown leak IDs (new behavior)

## Data Model Changes

No structural data model changes. The existing `ObjectGraph`, `DominatorTree`, `LeakInsight`, `GcPathResult` types all remain unchanged. The difference is that they will now be populated with real data from production HPROF files.

## Validation/Testing Strategy

### New Tests Required
1. **Tag constant regression test** — unit test asserting correct tag-to-name mappings
2. **HEAP_DUMP_SEGMENT parsing test** — unit test parsing a buffer with 0x1C records into a non-empty ObjectGraph
3. **Real-world HPROF fixture test** — integration test using a small real JVM dump:
   - ObjectGraph has >100 objects (not 7)
   - DominatorTree has meaningful retained sizes
   - `detect_leaks()` returns ≥1 candidate
   - `find_gc_path()` returns a non-synthetic path
4. **Heuristic fallback validation** — unit test confirming fallback produces candidates when graph path is disabled
5. **Leak-ID validation test** — integration test confirming `explain` and `fix` error on unknown IDs

### Acceptance Criteria
- `mnemosyne parse` correctly labels tag 0x1C as HEAP_DUMP_SEGMENT
- `binary_parser::parse_hprof_file()` produces a non-empty ObjectGraph from a real JVM heap dump
- `analyze_heap()` on a real dump shows object-level dominators (not record-tag-level)
- `detect_leaks()` on a real dump returns ≥1 leak candidate
- `gc-path` on a real dump returns a non-synthetic path at least some of the time
- `explain` and `fix` with an invalid leak-id return an error
- All existing 87 tests continue to pass
- At least 5 new tests exercise HEAP_DUMP_SEGMENT parsing and real-world validation
- CI runs clean

## Rollout/Implementation Phases

### Phase 1 — Tag Fix (P0, effort: Small)
1. Fix tag constants in `binary_parser.rs`
2. Fix `tag_name()` in `parser.rs`
3. Fix `HEAP_DUMP_SEGMENT_TAG` in `gc_path.rs`
4. Run existing 87 tests to confirm no regressions

### Phase 2 — Parser Dispatch (P0, effort: Small-Medium)
1. Add 0x1C to the binary parser's top-level record-tag match
2. Route to the same sub-record parser used for 0x0C
3. Add unit tests for 0x1C parsing

### Phase 3 — Real-World Validation (P0, effort: Medium)
1. Source or generate a small (~5-10MB) real JVM heap dump
2. Add integration tests asserting non-empty graph, real dominator tree, non-zero leaks, non-synthetic GC paths
3. Validate `parse`, `analyze`, `leaks`, `gc-path` pipeline end-to-end

### Phase 4 — Fallback Investigation (P1, effort: Medium)
1. Investigate heuristic fallback zero-results on real data
2. Tune thresholds if needed
3. Add fallback validation tests

### Phase 5 — Leak-ID Validation (P1, effort: Small)
1. Add validation in `explain` and `fix` command handlers
2. Return clear errors for unknown IDs
3. Add integration tests

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Tag fix may reveal other sub-record parsing bugs | Medium | Medium | Extensive validation against multiple real-world dumps from different JVM versions |
| Real-world ObjectGraph may exceed memory on moderate machines | Medium | High | After tag fix, measure RSS for ~150MB dump; defer to M3 if needed |
| Other HPROF tags may also be mismatched | Low | Medium | Verify ALL tag constants against authoritative JDK source (`hprof_b_spec.h`) |
| Heuristic thresholds may need significant retuning | Medium | Medium | May need separate validation pass after graph-backed path is working |
| Sourcing a small real-world test fixture may be non-trivial | Low | Low | Generate from a minimal Java app with `jmap -dump:format=b` |

### Dependencies
- **Blocks:** M3, M4, M5, M6 (all depend on a working object graph from real data)
- **Blocked by:** Nothing — this is the critical path
