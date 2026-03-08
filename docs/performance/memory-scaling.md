# Mnemosyne Performance Baseline - Memory Scaling

> **Captured:** 2026-03-08 (post M3-P1-B2)
> **Environment:** Linux (WSL2), Rust 1.x release profile
> **Heap fixture:** `resources/test-fixtures/heap.hprof` - 156 MB Kotlin + Spring Boot heap dump

---

## Benchmark Results (Criterion)

### Parser Throughput

| Benchmark | Time (median) | Throughput | Notes |
|---|---|---|---|
| parse_heap_synthetic | 7.77 us | 38.55 MiB/s | Synthetic fixture, streaming parser |
| parse_hprof_synthetic | 1.25 us | 239.90 MiB/s | Synthetic fixture, binary parser |
| parse_heap_real_fixture | 67.20 ms | 2.25 GiB/s | 156 MB real heap, streaming parser |
| parse_hprof_real_fixture | 1.71 s | 90.47 MiB/s | 156 MB real heap, binary parser -> full ObjectGraph |

### Graph Construction & Access

| Benchmark | Time (median) | Notes |
|---|---|---|
| graph_construct_parse_hprof | 1.14 us | Synthetic fixture graph construction |
| graph_get_references | 26.73 ns | Object reference lookup |
| graph_get_referrers | 15.09 ns | Reverse reference lookup |

### Dominator Tree

| Benchmark | Time (median) | Notes |
|---|---|---|
| dominator_build_synthetic | 1.40 us | Synthetic fixture Lengauer-Tarjan |
| dominator_top_retained_synthetic | 25.05 ns | Retained-size top-N query (synthetic) |
| dominator_build_real_fixture | 1.85 s | 156 MB real heap, full dominator tree |
| dominator_top_retained_real_fixture | 712.00 us | Real heap, retained-size top-N query |

---

## RSS Measurements

| Command | Dump Size | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|
| `parse` (streaming) | 156 MB | ~3.5 MB | **0.02x** | Streaming record-level scanning; minimal RAM |
| `analyze` (full graph) | 156 MB | ~555 MB | **3.56x** | Full ObjectGraph + dominator tree + retained sizes |
| `leaks` (full graph) | 156 MB | ~555 MB | **3.55x** | Same graph-backed pipeline as analyze |

---

## Scaling Analysis

### Key Observations

1. **Streaming parser is extremely efficient**: The `parse` command processes a 156 MB heap dump in ~67 ms at 2.25 GiB/s with only ~3.5 MB peak RSS. This path scales to arbitrarily large dumps.

2. **Full graph-backed analysis uses ~3.5x dump size in RAM**: For a 156 MB heap, peak RSS is ~555 MB. This is within the 4x target threshold established in the design template.

3. **Dominator tree is the bottleneck**: Building the dominator tree takes ~1.85s on the 156 MB fixture - comparable to the binary parser's ~1.71s. Together, full graph-backed analysis of a 156 MB dump takes ~3.5s wall time.

4. **Graph queries are sub-microsecond**: Once built, reference lookups (~27 ns) and referrer lookups (~15 ns) are effectively instantaneous. Top retained-size queries complete in ~712 us.

5. **Binary parser throughput**: ~90 MiB/s on real data vs ~240 MiB/s on synthetic fixtures. The gap reflects the complexity of real HPROF binary records (strings, classes, instances, arrays) vs minimal synthetic fixtures.

### Projected Scaling

| Dump Size | Estimated RSS | Estimated Parse+Dom Time | Feasibility |
|---|---|---|---|
| 156 MB | ~555 MB | ~3.5 s | ✅ Comfortable on any workstation |
| 500 MB | ~1.8 GB | ~11 s | ✅ Fine on modern laptops (16 GB+) |
| 1 GB | ~3.6 GB | ~22 s | ⚠️ Needs 8 GB+ free RAM |
| 2 GB | ~7.1 GB | ~45 s | ⚠️ Needs 16 GB+ system |
| 4 GB | ~14.2 GB | ~90 s | ❌ Exceeds most workstations; need streaming or mmap |

### Decision

Based on measured data:
- **Current architecture is sound for dumps up to ~1-2 GB** on modern developer workstations with 16-32 GB RAM.
- **The 3.5x RSS:dump ratio is within the 4x safety threshold.**
- **No immediate action required** - the streaming parser already provides a low-memory fallback path.
- **For 4 GB+ dumps**, investigate `memmap2` or disk-backed index as described in the design template. This is future M4+ work.

---

## Performance Notes

- All benchmarks run on release profile with optimizations.
- Criterion collected 100 samples per benchmark.
- RSS captured via `/proc/PID/status` sampling at 100ms intervals (peak tracking).
- `/usr/bin/time` was not available in the WSL2 environment; manual `/proc` sampling was used instead.
- Gnuplot not installed; Criterion used plotters backend for HTML reports saved to `target/criterion/`.
- Benchmark regression detection is available via Criterion's built-in comparison - the `change:` lines show delta vs previous runs.