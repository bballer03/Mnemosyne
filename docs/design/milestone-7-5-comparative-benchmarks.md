# Milestone 7-5 — Comparative Benchmarks vs Eclipse MAT and hprof-slurp

> **Status:** 🔲 Pending — design entering review. Predecessors M7-1, M7-2, M7-3, M7-4 ✅ complete.
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (slices A, B, D); **User (slice C — actual benchmark execution on the reference workstation)**.
> **Parent:** [milestone-7-production-readiness.md §10](milestone-7-production-readiness.md)
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Last updated:** 2026-04-26

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Slice | M7-5 |
| Phase | M7 — Production Readiness & Scale |
| Type | Credibility (the v0.3.0 release gate) |
| Predecessors | M7-1 ✅ (overview mode), M7-2 ✅ (`ci-check`), M7-3 ✅ (flame graphs), M7-4 ✅ (targeted OQL) |
| Successors | M7-6 (v0.3.0 release) |
| Touched crates | None — this milestone is documentation, shell scripts, and one small CSV-aggregation utility |
| Touched dirs | `scripts/bench/` (new), `docs/benchmarks/` (new), `docs/performance/raw/` (new), `docs/benchmarks.md`, `STATUS.md`, `docs/roadmap.md` |

## 2. Objective

Produce reproducible head-to-head benchmarks of Mnemosyne against Eclipse MAT and hprof-slurp on a shared fixture matrix, and publish the results — including raw data — as a defensible, third-party-reproducible artifact. M7-5 is the **credibility gate** for v0.3.0: all numerical claims that lead the v0.3.0 release notes must be backed by published runs on the reference workstation.

Concretely, the milestone delivers:

1. A documented **reference workstation spec** (CPU, RAM, storage, OS, JVM, MAT version, hprof-slurp version, Mnemosyne build profile).
2. A **fixture matrix** at four sizes (156 MB existing real heap, 1 GB / 4 GB / 10 GB synthetic).
3. A **benchmark harness** (`scripts/bench/`) that runs Mnemosyne deep, Mnemosyne overview, MAT (headless), and hprof-slurp on every fixture and emits raw CSV.
4. A small **output-equivalence calculator** that compares top-N class sets across tools (Jaccard index).
5. A **published comparative report** ([docs/benchmarks/comparative-v0.3.0.md](../benchmarks/comparative-v0.3.0.md)) with results tables, analysis, and explicit caveats.
6. **Raw CSV artifacts** under [docs/performance/raw/](../performance/raw/) so a third party can re-derive the report's tables.

## 3. Context

### 3.1 What credibility v0.3.0 needs to earn

v0.2.0 shipped honest internal benchmarks ([docs/benchmarks.md](../benchmarks.md)) but pointedly avoided head-to-head numbers vs MAT or hprof-slurp. The roadmap ([docs/roadmap.md](../roadmap.md) §4) explicitly lists "Published large-dump proof (10 GB+)" and "Comparative benchmark results versus MAT and hprof-slurp are published with reproducible methodology" as v0.3.0 release gates.

After M7-1, Mnemosyne can stream a 10 GB+ dump in overview mode without exhausting RAM. After M7-2 / M7-3, Mnemosyne owns CI-policy and flame-graph categories MAT does not target. M7-5 turns these capabilities into evidence that survives external scrutiny.

### 3.2 Claims we want to make defensible

The published report should let a reader confirm — by reading the raw data — that:

- **Streaming triage at scale.** Mnemosyne overview mode parses a 10 GB dense synthetic dump in well under one minute on the reference workstation, with peak RSS bounded at or below ~1 GiB. Concrete number TBD by the run; the design commits to publishing the actual measured value.
- **Deep-mode parity on small/medium dumps.** On the existing 156 MB real fixture and the 1 GB synthetic, Mnemosyne deep-mode wall time is competitive with MAT's headless `ParseHeapDump` on the same hardware, with a top-N class-set Jaccard ≥ 0.8 between the two tools.
- **Streaming-overview throughput is comparable to hprof-slurp.** On the 4 GB and 10 GB tiers, Mnemosyne overview-mode wall time and peak RSS are within roughly 2× of hprof-slurp's published numbers on equivalent hardware.
- **MAT does not always finish.** Where MAT runs out of memory or fails on the 10 GB tier under default heap settings, that failure is reported in the comparison table — not hidden.

### 3.3 Claims we explicitly will NOT make

- "Mnemosyne is faster than MAT in all cases." We do not tune to win.
- "Mnemosyne replaces MAT." Deep MAT depth and OQL breadth remain outside M7's scope.
- "These numbers generalize to all hardware." All numbers are pinned to the reference workstation; results may vary.

