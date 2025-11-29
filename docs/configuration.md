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

1. `.mnemosyne.toml` in current directory
2. `~/.config/mnemosyne/config.toml`
3. `/etc/mnemosyne/config.toml` (Linux)
4. Built-in defaults

### Example Configuration

```toml
# .mnemosyne.toml

[general]
# Enable verbose logging
verbose = false

# Output format: "text", "json", "markdown", "html"
output_format = "text"

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

# Enable AI-powered analysis
enable_ai = true

[llm]
# LLM provider: "openai", "anthropic", "local"
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

# Custom LLM endpoint
export MNEMOSYNE_LLM_ENDPOINT="http://localhost:8080/v1"

# Model to use
export MNEMOSYNE_LLM_MODEL="gpt-4"
```

### Performance

```bash
# Number of parser threads
export MNEMOSYNE_THREADS=8

# Disable memory-mapped I/O
export MNEMOSYNE_USE_MMAP=false

# Cache directory
export MNEMOSYNE_CACHE_DIR=/tmp/mnemosyne-cache
```

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
mnemosyne -f json analyze heap.hprof
mnemosyne --format json analyze heap.hprof
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
  --ai                   Enable AI analysis
  --package <PKG>        Filter by package
  --min-severity <LVL>   Minimum severity (LOW|MEDIUM|HIGH|CRITICAL)
  --leak-types <TYPES>   Comma-separated leak types
  -o, --output <FILE>    Output file
```

### Leaks Command

```bash
mnemosyne leaks [OPTIONS] <HEAP_FILE>

Options:
  --package <PKG>        Filter by package
  --min-severity <LVL>   Minimum severity
  --top <N>             Show top N leaks (default: 10)
```

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
[analysis]
enable_ai = false
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
