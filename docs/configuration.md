# Configuration Guide

This document describes all configuration options for Mnemosyne.

## Table of Contents

- [Configuration Files](#configuration-files)
- [Environment Variables](#environment-variables)
- [CLI Options](#cli-options)
- [MCP Server Configuration](#mcp-server-configuration)
- [AI/LLM Configuration](#aillm-configuration)
- [Performance Tuning](#performance-tuning)

---

## Configuration Files

Mnemosyne looks for configuration in the following locations (in order of precedence):

1. `--config /path/to/file.toml` (explicit CLI flag)
2. `$MNEMOSYNE_CONFIG` environment variable
3. `.mnemosyne.toml` in current directory
4. `~/.config/mnemosyne/config.toml`
5. `/etc/mnemosyne/config.toml` (Linux)
6. Built-in defaults

### Example Configuration

```toml
# .mnemosyne.toml

[general]
# Enable verbose logging
verbose = false

# Output format: "text", "toon", "markdown", "html", "json"
output_format = "text"

# Toggle AI-powered helpers globally
enable_ai = true

[parser]
# Maximum heap dump size to process (in GB, 0 = unlimited)
max_size_gb = 0

# Use memory-mapped I/O
use_mmap = true

# Number of threads for parsing
threads = 0  # 0 = auto-detect

[analysis]
# Minimum severity for leak reporting
min_severity = "MEDIUM"

# Package filters (empty = all packages)
packages = ["com.example", "org.myapp"]

# Leak types to detect
leak_types = ["COROUTINE", "THREAD", "CACHE", "HTTP_RESPONSE"]

# Retained/shallow ratio threshold for accumulation-point suspects
accumulation_threshold = 10.0

[llm]
# LLM provider: "openai" (default), "anthropic", "local"
provider = "openai"

# Model to use
model = "gpt-4"

# API endpoint (for custom/local models)
# endpoint = "http://localhost:8080/v1"

# Maximum tokens per request
max_tokens = 2000

# Temperature for AI responses (0.0 - 1.0)
temperature = 0.3

[code_mapping]
# Enable git integration
enable_git = true

# Only map files modified in last N days (0 = all)
recent_changes_days = 0

[report]
# Include full object graphs in reports
include_graphs = false

# Maximum items in top lists
max_top_items = 20

# Generate visualizations
generate_charts = false

[mcp]
# MCP server host
host = "127.0.0.1"

# MCP server port
port = 0  # 0 = auto-assign

# Request timeout (seconds)
timeout = 300

[performance]
# Cache parsed heap dumps
enable_cache = true

# Cache directory
cache_dir = "~/.cache/mnemosyne"

# Maximum cache size (in GB)
max_cache_gb = 10
```

> **Note:** The current alpha build consumes the `general`, `parser`, `analysis`, and `ai`/`llm` sections. The remaining tables are reserved for upcoming features and are ignored for now.

Valid `output_format` values today are `text`, `markdown`, `html`, `toon`, and `json` (case-insensitive). JSON pairs nicely with the new `--output-file` CLI flag to generate machine-readable artifacts without relying on shell redirection.

### Analysis Defaults

The `[analysis]` table feeds every CLI and MCP command that needs leak heuristics.

- `min_severity` acts as a hard cutoff for `mnemosyne leaks`, `analyze`, and `explain`. Candidates below the threshold are dropped entirely. CLI flags such as `--min-severity` override it case-by-case.
- `packages` accepts a list of package prefixes. Mnemosyne now treats them as an allow-list when real class histograms are available (only matching classes become leak candidates) and still rotates through the list when synthesizing fallback identifiers so each namespace shows up in the output. MCP endpoints receive the filtered list for richer prompting as well.
- `leak_types` constrains both the real leak list (parsed from class stats/dominator context) and the synthetic fallback. If at least one matching class exists, only those kinds survive; otherwise Mnemosyne emits one deterministic entry per requested kind. Supported values mirror the `LeakKind` enum (`CACHE`, `THREAD`, `HTTP_RESPONSE`, `CLASS_LOADER`, `COLLECTION`, `LISTENER`, `COROUTINE`, `UNKNOWN`).
- `accumulation_threshold` controls when graph-backed suspect ranking marks an object as an accumulation point. The default `10.0` is intentionally conservative: an object must retain at least 10x its own shallow size before it is flagged this way.

Set these once in `.mnemosyne.toml` and your CI, MCP sessions, and CLI invocations stay aligned.

---

## Environment Variables

Environment variables override configuration file settings.

### General

```bash
# Enable verbose logging
export MNEMOSYNE_VERBOSE=true

# Set output format
export MNEMOSYNE_OUTPUT_FORMAT=json

# Config file path
export MNEMOSYNE_CONFIG=/path/to/config.toml
```

### LLM/AI

```bash
# OpenAI API key (required for AI features)
export OPENAI_API_KEY="sk-..."

# Alternative: Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Enable/disable AI helpers
export MNEMOSYNE_AI_ENABLED=true

# Provider + model selection
export MNEMOSYNE_AI_PROVIDER=openai
export MNEMOSYNE_AI_MODEL="gpt-4.1-mini"

# Temperature for AI responses
export MNEMOSYNE_AI_TEMPERATURE=0.3
```

### Performance

```bash
# Number of parser threads
export MNEMOSYNE_THREADS=8

# Disable memory-mapped I/O
export MNEMOSYNE_USE_MMAP=false

# Limit parsed objects (0 = unlimited)
export MNEMOSYNE_MAX_OBJECTS=500000
```

### Analysis

```bash
# Minimum severity for leak reporting (LOW|MEDIUM|HIGH|CRITICAL)
export MNEMOSYNE_MIN_SEVERITY=HIGH

# Comma-separated package prefixes
export MNEMOSYNE_PACKAGES="com.example, org.demo"

# Comma-separated leak kinds (CACHE|THREAD|CLASS_LOADER|...)
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD,HTTP_RESPONSE"
```

Setting either variable applies the same logic as the config file: packages filter real classes first, and leak kinds gate both the parsed candidates and any synthetic fallback entries.

### Logging

```bash
# Rust log levels: error, warn, info, debug, trace
export RUST_LOG=mnemosyne=debug

# Log to file
export RUST_LOG_FILE=/var/log/mnemosyne.log
```

---

## CLI Options

Command-line options have the highest precedence.

### Global Options

```bash
# Verbose output
mnemosyne -v analyze heap.hprof
mnemosyne --verbose analyze heap.hprof

# Quiet mode (errors only)
mnemosyne -q analyze heap.hprof

# Custom config file
mnemosyne -c custom.toml analyze heap.hprof
mnemosyne --config custom.toml analyze heap.hprof

# Output format
mnemosyne -f toon analyze heap.hprof
mnemosyne --format toon analyze heap.hprof
```

To inspect the merged configuration and its origin, run:

```bash
mnemosyne config
```

### Parse Command

```bash
mnemosyne parse [OPTIONS] <HEAP_FILE>

Options:
  --include-strings       Include string table
  --max-objects <N>       Limit objects parsed
  --threads <N>           Number of threads
  --no-mmap              Disable memory-mapped I/O
```

### Analyze Command

```bash
mnemosyne analyze [OPTIONS] <HEAP_FILE>

Options:
  --ai                   Force-enable AI analysis (otherwise driven by config)
  --format <FMT>         Override output format (text|toon|markdown|html)
  --package <PKG>...     Restrict to specific packages (repeat flag or comma-separated list)
  --leak-kind <KIND>...  Restrict leak kinds (repeat flag or comma list)
```

`[analysis]` settings (e.g., `min_severity`, `packages`, `leak_types`) are picked up automatically.

### Leaks Command

```bash
mnemosyne leaks [OPTIONS] <HEAP_FILE>

Options:
  --package <PKG>...     Filter by one or more packages (repeat flag or use commas)
  --min-severity <LVL>   Minimum severity (LOW|MEDIUM|HIGH|CRITICAL)
  --leak-kind <KIND>...  Restrict leak kinds
```

If you omit these flags, the defaults come from `[analysis]`.

### Diff Command

```bash
mnemosyne diff [OPTIONS] <BEFORE> <AFTER>

Options:
  --min-growth <MB>      Minimum growth to report
  --by-class            Group by class
```
---

## MCP Server Configuration

### Starting the Server

```bash
# Default (stdio)
mnemosyne serve

# With specific config
mnemosyne serve --config mcp-config.toml

# With environment
OPENAI_API_KEY=sk-... mnemosyne serve
```

The MCP server reuses the exact same config loader as the CLI. That means `.mnemosyne.toml`, `$MNEMOSYNE_CONFIG`, `[analysis]` defaults, and all `MNEMOSYNE_*` environment overrides automatically shape every MCP request (leak severity, packages, leak kinds, AI provider, etc.).

### IDE-Specific Configuration

#### VS Code / Cursor

Create `.vscode/mcp-config.json`:

```json
{
  "mcpServers": {
    "mnemosyne": {
      "command": "mnemosyne",
      "args": ["serve"],
      "env": {
        "OPENAI_API_KEY": "${env:OPENAI_API_KEY}",
        "RUST_LOG": "info"
      },
      "settings": {
        "min_severity": "HIGH",
        "enable_ai": true
      }
    }
  }
}
```

#### Zed

Edit `~/.config/zed/settings.json`:

```json
{
  "mcp": {
    "servers": {
      "mnemosyne": {
        "command": "mnemosyne",
        "args": ["serve", "-c", "~/.config/mnemosyne/config.toml"]
      }
    }
  }
}
```

---

## AI/LLM Configuration

### OpenAI (Default)

```toml
[llm]
provider = "openai"
model = "gpt-4"
max_tokens = 2000
temperature = 0.3
```

Required environment variable:
```bash
export OPENAI_API_KEY="sk-..."
```

### Anthropic Claude

```toml
[llm]
provider = "anthropic"
model = "claude-3-opus-20240229"
max_tokens = 2000
```

Required environment variable:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Local LLM

```toml
[llm]
provider = "local"
endpoint = "http://localhost:8080/v1"
model = "llama-2-13b"
```

Compatible with:
- Ollama
- LM Studio
- LocalAI
- Any OpenAI-compatible API

### Disabling AI Features

```toml
[general]
enable_ai = false

# or

[ai]
enabled = false
```

Or via CLI:
```bash
mnemosyne analyze heap.hprof --no-ai
```

---

## Performance Tuning

### For Small Heaps (< 1 GB)

```toml
[parser]
use_mmap = false
threads = 4

[performance]
enable_cache = false
```

### For Large Heaps (> 10 GB)

```toml
[parser]
use_mmap = true
threads = 16  # or 0 for auto

[performance]
enable_cache = true
max_cache_gb = 50
```

### For CI/CD Environments

```toml
[general]
verbose = false
output_format = "json"

[analysis]
enable_ai = false  # Faster, no API dependency

[performance]
enable_cache = true
cache_dir = "/tmp/mnemosyne-ci-cache"
```

### Memory-Constrained Systems

```toml
[parser]
use_mmap = true  # Essential for low memory
threads = 2

[performance]
enable_cache = false  # Save disk space
```

---

## Advanced Configuration

### Custom Leak Detectors

```toml
[analysis.custom_detectors]
# Define custom leak patterns

[[analysis.custom_detectors.patterns]]
name = "MyFrameworkLeak"
class_pattern = "com.myframework.*.Cache$"
min_instances = 1000
severity = "HIGH"
```

### Report Templates

```toml
[report.templates]
# Path to custom report templates
markdown = "~/.config/mnemosyne/templates/report.md.hbs"
html = "~/.config/mnemosyne/templates/report.html.hbs"
```

---

## Configuration Validation

Validate your configuration:

```bash
mnemosyne config validate
mnemosyne config validate -c custom.toml
```

Show current configuration:

```bash
mnemosyne config show
```

Show configuration sources:

```bash
mnemosyne config show --sources
```

---

## Examples

See [examples/configs/](examples/configs/) for complete configuration examples for various use cases.