## 4. Scope

In scope:

- Methodology document (this addendum and the eventual comparative report).
- Fixture generation harness (reusing [scripts/generate_synthetic_heap.sh](../../scripts/generate_synthetic_heap.sh)).
- Benchmark driver scripts under `scripts/bench/`.
- Wall-time measurement via `hyperfine` (already required by [scripts/run_hyperfine_bench.sh](../../scripts/run_hyperfine_bench.sh)).
- Max-RSS measurement via `/usr/bin/time -v` (Linux) and `/proc/<pid>/status` polling fallback (already implemented in [scripts/measure_rss.sh](../../scripts/measure_rss.sh) and [scripts/run_step11_scaling_validation.sh](../../scripts/run_step11_scaling_validation.sh)).
- Peak file-cache measurement via `vmtouch` if available; documented skip with caveat otherwise.
- Output equivalence: top-N class-name set overlap (Jaccard index) between Mnemosyne and MAT.
- Tool installation guide (how to obtain and invoke MAT headless, how to build / install hprof-slurp).
- Comparative results doc + raw CSVs.

Out of scope (see §5).

### 4.1 Categorical claim boundaries

- **Will publish:** wall time, max RSS, peak file-cache (when measurable), top-N Jaccard, byte-count parity (where comparable).
- **Will not publish:** synthetic micro-benchmarks claiming "X% faster", per-feature MAT depth comparisons, JVM-tuning shootouts.

## 5. Non-scope

- **Performance optimization.** M7-5 reports what is. Tuning Mnemosyne to win specific numbers belongs to a follow-up batch and must not be folded into this milestone.
- **MAT bug reporting.** If MAT crashes on the 10 GB tier we record it; we do not file MAT bugs as part of this work.
- **hprof-slurp competitive analysis beyond the comparison table.** A feature-level critique of hprof-slurp is not a deliverable.
- **CI integration of the benchmark harness.** Wiring `scripts/bench/run_comparative.sh` into GitHub Actions is intentionally deferred — the runs are too slow and require too much disk to fit standard CI runners. A separate batch may add a manual workflow_dispatch wrapper later.
- **macOS / Windows reference numbers.** Reference workstation is Linux; cross-platform reproductions are welcome but not part of the published v0.3.0 numbers.
- **Tauri UI benchmark surfacing.** The desktop app (M8-9) will surface benchmark snapshots later; not part of M7-5.

## 6. Reference workstation spec

All v0.3.0 published numbers are produced on the following spec. The user (acting as benchmark operator) confirms or amends this spec before slice C is executed; the comparative report records the *actual* spec used.

### 6.1 Target spec (subject to confirmation in slice A)

| Component | Target spec |
|---|---|
| CPU | x86_64, 8+ physical cores, ≥ 3.5 GHz base clock (e.g., AMD Ryzen 7 5800X / Intel Core i7-12700) |
| RAM | ≥ 32 GiB DDR4 (so a 10 GB heap dump + MAT working set fits comfortably) |
| Storage | NVMe SSD, ≥ 200 GiB free (10 GB fixture × multiple tools × cold-cache rotations) |
| OS | Ubuntu 22.04 LTS or 24.04 LTS, kernel 5.15+ |
| JVM (for fixture gen + MAT) | Eclipse Temurin 17 LTS (`java -version` recorded verbatim) |
| Eclipse MAT | 1.15.0 standalone (linux.gtk.x86_64), invoked via `ParseHeapDump.sh` |
| hprof-slurp | Latest tagged release at the time of the run (commit hash recorded) |
| Mnemosyne | `cargo build --release` from the v0.3.0 release commit; binary at `target/release/mnemosyne-cli` |

### 6.2 Reproducibility statement

The comparative report explicitly states: *"All numbers in this document were produced on the reference workstation described in §6.1 of [milestone-7-5-comparative-benchmarks.md](../design/milestone-7-5-comparative-benchmarks.md). Results on other hardware may vary by 2× or more in either direction."*

## 7. Fixture matrix

| Fixture | Size | Source | Generation command | What it stresses |
|---|---|---|---|---|
| F-156M | ~156 MB | Existing real heap | `resources/test-fixtures/heap.hprof` (already in repo) | Real-world heap shape; deep-mode parity baseline; `ObjectGraph` + dominator + leaks |
| F-1G | ~1 GB | Synthetic dense | `scripts/generate_synthetic_heap.sh --size-mb 1024 --output fixtures/synthetic-1gb.hprof` | Deep-mode large-tier RSS; analyzer scaling |
| F-4G | ~4 GB | Synthetic dense | `scripts/generate_synthetic_heap.sh --size-mb 4096 --output fixtures/synthetic-4gb.hprof` | Boundary at default `OVERVIEW_AUTO_THRESHOLD_BYTES` (4 GiB); auto-mode resolution |
| F-10G | ~10 GB | Synthetic dense | `scripts/generate_synthetic_heap.sh --size-mb 10240 --output fixtures/synthetic-10gb.hprof` | Streaming-only tier; MAT failure-mode capture |

