# M7-5.C comparative benchmark run notes

- Date: 2026-04-26
- Branch: `m6-ecosystem-roadmap-restructure`
- Starting commit: `9d1b474`
- WSL distro: Arch Linux on WSL2
- Runs per measured cell: `3` (reduced from the design default `5`)

## Environment and execution notes

- The WSL inventory is recorded verbatim in `environment.txt`.
- The release build produced `target/release/mnemosyne-cli`; there is no `target/release/mnemosyne` binary on this branch.
- Native Linux `python3` and GNU `/usr/bin/time -v` were not available in this WSL environment at run start.
- For this slice, the harness was executed with temporary compatibility shims under `tmp/m7-5c-bin/`:
  - `python3` / `python` forward to the existing Windows Python installation.
  - `gtime` records wall time and `/proc` `VmHWM` peak RSS so `measure_run.sh` can run without `/usr/bin/time -v`.
  - `timeout` strips the extra post-duration `--` that the harness currently passes to GNU `timeout`.
- The checked-out shell scripts also had Windows CRLF line endings, so LF-normalized temporary execution copies were created under `tmp/m7-5c-exec/` for the WSL run.
- These shims and temp execution copies were used only to execute slice C locally; they are not part of the committed raw benchmark artifact set.

## Fixture notes

- `heap.hprof` was generated locally because the optional real-world `resources/test-fixtures/heap.hprof` fixture was absent in this workspace.
- Generated fixtures and exact byte sizes / SHA-256 digests are recorded in `fixtures-inventory.txt`.
- The `10 GB` fixture was skipped for this slice because the execution plan required explicit user opt-in for the xlarge tier.

## Tool availability

- `mnemosyne-cli`: built and runnable in WSL.
- `hprof-slurp`: installed in WSL during this slice and used by the harness if available.
- Eclipse MAT (`ParseHeapDump.sh`): not installed in WSL; MAT cells were skipped.
- Equivalence comparison: skipped because MAT did not run.

## Final execution outcome

- Completed runs:
  - `mnemo-overview` on `small`, `medium`, and `large` fixtures with `N=3` each.
  - `hprof-slurp` on `small`, `medium`, and `large` fixtures with `N=3` each.
- Result status:
  - All recorded rows in the completed harness batch are `status=ok` with `exit_code=0`.
  - The completed source-of-truth CSV for this slice is `results-overview-hprof.csv`; `results.csv` is kept as the canonical copy expected by the slice instructions.
- Skipped work:
  - `mnemo-deep` was not run in this slice. After build, fixture generation, environment compatibility work, and the observed large-tier `mnemo-overview` wall times (`183.701s`, `196.369s`, `238.545s` on the 6.47 GB fixture over WSL `/mnt/d`), I froze scope at the completed overview/hprof batch rather than claiming more coverage than the wall-clock budget realistically allowed.
  - Eclipse MAT was skipped because `ParseHeapDump.sh` was not installed in WSL.
  - Equivalence comparison was skipped because MAT did not run.
  - The `10 GB` fixture remained skipped because this execution plan required explicit user opt-in for the xlarge tier.
- Additional caveat:
  - This slice was executed against the Windows-mounted `/mnt/d/Mnemosyne` workspace inside WSL2, not on a native Linux filesystem. The completed large-tier wall times should therefore be read as WSL-hosted measurements, not as the design doc’s ideal native-Linux reference-workstation numbers.
