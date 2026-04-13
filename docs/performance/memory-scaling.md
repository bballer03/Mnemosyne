# Mnemosyne Performance Baseline - Memory Scaling

> **Captured:** 2026-04-12 (post Step 11 multi-tier dense synthetic validation)
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

### Multi-tier dense synthetic validation (Step 11 complete)

| Command | Dump Size | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|
| `parse` | ~494 MB | 8.88 MiB | **0.02x** | Dense synthetic tier T1 (`--size-mb 320`) |
| `analyze` (default) | ~494 MB | 1.40 GiB | **2.90x** | Lean graph-backed path |
| `leaks` (default) | ~494 MB | 1.40 GiB | **2.90x** | Lean graph-backed leak path |
| `analyze --threads --strings --collections` | ~494 MB | 1.89 GiB | **3.92x** | Investigation path still below 4.0x |
| `parse` | ~982 MB | 9.12 MiB | **0.01x** | Dense synthetic tier T2 (`--size-mb 640`) |
| `analyze` (default) | ~982 MB | 2.75 GiB | **2.87x** | Lean graph-backed path |
| `leaks` (default) | ~982 MB | 2.75 GiB | **2.87x** | Lean graph-backed leak path |
| `analyze --threads --strings --collections` | ~982 MB | 3.73 GiB | **3.89x** | Investigation path still below 4.0x |
| `parse` | ~1.88 GiB | 9.12 MiB | **0.00x** | Dense synthetic tier T3 (`--size-mb 1260`) |
| `analyze` (default) | ~1.88 GiB | 5.43 GiB | **2.89x** | Lean graph-backed path |
| `leaks` (default) | ~1.88 GiB | 5.43 GiB | **2.89x** | Lean graph-backed leak path |
| `analyze --threads --strings --collections` | ~1.88 GiB | 7.37 GiB | **3.92x** | Investigation path still below 4.0x |

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

2. **Step 11 confirmed the Phase 2 regression, remediated it, and cleared the multi-tier gate.** Unconditional `field_data` retention pushed graph-backed runs to 4.78x RSS:dump on the 156 MB fixture. The current `ParseOptions` split reduces default `analyze`/`leaks` back to 4.23x there, and dense synthetic validation now shows stable ~2.87x-2.90x default-path behavior plus ~3.89x-3.92x investigation-path behavior at ~500 MB, ~1 GB, and ~2 GB tiers.

3. **The default graph-backed path scales materially better than the 156 MB remediation baseline suggested.** The 156 MB real-world fixture remains an important regression sentinel at 4.23x, but the denser large-tier synthetic validation shows the lean path staying under 3.0x through the ~2 GB tier.

4. **Investigation-heavy runs intentionally cost more.** `analyze --threads --strings --collections` still measures ~4.78x because those analyzers need raw object field data and selected primitive-array contents.

5. **Dominator work remains the main time bottleneck.** Building the dominator tree (~1.85 s) is still comparable to full binary parsing (~1.71 s) on the 156 MB fixture. The dense synthetic validation focused on RSS rather than timing, but it confirms that memory does not spike super-linearly as tier size grows.

6. **Graph queries remain cheap after construction.** Reference lookups (~27 ns), referrer lookups (~15 ns), and top retained-size queries (~712 us) are still effectively instantaneous once the graph exists.

### Projected Scaling (historical pre-validation estimate)

| Dump Size | Estimated RSS | Estimated Parse+Dom Time | Feasibility |
|---|---|---|---|
| 156 MB | ~656 MiB | ~3.5 s | ✅ Comfortable on any workstation |
| 500 MB | ~2.1 GB | ~11 s | Historical estimate before dense multi-tier validation |
| 1 GB | ~4.2 GB | ~23 s | Historical estimate before dense multi-tier validation |
| 2 GB | ~8.5 GB | ~46 s | Historical estimate before dense multi-tier validation |
| 4 GB | ~16.9 GB | ~91 s | ❌ Exceeds most workstations; alternative architecture likely required |

For opt-in investigation runs, a 4.78x ratio is the better working projection until larger-tier measurements are available.

### Decision

Based on the current measured data:
- **Step 11 is complete.** The script enhancement, re-baseline, conditional field-retention remediation, dense synthetic generator redesign, and 500 MB / 1 GB / 2 GB validation tiers are all complete.
- **`ParseOptions` remains the architectural mitigation that separates lean and investigation-heavy paths.** Default graph-backed commands stay lean, while `--threads`, `--strings`, and `--collections` opt into the higher-cost path.
- **The current in-memory `ObjectGraph` architecture passes the Step 11 gate.** Dense multi-tier validation kept default `analyze`/`leaks` at 2.87x-2.90x RSS:dump and kept the opt-in investigation path at 3.89x-3.92x through the ~2 GB tier.
- **No immediate memory-architecture rewrite is justified.** Streaming overview mode, memory-mapped backing stores, and disk-backed alternatives stay on the backlog as future scaling levers rather than blockers for the current milestone.

---

## Performance Notes

- All benchmarks run on release profile with optimizations.
- Criterion collected 100 samples per benchmark.
- `scripts/measure_rss.sh` now profiles `parse`, `analyze`, and `leaks` in one pass and computes RSS:dump ratios automatically.
- `scripts/run_hyperfine_bench.sh <heap.hprof>` wraps an optional `hyperfine` install for a small `parse` / `analyze` / `leaks` command matrix; when `hyperfine` is absent it prints a skip message and exits successfully.
- `scripts/run_heaptrack_profile.sh [--command "analyze --threads --strings --collections"] <heap.hprof>` wraps an optional `heaptrack` install for one `mnemosyne-cli` subcommand plus flags, always appends the heap path as the final CLI argument, prints the expanded CLI command and chosen output path, and skips cleanly when `heaptrack` is absent.
- The script falls back to `/proc/PID/status` VmHWM sampling when `/usr/bin/time` is unavailable.
- The script now emits pass/warn/fail markers for the current 4.0x / 6.0x thresholds.
- Gnuplot was not installed; Criterion used the plotters backend for HTML reports saved to `target/criterion/`.
- Benchmark regression detection remains available through Criterion's built-in comparison output.
