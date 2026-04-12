# Examples

This directory is currently a lightweight landing page for real command examples.

## Current State

At the moment this folder does not ship the older example markdown files or sample `.hprof` fixtures that earlier docs referenced.

The source-of-truth examples live in:

- `README.md`
- `docs/QUICKSTART.md`
- `docs/api.md`
- `docs/configuration.md`

## Common CLI Examples

Parse a heap dump:

```bash
mnemosyne-cli parse heap.hprof
```

Detect leaks with filters:

```bash
mnemosyne-cli leaks heap.hprof --min-severity high --package com.example --leak-kind cache
```

Run full analysis with optional investigation reports:

```bash
mnemosyne-cli analyze heap.hprof \
  --group-by package \
  --threads \
  --strings \
  --collections \
  --classloaders \
  --top-instances \
  --top-n 10 \
  --min-capacity 32 \
  --ai
```

Persist a report artifact:

```bash
mnemosyne-cli analyze heap.hprof --format json --output-file report.json
```

Explain or fix a specific leak:

```bash
mnemosyne-cli explain heap.hprof --leak-id leak-usersession-1
mnemosyne-cli fix heap.hprof --leak-id leak-usersession-1 --style defensive --project-root ./service
```

Map a leak back to code:

```bash
mnemosyne-cli map com.example.Cache::deadbeef --project-root ./service --class com.example.Cache
```

Trace a GC root path:

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x1000 --max-depth 8
```

Run a heap query:

```bash
mnemosyne-cli query heap.hprof "SELECT @objectId, @className FROM \"com.example.BigCache\" LIMIT 25"
```

Inspect the merged config:

```bash
mnemosyne-cli config
```

## Minimal Config Examples

### Production-Oriented

```toml
[general]
output_format = "json"
enable_ai = true

[analysis]
min_severity = "HIGH"
packages = ["com.example"]
leak_types = ["CACHE", "THREAD"]

[ai]
mode = "provider"
provider = "openai"
model = "gpt-4.1-mini"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 30
```

### CI-Oriented

```toml
[general]
output_format = "json"
enable_ai = false

[analysis]
min_severity = "CRITICAL"

[parser]
max_objects = 500000
```

### Local Provider

```toml
[general]
enable_ai = true

[ai]
mode = "provider"
provider = "local"
endpoint = "http://localhost:11434/v1"
model = "llama3"

[[ai.tasks]]
kind = "top-leak"
enabled = true
```

## MCP Example

Start the server:

```bash
mnemosyne-cli serve
```

Send one request line:

```json
{"id":1,"method":"parse_heap","params":{"path":"heap.hprof"}}
```

See `docs/api.md` for the full wire format, the `list_tools` discovery method, and all fourteen live methods.

## Related Docs

- [Quick Start Guide](../QUICKSTART.md)
- [Configuration Guide](../configuration.md)
- [API Reference](../api.md)