### 7.1 Expected shape per fixture

The synthetic generator at [scripts/java/SyntheticHeapApp.java](../../scripts/java/SyntheticHeapApp.java) produces dense object graphs whose object/class counts are documented as part of slice A in the comparative report. Slice A measures and records:

- Total `INSTANCE_DUMP` count
- Total `OBJECT_ARRAY_DUMP` count
- Distinct loaded class count
- Total bytes (file size)

For each fixture. These become reference numbers in the comparative report.

### 7.2 Determinism note

The synthetic generator currently does not accept a seed argument. **Open question (§15):** add `--seed` to `generate_synthetic_heap.sh` so re-runs produce byte-identical fixtures. If determinism is added, it is a small CLI-only change and lives inside slice A; if not, the report includes the SHA-256 of each generated fixture so a third party can verify byte-for-byte parity with the published runs.

## 8. Tools matrix

Each tool is invoked through a single thin wrapper so timing/RSS measurement is uniform.

| Tool | Mode | Invocation template | Expected output | Timeout | Failure mode |
|---|---|---|---|---|---|
| Mnemosyne | Deep | `mnemosyne-cli analyze <heap> --mode deep --format json` | `AnalyzeResponse` JSON | 30 min | Non-zero exit; record stderr |
| Mnemosyne | Overview | `mnemosyne-cli analyze <heap> --mode overview --top-n 100 --format json` | `AnalyzeResponse` JSON with `mode=overview`, `Partial` provenance | 10 min | Non-zero exit; record stderr |
| Eclipse MAT | Headless parse | `ParseHeapDump.sh <heap> org.eclipse.mat.api:suspects org.eclipse.mat.api:overview` (configurable via `MAT_HOME`, `MAT_VMARGS`) | MAT's `<heap>_Suspects.zip`, `<heap>_System_Overview.zip`, indexes in `<heap>_Indexes/` | 60 min | Non-zero exit, OOM, or `OutOfMemoryError` in MAT log; record stderr + log; mark cell `OOM` in results |
| hprof-slurp | Default | `hprof-slurp -i <heap> --top 100` | Stdout summary text + JSON if `--format json` is supported by the installed version (recorded by harness) | 10 min | Non-zero exit; record stderr |

### 8.1 MAT invocation details

`ParseHeapDump.sh` (Linux) or `ParseHeapDump.bat` (Windows) is the official headless entry point. The harness:

1. Discovers `MAT_HOME` from env; errors if unset.
2. Allows `MAT_VMARGS` (default `-Xmx16g`) so MAT can take the 10 GB tier without us having to silently re-tune mid-run.
3. Captures MAT's workspace logs alongside the timing output.
4. Cleans `<heap>_Indexes/` between cold-cache rotations so MAT's first-run cost is included in the measurement.

### 8.2 hprof-slurp invocation details

hprof-slurp is installed via `cargo install hprof-slurp` or built from source; the harness records `hprof-slurp --version` and the binary's SHA-256 in the run metadata. If a hprof-slurp release does not match the documented behavior (e.g., flag rename), the harness fails fast with a clear message; the comparative report pins the exact version used.

## 9. Metrics & measurement methodology

### 9.1 Wall time

- Tool: `hyperfine` (already a soft dependency — see [scripts/run_hyperfine_bench.sh](../../scripts/run_hyperfine_bench.sh) which `exit 0`s with a skip if hyperfine is missing).
- N = **5 runs minimum** per (tool × fixture × cache state) cell.
- Warmup: 1 run for warm-cache cells; 0 runs for cold-cache cells (`hyperfine --prepare` drops caches before each run).
- Outliers: report median + min + max + standard deviation. Do not drop outliers; the raw CSV records every run.

### 9.2 Max RSS

- Linux: `/usr/bin/time -v` parsing `Maximum resident set size (kbytes)` (already implemented in [scripts/measure_rss.sh](../../scripts/measure_rss.sh) `profile_with_time`).
- Linux fallback: `/proc/<pid>/status` `VmHWM` polling at 100 ms intervals (already implemented as `profile_with_proc`).
- macOS / Windows alternative: documented as out of scope for the published v0.3.0 numbers; if a contributor wants to reproduce, the harness prints "RSS measurement requires Linux `/usr/bin/time -v` or `/proc`" and exits 0 without numbers.

