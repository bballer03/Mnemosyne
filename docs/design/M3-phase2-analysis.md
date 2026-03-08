# M3 Phase 2+ Analysis Architecture Design

> **Status:** Design Complete, Implementation Pending
> **Owner:** Design Consulting Agent
> **Created:** 2026-03-08
> **Last Updated:** 2026-03-08
> **Milestone:** M3 Phase 2 — Advanced Heap Analysis

---

## 1. Overview

This document defines the architecture for Mnemosyne's M3 Phase 2+ analysis capabilities: Thread Inspection, ClassLoader Analysis, Collection Inspection, String Analysis, Top-N Largest Instances, and an OQL Query Engine. These features close the remaining MAT-parity gaps and position Mnemosyne as a credible alternative to Eclipse MAT for production heap analysis.

New analyzers consume the `ObjectGraph` model from `core::hprof::object_graph` and the `DominatorTree` from `core::graph::dominator`. **However, a foundational prerequisite (Phase 2a) must extend the binary parser and `HeapObject` struct before most analyzers can operate** — instance field data and primitive array content are currently discarded after reference extraction. See Section 12 and Section 13 for details.

---

## 2. Architecture Diagram

```text
┌─────────────────────────────────────────────────────────┐
│                     CLI / MCP Interface                   │
├─────────────────────────────────────────────────────────┤
│                   Analysis Orchestrator                   │
│              (core::analysis::engine)                     │
├───────┬──────┬──────┬──────┬──────┬──────┬──────────────┤
│Thread │Class │Coll. │String│Top-N │ OQL  │Leak  │ Histogram    │
│Inspect│Loader│Insp. │Anal. │Inst. │Engine│Detect│ /Unreachable │
├───────┴──────┴──────┴──────┴──────┴──────┴──────────────┤
│                   Shared Analysis Core                    │
│           ObjectGraph + DominatorTree + GcPath            │
├─────────────────────────────────────────────────────────┤
│                  HPROF Parsing Layer                      │
│         binary_parser → ObjectGraph + parser              │
└─────────────────────────────────────────────────────────┘
```

Each analyzer is a standalone module that:
1. Takes `&ObjectGraph` and optionally `&DominatorTree` as input
2. Produces a typed result struct
3. Is invoked by the analysis orchestrator (`engine.rs`)
4. Has no direct dependency on other analyzers

---

## 3. Module Layout

```text
core/src/
├── analysis/
│   ├── mod.rs                   # Re-exports
│   ├── engine.rs                # Orchestrator (existing)
│   ├── ai.rs                    # AI insights (existing)
│   ├── thread.rs                # NEW — Thread inspection
│   ├── classloader.rs           # NEW — ClassLoader analysis
│   ├── collection.rs            # NEW — Collection inspection
│   ├── string_analysis.rs       # NEW — String deduplication/waste
│   └── top_instances.rs         # NEW — Top-N largest instances
├── query/                        # NEW module
│   ├── mod.rs                   # Re-exports
│   ├── parser.rs                # OQL query parser
│   ├── executor.rs              # OQL query executor
│   └── types.rs                 # OQL AST and result types
```

All new files live under existing module directories. The only new subdirectory is `query/` for the OQL engine, which is complex enough to warrant its own module group.

---

## 4. Thread Inspection

### 4.1 Data Sources

HPROF records already parsed by `binary_parser`:
- `STACK_TRACE` (tag `0x05`) — maps trace serial → thread serial + frame IDs
- `STACK_FRAME` (tag `0x04`) — maps frame ID → method name, source file, line
- Thread objects in the heap (instances of `java.lang.Thread`)
- `ROOT_THREAD_OBJECT` GC roots — link thread object IDs to stack trace serials

### 4.2 Required ObjectGraph Extensions

The binary parser currently **skips** `STACK_TRACE` (tag 0x05) and `STACK_FRAME` (tag 0x04) records — they fall through to the `_ => skip_bytes(reader, length)` branch in the top-level record loop. Phase 2a must add parsing handlers for these tags. New `ObjectGraph` fields needed:

