# Milestone 15 — MAT Backend Parity Completion

> **Status:** 🔲 Pending — design authored 2026-08-20, awaiting Implementation Agent pickup of Slice 15.A.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M15
> **Predecessors:** M7-4 (OQL targeted expansion, shipped) — this milestone closes the remaining ~70% of MAT's OQL surface that M7-4 explicitly deferred. M3 Phase 3 (`analyze_strings()` duplicate-group shape, shipped) — this milestone's duplicate-array detector mirrors it. M6 Phase 8 (plugin/extension design doc, shipped as design-only) — this milestone executes its own recommended Phase 2.
> **Last updated:** 2026-08-20

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M15 |
| Type | Parity-closing (last four 🟡/❌ rows in the MAT parity matrix that are pure analysis-capability gaps, not credibility/evidence gaps like M12) |
| Touched crates | `core`, `cli`. MCP wiring where each item's existing surface already has an MCP equivalent (query, group-by, string/array analysis all do). UI wiring is out of scope (M14's own pattern: this milestone ships capability, UI consumes it later only if a slice specifically needs UI to be meaningful — see §4). |
| Test count target | +40 to +60 net new Rust tests across four independent scope items. |
| Memory budget | No item introduces new full-graph passes beyond what's already paid for graph-backed `analyze`/`query` today — multi-hop traversal is bounded (see §4.1), duplicate-array detection is a single pass mirroring the existing string-duplicate pass's cost profile. |

## 2. Objective

After M15, the four remaining rows in roadmap.md's MAT parity matrix (§2) that represent genuine missing analysis capability — not missing UI, not missing benchmark evidence — are closed:

1. **OQL depth** — subqueries, `UNION`, multi-hop traversal, `eval(...)`, regex `=~`, `dominators(...)`, `outbounds`/`inbounds` — the ~70% of MAT's OQL surface M7-4 explicitly deferred.
2. **Duplicate primitive-array / boxed-array detection** — MAT flags arrays with identical contents wasting memory; Mnemosyne today only does this for `String`s.
3. **`--group-by superclass`** — MAT's "group by class → superclass tree" view; Mnemosyne's existing `--group-by {class,package,classloader}` has no superclass option.
4. **Custom plugin/extension runtime** — MAT's `IQuery` extension point; Mnemosyne has a library API (`mnemosyne-core` is a public crate) but no formal registration mechanism for third-party analyzers.

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [core/src/query/{types.rs,parser.rs,executor.rs,synth.rs}](../../core/src/query/) — `Query { select: SelectClause, from: FromClause, filter: Option<WhereClause>, limit: Option<usize> }`. **Flat, single-level only**: one `SELECT ... FROM <class-pattern> [WHERE ...] [LIMIT n]`, no nesting, no traversal beyond the single-hop `OBJECTS x.field` projection M7-4 already shipped. `ComparisonOp` has `Eq/Ne/Gt/Lt/Ge/Le/Like` (SQL-style `%`/`_`) and `Contains` (confirmed pattern from M7-4) — no regex operator exists.
- [core/src/analysis/string_analysis.rs](../../core/src/analysis/string_analysis.rs) — `DuplicateStringGroup { value: String, count: usize, total_wasted_bytes: u64 }` inside `StringReport`. This exact shape (value/count/wasted-bytes) is the direct template for item 2's array equivalent — the design decision here is "reuse this shape for arrays," not invent a new one.
- [core/src/graph/metrics.rs](../../core/src/graph/metrics.rs) — `HistogramGroupBy { Class, Package, ClassLoader }` (serde `snake_case`). Adding `Superclass` is a one-variant enum extension plus the grouping-key logic to walk `ClassInfo.super_class_id` — the same field M13's classloader work already reads for `parent_loader`-style single-edge resolution, so the walking pattern is proven in this codebase already (M13's `resolve_loader_chain`, bounded + cycle-guarded).
- [docs/design/m6-plugin-extension-system.md](m6-plugin-extension-system.md) — existing design doc, status "Design only — implementation deferred until demonstrated demand." Recommends a three-phase path: **Phase 1** (already true today — `mnemosyne-core` is a public library crate), **Phase 2** (trait registry: `AnalyzerPlugin`/`ReportFormatterPlugin` traits, static registration, triggered by "≥3 requests for custom analyzers/formats"), **Phase 3** (dynamic `cdylib` loading via `~/.mnemosyne/plugins/`, triggered by Phase 2 adoption + demand for out-of-tree plugins). **This milestone's item 4 executes Phase 2, not Phase 3** — see §3.3 for why.

### 3.2 What MAT does