### 9.3 Peak file-cache

- Tool: `vmtouch -t <heap>` before each cold-cache run to evict; `vmtouch <heap>` after the run to read resident pages.
- If `vmtouch` is not installed, the harness logs `vmtouch: not installed; peak file-cache skipped` and the report carries an explicit `n/a` in the corresponding cells with a footnote pointing at this method note.

### 9.4 Output equivalence (top-N class-set Jaccard)

- Inputs: Mnemosyne deep-mode top-100 classes by shallow size; MAT headless top-100 classes by shallow heap (from `<heap>_Top_Components.csv` or `<heap>_System_Overview.zip` extract).
- Computation: |A ∩ B| / |A ∪ B| over class-name sets.
- Acceptable threshold for the comparison report: ≥ 0.8 on F-156M and F-1G. Lower values are reported but flagged for follow-up; they do not gate the milestone.
- For Mnemosyne overview vs hprof-slurp on F-4G and F-10G: same Jaccard computation, threshold ≥ 0.7 (overview's `approx_shallow_bytes` is HPROF-payload-attributed and may rank slightly differently from hprof-slurp's bucketing).

### 9.5 Parser correctness (byte counts)

Where comparable: total `INSTANCE_DUMP` bytes attributed across classes should match between Mnemosyne overview mode and hprof-slurp (both attribute by HPROF record payload). The report tabulates per-tool totals and flags any difference > 0.1% as a parser-correctness concern.

### 9.6 Cache state matrix

Every (tool × fixture) cell is run twice:

- **Cold cache:** `vmtouch -e <heap>` before the run; first run after eviction.
- **Warm cache:** 1 warmup run, then N=5 measured runs back-to-back.

Both rows are published. Cold-cache numbers reflect first-touch user experience; warm-cache numbers reflect the steady-state.

## 10. Harness scripts

All harness scripts live under `scripts/bench/` (new directory) and reuse existing helpers in `scripts/`. None of them touch Rust source; the harness does not require a `cargo` rebuild between runs (it consumes a pre-built `target/release/mnemosyne-cli`).

### 10.1 `scripts/bench/run_comparative.sh` (top-level driver)

Responsibilities:

1. Verify reference-workstation prerequisites: `hyperfine`, `/usr/bin/time`, `vmtouch` (warns if missing), `MAT_HOME`, `hprof-slurp` on PATH.
2. Iterate the fixture × tool × cache-state matrix.
3. Delegate per-cell measurement to `scripts/bench/measure_run.sh`.
4. Aggregate per-cell CSV rows into one master CSV per fixture under `docs/performance/raw/<fixture>.csv`.
5. Emit a summary TSV (`docs/performance/raw/summary.tsv`) similar to [scripts/run_step11_scaling_validation.sh](../../scripts/run_step11_scaling_validation.sh) `summary.tsv`.

CLI:

```text
scripts/bench/run_comparative.sh \
    --fixtures-dir <dir> \
    --output-dir docs/performance/raw \
    [--tools "mnemosyne-deep mnemosyne-overview mat hprof-slurp"] \
    [--cache-states "cold warm"] \
    [--runs 5] [--warmup 1] \
    [--mat-home <dir>] [--mat-vmargs "-Xmx16g"]
```

Exit codes: 0 on full success; non-zero if any tool failed *unexpectedly* (i.e., other than a documented OOM cell, which is recorded in CSV and continues the run).

### 10.2 `scripts/bench/measure_run.sh` (single-cell wrapper)

Responsibilities:

- Drop file cache via `vmtouch -e <heap>` if `--cache cold`.
- Run the tool under `/usr/bin/time -v`, capturing wall time, max RSS, exit status, stdout SHA-256.
- Run the tool again under `hyperfine --runs N --warmup W` for statistical wall-time distribution.
- Emit one CSV row to stdout: `tool,fixture,cache_state,run_index,wall_seconds,max_rss_kib,vmtouch_resident_pages,exit_code,stdout_sha256,timestamp`.

CLI:

```text
scripts/bench/measure_run.sh \
    --tool {mnemosyne-deep|mnemosyne-overview|mat|hprof-slurp} \
    --fixture <heap> \
    --cache {cold|warm} \
    --runs 5 --warmup 1 \
    [--mat-home <dir>] [--mat-vmargs "-Xmx16g"]
```

### 10.3 `scripts/bench/equivalence.py` (output equivalence)

Responsibilities:

- Read Mnemosyne JSON output (deep or overview), extract top-100 class names.
- Read MAT `<heap>_Top_Components.csv` (or hprof-slurp top-N stdout/JSON), extract top-100 class names.
- Compute Jaccard index and pairwise rank correlation (Spearman) on the intersection.
- Emit a CSV row: `fixture,tool_a,tool_b,top_n,jaccard,spearman_rho`.

