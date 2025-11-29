# Quick Start Guide

Get up and running with Mnemosyne in 5 minutes!

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
sudo cp target/release/mnemosyne /usr/local/bin/
```

### Option 2: Using Cargo

```bash
cargo install mnemosyne
```

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
Parsing heap dump: heap.hprof (2.4 GB)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100% • 3.2s

✓ Heap Summary:
  Total Objects: 1,234,567
  Total Size: 2.4 GB
  Classes: 4,321

Top 5 Memory Consumers:
  1. java.lang.String[]         421 MB  (17.2%)
  2. com.example.UserSession    385 MB  (15.7%)
  3. byte[]                     312 MB  (12.7%)
  4. java.util.HashMap$Node     245 MB  (10.0%)
  5. char[]                     198 MB  ( 8.1%)
```

### Step 3: Detect Leaks

```bash
mnemosyne leaks heap.hprof
```

Output:

```
🔍 Analyzing for memory leaks...

⚠️  2 Potential Leaks Detected:

1. com.example.UserSessionCache (HIGH SEVERITY)
   Instances: 125,432
   Retained Size: 512 MB
   GC Root: Thread "session-cleanup" (BLOCKED)
   
   Issue: Cache growing unbounded, cleanup thread blocked

2. okhttp3.Response (MEDIUM SEVERITY)
   Instances: 8,921
   Retained Size: 89 MB
   
   Issue: Unclosed HTTP response bodies
```

### Step 4: Get AI Insights (Optional)

First, set your API key:

```bash
export OPENAI_API_KEY="sk-..."
```

Then run AI analysis:

```bash
mnemosyne analyze heap.hprof --ai
```

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

---

## Common Use Cases

### CI/CD Integration

```bash
# In your build pipeline
mnemosyne analyze heap.hprof --json --min-severity HIGH > report.json

# Check for critical leaks
if jq -e '.leaks[] | select(.severity == "CRITICAL")' report.json; then
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
Memory Growth Analysis
═══════════════════════════════════════════════════════════════

Overall Change: -347 MB (-14.2%)

Classes with Significant Changes:
┌────────────────────────────────┬──────────┬──────────┬─────────┐
│ Class                          │ Before   │ After    │ Change  │
├────────────────────────────────┼──────────┼──────────┼─────────┤
│ com.example.UserSession        │ 385 MB   │  89 MB   │ -77%  ✓ │
│ java.lang.String[]             │ 421 MB   │ 398 MB   │  -5%  ✓ │
│ byte[]                         │ 312 MB   │ 334 MB   │  +7%  ⚠ │
└────────────────────────────────┴──────────┴──────────┴─────────┘

✓ Optimization successful!
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
