# Configuration Guide

This document describes the configuration surface that is actually loaded by `cli/src/config_loader.rs` and consumed by the current CLI and MCP server.

## Lookup Order

Mnemosyne resolves config in this order:

1. `--config /path/to/file.toml`
2. `$MNEMOSYNE_CONFIG`
3. `.mnemosyne.toml` in the current directory
4. `~/.config/mnemosyne/config.toml`
5. `/etc/mnemosyne/config.toml`
6. built-in defaults

## Effective Config Shape

The live root config is:

```toml
[parser]
use_mmap = true
threads = 8
max_objects = 500000

[ai]
enabled = true
provider = "openai"
model = "gpt-4.1-mini"
temperature = 0.2
mode = "rules"
endpoint = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
max_tokens = 2000
timeout_secs = 30

[[ai.tasks]]
kind = "top-leak"
enabled = true

[[ai.tasks]]
kind = "healthy-heap"
enabled = true

[[ai.tasks]]
kind = "remediation-checklist"
enabled = true

[ai.privacy]
redact_heap_path = true
redact_patterns = ["secret-token-[0-9]+", "customer-[0-9]+"]
audit_log = true

[ai.prompts]
template_dir = "/absolute/path/to/prompts"

[analysis]
min_severity = "HIGH"
packages = ["com.example", "org.demo"]
leak_types = ["CACHE", "THREAD"]

[general]
output_format = "json"
enable_ai = true

# alternative top-level output shortcut
output = "json"
```

Notes:

- both `[ai]` and `[llm]` are accepted; `[ai]` wins if both are present
- `[general].output_format` and top-level `output` both feed `AppConfig.output`
- `[general].enable_ai` feeds `ai.enabled`
- `[analysis].accumulation_threshold` exists in core defaults and runtime analysis, but the current config loader does not load it from TOML or environment yet
- `[code_mapping]`, `[report]`, `[mcp]`, and `[performance]` tables are not part of the active config loader in this branch
- `verbose = ...` in TOML is not a live setting

## Current Defaults

Running `mnemosyne-cli config` with no file currently shows:

```json
{
  "parser": {
    "use_mmap": true,
    "threads": null,
    "max_objects": null
  },
  "ai": {
    "enabled": false,
    "provider": "openai",
    "model": "gpt-4.1-mini",
    "temperature": 0.2,
    "mode": "rules",
    "tasks": [
      { "kind": "top-leak", "enabled": true },
      { "kind": "healthy-heap", "enabled": true },
      { "kind": "remediation-checklist", "enabled": true }
    ],
    "privacy": {
      "redact_heap_path": false,
      "redact_patterns": [],
      "audit_log": false
    },
    "prompts": { "template_dir": null },
    "endpoint": null,
    "api_key_env": null,
    "max_tokens": null,
    "timeout_secs": 30
  },
  "analysis": {
    "min_severity": "HIGH",
    "packages": [],
    "leak_types": [],
    "accumulation_threshold": 10.0
  },
  "output": "text"
}
```

## File Sections

### `[general]`

Supported keys:

- `output_format = "text|toon|markdown|html|json"`
- `enable_ai = true|false`

This section is a convenience layer. It maps to the same underlying `output` and `ai.enabled` fields.

### `[parser]`

Supported keys:

- `use_mmap = true|false`
- `threads = <integer>`
- `max_objects = <integer>`

Current runtime truth:

- `max_objects` is live and is passed into `parse`, `analyze`, and MCP `parse_heap`
- `use_mmap` is loaded into config, but this branch does not currently document an active CLI path that changes behavior from it
- `threads` is loaded into config, but this branch does not currently document an active CLI path that changes behavior from it

### `[analysis]`

Supported keys loaded today:

- `min_severity = "LOW|MEDIUM|HIGH|CRITICAL"`
- `packages = ["com.example", "org.demo"]`
- `leak_types = ["CACHE", "THREAD", ...]`

Behavior:

- `min_severity` feeds `leaks`, `analyze`, `explain`, and MCP analysis surfaces unless a command-level override is passed
- `min_severity` also feeds `mnemosyne-cli chat` startup; chat analyzes once with those same filters and either shows the top-3 shortlist or an explicit healthy-heap context when nothing survives filtering
- `packages` acts as an allow-list when real class names are available and still shapes fallback leak synthesis when the graph-backed path cannot provide enough detail
- `leak_types` restricts both the real leak list and the fallback synthetic list