Python is acceptable here because:

- This is not on the v0.3.0 ship path (it is an analysis tool, not a runtime).
- Python is on every reference workstation and avoids adding a Rust binary just for set arithmetic.
- The script is < 100 lines and has no third-party dependencies (uses only stdlib `csv`, `json`, `argparse`, `statistics`).

If the M7-5 implementer prefers Rust, a one-binary equivalent under `scripts/bench/equivalence.rs` invoked via `cargo run --bin equivalence` is acceptable; the choice is left to slice B.

### 10.4 Reused existing scripts

- [scripts/generate_synthetic_heap.sh](../../scripts/generate_synthetic_heap.sh) — generates F-1G / F-4G / F-10G fixtures.
- [scripts/measure_rss.sh](../../scripts/measure_rss.sh) — Mnemosyne-only RSS reference; not invoked by the comparative harness directly but kept for parity validation.
- [scripts/run_hyperfine_bench.sh](../../scripts/run_hyperfine_bench.sh) — Mnemosyne-only timing reference; not invoked by the comparative harness directly.
- [scripts/run_step11_scaling_validation.sh](../../scripts/run_step11_scaling_validation.sh) — pattern source for `run_comparative.sh` (TSV summary, ratio formatters, awk helpers).

## 11. Output artifacts

### 11.1 Comparative report

`docs/benchmarks/comparative-v0.3.0.md` (new directory `docs/benchmarks/`). Required sections:

1. **Reference workstation spec** (full table, frozen at run time).
2. **Tool versions** (Mnemosyne commit, MAT version, hprof-slurp version + commit SHA, JVM version, OS kernel).
3. **Fixture inventory** (size in bytes, SHA-256, instance/class counts).
4. **Wall-time results** (median + IQR for each tool × fixture × cache cell).
5. **Max-RSS results** (peak KiB; ratio vs dump size).
6. **Peak file-cache results** (resident pages or `n/a`).
7. **Output equivalence** (Jaccard table, byte-count parity table).
8. **Failures** (every OOM, timeout, or non-zero-exit cell with stderr excerpt).
9. **Analysis** (3–5 paragraphs of honest interpretation; see §12).
10. **Reproduction instructions** (link to §13 of this addendum).

### 11.2 Raw CSVs

`docs/performance/raw/` (new directory):

- `summary.tsv` — one row per (fixture × tool × cache-state).
- `<fixture>.csv` — per-fixture detail (every individual run, all metrics).
- `equivalence.csv` — every Jaccard / Spearman comparison.
- `tool-versions.json` — captured tool versions and binary SHA-256s for the run.

### 11.3 SVG plots (optional)

Plots are **optional** for v0.3.0. If the milestone operator wants them:

- Use a single Python script (`scripts/bench/plot.py`) that reads `summary.tsv` and emits SVG via `matplotlib`.
- Ship the SVGs alongside the CSV in `docs/performance/raw/plots/`.
- Reference them inline in `comparative-v0.3.0.md`.

If the operator skips plots, the report is still complete; tables alone suffice.

## 12. Honesty contract

The comparative report binds itself to the following rules. Slice D enforces these in the report content; slice C enforces them in execution.

1. **No cherry-picking.** Every measured run is published in the raw CSV. Outliers are not removed; the report quotes median, min, max, and stddev so the spread is visible.
2. **Failures are first-class.** MAT OOMs, hprof-slurp parser errors, and Mnemosyne timeouts appear in the table, not as footnotes. The cell shows `OOM`, `TIMEOUT`, or `ERROR` with a link to the captured stderr.
3. **Shipped builds only.** Mnemosyne is benchmarked from `cargo build --release` against the v0.3.0 release commit. No debug binaries, no `RUSTFLAGS=-C target-cpu=native` tuning unless the same flag is documented and applied uniformly.
4. **Provenance carries through.** When Mnemosyne overview mode is benchmarked, the report annotates the corresponding rows with the `Partial` provenance marker text — readers must not be able to read the table and conclude overview-mode numbers cover deep-mode work.
5. **Caveats are loud.** The report's leading paragraph names every known caveat: hardware specificity, synthetic-vs-real fixture difference, MAT GUI vs headless behavior, hprof-slurp output-format differences, vmtouch availability.
6. **No competitive theater.** Mnemosyne is not framed as "winning" against MAT or hprof-slurp; the report frames each tool against its own design goals.

## 13. Reproducibility checklist

Verbatim instructions a third party can follow:

