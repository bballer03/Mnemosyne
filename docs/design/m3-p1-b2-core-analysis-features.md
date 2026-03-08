# M3-P1-B2 — Core Analysis Features

> **Status:** READY FOR IMPLEMENTATION  
> **Parent:** [Milestone 3 — Core Heap Analysis Parity](milestone-3-core-heap-analysis-parity.md)  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Deliver four core analysis features that close the most critical MAT parity gaps: histogram grouping, MAT-style leak suspect ranking, unreachable object reporting, and enhanced (class-level) heap diff.

## Context

M3-P1-B1 landed HPROF tag centralization, Criterion benchmarks, and RSS tooling. The object graph pipeline is validated on real-world dumps (M1.5). The dominator tree produces correct retained sizes. This batch builds directly on those foundations to deliver analysis features that make Mnemosyne a credible MAT alternative.

## Scope

1. **Histogram grouping** — group by fully-qualified class, package prefix, ClassLoader
2. **MAT-style leak suspects** — retained/shallow ratio ranking, accumulation-point detection, reference chain context
3. **Unreachable objects** — objects not reachable from any GC root, with sizes and class breakdown
4. **Enhanced heap diff** — class-level comparison using object graph data (not just record-level)

## Non-scope

- OQL query engine (Phase 4)
- Thread inspection / STACK_TRACE parsing (Phase 3)
- ClassLoader leak detection (Phase 3 — distinct from classloader-based histogram grouping)
- Collection inspection (Phase 3)
- Streaming overview mode (Phase 4)
- Web UI / TUI (M4)
- AI/LLM wiring (M5)
- Files owned by other agents or milestones

---

## Feature 1: Histogram Grouping

### Algorithm

Given a built `ObjectGraph` and `DominatorTree`:

1. **By fully-qualified class** (default): iterate all objects, group by resolved class name via `ObjectGraph::class_name(class_id)`. For each group, accumulate: instance count, total shallow size, total retained size (from dominator tree).

2. **By package prefix**: extract the package from each class FQN by taking everything before the last `.` segment (e.g., `com.example.cache.LRUCache` → `com.example.cache`). If no `.`, use `<default>`. Group by package string.

3. **By ClassLoader**: group by `ClassInfo::class_loader_id`. Resolve the classloader's class name via the object graph for display purposes. Instances with classloader ID `0` are grouped under `<bootstrap>`.

### New Types

```rust
// In core/src/graph/metrics.rs

/// A single row in a grouped histogram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramEntry {
    /// The group key: class FQN, package name, or classloader name.
    pub key: String,
    /// Number of instances in this group.
    pub instance_count: u64,
    /// Sum of shallow sizes of all instances in this group.
    pub shallow_size: u64,
    /// Sum of retained sizes of all instances in this group.
    pub retained_size: u64,
}

/// The grouping strategy for histogram construction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HistogramGroupBy {
    Class,
    Package,
    ClassLoader,
}

/// A grouped histogram result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramResult {
    pub group_by: HistogramGroupBy,
    pub entries: Vec<HistogramEntry>,
    pub total_instances: u64,
    pub total_shallow_size: u64,
}
```

### New Function

```rust
// In core/src/graph/metrics.rs

pub fn build_histogram(
    graph: &ObjectGraph,
    dom: &DominatorTree,
    group_by: HistogramGroupBy,
) -> HistogramResult {
    // 1. Iterate graph.objects
    // 2. For each object, resolve group key based on group_by strategy
    // 3. Accumulate instance_count, shallow_size, retained_size per group
    // 4. Sort entries by retained_size descending
    // ...
}
```

### CLI Impact

- Add `--group-by class|package|classloader` flag to the `analyze` command.
- When specified, the analysis report includes a histogram section grouped accordingly.
- Default remains `class` if omitted.

### API Impact

- `AnalyzeResponse` gains an optional `histogram: Option<HistogramResult>` field.
- Future MCP `get_histogram` handler accepts `group_by` parameter.

---

## Feature 2: MAT-style Leak Suspects

### Algorithm

Replace the current naive `top_retained(20)` ranking in `graph_backed_leaks()` with a proper suspect-scoring algorithm:

1. **Retained/shallow ratio**: For each object, compute `ratio = retained_size / max(shallow_size, 1)`. Objects with high ratios are accumulation points — they are small themselves but retain large subgraphs.

2. **Accumulation-point detection**: An accumulation point is an object where:
   - `retained_size / shallow_size > threshold` (default: 10.0)
   - The object has a significant number of dominated children
   - The object is not the virtual root or a trivial container

3. **Reference chain context**: For each suspect, provide a short reference chain summary (up to 3 ancestors) by walking `DominatorTree::immediate_dominator()` upward. This gives users context like: `GC Root → ThreadPoolExecutor → ConcurrentHashMap → LeakyCache`.