Supported leak kinds:

- `UNKNOWN`
- `CACHE`
- `COROUTINE`
- `THREAD`
- `HTTP_RESPONSE`
- `CLASS_LOADER`
- `COLLECTION`
- `LISTENER`

Important caveat:

- `accumulation_threshold` is part of `AnalysisConfig` and is used by graph-backed suspect ranking, but the current loader does not yet read it from TOML or `MNEMOSYNE_*` environment variables

### `[ai]` and `[llm]`

Supported keys:

- `enabled = true|false`
- `provider = "openai|anthropic|local"`
- `model = "..."`
- `temperature = 0.2`
- `mode = "rules|stub|provider"`
- `endpoint = "https://..."`
- `api_key_env = "OPENAI_API_KEY"`
- `max_tokens = 2000`
- `timeout_secs = 30`

Current provider budget behavior:

- `max_tokens` still flows to the provider transport as the response/token cap supported by that provider API
- in provider mode, very small `max_tokens` values also trigger a conservative prompt-context reduction before the external call
- Mnemosyne trims sampled leak context first, preserves `section instructions`, and marks the prompt with `context_truncated=true` when that reduction happens

Task runner support:

```toml
[[ai.tasks]]
kind = "top-leak"
enabled = true

[[ai.tasks]]
kind = "healthy-heap"
enabled = true

[[ai.tasks]]
kind = "remediation-checklist"
enabled = false
```

Supported task kinds:

- `top-leak`
- `healthy-heap`
- `remediation-checklist`

Prompt template support:

```toml
[ai.prompts]
template_dir = "/absolute/path/to/prompts"
```

If `template_dir` is set, Mnemosyne expects `provider-insights.yaml` in that directory. If it is unset, Mnemosyne uses the embedded default at `core/src/prompts/defaults/provider-insights.yaml`.

Provider-mode privacy support:

```toml
[ai.privacy]
redact_heap_path = true
redact_patterns = ["secret-token-[0-9]+", "customer-[0-9]+"]
audit_log = true
```

Behavior:

- these controls apply when `mode = "provider"`
- `redact_heap_path = true` replaces the outbound TOON `heap_path=...` value with `<REDACTED>`
- `redact_patterns` applies regex replacement across the fully rendered outbound provider prompt, including YAML-rendered instruction text
- `audit_log = true` emits a hashed audit record for the redacted outbound provider prompt through the existing tracing pipeline
- invalid regex patterns fail explicitly with `CoreError::InvalidInput`
- `AiWireExchange.prompt` captures the redacted prompt that was actually sent to the provider
- the audit log intentionally records metadata only: provider, model, SHA-256 of the redacted prompt, prompt byte length, and redaction settings counts
- the audit log does not emit raw prompt text, raw response text, regex values, or the original heap path
- when `max_tokens` is configured very low, Mnemosyne trims leak-context detail before the provider call and marks the prompt with `context_truncated=true`; the instruction section is preserved so the TOON contract remains intact
- `mnemosyne-cli chat` uses the same `rules` / `stub` / `provider` mode selection for follow-up turns and keeps only the running process' recent 3-turn history in memory
- there is no separate `[chat]` config block; the chat command inherits `[analysis]` for startup filtering and `[ai]` for turn generation

## Environment Variables

### Config Selection

```bash
export MNEMOSYNE_CONFIG=/path/to/config.toml
```

### Output

```bash
export MNEMOSYNE_OUTPUT_FORMAT=json
```

### Parser

```bash
export MNEMOSYNE_USE_MMAP=false
export MNEMOSYNE_THREADS=8
export MNEMOSYNE_MAX_OBJECTS=500000
```

### Analysis

```bash
export MNEMOSYNE_MIN_SEVERITY=HIGH
export MNEMOSYNE_PACKAGES="com.example,org.demo"
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD,HTTP_RESPONSE"
```

### AI

