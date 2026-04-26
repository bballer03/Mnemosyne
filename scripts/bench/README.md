# M7-5 Comparative Benchmark Harness

This directory contains the slice B harness scripts for the comparative benchmark workflow defined in `docs/design/milestone-7-5-comparative-benchmarks.md`.

The published benchmark path is Linux-first. Windows users should run these scripts through WSL2 so `/usr/bin/time -v`, GNU `timeout`, and the shell runtime behave like the reference workstation.

## Quick start

1. Build Mnemosyne and install the external benchmark tools described in `docs/benchmarks/tool-installation.md`.
2. Export the tool locations:

	 ```bash
	 export MNEMOSYNE_BIN="$PWD/target/release/mnemosyne-cli"
	 export MAT_HOME="$HOME/tools/mat-1.15.0"
	 export MAT_VMARGS="-Xmx16g"
	 ```

3. Generate the synthetic fixtures listed in `docs/benchmarks/fixtures.md`.

	 ```bash
	 mkdir -p fixtures
	 scripts/generate_synthetic_heap.sh --size-mb 1024 --output fixtures/synthetic-1gb.hprof
	 scripts/generate_synthetic_heap.sh --size-mb 4096 --output fixtures/synthetic-4gb.hprof
	 scripts/generate_synthetic_heap.sh --size-mb 10240 --output fixtures/synthetic-10gb.hprof
	 ```

4. Run the comparative harness.

	 ```bash
	 scripts/bench/run_comparative.sh \
		 --fixtures-dir fixtures \
		 --output-dir docs/performance/raw \
		 --runs 5
	 ```

5. Compute a single MAT-vs-Mnemosyne overlap score from one deep-mode run artifact.

	 ```bash
	 scripts/bench/equivalence.py \
		 --mnemo docs/performance/raw/runs/small-mnemo-deep-run1.stdout \
		 --mat resources/test-fixtures/heap.hprof_Suspects.zip \
		 --top-k 10
	 ```

## Scripts

### `measure_run.sh`

Purpose: wrap one tool invocation, measure wall time and max RSS, and emit a structured JSON record.

Usage:

```bash
scripts/bench/measure_run.sh \
	--tool mnemo-deep \
	--fixture resources/test-fixtures/heap.hprof \
	--output-dir docs/performance/raw/runs \
	--label small-mnemo-deep-run1 \
	-- ./target/release/mnemosyne-cli analyze resources/test-fixtures/heap.hprof --mode deep --format json
```

Inputs:

- `--tool`: audit label recorded in the JSON output.
- `--fixture`: existing fixture path.
- `--output-dir`: writable directory for JSON/stdout/stderr artifacts.
- `--label`: basename for `<label>.json`, `<label>.stdout`, and `<label>.stderr`.
- `--timeout-seconds`: optional timeout override; defaults to `1800`.
- `-- <command...>`: the exact command to execute.

Behavior:

- Uses GNU `time -v` (`/usr/bin/time` on Linux or `gtime` on macOS) to capture wall time and max RSS.
- Uses GNU `timeout` or `gtimeout` for the wall-clock limit.
- Treats non-zero exit codes and timeouts as first-class results. They are recorded in JSON, and the wrapper still returns `0` unless wrapper setup itself failed.

Outputs:

- `<output-dir>/<label>.json`
- `<output-dir>/<label>.stdout`
- `<output-dir>/<label>.stderr`
- One JSON object on stdout, identical to `<label>.json`

JSON schema:

```json
{
	"tool": "mnemo-deep",
	"fixture": "resources/test-fixtures/heap.hprof",
	"fixture_size_bytes": 163577856,
	"label": "small-mnemo-deep-run1",
	"status": "ok",
	"wall_time_seconds": 12.345678,
	"max_rss_kb": 654321,
	"exit_code": 0,
	"timeout_seconds": 1800,
	"timestamp_utc": "2026-04-26T12:34:56Z",
	"stdout_path": "docs/performance/raw/runs/small-mnemo-deep-run1.stdout",
	"stderr_path": "docs/performance/raw/runs/small-mnemo-deep-run1.stderr",
	"json_path": "docs/performance/raw/runs/small-mnemo-deep-run1.json",
	"command": "./target/release/mnemosyne-cli analyze resources/test-fixtures/heap.hprof --mode deep --format json"
}
```

Status values:

- `ok`: measured command exited `0`
- `error`: measured command exited non-zero
- `timeout`: measured command hit the configured timeout

### `run_comparative.sh`