```rust
// In ObjectGraph:
pub stack_traces: HashMap<u32, StackTrace>,    // serial → trace
pub stack_frames: HashMap<ObjectId, StackFrame>, // frame ID → frame

pub struct StackTrace {
    pub serial: u32,
    pub thread_serial: u32,
    pub frame_ids: Vec<ObjectId>,
}

pub struct StackFrame {
    pub method_name: String,
    pub class_name: String,
    pub source_file: Option<String>,
    pub line_number: i32,  // -1 = unknown, -2 = compiled, 0+ = actual
}
```

### 4.3 Analysis API

```rust
// core::analysis::thread

pub struct ThreadInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub daemon: bool,
    pub state: String,
    pub stack_trace: Option<Vec<StackFrame>>,
    pub retained_bytes: u64,
    pub thread_local_count: usize,
    pub thread_local_bytes: u64,
}

pub struct ThreadReport {
    pub threads: Vec<ThreadInfo>,
    pub total_thread_retained: u64,
    pub top_retainers: Vec<ThreadInfo>,  // sorted by retained_bytes desc
}

pub fn inspect_threads(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
    top_n: usize,
) -> ThreadReport;
```

### 4.4 Implementation Notes
- Thread objects are identified by class name `java.lang.Thread` or subclasses
- Thread names are extracted from the `name` field (char array or String reference)
- Thread-local retention is computed by walking objects dominated by the thread object
- Stack traces are correlated via `ROOT_THREAD_OBJECT` → stack trace serial mapping

---

## 4a. Top-N Largest Instances

### 4a.1 Purpose

Answers the question: *"Which single object is eating 2 GB?"* This is a high-value, low-effort triage feature that complements per-class histograms (which show aggregate size) by drilling into individual instance sizes.

### 4a.2 Data Sources
- `ObjectGraph::objects` — all heap objects with `shallow_size`
- `DominatorTree` — retained sizes per object (when available)
- `ObjectGraph::classes` — for resolving class names

### 4a.3 Analysis API

```rust
// core::analysis::top_instances

pub struct LargestInstance {
    pub object_id: ObjectId,
    pub class_name: String,
    pub shallow_size: u64,
    pub retained_size: u64,
}

pub struct TopInstancesReport {
    pub global_top: Vec<LargestInstance>,      // top-N across all classes
    pub per_class_top: HashMap<String, Vec<LargestInstance>>,  // top-N per class
}

pub fn find_top_instances(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
    global_top_n: usize,
    per_class_top_n: usize,
) -> TopInstancesReport;
```

### 4a.4 Implementation Notes
- Global top-N: iterate all objects, maintain a min-heap of size N sorted by retained_size descending. O(n log N) time.
- Per-class top-N: group objects by `class_id`, apply the same min-heap within each group.
- **Does NOT require field extraction** — uses only `shallow_size` from `HeapObject` and `retained_size` from `DominatorTree`. Can be implemented independently of Phase 2a.
- CLI integration: `mnemosyne analyze heap.hprof --top-instances --top-n 20`
- AnalyzeResponse extension: `pub top_instances: Option<TopInstancesReport>`

---

## 5. ClassLoader Analysis

### 5.1 Data Sources
- Class objects in `ObjectGraph::classes` contain `class_loader_id` (currently parsed but not always populated)
- ClassLoader hierarchy is reconstructable from instance references
- The `--group-by classloader` histogram already exists in `core::graph::metrics`

### 5.2 Required ObjectGraph Extensions

```rust
// In ClassInfo (extend existing):
pub class_loader_id: Option<ObjectId>,  // May already be partially populated
```

### 5.3 Analysis API

