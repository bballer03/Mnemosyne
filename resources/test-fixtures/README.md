# Test Fixtures

This directory documents both the synthetic HPROF fixtures and the optional real-world heap dump used by Mnemosyne tests.

## Synthetic fixtures

- `build_simple_fixture()` in `core/src/hprof/test_fixtures.rs`
  - Generates a small, valid HPROF binary in memory.
  - Uses 8-byte identifiers and the `JAVA PROFILE 1.0.2` header.
  - Adds string records for `java/lang/Object`, `com/example/Node`, `next`, `value`, and `[Lcom/example/Node;`.
  - Adds `LOAD_CLASS` records for `java/lang/Object`, `com/example/Node`, and the node array class.
  - Adds a `HEAP_DUMP` with:
    - one GC root thread object
    - class dumps for `Object` and `Node`
    - three `Node` instances linked as `node1 -> node2 -> node3`
    - one object array referencing two nodes
    - one primitive `int[]` array with values `10, 20, 30`

- `build_segment_fixture()` in `core/src/hprof/test_fixtures.rs`
  - Generates a valid HPROF binary that uses `HEAP_DUMP_SEGMENT` (`0x1C`) instead of a monolithic `HEAP_DUMP` record.
  - Exists specifically to cover the real-world parser path that many JVM heap dumps use.
  - Exercises the same core object graph shape as the simple fixture so segmented-record parsing can be regression tested independently.

## Real-world fixture

- `heap.hprof`
  - Optional real JVM heap dump used by CLI integration tests to validate the end-to-end `parse`, `analyze`, `leaks`, and `gc-path` pipeline against a non-synthetic heap.
  - This file may be absent locally and may not be committed because real heap dumps are often too large or contain environment-specific data.
  - When the file is absent, the real-world integration tests skip gracefully and print a skip message instead of failing the workspace.

## How tests use these fixtures

- Synthetic fixtures provide deterministic, fast coverage for parser, graph, dominator, and GC-path behavior without checking large binary blobs into the repository.
- The real-world `heap.hprof` fixture validates behavior that synthetic fixtures can miss, including `HEAP_DUMP_SEGMENT` parsing and full command-path behavior on an actual JVM dump.
- Together, they cover both stable unit-test scenarios and the production-shaped parser path that previously allowed the segmented-tag bug to slip through.

## Generating or obtaining `heap.hprof`

For local testing, generate a small heap dump from a JVM process you control:

```bash
# Find a target JVM
jps

# Capture a heap dump
jmap -dump:format=b,file=resources/test-fixtures/heap.hprof <PID>
```

Alternative sources:

- Export a sanitized heap dump from a local sample app or integration environment.
- Copy a previously approved test heap dump into `resources/test-fixtures/heap.hprof`.

Keep the fixture small enough for local test runs, and avoid using production dumps that contain sensitive data unless they have been explicitly sanitized and approved for local handling.