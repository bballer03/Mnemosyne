# M7-5 Tool Installation Guide

This guide installs the three tools used by the M7-5 comparative benchmark harness: Mnemosyne, Eclipse MAT, and hprof-slurp. The published comparison is Linux-first; Windows notes are included only for MAT's batch entry point.

## 1. Linux prerequisites

```bash
sudo apt-get update
sudo apt-get install -y openjdk-17-jdk unzip hyperfine time vmtouch build-essential pkg-config curl
```

If Rust is not installed yet:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
```

## 2. Mnemosyne

Build the benchmarked CLI from the checked-out comparison commit:

```bash
git checkout <comparison-commit-or-tag>
cargo build --release -p mnemosyne-cli
./target/release/mnemosyne-cli --version
```

Smoke-check the binary against the existing real fixture:

```bash
./target/release/mnemosyne-cli parse resources/test-fixtures/heap.hprof --mode overview
```

If you prefer a stable shell alias during slice C:

```bash
export MNEMOSYNE_BIN="$PWD/target/release/mnemosyne-cli"
```

## 3. Eclipse MAT 1.15.0

Download and unpack the standalone Linux GTK build pinned by the design doc:

```bash
wget "https://www.eclipse.org/downloads/download.php?file=/mat/1.15.0/rcp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip" \
  -O /tmp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip
unzip -q /tmp/MemoryAnalyzer-1.15.0.20231206-linux.gtk.x86_64.zip -d "$HOME/tools/mat-1.15.0"
export MAT_HOME="$HOME/tools/mat-1.15.0"
export MAT_VMARGS="-Xmx16g"
```

Headless parse entry points:

- Linux: `"$MAT_HOME/ParseHeapDump.sh"`
- Windows: `"%MAT_HOME%\ParseHeapDump.bat"`

Recommended headless smoke check:

```bash
"$MAT_HOME/ParseHeapDump.sh" resources/test-fixtures/heap.hprof \
  org.eclipse.mat.api:suspects \
  org.eclipse.mat.api:overview
```

Required MAT heap sizing for published runs:

- Use `MAT_VMARGS="-Xmx16g"` as the default M7-5 setting.
- For the 1 GiB and 4 GiB fixtures, lower values may work, but the published workflow keeps one pinned value to avoid per-fixture retuning.
- If MAT still fails on the 10 GiB tier with `-Xmx16g`, record the failure as `OOM` or `ERROR` in the comparison table instead of changing the methodology mid-run.

## 4. hprof-slurp 0.6.3

Pinned Cargo install:

```bash
cargo install --locked --version 0.6.3 hprof-slurp
hprof-slurp --version
```

Pinned release-binary source:

- GitHub releases: `https://github.com/agourlay/hprof-slurp/releases`
- crates.io package: `https://crates.io/crates/hprof-slurp`

Basic smoke check:

```bash
hprof-slurp -i resources/test-fixtures/heap.hprof --top 20 --json
```

## 5. Verification checklist

Confirm the benchmark workstation can see every tool before slice C:

```bash
java -version
"$MAT_HOME/ParseHeapDump.sh" --help || true
hprof-slurp --version
./target/release/mnemosyne-cli --version
hyperfine --version
/usr/bin/time --version
vmtouch -v || true
```

## 6. Expected locations used by slice B and slice C

- `MAT_HOME` points at the unpacked MAT directory.
- `MAT_VMARGS` is exported before every MAT run.
- `MNEMOSYNE_BIN` may point at `./target/release/mnemosyne-cli`.
- `hprof-slurp` is expected on `PATH`.

The upcoming `scripts/bench/run_comparative.sh` harness will assume this installation layout unless the operator overrides it with explicit flags.