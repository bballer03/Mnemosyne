# Mnemosyne v0.3.0 Release Notes

> Release date: 2026-04-26
> Tag: v0.3.0
> Previous release: [v0.2.0](release-notes-v0.2.0.md)

Mnemosyne v0.3.0 focuses on production-readiness work already shipped on this branch: overview-mode streaming, CI regression policies, allocation flame graphs, targeted OQL expansion, and a comparative benchmark harness backed by a published partial WSL report.

## Highlights

- **Streaming Overview Mode**: `--mode auto|deep|overview` now flows across CLI, MCP, and core. `auto` flips to overview at 4 GiB by default, overview mode never allocates the `ObjectGraph`, and overview reports label approximate shallow-size output honestly instead of implying retained-size semantics.
- **CI Regression Policies**: `mnemosyne-cli ci-check` adds a TOML policy DSL, 10 predicates, four output formats (`text`, `json`, `junit`, `github-actions`), and the `0` through `4` exit-code contract. When `auto` resolves to overview, deep-only rules are skipped with structured reasons instead of being silently mis-evaluated.
- **Allocation-Site Flame Graphs**: `mnemosyne-cli flamegraph` exports retained-size flame graphs with three rooting strategies (`dominator`, `class-hierarchy`, `gc-root-path`) and three output formats (`svg`, `folded-stack`, `json`). SVG rendering is powered by `inferno`.
- **OQL Targeted Expansion**: the shipped M7-4 query slice adds `@retainedSize`, real `@toString`, `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS x.field`, and `IS NULL` / `IS NOT NULL` to the shared query surface. Deep-only features fail with structured `feature_unavailable_in_overview_mode` errors when the executor is reused without a deep graph.
- **Comparative Benchmark Harness**: `scripts/bench/` now ships the comparative harness (`measure_run.sh`, `run_comparative.sh`, `equivalence.py`), and the partial publication in [docs/benchmarks/comparative-v0.3.0.md](benchmarks/comparative-v0.3.0.md) records the current WSL run. That publication covers `mnemo-overview` and `hprof-slurp` on `small`, `medium`, and `large` with `N=3`; Eclipse MAT, `mnemo-deep`, equivalence, and the reference-workstation rerun remain future work.

## What's New in Detail

### M7-1 - Streaming Overview Mode

`mnemosyne-cli analyze heap.hprof --mode overview --format json`

The new overview path gives Mnemosyne a bounded-memory triage mode for large dumps while keeping deep mode intact for graph-backed investigation. `auto` now resolves between `deep` and `overview` at the CLI and MCP boundary, with a default cutoff of 4 GiB and an override through `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD`. Overview output stays explicit about its limits: no dominator tree, no retained sizes, and no leak suspects. Full semantics live in [milestone-7-production-readiness.md](design/milestone-7-production-readiness.md).

### M7-2 - CI Regression Policies

`mnemosyne-cli ci-check heap.hprof --policy policy.toml --format junit --output policy.xml`

Mnemosyne can now act as its own heap-regression gate instead of requiring custom CI glue. Policies are TOML-backed, severity-aware, and renderable for human terminals, JSON consumers, JUnit dashboards, and GitHub Actions annotations. The important contract change is honesty around mode compatibility: overview-compatible predicates still run in overview mode, while deep-only predicates are skipped or rejected with structured reasons depending on how mode was requested. Full policy semantics live in [milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md).

### M7-3 - Allocation-Site Flame Graphs

`mnemosyne-cli flamegraph heap.hprof -o flame.svg --mode deep`

The new flamegraph surface projects the retained-size view of a dump into exportable SVG, folded-stack text, or JSON without changing the existing `AnalyzeResponse` wire contract. `dominator` is the default rooting strategy, while `class-hierarchy` and `gc-root-path` provide alternate views for class families and reachability chains. This command is intentionally deep-mode-only; explicit overview mode, or `auto` that resolves to overview, exits with code `5` instead of pretending a graph-backed artifact exists. Full rendering and rooting semantics live in [milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md).

### M7-4 - OQL Targeted Expansion

`mnemosyne-cli query heap.hprof "SELECT @gcRootPath FROM \"com.example.Target\" WHERE @gcRootPath CONTAINS 'ThreadLocal'"`

The shared query engine now covers the highest-value MAT-style gap for real triage: retained-size filtering, synthetic string rendering, GC-root-path inspection, substring matching, one-hop referent projection, and null checks. The standard `query` CLI already builds the deep graph it needs, so most users see the richer behavior directly through the existing command surface. The shared executor also now has a structured deep-only error path for overview-backed callers, which matters for reused or embedded query flows even though the everyday CLI still runs deep today. Full grammar and mode rules live in [milestone-7-4-oql-targeted-expansion.md](design/milestone-7-4-oql-targeted-expansion.md).

### M7-5 - Comparative Benchmarks

`scripts/bench/run_comparative.sh --fixtures-dir resources/test-fixtures --output-dir docs/performance/raw --runs 3 --tools mnemo-overview,hprof-slurp --fixtures small,medium,large`