Purpose: iterate the fixture x tool x run matrix, call `measure_run.sh` for each cell, and aggregate the source-of-truth CSV.

Usage:

```bash
scripts/bench/run_comparative.sh \
	--fixtures-dir fixtures \
	--output-dir docs/performance/raw \
	--runs 5 \
	--tools mnemo-deep,mnemo-overview,mat,hprof-slurp \
	--fixtures small,medium,large,xlarge
```

Fixture mapping:

- `small`: `<fixtures-dir>/heap.hprof` if present, otherwise `resources/test-fixtures/heap.hprof`
- `medium`: `<fixtures-dir>/synthetic-1gb.hprof`
- `large`: `<fixtures-dir>/synthetic-4gb.hprof`
- `xlarge`: `<fixtures-dir>/synthetic-10gb.hprof`

Tool command templates embedded in the script:

- `mnemo-deep`: `mnemosyne-cli analyze <fixture> --mode deep --format json`
- `mnemo-overview`: `mnemosyne-cli analyze <fixture> --mode overview --format json --top-n 100`
- `mat`: `ParseHeapDump.sh <fixture> org.eclipse.mat.api:suspects org.eclipse.mat.api:overview`
- `hprof-slurp`: `hprof-slurp -i <fixture>`

External tool handling:

- `mnemo-deep` and `mnemo-overview` require `MNEMOSYNE_BIN` or a resolvable `mnemosyne-cli` binary.
- `mat` is skipped with a warning if `MAT_HOME/ParseHeapDump.sh` is unavailable.
- `hprof-slurp` is skipped with a warning if `hprof-slurp` is not on `PATH`.

Outputs:

- `<output-dir>/results.csv` as the source-of-truth aggregate
- `<output-dir>/runs/*.json`, `*.stdout`, `*.stderr` from `measure_run.sh`
- A summary table on stdout showing median wall time, median RSS, and success rate per fixture/tool pair

CSV schema:

| Column | Meaning |
|---|---|
| `fixture` | Fixture label: `small`, `medium`, `large`, `xlarge` |
| `fixture_size_bytes` | Size of the input heap dump in bytes |
| `tool` | Tool label used by the harness |
| `run_index` | One-based run index within the fixture/tool cell |
| `wall_time_seconds` | Wall-clock duration measured by GNU `time -v` |
| `max_rss_kb` | Maximum resident set size in KiB |
| `exit_code` | Actual exit code from the measured command |
| `status` | `ok`, `error`, or `timeout` |
| `timestamp_utc` | UTC timestamp for the run record |

Honesty contract notes:

- The CSV is the source of truth.
- Failed runs remain in `results.csv` with their actual exit code and status.
- Missing optional tools are reported as warnings and skipped, not silently treated as successes.

### `equivalence.py`

Purpose: compute top-K class-set Jaccard overlap between one Mnemosyne deep-mode JSON artifact and one MAT retained-heap artifact.

Usage:

```bash
scripts/bench/equivalence.py \
	--mnemo docs/performance/raw/runs/small-mnemo-deep-run1.stdout \
	--mat resources/test-fixtures/heap.hprof_Suspects.zip \
	--top-k 10
```

Inputs:

- `--mnemo`: deep-mode Mnemosyne JSON output. The parser expects `histogram.entries[*].retained_size` and will fail if you pass overview output.
- `--mat`: one of:
	- an extracted MAT HTML file with a retained-heap table
	- an extracted MAT CSV file with class-name and retained-heap columns
	- the original MAT zip artifact containing one of those files

MAT parser expectation:

- The selected MAT file must contain both a class-name column and a retained-heap column.
- If the file exists but that table cannot be found, the script exits `2` with a useful error instead of silently returning zero overlap.

Output schema:

```json
{
	"fixture": "resources/test-fixtures/heap.hprof",
	"top_k": 10,
	"mnemo_classes": ["com.example.Cache", "java.lang.String"],
	"mat_classes": ["com.example.Cache", "java.lang.String"],
	"intersection": ["com.example.Cache", "java.lang.String"],
	"jaccard": 1.0
}
```

The comparison intentionally targets Mnemosyne-vs-MAT only. hprof-slurp output is not normalized here because its aggregation surface is different enough to make a one-file equivalence score misleading.

## Cross references

- Design source of truth: `docs/design/milestone-7-5-comparative-benchmarks.md`
- Reference workstation contract: `docs/benchmarks/reference-spec.md`
- Tool installation: `docs/benchmarks/tool-installation.md`
- Fixture matrix: `docs/benchmarks/fixtures.md`