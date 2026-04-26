# Mnemosyne Example Projects

This directory contains small, self-contained Java programs that reproduce common JVM memory problems and matching walkthroughs for analyzing them with Mnemosyne.

Each example is designed to be:

- easy to compile with `javac`
- easy to run with `java`
- small enough to understand in one sitting
- useful for practicing heap-dump capture and CLI analysis

For the full CLI reference, see [the Mnemosyne user guide](../docs/user-guide.md).

## Available Examples

| Example | Memory issue pattern | Best Mnemosyne views | Entry point |
| --- | --- | --- | --- |
| [cache-leak](cache-leak/README.md) | Unbounded `HashMap` cache that retains entries forever | `analyze --collections`, `leaks`, `gc-path`, `map` | `CacheLeakApp.java` |
| [thread-leak](thread-leak/README.md) | Worker threads that keep large buffers and are never cleaned up | `analyze --threads`, `leaks`, `gc-path`, `map` | `ThreadLeakApp.java` |
| [string-duplication](string-duplication/README.md) | Repeated duplicate `String` instances from CSV-like parsing | `analyze --strings`, `leaks`, `gc-path`, `map` | `StringDupApp.java` |

## Prerequisites

- JDK 11 or newer
- `javac` and `java` on your `PATH`
- `jmap` available if you want a live heap snapshot instead of an OOM-triggered dump
- `mnemosyne-cli` on your `PATH`

## Quick Start

Pick an example directory, compile the Java file, run it with heap-dump flags, then analyze the resulting `.hprof` file.

Example with the cache leak project:

```bash
cd examples/cache-leak
javac CacheLeakApp.java
java -Xms64m -Xmx256m -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=cache-leak.hprof CacheLeakApp
mnemosyne-cli parse cache-leak.hprof
mnemosyne-cli analyze cache-leak.hprof --profile incident-response --collections --top-instances
mnemosyne-cli leaks cache-leak.hprof --leak-kind cache
```

If you prefer capturing a dump from a still-running process instead of waiting for an out-of-memory crash:

```bash
jps -l
jmap -dump:format=b,file=cache-leak-live.hprof <pid>
```

Then point Mnemosyne at the captured dump:

```bash
mnemosyne-cli analyze cache-leak-live.hprof --profile incident-response --collections --top-instances
```

## How To Use These Examples

1. Run the Java app until it produces a heap dump.
2. Start with `mnemosyne-cli parse <dump.hprof>` for a quick sanity check.
3. Use `mnemosyne-cli analyze <dump.hprof>` with the relevant optional analyzers.
4. Narrow down suspects with `mnemosyne-cli leaks <dump.hprof>`.
5. Use `mnemosyne-cli explain`, `mnemosyne-cli gc-path`, and `mnemosyne-cli map` to move from symptom to source.

Each example subdirectory includes a focused walkthrough with concrete commands, interpretation guidance, and a recommended code fix.