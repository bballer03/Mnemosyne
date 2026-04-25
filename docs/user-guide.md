# Mnemosyne User Guide

This guide is the practical, end-to-end companion to Mnemosyne's CLI and MCP surfaces. It focuses on how to use the current v0.2.0 runtime effectively without repeating material that already lives in the quickstart, configuration reference, or MCP API reference.

For a fast first run, start with [QUICKSTART.md](QUICKSTART.md). For the full config surface, see [configuration.md](configuration.md). For the stdio MCP wire contract, see [api.md](api.md). For installation details and release packaging, see [../README.md](../README.md).

## 1. Introduction

Mnemosyne is a JVM heap-analysis tool for engineers who need answers from `.hprof` dumps without waiting on a slow toolchain or manually stitching together multiple utilities.

Today it combines:

- Rust-based parsing for fast summary and graph-backed analysis paths.
- Real heap investigation features such as retained sizes, dominators, GC-root tracing, string analysis, collection inspection, thread inspection, classloader reporting, and top-instance ranking.
- Explicit provenance markers so fallback, synthetic, partial, and placeholder output is labeled instead of being presented as authoritative fact.
- MCP-native integration so the same core analysis surface can be used from editors and automation.
- AI-assisted explanation and fix-generation paths that can run in offline `rules` mode or provider-backed `provider` mode.

Mnemosyne is a good fit for:

- JVM engineers investigating memory growth, retained-size hotspots, or leak candidates.
- CI and release pipelines that need machine-readable heap summaries or regression artifacts.
- Editor-based workflows where heap analysis should be callable through MCP instead of a one-off shell session.

What makes it different from a basic heap-summary tool is that the fast path and the deep path live in one CLI. You can start with a lightweight parse, move into graph-backed investigation, then keep going into AI explanation, source mapping, or MCP automation without changing tools.

## 2. Installation

Mnemosyne currently ships through five distribution channels. Use the one that matches your environment, then refer to [../README.md](../README.md) for the full install steps and release-specific details.

### Cargo install

```bash
cargo install mnemosyne-cli
```

### GitHub Releases

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

### `parse`

Use `parse` when you want the fastest possible look at a heap dump before committing to a graph-backed analysis pass.

Usage:

```bash
mnemosyne-cli parse heap.hprof
```

Flags:

- no parse-specific CLI flags beyond the global `-c` and `-v` options
- `parser.max_objects` from config or `MNEMOSYNE_MAX_OBJECTS` still affects the underlying parse job

What it does:

- validates the input path
- reads the HPROF header
- prints summary metadata, record counts, aggregate record-category sizes, and top record tags
- stays on the lightweight summary path instead of building the full object graph

Example:

```bash
mnemosyne-cli parse heap.hprof
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

### `analyze`

`analyze` is the main report-generation command. It runs the graph-backed analysis pipeline when possible, can attach optional investigation reports, and is the only CLI surface that currently owns `--format` and `--output-file`.

Usage:

```bash
mnemosyne-cli analyze <HEAP> [OPTIONS]
```

Flags:

- `--format text|markdown|html|json|toon`
- `--profile overview|incident-response|ci-regression`
- `--group-by class|package|classloader`
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

What it does:

- validates the heap file
- builds a full analysis response
- uses the configured analysis filters plus any command-line overrides
- attempts graph-backed retained-size analysis first and falls back honestly when needed
- renders the result in text, Markdown, HTML, JSON, or TOON

Profile behavior:

- `overview`: disables optional investigation reports and keeps defaults conservative
- `incident-response`: enables threads, strings, collections, classloaders, and top instances; ensures at least `--top-n 15` and `--min-capacity 32`
- `ci-regression`: enables top instances with tighter defaults and uses `--min-capacity 64`

Examples:

```bash
mnemosyne-cli analyze heap.hprof
mnemosyne-cli analyze heap.hprof --group-by package --top-instances
mnemosyne-cli analyze heap.hprof --profile incident-response --threads --strings --collections
mnemosyne-cli analyze heap.hprof --format html --output-file heap-report.html
mnemosyne-cli analyze heap.hprof --format json --profile ci-regression
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

When you write to a file, the CLI prints a confirmation instead of dumping the report to stdout:

```text
Report (text/plain) written to heap-report.txt
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

What it does:

- loads the configured analysis filters
- applies command-line overrides for severity, package allow-listing, and leak kinds
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

### `gc-path`

Use `gc-path` when you already know a target object ID and want to see how it stays reachable from a GC root.

Usage:

```bash
mnemosyne-cli gc-path <HEAP> --object-id <ID> [--max-depth <N>]
```

Flags:

- `--object-id <ID>`
- `--max-depth <N>`

What it does:

- traces a path from the requested object toward a root
- prefers a full `ObjectGraph` BFS path
- falls back to a budget-limited graph and then synthetic output when needed
- labels fallback output through provenance markers

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

### `diff`

Use `diff` to compare two snapshots and highlight aggregate change between them.

Usage:

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Flags:

- no diff-specific CLI flags in the current runtime

What it does:

- validates both dumps
- prints total delta size and object-count delta
- prints top changed classes or record categories
- prints class-level retained deltas when both heaps build graph-backed diff context successfully

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

Current limitation: `diff` is still record-level plus class-level retained deltas. It does not yet provide object-identity or reference-chain diffing.

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

- no query-specific flags in the current runtime

What it does:

- builds the graph-backed query context
- parses the query string
- prints matched column names, match count, rows, and a truncation note when `LIMIT` cuts off the result set

Examples:

```bash
mnemosyne-cli query heap.hprof "SELECT @objectId, @className FROM \"com.example.*\" LIMIT 25"
mnemosyne-cli query heap.hprof "SELECT @objectId, entries FROM \"com.example.BigCache\" LIMIT 10"
```

Expected output pattern:

```text
Columns: @objectId, entries
Matched: 1
0x00001000 | 42
```

Current limitation: the query surface is real, but still smaller than a full MAT-style OQL environment. Built-in fields, retained instance-field projection/filtering, and `INSTANCEOF` support are in place; richer predicates and broader explorer semantics are still future work.

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

This is a good interactive workflow when you want to correlate retained-size hotspots with thread-local retention, duplicate string waste, oversized collections, and the largest individual objects.

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

For CI or artifact comparison, keep the output machine-readable.

```bash
mnemosyne-cli analyze heap.hprof --profile ci-regression --format json --output-file analysis.json
```

This keeps the output structured, reproducible, and easy to archive. The `ci-regression` profile enables top-instance reporting with tighter defaults without turning on every investigation module.

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
- `query_heap`
- `map_to_code`
- `find_gc_path`
- `create_ai_session`
- `resume_ai_session`
- `get_ai_session`
- `close_ai_session`
- `chat_session`
- `explain_leak`
- `propose_fix`

## 8. Output Formats

Mnemosyne currently renders five output formats through `analyze`.

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
export MNEMOSYNE_MIN_SEVERITY=HIGH
export MNEMOSYNE_PACKAGES="com.example,org.demo"
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD"
```

Use `mnemosyne-cli config` any time you want to confirm the final merged config and the source it came from.