```rust
// core::analysis::classloader

pub struct ClassLoaderInfo {
    pub object_id: ObjectId,
    pub class_name: String,          // e.g., "sun.misc.Launcher$AppClassLoader"
    pub loaded_class_count: usize,
    pub total_shallow_bytes: u64,
    pub total_retained_bytes: u64,
    pub parent_loader: Option<ObjectId>,
    pub duplicate_classes: Vec<String>,  // classes also loaded by another loader
}

pub struct ClassLoaderReport {
    pub loaders: Vec<ClassLoaderInfo>,
    pub potential_leaks: Vec<ClassLoaderLeakCandidate>,
    pub duplicate_class_count: usize,
}

pub struct ClassLoaderLeakCandidate {
    pub loader: ClassLoaderInfo,
    pub reason: String,  // e.g., "Retains 45MB but loads only 3 classes"
}

pub fn analyze_classloaders(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
) -> ClassLoaderReport;
```

### 5.4 Implementation Notes
- ClassLoader leaks are common in application servers (Tomcat, WildFly) where redeployed apps fail to release their classloaders
- A classloader is considered a leak candidate when its retained size is disproportionate to its loaded class count
- Duplicate class detection: same fully qualified class name loaded by different classloaders

---

## 6. Collection Inspection

### 6.1 Target Collections

| Collection | Key Fields | Capacity Source |
|---|---|---|
| `java.util.HashMap` | `table` (Node[]), `size` | `table.length` |
| `java.util.ArrayList` | `elementData` (Object[]), `size` | `elementData.length` |
| `java.util.HashSet` | delegates to internal HashMap | Same as HashMap |
| `java.util.LinkedList` | `size` | N/A (no capacity) |
| `java.util.concurrent.ConcurrentHashMap` | `table`, `baseCount` | `table.length` |
| `java.util.TreeMap` | `size` | N/A (tree structure) |

### 6.2 Analysis API

```rust
// core::analysis::collection

pub struct CollectionInfo {
    pub object_id: ObjectId,
    pub collection_type: String,
    pub owner_class: String,        // class containing this collection
    pub size: usize,                // actual element count
    pub capacity: Option<usize>,    // backing array length
    pub fill_ratio: Option<f64>,    // size / capacity
    pub shallow_bytes: u64,
    pub retained_bytes: u64,
    pub waste_bytes: u64,           // estimated wasted capacity
}

pub struct CollectionReport {
    pub collections: Vec<CollectionInfo>,
    pub total_waste_bytes: u64,
    pub empty_collections: usize,
    pub oversized_collections: Vec<CollectionInfo>,  // fill_ratio < 0.25
    pub summary_by_type: HashMap<String, CollectionTypeSummary>,
}

pub struct CollectionTypeSummary {
    pub count: usize,
    pub total_shallow: u64,
    pub total_retained: u64,
    pub total_waste: u64,
    pub avg_fill_ratio: f64,
}

pub fn inspect_collections(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
    min_capacity: usize,       // skip tiny collections
) -> CollectionReport;
```

### 6.3 Implementation Notes
- **`HeapObject` does NOT currently have a `field_data` field.** The binary parser reads instance field bytes into a temporary buffer (see `parse_instance_dump()` in `binary_parser.rs`), extracts only reference-type field values into `references: Vec<ObjectId>`, and discards the raw bytes. Phase 2a must add a `field_data: Vec<u8>` field to `HeapObject` and retain the raw instance bytes so that collection, string, and thread analyzers can extract typed field values.
- Array lengths come from primitive/object array HPROF records (already parsed)
- **Primitive array data is also discarded** — `parse_prim_array_dump()` calls `skip_bytes()` over element data. String analysis requires reading `char[]`/`byte[]` content. Phase 2a must selectively retain primitive array data (at minimum for byte/char arrays).
- Waste calculation: `(capacity - size) * element_size` for array-backed collections
- Fill ratio thresholds: empty (0.0), sparse (<0.25), normal (0.25-0.75), dense (>0.75)

---

## 7. String Analysis

### 7.1 Data Sources
- `java.lang.String` instances in ObjectGraph
- String `value` field → underlying `char[]` or `byte[]` (Java 9+ compact strings)
- `ObjectGraph::strings` already has the HPROF string table, but these are class/field names, not heap string values

