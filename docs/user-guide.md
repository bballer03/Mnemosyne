# Mnemosyne User Guide

This guide is the practical, end-to-end companion to Mnemosyne's CLI and MCP surfaces. It focuses on how to use the current v0.3.0 runtime effectively without repeating material that already lives in the quickstart, configuration reference, or MCP API reference.

For a fast first run, start with [QUICKSTART.md](QUICKSTART.md). For the full config surface, see [configuration.md](configuration.md). For the stdio MCP wire contract, see [api.md](api.md). For installation details and release packaging, see [../README.md](../README.md).

## 1. Introduction

Mnemosyne is a JVM heap-analysis tool for engineers who need answers from `.hprof` dumps without waiting on a slow toolchain or manually stitching together multiple utilities.

Today it combines:

- Rust-based parsing for fast summary and graph-backed analysis paths.
- Retained-size flame-graph exports for dominator, class-hierarchy, and GC-root-path visualization.
- Real heap investigation features such as retained sizes, dominators, GC-root tracing, string analysis, collection inspection, thread inspection, classloader reporting, and top-instance ranking.
- Explicit provenance markers so fallback, synthetic, partial, and placeholder output is labeled instead of being presented as authoritative fact.
- MCP-native integration so the same core analysis surface can be used from editors and automation.
- AI-assisted explanation and fix-generation paths that can run in offline `rules` mode or provider-backed `provider` mode.

Mnemosyne is a good fit for:

- JVM engineers investigating memory growth, retained-size hotspots, or leak candidates.
- CI and release pipelines that need machine-readable heap summaries or regression artifacts.
- Editor-based workflows where heap analysis should be callable through MCP instead of a one-off shell session.

What makes it different from a basic heap-summary tool is that the lightweight parse path, the graph-free overview path, and the full deep path live in one CLI. You can start with a lightweight parse, move into bounded-memory overview triage or graph-backed investigation, then keep going into AI explanation, source mapping, or MCP automation without changing tools.

## 2. Installation

Mnemosyne ships CLI and (on post-M16 tags) desktop GUI channels. Use the path that matches your environment; full release filenames and signing notes live in [../README.md](../README.md).

### Desktop app (GUI) — unzip → double-click goal

**Goal:** download → unzip/mount → double-click. No Java/JVM or developer toolchain.

**Honesty:**
- **JVM-free alone is not enough.** Windows needs the **WebView2** runtime; Linux AppImage/deb paths need **WebKitGTK** on many distros; macOS unsigned builds may hit **Gatekeeper**.
- Prefer `Mnemosyne-<version>-windows-x64-portable.zip` on Windows when attached to the release: unzip → double-click `Mnemosyne.exe`, labeled **portable with WebView2 prerequisite**.
- Prefer frozen `Mnemosyne-<version>-linux-{x86_64,aarch64}.AppImage` on Linux (`chmod +x`, then run) and `Mnemosyne-<version>-macos-{aarch64,x64}-app.zip` on macOS; keep MSI/setup, DMG, `.deb`, and `.rpm` for managed installs.
- **WSL cannot prove packaged GUI smoke.** Treat WSL work as command/unit/build evidence only; launch claims need matching native hosts.