```bash
# 1. Acquire and unpack Eclipse MAT
wget https://download.eclipse.org/mat/<version>/<archive>.tar.gz
tar -xzf <archive>.tar.gz
export MAT_HOME=$PWD/mat
export MAT_VMARGS="-Xmx16g"

# 2. Install hprof-slurp
cargo install hprof-slurp     # or: git clone <repo> && cargo build --release

# 3. Build Mnemosyne at the v0.3.0 commit
git checkout v0.3.0
cargo build --release

# 4. Generate fixtures (run from repo root)
mkdir -p fixtures
scripts/generate_synthetic_heap.sh --size-mb 1024  --output fixtures/synthetic-1gb.hprof
scripts/generate_synthetic_heap.sh --size-mb 4096  --output fixtures/synthetic-4gb.hprof
scripts/generate_synthetic_heap.sh --size-mb 10240 --output fixtures/synthetic-10gb.hprof

# 5. Run the comparative harness
scripts/bench/run_comparative.sh \
    --fixtures-dir fixtures \
    --output-dir docs/performance/raw \
    --runs 5 --warmup 1

# 6. Compute output equivalence
scripts/bench/equivalence.py \
    --raw-dir docs/performance/raw \
    --output docs/performance/raw/equivalence.csv

# 7. Diff against published numbers
diff -u docs/performance/raw/summary.tsv published-summary.tsv
```

The published `comparative-v0.3.0.md` includes the exact MAT version URL, hprof-slurp commit SHA, and Mnemosyne git tag in §6.

## 14. Slice breakdown

M7-5 differs from prior M7 slices in a critical way: **slice C (actual benchmark execution) cannot run inside an agent session.** Running 4 tools × 4 fixtures × 2 cache states × 5 runs = 160 measured runs, including a 10 GB fixture that MAT may take 30–60 minutes per run to parse, requires hardware access, time, and disk that no agent session has. Slices A, B, and D are agent-doable; slice C is the user's responsibility on a real reference workstation, with the agent producing the harness and analysis-template.

### Slice M7-5.A — Fixture generation, reference spec, tool installation guide

**Owner:** Implementation Agent.
**Files affected:**

- `docs/design/milestone-7-5-comparative-benchmarks.md` — confirm/refine §6.1 reference spec based on actual hardware available to the user (ask if uncertain).
- `docs/benchmarks/tool-installation.md` (new) — step-by-step MAT install, hprof-slurp install, JDK install for fixture generation.
- `scripts/generate_synthetic_heap.sh` — *optionally* add `--seed` argument for deterministic fixtures (see §15 open question). If accepted, the slice is still small (single CLI flag + pass-through to the Java app).
- `scripts/java/SyntheticHeapApp.java` — same; if `--seed` is added, plumb through.

**Test gate:** Running `scripts/generate_synthetic_heap.sh --size-mb 256 --output /tmp/probe.hprof` produces a parseable HPROF file (`mnemosyne-cli parse /tmp/probe.hprof` succeeds). If `--seed` is added, two runs with the same seed produce byte-identical files (or files with identical SHA-256 modulo HPROF timestamp — to be confirmed in slice A).

**Output:** PR includes the spec table, tool-installation guide, and any seed plumbing. No benchmark numbers yet.

**Target size:** ~150 LOC of script changes (mostly optional seed plumbing) + ~200 LOC of docs.

### Slice M7-5.B — Harness scripts

**Owner:** Implementation Agent.
**Files affected:**

- `scripts/bench/run_comparative.sh` (new).
- `scripts/bench/measure_run.sh` (new).
- `scripts/bench/equivalence.py` (new) — or `scripts/bench/equivalence.rs` if the implementer prefers Rust; both are acceptable.
- `scripts/bench/README.md` (new) — what each script does, which deps are required, how to invoke.

**Test gate:**

- `bash -n scripts/bench/run_comparative.sh && bash -n scripts/bench/measure_run.sh` (syntax check — full execution requires the reference workstation).
- `python3 -m py_compile scripts/bench/equivalence.py` (or `cargo check` if the Rust variant is chosen).
- A *self-test* invocation: `scripts/bench/run_comparative.sh --fixtures-dir resources/test-fixtures --tools mnemosyne-deep --cache-states warm --runs 1 --warmup 0 --output-dir /tmp/m7-5-self-test` produces a valid `summary.tsv` with one row. This is doable in an agent session because it only exercises Mnemosyne on the existing 156 MB fixture.
- `equivalence.py --help` prints usage cleanly.

**Output:** Harness scripts ready for slice C. No benchmark numbers yet beyond the self-test row.

**Target size:** ~400 LOC of shell + ~80 LOC of Python (or Rust) + ~80 LOC of README.

### Slice M7-5.C — Run benchmarks; capture raw data

