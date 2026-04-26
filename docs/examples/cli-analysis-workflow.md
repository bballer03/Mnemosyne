# CLI Analysis Workflow

Scenario: a service incident leaves `heap.hprof` on disk after memory use climbs and restart pressure starts hiding the root cause. This flow keeps the first pass lightweight, then escalates into graph-backed analysis, flame-graph export, and follow-up investigation commands.

## 1. Quick Parse

```bash
mnemosyne-cli parse heap.hprof
```

Use `parse` first when you want the header, record counts, and aggregate record-category totals without paying for the richer object-graph path.

For very large dumps, switch to overview mode when you want bounded-memory class-resolved triage:

```bash
mnemosyne-cli parse heap.hprof --mode overview
```

## 2. Triage Leak Suspects

```bash
mnemosyne-cli leaks heap.hprof --min-severity high --package com.example --leak-kind cache
```

Use `leaks` to narrow the shortlist before you spend time on a full report. Repeat `--package` or `--leak-kind` if you need to widen the incident slice.

## 3. Run Full Analysis

```bash
mnemosyne-cli analyze heap.hprof --mode deep --group-by package --threads --strings --collections --classloaders --top-instances --top-n 10 --min-capacity 32
```

This is the richer operator path when you need grouped histogram output plus the optional thread, string, collection, classloader, and top-instance reports in one run.

If you only need graph-free triage, use `--mode overview` instead. That path is streaming and bounded-memory, but it reports approximate shallow sizes only and does not produce retained sizes or leak suspects.

## 4. Generate Flame Graphs

```bash
mnemosyne-cli flamegraph heap.hprof -o flame-dominator.svg --mode deep --root dominator
mnemosyne-cli flamegraph heap.hprof -o flame-class.folded --mode deep --format folded-stack --root class-hierarchy
mnemosyne-cli flamegraph heap.hprof -o flame-gc-path.json --mode deep --format json --root gc-root-path
```

Use `dominator` for the broad retained-size answer to "what holds memory", `class-hierarchy` when you want inheritance-chain rollups, and `gc-root-path` when you want shortest-path reachability context for important retained objects.

Open the SVG in a browser after generation: you get the standard interactive flame graph view with colored stacked frames, hover tooltips, click-to-zoom navigation, and search. This workflow does not inline a screenshot because the artifact is more useful as an openable file than as a static Markdown image.

If `--mode auto` would flip to overview because the dump is at or above the 4 GiB cutoff, `mnemosyne-cli flamegraph` exits `5`; rerun with `--mode deep` only when you have enough RAM for the full graph-backed pass.

## 5. Compare Before and After

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Use `diff` when you have a known-good or pre-change snapshot and want growth evidence instead of a single-dump snapshot.

## 6. Explain the Top Suspect

```bash
mnemosyne-cli explain heap.hprof --leak-id leak-usersession-1 --min-severity high
```

Use `explain` once you have a candidate worth summarizing for an incident channel or remediation ticket.

## 7. Map the Leak Back to Code

```bash
mnemosyne-cli map leak-usersession-1 --project-root ./service --class com.example.UserSessionCache
```

Use `map` to turn a leak identifier and class into likely source locations in the owning repo.

## 8. Trace a GC Root Path

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x1000 --max-depth 8
```

Use `gc-path` when you need a retention chain to confirm why an object is still reachable.

## Notes

- `leaks` and `analyze` both attempt the graph-backed path first, then fall back to heuristic output with explicit provenance markers when the heap dump cannot support the full graph path.
- `analyze` is the richer path because it can attach grouped histogram data and the optional investigation reports to the same run.
- `flamegraph` is the visualization follow-through for the deep path; it always needs graph-backed analysis and writes an artifact via `-o`.
- `auto` is the default mode on `parse` and `analyze`; it resolves to overview at or above 4 GiB unless `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` overrides the cutoff.
- `auto` on `flamegraph` is stricter: if it resolves to overview, the command exits `5` and asks you to rerun with `--mode deep` if the machine can handle it.
- `diff` gains class-level instance, shallow-byte, and retained-byte deltas when both snapshots successfully build object graphs.
