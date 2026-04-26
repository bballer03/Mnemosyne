# Cache Leak Example

This example reproduces a classic cache leak: a long-lived `HashMap` that keeps growing because entries are never evicted.

For the full CLI reference, see [the Mnemosyne user guide](../../docs/user-guide.md).

## What It Demonstrates

- an unbounded cache retained by a static field
- a large retained set made mostly of `byte[]` values
- a leak pattern that usually appears as a cache or collection hotspot

## How To Build And Run

From this directory:

```bash
javac CacheLeakApp.java
java -Xms64m -Xmx256m -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=cache-leak.hprof CacheLeakApp
```

What happens:

- the app inserts a new entry into the cache on every loop iteration
- each entry keeps about 10 KiB alive
- the process eventually throws `OutOfMemoryError` and writes `cache-leak.hprof`

## How To Generate The Heap Dump

Automatic dump on OOM:

```bash
java -Xms64m -Xmx256m -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=cache-leak.hprof CacheLeakApp
```

Manual dump from a live process:

```bash
jps -l
jmap -dump:format=b,file=cache-leak-live.hprof <pid>
```

Use the OOM path when you want the easiest reproduction. Use `jmap` when you want to stop earlier and inspect the leak before the process crashes.

## How To Analyze With Mnemosyne

Quick sanity check:

```bash
mnemosyne-cli parse cache-leak.hprof
```

Main analysis run for this example:

```bash
mnemosyne-cli analyze cache-leak.hprof --profile incident-response --collections --top-instances
```

Leak-focused view:

```bash
mnemosyne-cli leaks cache-leak.hprof --leak-kind cache
```

Natural-language explanation of the leak candidate you just found:

```bash
mnemosyne-cli explain cache-leak.hprof --leak-id <leak-id-from-leaks>
```

Optional object lookup to find a cache object id before tracing reachability:

```bash
mnemosyne-cli query cache-leak.hprof "SELECT @objectId, table FROM \"java.util.HashMap\" LIMIT 5"
```

GC-root trace for a specific object returned by `query` or another report:

```bash
mnemosyne-cli gc-path cache-leak.hprof --object-id <object-id-from-query> --max-depth 8
```

Source mapping back to this sample app:

```bash
mnemosyne-cli map <leak-id-from-leaks> --project-root . --class CacheLeakApp
```

## What To Look For

- a dominant retained set rooted in `java.util.HashMap` or the `CacheLeakApp` cache path
- many `byte[]` values retained by the cache
- a leak suspect that Mnemosyne categorizes as cache-like retention
- top retained objects or GC paths that point back to a long-lived static owner

In practice, the important signal is not just that there are many `byte[]` objects. It is that one cache owner keeps them all reachable.

## How To Fix It

The real fix is to make the cache bounded and lifecycle-aware. For a simple in-process fix, replace the unbounded `HashMap` with an evicting map.

```java
private static final int MAX_ENTRIES = 10_000;

private static final Map<String, byte[]> cache = new LinkedHashMap<>(16, 0.75f, true) {
    @Override
    protected boolean removeEldestEntry(Map.Entry<String, byte[]> eldest) {
        return size() > MAX_ENTRIES;
    }
};
```

Also consider:

- clearing request-scoped entries when the request finishes
- adding time-based eviction
- using a dedicated cache library such as Caffeine for production code