The branch now ships a Linux-first comparative harness plus a published partial report in [docs/benchmarks/comparative-v0.3.0.md](benchmarks/comparative-v0.3.0.md). The current publication is intentionally narrow: WSL2 on `/mnt/d`, `mnemo-overview` and `hprof-slurp` only, `small` through `large` only, `N=3`, and no published RSS comparison table because the run used compatibility shims instead of GNU `/usr/bin/time -v`. The strongest evidence in the published report is credible streaming behavior on the 6.47 GB `large` fixture in WSL; the native-Linux reference-workstation rerun remains user-owned. Full methodology lives in [milestone-7-5-comparative-benchmarks.md](design/milestone-7-5-comparative-benchmarks.md).

## Breaking Changes

No breaking changes; v0.2.0 users can upgrade in place.

The [0.3.0] section of [CHANGELOG.md](../CHANGELOG.md#030---2026-04-26) introduces new commands, modes, and report surfaces, but it does not document any breaking CLI, MCP, report-shape, or configuration changes.

## New CLI Surface

The packaged binary remains `mnemosyne-cli`.

| Command | Purpose |
|---|---|
| `mnemosyne-cli analyze --mode {auto,deep,overview} <heap>` | Streaming or deep triage, depending on mode resolution |
| `mnemosyne-cli ci-check <heap> --policy <file>` | Heap-regression gating for CI |
| `mnemosyne-cli flamegraph <heap> -o <out>` | Retained-size flame-graph export |
| `mnemosyne-cli query <heap> "<oql>"` | OQL with the targeted M7-4 operators and pseudo-attributes |

## New Exit Codes

| Exit code | Surface | Meaning |
|---|---|---|
| `5` | `flamegraph` | Feature unavailable in overview mode; rerun with `--mode deep` when the machine has enough RAM |
| `6` | `query` | Deep-only query feature was evaluated without a deep graph |

See [docs/troubleshooting.md](troubleshooting.md) for the user-facing remediation guidance behind both codes.

## Known Limitations

- **Comparative benchmarks are partial.** The published WSL run covers Mnemosyne overview mode and `hprof-slurp` on `small`, `medium`, and `large` fixtures with `N=3`. Eclipse MAT comparison, Mnemosyne deep-mode rows, the 10 GiB fixture, and Jaccard equivalence remain pending the native-Linux reference-workstation rerun. See [docs/benchmarks/comparative-v0.3.0.md](benchmarks/comparative-v0.3.0.md) for the published caveats.
- **OQL overview-mode feature-unavailable handling is structurally implemented before it is fully user-reachable everywhere.** The shared executor now emits `feature_unavailable_in_overview_mode`, but the standard `query` CLI still builds the deep graph today, so not every CLI or MCP path exposes that mismatch end to end yet.
- **The reference workstation spec is published but not yet executed against.** The current comparative report records WSL-on-NTFS numbers, not native-Linux reference-workstation numbers.

## Upgrade Instructions

The wired v0.3.0 distribution channels are GitHub release archives, source builds, and the GHCR image produced by the tag-triggered release workflow. Homebrew remains a post-release follow-up, and crates.io publication is not part of the automated v0.3.0 release flow.

- **GitHub Release archive**: the `v0.3.0` release page is expected to carry platform archives for the five wired targets in `release.yml`.
- **Docker**: after the `v0.3.0` tag is published, `docker pull ghcr.io/bballer03/mnemosyne:0.3.0`
- **Source**: `git checkout v0.3.0 && cargo build --release --workspace`
- **Homebrew**: `brew upgrade mnemosyne` after Slice M7-6.D updates `HomebrewFormula/mnemosyne.rb` with the v0.3.0 archive checksums.

## Dependency Changes

The runtime dependency deltas most visible in v0.3.0 are:

| Dependency | Why it matters in v0.3.0 |
|---|---|
| `inferno 0.11.21` | SVG flame-graph rendering for `mnemosyne-cli flamegraph` |
| `toml 0.8` | Runtime policy-file parsing in `mnemosyne-core` for `ci-check` |
| `regex 1` | Query, policy, and prompt-redaction matching shipped on the v0.3.0 branch |
| `sha2 0.10` | Hashed audit-log support for provider-mode AI flows on the v0.3.0 branch |
| `serde_yaml 0.9` | YAML-backed provider prompt-template loading that ships on the v0.3.0 branch |
| `reqwest 0.12` | Provider-mode AI transport carried into the v0.3.0 runtime |

## Links

- [CHANGELOG entry](../CHANGELOG.md#030---2026-04-26)
- [Comparative report](benchmarks/comparative-v0.3.0.md)
- [M7 parent design](design/milestone-7-production-readiness.md)
- [M7-2 design](design/milestone-7-2-ci-regression-policies.md)
- [M7-3 design](design/milestone-7-3-allocation-site-flame-graphs.md)
- [M7-4 design](design/milestone-7-4-oql-targeted-expansion.md)
- [M7-5 design](design/milestone-7-5-comparative-benchmarks.md)
- [M7-6 design](design/milestone-7-6-v0-3-0-release.md)
- [Roadmap](roadmap.md)