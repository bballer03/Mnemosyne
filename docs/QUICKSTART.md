# Quick Start Guide

Get from heap dump to actionable output with the current `mnemosyne-cli` surface.

## Prerequisites

- Rust installed if you are building from source
- a JVM heap dump (`.hprof`)
- optional AI provider credentials if you want provider-backed AI instead of the default offline `rules` mode

## Install

Build from source:

```bash
git clone https://github.com/bballer03/mnemosyne
cd mnemosyne
cargo build --release
```

Or install the published CLI crate:

```bash
cargo install mnemosyne-cli
```

The packaged binary name is `mnemosyne-cli`.

## Step 1: Capture a Heap Dump

```bash
jps
jmap -dump:format=b,file=heap.hprof <PID>
```

## Step 2: Parse the Dump

```bash
mnemosyne-cli parse heap.hprof
```

`parse` is the lightweight path. It reports header metadata, total records, and record-category byte totals without building the full object graph.

## Step 3: Detect Leaks

```bash
mnemosyne-cli leaks heap.hprof
```

Useful live filters:

```bash
mnemosyne-cli leaks heap.hprof --min-severity high
mnemosyne-cli leaks heap.hprof --package com.example --package org.demo
mnemosyne-cli leaks heap.hprof --leak-kind cache --leak-kind thread
```

Notes:

- `leaks` attempts the graph-backed path first
- if that path is unavailable, Mnemosyne falls back to heuristic output with provenance markers
- zero-result runs print `No leak suspects detected.` explicitly
- there is no `--exclude-package` flag

## Step 4: Run Full Analysis

```bash
mnemosyne-cli analyze heap.hprof
```

Useful live options:

```bash
mnemosyne-cli analyze heap.hprof --format json
mnemosyne-cli analyze heap.hprof --group-by package
mnemosyne-cli analyze heap.hprof --threads --strings --collections --classloaders --top-instances
mnemosyne-cli analyze heap.hprof --profile incident-response
mnemosyne-cli analyze heap.hprof --output-file report.html --format html
```

`analyze` can attach optional investigation reports to the same run:

- `--threads`
- `--strings`
- `--collections`
- `--classloaders`
- `--top-instances`
- `--top-n <N>`
- `--min-capacity <N>`

Profile presets currently mean:

- `overview`: disables the optional investigation reports
- `incident-response`: enables all optional investigation reports and raises the default depth
- `ci-regression`: enables `top-instances` with tighter defaults

## Step 5: AI Insights

The CLI flag is still `--ai`:

```bash
mnemosyne-cli analyze heap.hprof --ai
mnemosyne-cli explain heap.hprof --leak-id leak-usersession-1
```

Current AI modes:

- `rules`: default, offline-safe, built into the repo
- `stub`: deterministic compatibility mode
- `provider`: calls a configured provider and parses strict TOON back into the stable `AiInsights` contract

Provider-backed AI is configured through `[ai]` or environment variables. OpenAI-compatible, local, and Anthropic provider paths all have targeted verification coverage in this branch.

Start a bounded leak-focused chat session with:

```bash
mnemosyne-cli chat heap.hprof
```

`chat` analyzes the heap once, prints the top 3 leak candidates, and supports `/focus <leak-id>`, `/list`, `/help`, and `/exit`. It keeps only the running process' recent history in memory and reuses the same `rules` / `stub` / `provider` AI mode plus provider privacy controls as `explain`. The startup shortlist still respects `[analysis]` filters, so chat can also begin in an explicit healthy-heap context when no leaks survive filtering.

## Step 6: Save Reports

```bash
mnemosyne-cli analyze heap.hprof --format html --output-file report.html
mnemosyne-cli analyze heap.hprof --format json --output-file report.json
mnemosyne-cli analyze heap.hprof --format toon --output-file report.toon
```

Supported output formats:

- `text`
- `toon`
- `markdown`
- `html`
- `json`

## Step 7: Inspect the Effective Config

```bash
mnemosyne-cli config
mnemosyne-cli config --config .mnemosyne.toml
```

The current `config` command prints:

- the merged configuration as pretty JSON
- then a one-line origin message such as `Using built-in defaults...` or `Loaded configuration from ...`

There are currently no `config show` or `config validate` subcommands.

## Minimal Config File

```toml
[general]
output_format = "toon"
enable_ai = true

[ai]
mode = "rules"
model = "gpt-4.1-mini"
temperature = 0.2

[[ai.tasks]]
kind = "top-leak"
enabled = true

[[ai.tasks]]
kind = "healthy-heap"
enabled = true

[[ai.tasks]]
kind = "remediation-checklist"
enabled = true

[analysis]
min_severity = "HIGH"
packages = ["com.example"]
leak_types = ["CACHE", "THREAD"]

[parser]
max_objects = 500000
```

Config lookup order:

1. `--config /path/to/file.toml`
2. `$MNEMOSYNE_CONFIG`
3. `.mnemosyne.toml` in the current directory
4. `~/.config/mnemosyne/config.toml`
5. `/etc/mnemosyne/config.toml`
6. built-in defaults

## Useful Environment Variables

```bash
export MNEMOSYNE_OUTPUT_FORMAT=json
export MNEMOSYNE_MAX_OBJECTS=500000
export MNEMOSYNE_MIN_SEVERITY=HIGH
export MNEMOSYNE_PACKAGES="com.example,org.demo"
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD"

export MNEMOSYNE_AI_ENABLED=true
export MNEMOSYNE_AI_MODE=provider
export MNEMOSYNE_AI_PROVIDER=openai
export MNEMOSYNE_AI_MODEL=gpt-4.1-mini
export MNEMOSYNE_AI_API_KEY_ENV=OPENAI_API_KEY
export OPENAI_API_KEY="sk-..."
```

Notes:

- `MNEMOSYNE_VERBOSE` is not a real config/env knob
- `MNEMOSYNE_USE_MMAP` and `MNEMOSYNE_THREADS` are loaded, but they are not currently documented as changing the active CLI execution path in this branch

## MCP Setup

Start the stdio server:

```bash
mnemosyne-cli serve
```

Current MCP methods:

- `list_tools`
- `parse_heap`
- `detect_leaks`
- `analyze_heap`
- `query_heap`
- `map_to_code`
- `find_gc_path`
- `explain_leak`
- `propose_fix`

There is currently no `apply_fix` MCP method.
There is currently no MCP chat/session method.

## Memory Scaling Status

Step 11 is complete.

The current published memory-scaling story is:

- default graph-backed `analyze` / `leaks`: about `2.87x-2.90x` RSS:dump on dense synthetic ~500 MB / ~1 GB / ~2 GB tiers
- investigation-heavy path: about `3.89x-3.92x` on the same tiers
- `parse`: remains near-constant and very small in RSS because it stays on the streaming summary path

See `docs/performance/memory-scaling.md` for the measured tables.

## Next Docs

- `docs/api.md` for the live MCP wire format
- `docs/configuration.md` for the full config surface
- `README.md` for the project overview
