# Test Fixtures

This directory documents the synthetic HPROF fixtures used by Mnemosyne tests.

## Current fixtures

- `build_simple_fixture()` in `core/src/test_fixtures.rs`
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

## Purpose

These fixtures give unit tests deterministic HPROF bytes without checking large binary files into the repository. The builder API also makes it easier to add focused heap-shape fixtures for parser, graph, and GC path tests as those subsystems gain coverage.