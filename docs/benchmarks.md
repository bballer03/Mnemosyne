# Mnemosyne Benchmarks

> Published benchmark results and comparative notes for Mnemosyne's current parser, graph, and dominator performance.

This document separates three kinds of statements:

- **Measured Mnemosyne results** from the repository's Criterion benchmarks and RSS scripts.
- **Published external claims** for competitor tools, summarized from the roadmap research.
- **Interpretation and tradeoffs** that explain where each tool is stronger or weaker.

Mnemosyne is not trying to win every benchmark category with one architecture. The current design favors deeper graph-backed analysis, provenance, structured outputs, and MCP or AI workflows over the lowest possible memory ceiling on very large dumps.

## 1. Mnemosyne Benchmark Results

### Published Throughput and Latency

| Workload | Result | Fixture / Scope | Source |
|---|---|---|---|
| Streaming parser throughput | **2.25 GiB/s** | 156 MB real heap fixture | Criterion `parse_heap_real_fixture` |
| Binary parser throughput | **90.47 MiB/s** | 156 MB real heap fixture, full `ObjectGraph` build | Criterion `parse_hprof_real_fixture` |
| Dominator tree build | **1.85 s** | 156 MB real heap fixture | Criterion `dominator_build_real_fixture` |
| Dominator top-retained query | **712 us** | 156 MB real heap fixture | Criterion `dominator_top_retained_real_fixture` |

### Memory Scaling Ratios

The Step 11 dense synthetic validation is the current scaling reference for large-tier RSS behavior.

| Path | ~500 MB tier | ~1 GB tier | ~2 GB tier | Notes |
|---|---|---|---|---|
| `parse` | 0.02x | 0.01x | 0.00x | Streaming summary path stays near-flat in memory |
| `analyze` default | **2.90x** | **2.87x** | **2.89x** | Lean graph-backed path |
| `leaks` default | **2.90x** | **2.87x** | **2.89x** | Same lean graph-backed path |
| `analyze --threads --strings --collections` | **3.92x** | **3.89x** | **3.92x** | Opt-in investigation path with retained field data |

For the real 156 MB fixture, the currently published RSS numbers are:

| Command | Peak RSS | RSS:Dump Ratio | Notes |
|---|---|---|---|
| `parse` | 5.12 MiB | **0.03x** | Streaming record scan |
| `analyze` | 656.65 MiB | **4.23x** | Lean graph-backed deep analysis |
| `leaks` | 656.46 MiB | **4.23x** | Lean graph-backed leak path |
| `analyze --strings --threads --collections` | ~741 MiB | **4.78x** | Higher-memory investigation path |

### Reproduction

Use the existing benchmark and measurement entry points:

```bash
cargo bench
```

```bash
scripts/measure_rss.sh resources/test-fixtures/heap.hprof
```

Additional wrappers exist for optional tooling:

- `scripts/run_hyperfine_bench.sh <heap.hprof>`
- `scripts/run_heaptrack_profile.sh <heap.hprof>`

## 2. Competitive Comparison Table

The table below compares Mnemosyne's current published results against external project positioning described in the roadmap. External numbers are not apples-to-apples lab reruns inside this repository, so treat them as published reference points rather than directly normalized measurements.

| Dimension | Mnemosyne | hprof-slurp | Eclipse MAT |
|---|---|---|---|
| Speed | **2.25 GiB/s** streaming parser on 156 MB fixture; **90.47 MiB/s** full binary parse; **1.85 s** dominator build on 156 MB fixture | Published at **~2 GB/s** on 4+ cores for streaming-style triage workloads | Slower initial parse/index build; strong follow-up exploration after indexes are built |
| Memory | Streaming path is tiny; deep analysis validated at **2.87x-2.90x** default RSS through roughly 2 GB synthetic tiers, **3.89x-3.92x** on investigation path | Published at **~500 MB flat** for a 34 GB dump by avoiding full in-memory graph construction | High memory footprint plus disk indexes; designed for deep interactive analysis rather than low RSS |
| Analysis depth | Medium-high today: object graph, dominators, retained sizes, leak detection, GC paths, thread analysis, string analysis, collection analysis, top instances, classloader analysis, minimal query surface | Shallow by design: fast overview, top-N summaries, strings, thread stacks; no retained-size graph analysis | Deep and mature: dominators, retained sizes, OQL, extensive interactive exploration |
| Output formats | Text, Markdown, HTML, TOON, JSON | Text, JSON | GUI first; batch and export flows such as HTML and CSV |
| AI integration | Yes: shared AI pipeline with `rules`, `stub`, and provider-backed modes | None | None |
| Provenance | Yes: explicit provenance markers for fallback, partial, synthetic, and placeholder results | None | None |
| MCP integration | Yes: stdio MCP server with 14 methods and persisted AI sessions | None | None |