4. **Scoring formula**: `score = retained_size * log2(ratio + 1)`. This ranks objects that both retain a lot AND have a high ratio above objects that merely retain a lot due to being near the root.

### New Types

```rust
// In core/src/analysis/engine.rs

/// A MAT-style leak suspect with retained/shallow analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakSuspect {
    /// The suspect object's ID.
    pub object_id: ObjectId,
    /// Fully-qualified class name.
    pub class_name: String,
    /// Shallow size of this object alone.
    pub shallow_size: u64,
    /// Retained size of this object's dominated subtree.
    pub retained_size: u64,
    /// Ratio of retained to shallow size.
    pub ratio: f64,
    /// Whether this object is an accumulation point.
    pub is_accumulation_point: bool,
    /// Number of objects dominated by this object.
    pub dominated_count: u64,
    /// Short reference chain from GC root to this object (up to 3 ancestors).
    pub reference_chain: Vec<String>,
    /// Composite score used for ranking.
    pub score: f64,
}
```

### Changes to Existing Code

- `graph_backed_leaks()` in [engine.rs](../../core/src/analysis/engine.rs) is refactored to compute `LeakSuspect` entries and convert them to `LeakInsight` for backward compatibility.
- `DominatorTree` must expose a method to retrieve shallow size: `pub fn shallow_size(&self, id: ObjectId) -> u64`. This requires storing or accepting the `ObjectGraph` reference. The simplest approach is to pass `&ObjectGraph` into the suspect-ranking function rather than adding state to `DominatorTree`.
- The existing `LeakInsight` struct gains an optional `shallow_size_bytes: Option<u64>` field and an optional `suspect_score: Option<f64>` field for richer reporting.

### Threshold Configuration

The accumulation-point threshold (default 10.0) should be a field on `AnalysisConfig`:

```rust
// In core/src/config.rs
pub struct AnalysisConfig {
    // ... existing fields ...
    /// Minimum retained/shallow ratio to flag as accumulation point (default: 10.0)
    pub accumulation_threshold: f64,
}
```

---

## Feature 3: Unreachable Objects

### Algorithm

1. Build a `HashSet<ObjectId>` of all reachable objects by BFS/DFS from GC roots through the `ObjectGraph`.
2. Any object in `graph.objects` not in the reachable set is unreachable.
3. Group unreachable objects by class name, accumulate count and shallow size per class.
4. Sort by total shallow size descending.

This is a straightforward graph reachability computation. The dominator tree already identifies objects reachable from GC roots through domination, but an explicit reachability walk is more correct because some objects may be reachable through non-dominating paths.

### New Types

```rust
// In core/src/graph/metrics.rs

/// Summary of unreachable objects in the heap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreachableSet {
    /// Total number of unreachable objects.
    pub total_count: u64,
    /// Total shallow size of all unreachable objects.
    pub total_shallow_size: u64,
    /// Breakdown by class: (class_name, count, shallow_size), sorted by size desc.
    pub by_class: Vec<UnreachableClassEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreachableClassEntry {
    pub class_name: String,
    pub count: u64,
    pub shallow_size: u64,
}
```

### New Functions

```rust
// In core/src/graph/metrics.rs

/// Compute the set of objects not reachable from any GC root.
pub fn find_unreachable_objects(graph: &ObjectGraph) -> UnreachableSet {
    // 1. BFS from all GC root object IDs through graph.objects references
    // 2. Collect all objects NOT visited
    // 3. Group by class, accumulate count + shallow_size
    // ...
}
```

### Integration

- `AnalyzeResponse` gains an optional `unreachable: Option<UnreachableSet>` field.
- Computed only when graph-backed analysis succeeds (not in heuristic fallback).
- Report renderers add an "Unreachable Objects" section when the field is present.

---

## Feature 4: Enhanced Heap Diff

### Strategy

The current `diff_heaps()` compares two `HeapSummary` values from the streaming parser. The enhanced diff compares two `ObjectGraph`s at the class level.

**Key design decision:** Object IDs are **not stable** across dumps — the JVM assigns different IDs each time. Therefore, object-level identity matching (tracking individual objects) is not feasible. Instead, the enhanced diff works at the **class level**: for each class, compare total instance count, total shallow size, and total retained size between the two graphs.

### Algorithm

1. Build `ObjectGraph` + `DominatorTree` for both before and after dumps.
2. For each class present in either graph:
   - Count instances, sum shallow sizes, sum retained sizes in both graphs.
   - Compute deltas (after − before) for each metric.
3. Sort by absolute retained-size delta descending.
4. Identify new classes (present in after, absent in before) and removed classes.

### Changes to Existing Types