```bash
export MNEMOSYNE_AI_ENABLED=true
export MNEMOSYNE_AI_PROVIDER=openai
export MNEMOSYNE_AI_MODE=provider
export MNEMOSYNE_AI_MODEL=gpt-4.1-mini
export MNEMOSYNE_AI_ENDPOINT=https://api.openai.com/v1
export MNEMOSYNE_AI_API_KEY_ENV=OPENAI_API_KEY
export MNEMOSYNE_AI_TEMPERATURE=0.2
export MNEMOSYNE_AI_PROMPT_TEMPLATE_DIR=/path/to/prompts
export MNEMOSYNE_AI_REDACT_HEAP_PATH=true
export MNEMOSYNE_AI_REDACT_PATTERNS="secret-token-[0-9]+,customer-[0-9]+"
export MNEMOSYNE_AI_AUDIT_LOG=true
export MNEMOSYNE_AI_MAX_TOKENS=2000
export MNEMOSYNE_AI_TIMEOUT_SECS=30
```

Provider credentials are then read from the environment variable named by `api_key_env`, or from the provider default when `api_key_env` is omitted:

- OpenAI default: `OPENAI_API_KEY`
- Anthropic default: `ANTHROPIC_API_KEY`
- Local provider: no API key by default

Notable non-features:

- `MNEMOSYNE_VERBOSE` is not supported
- there is no environment override for `analysis.accumulation_threshold` yet

## CLI Overrides

Global CLI options:

```bash
mnemosyne-cli -v analyze heap.hprof
mnemosyne-cli -c custom.toml analyze heap.hprof
```

There is currently:

- no global `--format`
- no `--quiet`
- no `--no-ai`

Live subcommand surfaces:

### `parse`

```bash
mnemosyne-cli parse <HEAP>
```

The command currently has no parse-specific flags beyond the global `-v` and `-c` options.

### `leaks`

```bash
mnemosyne-cli leaks <HEAP> --min-severity high --package com.example --leak-kind cache
```

### `analyze`

```bash
mnemosyne-cli analyze <HEAP> \
  --format json \
  --profile incident-response \
  --group-by package \
  --ai \
  --threads \
  --strings \
  --collections \
  --classloaders \
  --top-instances \
  --top-n 15 \
  --min-capacity 32 \
  --package com.example \
  --leak-kind cache
```

Important truth:

- `analyze` does not currently expose `--min-severity`

### `diff`

```bash
mnemosyne-cli diff before.hprof after.hprof
```

No diff-specific flags are currently exposed.

### `map`

```bash
mnemosyne-cli map leak-id --project-root ./service --class com.example.Cache --no-git
```

### `gc-path`

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x1000 --max-depth 8
```

### `query`

```bash
mnemosyne-cli query heap.hprof "SELECT @objectId, @className FROM \"com.example.BigCache\" LIMIT 25"
```

### `explain`

```bash
mnemosyne-cli explain heap.hprof --leak-id leak-usersession-1 --min-severity high
```

### `fix`

```bash
mnemosyne-cli fix heap.hprof --leak-id leak-usersession-1 --style defensive --project-root ./service
```

### `serve`

```bash
mnemosyne-cli serve --host 127.0.0.1 --port 0
```

The current implementation still serves over stdio. `host` and `port` are accepted and logged, but `core/src/mcp/server.rs` labels them as currently informational.

### `config`

```bash
mnemosyne-cli config
```

The command prints pretty JSON plus the config origin note. There are no nested `config` subcommands.

## AI Mode Details

### `rules`

Default mode. Runs the built-in rule engine locally and preserves the stable `AiInsights` wire contract.

### `stub`

Deterministic compatibility mode. Useful when you want predictable output without provider calls.

### `provider`

Uses the configured provider transport and requires strict TOON responses.

Current provider status:

- `openai`: live
- `local`: live via OpenAI-compatible endpoint
- `anthropic`: live, with targeted core and CLI verification coverage in this branch

Failure behavior is explicit. Mnemosyne does not silently fake provider success when:

- the configured API key env var is missing
- `provider = "local"` is selected without `endpoint`
- the provider returns no usable completion text
- the returned text is not valid TOON for the parser

## MCP Relationship

The MCP server uses the same loaded `AppConfig` as the CLI.

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

There is no `apply_fix` method.

See `docs/api.md` for the actual wire contract.
