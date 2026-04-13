# CLI Analysis Workflow

Scenario: a service incident leaves `heap.hprof` on disk after memory use climbs and restart pressure starts hiding the root cause. This flow keeps the first pass lightweight, then escalates into graph-backed analysis and follow-up investigation commands.

## 1. Quick Parse

```bash
mnemosyne-cli parse heap.hprof
```

Use `parse` first when you want the header, record counts, and aggregate record-category totals without paying for the richer object-graph path.

## 2. Triage Leak Suspects

```bash
mnemosyne-cli leaks heap.hprof --min-severity high --package com.example --leak-kind cache
```

Use `leaks` to narrow the shortlist before you spend time on a full report. Repeat `--package` or `--leak-kind` if you need to widen the incident slice.

## 3. Run Full Analysis

```bash
mnemosyne-cli analyze heap.hprof --group-by package --threads --strings --collections --classloaders --top-instances --top-n 10 --min-capacity 32
```

This is the richer operator path when you need grouped histogram output plus the optional thread, string, collection, classloader, and top-instance reports in one run.

## 4. Compare Before and After

```bash
mnemosyne-cli diff before.hprof after.hprof
```

Use `diff` when you have a known-good or pre-change snapshot and want growth evidence instead of a single-dump snapshot.

## 5. Explain the Top Suspect

```bash
mnemosyne-cli explain heap.hprof --leak-id leak-usersession-1 --min-severity high
```

Use `explain` once you have a candidate worth summarizing for an incident channel or remediation ticket.

## 6. Map the Leak Back to Code

```bash
mnemosyne-cli map leak-usersession-1 --project-root ./service --class com.example.UserSessionCache
```

Use `map` to turn a leak identifier and class into likely source locations in the owning repo.

## 7. Trace a GC Root Path

```bash
mnemosyne-cli gc-path heap.hprof --object-id 0x1000 --max-depth 8
```

Use `gc-path` when you need a retention chain to confirm why an object is still reachable.

## Notes

- `leaks` and `analyze` both attempt the graph-backed path first, then fall back to heuristic output with explicit provenance markers when the heap dump cannot support the full graph path.
- `analyze` is the richer path because it can attach grouped histogram data and the optional investigation reports to the same run.
- `diff` gains class-level instance, shallow-byte, and retained-byte deltas when both snapshots successfully build object graphs.
