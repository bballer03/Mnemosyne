# M7-5 Reference Workstation Specification

This document freezes the target environment for the published M7-5 comparative benchmark runs. The benchmark operator should either match this spec closely or amend this file before executing slice C. All published numbers are reproducible on this class of Linux workstation; results on different hardware can vary materially.

## Target spec

| Component | Reference target | Why it is pinned |
|---|---|---|
| CPU | x86_64 workstation CPU, 8+ physical cores, >= 3.5 GHz base clock. Reference examples: AMD Ryzen 7 5800X, Intel Core i7-12700. | The harness runs four tools across cold and warm cache states; MAT and hprof-slurp both benefit from sustained parallel CPU throughput. |
| RAM | 32 GiB minimum, 64 GiB preferred. | The 10 GiB fixture plus MAT's headless working set should fit without swapping. |
| Storage | NVMe SSD, 200 GiB free before a full run. | Slice C generates three synthetic fixtures, MAT index folders, raw CSVs, logs, and repeated cold-cache rotations. |
| OS | Ubuntu 22.04 LTS or Ubuntu 24.04 LTS, kernel 5.15+. | Published M7-5 numbers are Linux-only because RSS measurement depends on `/usr/bin/time -v` and `/proc`. |
| JVM | OpenJDK 17 LTS or Eclipse Temurin 17 LTS. Capture `java -version` verbatim in the run manifest. | The fixture generator and Eclipse MAT both depend on a stable Java 17 baseline. |
| Mnemosyne build | `cargo build --release -p mnemosyne-cli` from the v0.3.0 comparison commit. | The report compares shipped release builds, not debug binaries or local tuning experiments. |
| Eclipse MAT | 1.15.0 standalone, Linux GTK x86_64 build, launched through `ParseHeapDump.sh`. | The design doc pins MAT 1.15.0 for the first published comparison. |
| hprof-slurp | `hprof-slurp` v0.6.3, installed from crates.io or the matching GitHub release. | The report must pin a concrete version so later releases do not silently drift the comparison. |
| Benchmark helpers | `hyperfine`, GNU `time` (`/usr/bin/time -v`), optional `vmtouch`. | These tools provide the published wall-time, max-RSS, and file-cache measurements. |

## Reproducibility contract

- All published M7-5 numbers are tied to this reference spec and the exact tool versions captured in the run manifest.
- Results from laptops, SATA disks, lower-memory machines, or different JVMs are useful for local exploration but must not replace the published v0.3.0 numbers.
- If the operator uses different hardware, this file must be updated before slice C and the comparative report must quote the amended spec verbatim.

## Baseline provisioning commands

Run these commands on the Linux reference workstation before generating fixtures or launching the comparative harness:

```bash
sudo apt-get update
sudo apt-get install -y openjdk-17-jdk unzip hyperfine time vmtouch build-essential pkg-config curl
rustup default stable
cargo build --release -p mnemosyne-cli
```

Download Eclipse MAT 1.15.0 and pin hprof-slurp to the comparison version:

```bash
wget "https://www.eclipse.org/downloads/download.php?file=/mat/1.15.0/rcp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip" \
  -O /tmp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip
unzip -q /tmp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip -d "$HOME/tools/mat-1.15.0"
export MAT_HOME="$HOME/tools/mat-1.15.0"
export MAT_VMARGS="-Xmx16g"

cargo install --locked --version 0.6.3 hprof-slurp
```

## Required capture for every published run

Before slice C starts, record these commands into the raw-artifact manifest:

```bash
uname -a
lsblk -o NAME,MODEL,SIZE,ROTA
lscpu
free -h
java -version
"$MAT_HOME/ParseHeapDump.sh" --help || true
hprof-slurp --version
./target/release/mnemosyne-cli --version
```

## Notes

- `MAT_VMARGS="-Xmx16g"` is the default comparison setting. If MAT still fails on the 10 GiB fixture, keep the failure in the result table instead of silently retuning the JVM mid-run.
- `vmtouch` is optional. If it is missing, the harness should mark peak file-cache as `n/a` and continue.
- Windows and macOS can still run the tools, but they are not the reference path for the published M7-5 numbers.