### 7.2 Analysis API

```rust
// core::analysis::string_analysis

pub struct StringInfo {
    pub object_id: ObjectId,
    pub value: String,
    pub byte_length: u64,
    pub retained_bytes: u64,
    pub referrer_count: usize,
}

pub struct DuplicateStringGroup {
    pub value: String,
    pub instances: Vec<ObjectId>,
    pub count: usize,
    pub total_wasted_bytes: u64,  // (count - 1) * byte_length
}

pub struct StringReport {
    pub total_strings: usize,
    pub total_string_bytes: u64,
    pub unique_strings: usize,
    pub duplicate_groups: Vec<DuplicateStringGroup>,  // sorted by waste desc
    pub total_duplicate_waste: u64,
    pub top_strings_by_retention: Vec<StringInfo>,
    pub charset_breakdown: CharsetBreakdown,
}

pub struct CharsetBreakdown {
    pub latin1_count: usize,   // Java 9+ compact strings (1 byte/char)
    pub utf16_count: usize,    // 2 bytes/char
}

pub fn analyze_strings(
    graph: &ObjectGraph,
    dominator: &DominatorTree,
    top_n: usize,
    min_duplicate_count: usize,
) -> StringReport;
```

### 7.3 Implementation Notes
- String value extraction requires reading the `value` field reference → char[]/byte[] content
- **Prerequisite:** Phase 2a must retain both (a) instance `field_data` on `HeapObject` (to read the `value` field reference and `coder` field) and (b) primitive array element data (to read the actual char[]/byte[] backing content). Both are currently discarded by the parser.
- Java 9+ uses compact strings: Latin-1 (1 byte/char) by default, UTF-16 when needed
- Java 8 always uses char[] (2 bytes/char)
- The `coder` field (Java 9+) indicates encoding: 0 = Latin-1, 1 = UTF-16
- Deduplication analysis: hash string values and group — O(n) with HashMap
- This analyzer is the most memory-intensive since it materializes string values

---

## 8. OQL Query Engine

### 8.1 Query Language

Mnemosyne OQL is a simplified subset of Eclipse MAT's OQL, designed for ad-hoc heap exploration.

#### Grammar (EBNF-style)
```
query       := select_clause from_clause [where_clause] [limit_clause]
select_clause := "SELECT" (field_list | "*")
from_clause := "FROM" class_pattern
where_clause := "WHERE" condition
limit_clause := "LIMIT" integer

field_list  := field ("," field)*
field       := "@" builtin_field | field_name
builtin_field := "objectId" | "className" | "shallowSize" | "retainedSize" | "objectAddress"

class_pattern := quoted_string | class_name_glob
condition   := comparison (("AND" | "OR") comparison)*
comparison  := field operator value
operator    := "=" | "!=" | ">" | "<" | ">=" | "<=" | "LIKE" | "INSTANCEOF"
value       := integer | float | quoted_string | "null" | "true" | "false"
```

#### Example Queries
```sql
-- Find all large HashMaps
SELECT @objectId, @shallowSize, @retainedSize
FROM "java.util.HashMap"
WHERE @retainedSize > 1048576

-- Find strings containing "password"
SELECT @objectId, @className
FROM "java.lang.String"
WHERE @toString LIKE "%password%"

-- Find all instances of a class hierarchy
SELECT *
FROM INSTANCEOF "java.util.AbstractMap"
WHERE @shallowSize > 1024
LIMIT 100

-- Find objects retained by a specific class
SELECT @objectId, @className, @retainedSize
FROM "com.example.*"
WHERE @retainedSize > 0
```

### 8.2 Module Design

```text
core/src/query/
├── mod.rs        # Public API: parse_query() + execute_query()
├── types.rs      # AST types: Query, SelectClause, FromClause, WhereClause, etc.
├── parser.rs     # Recursive descent parser: &str → Query AST
└── executor.rs   # Query executor: (Query, &ObjectGraph, &DominatorTree) → QueryResult
```