```rust
// In core/src/hprof/parser.rs — extend HeapDiff

pub struct HeapDiff {
    pub before: String,
    pub after: String,
    pub delta_bytes: i64,
    pub delta_objects: i64,
    pub changed_classes: Vec<ClassDelta>,
    /// Enhanced class-level diff from object graph analysis.
    /// None when graph-backed analysis is unavailable.
    pub class_diff: Option<Vec<ClassLevelDelta>>,
}

/// Class-level comparison from object graph analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassLevelDelta {
    pub class_name: String,
    pub before_instances: u64,
    pub after_instances: u64,
    pub before_shallow_bytes: u64,
    pub after_shallow_bytes: u64,
    pub before_retained_bytes: u64,
    pub after_retained_bytes: u64,
}
```

### Changes to `diff_heaps()`

The function signature remains the same but the implementation is enhanced:

1. After computing the existing record-level diff, attempt to build object graphs for both paths.
2. If both succeed, compute the class-level diff and attach it to `HeapDiff::class_diff`.
3. If either fails, leave `class_diff` as `None` — the record-level diff still works.

This is a graceful enhancement: the existing diff path is preserved; the new class-level data is additive.

---

## Shared Implementation Concerns

### Memory Implications

Building two `ObjectGraph`s simultaneously for enhanced diff doubles peak memory usage. For large dumps this may be prohibitive. The implementation should:
- Log a warning when diff is requested on files over a size threshold (e.g., 500MB each).
- Document that graph-backed diff requires sufficient memory for both dumps.
- Future work (not this batch): add a streaming class-stat diff mode that avoids full graph construction.

### Test Strategy

Each feature requires:

| Feature | Unit Tests | Integration Tests |
|---|---|---|
| Histogram grouping | Group by class/package/classloader on synthetic graph; empty graph edge case | CLI `analyze --group-by package` on fixture |
| MAT-style suspects | Scoring algorithm correctness; accumulation-point detection; reference chain extraction | CLI `leaks` output includes suspect ranking |
| Unreachable objects | BFS reachability on synthetic graph with known unreachable objects | CLI `analyze` output includes unreachable section |
| Enhanced heap diff | Class-level delta computation on two synthetic graphs | CLI `diff` output includes class-level diff |

### Backward Compatibility

All existing types gain **optional** new fields. No breaking changes to the existing API:
- `AnalyzeResponse` — new optional fields with `skip_serializing_if`
- `HeapDiff` — new optional `class_diff` field
- `LeakInsight` — new optional `shallow_size_bytes` and `suspect_score` fields
- All existing 101 tests must continue to pass unchanged.

### File Ownership (Implementation Agent)

| File | Owner | Change Type |
|---|---|---|
| `core/src/graph/metrics.rs` | Implementation Agent | Add `HistogramResult`, `UnreachableSet`, `build_histogram()`, `find_unreachable_objects()` |
| `core/src/analysis/engine.rs` | Implementation Agent | Refactor `graph_backed_leaks()` for suspect scoring; add `LeakSuspect`; extend `AnalyzeResponse`; enhance `diff_heaps()` |
| `core/src/hprof/parser.rs` | Implementation Agent | Add `ClassLevelDelta`; extend `HeapDiff` |
| `core/src/lib.rs` | Implementation Agent | Re-export new public types |
| `core/src/config.rs` | Implementation Agent | Add `accumulation_threshold` to `AnalysisConfig` |
| `cli/src/main.rs` | Implementation Agent | Add `--group-by` flag |

### Dependency Order

1. **Histogram grouping** — independent, can start immediately
2. **Unreachable objects** — independent, can start immediately
3. **MAT-style leak suspects** — independent, can start immediately
4. **Enhanced heap diff** — independent, can start immediately

All four features are independent of each other and can be implemented in any order. However, the recommended order is: histogram grouping → unreachable objects → MAT-style suspects → enhanced diff, because each subsequent feature builds confidence with the `ObjectGraph` traversal patterns used by the next.

---

## Risks and Open Questions

| Risk | Impact | Mitigation |
|---|---|---|
| Double memory for enhanced diff | High for large dumps | Log warning; document requirement; future streaming fallback |
| Accumulation-point threshold tuning | Medium | Default 10.0 is conservative; make configurable via `AnalysisConfig` |
| Class name resolution failures | Low | Fall back to `<unknown>` with class ID for display |
| Package extraction edge cases | Low | Handle default package, array types, inner classes |

### Resolved Questions
- **Should unreachable use dominator tree or BFS?** → BFS from GC roots. The dominator tree only covers dominated objects; BFS catches all reachable objects including those reachable through non-dominating paths.
- **Should enhanced diff match objects by ID?** → No. Object IDs are not stable across dumps. Class-level aggregation is the right granularity.
- **Where do new types live?** → Graph-related types in `metrics.rs`, analysis types in `engine.rs`, parser types in `parser.rs`. Follows existing module structure.