- **Full OQL**: MAT's Object Query Language supports subqueries (`SELECT * FROM OBJECTS (SELECT ... )`), `UNION`, `eval(...)` (embedded Java-like scriptlets), regex matching in `WHERE`, `dominators(...)` and `outbounds(...)`/`inbounds(...)` traversal functions, and multi-class `FROM` lists. This is the single largest remaining MAT capability gap.
- **Duplicate arrays**: MAT's "Duplicate Classes"/memory-waste reports include primitive and boxed array content-equality detection alongside string interning waste.
- **Group by superclass**: MAT's class histogram can re-root by superclass hierarchy instead of exact class name, useful for framework-heavy heaps where many leaf classes share a common ancestor worth aggregating.
- **`IQuery` plugins**: MAT's extension point — a Java interface implementers register via Eclipse's plugin.xml mechanism, executed inside the same JVM/Eclipse process as MAT itself.

### 3.3 Design decision: plugin runtime ships as Phase 2 (trait registry), not Phase 3 (dynamic loading)

The existing M6 design doc gates Phase 3 (dynamic loading) behind "Phase 2 adoption + demand for out-of-tree plugins" — evidence that does not exist for this solo/pre-adoption project. Shipping Phase 3 now would mean building a `cdylib` ABI-stability story, a plugin-discovery/sandboxing model, and versioning guarantees against **zero** real plugin authors — the textbook YAGNI violation this project's own `CLAUDE.md`/`copilot-instructions.md` explicitly warns against ("Don't add features... beyond what the task requires... Don't design for hypothetical future requirements"). **Resolution: M15 ships Phase 2 only** — `AnalyzerPlugin`/`ReportFormatterPlugin` traits with static (compile-time) registration, so any Rust project depending on `mnemosyne-core` can plug a custom analyzer into the existing pipeline today. This genuinely closes the "can a third party extend Mnemosyne's analysis" gap MAT's `IQuery` answers, without inventing infrastructure no one has asked for. Phase 3 stays exactly where the M6 doc already put it: deferred until Phase 2 sees real use.

## 4. Scope

In:

### 4.1 OQL depth (bounded, named sub-list — not open-ended)

Per roadmap.md's own risk register ("OQL scope creep... treadmill risk... absorb incrementally, never as a standalone open-ended milestone" — §8), this item is a **fixed, named list**, not "implement full MAT OQL":

1. `outbounds(<object-id-or-query>)` / `inbounds(<object-id-or-query>)` traversal functions — single-hop, reusing `ObjectGraph::get_references`/`get_referrers` (already exist, M8-proven).
2. `dominators(<object-id-or-query>)` — projects to the dominator-chain, reusing `DominatorTree::immediate_dominator` (already exists).
3. Regex operator `=~` on string-capable fields, alongside the existing `Like`/`Contains` operators — same `ComparisonOp` enum extension shape M7-4 already used to add `Contains`.
4. Subqueries: `SELECT * FROM OBJECTS (<subquery>)` — a subquery is itself a `Query`, evaluated first, its result object-id set feeding the outer query's `FROM`. Bounded to one level of nesting (not arbitrary recursion) to keep the grammar and evaluator tractable — MAT itself rarely nests beyond one level in practice.
5. `UNION` — two `Query` results concatenated, deduplicated by object id.

