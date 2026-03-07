# Quick Start Guide

Get up and running with Mnemosyne in 5 minutes!

Current release: v0.1.1. The core crate is now organized into grouped module directories under `core/src/`, but the CLI workflow shown here is unchanged.

## Prerequisites

- Rust 1.70+ installed
- A JVM heap dump file (`.hprof`)
- Optional: OpenAI API key for AI features

---

## Installation

### Option 1: Build from Source

```bash
git clone https://github.com/bballer03/mnemosyne
cd mnemosyne
cargo build --release
sudo cp target/release/mnemosyne-cli /usr/local/bin/
```

### Option 2: Using Cargo

```bash
cargo install mnemosyne-cli
```

### Option 3: Using Homebrew (macOS)

```bash
brew install ./HomebrewFormula/mnemosyne.rb
```

The current packaged binary name is `mnemosyne-cli`. Commands below still use the shorter `mnemosyne` form in examples, but the packaged artifact currently installs as `mnemosyne-cli`.

Mnemosyne is maintained by **bballer03**. For releases, issues, and discussions, use the upstream repository at https://github.com/bballer03/mnemosyne.

---

## Your First Analysis

### Step 1: Get a Heap Dump

If you don't have one already:

```bash
# Find your Java process
jps

# Take a heap dump
jmap -dump:format=b,file=heap.hprof <PID>
```

### Step 2: Parse It

```bash
mnemosyne parse heap.hprof
```

You'll see output like:

```
✓ Parsed heap dump.
Heap path: heap.hprof
File size: 2.40 GB
Format: JAVA PROFILE 1.0.2 | Identifier bytes: 8 | Timestamp(ms): 1709836800000
Estimated objects: 1234567
Total HPROF records: 5678901
Top heap record categories by aggregate bytes:
 #  Record Category           Bytes      Share  Entries
 1  INSTANCE_DUMP           421.00 MB    50.1%   345678
 2  PRIMITIVE_ARRAY_DUMP    312.00 MB    37.1%   234567
 3  OBJECT_ARRAY_DUMP        89.00 MB    10.6%    67890
 4  CLASS_DUMP               12.00 MB     1.4%     4321
 5  HEAP_DUMP_SEGMENT         7.50 MB     0.9%       12
Top record tags:
 Record Tag                    Hex  Entries       Size
 HEAP_DUMP_SEGMENT            0x1C       12  841.50 MB
 STRING_IN_UTF8               0x01    89012   15.00 MB
 LOAD_CLASS                   0x02     4321    0.50 MB
 STACK_TRACE                  0x05     1234    0.20 MB
 STACK_FRAME                  0x04     5678    0.10 MB
```

These values are calculated from the raw HPROF record tags, so even a lightweight `parse` run tells you which record categories dominate the dump. `mnemosyne leaks` still uses this fast histogram path directly, while `mnemosyne analyze` can now upgrade to full object-graph parsing and real dominator-backed retained sizes when the heap contains enough detail.

### Step 3: Detect Leaks

```bash
mnemosyne leaks heap.hprof
```

Output:

```
✓ Leak detection complete.
Potential leaks:
 Leak ID               Class                               Kind      Severity  Retained    Instances
 leak-usersession-1    com.example.UserSessionCache         Cache     High      512.00 MB      125432
 leak-okhttp-1         okhttp3.Response                     Resource  Medium     89.00 MB        8921

  Leak: leak-usersession-1
    Description: Cache growing unbounded, cleanup thread blocked
    Provenance:
      [SYNTHETIC] generated from histogram heuristics

  Leak: leak-okhttp-1
    Description: Unclosed HTTP response bodies
    Provenance:
      [SYNTHETIC] generated from histogram heuristics
```

When class names or leak IDs are too long for the table, Mnemosyne truncates them inline and prints a disclosure section beneath with the full values keyed by row number.

Limit the output to specific categories whenever you need deterministic CI signals:

```bash
mnemosyne leaks heap.hprof --leak-kind cache --leak-kind thread
```

Because the leak engine now consumes the parsed class histogram, package or severity filters operate on real data first and only fall back to synthetic names if the heap lacks useful symbols.

The dedicated `leaks` command remains summary-driven today. If you need retained sizes backed by the actual object graph and dominator tree, run `mnemosyne analyze` and check the response/report provenance to see whether the graph-backed path succeeded.

### Step 4: Get AI Insights (Optional)

First, set your API key:

```bash
export OPENAI_API_KEY="sk-..."
```

Then run AI analysis:

```bash
mnemosyne analyze heap.hprof --ai
```

When full HPROF object-graph parsing succeeds, `analyze` now computes retained sizes from the dominator tree and includes graph-backed dominator metrics in the report. If parsing or filters prevent that path, Mnemosyne falls back to the summary-driven preview and labels the response accordingly.

Output:

```
🧠 AI Analysis:

═══════════════════════════════════════════════════════════════

Root Cause Analysis:
────────────────────────────────────────────────────────────────
The UserSessionCache is experiencing unbounded growth because the 
cleanup thread is deadlocked. The thread is waiting on a monitor 
lock held by the request handler, which is itself waiting on the 
cleanup thread to finish.

Impact:
────────────────────────────────────────────────────────────────
• 512 MB of unnecessary memory retention
• Risk of OutOfMemoryError under sustained load
• Degraded response times due to thread contention

Recommendations:
────────────────────────────────────────────────────────────────
1. Break the deadlock cycle:
   - Add a timeout to cache.cleanup() method
   - Use tryLock() with timeout instead of synchronized
   
2. Architectural improvements:
   - Replace synchronized HashMap with ConcurrentHashMap
   - Use weak references for session storage
   - Implement time-based eviction with ScheduledExecutorService

Code fixes available. Run: mnemosyne fix heap.hprof
```

