# Mnemosyne Performance Baseline - Memory Scaling

> **Captured:** 2026-03-09 (post Step 11 remediation)
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

### Current (post-remediation)

| Command | Dump Size | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|
| `parse` (streaming) | 156 MB | 5.12 MiB | **0.03x** | Streaming record-level scanning; minimal RAM |
| `analyze` (default) | 156 MB | 656.65 MiB | **4.23x** | Lean graph-backed path with `retain_field_data = false` |
| `leaks` (default) | 156 MB | 656.46 MiB | **4.23x** | Same lean graph-backed path as default `analyze` |
| `analyze --strings --threads --collections` | 156 MB | ~741 MiB | **4.78x** | Opt-in higher-memory investigation path |

### Historical (Step 11 trigger before remediation)

| Command | Dump Size | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|
| `analyze` (unconditional field retention) | 156 MB | 741 MiB | **4.78x** | Historical post-Phase-2 re-baseline before `ParseOptions` remediation |
| `leaks` (unconditional field retention) | 156 MB | 741 MiB | **4.78x** | Historical post-Phase-2 re-baseline before `ParseOptions` remediation |

### Historical (pre-Phase-2 baseline)

| Command | Dump Size | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|
| `analyze` / `leaks` (pre-Phase-2) | 156 MB | ~555 MB | **3.56x** | Kept for comparison; no longer the current default-path measurement |

---

## Scaling Analysis

### Key Observations

1. **Streaming parse remains extremely efficient.** `parse` still processes the 156 MB heap in ~67 ms at 2.25 GiB/s with only 5.12 MiB peak RSS.

2. **Step 11 confirmed the Phase 2 regression and partially remediated it.** Unconditional `field_data` retention pushed graph-backed runs to 4.78x RSS:dump. The current `ParseOptions` split reduces default `analyze`/`leaks` back to 4.23x by keeping raw field retention disabled unless thread, string, or collection analysis is explicitly requested.

3. **The default graph-backed path is improved but still above the original 4.0x target.** Default `analyze`/`leaks` now sit only slightly above threshold, which is acceptable for the 156 MB fixture as a partial remediation, but it does not close the large-dump question.

4. **Investigation-heavy runs intentionally cost more.** `analyze --threads --strings --collections` still measures ~4.78x because those analyzers need raw object field data and selected primitive-array contents.

5. **Dominator work remains the main time bottleneck.** Building the dominator tree (~1.85 s) is still comparable to full binary parsing (~1.71 s). Together they keep full graph-backed analysis near ~3.5 s wall time on the 156 MB fixture.

6. **Graph queries remain cheap after construction.** Reference lookups (~27 ns), referrer lookups (~15 ns), and top retained-size queries (~712 us) are still effectively instantaneous once the graph exists.

### Projected Scaling (default 4.23x path)

| Dump Size | Estimated RSS | Estimated Parse+Dom Time | Feasibility |
|---|---|---|---|
| 156 MB | ~656 MiB | ~3.5 s | ✅ Comfortable on any workstation |
| 500 MB | ~2.1 GB | ~11 s | ⚠️ Slightly above the original 4x target; still practical on 16 GB+ systems |
| 1 GB | ~4.2 GB | ~23 s | ⚠️ Needs 8 GB+ free RAM and remains above target |
| 2 GB | ~8.5 GB | ~46 s | ⚠️ Needs 16 GB+ system and careful headroom |
| 4 GB | ~16.9 GB | ~91 s | ❌ Exceeds most workstations; alternative architecture likely required |

For opt-in investigation runs, a 4.78x ratio is the better working projection until larger-tier measurements are available.

### Decision

Based on the current measured data:
- **Step 11 is partially complete.** The script enhancement, re-baseline, and conditional field-retention remediation are done; 500 MB / 1 GB / 2 GB validation tiers are still pending.
- **`ParseOptions` is the current architectural mitigation.** Default graph-backed commands stay on the lean path, while `--threads`, `--strings`, and `--collections` explicitly opt into higher memory usage.
- **The default path is no longer catastrophically regressed, but it is still above the 4.0x target.** Current reality is 4.23x, not the old 3.56x baseline.
- **No further architectural rewrite is justified yet, but the decision is not closed.** Multi-tier validation must determine whether the 4.23x default path holds acceptably at 500 MB+ or whether streaming overview mode / memory-mapped / disk-backed work becomes mandatory.

---

## Performance Notes

- All benchmarks run on release profile with optimizations.
- Criterion collected 100 samples per benchmark.
- `scripts/measure_rss.sh` now profiles `parse`, `analyze`, and `leaks` in one pass and computes RSS:dump ratios automatically.
- The script falls back to `/proc/PID/status` VmHWM sampling when `/usr/bin/time` is unavailable.
- The script now emits pass/warn/fail markers for the current 4.0x / 6.0x thresholds.
- Gnuplot was not installed; Criterion used the plotters backend for HTML reports saved to `target/criterion/`.
- Benchmark regression detection remains available through Criterion's built-in comparison output.