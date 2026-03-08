# Memory Scaling Decision Template

> Status: Active - Step 11 partial complete
> Owner: Implementation Agent + Testing Agent
> Last Updated: 2026-03-09
> Design Reference: Roadmap Step 11

## Objective

Document measured resident memory usage for Mnemosyne's in-memory `ObjectGraph` pipeline and decide when the current approach becomes impractical for real-world heap dumps.

## Current RSS Measurements

| Fixture | Dump Size | Command | Max RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|---|
| Synthetic graph fixture | ~300 B | `cargo bench` parser_bench | N/A | N/A | Sub-microsecond; RSS not measured separately |
| Real heap fixture (parse) | 156 MB | `mnemosyne-cli parse` | 5.12 MiB | 0.03x | Streaming parser, minimal RAM |
| Real heap fixture (analyze, default) | 156 MB | `mnemosyne-cli analyze` | 656.65 MiB | 4.23x | Lean graph-backed path with `retain_field_data = false` |
| Real heap fixture (leaks, default) | 156 MB | `mnemosyne-cli leaks` | 656.46 MiB | 4.23x | Lean graph-backed leak detection |
| Real heap fixture (analyze, investigation) | 156 MB | `mnemosyne-cli analyze --strings --threads --collections` | ~741 MiB | 4.78x | Opt-in field-data retention |

### Historical Baselines

| Baseline | Max RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|
| Pre-Phase-2 baseline | ~555 MB | 3.56x | Historical reference only |
| Post-Phase-2 re-baseline before remediation | 741 MiB | 4.78x | Triggered Step 11 remediation |

## Object/Dump Size Ratio Expectations

- Working assumption: parsed in-memory graphs will exceed on-disk dump size because Mnemosyne materializes objects, classes, references, and supporting indexes.
- Original healthy target for current architecture: less than or equal to 4x RSS relative to dump size for incident-response sized dumps.
- Current measured reality after Step 11 remediation: default graph-backed runs are 4.23x on the 156 MB fixture, while investigation-heavy runs are 4.78x.
- Investigate immediately if larger-tier validation shows further growth beyond the current default-path ratio or materially worse non-linearity.

## Decision Threshold

In-memory `ObjectGraph` becomes impractical when one or more of the following is true:

- Max RSS consistently exceeds 4x dump size on representative production heaps without the user opting into heavy investigation features.
- Typical analysis requires more memory than a common developer workstation can provide.
- Dominator and graph queries become dominated by allocator pressure rather than traversal work.
- Benchmark data shows that large-dump runs force swapping or severe OS memory pressure.

## Alternatives Considered

### `memmap2`

- Potential upside: lower peak resident memory by moving large backing stores to memory-mapped files.
- Risks: more complex ownership model, random-access performance tradeoffs, and additional serialization format decisions.

### Disk-backed storage / index files

- Potential upside: scales beyond RAM and supports repeated queries without reparsing.
- Risks: much larger implementation scope, new failure modes, and persistence format maintenance.

### Streaming-only mode

- Potential upside: bounded memory and fast overview mode for large production dumps.
- Risks: does not answer deep graph questions by itself; likely needs to coexist with the current deep-analysis path.

## Decision And Rationale

- **Current decision:** keep the in-memory `ObjectGraph` architecture for now, but treat it as conditionally acceptable rather than fully cleared.
- **Rationale:** Step 11 confirmed that unconditional field retention regressed the 156 MB fixture to 4.78x RSS:dump. The implemented `ParseOptions` remediation reduced default `analyze`/`leaks` runs to 4.23x by making field retention opt-in. That is materially better than the regressed state and preserves the feature set, but it is still above the original 4.0x target.
- **Architectural takeaway:** `ParseOptions` is now part of the public parser surface and is the primary mechanism for separating lean default runs from higher-cost investigation runs.
- **Follow-up work:** multi-tier validation at 500 MB / 1 GB / 2 GB remains mandatory before M3 Phase 3 is considered safe.

### Post-Phase-2 Re-baseline (Historical Step 11 Trigger)

Step 11 re-baselining on 2026-03-08 measured the 156 MB fixture with unconditional Phase 2 field retention enabled:

| Command | Max RSS | RSS:Dump Ratio | Status |
|---------|---------|----------------|--------|
| parse | 3.50 MiB | 0.02x | ✅ Unchanged (streaming) |
| analyze | 741 MiB | 4.78x | ⚠️ FAIL - exceeds 4.0x |
| leaks | 741 MiB | 4.78x | ⚠️ FAIL - exceeds 4.0x |

