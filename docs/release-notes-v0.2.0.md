# Mnemosyne v0.2.0 Release Notes

> Release date: 2026-03-08
> Previous release: v0.1.1

## Highlights

### New Heap Analysis Features (M3 Phase 1)
- **Graph-backed histogram grouping** by class, package, or classloader with instance counts, shallow sizes, and retained sizes
- **MAT-style suspect ranking** with retained/shallow ratio, accumulation-point detection, dominated-object counts, and composite scoring
- **Unreachable-object analysis** with per-class breakdown of unreachable instances and shallow byte totals
- **Class-level heap diff** augmenting record-level comparisons with instance, shallow-byte, and retained-byte deltas

### Memory Analysis Improvements
- **HPROF tag centralization** — all tag constants now flow from `core::hprof::tags`, eliminating duplication across parser, binary parser, fixtures, and GC-path code
- **Leak-ID validation** — `explain`, `fix`, and MCP `explain_leak` now fail fast on unknown leak identifiers instead of silently falling back
- **Real-world validation** — end-to-end pipeline validated on Kotlin + Spring Boot heap dumps (~110-156 MB)

### Performance Baseline
First published benchmark data on a 156 MB real-world heap dump:
- Streaming parser: **2.25 GiB/s** throughput, ~3.5 MB peak RSS
- Binary parser → ObjectGraph: **90.5 MiB/s**
- Dominator tree (Lengauer–Tarjan): **1.85s** for 156 MB dump
- Graph queries: **sub-microsecond** (27 ns references, 15 ns referrers)
- Release-time full graph analysis RSS: **~555 MB** (3.56x dump size, within the original 4x safety threshold)

> Post-release note (2026-03-09): Step 11 large-dump re-baselining later found a 4.78x regression after Phase 2 field retention landed. A follow-up remediation introduced `ParseOptions` and restored default `analyze`/`leaks` runs to ~656 MiB / 4.23x on the same 156 MB fixture, while opt-in investigation runs remain ~741 MiB / 4.78x. See `docs/performance/memory-scaling.md` for the current measurements.

### Bug Fixes
- **Critical HPROF tag-constant fix**: `TAG_HEAP_DUMP_SEGMENT` corrected from `0x0D` to `0x1C` — real-world JVM dumps now parse correctly
- Corrected `tag_name()` mappings for `CPU_SAMPLES`, `CONTROL_SETTINGS`, `HEAP_DUMP_SEGMENT`, `HEAP_DUMP_END`
- Fixed edge-label regression in GC path reconstruction from ObjectGraph BFS

### Testing
- 110 passing tests (75 core + 35 CLI) including real-world HPROF integration tests
- Criterion benchmark suite with parser, graph, and dominator benchmarks

### Roadmap Progress
- M1 (Stability & Trust): ✅ Complete
- M1.5 (Real-World Hardening): ✅ Complete
- M2 (Packaging & DX): ✅ Complete
- M3 Phase 1 (Core Analysis Features): ✅ Complete
- M3 Phase 2 (Advanced Analysis): Design phase

## Upgrade Notes
- No breaking API changes from v0.1.1
- `LeakInsight` has new optional fields (`shallow_size_bytes`, `suspect_score`) with `skip_serializing_if` for backward compatibility
- `AnalyzeResponse` has new optional fields (`histogram`, `unreachable_objects`)
- `HeapDiff` has a new optional `class_diff` field