First-run intent (when the desktop shell is available): open the app → choose an `.hprof` → select an analysis profile → investigate. From the workbench shell you can also reach **Policies**, **Snapshots** (list/save/remove), **Flamegraphs**, and the **Assistant** Investigation session (`/assistant`) without dropping to the CLI. The Assistant keeps measured heap facts separate from advisory AI text (rules mode offline by default) and deep-links to deterministic power views — it does **not** replace MAT-equivalent analysis. Capability status and caveats (including deferred snapshot-open, WSL GUI limits, and M23 NOT-proven live-provider/GUI rows) live in [product/ui-capability-matrix.md](product/ui-capability-matrix.md), [evidence/m20-ui-workbench.md](evidence/m20-ui-workbench.md), and [evidence/m23-guided-investigation.md](evidence/m23-guided-investigation.md). See the README Desktop section for per-OS steps and [../SECURITY.md](../SECURITY.md#desktop-app-distribution-m16) for SmartScreen/Gatekeeper workarounds.

### Cargo install

```bash
cargo install mnemosyne-cli
```

### GitHub Releases (CLI)

Download the tagged `mnemosyne-cli` archive for your platform from the repository Releases page. The README covers the current release artifacts and supported targets.

### Homebrew

```bash
brew install ./HomebrewFormula/mnemosyne.rb
```

### Docker

```bash
docker pull ghcr.io/bballer03/mnemosyne:0.2.0
docker run --rm -v /path/to/dumps:/data:ro ghcr.io/bballer03/mnemosyne:0.2.0 parse /data/heap.hprof
```

### Build from source

```bash
git clone https://github.com/bballer03/mnemosyne
cd mnemosyne
cargo build --release
./target/release/mnemosyne-cli --help
```

## 3. Quick Start

The fastest route from "I have a heap dump" to "I have a usable analysis" is already documented in [QUICKSTART.md](QUICKSTART.md).

That guide covers:

- capturing a heap dump with `jmap`
- using `parse` for a lightweight first pass
- using `leaks` and `analyze` for deeper inspection
- exporting retained-size flame graphs through `flamegraph`
- saving reports in text, HTML, JSON, or TOON
- inspecting the effective config with `config`
- starting the stdio MCP server with `serve`

If you are new to Mnemosyne, read that guide first, then return here for the complete command reference and longer workflows.

## 4. CLI Command Reference

The packaged binary name is `mnemosyne-cli`.

### Global options

These flags apply before the subcommand:

- `-c, --config <FILE>`: load a specific config file.
- `-v, --verbose`: increase CLI verbosity. It can be repeated.

Important current runtime truth:

- there is no global `--format`
- there is no global `--quiet`
- there is no global `--no-ai`
- report rendering lives on `analyze`; there is no standalone `report` subcommand in the current CLI
- `ci-check` is the separate policy-gate surface; it owns `--format text|json|junit|github-actions`, `--output`, and `--fail-on`
- `flamegraph` is the separate retained-size visualization surface; it owns `--root`, `--format svg|folded-stack|json`, required `--output`, and exit code `5` for overview-mode mismatch

### `parse`

Use `parse` when you want the fastest possible look at a heap dump before committing to a graph-backed analysis pass.

Usage:

```bash
mnemosyne-cli parse heap.hprof
```

Flags:

- `--mode auto|deep|overview`
- `parser.max_objects` from config or `MNEMOSYNE_MAX_OBJECTS` still affects the underlying parse job

What it does:

- validates the input path
- resolves the requested mode at the CLI boundary (`auto` by default)
- reads the HPROF header
- in `deep`, prints summary metadata, record counts, aggregate record-category sizes, and top record tags without building the full object graph
- in `overview`, uses bounded accumulators to surface class-resolved top-N data, GC-root counts, and capped thread-frame samples without building the `ObjectGraph`

Example:

```bash
mnemosyne-cli parse heap.hprof
mnemosyne-cli parse heap.hprof --mode overview
```

Expected output pattern:

```text
Heap path: heap.hprof
File size: 2.40 GB
Format: JAVA PROFILE 1.0.2 | Identifier bytes: 8 | Timestamp(ms): 1709836800000
Estimated objects: 1234567
Total HPROF records: 5678901
Top heap record categories by aggregate bytes:
  #  Record Category        Bytes      Share  Entries
  1  INSTANCE_DUMP          421.00 MB  50.1%  345678
Top record tags:
  ...
```

Choose `parse` first when you want to confirm that a dump is valid, estimate scale, or decide whether a deeper investigation is worth the time and memory.

`auto` resolves to overview for dumps at or above 4 GiB by default, or whatever byte threshold `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` supplies. Overview mode is streaming and honest: it reports approximate shallow sizes only, not retained sizes, dominator data, or leak suspects.

### `snapshot`

Use `snapshot` when you want to explicitly manage the parse-once-query-many cache instead of relying on implicit auto-caching alone. A snapshot stores an already-parsed `ObjectGraph` + `DominatorTree` pair to disk, keyed by the heap file's SHA-256 hash, so a later `analyze`/`leaks`/`gc-path`/`inspect`/`query` run can deserialize it instead of re-parsing the HPROF binary from scratch.

Usage:

```bash
mnemosyne-cli snapshot save <HEAP> [--output <DIR>]
mnemosyne-cli snapshot load <HASH_OR_PATH>
mnemosyne-cli snapshot list
mnemosyne-cli snapshot rm <HASH>
```

Flags:

- `snapshot save`: `--output <DIR>` — override the default cache root for this save only
- `snapshot load` / `snapshot rm`: positional `<HASH_OR_PATH>` / `<HASH>`

What each subcommand does:

- `save`: parses the heap dump (deep mode only — overview mode never builds an `ObjectGraph`, so there is nothing to snapshot) and atomically writes its object graph + dominator tree to the cache under `<cache-root>/<heap-sha256>.json`, then prints the manifest (hash, object count, `has_field_data`, schema version, created-at)
- `load`: deserializes a cached snapshot by hash or direct file path and prints its manifest; it does not run any analysis on its own — pair it with `--snapshot` on another command
- `list`: prints a table of every cached snapshot's hash, heap path, object count, created-at, and schema version
- `rm`: deletes a cached snapshot by hash; removing a hash that doesn't exist is a loud `snapshot_not_found` error, not a silent no-op

Cache location: `dirs::cache_dir()/mnemosyne` (for example `~/.cache/mnemosyne/` on Linux, `~/Library/Caches/mnemosyne/` on macOS, `%LOCALAPPDATA%\mnemosyne\` on Windows), overridable with the `MNEMOSYNE_SNAPSHOT_DIR` environment variable. Snapshots inherit the same sensitivity as the source HPROF file (they can contain retained string/field contents when `--retain-field-data` was used to build them) — treat the cache directory with the same care as your heap dumps, and `snapshot rm` when you're done with a sensitive dump.

Example:

```bash
mnemosyne-cli snapshot save heap.hprof
mnemosyne-cli snapshot list
mnemosyne-cli analyze heap.hprof --snapshot a1b2c3d4...
mnemosyne-cli snapshot rm a1b2c3d4...
```

Expected output pattern:

```text
Snapshot saved: a1b2c3d4e5f6...
  Heap path: heap.hprof
  Object count: 1234567
  Has field data: false
  Schema version: 1
  Created at: 1745766000
```

`mnemosyne-cli snapshot list` with cached entries:

```text
Cached snapshots:
  Hash      Heap Path    Objects   Created At   Schema
  a1b2c3d4  heap.hprof   1234567   1745766000   1
```

Beyond explicit `snapshot save|load|list|rm`, every command that currently parses a heap dump directly (`analyze`, `leaks`, `gc-path`, `inspect`, `query`) also accepts additive `--snapshot <HASH_OR_PATH>` and `--refresh` flags:

- no `--snapshot` and no `--refresh`: silently checks the default cache dir for a fresh snapshot matching the heap file's current SHA-256; uses it if present, otherwise parses normally and best-effort writes a new cache entry (a cache-write failure never fails your actual command, it just means no entry got cached)
- `--snapshot <HASH_OR_PATH>`: loads exactly that cached entry instead of parsing; errors loudly (never silently falls back to a fresh parse) if the entry is missing, corrupt, schema-mismatched, or stale relative to the heap file you passed
- `--refresh`: always re-parses the heap dump and overwrites its cache entry, even if a fresh cached snapshot already exists

Exit codes (additive on `analyze`/`leaks`/`gc-path`/`inspect`/`query`, and on `snapshot load`/`snapshot rm`): `10` `snapshot_not_found` (explicit `--snapshot <key>` / `snapshot load`/`rm` key doesn't exist), `11` `snapshot_schema_mismatch` (cached snapshot's schema version doesn't match the running binary), `12` `snapshot_stale_source` (the heap file's bytes changed since the snapshot was taken), `13` `snapshot_corrupt` (the cached file failed to deserialize). These four codes only apply to *explicit* `--snapshot`/`snapshot load` usage — a cache miss during silent auto-discovery (no `--snapshot` passed) is never an error, since "nothing cached yet" is the expected first-run state.

Current limitation: the only thing cached is the object graph + dominator tree. M8's analyzer outputs (referrer report, object inspections, thread frame-locals) are *not* precomputed into the snapshot — they're cheap enough to recompute from the loaded graph on every call, so caching them would only add staleness risk and file bloat for no real speed win.

### `analyze`

`analyze` is the main report-generation command. It runs the graph-backed analysis pipeline when possible, can attach optional investigation reports, and is the only CLI surface that currently owns `--format` and `--output-file`.

Usage:

```bash
mnemosyne-cli analyze <HEAP> [OPTIONS]
```

Flags:

- `--mode auto|deep|overview`
- `--format text|markdown|html|json|toon`
- `--profile overview|incident-response|ci-regression`
- `--group-by class|package|classloader|superclass`
- `--duplicate-arrays` — attach duplicate primitive-array content detection (requires field-data retention, same precondition as `--strings`)
- `--by-referrer` — attach a group-by-referrer report ranking objects by incoming-reference count
- `-o, --output-file <FILE>`
- `--ai`
- `--threads`
- `--strings`
- `--collections`
- `--classloaders`
- `--top-instances`
- `--top-n <N>`
- `--min-capacity <N>`
- `--package <PKG>[,<PKG>...]`
- `--leak-kind <KIND>[,<KIND>...]`
- `--snapshot <HASH_OR_PATH>` — load exactly this cached snapshot instead of parsing; errors loudly if missing/corrupt/stale/schema-mismatched
- `--refresh` — always re-parse and overwrite the snapshot cache entry, even if a fresh one exists

What it does:

- validates the heap file
- resolves the requested mode at the CLI boundary (`auto` by default)
- uses the configured analysis filters plus any command-line overrides
- with no `--snapshot`/`--refresh`: silently auto-uses a fresh matching snapshot from the default cache dir if one exists, else parses normally and best-effort caches the result
- in `deep`, builds the full analysis response, attempts graph-backed retained-size analysis first, and falls back honestly when needed
- in `overview`, skips object-graph analysis entirely and renders the streaming partial summary with approximate shallow sizes only
- renders the result in text, Markdown, HTML, JSON, or TOON
- with `--by-referrer`: ranks objects by incoming-reference count (tiebreak retained size), listing up to 5 top referrer classes per entry, via the new `core::analysis::referrers` module (`ReferrerReport`, optional field on `AnalyzeResponse`)

Profile behavior:

- `overview`: disables optional investigation reports and keeps defaults conservative
- `incident-response`: enables threads, strings, collections, classloaders, and top instances; ensures at least `--top-n 15` and `--min-capacity 32`
- `ci-regression`: enables top instances with tighter defaults and uses `--min-capacity 64`

Important distinction: `--profile overview` is a deep-mode preset. It is not the same thing as `--mode overview`.

Examples:

```bash
mnemosyne-cli analyze heap.hprof
mnemosyne-cli analyze heap.hprof --mode overview --format json
mnemosyne-cli analyze heap.hprof --group-by package --top-instances
mnemosyne-cli analyze heap.hprof --profile incident-response --threads --strings --collections
mnemosyne-cli analyze heap.hprof --format html --output-file heap-report.html
mnemosyne-cli analyze heap.hprof --format json --profile ci-regression
mnemosyne-cli analyze heap.hprof --by-referrer --top-n 10
```

Expected output pattern in text mode:

```text
Mnemosyne Analysis
Total Objects: ...
Detected Leaks: ...
Graph Nodes: ...

Histogram:
  Group                    Instances   Shallow   Retained
  com.example.cache        ...         ...       ...

Top Instances by Size:
  Rank  Class                           Shallow   Retained
  1     com.example.BigCache            ...       ...

Thread Report (... threads):
ClassLoader Report:
String Analysis (... strings, ... unique):
Collection Report (... collections):
```

`--by-referrer` output pattern:

```text
Top referenced objects (by incoming reference count)
-----------------------------------------------------
Objects considered: 4213
0x7f2a3000 com.example.SharedCache referrers=184 retained=536870912B top=[ConnectionPool(120), RequestHandler(64)]
```

`--threads` output now includes resolved frame-locals under each stack frame when `ROOT_JAVA_FRAME`/`ROOT_JNI_LOCAL` roots are present:

```text
Stack: pool-1-thread-3
  at com.example.Worker.run(Worker.java:42)
      local: 0x00001000 (com.example.Task)
      jni-local: 0x00002000 (java.nio.ByteBuffer)
```

`--classloaders` output ships **two independent leak signals that coexist** (neither replaces the other, see [design/milestone-13-classloader-explorer.md](design/milestone-13-classloader-explorer.md) §3.3): the pre-existing single-loader `potential_leaks` heuristic (a loader that retains a lot but declares almost no classes of its own) and the newer cross-loader "Duplicate classes across loaders" signal, MAT's actual "Duplicate Classes" report shape and the defining pattern of the classic Tomcat/Jetty/Spring hot-redeploy leak — the same class name loaded by two or more distinct classloaders that don't know about each other. The per-loader table also gains an `Ancestors` column reporting the length of each loader's resolved parent chain (bounded walk, depth 16, cycle-guarded against adversarial/malformed HPROF data). Both new sections print only when non-empty:

```text
ClassLoader Report:
  Loader                     Classes   Ancestors   Instances   Shallow    Retained
  com.example.WebappLoader   142       1           8213        4.2 MB     61.8 MB

Duplicate classes across loaders (2):
  com.example.webapp.RequestHandler  loaded by 3 loaders: 0x1000, 0x2400, 0x3800
  com.example.webapp.SessionCache    loaded by 2 loaders: 0x1000, 0x2400

Potential classloader leaks:
  com.example.WebappLoader (Retains 61.80 MB but loads only 2 classes)
```

`unique_class_count` (classes loaded by a given loader and no other, derived from the same grouping pass that builds the duplicate-classes list) is computed and available in JSON/TOON output on each `ClassLoaderInfo` entry, but is not currently rendered as its own text-table column.

When you write to a file, the CLI prints a confirmation instead of dumping the report to stdout:

```text
Report (text/plain) written to heap-report.txt
```

Overview mode renders a different banner-led report focused on top classes, instance samples, GC roots, and capped thread frames. It explicitly states that retained sizes, the dominator tree, and leak suspects are not available in that mode.

See [`snapshot`](#snapshot) for the `--snapshot`/`--refresh` cache flags shared with `leaks`, `gc-path`, `inspect`, and `query`. Exit codes `10`-`13` apply only to explicit `--snapshot <key>` usage (see the `snapshot` section for the full table); a silent cache miss during auto-discovery is never an error.

### `ci-check`

Use `ci-check` when you want Mnemosyne itself to decide pass/fail for a heap regression policy in local automation or CI.

Usage:

```bash
mnemosyne-cli ci-check <HEAP> --policy <FILE> [OPTIONS]
```

Flags:

- `--policy <FILE>`
- `--mode auto|deep|overview`
- `--format text|json|junit|github-actions`
- `--output <FILE>`
- `--fail-on info|warning|error|critical`
- `--baseline <BEFORE_HEAP>` (M10-B) — required only when the loaded policy contains an `object_growth_threshold` rule

What it does:

- loads a dedicated TOML policy file from `--policy`
- if the policy contains an `object_growth_threshold` rule, requires `--baseline` and runs a `--mode object` diff between `--baseline` and `<HEAP>` up front, once, regardless of how many rules need it
- resolves the requested mode at the CLI boundary (`auto` by default)
- in `deep`, runs the full analysis path and evaluates the resulting `AnalyzeResponse`
- in `overview`, parses the bounded-memory overview summary and evaluates the resulting `OverviewSummary`
- renders the selected policy report format and exits with a CI-oriented status code

Policy TOML shape:

- optional `[meta]` for display metadata
- optional `[defaults]` for default rule severity
- repeated `[[rule]]` blocks for predicate checks

The current policy surface supports 12 predicates. Overview-compatible predicates are `total_bytes`, `total_instances`, `class_instances`, `class_bytes`, `loaded_class_count`, `gc_root_count`, and `provenance_must_not_contain`. Deep-only predicates are `leak_count`, `retained_size`, `dominator_root_count`, `classloader_leak_count`, and `object_growth_threshold`. For the full catalog and field-level schema, see [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md).

`classloader_leak_count` (M13) thresholds on the number of `DuplicateClassGroup` entries — the cross-loader "same class name loaded by 2+ distinct classloaders" signal, not the older single-loader `potential_leaks` heuristic (there is no predicate over `potential_leaks`). Like the other deep-only predicates, it is skipped (not errored) on overview-mode input, and also skipped if the policy is evaluated against a deep `AnalyzeResponse` that never had classloader analysis enabled — `ci-check` handles this automatically by turning on `enable_classloaders` whenever the loaded policy declares a `classloader_leak_count` rule, so no extra flag is needed:

```toml
[[rule]]
id = "no-classloader-duplicates"
predicate = "classloader_leak_count"
op = "<="
value = 0
severity = "error"
```

This example fails the build the moment any class name is loaded by more than one classloader in the analyzed heap — the standard first gate for catching a webapp redeploy leak before it compounds across further redeploys.

`object_growth_threshold` (M10-B) is the first predicate scoped to a specific class and the first genuinely **two-heap** predicate — it fails CI when a tracked object (or class of objects) grows beyond a per-class retained-size limit between `--baseline` and `<HEAP>`. It scans the object diff's `retained_changed` entries (comparing `|after_retained_bytes - before_retained_bytes|`) and `added` entries (comparing `retained_bytes`, since an added object has no "before") for every entry whose class matches the rule's `class` field — or every entry, when `class` is omitted. Multiple violating objects within one rule are reported as a single aggregate violation with a count and the worst (largest-delta) offender cited by id, not one violation per object:

```toml
[[rule]]
id = "no-runaway-cache-growth"
predicate = "object_growth_threshold"
class = "com.example.CacheEntry"   # omit to apply to every tracked object
op = "<="
value = 10485760                   # bytes; max allowed retained-size delta per object
severity = "error"
```

```bash
mnemosyne-cli ci-check heap.hprof --policy policy.toml --baseline before.hprof
```

Because this predicate cannot be evaluated without a baseline to diff against, `ci-check` refuses loudly — exit code `2`, the same family as a malformed policy file — when the loaded policy contains an `object_growth_threshold` rule and `--baseline` was not supplied. It never silently evaluates the rule with no growth data, and never silently skips it either.

Severity and mode behavior:

- the ladder is `info < warning < error < critical`
- omitted rule severities fall back to `[defaults].severity`, then to `error`
- `--fail-on` defaults to `error` and controls the process exit code only
- all renderers still include every violation and skipped rule, even when a violation is below `--fail-on`
- if `--mode auto` resolves to overview, deep-only rules are skipped and reported as skipped
- if `--mode overview` is explicit and the policy needs deep-only predicates, `ci-check` exits `4`

Exit codes:

- `0`: clean, or only violations below `--fail-on`
- `1`: at least one violation met or exceeded `--fail-on`
- `2`: invalid policy file or schema error, or an `object_growth_threshold` rule with `--baseline` omitted (M10-B)
- `3`: unreadable heap or analysis failure
- `4`: explicit `--mode overview` with a deep-only rule

Examples:

```bash
mnemosyne-cli ci-check heap.hprof --policy policy.toml
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format json --output policy.json
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format junit --output policy.xml
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format github-actions --fail-on warning
mnemosyne-cli ci-check heap.hprof --policy policy.toml --baseline before.hprof
```

### `flamegraph`

Use `flamegraph` when you want an exportable retained-size visualization from the deep object-graph path.

Usage:

```bash
mnemosyne-cli flamegraph <HEAP> -o <FILE> [OPTIONS]
```

Flags:

- `-o, --output <FILE>`
- `--root dominator|class-hierarchy|gc-root-path`
- `--format svg|folded-stack|json`
- `--mode auto|deep|overview`
- `--min-fraction <FRACTION>`
- `--title <TEXT>`
- `--max-frames <N>`

What it does:

- validates the heap file
- resolves the requested mode at the CLI boundary (`auto` by default)
- rejects explicit overview mode and `auto` runs that resolve to overview because flame graphs require the full `ObjectGraph` and `DominatorTree`
- calls the deep-mode `analyze_heap_with_graph()` path so the flamegraph renderer can reuse the same `AnalyzeResponse` plus graph internals without changing the serialized report contract
- collapses the graph into folded stacks using the selected rooting strategy
- renders the artifact as SVG, folded-stack text, or a JSON envelope

Rooting strategies:

- `dominator` (default): best for "what holds memory"; the folded stacks follow the dominator chain and reflect retained-size structure
- `class-hierarchy`: best for class-family aggregation; stacks follow the inheritance chain and weight classes by their aggregate shallow bytes
- `gc-root-path`: best for "why is this still reachable"; stacks follow shortest GC-root paths for leak suspects and the highest-retained objects

Output formats:

- `svg`: interactive browser-openable flame graph rendered through `inferno`
- `folded-stack`: plain text compatible with `inferno`, `flamegraph.pl`, and other folded-stack tooling
- `json`: pretty JSON envelope with `{ tool, subcommand, version, strategy, total_weight, truncated_to_other, frame_count, stacks }`

Mode and exit behavior:

- flame graphs require deep mode
- explicit `--mode overview` exits `5`
- `--mode auto` also exits `5` if the heap is at or above the 4 GiB auto-overview cutoff
- `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` overrides that cutoff if you need a different auto-mode boundary
- use `--mode deep` only when you have enough RAM for full object-graph analysis; otherwise prefer `parse --mode overview` or `analyze --mode overview` for triage

Examples:

```bash
mnemosyne-cli flamegraph heap.hprof -o flame.svg --mode deep
mnemosyne-cli flamegraph heap.hprof -o flame.folded --mode deep --format folded-stack --root class-hierarchy
mnemosyne-cli flamegraph heap.hprof -o flame.json --mode deep --format json --root gc-root-path
```

### `leaks`

Use `leaks` when you want a focused list of leak candidates without the full report surface from `analyze`.

Usage:

```bash
mnemosyne-cli leaks <HEAP> [OPTIONS]
```

Flags:

- `--min-severity low|medium|high|critical`
- `--package <PKG>[,<PKG>...]`
- `--leak-kind <KIND>[,<KIND>...]`
- `--snapshot <HASH_OR_PATH>` — load exactly this cached snapshot instead of parsing; errors loudly if missing/corrupt/stale/schema-mismatched
- `--refresh` — always re-parse and overwrite the snapshot cache entry, even if a fresh one exists

What it does:

- loads the configured analysis filters
- applies command-line overrides for severity, package allow-listing, and leak kinds
- with no `--snapshot`/`--refresh`: silently auto-uses a fresh matching snapshot from the default cache dir if one exists, else parses normally and best-effort caches the result
- prints a compact table plus per-leak descriptions and provenance details

Examples:

```bash
mnemosyne-cli leaks heap.hprof
mnemosyne-cli leaks heap.hprof --min-severity high
mnemosyne-cli leaks heap.hprof --package com.example --leak-kind cache,thread
```

Expected output pattern:

```text
Potential leaks:
  Leak ID               Class                           Kind    Severity  Retained   Instances
  com.example.CacheLeak com.example.CacheHolder         CACHE   HIGH      42.00 MB   3

  Leak: com.example.CacheLeak
    Description: Cache retains request state across sessions.
    Provenance:
      [FALLBACK] Graph-backed ranking was unavailable for this candidate.
```

If nothing survives the filters, Mnemosyne prints an explicit zero-result message instead of returning silently:

```text
No leak suspects detected.
```

See [`snapshot`](#snapshot) for the shared `--snapshot`/`--refresh` cache flags and exit codes `10`-`13` (explicit `--snapshot <key>` only).

### `gc-path`

Use `gc-path` when you already know a target object ID and want to see how it stays reachable from a GC root.

Usage:

```bash
mnemosyne-cli gc-path <HEAP> --object-id <ID> [--max-depth <N>]
mnemosyne-cli gc-path <HEAP> [--object-id <ID> | --by-class <CLASS_NAME>] --all-paths [--max-paths <N>] [--max-depth <N>]
```

Flags:

- `--object-id <ID>` — required unless `--by-class` is used
- `--max-depth <N>`
- `--all-paths` — return every enumerated GC root path instead of only the shortest one
- `--by-class <CLASS_NAME>` — find all-paths for every live instance of this class instead of a single `--object-id`; mutually exclusive with `--object-id`
- `--max-paths <N>` — default `20`; a **shared** budget across the whole `--all-paths`/`--by-class` query, not per-path or per-instance
- `--snapshot <HASH_OR_PATH>` — load exactly this cached snapshot instead of parsing; errors loudly if missing/corrupt/stale/schema-mismatched
- `--refresh` — always re-parse and overwrite the snapshot cache entry, even if a fresh one exists

What it does:

- traces a path from the requested object toward a root
- prefers a full `ObjectGraph` BFS path
- falls back to a budget-limited graph and then synthetic output when needed
- labels fallback output through provenance markers
- with `--all-paths`/`--by-class`: enumerates every GC root path up to the shared `--max-paths` budget instead of stopping at the first hit; the plain (no new flags) `gc-path` output is unaffected and stays byte-identical
- with no `--snapshot`/`--refresh`: silently auto-uses a fresh matching snapshot from the default cache dir if one exists, else parses normally and best-effort caches the result

Example:

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x00001000 --max-depth 8
```

Expected output pattern:

```text
GC path for 0x00001000:
#0 -> com.example.CacheEntry [0x00001000] via owner
#1 -> com.example.RequestCache [0x00000F40] via entries
ROOT -> java.lang.Thread [0x00000011] via <direct>
```

All-paths / by-class example:

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x00001000 --all-paths --max-paths 5
mnemosyne-cli gc-path heap.hprof --by-class com.example.CacheEntry --all-paths --max-paths 20
```

```text
GC root paths for 0x00001000 (3 found, not truncated):
  Path 1 (depth 4):
    ROOT -> java.lang.Thread [0x00000011] via <direct>
    #1 -> com.example.RequestCache [0x00000F40] via entries
    #2 -> com.example.CacheEntry [0x00001000] via owner
  Path 2 (depth 5):
    ...
  Path 3 (depth 6):
    ...
```

When the shared `--max-paths` budget is hit before enumeration finishes, the header instead reads `(20 found, truncated)` — the count found and the truncation state are both reported honestly rather than silently capping.

Exit codes: `0` success, `8` `--object-id` not found in the heap, `9` `--by-class` matches zero live instances, `10`-`13` explicit `--snapshot <key>` cache errors (see [`snapshot`](#snapshot)).

### `inspect`

Use `inspect` when you want a focused, single-object view — fields, refs in/out, dominator context — without running the full `analyze` report. This is the CLI/MCP equivalent of the UI's Object Inspector pane.

Usage:

```bash
mnemosyne-cli inspect <HEAP> --object-id <ID> [--retain-field-data] [--format text|json|toon]
```

Flags:

- `--object-id <ID>` — required
- `--retain-field-data` — opt in to typed field values (re-parses the heap with field data retained); without it, the `fields` section is omitted
- `--format text|json|toon` — default `text`
- `--snapshot <HASH_OR_PATH>` — load exactly this cached snapshot instead of parsing; errors loudly if missing/corrupt/stale/schema-mismatched
- `--refresh` — always re-parse and overwrite the snapshot cache entry, even if a fresh one exists

What it does:

- resolves the object's shallow/retained size, dominator parent/children, references out, and referrers in via existing `ObjectGraph`/`DominatorTree` accessors — no new graph-walking logic
- references and dominator context are structured (`object_id` + `class_name`), not baked display strings, so JSON/TOON/MCP consumers can chain the returned ids straight back into another `inspect`/`gc-path`/`query` call
- with `--retain-field-data`: also reads and renders typed instance field values
- with no `--snapshot`/`--refresh`: silently auto-uses a fresh matching snapshot from the default cache dir if one exists; when `--retain-field-data` is also passed, a cached snapshot lacking field data is treated as a miss (falls through to a fresh field-data-retaining parse) rather than silently serving fields-less data — but an *explicit* `--snapshot <key>` always loads exactly what's cached, so a field-data-less explicit snapshot yields no `fields` section even with `--retain-field-data`

Example:

```bash
mnemosyne-cli inspect heap.hprof --object-id 0x00001000 --retain-field-data
```

```text
Object 0x00001000  (com.example.CacheEntry)
  Shallow: 48 B   Retained: 1.20 MB
  Dominator parent: 0x00004000 (com.example.Cache)
  Dominator children: 2
  References out (1): 0x00003000 (java.lang.String)
  Referrers in (1): 0x00004000 (com.example.Cache)
  Fields (--retain-field-data only):
    key: com.example.Key = 0x00002000
```

Exit codes: `0` success, `8` `--object-id` not found in the heap, `10`-`13` explicit `--snapshot <key>` cache errors (see [`snapshot`](#snapshot)).

### `diff`

Use `diff` to compare two snapshots and highlight aggregate change between them.

Usage:

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Flags:

- `--mode {class|object}` — default `class` (today's behavior, byte-identical to v0.3.0). `object` additionally runs fingerprint-based per-object identity diffing (requires deep mode on both dumps).
- `--identity-strategy {class+retained|class+dominator|full-fingerprint}` — default `class+dominator`. Ignored when `--mode class`. `full-fingerprint` requires `--retain-field-data`.
- `--retained-bucket-bits <u8>` — default `10` (1 KB power-of-two retained-size bucket).
- `--retained-change-threshold <bytes>` — default `1048576`; minimum absolute retained-size delta for an object to appear in `retained_changed`.
- `--top <n>` — default `50`; per-section cap for `added` / `removed` / `retained_changed`.
- `--object-diff-min-retained <bytes>` — default `4096`; objects below this retained-size floor are skipped to keep memory bounded (hidden in `--help`, shown in `--help-long`).
- `--retain-field-data` — opt in to field-level retention; required for `full-fingerprint`.
- `--format {text|json|toon}` — default `text`.
- `--cross-reference-leaks` (M10-B) — opt in, default off. Ignored when `--mode class`.

What it does:

- validates both dumps
- prints total delta size and object-count delta
- prints top changed classes or record categories
- prints class-level retained deltas when both heaps build graph-backed diff context successfully
- with `--mode object`: fingerprints objects in both dumps (HPROF ids are never used as identity — they are not stable across dumps), then reports objects present only in `after` (`added`), only in `before` (`removed`), and present in both with a retained-size delta beyond the threshold (`retained_changed`), each with a dominator-class chain and reference chain, plus a `match_quality` block reporting the fingerprint collision rate
- with `--mode object --cross-reference-leaks`: additionally runs `detect_leaks()` against the after-heap and annotates any `added`/`retained_changed` entry whose class matches a leak suspect with that suspect's severity — connects "this object grew" with "this object grew *and* is already a flagged leak suspect." Text output appends a `[LEAK: <severity>]` suffix to matching lines; JSON/TOON carry the same data as `leak_severity` on the delta. Off by default, so pre-M10-B `diff --mode object` output is byte-identical when the flag is not passed.

Example:

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Expected output pattern:

```text
Heap diff: before.hprof -> after.hprof
  Delta size: +128.50 MB
  Delta objects: +18342
  Top changes:
    - com.example.CacheEntry: +84.20 MB (before 10.10 MB -> after 94.30 MB)
  Class-level retained deltas:
    Class                         Instances  Shallow              Retained Delta
    com.example.CacheEntry        +12000     10.10 -> 94.30 MB   +90.50 MB
```

Object-level example:

```bash
mnemosyne-cli diff before.hprof after.hprof --mode object
```

```text
object diff (strategy=class+dominator, bucket=1KB, threshold=1MB):
  added (3):
    com.example.UserSession    +52428800 bytes  count=1  dom=[Server,Pool,Cache,...]
  removed (1):
    com.example.LegacyCache    -67108864 bytes  count=2  dom=…
  retained_changed (5):
    com.example.RequestMap    +12582912 bytes  count=1->1  dom=…
  match quality: collision_rate=0.012  false_match_risk=Low  false_split_risk=Medium
```

With `--cross-reference-leaks`, an annotated line gets a suffix:

```text
    com.example.UserSession    +52428800 bytes  count=1  dom=[Server,Pool,Cache,...]  [LEAK: HIGH]
```

Exit codes: `0` diff produced, `2` I/O error, `3` heap parse error, `5` mode mismatch (e.g. `--mode object` against an overview-only dump), `6` fingerprint budget exceeded (`feature_unavailable_object_diff_too_large` — raise `--object-diff-min-retained` or use a smaller dump), `7` `full-fingerprint` requested without `--retain-field-data`.

Current limitation: object-level diff is a two-snapshot comparison only (no 3+ snapshot trend tracking). `ci-check --baseline` and `diff --cross-reference-leaks` shipped in M10-B; MCP wiring for both remains open future work.

### `fix`

Use `fix` when you want remediation suggestions and an example patch draft for a leak candidate.

Usage:

```bash
mnemosyne-cli fix <HEAP> [OPTIONS]
```

Flags:

- `--leak-id <ID>`
- `--project-root <DIR>`
- `--style minimal|defensive|comprehensive`

What it does:

- generates suggestions for the targeted leak set
- may use provider-backed AI when configured and enough source context exists
- otherwise falls back to heuristic patch guidance with provenance markers

Examples:

```bash
mnemosyne-cli fix heap.hprof --leak-id com.example.CacheLeak --style defensive
mnemosyne-cli fix heap.hprof --leak-id com.example.CacheLeak --project-root ./service
```

Expected output pattern:

```text
Fix for com.example.CacheHolder [com.example.CacheLeak] (Defensive, confidence 84%):
File: src/main/java/com/example/CacheLeak.java
Evict idle entries before they accumulate.
Patch:
--- a/src/main/java/com/example/CacheLeak.java
+++ b/src/main/java/com/example/CacheLeak.java
@@ ...
```

If nothing matches the requested criteria, the CLI prints:

```text
No fix suggestions available for the provided criteria.
```

### `query`

Use `query` to run the current OQL-style surface over the heap graph.

Usage:

```bash
mnemosyne-cli query <HEAP> "<QUERY>"
```

Flags:

- `--snapshot <HASH_OR_PATH>` — load exactly this cached snapshot instead of parsing; errors loudly if missing/corrupt/stale/schema-mismatched
- `--refresh` — always re-parse and overwrite the snapshot cache entry, even if a fresh one exists

What it does:

- builds the graph-backed query context
- with no `--snapshot`/`--refresh`: silently auto-uses a fresh matching snapshot from the default cache dir if one exists, else parses normally and best-effort caches the result
- parses the query string
- prints matched column names, match count, rows, and a truncation note when `LIMIT` cuts off the result set

Examples:

```bash
mnemosyne-cli query heap.hprof "SELECT @objectId, @className FROM \"com.example.*\" LIMIT 25"
mnemosyne-cli query heap.hprof "SELECT @objectId, entries FROM \"com.example.BigCache\" LIMIT 10"
```

Pseudo-attributes:

| Field | Semantics | Example |
|---|---|---|
| `@retainedSize` | Real retained bytes for each matched object. | `SELECT @objectId FROM "com.example.BigCache" WHERE @retainedSize > 1048576` |
| `@toString` | Synthetic string rendering: real `java.lang.String` contents when available, otherwise a stable class-and-id form. | `SELECT @objectId FROM "java.lang.String" WHERE @toString LIKE 'hello%'` |
| `@gcRootPath` | Shortest GC-root path rendered as `GcRoot/... -> ... -> target`, or `null` for unreachable objects. | `SELECT @gcRootPath FROM "com.example.Target" WHERE @gcRootPath CONTAINS 'ThreadLocal'` |

Operators:

| Operator | Semantics | Example |
|---|---|---|
| `LIKE` | SQL-style string pattern matching with `%` and `_` wildcards on built-in or retained instance fields. | `SELECT @objectId FROM "com.example.User" WHERE name LIKE 'admin%'` |
| `CONTAINS` | Plain substring matching on built-in or retained instance fields. | `SELECT @objectId FROM "com.example.User" WHERE name CONTAINS 'min'` |
| `OBJECTS x.field` | One-hop referent projection for object-reference fields. | `SELECT OBJECTS n.parent FROM "com.example.Node" WHERE payload IS NULL` |
| `IS NULL` / `IS NOT NULL` | Nullability checks for object-reference fields. | `SELECT @objectId FROM "com.example.Node" WHERE payload IS NOT NULL` |
| `=~` | Regex match on string-capable fields (uses the linear-time `regex` crate; malformed patterns fail at parse time). | `SELECT @objectId FROM "com.example.User" WHERE name =~ "^admin.*"` |
| `outbounds(id)` / `inbounds(id)` / `dominators(id)` | Traversal functions in `FROM` clauses: outgoing references, incoming referrers, or immediate-dominator chain. | `SELECT @objectId FROM outbounds(1) WHERE @objectId = 3` |
| Subqueries | One-level nesting via `FROM OBJECTS (SELECT ...)`. Deeper nesting returns a structured "nesting depth exceeded" error. | `SELECT * FROM OBJECTS (SELECT @objectId FROM "com.example.*" LIMIT 10)` |
| `UNION` | Combine two queries; results deduplicated by object id. Each side's own `LIMIT` applies before the merge. | `SELECT @objectId FROM "A" WHERE x < 3 UNION SELECT @objectId FROM "A" WHERE x > 7` |

Other query notes:

- single-quoted string literals now work alongside double-quoted ones
- `OBJECTS` is intentionally single-hop only in the shipped surface
- `SELECT @gcRootPath` returns `Null` for matched objects that are unreachable from any GC root
- M7-4 baseline: [design/milestone-7-4-oql-targeted-expansion.md](design/milestone-7-4-oql-targeted-expansion.md); M15 bounded expansion: [design/milestone-15-mat-backend-parity.md](design/milestone-15-mat-backend-parity.md)

Expected output pattern:

```text
Columns: @objectId, entries
Matched: 1
0x00001000 | 42
```

Mode behavior: the targeted M7-4 features depend on the deep graph-backed query path. The current `query` CLI already builds that deep path; when other callers reach the shared query engine without a deep graph, the runtime returns `FeatureUnavailableInOverviewMode` and the CLI reserves exit code `6` for that mismatch. Use overview-mode `parse` / `analyze` for large-dump triage, then come back to `query` when you need `@retainedSize`, `@toString`, `@gcRootPath`, `OBJECTS`, `IS NULL`, or `LIKE` / `CONTAINS` on retained instance fields. Exit codes `10`-`13` apply to explicit `--snapshot <key>` cache errors (see [`snapshot`](#snapshot)).

Shipped beyond M7-4 + M15 (M22.B–C): bounded multi-class `FROM` and 1–3 hop `OBJECTS` (4+ hops rejected). Named deferrals (not silent gaps): `eval(...)` scriptlets, arbitrary-depth subquery nesting, live JVM attach, and MAT `.index` interchange. Corpus honesty: handbook-linked cases are `documentation-referenced` only — see [docs/evidence/m22-oql-compatibility.md](evidence/m22-oql-compatibility.md). MAT golden / `mat-referenced` remain **NOT PROVEN** (0 cases). The M7-4 + M15 + M22 bounded list covers the highest-value MAT OQL workflows for heap triage without claiming golden MAT equivalency.

### Duplicate primitive-array detection

Use `--duplicate-arrays` on `analyze` (or MCP `analyze_heap` with `enable_duplicate_arrays: true`) to detect primitive arrays with identical element type, length, and byte-for-byte content — the same memory-waste pattern MAT flags alongside duplicate strings.

Requires field-data retention (the heap must have been parsed with retained array contents available — same precondition as `--strings`). The `incident-response` profile enables `--duplicate-arrays` automatically.

Output shape mirrors string duplicate detection: `DuplicateArrayGroup { element_type, content_hash, length, count, total_wasted_bytes }` in an optional `array_report` on `AnalyzeResponse`.

```bash
mnemosyne-cli analyze heap.hprof --duplicate-arrays --strings
```

### Plugin extension API (Phase 2)

Mnemosyne ships a **Phase-2 static plugin registry** (M15 Slice 15.F) for Rust projects that depend on `mnemosyne-core`:

- `AnalyzerPlugin` — custom analysis pass over `ObjectGraph` + optional `DominatorTree`; results append to `AnalyzeResponse::plugin_results` via `analyze_heap_with_plugins()`
- `ReportFormatterPlugin` — custom output format selectable via `OutputFormat::Custom(name)` and `render_report_with_plugins()`

Registration is compile-time only (`registry.register_analyzer(...)`, `registry.register_formatter(...)`). There is no filesystem discovery, no `cdylib` loading, and no CLI `--plugin` flag — Phase 3 dynamic loading stays gated per [design/m6-plugin-extension-system.md](design/m6-plugin-extension-system.md).

See `core/src/plugin/mod.rs` module docs and [design/milestone-15-mat-backend-parity.md](design/milestone-15-mat-backend-parity.md) §4.4.

### `explain`

Use `explain` when you want a natural-language explanation of one leak or the filtered leak set.

Usage:

```bash
mnemosyne-cli explain <HEAP> [OPTIONS]
```

Flags:

- `--leak-id <ID>`
- `--min-severity low|medium|high|critical`
- `--package <PKG>[,<PKG>...]`
- `--leak-kind <KIND>[,<KIND>...]`

What it does:

- forces AI explanation mode on for the command
- runs analysis with the selected filters
- validates `--leak-id` before generating the explanation
- prints model name, confidence, summary, and recommendation list

Examples:

```bash
mnemosyne-cli explain heap.hprof --leak-id com.example.CacheLeak
mnemosyne-cli explain heap.hprof --min-severity high --package com.example
```

Expected output pattern:

```text
Model: rules (confidence 83%)
The dominant retained set is rooted in a long-lived cache that still references request-scoped state.
Recommendations:
- Bound the cache.
- Add eviction on request completion.
```

### `chat`

Use `chat` for an interactive, bounded conversation about the current heap.

Usage:

```bash
mnemosyne-cli chat <HEAP>
```

Flags:

- no chat-specific CLI flags in the current runtime

What it does:

- analyzes the heap once at startup with AI disabled
- prints the top three leak candidates, or an explicit healthy-heap message when nothing survives filtering
- turns AI back on for the interactive question loop
- keeps only the last three turns in memory

Interactive commands:

- `/focus <leak-id>`
- `/list`
- `/help`
- `/exit`

Example session:

```text
$ mnemosyne-cli chat heap.hprof
Analyzed heap: heap.hprof
Top leak candidates:
  Leak ID               Class                     Kind   Severity  Retained  Instances
  com.example.CacheLeak com.example.CacheHolder   CACHE  HIGH      42.00 MB  3
Commands: /focus <leak-id>, /list, /help, /exit
chat> /focus com.example.CacheLeak
Focused leak: com.example.CacheLeak
chat> Why is this leaking?
Question: Why is this leaking?
Answer:
The cache owner outlives the request lifecycle and keeps stale entries reachable.
Recommendations:
- Add eviction when a request completes.
```

### `map`

Use `map` when you want likely source locations for a leak candidate.

Usage:

```bash
mnemosyne-cli map <LEAK_ID> --project-root <DIR> [--class <NAME>] [--no-git]
```

Flags:

- `--project-root <DIR>`
- `--class <NAME>`
- `--no-git`

What it does:

- uses the leak identifier and optional class hint to find likely source locations
- prints file, line, symbol name, and code snippet
- includes git metadata unless `--no-git` is set

Example:

```bash
mnemosyne-cli map com.example.CacheLeak --project-root ./service --class com.example.CacheHolder
```

Expected output pattern:

```text
Source candidates for `com.example.CacheLeak`:
- src/main/java/com/example/CacheHolder.java:118 (put)
    cache.put(sessionId, value);
    Git: Jane Doe @ abc1234 (2026-04-10) - Add request cache
```

### `serve`

Use `serve` to start Mnemosyne's stdio MCP server.

Usage:

```bash
mnemosyne-cli serve [--host <HOST>] [--port <PORT>]
```

Flags:

- `--host <HOST>`
- `--port <PORT>`

Current runtime truth:

- the server transport is stdio, not an HTTP or TCP listener
- `host` and `port` are accepted as configuration fields, but are currently informational

Example:

```bash
mnemosyne-cli serve
```

Expected interaction pattern:

```text
stdin  -> {"id":1,"method":"list_tools","params":{}}
stdout <- {"id":1,"success":true,"result":{"tools":[...]},"error":null}
```

For the full request and response contract, use [api.md](api.md) rather than treating this guide as a wire-format reference.

### `config`

Use `config` when you want to see the merged effective configuration and the source it came from.

Usage:

```bash
mnemosyne-cli config
mnemosyne-cli --config .mnemosyne.toml config
```

Flags:

- no nested config subcommands in the current runtime

What it does:

- prints the merged config as pretty JSON
- prints one follow-up line showing whether it came from built-in defaults or a file source

Expected output pattern:

```text
{
  "parser": {
    "use_mmap": true,
    "threads": null,
    "max_objects": null
  },
  ...
}
Using built-in defaults (no config file found).
```

### Report generation (current runtime path)

The current CLI does not expose a standalone `report` subcommand. Report generation is part of `analyze`.

Use these flags on `analyze` instead:

- `--format text|markdown|html|json|toon`
- `-o, --output-file <FILE>`

Examples:

```bash
mnemosyne-cli analyze heap.hprof --format markdown --output-file heap-report.md
mnemosyne-cli analyze heap.hprof --format html --output-file heap-report.html
mnemosyne-cli analyze heap.hprof --format json --output-file heap-report.json
mnemosyne-cli analyze heap.hprof --format toon --output-file heap-report.toon
```

## 5. Analysis Workflows

These workflows reflect how the current CLI is designed to be used in practice.

### Basic triage

Start light, then get progressively deeper only if the dump justifies it.

```bash
mnemosyne-cli parse heap.hprof
mnemosyne-cli analyze heap.hprof
mnemosyne-cli leaks heap.hprof
```

Why this works:

- `parse` confirms the dump is valid and shows shape without the graph cost
- `analyze` gives the full summary and retained-size context
- `leaks` then gives you a concise suspect list you can share or filter further

### Deep investigation

When the first pass suggests real heap pressure, enable the investigation modules in one run.

```bash
mnemosyne-cli analyze heap.hprof \
  --threads \
  --strings \
  --collections \
  --top-instances \
  --top-n 15 \
  --min-capacity 32
```

This is a good interactive workflow when you want to correlate retained-size hotspots with thread-local retention, duplicate string waste, oversized collections, and the largest individual objects. `--threads` output includes resolved `local:`/`jni-local:` lines under each stack frame wherever `ROOT_JAVA_FRAME`/`ROOT_JNI_LOCAL` GC roots are present.

### Reachability & references

Once `leaks` or `--top-instances` names a suspect, use the M8 reachability surfaces to see the full picture instead of just the shortest path.

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x00001000 --all-paths --max-paths 10
mnemosyne-cli gc-path heap.hprof --by-class com.example.CacheEntry --all-paths --max-paths 20
mnemosyne-cli analyze heap.hprof --by-referrer --top-n 10
mnemosyne-cli inspect heap.hprof --object-id 0x00001000 --retain-field-data
```

Recommended practice:

- use `gc-path --all-paths` when the shortest path alone doesn't explain retention, or `--by-class` when you want every reachable instance of a class merged into one query
- use `analyze --by-referrer` to find the objects other things point at the most — a strong signal for shared caches, registries, and listener lists
- use `inspect` for a one-shot, scriptable field-level view of a single object instead of paging through the full `analyze` report

### Flame graphs

Use `flamegraph` when you want a portable visualization artifact instead of a prose or table report.

```bash
mnemosyne-cli flamegraph heap.hprof -o flame-dominator.svg --mode deep --root dominator
mnemosyne-cli flamegraph heap.hprof -o flame-class.folded --mode deep --format folded-stack --root class-hierarchy
mnemosyne-cli flamegraph heap.hprof -o flame-gc-path.json --mode deep --format json --root gc-root-path
```

Recommended interpretation:

- use `dominator` for the broad retained-size answer to "what is holding memory"
- use `class-hierarchy` when you want a class-family rollup instead of object-to-object paths
- use `gc-root-path` when you want shortest-path reachability context for leak suspects and top retained objects

What you see in the SVG:

- a browser-openable interactive flame graph with colored stacked frames, hover tooltips, click-to-zoom navigation, and search
- the artifact is more useful as a file you open locally than as a static Markdown image, so the docs intentionally describe it instead of trying to inline a screenshot

If `--mode auto` would switch to overview because the heap is at or above the 4 GiB cutoff, `flamegraph` exits `5`; rerun with `--mode deep` only when you have enough RAM for the full graph-backed path.

### Leak resolution

Once you have a suspect, tighten the loop around explanation, reachability, and remediation.

```bash
mnemosyne-cli leaks heap.hprof
mnemosyne-cli explain heap.hprof --leak-id com.example.CacheLeak
mnemosyne-cli gc-path heap.hprof --object-id 0x00001000
mnemosyne-cli fix heap.hprof --leak-id com.example.CacheLeak --style defensive --project-root ./service
```

Recommended practice:

- use `leaks` to identify the candidate
- use `explain` to understand the likely retention story
- use `gc-path` when you need exact reachability context for a concrete object
- use `fix` after you know which code path you actually want to change

### CI regression workflow

Use `ci-check` when you want Mnemosyne itself to own the policy gate instead of wiring shell logic around `analyze`.

```bash
mnemosyne-cli analyze heap.hprof --profile ci-regression --format json --output-file analysis.json
mnemosyne-cli ci-check heap.hprof --policy .mnemosyne/policy.toml --format junit --output heap-policy.xml
```

This combination keeps the richer analysis artifact for later inspection while producing a deterministic policy result for CI. Swap `--format github-actions` when you want workflow annotations in GitHub Actions, or `--format json` when you want the structured `PolicyResult` envelope for a custom dashboard or follow-on automation.

### Heap comparison

Use `diff` when you already have a before/after pair and want to confirm whether a suspected fix changed the heap profile.

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Use this to answer questions like:

- did the retained footprint of one class drop after a change?
- did object count growth move from one subsystem to another?
- did the overall heap get smaller even if the leak is not fully gone?

## 6. AI Provider Setup

Mnemosyne supports three AI modes:

- `rules`: default, offline-safe, built into the repo
- `stub`: deterministic compatibility mode
- `provider`: external provider-backed mode

Provider-backed mode lives under the `[ai]` config section.

### Core `[ai]` options

```toml
[ai]
enabled = true
mode = "provider"
provider = "openai"
model = "gpt-4.1-mini"
temperature = 0.2
endpoint = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
max_tokens = 2000
timeout_secs = 30
```

Meaning:

- `enabled`: default AI on or off for surfaces that consult config
- `mode`: `rules`, `stub`, or `provider`
- `provider`: `openai`, `anthropic`, or `local`
- `model`: provider model name
- `temperature`: provider sampling temperature
- `endpoint`: override endpoint; required for `local`
- `api_key_env`: environment variable that stores the provider key
- `max_tokens`: provider response budget, and a low-budget hint for prompt trimming
- `timeout_secs`: provider request timeout

### Safe activation checklist

1. Keep the credential out of Mnemosyne TOML. `api_key_env` contains a variable name such as `OPENAI_API_KEY`, never the key value.
2. Store the config in a user-only location and restrict its permissions:

   ```bash
   install -d -m 700 ~/.config/mnemosyne
   chmod 600 ~/.config/mnemosyne/config.toml
   ```

3. Inject the key with an OS keychain, credential manager, service secret, or a non-echoing shell prompt:

   ```bash
   read -rsp "OpenAI API key: " OPENAI_API_KEY && echo
   export OPENAI_API_KEY
   test -n "${OPENAI_API_KEY:-}" && echo "OPENAI_API_KEY=true" || echo "OPENAI_API_KEY=false"
   ```

4. Run provider mode from the same environment, then clear an interactive shell value when finished:

   ```bash
   mnemosyne-cli --config ~/.config/mnemosyne/config.toml chat heap.hprof
   # or: mnemosyne-cli --config ~/.config/mnemosyne/config.toml serve
   unset OPENAI_API_KEY
   ```

Do not pass keys as command-line arguments, paste them into chat/evidence, or commit them in `.env`, TOML, shell scripts, or MCP client settings. Mnemosyne does not automatically load `.env` files. `mnemosyne-cli config` reports the configured environment-variable name, not the variable's value; the value is resolved only when provider mode sends a request.

For MCP, start `mnemosyne-cli serve` from an environment that can resolve `api_key_env`, or use the client/host's secret-reference mechanism. Avoid literal secrets in versioned JSON. Provider HTTP failures remain structured as `provider_error` or `provider_timeout` in MCP responses; switching the config back to `mode = "rules"` restores the offline default.

The current desktop Assistant bridge forwards `chatSession` to the native session adapter, but its `HeapSession` starts with `AppConfig::default()` and does not load the CLI config chain. Therefore this guide does not claim external-provider desktop chat is currently operator-configurable; the evidenced provider path is CLI/MCP. Desktop remains on rules mode unless a native host supplies a non-default in-memory config.

Optional task toggles:

```toml
[[ai.tasks]]
kind = "top-leak"
enabled = true

[[ai.tasks]]
kind = "healthy-heap"
enabled = true

[[ai.tasks]]
kind = "remediation-checklist"
enabled = true
```

Optional prompt override:

```toml
[ai.prompts]
template_dir = "/absolute/path/to/prompts"
```

### Privacy controls

Provider mode can redact sensitive material before it leaves the machine.

```toml
[ai.privacy]
redact_heap_path = true
redact_patterns = ["secret-token-[0-9]+", "customer-[0-9]+"]
audit_log = true
```

What these do:

- `redact_heap_path = true`: replaces outbound `heap_path` with `<REDACTED>`
- `redact_patterns`: regex-based prompt redaction across the fully rendered outbound prompt
- `audit_log = true`: emits hashed audit metadata for the redacted prompt without logging the raw prompt text

### OpenAI-compatible example

```toml
[ai]
enabled = true
mode = "provider"
provider = "openai"
model = "gpt-4.1-mini"
endpoint = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 30
max_tokens = 2000

[ai.privacy]
redact_heap_path = true
redact_patterns = []
audit_log = false
```

```bash
export OPENAI_API_KEY="sk-..."
```

### Anthropic example

```toml
[ai]
enabled = true
mode = "provider"
provider = "anthropic"
model = "claude-3-5-sonnet-latest"
api_key_env = "ANTHROPIC_API_KEY"
timeout_secs = 30
max_tokens = 2000
```

```bash
export ANTHROPIC_API_KEY="..."
```

### Local provider example

```toml
[ai]
enabled = true
mode = "provider"
provider = "local"
model = "local-model"
endpoint = "http://127.0.0.1:11434/v1"
timeout_secs = 30
max_tokens = 2000
```

Notes for local mode:

- `endpoint` is required
- no API key is required by default unless your local gateway expects one

### Environment variables

Common overrides:

```bash
export MNEMOSYNE_AI_ENABLED=true
export MNEMOSYNE_AI_MODE=provider
export MNEMOSYNE_AI_PROVIDER=openai
export MNEMOSYNE_AI_MODEL=gpt-4.1-mini
export MNEMOSYNE_AI_ENDPOINT=https://api.openai.com/v1
export MNEMOSYNE_AI_API_KEY_ENV=OPENAI_API_KEY
export MNEMOSYNE_AI_TEMPERATURE=0.2
export MNEMOSYNE_AI_MAX_TOKENS=2000
export MNEMOSYNE_AI_TIMEOUT_SECS=30
export MNEMOSYNE_AI_REDACT_HEAP_PATH=true
export MNEMOSYNE_AI_REDACT_PATTERNS="secret-token-[0-9]+,customer-[0-9]+"
export MNEMOSYNE_AI_AUDIT_LOG=true
```

Provider-key defaults when `api_key_env` is omitted:

- OpenAI-compatible: `OPENAI_API_KEY`
- Anthropic: `ANTHROPIC_API_KEY`
- Local: no default API key

## 7. MCP Integration

Mnemosyne's MCP surface is exposed through the stdio server started by `mnemosyne-cli serve`.

Start it manually with:

```bash
mnemosyne-cli serve
```

Practical guidance:

- treat it as a stdio tool, not an HTTP service
- call `list_tools` first if your client wants the live method catalog and parameter shapes
- use [api.md](api.md) for the actual request and response contract

Representative editor config for MCP-compatible clients such as VS Code or Cursor:

```json
{
  "mcpServers": {
    "mnemosyne": {
      "command": "mnemosyne-cli",
      "args": ["serve"],
      "env": {
        "MNEMOSYNE_CONFIG": "/absolute/path/to/.mnemosyne.toml"
      }
    }
  }
}
```

The exact configuration file name and UI for that block depends on the client version. The important part is the stdio command: `mnemosyne-cli serve`.

Useful MCP methods to know up front:

- `list_tools`
- `parse_heap`
- `detect_leaks`
- `analyze_heap`
- `diff_heaps`
- `query_heap`
- `map_to_code`
- `find_gc_path`
- `inspect_object`
- `open_snapshot`
- `list_snapshots`
- `detect_classloader_leaks`
- `describe_workflow`
- `start_workflow`
- `next_step`
- `get_workflow`
- `close_workflow`
- `create_ai_session`
- `resume_ai_session`
- `get_ai_session`
- `close_ai_session`
- `chat_session`
- `explain_leak`
- `propose_fix`

`parse_heap` and `analyze_heap` both accept an optional `mode: "auto"|"deep"|"overview"` parameter. When the server resolves to overview, the response carries `"mode": "overview"` and returns the streaming partial summary instead of deep-mode object-graph data.

`find_gc_path` gains optional `all_paths: boolean`, `by_class: string`, and `max_paths: number` params (default `20`) for all-paths / by-class enumeration — additive params on the existing tool, not a new tool. `analyze_heap` gains an optional `by_referrer: boolean` param that populates `referrer_report`. `inspect_object` is a new tool taking `heap_path`, `object_id`, and optional `retain_field_data`, returning an `ObjectInspection` with structured `object_id`/`class_name` refs. `diff_heaps` takes `before`, `after`, and an optional `mode: "class"|"object"` param (plus identity-strategy and budget params) for object-level diffing.

`open_snapshot` (params: `key`) loads a cached snapshot by SHA-256 hash or file path and returns its `SnapshotManifest`; it does not run any analysis on its own. `list_snapshots` (no params) returns every cached manifest. `analyze_heap`, `parse_heap`, `find_gc_path`, `inspect_object`, and `query_heap` all gain an additive `snapshot: string` param: when set, the server deserializes the cached object graph instead of re-parsing `heap_path`/`path`, and an invalid, stale, or schema-mismatched key returns a structured `snapshot_not_found`/`snapshot_stale_source`/`snapshot_schema_mismatch`/`snapshot_corrupt` error rather than silently falling back to a fresh parse. `parse_heap`'s `snapshot` response is a distinctly-shaped partial object (not a real `HeapSummary`) carrying a `ProvenanceKind::Partial` marker, since a cached snapshot has no raw HPROF record-tag data to reconstruct the real summary from.

`detect_classloader_leaks` (params: `heap_path`, required) runs `core::analysis::classloader::detect_duplicate_classes()` standalone and returns `Vec<DuplicateClassGroup>` — the cross-loader "Duplicate Classes" signal only, without a full `analyze_heap` call. This is a focused, cheaper single-purpose tool by design, the same rationale as `diff_heaps` existing on its own rather than folding into `analyze_heap`. `analyze_heap`'s existing `enable_classloaders` param needs no new param of its own to get the M13 signals: once set, the returned `classloader_report` automatically includes the new `duplicate_classes`, `unique_class_count` (per loader), and `ancestor_chain` (per loader) fields alongside the pre-existing `loaders` and `potential_leaks`.

### 7.1 MCP workflow suite

The tools above are independent, one-shot primitives. The five workflow tools — `describe_workflow`, `start_workflow`, `next_step`, `get_workflow`, `close_workflow` — add a stateful layer on top: a **workflow** is a small, named, server-persisted state machine that chains several of those same primitives into a fixed, documented sequence, so a client doesn't have to know the right call order itself. No new heap-analysis logic is introduced by this layer — every workflow step wraps an existing, already-tested primitive.

Four workflow kinds ship:

- **`triage_memory_leak`** — `detect` → `investigate_suspect` → `explain` → `propose_fix` → `complete`. End-to-end leak triage: find candidates, drill into the top suspect's GC-root path and referrer profile, get an AI explanation, optionally get a fix suggestion.
- **`tune_gc`** — `root_kind_breakdown` → `thread_local_review` → `top_retainers` → `complete`. A GC-root retention review (which root kinds retain the most, which threads carry the largest thread-local footprint, where the dominator tree's top retainers sit). **Diagnostic data only** — Mnemosyne never touches a live JVM or applies a GC flag; the workflow informs a human's own manual tuning decisions.
- **`traverse_object_graph`** — `inspect` ⇄ `choose_direction` → `complete`. A structured walk starting from one object: inspect it, list refs in/out, pick a direction to step into next, repeat. The one workflow kind with a real branch point — `choose_direction` must name an id the prior `inspect` step actually returned.
- **`compare_snapshots`** — `resolve_snapshots` → `diff` → `complete`. Resolve two heaps (by path, or by an existing M9 snapshot key via `before_snapshot_key`/`after_snapshot_key`), run an M10 object-level diff between them, and surface the ranked suspects.

Typical call shape: `describe_workflow({ kind })` to introspect a kind's step sequence with no side effects, then `start_workflow({ kind, heap_path, ... })` to create an instance and run its first step, then `next_step({ workflow_id, step_input })` repeatedly until `current_step` comes back `"complete"`. `start_workflow`/`next_step` both return `{ workflow_id, current_step, step_result, next_expected_input }`, so a client always knows what to send next without hardcoding the sequence. `get_workflow({ workflow_id })` dumps the full persisted state and step history; `close_workflow({ workflow_id })` deletes it — workflow state is **not** evicted automatically, so a long-lived client should close workflows it no longer needs. Four structured error codes cover the failure modes: `workflow_not_found`, `workflow_corrupt`, `workflow_step_input_mismatch` (the `step_input` doesn't match what the current step expects), and `workflow_already_complete`.

See [docs/mcp-workflows.md](mcp-workflows.md) for one full, real, captured request/response transcript per workflow kind — this guide deliberately doesn't duplicate them here.

## 8. Output Formats

Mnemosyne currently renders five analysis formats through `analyze`, three visualization formats through `flamegraph`, and four policy-gate formats through `ci-check`.

### Text

Default terminal format. Best for direct human use in a shell. In text mode, `analyze` also appends extra investigation tables for histogram, threads, strings, collections, classloaders, and top instances when those modules are enabled.

### Markdown

Useful for tickets, incident notes, or PR artifacts when you want a readable report that still renders cleanly on GitHub or similar tools.

### HTML

Good for sharing with teammates who want a polished static artifact. Current HTML report output escapes user-controlled content to harden the report against XSS.

### JSON

Best for automation, regression tracking, and downstream tooling. Use it for CI or when you want to archive structured artifacts.

### TOON

Compact structured text used by Mnemosyne's AI and integration surfaces. It is human-readable enough to inspect, but primarily useful when you want a concise serialized form that is smaller and more regular than prose.

### Flame graph formats

`flamegraph` supports three formats:

- `svg`: interactive browser-openable flame graph rendered via `inferno`
- `folded-stack`: one folded stack per line, suitable for downstream flamegraph tooling
- `json`: pretty JSON envelope with strategy metadata, total weight, frame counts, and the full folded-stack payload

### CI policy formats

`ci-check` supports four formats:

- `text`: human-readable policy summary with requested/used mode, skipped rules, grouped violations, and a final `RESULT: PASS|FAIL` footer
- `json`: pretty JSON envelope with `{ "tool": "mnemosyne", "subcommand": "ci-check", "version": ..., "result": PolicyResult }`
- `junit`: one testcase per rule so Jenkins and other test reporters can surface heap-policy results alongside tests
- `github-actions`: workflow commands plus a summary line for GitHub Actions annotations; the current runtime emits `file=` but not `line=` because policy source spans are not tracked yet

All four formats include every violation and skipped rule. `--fail-on` changes the process exit code only.

### Provenance markers

Mnemosyne marks uncertain data explicitly instead of letting it blend into normal output.

You may see markers such as:

- `SYNTHETIC`
- `PARTIAL`
- `FALLBACK`
- `PLACEHOLDER`

Interpretation:

- `FALLBACK`: Mnemosyne could not stay on the preferred path and used a secondary one
- `SYNTHETIC`: Mnemosyne generated a stand-in artifact rather than extracting a direct runtime truth
- `PARTIAL`: the command produced a real result, but with incomplete supporting context
- `PLACEHOLDER`: the field exists, but full implementation is not yet there

Overview mode is the main current example of `PARTIAL`: the parser streams real class-resolved aggregates, but it does not build the graph needed for retained sizes, dominators, or leak suspects.

In text-like formats these usually appear as bracketed labels such as `[FALLBACK]`. In JSON they are structured data.

## 9. Configuration Reference

For the full live config surface, use [configuration.md](configuration.md). The essentials are below.

### Config file lookup order

Mnemosyne resolves the config source in this order:

1. `--config /path/to/file.toml`
2. `MNEMOSYNE_CONFIG`
3. `.mnemosyne.toml` in the current working directory
4. `~/.config/mnemosyne/config.toml`
5. `/etc/mnemosyne/config.toml`
6. built-in defaults

### Effective precedence

Think about precedence in two layers:

1. config source selection: the file path is chosen using the lookup order above
2. value overrides after loading: environment overrides apply after the chosen file is loaded, and command-specific CLI flags override config values for that command

Practical examples:

- `leaks --min-severity high` overrides `[analysis].min_severity`
- `analyze --group-by package` overrides the default class histogram grouping for that run
- `MNEMOSYNE_OUTPUT_FORMAT=json` overrides `output = "text"` from a file

### Useful config sections

```toml
[parser]
use_mmap = true
threads = 8
max_objects = 500000

[analysis]
min_severity = "HIGH"
packages = ["com.example", "org.demo"]
leak_types = ["CACHE", "THREAD"]

[general]
output_format = "json"
enable_ai = true
```

Current caveats:

- `[analysis].accumulation_threshold` exists in core defaults, but is not currently loaded from TOML or environment overrides
- `parser.max_objects` is live
- `parser.use_mmap` and `parser.threads` are loaded, but are not currently documented as user-visible execution toggles in the CLI

### Useful environment overrides

```bash
export MNEMOSYNE_OUTPUT_FORMAT=json
export MNEMOSYNE_MAX_OBJECTS=500000
export MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD=4294967296
export MNEMOSYNE_MIN_SEVERITY=HIGH
export MNEMOSYNE_PACKAGES="com.example,org.demo"
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD"
```

`MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` is env-only, measured in bytes, and defaults to 4 GiB. It affects `mode=auto` for CLI and MCP mode resolution, but it is not a TOML key and will not appear in `mnemosyne-cli config` output.

Use `mnemosyne-cli config` any time you want to confirm the final merged config and the source it came from.
