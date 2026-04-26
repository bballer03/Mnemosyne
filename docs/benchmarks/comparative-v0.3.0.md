# Mnemosyne v0.3.0 comparative benchmarks vs hprof-slurp (and MAT, pending)

This document publishes the slice C raw benchmark run requested by [milestone-7-5-comparative-benchmarks.md](../design/milestone-7-5-comparative-benchmarks.md). It is intentionally a partial comparative report: only `mnemo-overview` and `hprof-slurp` ran, only `small`, `medium`, and `large` fixtures were measured, and the execution environment was Arch Linux on WSL2 against `/mnt/d/Mnemosyne` rather than the native-Linux reference workstation frozen in [reference-spec.md](reference-spec.md).

> TL;DR
>
> - This is the slice D publication for a partial slice C run, not the final M7-5 reference-workstation report.
> - Measured: `mnemo-overview` and `hprof-slurp` on `small`, `medium`, and `large` fixtures with `N=3`.
> - Not measured: Eclipse MAT, `mnemo-deep`, the `xlarge` 10 GiB fixture, equivalence, or native-Linux reference-workstation numbers.
> - Raw artifacts live under [docs/performance/raw/](../performance/raw/), with caveats captured in [run-notes.md](../performance/raw/run-notes.md).

## Methodology

This report is scoped by [milestone-7-5-comparative-benchmarks.md](../design/milestone-7-5-comparative-benchmarks.md), especially its output-artifact requirements in sections 11 and 13 and its honesty contract in section 12. The source-of-truth measurements are [results-overview-hprof.csv](../performance/raw/results-overview-hprof.csv) and [results.csv](../performance/raw/results.csv); both files contain the same completed slice C rows.

The measured subset is narrower than the full design:

- Tools: `mnemo-overview` and `hprof-slurp` only.
- Fixtures: `small`, `medium`, and `large` only.
- Repetitions: `N=3` per tool x fixture cell, not the design default `N=5`.
- Environment: Arch Linux on WSL2 against the Windows-mounted `/mnt/d/Mnemosyne` workspace, not the native-Linux reference workstation in [reference-spec.md](reference-spec.md).

Two caveats matter for interpretation:

- The `small` fixture in this run is a locally generated `resources/test-fixtures/heap.hprof`, not the historical ~156 MiB real-world regression sentinel described elsewhere in the benchmark docs.
- The `mnemo-overview` rows are overview-mode numbers only. They reflect the structured overview summary path, not deep-mode graph, dominator, retained-size, or leak work.

## Environment

Relevant excerpt from [environment.txt](../performance/raw/environment.txt) plus [run-notes.md](../performance/raw/run-notes.md):

```text
WSL distro: Arch Linux on WSL2
Kernel: Linux DESKTOP-8KSNRFK 6.6.87.2-microsoft-standard-WSL2 #1 SMP PREEMPT_DYNAMIC Thu Jun 5 18:30:46 UTC 2025 x86_64 GNU/Linux
CPU-count probe captured in raw environment file: 16
Memory snapshot: Mem: 15Gi total, 1.3Gi used, 12Gi free, 14Gi available
Workspace filesystem: D:\ mounted at /mnt/d (954G size, 157G available, 84% used)
Java path: /usr/lib/jvm/java-17-amazon-corretto/bin/java
Cargo: cargo 1.91.1 (ea2d97820 2025-10-10)
GNU time availability: bash: line 1: /usr/bin/time: No such file or directory
```

Slice C therefore used temporary WSL compatibility shims for `python3`, `gtime`, and `timeout`, and LF-normalized temporary execution copies of the harness scripts, all described in [run-notes.md](../performance/raw/run-notes.md). Those shims are not part of the committed artifact set.

## Fixtures Used

| Fixture | Path | Bytes | SHA-256 | Generation command |
|---|---|---:|---|---|
| `small` (`heap.hprof`) | `resources/test-fixtures/heap.hprof` | 274640397 | `460c75be4cfd22a55077112ba82c9d40ff36a433658f12c93398951b8b6e52d3` | Not preserved in the slice C raw manifest; [run-notes.md](../performance/raw/run-notes.md) records only that the file was generated locally because the optional real-world fixture was absent. |
| `medium` (`synthetic-1gb.hprof`) | `resources/test-fixtures/synthetic-1gb.hprof` | 1586353485 | `dd89c7f70a9e7de436a4d3a2fd8e846559c35350e76f2052ea32842eb5df6a91` | `scripts/generate_synthetic_heap.sh --size-mb 1024 --output resources/test-fixtures/synthetic-1gb.hprof` |
| `large` (`synthetic-4gb.hprof`) | `resources/test-fixtures/synthetic-4gb.hprof` | 6471188323 | `f7683c803a37094782310ee7b8267b8b9e21e5490c971bc6a37d7d52ee8dd7d4` | `scripts/generate_synthetic_heap.sh --size-mb 4096 --output resources/test-fixtures/synthetic-4gb.hprof` |

The `small` fixture's contents confirm the caveat above: its overview output includes `SyntheticHeapApp` classes, so it should be treated as a locally generated small synthetic heap written to `resources/test-fixtures/heap.hprof`, not as the earlier real-world sentinel.

## Results - Wall Time

