# Milestone 7-4 — OQL Targeted Expansion

> **Status:** 🔲 Pending — design entering review. Predecessors M7-1, M7-2, M7-3 ✅ complete.
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Parent:** [milestone-7-production-readiness.md §9](milestone-7-production-readiness.md)
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Last updated:** 2026-04-26

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Slice | M7-4 |
| Phase | M7 — Production Readiness & Scale |
| Type | Parity (targeted) |
| Predecessors | M7-1 ✅ (overview mode), M7-2 ✅ (`ci-check`), M7-3 ✅ (flame graphs) |
| Successors | M7-5 (comparative benchmarks), M7-6 (v0.3.0 release) |
| Touched crates | `core` only (no CLI flag changes; existing `query` CLI / MCP surface absorbs the new syntax transparently) |

## 2. Objective

After M7-4, a Mnemosyne user can express a small but high-value set of MAT-flavored OQL queries that are routinely used during real heap triage:

- "Find every cache instance whose retained size exceeds 100 MB" — `WHERE @retainedSize > N`.
- "Find every `String` whose contents contain `password`" — `WHERE @toString LIKE '%password%'`.
- "Walk one hop from each large `HashMap` to its `table` array" — `SELECT OBJECTS table FROM ...`.
- "Find leak-suspect objects whose GC-root path goes through a thread-local" — `WHERE @gcRootPath CONTAINS 'ThreadLocal'`.
- "Find non-empty cache entries that still have a null value reference" — `WHERE entries > 0 AND value IS NULL`.

These five workflows account for most of the everyday MAT OQL queries we see in the wild. M7-4 closes that gap **without** committing to MAT's full grammar (which is M8-2's problem).

## 3. Context

### 3.1 What MAT OQL supports today (the universe)

MAT's OQL supports `SELECT`, `FROM`, `WHERE`, `INSTANCEOF`, sub-queries (`(SELECT ...)`), `OBJECTS`/`AS RETAINED SET`, multi-hop field navigation (`x.y.z`), arithmetic, regex `LIKE` (Java `Pattern`), built-in attributes (`@retainedHeapSize`, `@objectAddress`, `@displayName`, `@GCRootInfo`, …), pseudo-functions (`toString(x)`, `dominators(x)`, `outbounds(x)`, `inbounds(x)`), and a handful of set operators. It is large and treadmill-prone.

### 3.2 What Mnemosyne supports today (the floor)

Inspected files: [core/src/query/mod.rs](../../core/src/query/mod.rs), [core/src/query/types.rs](../../core/src/query/types.rs), [core/src/query/parser.rs](../../core/src/query/parser.rs), [core/src/query/executor.rs](../../core/src/query/executor.rs), [core/tests/query_parser.rs](../../core/tests/query_parser.rs), [core/tests/query_executor.rs](../../core/tests/query_executor.rs).

Concretely, today the engine supports:

- **Grammar:** `SELECT (* | field, field, …) FROM [INSTANCEOF] "<class-pattern>" [WHERE <cond> ((AND|OR) <cond>)*] [LIMIT n]`. No grouping parens, no precedence — `AND`/`OR` are evaluated left-to-right (executor.rs `matches_filter`).
- **Class pattern:** `Exact("a.b.C")` or `Glob("a.b.*")`. Glob supports a single `*` only (`glob_match` uses `split_once('*')`).
- **Built-in fields (`BuiltInField`):** `@objectId`, `@className`, `@shallowSize`, `@retainedSize`, `@objectAddress`, `@toString`. `@retainedSize` returns `0` when no dominator tree is supplied (silent in overview mode). `@toString` currently returns the class name as a placeholder — **not** the real `String` contents.
- **Instance fields:** parsed as `FieldRef::InstanceField(String)`, resolved through `hprof::read_field`. Single-name only — no dotted multi-hop traversal.
- **Operators:** `=`, `!=`, `>`, `<`, `>=`, `<=`, `LIKE`, `INSTANCEOF`. `LIKE` already exists for strings and is implemented as `glob_match(pattern.replace('%', "*"), value)` — supports a single `%` per pattern and is **not** a regex.
- **Values:** `Value::Int(i64)`, `Value::Str`, `Value::Null`, `Value::Bool`. The lexer accepts `null`, `true`, `false`, integer literals, and double-quoted strings.
- **Null comparison:** `field = null` works; `field != null` does **not** (the `(CellValue::Id, Value::Null)` case falls through to `_ => false`).
- **Test coverage:** five executor tests in [core/tests/query_executor.rs](../../core/tests/query_executor.rs) (built-in fields, `LIMIT`, `INSTANCEOF` from-clause, instance-field projection+filter, `INSTANCEOF` filter on instance field) plus parser tests for class patterns, globs, `LIKE`, `LIMIT`.