- **Memory increase:** +186 MB (+34%) over the pre-Phase-2 baseline (555 MB -> 741 MB)
- **Root cause:** `HeapObject.field_data: Vec<u8>` unconditionally stored raw field bytes for all `INSTANCE_DUMP` records, and selected `byte[]`/`char[]` arrays also retained content.
- **Decision outcome:** RSS:dump > 4.0x on the 156 MB fixture triggered the Step 11 remediation path.

---

## Step 11 Remediation - Conditional field_data Retention

> Status: Implemented and validated on the 156 MB fixture
> Date: 2026-03-09

### Problem Statement

M3 Phase 2 added `field_data: Vec<u8>` to `HeapObject` to enable field-level extraction for thread inspection, string analysis, and collection inspection. Unconditional retention raised the 156 MB fixture from 3.56x to 4.78x RSS:dump, exceeding the 4.0x target and forcing a remediation decision.

### Remediation Approach: Opt-in field_data Retention

Make `field_data` retention controlled by a parser-level flag. Only populate `field_data` when advanced analyzers that need it are enabled.

### Implemented Design

1. **Public `ParseOptions` struct** in `binary_parser.rs`:
    ```rust
    pub struct ParseOptions {
         pub retain_field_data: bool,
    }
    ```

2. **New parser entry points**:
    ```rust
    pub fn parse_hprof_file_with_options(path: &str, options: ParseOptions) -> CoreResult<ObjectGraph>
    pub fn parse_hprof_with_options(data: &[u8], options: ParseOptions) -> CoreResult<ObjectGraph>
    ```

3. **`parse_instance_dump()` gating:** when `retain_field_data` is false, raw instance bytes are not copied into `HeapObject.field_data`.

4. **`parse_prim_array_dump()` gating:** `byte[]` / `char[]` payload retention only happens when `retain_field_data` is true; the existing retained-array size cap still applies.

5. **`analyze_heap()` flag plumbing:** `retain_field_data = true` only when any of `enable_strings`, `enable_collections`, or `enable_threads` is requested.

6. **Lean default callers:** `detect_leaks()` and `gc_path` now always use `retain_field_data = false`.

7. **Public API alignment:** `core::lib` now re-exports `ParseOptions` so callers can explicitly choose the heavy path.

### Dependency Analysis - Which Analyzers Need field_data?

| Analyzer | Uses `read_field()` / `field_data` | Needs `retain_field_data: true` |
|----------|-----------------------------------|---------------------------------|
| `inspect_threads()` | Yes - reads `daemon`, `name` fields | Yes |
| `analyze_strings()` | Yes - reads `value`, `coder` fields + backing array content | Yes |
| `inspect_collections()` | Yes - reads `size`, `table`, `elementData`, `map` fields | Yes |
| `find_top_instances()` | No - uses only `shallow_size` + `retained_size` | No |
| `graph_backed_leaks()` | No - uses only sizes, class names, dominator tree | No |
| `build_histogram()` | No - uses only class IDs, sizes | No |
| `find_unreachable_objects()` | No - uses only GC-root reachability | No |
| `find_gc_path()` | No - uses only references + GC roots | No |

### Measured Memory Impact

| Scenario | `retain_field_data` | Measured RSS:Dump Ratio |
|----------|--------------------|-------------------------|
| `analyze` (default - no investigation flags) | false | 4.23x |
| `leaks` | false | 4.23x |
| `gc-path` | false | Lean path by design; not re-measured in this batch |
| `analyze --threads --strings --collections` | true | 4.78x |
| `parse` (streaming) | N/A | 0.03x |

### File Impact

| File | Change |
|------|--------|
| `core/src/hprof/binary_parser.rs` | Added `ParseOptions`, `parse_hprof_file_with_options()`, `parse_hprof_with_options()`, and gated retention in instance / primitive-array parsing. |
| `core/src/analysis/engine.rs` | Routed `retain_field_data` based on requested analyzers. |
| `core/src/graph/gc_path.rs` | Switched to the lean parser path. |
| `core/src/lib.rs` | Re-exported `ParseOptions`. |
| `scripts/measure_rss.sh` | Added multi-command profiling, `/proc` fallback, ratio computation, and status markers. |

### Backward Compatibility

- `parse_hprof_file(path)` and `parse_hprof(data)` signatures remain unchanged.
- The default parser behavior is now intentionally lean, matching the pre-Phase-2 expectation that raw field bytes are not retained unless requested.
- Callers that require raw `field_data` must opt into `ParseOptions { retain_field_data: true }` explicitly.

### Remaining Risks