## 3. Where Mnemosyne Wins

### Against hprof-slurp

Mnemosyne already goes materially deeper than hprof-slurp in the analysis pipeline. The current core can build a real `ObjectGraph`, compute dominator trees, emit retained sizes, rank leak suspects, inspect collections and strings, trace GC paths, and attach provenance to fallback paths. hprof-slurp is faster in raw overview-oriented parsing, but it intentionally does not try to provide that depth.

Mnemosyne also has product surfaces that hprof-slurp does not target: structured multi-format reports, provenance-aware outputs, MCP for IDE automation, and AI-assisted explanation and fix workflows.

### Against Eclipse MAT

Mnemosyne's strongest advantage over Eclipse MAT is architecture and workflow style rather than pure feature count. The Rust implementation avoids JVM GC overhead in the parser and analysis engine, the streaming parser is already very fast, and the tool is designed for CLI, CI, and editor-copilot workflows from the start.

Mnemosyne also has two differentiators that are effectively unique in this comparison set:

- **Provenance tracking**, so users can see whether a result is graph-backed, fallback-driven, partial, or synthetic.
- **MCP and AI integration**, which let Mnemosyne plug into IDE agents and automation workflows that MAT does not address.

## 4. Where Competitors Win

### hprof-slurp

hprof-slurp wins on the narrow benchmark it is designed for: raw overview throughput and bounded memory at very large scales. Its multithreaded streaming pipeline and refusal to materialize a full object graph let it stay close to `~2 GB/s` with a published `~500 MB` ceiling even for a `34 GB` dump. Mnemosyne's current deep-analysis architecture cannot match that memory profile on equivalent workloads.

### Eclipse MAT

Eclipse MAT still wins on maturity, breadth of exploration, and the depth of its long-established GUI workflow. It has a broader and more battle-tested query and exploration surface, years of operational familiarity in the JVM ecosystem, and richer desktop-oriented inspection patterns than Mnemosyne currently offers.

## 5. Methodology Notes

### How Mnemosyne benchmarks were run

- Throughput and dominator timings come from Criterion benchmarks under `core/benches/`.
- RSS measurements come from `scripts/measure_rss.sh`, which profiles `parse`, `analyze`, and `leaks` and computes RSS-to-dump ratios.
- The main published real-world fixture is `resources/test-fixtures/heap.hprof`, a 156 MB Kotlin plus Spring Boot heap dump.
- Large-tier RSS validation currently comes from the Step 11 dense synthetic tiers at roughly 500 MB, 1 GB, and 2 GB dump sizes.

### Hardware placeholder

Hardware details were not captured in this document's source inputs. Future comparative publications should record at least:

- CPU model and core count
- RAM size
- Storage type
- OS and kernel
- Rust toolchain version

Without that metadata, external reruns should be treated as directional rather than strictly reproducible.

### Fair-comparison caveats

- Mnemosyne's streaming parser and deep-analysis paths solve different problems. Comparing `parse_heap` directly against full graph construction is misleading.
- hprof-slurp's published numbers reflect a different product goal: very fast triage with limited analysis depth.
- Eclipse MAT optimizes for rich desktop exploration and indexed re-query behavior, not low-overhead CLI parsing.
- Mnemosyne's current external comparison section uses published competitor figures from roadmap research, not an in-repo apples-to-apples rerun on identical hardware and fixtures.
- The honest current claim is: **Mnemosyne already has strong published internal baselines and competitive product differentiation, but it does not yet have a fully normalized external benchmark shootout.**