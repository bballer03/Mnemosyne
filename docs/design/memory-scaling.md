# Memory Scaling Decision Template

> Status: Draft template for M3-P1-B1
> Owner: Implementation Agent
> Last Updated: 2026-03-08

## Objective

Document measured resident memory usage for Mnemosyne's in-memory ObjectGraph pipeline and decide when the current approach becomes impractical for real-world heap dumps.

## Current RSS Measurements

| Fixture | Dump Size | Command | Max RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|---|---|
| Synthetic graph fixture | ~300 B | `cargo bench` parser_bench | N/A | N/A | Sub-microsecond; RSS not measured separately |
| Real heap fixture (parse) | 156 MB | `mnemosyne-cli parse` | ~3.5 MB | 0.02x | Streaming parser, minimal RAM |
| Real heap fixture (analyze) | 156 MB | `mnemosyne-cli analyze` | ~555 MB | 3.56x | Full ObjectGraph + dominator |
| Real heap fixture (leaks) | 156 MB | `mnemosyne-cli leaks` | ~555 MB | 3.55x | Graph-backed leak detection |

## Object/Dump Size Ratio Expectations

- Working assumption: parsed in-memory graphs will exceed on-disk dump size because Mnemosyne materializes objects, classes, references, and supporting indexes.
- Healthy target for current architecture: less than or equal to 4x RSS relative to dump size for incident-response sized dumps.
- Investigate immediately if RSS exceeds 4x on medium fixtures or shows non-linear growth with dump size.

## Decision Threshold

In-memory ObjectGraph becomes impractical when one or more of the following is true:

- Max RSS consistently exceeds 4x dump size on representative production heaps.
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

- **Decision:** Current in-memory `ObjectGraph` pipeline is appropriate for dumps up to ~1-2 GB. No architectural change required for the near term.
- **Rationale:** Measured RSS:dump ratio of ~3.5x on a 156 MB real-world Kotlin+Spring Boot heap dump is within the 4x safety threshold. The streaming parser provides a zero-overhead fallback for summary-level commands. Graph queries are sub-microsecond once the graph is built.
- **Follow-up work:** For M4+, evaluate `memmap2` or disk-backed storage if users report needing 4 GB+ dump support. Add larger test fixtures (500 MB, 1 GB) when available to validate scaling projections.