### 3.3 The gap M7-4 closes

The MAT-style queries listed in §2 either don't parse, parse but produce wrong/empty results, or have placeholder semantics:

| MAT-style query | Today's behavior | Gap |
|---|---|---|
| `WHERE @retainedSize > N` | parses; works in deep mode; silently `> 0 = false` everywhere in overview mode | mode-honesty |
| `WHERE @toString LIKE '%pwd%'` | parses; matches against class name, never the actual `String` content | placeholder semantics |
| `SELECT OBJECTS x.field FROM …` | parse error on `OBJECTS` | unsupported |
| `WHERE @gcRootPath CONTAINS 'X'` | parse error on `@gcRootPath` and on `CONTAINS` | unsupported |
| `WHERE x.field IS NULL` / `IS NOT NULL` | parse error on `IS` | unsupported (`= null` works partially; `!= null` is broken) |

M7-4 closes exactly this gap and nothing more.

## 4. Scope

In-scope for M7-4:

1. **Real `@toString` semantics** for `java.lang.String` (decode the underlying `value` field's `char[]` / `byte[]`); honest fallback (`ClassName@<id-hex>`) for arbitrary classes; `null` for `null` references.
2. **`@gcRootPath` pseudo-attribute** — a synthetic string projection of the shortest GC-root path for an object (`"<root-kind>;a.b.C;a.b.D;target"`).
3. **`OBJECTS x.field` projection** — `SELECT OBJECTS field` projects the **referent** of `field` instead of the matched object itself; one hop only.
4. **`CONTAINS` operator** for substring match against `CellValue::Str` columns (used primarily with `@gcRootPath` but defined generically for any string column).
5. **`IS NULL` / `IS NOT NULL`** sugar for object-ref instance fields (and consistent fix to `field != null` so it returns `true` when the field is set).
6. **Mode-aware `@retainedSize`**: in overview mode, queries that read or filter on `@retainedSize` (or `@gcRootPath`, which depends on the dominator-driven seed logic) return a partial-result envelope with `feature_unavailable_in_overview_mode` instead of silently treating retained size as `0`.

Predicate count: **five** cleanly numbered features (1+2 are tightly coupled and ship together as the pseudo-attribute infrastructure slice; 3, 4, 5 are independent).

AST changes: new `BuiltInField::GcRootPath`, new `ComparisonOp::Contains`, new `SelectClause::Objects(FieldRef)`, new `Value::IsNull` / op-shaped `IsNull` / `IsNotNull` (see §8). Lexer changes: recognize `OBJECTS`, `CONTAINS`, `IS`, `NOT`, and `@gcRootPath`. Executor changes: see §9.

## 5. Non-scope

Explicitly **not** in M7-4 (deferred to M8-2 or a future milestone):

- Full MAT OQL parity (sub-queries, `dominators(x)`, `outbounds(x)`, `inbounds(x)`, `AS RETAINED SET`, set arithmetic).
- Multi-hop field navigation (`x.y.z`); only single-hop `OBJECTS x.field` is in scope.
- New joins or cross-product `FROM` clauses.
- Custom user-defined function definitions.
- Regex `LIKE` (we keep glob-style `%`/`*`; regex is M8-2).
- Operator precedence / parentheses in `WHERE` (still left-to-right). If a query needs precedence, it is rewritten by hand.
- Streaming OQL execution. Executor remains an in-memory linear scan.
- Any change to overview-mode bounded-memory invariants.

## 6. Pre-design current-state summary

A concrete map of what already exists, with file references, so the implementation slice can land surgical edits.

### 6.1 Module shape — [core/src/query/mod.rs](../../core/src/query/mod.rs)

```rust
mod executor; mod parser; mod types;
pub use executor::execute_query;
pub use parser::parse_query;
pub use types::{
    BuiltInField, CellValue, ClassPattern, ComparisonOp, Condition, FieldRef, FromClause,
    LogicalOp, Query, QueryParseError, QueryResult, SelectClause, Value, WhereClause,
};
```

Three internal files, one public surface — `parse_query` and `execute_query` plus the AST/value types.

### 6.2 AST — [core/src/query/types.rs](../../core/src/query/types.rs)

- `Query { select: SelectClause, from: FromClause, filter: Option<WhereClause>, limit: Option<usize> }`.
- `SelectClause::All` (defaults to `[@objectId, @className]`) and `SelectClause::Fields(Vec<FieldRef>)`.
- `FieldRef::BuiltIn(BuiltInField)` / `FieldRef::InstanceField(String)`.
- `BuiltInField::{ObjectId, ClassName, ShallowSize, RetainedSize, ObjectAddress, ToString}`.
- `ClassPattern::{Exact(String), Glob(String)}` — single-`*` glob only.
- `ComparisonOp::{Eq, Ne, Gt, Lt, Ge, Le, Like, InstanceOf}`.
- `Value::{Int(i64), Str(String), Null, Bool(bool)}`.
- `CellValue::{Id(u64), Str(String), Int(i64), Bool(bool), Null}`.
- `WhereClause` is flat: `conditions: Vec<Condition>` plus parallel `operators: Vec<LogicalOp>` of length `conditions.len() - 1`. Evaluated strictly left-to-right.

### 6.3 Parser — [core/src/query/parser.rs](../../core/src/query/parser.rs)

Hand-written recursive-descent parser, ~330 lines, no separate lexer. Notable hooks the slice will extend:

- `parse_field_ref` — single point where `@<ident>` is mapped to `BuiltInField`. New built-ins (`@gcRootPath`) drop in here.
- `parse_comparison_op` — new ops (`CONTAINS`, `IS NULL`, `IS NOT NULL`) drop in here.
- `parse_select_clause` — currently can't see the `OBJECTS` keyword; needs a new branch before the field-list parse.
- `consume_keyword` is case-insensitive and word-boundary-aware. New keywords slot in for free.

### 6.4 Executor — [core/src/query/executor.rs](../../core/src/query/executor.rs)

Three concentrated extension points:

- `resolve_field_value(field, graph, dominator, object_id) -> CellValue` — the visitor for projections. New built-ins (`@toString` real, `@gcRootPath`) extend the `match` here.
- `compare_values(left, op, right) -> bool` — the operator visitor. New ops (`Contains`) and the missing `(Id, Null)` / `(Str, Null)` arms extend the `match` here.
- `project_row` (top of file) — only needs touching if `SELECT OBJECTS` materializes a different row shape (it does: see §11).

### 6.5 Test coverage — [core/tests/query_executor.rs](../../core/tests/query_executor.rs), [core/tests/query_parser.rs](../../core/tests/query_parser.rs)

Five executor tests, four parser tests. M7-4 will add at minimum 15 new tests (3 per predicate × 5 predicates) plus a regression sweep over the existing 9.

## 7. Predicate catalog (the five chosen)

### 7.1 P1 — Real `@toString` projection / filter

**Syntax (unchanged from today):** `SELECT @toString FROM …` or `WHERE @toString LIKE '%pwd%'`.

**Semantics (new):**
- For an object whose class is `java.lang.String` (or a `java.lang.String` subclass via `INSTANCEOF` chain): decode the `value` field. JDK 9+ stores it as `byte[] value` plus `byte coder` (0 = LATIN1, 1 = UTF16); pre-9 as `char[] value`. The implementation reads `value` via `hprof::read_field`, then dereferences the array referent through `graph.objects` and decodes up to a configurable cap (`STRING_TOSTRING_BYTE_CAP = 4096` bytes) to avoid pathological 1 GB string materialization.
- For any other class: `format!("{}@{:08x}", class_name, object_id)`. This is honest — it tells the user we don't have a `toString` and gives them an addressable identifier.
- For an object whose class is `null` or unresolved: `CellValue::Null`.

**Mode requirement:** works in **deep mode only** (overview mode does not retain field data). In overview mode, returns a partial-result envelope (see §10).

**Execution strategy:** extend `resolve_field_value` `BuiltInField::ToString` arm. Helper `synth_to_string(graph, object_id) -> CellValue` keeps the logic out of the visitor and lets us unit-test it.

**Error cases:** truncated `byte[]`/`char[]`; corrupt UTF-16 (replace with `\u{FFFD}`); `value` field missing → fall through to `ClassName@<id>`.

### 7.2 P2 — `WHERE @retainedSize <op> N` mode-aware

**Syntax (unchanged):** `WHERE @retainedSize > N` (or `>=`, `<`, `<=`, `=`, `!=`).

**Semantics:** unchanged in deep mode (correct today). **Changed in overview mode:** instead of silently returning `0` for every retained size and producing wrong results, the executor short-circuits with a structured error:

```rust
QueryError::FeatureUnavailableInOverviewMode {
    feature: "@retainedSize",
    hint: "re-run with --mode deep or use @shallowSize",
}
```

**Mode requirement:** deep only (when used in `WHERE` or `SELECT`).

**Execution strategy:** at query-execution entry, walk the AST once and collect any `BuiltInField::RetainedSize` / `GcRootPath` references; if `dominator.is_none()` and any are present, return the error before the scan.

**Error cases:** see above. Also: integer overflow on retained size — already saturating in `dom.retained_size`.

### 7.3 P3 — `LIKE` with multi-`%` and `CONTAINS` operator

**Syntax (new):**
- `LIKE` already exists. Slice C generalizes the matcher to support **multiple `%` wildcards** (today: only one) and to anchor: `'%foo%'`, `'foo%'`, `'%foo'`, `'foo'`. No `_` single-char wildcard (deferred). No regex.
- `CONTAINS` is a new operator: `WHERE col CONTAINS 'substr'`. Equivalent to `LIKE '%substr%'` but reads better, and is the natural verb for path-style fields like `@gcRootPath`.

**Semantics:** plain substring match on the string representation of the left-hand `CellValue` (`Str`, or a coerced `Id` formatted as `0x…`). On non-string `CellValue` (e.g. `Int`, `Bool`), `CONTAINS` returns `false`.

**Mode requirement:** both modes (when applied to fields available in that mode).

**Execution strategy:** extend `compare_values` with a `Contains` arm; replace `glob_match` with a multi-`%` variant (split by `%`, each segment must occur in order).

**Error cases:** empty pattern (`CONTAINS ''`) → matches everything (documented; matches MAT's behavior).

### 7.4 P4 — `OBJECTS x.field` single-hop projection

**Syntax (new):** `SELECT OBJECTS field FROM "ClassName" [WHERE …]`. Exactly one identifier after `OBJECTS`; either an instance field or `@objectId` / `@className` (which collapse to no-op projection of the same object). No multi-hop `OBJECTS x.y.z`.

**Semantics:** for each matched object, resolve `field` to a referent `ObjectId` via `read_field`. If the field is an object reference and non-null, emit a row representing the **referent**, not the source. Output columns: `[@objectId, @className]` (the standard `SelectClause::All` shape). Null or non-reference fields are skipped (do not emit a row). Duplicates are de-duplicated (a `HashSet<ObjectId>` accumulator); rows are sorted by `@objectId` like the existing executor.

**Mode requirement:** deep mode (requires field data on the source object). Overview mode: same partial-result envelope as P2.

**Execution strategy:** new AST `SelectClause::Objects(FieldRef)`. Executor branch in `execute_query`: build matched set as today, then for each matched object resolve referent, dedup, project standard `[@objectId, @className]`. `LIMIT` applies to the post-projection deduped set, not to the matched set, to match user intuition.

**Error cases:** `OBJECTS @objectId` is allowed (no-op self-projection). `OBJECTS @retainedSize` (numeric) is a syntax-time error: `"OBJECTS requires an object-reference field"`. Field that's a primitive or a primitive array is a runtime-empty result with a warning logged via `tracing::debug!`.

### 7.5 P5 — `@gcRootPath` field with `CONTAINS` (and `IS NULL` cleanup)

**Syntax (new):**
- New built-in field `@gcRootPath`. Both projectable (`SELECT @gcRootPath`) and filterable (`WHERE @gcRootPath CONTAINS 'ThreadLocal'`).
- New keyword pair `IS NULL` / `IS NOT NULL`. Applies to any field that resolves to `CellValue::Null` / non-`Null`. Maps internally to `Eq Value::Null` / `Ne Value::Null`. Side-effect: fix the `(CellValue::Id, Value::Null)` arm of `compare_values` so `field != null` actually works.

**Semantics for `@gcRootPath`:** for an object `o`, compute the **shortest path from `o` back to a GC root** by reverse-edge BFS, then format as a `;`-separated string of class names: `"<RootKind>;a.b.C;a.b.D;…;<source-class>"`. Reuses the algorithm in [core/src/report/flamegraph/collapse/gc_root_path.rs](../../core/src/report/flamegraph/collapse/gc_root_path.rs) — the slice **lifts the helpers** (`build_reverse_edges`, `build_gc_root_lookup`, `shortest_gc_root_path`) into a shared `core::graph::gc_root_path` module rather than duplicating them. The flamegraph collapser becomes a consumer of the shared helper.

If no GC-root path exists (unreachable object), `@gcRootPath` is `CellValue::Null`. Path frames are capped at `MAX_TOTAL_PATH_FRAMES = 32` (already the flamegraph cap).

**Mode requirement:** deep mode (needs reverse edges + GC-root list, neither of which exists in overview mode). Overview mode: §10 partial-result envelope.

**Execution strategy:** add `BuiltInField::GcRootPath` arm in `resolve_field_value` that calls the lifted helper. **Caveat:** computing the path for every matched object on a 100M-object heap is expensive. The slice memoizes the reverse-edge map and GC-root lookup once per `execute_query` call (built lazily on first `@gcRootPath` reference); subsequent rows pay only BFS cost. See §14 risk #3.

**Error cases:** unreachable object → `Null`. Path exceeds cap → truncated and suffixed with `;…`. Reverse-edge construction failure → query error (treat as a deep-mode invariant violation, same as missing dominator).

## 8. AST extensions

| Change | File | Detail |
|---|---|---|
| `BuiltInField::GcRootPath` | `types.rs` | New variant; serde-renamed `@gcRootPath`. |
| `SelectClause::Objects(FieldRef)` | `types.rs` | New variant; carries the single field to dereference. |
| `ComparisonOp::Contains` | `types.rs` | New variant. |
| `Condition` extension | `types.rs` | Add `Condition::IsNull(FieldRef)` and `Condition::IsNotNull(FieldRef)` as sibling variants of the existing struct; refactor `Condition` from a struct to an enum (`Compare { field, op, value }`, `IsNull(FieldRef)`, `IsNotNull(FieldRef)`). This is internal — `WhereClause` shape unchanged. |
| Lexer keywords | `parser.rs` | Add `OBJECTS`, `CONTAINS`, `IS`, `NOT`. All routed through existing `consume_keyword` (case-insensitive, word-boundary-aware). |
| Lexer pseudo-attribute | `parser.rs` | Map `@gcRootPath` in `parse_field_ref`. |

The `Condition` enum refactor is the only invasive change. It is contained: `WhereClause`, the parser's `parse_condition`, and `executor::evaluate_condition` are the only call sites.

## 9. Executor changes

Concentrated and surgical:

1. **`resolve_field_value`** — extend `match field` with new arms:
   - `BuiltInField::ToString` — real `synth_to_string` helper.
   - `BuiltInField::GcRootPath` — call into shared `core::graph::gc_root_path::path_for`.
2. **`compare_values`** — extend with `ComparisonOp::Contains` (substring match on `Str`, `false` otherwise) and add the missing null arms: `(CellValue::Id, Value::Null)` and `(CellValue::Str, Value::Null)` honor `Eq`/`Ne` symmetrically.
3. **`evaluate_condition`** — switch on `Condition::Compare` (existing path) vs `Condition::IsNull` / `Condition::IsNotNull` (new — both delegate to `resolve_field_value` and check `CellValue::Null`).
4. **`execute_query`** — pre-scan AST once for any deep-mode-only built-ins (`@retainedSize`, `@gcRootPath`); if `dominator.is_none()` and any are present, short-circuit with `QueryError::FeatureUnavailableInOverviewMode`.
5. **`execute_query`** `SelectClause::Objects` branch — after the matched-set scan, walk matched objects, resolve referent, dedup into a `BTreeSet<ObjectId>`, project standard `[@objectId, @className]`. Apply `LIMIT` on the deduped set.
6. **One-time per-query memoization** for `@gcRootPath`: build reverse-edge map and GC-root lookup once on first use, reuse for the rest of the scan.

No changes to the visitor pattern itself — the executor is still a top-down match dispatch over `FieldRef` / `ComparisonOp` / `Condition` / `SelectClause`.

## 10. Mode interaction

| Feature | Deep mode | Overview mode |
|---|---|---|
| `@objectId`, `@className`, `@shallowSize`, `@objectAddress` | ✅ | ✅ (overview retains classes) |
| `@retainedSize` (P2) | ✅ | ❌ → `FeatureUnavailableInOverviewMode("@retainedSize")` |
| `@toString` (P1) | ✅ | ❌ → `FeatureUnavailableInOverviewMode("@toString")` (no field data) |
| `@gcRootPath` (P5) | ✅ | ❌ → `FeatureUnavailableInOverviewMode("@gcRootPath")` |
| `OBJECTS x.field` (P4) | ✅ | ❌ → `FeatureUnavailableInOverviewMode("OBJECTS")` |
| `LIKE` / `CONTAINS` on `@className` | ✅ | ✅ |
| `CONTAINS` on instance fields | ✅ | ❌ |
| `IS NULL` / `IS NOT NULL` on instance fields | ✅ | ❌ |

The error is rendered as a partial-result envelope at the CLI/MCP boundary (consistent with M7-1's `Partial { reason, hint }` pattern). The query engine itself returns a structured `QueryError` enum that the boundary translates.

Boundary rendering example (CLI text):

```
error: '@retainedSize' is a deep-mode-only OQL feature.
hint:  re-run with --mode deep, or use @shallowSize for a per-object approximation.
```

## 11. Output shape

Existing query output shape:

```jsonc
{ "columns": ["@objectId", "@className"], "rows": [[123, "java.util.HashMap"], …], "total_matched": 42, "truncated": false }
```

M7-4 changes:

- **`SELECT @toString`** column → `["@toString"]`, cell type `CellValue::Str` (or `CellValue::Null` for unresolved).
- **`SELECT @gcRootPath`** column → `["@gcRootPath"]`, cell type `CellValue::Str` (semicolon-separated frames) or `CellValue::Null`.
- **`SELECT OBJECTS field`** columns → `["@objectId", "@className"]` (standard projection of the **referent**, not the source). `total_matched` reports the size of the deduped referent set, not the source set; `truncated` semantics unchanged.
- **`IS NULL` / `IS NOT NULL`** in `WHERE` → no column-shape change.
- **Mode-mismatch error** → no rows; surfaced as a structured error before any execution work.

JSON output (MCP, CLI `--json`) gains no new top-level fields. The existing `QueryResult` envelope is sufficient.

## 12. Test plan

Each predicate gets at minimum one positive test, one negative (no-match) test, and one syntax-error test, plus regression tests over the existing surface.

| Slice | New tests (target) | Regression tests |
|---|---|---|
| M7-4.A | `synth_to_string` unit (String + non-String + null), `@gcRootPath` unit (linear path + unreachable + cap) | parser round-trips for existing queries — assert `parse_query` output is byte-identical for the 9 existing test queries |
| M7-4.B | `WHERE @retainedSize > N` deep ✅, deep ❌ no match, overview-mode error path | re-run [core/tests/query_executor.rs](../../core/tests/query_executor.rs) to confirm same row order |
| M7-4.C | `LIKE '%a%b%'` (multi-`%`), `CONTAINS 'foo'` ✅, `CONTAINS 'xyz'` ❌, `CONTAINS` on `Int` returns false, syntax error: `CONTAINS` without RHS | existing single-`%` `LIKE` tests still pass |
| M7-4.D | `OBJECTS field` ✅ (referent emitted), `OBJECTS field` with null field (skipped), `OBJECTS @retainedSize` syntax error, dedup test (two sources sharing a referent → one row) | existing `SELECT *` queries unchanged |
| M7-4.E | `@gcRootPath CONTAINS 'X'` ✅, unreachable object → `IS NULL` matches, `field IS NOT NULL` ✅, `field != null` regression (was broken; now works) | full executor regression sweep: every existing test asserts identical `QueryResult` |

Hard regression gate: a single **golden test** that runs all 9 pre-M7-4 queries against a shared fixture and asserts the `QueryResult` JSON is byte-identical to a recorded snapshot. This is the contract that M7-4 is non-breaking.

Test count target: **+15 minimum**, +20 likely. Workspace test count expected to advance from 330+ (post-M7-3) to ~350 by end of M7-4.

## 13. Slice breakdown

Five TDD slices, each 2–5 hours of focused work, each independently shippable.

### M7-4.A — Pseudo-attribute infrastructure

- Lift GC-root-path helpers from `core/src/report/flamegraph/collapse/gc_root_path.rs` into a new shared module `core::graph::gc_root_path` (`build_reverse_edges`, `build_gc_root_lookup`, `shortest_gc_root_path`, `format_path`). Update flamegraph collapser to import from there.
- Implement `synth_to_string(graph, object_id) -> CellValue` with the JDK 9+ byte/coder + pre-9 `char[]` decode logic and the 4096-byte cap.
- Add `BuiltInField::GcRootPath` to AST + parser (`@gcRootPath` lex hook).
- Add lexer recognition for `OBJECTS`, `CONTAINS`, `IS`, `NOT` keywords (token-level only — no AST/executor wiring yet; parsing them in unsupported positions still errors).
- Wire `BuiltInField::ToString` to `synth_to_string` (replacing the placeholder), and `BuiltInField::GcRootPath` to the lifted helper.
- Tests: `synth_to_string` unit tests, `@gcRootPath` unit tests, parser-recognizes-keyword tests.

### M7-4.B — `@retainedSize` mode-aware predicate

- Add `QueryError` enum (`FeatureUnavailableInOverviewMode { feature, hint }`).
- Add AST pre-scan in `execute_query` that collects deep-mode-only built-ins.
- Wire CLI/MCP boundaries to translate `QueryError` into the partial-result envelope.
- Tests: deep-mode positive/negative, overview-mode error path, structured error JSON shape.

### M7-4.C — `LIKE` multi-`%` and `CONTAINS` operator

- Replace single-`*` `glob_match` with a multi-segment matcher that handles arbitrary `%` count and explicit anchors.
- Add `ComparisonOp::Contains` to AST + parser + executor (`compare_values`).
- Add the missing null arms to `compare_values` (`(Id, Null)` and `(Str, Null)` for `Eq`/`Ne`).
- Tests: multi-`%` `LIKE`, `CONTAINS` ✅/❌, type-mismatch returns false, `field != null` regression.

### M7-4.D — `OBJECTS x.field` projection

- Refactor `Condition` from struct to enum (`Compare { … }`, `IsNull(FieldRef)`, `IsNotNull(FieldRef)`) — unblocks E too.
- Add `SelectClause::Objects(FieldRef)` to AST + parser (`SELECT OBJECTS …` branch).
- Executor branch: matched-set walk + referent resolve + dedup + standard `[@objectId, @className]` projection. `LIMIT` applies post-dedup.
- Reject `OBJECTS @retainedSize` / `OBJECTS @gcRootPath` at parse time with a typed error.
- Tests: positive, null-field skip, dedup, syntax-error.

### M7-4.E — `IS NULL` / `IS NOT NULL` + `@gcRootPath` end-to-end + regression sweep

- Wire `IS NULL` / `IS NOT NULL` parser productions to the `Condition::IsNull` / `Condition::IsNotNull` enum variants from D.
- Wire `@gcRootPath CONTAINS 'X'` end-to-end: parser already recognizes `@gcRootPath` (A) and `CONTAINS` (C); this slice asserts the integration test.
- Add per-`execute_query` memoization for the reverse-edge map and GC-root lookup.
- Snapshot regression test: 9 pre-M7-4 queries produce byte-identical `QueryResult`.
- Tests: `@gcRootPath CONTAINS 'X'` positive, unreachable-object `IS NULL`, `IS NOT NULL`, snapshot regression.

## 14. Risks & open questions

1. **`LIKE` pattern syntax — SQL `%` vs MAT regex.** MAT uses Java regex; we keep glob `%`/`*`. **Decision:** document the divergence in [docs/api.md](../api.md) under OQL; provide a migration note ("if you were using MAT regex, replace `.*` with `%`"). Risk low — `LIKE '%foo%'` is by far the most common shape.
2. **Null safety in field traversal (P4 `OBJECTS`).** Field that resolves to a null object reference must skip rather than emit a `Null` row. Field that resolves to a primitive must produce a typed parse-time error, not a runtime empty result. Guard tested in M7-4.D.
3. **Performance of `@gcRootPath` on large result sets.** BFS over reverse edges, per matched object, is O(matched × heap) worst-case. **Mitigation:** memoize reverse-edge map + GC-root lookup once per `execute_query` invocation (lazy, only built if any `@gcRootPath` reference exists). For pathological workloads (`SELECT @gcRootPath FROM "*"`), the cost is unavoidable; we document it as "use a `WHERE` filter to narrow the matched set first." Open: do we need a `LIMIT` push-down so `LIMIT 50` short-circuits the path computation? **Proposed:** yes — apply `LIMIT` at scan time when no ordering-sensitive projections are present. Defer to M7-4.E review.
4. **Mode-mismatch error rendering across CLI / MCP / JSON.** Three render paths must agree. **Mitigation:** `QueryError` is the single source of truth; CLI and MCP boundaries each have one translation site, both covered by tests in M7-4.B.
5. **`Condition` enum refactor breaks downstream consumers.** Internal-only — `query::Condition` is exported but no test or non-`query` module pattern-matches on it (verified by `grep_search`). Risk low; if we discover a hidden consumer during slice D, we promote a `Condition::compare(field, op, value) -> Condition` constructor and keep field accessors stable.
6. **`@toString` on very large strings.** A `byte[]` of 1 GB would blow up materialization. **Mitigation:** 4096-byte cap with a trailing `…` marker; documented in §7.1.
7. **MAT users will expect more than five predicates.** True. M8-2 is the full-grammar slice; M7-4 ships the targeted parity that closes 80% of real triage queries and explicitly punts the rest.

## 15. Implementation readiness verdict

**READY** — the design is grounded in the actual `core/src/query/` code, all five predicates have concrete syntax / semantics / mode behavior / error cases, the AST and executor extension points are named at file-and-function granularity, the slice breakdown is independent and ordered, and the regression-snapshot strategy guarantees the existing surface is byte-identical after M7-4. Implementation Agent may proceed with **Slice M7-4.A (pseudo-attribute infrastructure: lift `gc_root_path` helpers, real `synth_to_string`, `@gcRootPath` AST + lexer hooks, `OBJECTS`/`CONTAINS`/`IS`/`NOT` keyword recognition)**.
