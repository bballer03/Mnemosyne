# Step 11 Dense Synthetic Heaps Design

> Status: approved via unattended execution override
> Date: 2026-04-12
> Scope: Step 11 large-dump scaling validation tooling only

## Goal

Replace the current sparse synthetic heap generator with a denser and more representative generator so Step 11 scaling measurements stress both Mnemosyne's object-graph/dominator pipeline and the opt-in investigation analyzers.

## Problem

The current `SyntheticHeapApp.java` scales dump size mostly by allocating a few very large arrays per chunk. That grows `.hprof` bytes without materially increasing parsed object count or reference count. Initial large-tier results therefore produced unrealistically low RSS:dump ratios and are not suitable for the Step 11 decision gate.

## Constraints

- Keep the shell interface stable:
  - `scripts/generate_synthetic_heap.sh --size-mb <mb> --output <file.hprof> [--xmx-mb <mb>]`
- Keep dumps in temp/output locations only.
- Preserve the `READY <pid> <root-count>` protocol used by the dump wrapper.
- Prefer the smallest change set that makes the scaling measurements decision-quality.

## Chosen Approach

Use a layered dense-object generator.

Each generated heap tier will contain many retained object clusters rather than a few payload-heavy chunks. Each cluster will mix linked node objects, cross-links, object arrays, `ArrayList`, `HashMap`, duplicate strings, unique strings, and moderate primitive arrays. Primitive arrays remain part of the heap shape, but they stop being the dominant growth mechanism.

This approach intentionally targets both:

- graph stress: more heap objects and more references for `ObjectGraph` and dominator memory
- realistic analyzer input: strings, collections, and arrays for `--threads --strings --collections`

## Heap Shape

### Core objects

- `Node`
  - small retained object
  - object references such as `next`, `back`, `jump`, `owner`, and `payload`
  - compact primitive payload so per-node size is non-trivial but not dominant

- `Cluster`
  - owns a dense set of nodes plus collection/object-array wrappers
  - creates long chains, back-links, and cross-links between node groups
  - owns realistic containers used by investigation analyzers

- root sets
  - retained lists/maps/arrays that keep all clusters reachable
  - global duplicate string pools and lookup structures

### Per-cluster contents

- many `Node` objects connected in a chain
- additional side links and back-links to increase reference density
- `Node[]` arrays for object-array pressure
- `ArrayList<Node>` and `HashMap<String, Node>` for collection realism
- duplicate and unique `String` instances
- moderate `byte[]`, `char[]`, and `int[]` payloads attached to many objects rather than concentrated in a few megastructures

## Success Criteria

1. `scripts/tests/test_generate_synthetic_heap_density.sh` passes honestly.
2. A `32 MB` synthetic heap parses with at least `100000` estimated objects.
3. A larger generated heap yields materially more parsed objects than a smaller heap, not just more dump bytes.
4. Existing Step 11 wrapper and RSS smoke tests remain compatible.
5. Recalibrated 500 MB / 1 GB / 2 GB outputs are suitable for Step 11 memory-decision measurements.

## Testing Strategy

TDD for generator behavior:

1. Keep the existing density smoke test as the first failing test.
2. Add a second failing test that compares object-count growth across sizes.
3. Implement the minimal generator redesign to satisfy both tests.
4. Re-run existing generator/wrapper/RSS smoke tests.
5. Recalibrate the size mapping.
6. Re-run the Step 11 validation batch and inspect `summary.tsv`.

## File Impact

- Modify: `scripts/java/SyntheticHeapApp.java`
- Possibly modify: `scripts/tests/test_generate_synthetic_heap_density.sh`
- Add: one generator growth smoke test if needed
- Reuse unchanged interfaces in:
  - `scripts/generate_synthetic_heap.sh`
  - `scripts/run_step11_scaling_validation.sh`

## Risks

- Calibration will change because denser heaps change the relationship between requested target size and produced dump size.
- If the generator becomes too dense relative to `-Xmx`, large-tier generation may fail before dump capture; the wrapper must fail clearly in that case.
- Investigation results could still be unrepresentative if string/collection content is too small, so the object mix must remain intentionally varied.

## Out Of Scope

- Changes to core parser, object graph, dominator, or CLI command semantics
- Architectural memory remediation beyond Step 11 measurement tooling
- New generator CLI flags unless absolutely required
