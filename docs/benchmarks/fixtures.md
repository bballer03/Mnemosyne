# M7-5 Fixture Matrix

This matrix defines the four fixtures used by the M7-5 comparative benchmark campaign. The synthetic tiers all come from `scripts/generate_synthetic_heap.sh`, which already accepts arbitrary `--size-mb` values and delegates to a deterministic Java layout builder keyed only by target size.

## Fixture table

| Fixture | Approx. size | Generation command | Expected object count | Expected class count | What it stresses |
|---|---|---|---|---|---|
| F-156M | ~156 MiB real heap | Existing file: `resources/test-fixtures/heap.hprof` | ~300k live objects in the graph-backed path; treat this as the real-world regression sentinel rather than a synthetic invariant | ~4.3k `LOAD_CLASS` records on the reference fixture | Real-world JVM heap shape, segmented dump parsing, deep-mode parity baseline, leak and dominator behavior |
| F-1G | 1,024 MiB synthetic | `scripts/generate_synthetic_heap.sh --size-mb 1024 --output fixtures/synthetic-1gb.hprof` | Low teens of millions. Lower-bound rooted-object model: ~11.9M objects across 90 synthetic clusters before JVM string-payload arrays and transient concatenation noise | Stable JDK/app class set on Java 17; expect roughly low-thousands and capture the exact value in the slice C manifest | Deep-mode large-tier RSS, analyzer scaling, MAT vs Mnemosyne parity at the smallest synthetic tier |
| F-4G | 4,096 MiB synthetic | `scripts/generate_synthetic_heap.sh --size-mb 4096 --output fixtures/synthetic-4gb.hprof` | High tens of millions. Lower-bound rooted-object model: ~47.4M objects across 359 synthetic clusters | Stable JDK/app class set on Java 17; same order of magnitude as F-1G because fixture size grows by cluster count rather than by loading new classes | Auto-mode boundary at 4 GiB, warm/cold cache behavior, overview-mode throughput vs hprof-slurp |
| F-10G | 10,240 MiB synthetic | `scripts/generate_synthetic_heap.sh --size-mb 10240 --output fixtures/synthetic-10gb.hprof` | Low hundreds of millions. Lower-bound rooted-object model: ~118.3M objects across 896 synthetic clusters | Stable JDK/app class set on Java 17; same class family as F-1G and F-4G, exact count recorded at run time | Streaming-only tier, MAT failure-mode capture, overview-mode memory ceiling, file-cache sensitivity |

## Why the synthetic counts are stated as expectations

- The Java fixture generator is deterministic in structure, but `jmap -dump` records the JVM heap, not just the intentionally retained application objects.
- That means exact raw-dump object totals can include JVM string payload arrays and short-lived concatenation artifacts in addition to the rooted synthetic graph.
- Slice C must capture the exact file size, SHA-256, object count, and loaded-class count for every generated fixture and publish those figures alongside the raw CSVs.

## Synthetic generator notes

- `scripts/generate_synthetic_heap.sh` already supports the full M7-5 size matrix through `--size-mb` and `--output`.
- The underlying `SyntheticHeapApp.java` does not use randomness, timestamps, or external inputs to shape the retained object graph; the cluster layout is derived from the requested target size.
- No seed plumbing is required for slice A. For published runs, retain each fixture's SHA-256 in the slice C artifact manifest so reruns can be compared byte-for-byte when the same environment is used.

## Derived cluster counts for the synthetic tiers

The current Java generator uses `nodeCountForTarget()` = 22,000 nodes for all three comparative synthetic tiers and `estimateClusterBytes()` = 11,992,576 bytes per cluster. That yields the following planning counts:

| Fixture | Target bytes | Cluster count | Planning note |
|---|---|---|---|
| F-1G | 1,073,741,824 | 90 | First synthetic tier where deep mode is still expected to be practical for all tools |
| F-4G | 4,294,967,296 | 359 | Matches the default auto-overview cutoff boundary |
| F-10G | 10,737,418,240 | 896 | Streaming-focused comparison tier; MAT may fail even with `-Xmx16g` |

## Recommended generation workflow

```bash
mkdir -p fixtures
scripts/generate_synthetic_heap.sh --size-mb 1024 --output fixtures/synthetic-1gb.hprof
scripts/generate_synthetic_heap.sh --size-mb 4096 --output fixtures/synthetic-4gb.hprof
scripts/generate_synthetic_heap.sh --size-mb 10240 --output fixtures/synthetic-10gb.hprof
```

After generation, record each fixture's SHA-256 and exact parse summary before the full comparative harness runs.