### 8.3 AST Types

```rust
// core::query::types

pub struct Query {
    pub select: SelectClause,
    pub from: FromClause,
    pub filter: Option<WhereClause>,
    pub limit: Option<usize>,
}

pub enum SelectClause {
    All,
    Fields(Vec<FieldRef>),
}

pub enum FieldRef {
    BuiltIn(BuiltInField),
    InstanceField(String),
}

pub enum BuiltInField {
    ObjectId,
    ClassName,
    ShallowSize,
    RetainedSize,
    ObjectAddress,
    ToString,
}

pub struct FromClause {
    pub class_pattern: ClassPattern,
    pub instanceof: bool,
}

pub enum ClassPattern {
    Exact(String),
    Glob(String),  // supports * wildcard
}

pub struct WhereClause {
    pub conditions: Vec<Condition>,
    pub operators: Vec<LogicalOp>,
}

pub enum LogicalOp { And, Or }

pub struct Condition {
    pub field: FieldRef,
    pub op: ComparisonOp,
    pub value: Value,
}

pub enum ComparisonOp { Eq, Ne, Gt, Lt, Ge, Le, Like }
pub enum Value { Int(i64), Float(f64), Str(String), Null, Bool(bool) }

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub total_matched: usize,
    pub truncated: bool,
}

pub enum CellValue {
    Id(ObjectId),
    Str(String),
    Int(i64),
    Float(f64),
    Null,
}
```

### 8.4 Execution Strategy

1. **FROM resolution**: Scan `ObjectGraph::classes` for matching class patterns → collect matching `ObjectId` sets
2. **INSTANCEOF resolution**: Walk class hierarchy (super_class chain) to find all subclass instances
3. **WHERE filtering**: Evaluate conditions against each object. Built-in fields are resolved directly from `ObjectGraph`/`DominatorTree`; instance fields require field extraction
4. **SELECT projection**: Extract requested fields for matching objects
5. **LIMIT**: Truncate result set

Performance: Full table scan over `ObjectGraph::objects`. For a 156 MB dump with ~300k objects, expect sub-second query times. Larger dumps may need index support in the future.

### 8.5 Integration Points

```rust
// core::query::mod.rs

pub fn parse_query(input: &str) -> Result<Query, QueryParseError>;

pub fn execute_query(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> CoreResult<QueryResult>;
```

- CLI integration: `mnemosyne query heap.hprof "SELECT * FROM ..."` subcommand
- MCP integration: `execute_query` JSON-RPC handler
- REPL mode (future): interactive query session over a loaded heap

---

## 9. Analysis Pipeline Flow

```text
                        ┌──────────────┐
                        │  HPROF File  │
                        └──────┬───────┘
                               │
                    ┌──────────▼──────────┐
                    │   binary_parser     │
                    │  parse_hprof_file() │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    ObjectGraph       │
                    │  (shared, immutable) │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  build_dominator_   │
                    │  tree()             │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
    ┌─────────▼────────┐ ┌────▼─────┐ ┌───────▼────────┐
    │  Existing         │ │  New     │ │  OQL Engine    │
    │  Analyzers        │ │  Analyzers│ │  (on-demand)   │
    │  - detect_leaks() │ │  - thread │ │  parse_query() │
    │  - analyze_heap() │ │  - class  │ │  execute_query()│
    │  - diff_heaps()   │ │    loader │ │                │
    │  - histogram      │ │  - coll.  │ │                │
    │  - unreachable    │ │  - string │ │                │
    └──────────────────┘ └──────────┘ └────────────────┘
              │                │                │
              └────────────────┼────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │   AnalyzeResponse   │
                    │  (extended with new │
                    │   optional fields)  │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  CLI / MCP / Report │
                    └─────────────────────┘
```

