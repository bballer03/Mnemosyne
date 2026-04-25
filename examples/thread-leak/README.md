# Thread Leak Example

This example simulates a thread leak by spawning worker threads that hold large buffers and are never cleaned up.

For the full CLI reference, see [the Mnemosyne user guide](../../docs/user-guide.md).

## What It Demonstrates

- long-lived worker threads that keep per-thread buffers alive
- a leak pattern that shows up as both thread retention and retained heap growth
- a common anti-pattern: creating threads instead of using a bounded executor

## How To Build And Run

From this directory:

```bash
javac ThreadLeakApp.java
java -Xms64m -Xmx256m -Xss256k -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=thread-leak.hprof ThreadLeakApp
```

Why the flags matter:

- `-Xmx256m` keeps the example small enough to fail quickly
- `-Xss256k` reduces native stack size so the retained buffers dominate sooner
- `-XX:+HeapDumpOnOutOfMemoryError` writes a dump when the JVM fails

## How To Generate The Heap Dump

Automatic dump on OOM:

```bash
java -Xms64m -Xmx256m -Xss256k -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=thread-leak.hprof ThreadLeakApp
```

Manual dump from a live process:

```bash
jps -l
jmap -dump:format=b,file=thread-leak-live.hprof <pid>
```

Manual capture is useful if you want to inspect the heap after a few hundred worker threads instead of waiting for failure.

## How To Analyze With Mnemosyne

Quick sanity check:

```bash
mnemosyne-cli parse thread-leak.hprof
```

Main analysis run for this example:

```bash
mnemosyne-cli analyze thread-leak.hprof --profile incident-response --threads --top-instances
```

Leak-focused view:

```bash
mnemosyne-cli leaks thread-leak.hprof --leak-kind thread
```

Natural-language explanation:

```bash
mnemosyne-cli explain thread-leak.hprof --leak-id <leak-id-from-leaks>
```

Optional object lookup for thread objects:

```bash
mnemosyne-cli query thread-leak.hprof "SELECT @objectId, name FROM \"java.lang.Thread\" LIMIT 10"
```

GC-root trace for one of the retained worker threads:

```bash
mnemosyne-cli gc-path thread-leak.hprof --object-id <worker-thread-object-id> --max-depth 8
```

Source mapping back to this sample app:

```bash
mnemosyne-cli map <leak-id-from-leaks> --project-root . --class ThreadLeakApp
```

## What To Look For

- a growing thread report with many `java.lang.Thread` objects
- retained buffers held by worker closures or thread-owned state
- GC paths that lead back to the static worker list
- top retained objects that suggest each thread keeps a large payload alive

The important pattern is that the threads are not transient work. They become permanent owners of memory.

## How To Fix It

The real fix is to stop creating unbounded threads and move the work into a bounded executor that can be shut down cleanly.

```java
ExecutorService pool = Executors.newFixedThreadPool(8);

for (int i = 0; i < 1_000; i++) {
    pool.submit(() -> {
        byte[] buffer = new byte[1_048_576];
        try {
            Thread.sleep(1_000);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
    });
}

pool.shutdown();
pool.awaitTermination(1, TimeUnit.MINUTES);
```

Also remove any static list that keeps completed workers reachable after their work is done.