**Owner:** **User** (acting as benchmark operator on the reference workstation). The agent does not execute this slice.

**Status when slice B lands:** "harness ready; awaits user execution; publish results when available."

**What the user does:**

1. Pull the v0.3.0 release branch with slices A and B merged.
2. Confirm the reference workstation matches §6.1 (or amend the doc with the actual spec).
3. Generate F-1G, F-4G, F-10G fixtures.
4. Run `scripts/bench/run_comparative.sh` end-to-end (expected duration: 8–24 hours wall clock on the reference workstation, dominated by MAT on F-10G and cold-cache rotations).
5. Inspect `docs/performance/raw/summary.tsv` and the per-fixture CSVs for sanity (no all-zero columns, no missing tools, OOM cells correctly recorded).
6. If metrics are unreliable (e.g., a tool fails for an environmental reason rather than a real failure), iterate on the harness — slice B remains open until slice C produces a clean run.
7. Hand the raw CSVs back to the agent (or the Implementation Agent) to drive slice D.

**Slice C exit criterion:** `docs/performance/raw/summary.tsv` exists, contains one row per (fixture × tool × cache-state) cell, and every cell either has a measured value or a documented `OOM`/`TIMEOUT`/`ERROR` marker.

### Slice M7-5.D — Publish comparative report; update STATUS / roadmap

**Owner:** Implementation Agent (with Documentation Sync handoff at the end).
**Files affected:**

- `docs/benchmarks/comparative-v0.3.0.md` (new) — fully populated with the slice-C raw data per §11.1.
- `docs/benchmarks.md` — add a "Comparative results" section with a one-paragraph summary and a link to `comparative-v0.3.0.md`.
- `STATUS.md` — mark M7-5 ✅; advance test count if any (none expected — this slice adds no Rust tests).
- `docs/roadmap.md` — flip M7-5 to ✅; update the "Status detail" prose; remove the "Comparative benchmarks pending" caveats from §1 and §3 of the roadmap.
- `docs/design/milestone-7-production-readiness.md` §3 scope table — flip M7-5 row to ✅ with the slice-D commit hash.
- `docs/design/milestone-7-production-readiness.md` §14 — append `### 14.5 Post-M7-5 update`.
- This addendum (`milestone-7-5-comparative-benchmarks.md`) — flip §1 status to ✅ shipped with commits.

**Test gate:**