Key design decisions:
- **ObjectGraph is shared and immutable** after construction — all analyzers take `&ObjectGraph`
- **DominatorTree is optional** — analyzers that need retained sizes take `Option<&DominatorTree>`
- **New analyzers are opt-in** — enabled via CLI flags or config, not run by default
- **Results extend AnalyzeResponse** with optional fields (same pattern as histogram/unreachable)
- **OQL is separate** — invoked on-demand, not part of the default analyze pipeline

---

## 10. AnalyzeResponse Extensions

```rust
// Extend existing AnalyzeResponse with optional fields:

pub struct AnalyzeResponse {
    // ... existing fields ...
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_report: Option<ThreadReport>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classloader_report: Option<ClassLoaderReport>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_report: Option<CollectionReport>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_report: Option<StringReport>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_instances: Option<TopInstancesReport>,
}
```

This follows the same pattern used for `histogram` and `unreachable_objects` — backward-compatible optional extensions.

---

## 11. CLI Integration

```text
# Thread inspection
mnemosyne analyze heap.hprof --threads
mnemosyne analyze heap.hprof --threads --top-n 10

# ClassLoader analysis
mnemosyne analyze heap.hprof --classloaders

# Collection inspection
mnemosyne analyze heap.hprof --collections --min-capacity 16

# String analysis
mnemosyne analyze heap.hprof --strings --top-n 50

# OQL query (new subcommand)
mnemosyne query heap.hprof "SELECT @objectId, @retainedSize FROM \"java.util.HashMap\" WHERE @retainedSize > 1048576"

# Top-N largest instances
mnemosyne analyze heap.hprof --top-instances
mnemosyne analyze heap.hprof --top-instances --top-n 20

# Combined analysis
mnemosyne analyze heap.hprof --threads --collections --strings --top-instances
```

---

## 12. Implementation Sequence

### Phase 2a — Foundation (prerequisite)
The binary parser currently discards all non-reference data during instance parsing and skips primitive array content entirely. Before any new analyzer can read field values, the following foundational work is required:

1. **Add `field_data: Vec<u8>` to `HeapObject`** — Retain raw instance field bytes during `parse_instance_dump()`. Currently the local `data` buffer is discarded after reference extraction. Default to empty `Vec` for arrays and objects where field data is not applicable.
2. **Retain primitive array element data** — Add a `data: Option<Vec<u8>>` or similar field to `HeapObject` for `PrimitiveArray` variants. At minimum, retain `byte[]` (type 8) and `char[]` (type 5) arrays, which are needed for string analysis. Consider opt-in retention via a parser config flag to manage memory impact.
3. **Parse STACK_TRACE (0x05) and STACK_FRAME (0x04) top-level records** — These are currently skipped by the `_ => skip_bytes()` fallback in the top-level record loop. Add handlers that populate new `ObjectGraph` fields: `stack_traces: HashMap<u32, StackTrace>` and `stack_frames: HashMap<u64, StackFrame>`.
4. **Implement the typed field extraction API** — `FieldValue` enum + `read_field()` / `read_all_fields()` functions that interpret `HeapObject::field_data` bytes using `ClassInfo::instance_fields` layout. This is the shared prerequisite for collection, string, and thread analyzers.
5. **Memory impact assessment** — Measure RSS delta on the 156 MB test fixture with field_data retention enabled vs. disabled. If the RSS:dump ratio exceeds 5x, implement opt-in retention or selective storage for target classes only.

### Phase 2b — Individual Analyzers (parallelizable after 2a)
2. **String Analysis** — Highest value-to-effort ratio. String waste is common and easy to detect.
3. **Collection Inspection** — Second highest impact. Collection waste is the #1 finding in most MAT sessions.
4. **Thread Inspection** — Requires stack trace storage extension in ObjectGraph.
5. **Top-N Largest Instances** — Per-class top-N by retained size (see Section 4a).
6. **ClassLoader Analysis** — Niche but critical for app-server environments. (May be deferred to Phase 3 per roadmap Step 12.)

### Phase 2c — Query Engine
6. **OQL Parser** — Recursive descent, well-defined grammar.
7. **OQL Executor** — Object scan with filter evaluation.
8. **CLI + MCP integration** — `mnemosyne query` subcommand + `execute_query` RPC handler.

