# String Duplication Example

This example simulates a parser that repeatedly creates duplicate `String` instances for a small set of recurring values such as city names and statuses.

For the full CLI reference, see [the Mnemosyne user guide](../../docs/user-guide.md).

## What It Demonstrates

- excessive duplicate `String` instances from CSV-like parsing
- heap growth caused by repeated copies of the same logical values
- a pattern that should stand out in Mnemosyne's string analysis

## How To Build And Run

From this directory:

```bash
javac StringDupApp.java
java -Xms64m -Xmx256m -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=string-duplication.hprof StringDupApp
```

What happens:

- the app repeatedly creates fresh `String` objects for the same small vocabulary
- duplicate city, status, and row strings accumulate in lists
- the process eventually throws `OutOfMemoryError` and writes a heap dump

## How To Generate The Heap Dump

Automatic dump on OOM:

```bash
java -Xms64m -Xmx256m -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=string-duplication.hprof StringDupApp
```

Manual dump from a live process:

```bash
jps -l
jmap -dump:format=b,file=string-duplication-live.hprof <pid>
```

Use the manual path if you want to capture a dump once duplicate strings are clearly visible but before the JVM exits.

## How To Analyze With Mnemosyne

Quick sanity check:

```bash
mnemosyne-cli parse string-duplication.hprof
```

Main analysis run for this example:

```bash
mnemosyne-cli analyze string-duplication.hprof --profile incident-response --strings --top-instances
```

Leak-focused summary:

```bash
mnemosyne-cli leaks string-duplication.hprof
```

This example may or may not produce a classic leak candidate. If `leaks` prints `No leak suspects detected.`, that is still consistent with the example: the main signal is duplicate-string waste in the `--strings` analysis.

Natural-language explanation when `leaks` returns a candidate:

```bash
mnemosyne-cli explain string-duplication.hprof --leak-id <leak-id-from-leaks>
```

Optional object lookup for individual `String` instances:

```bash
mnemosyne-cli query string-duplication.hprof "SELECT @objectId, @className FROM \"java.lang.String\" LIMIT 10"
```

GC-root trace for one retained string object:

```bash
mnemosyne-cli gc-path string-duplication.hprof --object-id <string-object-id> --max-depth 8
```

Source mapping back to this sample app:

```bash
mnemosyne-cli map <leak-id-from-leaks> --project-root . --class StringDupApp
```

## What To Look For

- a string analysis section with a small set of repeated values consuming surprising space
- many `java.lang.String` objects and their backing arrays near the top of retained-size views
- GC paths that end in the long-lived lists storing parsed values
- leak output that may be empty even though string analysis clearly shows waste
- explanations that call out duplicate-string waste rather than a single object leak

The key lesson is that not every memory problem is one big leaking object. Sometimes it is the same small data copied far too many times.

## How To Fix It

The fix is to canonicalize repeated values instead of forcing a fresh `String` object every time.

```java
private static final Map<String, String> CITY_POOL = new HashMap<>();

private static String canonicalizeCity(String raw) {
    return CITY_POOL.computeIfAbsent(raw, key -> key);
}
```

Then use the canonical value instead of `new String(...)`. If the vocabulary is fixed and small, an `enum` or lookup table is even better.