- `cargo check`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` clean (no Rust changes expected; this is a smoke test that the doc-only batch did not break the workspace).
- Markdown-lint clean on the new doc (if a markdown linter is configured).
- Every numerical claim in `comparative-v0.3.0.md` traces back to a row in `docs/performance/raw/summary.tsv` (manual self-review checklist in the PR).

**Target size:** ~600 LOC of documentation, depending on table width.

### 14.1 Slice ownership summary

| Slice | Agent-doable? | Owner | Blocking |
|---|---|---|---|
| M7-5.A | ✅ Yes | Implementation Agent | — |
| M7-5.B | ✅ Yes (modulo self-test on F-156M) | Implementation Agent | A |
| M7-5.C | ❌ **No** — requires reference workstation, ~24 h wall clock, 200 GiB disk | **User** | B |
| M7-5.D | ✅ Yes | Implementation Agent | C |

## 15. Risks and open questions

| # | Risk / question | Mitigation / decision |
|---|---|---|
| 1 | MAT GUI mode vs `ParseHeapDump.sh` headless mode warm/cold-cache behavior differs (GUI primes indexes interactively). | Headless only. The report explicitly states "MAT measurements use `ParseHeapDump.sh`; interactive GUI workflow is not benchmarked." |
| 2 | hprof-slurp's stdout / JSON format may differ from Mnemosyne's, complicating Jaccard computation. | `equivalence.py` normalizes both sides to a sorted set of fully-qualified class names and a parallel size-rank list; format differences live in the parser, not the comparator. |
| 3 | 10 GB fixture generation requires ~15 GiB JVM heap (`--xmx-mb 15616` per the script's heuristic) and ≥ 30 GiB free disk for the dump + temp. | Documented in §13; slice A's tool-installation guide states the disk requirement up front. If the reference workstation cannot fit it, the milestone publishes 156 MB / 1 GB / 4 GB tiers and explicitly notes the 10 GB tier as deferred. |
| 4 | macOS RSS measurement (no `/proc`, different `/usr/bin/time` semantics) is unreliable. | Reference workstation is Linux. macOS reproductions are welcome but not part of the published v0.3.0 numbers; the harness errors clearly on macOS. |
| 5 | Synthetic dump determinism: `generate_synthetic_heap.sh` does not currently take a `--seed`. Two runs may produce different fixtures, which weakens reproducibility claims. | Slice A optionally adds `--seed`. If the underlying Java app uses non-deterministic JVM behavior (e.g., `HashMap` iteration order), the report falls back to publishing per-fixture SHA-256 so a third party can verify byte-for-byte parity. |
| 6 | hprof-slurp version drift between when the harness is written and when the user runs it. | Harness records `hprof-slurp --version` and binary SHA-256; report pins the exact version. If a later run uses a different version, that is a new run, not the published one. |
| 7 | MAT may simply OOM on F-10G even with `-Xmx16g`. | Acceptable; the report documents the failure transparently. The honesty contract (§12) requires it. |
| 8 | `vmtouch` is not installed on a fresh reference workstation. | Harness logs and skips peak file-cache cells; report shows `n/a` with a footnote. Not a milestone blocker. |
| 9 | Slice C duration (8–24 h) makes iteration painful if the harness has bugs. | Slice B includes a self-test invocation against F-156M only that exercises every code path of `run_comparative.sh` and `measure_run.sh`. The user runs the self-test before launching the full matrix. |
| 10 | Comparative report becomes stale as MAT and hprof-slurp release new versions. | The report is pinned to v0.3.0; future versions get their own dated `docs/benchmarks/comparative-vX.Y.Z.md`. The general benchmarks doc ([docs/benchmarks.md](../benchmarks.md)) links to all of them. |

## 16. Implementation readiness verdict

**READY** — this addendum specifies the reference workstation spec, fixture matrix, tool matrix, metrics methodology, harness scripts, output artifacts, honesty contract, reproducibility checklist, and a 4-slice breakdown with explicit user-vs-agent ownership boundaries. M7-1 through M7-4 are complete; nothing else blocks M7-5.

The Implementation Agent may begin with **Slice M7-5.A (fixture generation + reference workstation spec confirmation + tool installation guide)** as the first task. Slice M7-5.B (harness scripts) follows. Slice M7-5.C (the actual benchmark run) is the user's responsibility on the reference workstation; the milestone enters "harness ready; awaits user execution" status when slice B lands. Slice M7-5.D (publish report + update STATUS / roadmap) runs once slice C produces clean raw data.

---

## Mandatory handoff fields (Design Consulting → Orchestration)

1. **Task received:** Pre-coding design gate for M7-5 — Comparative Benchmarks vs Eclipse MAT and hprof-slurp.
2. **Scope:** Update parent M7 doc (status block, scope table M7-4 row, §14 readiness append). Create this addendum (`docs/design/milestone-7-5-comparative-benchmarks.md`) with all 16 required sections.
3. **Non-scope:** No source-code edits in `core/` or `cli/`. No Cargo.toml changes. No script edits (slice A may optionally add `--seed` to `generate_synthetic_heap.sh`, but that is implementation work for the next agent, not for this design pass). No actual benchmark execution.
4. **Files inspected:** [docs/design/milestone-7-production-readiness.md](milestone-7-production-readiness.md), [docs/design/milestone-7-4-oql-targeted-expansion.md](milestone-7-4-oql-targeted-expansion.md), [docs/benchmarks.md](../benchmarks.md), [docs/roadmap.md](../roadmap.md), [scripts/generate_synthetic_heap.sh](../../scripts/generate_synthetic_heap.sh), [scripts/measure_rss.sh](../../scripts/measure_rss.sh), [scripts/run_hyperfine_bench.sh](../../scripts/run_hyperfine_bench.sh), [scripts/run_step11_scaling_validation.sh](../../scripts/run_step11_scaling_validation.sh).
5. **Files owned:** [docs/design/milestone-7-production-readiness.md](milestone-7-production-readiness.md) (status block + §3 scope table M7-5 row + §14.4 append), [docs/design/milestone-7-5-comparative-benchmarks.md](milestone-7-5-comparative-benchmarks.md) (this file, full ownership).
6. **Changes made:** Status block updated (M7-1..M7-4 ✅, M7-5 entering design); §3 scope table M7-4 row marked ✅ shipped with the seven supplied commits and M7-5 row points at this addendum; §14 gained a new §14.4 Post-M7-4 update paragraph that announces M7-5 design start and explains the agent-vs-user execution split. This addendum was created with all 16 required sections and a `READY` verdict.
7. **Risks/blockers:** None for the design gate itself. Slice C requires hardware the agent does not have; that boundary is documented in §14 of this addendum and in §14.4 of the parent doc.
8. **Follow-up required:** Implementation Agent picks up Slice M7-5.A. Documentation Sync runs at the end of slice D.
9. **Recommended next agent:** **Implementation Agent** for the design-gate commit (this batch's two file changes) and then for **Slice M7-5.A**.