### Phase 2d — Polish
9. **Report rendering** — All new analyzer results rendered in 5 output formats.
10. **CLI table formatting** — Consistent with existing comfy-table patterns.
11. **Documentation** — User-facing docs for each new subcommand/flag.

---

## 13. Dependencies

| Analyzer | Requires ObjectGraph | Requires DominatorTree | Requires Field Extraction | Requires ObjectGraph Extension |
|---|---|---|---|---|
| Thread Inspection | ✅ | ✅ (for retained sizes) | ✅ (thread name) | ✅ (stack traces) |
| **Top-N Largest Instances** | **✅** | **✅** | **❌** | **❌** |
| ClassLoader Analysis | ✅ | ✅ | ❌ | ❌ (class_loader_id exists) |
| Collection Inspection | ✅ | ✅ | ✅ (size, table fields) | ❌ |
| String Analysis | ✅ | ✅ | ✅ (value field) | ❌ |
| OQL Engine | ✅ | Optional | ✅ (instance fields) | ❌ |

### Critical Prerequisite: Field Extraction API

The biggest shared dependency is a **typed field extraction API** for `HeapObject`. Currently `HeapObject` does **not** store field data at all — the binary parser reads instance bytes into a temporary buffer, extracts reference-type fields, and discards the raw data. Phase 2a must first add `field_data: Vec<u8>` storage, then build the typed accessor API on top of it:

```rust
// In core::hprof::object_graph or a new field_reader module

pub enum FieldValue {
    Boolean(bool),
    Byte(i8),
    Char(char),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ObjectRef(Option<ObjectId>),
}

pub fn read_field(
    object: &HeapObject,
    class: &ClassInfo,
    field_name: &str,
    id_size: usize,
) -> Option<FieldValue>;

pub fn read_all_fields(
    object: &HeapObject,
    class: &ClassInfo,
    id_size: usize,
) -> Vec<(String, FieldValue)>;
```

This is the **gating prerequisite** for Phase 2b/2c work.

---

## 14. Risk Analysis

| Risk | Impact | Mitigation |
|---|---|---|
| Field extraction complexity | High — many analyzers blocked | Implement as Phase 2a prerequisite |
| **`field_data` retention increases RSS** | **Resolved in Step 11 (partial)** — unconditional retention did raise the 156 MB fixture from 3.56x to 4.78x RSS:dump. The current implementation now uses `ParseOptions { retain_field_data: false }` by default and only opts into field retention for thread, string, and collection analyzers, bringing default `analyze`/`leaks` runs down to 4.23x. | **Keep validating at larger tiers.** Multi-tier Step 11 runs at 500 MB / 1 GB / 2 GB are still pending; if larger dumps regress further, evaluate streaming overview mode or disk-backed storage. |
| **Primitive array data retention** | **Reduced by Step 11 remediation** — `byte[]` and `char[]` payloads are now only retained when field-data retention is explicitly enabled, so default `leaks` and default `analyze` runs no longer pay that overhead. | Keep the existing array-size cap, keep retention opt-in, and re-measure investigation-heavy runs during multi-tier validation. |
| Memory overhead from string materialization | Medium — string analysis reads all string values | Stream and hash without full retention |
| OQL injection/abuse | Low — local tool, not a web service | Validate query structure, enforce LIMIT |
| HPROF version differences (Java 8 vs 17) | Medium — field layouts differ | Test with both Java 8 and 17+ dumps |
| Performance on large dumps | Medium — collection/string scans are O(n) | Add progress indicators, enforce limits |

---

## 15. Non-Goals (Explicit Non-Scope)

- **No AI integration** in Phase 2 — AI wiring is M5
- **No persistence/caching** — all analysis is in-memory per session
- **No REPL mode** for OQL yet — CLI one-shot queries first
- **No web UI** — that is M4
- **No JVM agent** — that is M4+