1. **Silent capability loss if future analyzers forget to opt in:** safe failure mode, but still a maintenance risk.
2. **Benchmark split between lean and investigation paths:** both paths now need explicit tracking.
3. **Large-dump uncertainty remains:** 156 MB is validated; 500 MB+ is not.

### Validation Result

Completed in this batch:
1. Re-measured the 156 MB fixture with default `analyze` -> 4.23x
2. Re-measured the 156 MB fixture with default `leaks` -> 4.23x
3. Re-measured the 156 MB fixture with `analyze --threads --strings --collections` -> 4.78x
4. Confirmed `cargo check`, `cargo test` (129 passed), `cargo clippy -- -D warnings`, and `cargo build --release`

Still pending:
1. Multi-tier validation at 500 MB / 1 GB / 2 GB
2. Re-measuring `gc-path` explicitly on a larger dump tier

---

## Step 11 - Multi-Tier Scaling Validation Protocol

### Purpose

Step 11 is a validation and decision gate. The re-baseline, remediation, and 156 MB post-remediation validation are complete; the remaining goal is to confirm whether the current default-path ratio holds acceptably at larger dump sizes.

### Current Step 11 State

- **Done:** script enhancement
- **Done:** post-Phase-2 re-baseline (4.78x regression discovered)
- **Done:** conditional field-retention remediation via `ParseOptions`
- **Done:** post-remediation 156 MB validation (default `analyze`/`leaks` at 4.23x)
- **Pending:** multi-tier validation at 500 MB / 1 GB / 2 GB (+ optional 5 GB stretch tier)

### Test Tier Definitions

| Tier | Dump Size | Target RSS (4x threshold) | Pass Criterion | 16 GB Workstation Feasibility |
|------|-----------|--------------------------|----------------|-------------------------------|
| T1 | ~500 MB | <= 2.0 GB | RSS:dump <= 4.0x | ✅ Comfortable |
| T2 | ~1 GB | <= 4.0 GB | RSS:dump <= 4.0x | ⚠️ Needs >= 8 GB free |
| T3 | ~2 GB | <= 8.0 GB | RSS:dump <= 4.0x | ⚠️ Needs >= 16 GB system |

Optional stretch tier (if resources allow):

| Tier | Dump Size | Target RSS | Purpose |
|------|-----------|------------|---------|
| T4 | ~5 GB | <= 20 GB | Confirm failure mode is graceful OOM, not crash |

### Dump Generation Methodology

1. **Preferred:** real-world dumps from OpenJDK 17/21 + Spring Boot applications with diverse object graphs.
2. **Fallback:** synthetic generators that stress references, collections, strings, and large primitive arrays.
3. **Dump provenance:** record JDK version, JVM flags, application type, and heap settings for each tier.

### Commands to Profile

Each tier should profile:

| Command | Expected Memory Profile | Why |
|---------|------------------------|-----|
| `parse` | Minimal (streaming) | Confirms streaming path remains unaffected |
| `analyze` | Lean by default, heavier with investigation flags | Confirms both the default path and the opt-in investigation path |
| `leaks` | Lean graph-backed path | Confirms default leak-detection memory profile |

### Measurement Method

- **Primary:** `/proc/PID/status` VmHWM sampling via `scripts/measure_rss.sh`
- **Secondary:** `/usr/bin/time -v` when available

The script already supports the required multi-command profiling, fallback sampling, ratio computation, and tabular output.

### Decision Criteria

| Outcome | Condition | Action |
|---------|-----------|--------|
| **PASS - No action** | RSS:dump <= 4.0x at T1 and T2 | Update docs with measured data. Proceed to M3 Phase 3. |
| **PASS - Monitor** | RSS:dump <= 4.0x at T1/T2 but > 4.0x at T3 | Document as known limitation. 2 GB+ dumps are out of scope for in-memory analysis. Streaming overview mode (backlog #37) becomes P2 for M4. |
| **FAIL - Investigate** | RSS:dump > 4.0x at T2 (1 GB) | Evaluate alternatives before M3 Phase 3. Options in priority order: (1) reduce retained-data scope further, (2) `memmap2` for large backing stores, (3) streaming overview mode as the primary path for large dumps. |
| **FAIL - Remediate** | RSS:dump > 4.0x at T1 (500 MB) | Treat the current 4.23x default path as non-acceptable at scale. Block M3 Phase 3 until the memory model is improved again. |

### Deliverables

1. Updated `docs/performance/memory-scaling.md` with measured tier data
2. Updated `docs/design/memory-scaling.md` decision section with validated rationale
3. Decision recorded: proceed / investigate / remediate