| Fixture | Tool | Run 1 | Run 2 | Run 3 | Median |
|---|---|---:|---:|---:|---:|
| `small` | `mnemo-overview` | 9.492 | 8.575 | 8.727 | 8.727 |
| `small` | `hprof-slurp` | 2.141 | 2.107 | 2.111 | 2.111 |
| `medium` | `mnemo-overview` | 79.063 | 52.710 | 53.772 | 53.772 |
| `medium` | `hprof-slurp` | 12.665 | 12.328 | 11.096 | 12.328 |
| `large` | `mnemo-overview` | 238.545 | 196.369 | 183.701 | 196.369 |
| `large` | `hprof-slurp` | 44.843 | 46.009 | 45.195 | 45.195 |

## Results - Max RSS

The raw CSV includes a `max_rss_kb` column, but this slice measured RSS through a temporary WSL compatibility shim that sampled `/proc` `VmHWM` because GNU `/usr/bin/time -v` was unavailable. Given the WSL2 `/mnt/d` environment and that temporary shim path, these RSS numbers are not published here as a comparative table. The raw values remain in [results.csv](../performance/raw/results.csv) and [results-overview-hprof.csv](../performance/raw/results-overview-hprof.csv) for forensic reference only.

## Analysis

`hprof-slurp` is faster on every measured fixture in this slice, but this is not an apples-to-apples comparison. Mnemosyne was run as `analyze --mode overview --format json --top-n 100`, which emits a structured overview summary with class histogram data, GC-root counts, and thread-frame buffers. `hprof-slurp` was run as a parse-only `-i <fixture>` summary. The `hprof-slurp` medians are therefore best read as a parser-throughput floor, not as a claim that Mnemosyne should match parse-only output at identical wall time.

Wall time still scales roughly linearly with fixture size for both tools. The fixture sizes grew by about `5.78x` from `small` to `medium` and `4.08x` from `medium` to `large`. Mnemosyne's medians grew by `6.16x` and `3.65x` across those same steps; `hprof-slurp` grew by `5.84x` and `3.67x`. The spread is visible in the raw rows, especially for `mnemo-overview` on `medium` and `large`, which is exactly why the raw CSV is published alongside the median table.

The most important credibility result from this partial run is survival on the 6.47 GB tier. `mnemo-overview` completed all three `large` runs successfully with `exit_code=0`, a median wall time of `196.369s` (about `3.27` minutes), and no reported crash or OOM. On a WSL2 `/mnt/d` setup, that is credible evidence that the overview path streams large dumps robustly even before the native-Linux reference-workstation rerun is complete.

## What Was Not Measured

- Eclipse MAT comparison (not installed in this WSL session).
- Mnemosyne deep mode (wall-clock budget).
- The `10 GB` fixture (disk budget plus no explicit opt-in).
- Equivalence (`Jaccard` top-K class overlap), which is gated on MAT.
- Native Linux execution; these results are WSL-on-NTFS measurements and the reference-workstation rerun remains pending.

## Reproducing These Numbers

Use the slice B harness plus the exact measured subset from slice C:

```bash
cargo build --release -p mnemosyne-cli
export MNEMOSYNE_BIN="$PWD/target/release/mnemosyne-cli"

scripts/generate_synthetic_heap.sh --size-mb 1024 --output resources/test-fixtures/synthetic-1gb.hprof
scripts/generate_synthetic_heap.sh --size-mb 4096 --output resources/test-fixtures/synthetic-4gb.hprof

scripts/bench/run_comparative.sh \
  --fixtures-dir resources/test-fixtures \
  --output-dir docs/performance/raw \
  --runs 3 \
  --tools mnemo-overview,hprof-slurp \
  --fixtures small,medium,large
```

Actual executed per-run command lines are preserved in [docs/performance/raw/runs/](../performance/raw/runs/) JSON artifacts. For example:

- `mnemo-overview`: `/mnt/d/Mnemosyne/target/release/mnemosyne-cli analyze <fixture> --mode overview --format json --top-n 100`
- `hprof-slurp`: `/home/bballer09/.cargo/bin/hprof-slurp -i <fixture>`

To reproduce the published tables exactly, derive them from [results-overview-hprof.csv](../performance/raw/results-overview-hprof.csv) or [results.csv](../performance/raw/results.csv), and read the WSL compatibility-shim caveats in [run-notes.md](../performance/raw/run-notes.md) first.

## Roadmap to Full Reference Results

The remaining work to satisfy the full M7-5 design coverage is:

1. Provision a native Linux (or VM) reference workstation per [reference-spec.md](reference-spec.md).
2. Install Eclipse MAT `1.15.0` and `hprof-slurp` `0.6.3` per [tool-installation.md](tool-installation.md).
3. Generate the `10 GB` fixture.
4. Re-run `scripts/bench/run_comparative.sh` with `--runs 5 --tools mnemo-deep,mnemo-overview,mat,hprof-slurp --fixtures small,medium,large,xlarge`.
5. Run `equivalence.py` for each fixture once MAT is available.
6. Update this report and commit the refreshed raw CSVs under [docs/performance/raw/](../performance/raw/).

## Cross-References

- Design addendum: [milestone-7-5-comparative-benchmarks.md](../design/milestone-7-5-comparative-benchmarks.md)
- Reference workstation spec: [reference-spec.md](reference-spec.md)
- Fixture matrix: [fixtures.md](fixtures.md)
- Tool installation: [tool-installation.md](tool-installation.md)
- Raw artifacts: [docs/performance/raw/](../performance/raw/)
- Raw run notes: [run-notes.md](../performance/raw/run-notes.md)
- Raw measurements: [results.csv](../performance/raw/results.csv), [results-overview-hprof.csv](../performance/raw/results-overview-hprof.csv)