**Explicitly still deferred after M15** (named, not silently dropped): `eval(...)` scriptlets (MAT embeds a Java-like expression language; Mnemosyne has no embedded scripting story and building one is a separate, much larger design question outside this milestone's bounded list), multi-class `FROM` lists, arbitrary-depth subquery nesting.

### 4.2 Duplicate primitive/boxed array detection

New `DuplicateArrayGroup { element_type: String, content_hash: u64, length: usize, count: usize, total_wasted_bytes: u64 }` (mirrors `DuplicateStringGroup`'s shape, adapted for arrays — `content_hash` instead of a literal `value: String` since array contents aren't always meaningfully stringifiable, e.g. `byte[]`). New function `analyze_duplicate_arrays()` in `core::analysis::string_analysis` (extend the existing module — it's already "duplicate-content detection," arrays are the same family, not a new module) or a new sibling `core::analysis::array_analysis` if the existing file's own organization suggests separation (implementation's call, follow the file's current internal structure). `StringReport` or a new `ArrayReport` gains the result (decide based on whether `StringReport` is genuinely about strings-only elsewhere in its own fields, or already a general "duplicate content" report in disguise — read the file before deciding).

### 4.3 `--group-by superclass`

`HistogramGroupBy::Superclass` variant. Grouping key: walk `ClassInfo.super_class_id` to the root (`java.lang.Object`, `super_class_id == 0`), same bounded-walk-with-cycle-guard pattern as M13's `resolve_loader_chain` (reuse the *pattern*, not the code — different field, different domain). CLI `--group-by superclass`, MCP `histogram_group_by` param gains the new enum value (additive, existing values unchanged).

### 4.4 Plugin/extension runtime (Phase 2 only, per §3.3)

`AnalyzerPlugin` trait: `fn name(&self) -> &str`, `fn analyze(&self, graph: &ObjectGraph, dominator: Option<&DominatorTree>) -> AnalyzerResult` (or closely matching the shape already sketched in `m6-plugin-extension-system.md` §2.1 — that sketch is the starting point, not necessarily final; adjust only if implementation finds a genuine mismatch with the real `ObjectGraph`/`DominatorTree` APIs, which didn't fully exist when that doc was written). `ReportFormatterPlugin` trait for custom output formats. Static registration via a `PluginRegistry` the CLI/MCP construct at startup with whatever built-in + statically-linked third-party plugins the binary was compiled with (no dynamic loading, no runtime discovery — that's Phase 3). Ship with zero built-in example plugins beyond what's needed to prove the trait shape works end-to-end in tests (a test-only demo plugin, not a shipped user-facing one — avoid the temptation to invent a "flagship" plugin nobody asked for).

Out:

- **UI surfacing of any item above.** M14's own pattern (backend-before-UI); this milestone's items get UI treatment only in a future milestone once real usage justifies which of the four is worth UI investment.
- **`eval(...)` scriptlets, arbitrary-depth OQL nesting, multi-class `FROM`.** Named exclusions from §4.1, not silent gaps.
- **Plugin runtime Phase 3 (dynamic loading).** §3.3's binding decision.
- **Group-by-superclass UI tree view** (MAT's actual visual is a collapsible tree; the CLI/MCP surface here is a flat grouped list, same shape as the existing `--group-by class|package|classloader`). Tree presentation is UI-layer work, out of scope here.

## 5. Sub-slice plan

All slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice 15.A — Duplicate array detection

- **Scope:** §4.2, smallest and lowest-risk item, good first slice to prove the pattern before the larger OQL work.
- **Files owned:** `core/src/analysis/string_analysis.rs` (extend) or new `core/src/analysis/array_analysis.rs`, `core/src/analysis/engine.rs` (wire into `AnalyzeResponse`), `cli/src/main.rs`, `core/src/mcp/server.rs`.
- **Validation gates:** fixture with two byte-array instances of identical content is detected as one `DuplicateArrayGroup`; arrays of different content or different element type never merge; existing string-duplicate tests pass unchanged (regression gate — confirms the extension didn't disturb the existing detector).
- **Target size:** ~250 LOC + ~250 LOC tests.

### Slice 15.B — Group by superclass

- **Scope:** §4.3.
- **Files owned:** `core/src/graph/metrics.rs` (extend `HistogramGroupBy`), wherever the grouping-key logic lives (likely `core/src/analysis/engine.rs`'s `build_histogram`), `cli/src/main.rs`, `core/src/mcp/server.rs`.
- **Validation gates:** a 3-level class hierarchy (leaf → mid → `java.lang.Object`) groups correctly by each ancestor level requested; existing `--group-by class|package|classloader` tests pass unchanged.
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 15.C — OQL: `outbounds`/`inbounds`/`dominators` traversal functions

- **Scope:** §4.1 items 1–2, the two items reusing existing, already-tested graph primitives (no new grammar complexity beyond function-call syntax in the query parser).
- **Files owned:** `core/src/query/{types.rs,parser.rs,executor.rs}`, `cli/tests/`, `core/tests/`.
- **Validation gates:** `outbounds`/`inbounds`/`dominators` each produce results matching direct `ObjectGraph`/`DominatorTree` calls on the same fixture; existing OQL tests (M7-4's targeted slice) pass unchanged.
- **Target size:** ~350 LOC + ~350 LOC tests.

### Slice 15.D — OQL: regex `=~` operator

- **Scope:** §4.1 item 3.
- **Files owned:** `core/src/query/{types.rs,parser.rs,executor.rs}`. Check whether `regex` crate is already a dependency (it's widely used in Rust query engines; verify in `Cargo.toml` before adding a new one).
- **Validation gates:** regex matching against string-capable fields (class name, synthetic `@toString`) produces correct matches; malformed regex patterns fail with a structured parse error, not a panic; existing `Like`/`Contains` tests pass unchanged.
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 15.E — OQL: subqueries + `UNION`

- **Scope:** §4.1 items 4–5, the largest grammar/evaluator change in this milestone — sequenced last within the OQL sub-list so the simpler function-call and operator extensions (15.C/15.D) are already proven before tackling nested query evaluation.
- **Files owned:** `core/src/query/{types.rs,parser.rs,executor.rs}`.
- **Validation gates:** one-level subquery (`SELECT * FROM OBJECTS (SELECT ...)`) evaluates the inner query first and correctly restricts the outer `FROM`; `UNION` of two queries deduplicates by object id; a second-level nested subquery is rejected with a clear "nesting depth exceeded" error, not silently truncated or infinitely evaluated.
- **Target size:** ~400 LOC + ~400 LOC tests.

### Slice 15.F — Plugin/extension runtime (Phase 2)

- **Scope:** §4.4. Sequenced last — highest design uncertainty of the four scope items (the M6 doc's sketch predates several APIs it will now actually touch), and the roadmap's own note ("consider splitting into M15-B if scope proves too large once designed" — see roadmap.md M15 entry) applies here specifically. If this slice's own implementation discovers the trait shape needs materially more than a `PluginRegistry` + two traits, split it into its own follow-up rather than scope-creeping this slice.
- **Files owned:** new `core/src/plugin/` module, `cli/src/main.rs` (wherever the CLI constructs its analysis pipeline, to register built-in plugins), test-only demo plugin.
- **Validation gates:** a test-only `AnalyzerPlugin` implementation registers and its `analyze()` output appears in a full pipeline run; a `ReportFormatterPlugin` implementation's output is selectable via the existing `--format` mechanism (extend, don't replace, the existing format enum).
- **Target size:** ~350 LOC + ~300 LOC tests.

### Slice 15.G — Documentation sync

- **Scope:** `docs/roadmap.md` (mark M15 shipped, parity matrix, scorecard, design-doc index), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md`, `docs/user-guide.md` (extend OQL reference, `--group-by`, duplicate-array section, plugin API docs pointing developers at the new traits), `docs/design/m6-plugin-extension-system.md` (update its own status table to reflect Phase 2 shipped, Phase 3 still gated).
- **Files owned:** Documentation only.
- **Validation gates:** Full-workspace `cargo` gate still green. Matches the M8/M9/M11/M13/M10-B doc-sync precedent.
- **Target size:** Documentation-only.

## 6. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | OQL scope creep past the named §4.1 list | The list is fixed and named in this doc; `eval(...)`/multi-class-FROM/deep-nesting are explicit named exclusions, not silent gaps — any implementer tempted to add "just one more OQL feature" must come back to this doc and get it added to §4.1 first, not slip it into a slice. |
| R2 | Plugin runtime scope balloons past Phase 2 | §3.3 is binding; Slice 15.F's own validation gates only cover Phase 2's trait-registry shape. |
| R3 | Regex operator becomes a footgun on adversarial/huge input (ReDoS-style pathological patterns) | Use the `regex` crate (linear-time guarantee, not backtracking) if not already a dependency — do not use a backtracking regex engine for user-supplied query patterns. |
| R4 | Subquery evaluation cost (running two queries instead of one) on very large graphs | Bounded to one nesting level (§4.1 item 4) keeps worst-case cost at 2x a single query, not exponential — no further budget mechanism needed at this bound. |
| R5 | `DuplicateArrayGroup`'s `content_hash` collisions produce false-positive duplicate groups | Use a strong hash (reuse `xxhash-rust` if already a dependency per M10's diff work, or `sha2` already used elsewhere) over the full array content, not a truncated/sampled hash — same "no shortcuts on identity" discipline M10's object-diff fingerprinting held. |

## 7. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M15 backlog entry.
- Predecessor OQL work: M7-4 (targeted expansion, shipped — no dedicated design doc found in `docs/design/`, referenced only in roadmap history).
- Duplicate-detection template: [core/src/analysis/string_analysis.rs](../../core/src/analysis/string_analysis.rs).
- Bounded-walk-with-cycle-guard template: M13's `resolve_loader_chain` in [milestone-13-classloader-explorer.md](milestone-13-classloader-explorer.md).
- Plugin design precedent (binding for §3.3's Phase 2 scope decision): [m6-plugin-extension-system.md](m6-plugin-extension-system.md).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 15.G.

## 8. Implementation readiness verdict

**READY.** Slice 15.A (duplicate arrays) first — smallest, lowest-risk, proves the milestone's working pattern. 15.B (group-by-superclass) is independent and may run in a **separate isolated git worktree** in parallel with 15.A (different files: `analysis/string_analysis.rs` vs `graph/metrics.rs`/`analysis/engine.rs`'s histogram path). 15.C→15.D→15.E are sequential (same files, increasing grammar complexity, each proving out before the next). 15.F is independent of the OQL slices (different module) but sequenced last per its own higher design uncertainty. 15.G is gated behind all of 15.A–15.F.
