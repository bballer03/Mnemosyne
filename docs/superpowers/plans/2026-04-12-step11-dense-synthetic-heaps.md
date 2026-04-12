# Step 11 Dense Synthetic Heaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the synthetic heap generator so Step 11 large-dump validation uses dense, representative heaps that materially scale object and reference counts.

**Architecture:** Replace the current chunk-based generator with cluster-based retained object graphs that mix linked nodes, object arrays, collections, duplicate strings, unique strings, and moderate primitive payloads. Keep the existing shell contracts stable so the Step 11 wrapper and smoke tests continue to operate unchanged.

**Tech Stack:** Bash, Java, Rust CLI validation via `mnemosyne-cli parse`, existing shell smoke tests

---

### Task 1: Add Growth-Focused Red Tests

**Files:**
- Modify: `scripts/tests/test_generate_synthetic_heap_density.sh`
- Create: `scripts/tests/test_generate_synthetic_heap_growth.sh`
- Test: `scripts/tests/test_generate_synthetic_heap_density.sh`, `scripts/tests/test_generate_synthetic_heap_growth.sh`

- [ ] **Step 1: Keep the density assertion as the primary red test**

```bash
bash "scripts/tests/test_generate_synthetic_heap_density.sh"
```

Expected: FAIL because the current generator produces fewer than `100000` objects for the 32 MB heap.

- [ ] **Step 2: Write the growth test**

```bash
#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)
small="$tmpdir/small.hprof"
large="$tmpdir/large.hprof"

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

bash "$repo_root/scripts/generate_synthetic_heap.sh" --size-mb 16 --xmx-mb 256 --output "$small"
bash "$repo_root/scripts/generate_synthetic_heap.sh" --size-mb 32 --xmx-mb 320 --output "$large"

small_parse=$(bash -lc '"$PWD/target/debug/mnemosyne-cli" parse "$1"' -- "$small")
large_parse=$(bash -lc '"$PWD/target/debug/mnemosyne-cli" parse "$1"' -- "$large")

small_objects=$(awk '/Estimated objects:/ { print $3; exit }' <<<"$small_parse")
large_objects=$(awk '/Estimated objects:/ { print $3; exit }' <<<"$large_parse")

if (( large_objects <= small_objects )); then
    echo "expected larger synthetic heap to produce more objects: $small_objects vs $large_objects" >&2
    exit 1
fi
```

- [ ] **Step 3: Run the new growth test to verify it fails or exposes the current weakness**

Run: `bash "scripts/tests/test_generate_synthetic_heap_growth.sh"`
Expected: either FAIL on generation limits or show the current sparse scaling problem.

### Task 2: Implement Dense Cluster-Based Heap Generation

**Files:**
- Modify: `scripts/java/SyntheticHeapApp.java`
- Test: `scripts/tests/test_generate_synthetic_heap_density.sh`, `scripts/tests/test_generate_synthetic_heap_growth.sh`

- [ ] **Step 1: Replace the current `Chunk` design with smaller linked objects and collection-heavy clusters**

Implementation requirements:

```java
static final class Node {
    Node next;
    Node back;
    Node jump;
    Object payload;
    String label;
    int weight;
}

static final class Cluster {
    Node[] nodes;
    ArrayList<Node> nodeList;
    HashMap<String, Node> nodeMap;
    ArrayList<String> duplicates;
    ArrayList<String> uniques;
    Object[] wrappers;
}
```

The final implementation may rename fields, but it must preserve the same design intent: many retained objects with many references plus realistic collection/string content.

- [ ] **Step 2: Ensure growth is driven by many clusters, not a few huge arrays**

Required behavior:

```java
while (approxAllocatedBytes < targetBytes) {
    Cluster cluster = buildCluster(seed++, previousClusters, duplicatePool);
    roots.add(cluster);
    approxAllocatedBytes += cluster.approxBytes();
}
```

- [ ] **Step 3: Run density test and make it pass**