### Step 5: Persist Your Preferences (Optional)

Drop a `.mnemosyne.toml` file in your project (or `~/.config/mnemosyne/config.toml`) to avoid retyping flags:

```toml
# .mnemosyne.toml

[general]
output_format = "toon"
enable_ai = true

[ai]
model = "gpt-4.1-mini"
temperature = 0.2

[analysis]
min_severity = "MEDIUM"
packages = ["com.example", "org.demo"]
leak_types = ["CACHE", "THREAD"]
```

When Mnemosyne starts it resolves configuration in this order:

1. `mnemosyne --config /path/file.toml`
2. `$MNEMOSYNE_CONFIG`
3. `.mnemosyne.toml` (current directory)
4. `~/.config/mnemosyne/config.toml`
5. `/etc/mnemosyne/config.toml`
6. Built-in defaults

Use `mnemosyne config` to view the merged result along with the source file.

Prefer shell variables instead? Export the same knobs:

```bash
export MNEMOSYNE_MIN_SEVERITY=HIGH
export MNEMOSYNE_PACKAGES="com.example, org.demo"
export MNEMOSYNE_LEAK_TYPES="CACHE,THREAD"
```

`packages` now act as an allow-list for real classes first (only matching entries from the histogram become candidates) before Mnemosyne rotates through them while synthesizing fallback IDs. Likewise, `leak_types` either filters the actual leak list or, if none match, forces one synthetic entry per requested kind so your CI remains deterministic.

### Step 6: Save the Report

Need an artifact for CI or teammates?

```bash
# HTML report
mnemosyne analyze heap.hprof --format html --output-file report.html

# JSON payload for automated checks
mnemosyne analyze heap.hprof --format json --output-file report.json
```

`--output-file` works with every format (text/markdown/html/toon/json). If you omit it, the report streams to stdout so you can still pipe through `tee`.

---

## Common Use Cases

### CI/CD Integration

```bash
# In your build pipeline
mnemosyne analyze heap.hprof --format toon --min-severity HIGH > report.toon

# Check for critical leaks
if grep -q "severity=Critical" report.toon; then
  echo "Critical memory leak detected!"
  exit 1
fi
```

### Comparing Before/After

```bash
# Take heap dump before optimization
jmap -dump:format=b,file=before.hprof <PID>

# Run your optimization
# ...

# Take heap dump after
jmap -dump:format=b,file=after.hprof <PID>

# Compare
mnemosyne diff before.hprof after.hprof
```

Output:

```
Heap diff: before.hprof -> after.hprof
  Delta size: -347.00 MB
  Delta objects: -156789
  Top changes:
    - com.example.UserSession: -296.00 MB (before 385.00 MB -> after 89.00 MB)
    - java.lang.String[]: -23.00 MB (before 421.00 MB -> after 398.00 MB)
    - byte[]: +22.00 MB (before 312.00 MB -> after 334.00 MB)
```

### Filtering by Package

```bash
# Only analyze your application code
mnemosyne leaks heap.hprof --package com.myapp

# Exclude framework code
mnemosyne leaks heap.hprof --exclude-package org.springframework
```

---

## IDE Integration

### VS Code / Cursor

1. Install the MCP extension (if needed)

2. Create `.vscode/mcp-config.json`:
   ```json
   {
     "mcpServers": {
       "mnemosyne": {
         "command": "mnemosyne",
         "args": ["serve"]
       }
     }
   }
   ```

3. Restart VS Code

4. Now you can chat with Mnemosyne:
   - "Analyze heap.hprof"
   - "Show me the leak in UserSessionCache"
   - "Generate a fix for this leak"

---

## Next Steps

- Read the [Architecture](../ARCHITECTURE.md) to understand how it works
- Check the [API Documentation](api.md) for MCP integration
- See [Configuration](configuration.md) for advanced options
- Browse [Examples](examples/) for more use cases

---

## Troubleshooting

### "Command not found: mnemosyne"

Make sure `~/.cargo/bin` is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Failed to parse heap dump"

Ensure the file is a valid HPROF format:

```bash
file heap.hprof
# Should output: heap.hprof: Java hprof dump
```

### "AI analysis unavailable"

Set your OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
# Or add to ~/.bashrc or ~/.zshrc
```

### Slow parsing on large heaps

Enable memory-mapped I/O and increase threads:

```bash
mnemosyne parse heap.hprof --threads 16
```

---

## Getting Help

- **Documentation**: Check [docs/](.)
- **Issues**: [GitHub Issues](https://github.com/bballer03/mnemosyne/issues)
- **Discussions**: [GitHub Discussions](https://github.com/bballer03/mnemosyne/discussions)

---

Happy debugging! 🚀

- Read the [Architecture](../ARCHITECTURE.md) to understand how it works
- Check the [API Documentation](api.md) for MCP integration
- See [Configuration](configuration.md) for advanced options
- Browse [Examples](examples/) for more use cases

---

## Troubleshooting

### "Command not found: mnemosyne"

Make sure `~/.cargo/bin` is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Failed to parse heap dump"

Ensure the file is a valid HPROF format:

```bash
file heap.hprof
# Should output: heap.hprof: Java hprof dump
```

### "AI analysis unavailable"

Set your OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
# Or add to ~/.bashrc or ~/.zshrc
```

### Slow parsing on large heaps

Enable memory-mapped I/O and increase threads:

```bash
mnemosyne parse heap.hprof --threads 16
```

---

## Getting Help

- **Documentation**: Check [docs/](.)
- **Issues**: [GitHub Issues](https://github.com/bballer03/mnemosyne/issues)
- **Discussions**: [GitHub Discussions](https://github.com/bballer03/mnemosyne/discussions)

---

Happy debugging! 🚀