Run: `bash "scripts/tests/test_generate_synthetic_heap_density.sh"`
Expected: PASS with at least `100000` objects for the 32 MB heap.

- [ ] **Step 4: Run growth test and make it pass**

Run: `bash "scripts/tests/test_generate_synthetic_heap_growth.sh"`
Expected: PASS with larger heap producing more objects.

### Task 3: Re-verify Existing Step 11 Tooling Contracts

**Files:**
- Test: `scripts/tests/test_generate_synthetic_heap.sh`
- Test: `scripts/tests/test_measure_rss_short_parse.sh`
- Test: `scripts/tests/test_measure_rss_450mb.sh`
- Test: `scripts/tests/test_run_step11_scaling_validation.sh`

- [ ] **Step 1: Re-run base generator smoke test**

Run: `bash "scripts/tests/test_generate_synthetic_heap.sh"`
Expected: PASS with non-empty dump output.

- [ ] **Step 2: Re-run short parse RSS smoke test**

Run: `bash "scripts/tests/test_measure_rss_short_parse.sh"`
Expected: PASS with a valid ratio row and no `/proc` sampling error.

- [ ] **Step 3: Re-run 450 MB RSS smoke test**

Run: `bash "scripts/tests/test_measure_rss_450mb.sh"`
Expected: PASS with `parse`, `analyze`, and `leaks` ratio rows.

- [ ] **Step 4: Re-run wrapper smoke test**

Run: `bash "scripts/tests/test_run_step11_scaling_validation.sh"`
Expected: PASS with dump, default measurement, investigation measurement, and `summary.tsv` present.

### Task 4: Recalibrate Dense Heap Size Mapping

**Files:**
- Modify if needed: `scripts/run_step11_scaling_validation.sh`
- Evidence: temp calibration output only

- [ ] **Step 1: Generate several calibration heaps**

Run examples:

```bash
bash "scripts/generate_synthetic_heap.sh" --size-mb 128 --xmx-mb 384 --output /tmp/mn-step11-calibration-128.hprof
bash "scripts/generate_synthetic_heap.sh" --size-mb 256 --xmx-mb 640 --output /tmp/mn-step11-calibration-256.hprof
bash "scripts/generate_synthetic_heap.sh" --size-mb 512 --xmx-mb 1152 --output /tmp/mn-step11-calibration-512.hprof
```

- [ ] **Step 2: Record actual dump sizes and choose new request values for the 500 MB / 1 GB / 2 GB tiers**

Use: `wc -c <file>` and choose the smallest stable request values that land near the target dump tiers.

- [ ] **Step 3: Update tier sizes only if required**

If calibration shows the wrapper defaults are materially off, modify `scripts/run_step11_scaling_validation.sh` or call it with explicit `--sizes-mb` values. Do not change the shell interface.

### Task 5: Run Full Step 11 Validation And Update Docs

**Files:**
- Modify: `docs/performance/memory-scaling.md`
- Modify: `docs/design/memory-scaling.md`
- Modify: `STATUS.md`
- Evidence: temp output directory with `summary.tsv`

- [ ] **Step 1: Run the full Step 11 batch**

Run: `bash "scripts/run_step11_scaling_validation.sh" --output-dir /tmp/mn-step11-runs`

Expected: tier dumps, default measurement files, investigation measurement files, and `summary.tsv`.

- [ ] **Step 2: Inspect `summary.tsv` and derive the Step 11 decision**

Decision rules come from `docs/design/memory-scaling.md`:
- `<= 4.0x` at T1/T2: PASS
- `> 4.0x` at T2: FAIL investigate
- `> 4.0x` at T1: FAIL remediate

- [ ] **Step 3: Update performance/design/status docs with measured data and decision**

Required content:
- actual measured tier rows
- whether default path and investigation path are acceptable
- whether Step 11 remains partial or becomes complete

- [ ] **Step 4: Re-run the changed shell tests after doc edits if scripts changed during calibration**

Run the same shell tests from Task 3 if any script changed during Task 4 or Task 5.
