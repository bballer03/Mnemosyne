# Milestone 13 — Classloader Explorer & Leak Detection

> **Status:** ✅ **Shipped** — Slices 13.A–13.C (implementation) and 13.D (this doc-sync pass) all complete. See §16 for the closeout note.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M13
> **Predecessors:** None blocking. Roadmap.md lists M13 as "parallel-eligible with M11" — no hard dependency on M8/M9/M10/M11's own surfaces, though §4 notes one soft dependency on M9-shipped conventions for consistency.
> **Last updated:** 2026-08-18 (Slice 13.D doc-sync)

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M13 |
| Type | Parity-closing (the classic Tomcat/Jetty/Spring webapp classloader leak — a real MAT capability Mnemosyne users in the JVM-webapp segment specifically lack) |
| Touched crates | `core`, `cli`. `ui/` leak-workspace panel is listed in roadmap.md's original M13 scope text but downgraded to a documented follow-up here — see §4 "Out." |
| Test count target | +25 to +35 net new Rust tests. |
| Memory budget | Cross-loader duplicate detection is a single pass over `graph.classes` grouped by normalized class name — no new graph traversal, no new peak-RSS profile beyond the existing `analyze --classloaders` cost. |

## 2. Objective

After M13, a Mnemosyne user investigating a suspected classloader leak (the classic "redeploying this webapp N times without restarting the app server leaves N generations of the same classes alive, because N different webapp classloaders are each still reachable") can answer:

1. **"Is the same class loaded by more than one classloader right now?"** — the actual MAT-defining signal for this leak pattern. Today Mnemosyne cannot answer this at all.
2. **"For a given classloader, which of its loaded classes are unique to it versus shared with others?"** — `mnemosyne analyze --classloaders`'s per-loader `loaded_class_count` today is a bare count with no unique/shared breakdown.
3. **"Show me the classloader parent chain, not just the immediate parent."** — `ClassLoaderInfo.parent_loader` already exists but is a single `Option<ObjectId>` edge; there is no drill-down into grandparents/ancestry.
4. **"Fail my CI build if classloader-leak count exceeds N between deploys."** — `core::policy` ships 10 predicates today; none cover classloaders.

### 2.1 What already ships today — corrected against roadmap.md's original framing

Grounding work for this design doc found that roadmap.md's MAT parity matrix (§2, "Classloader leak detection | 🟡 (per-loader histogram only)") **understates what's already shipped**. `core::analysis::classloader::analyze_classloaders()` (M3 Phase 3) already computes `ClassLoaderReport { loaders: Vec<ClassLoaderInfo>, potential_leaks: Vec<ClassLoaderLeakCandidate> }` — a **single-loader heuristic** leak flag (a loader is flagged when it retains ≥ 8 MB but declares ≤ 3 classes: "retains a lot, loads almost nothing else," a real and useful signal, just a *different* signal than MAT's cross-loader duplicate check). Until this design pass, `potential_leaks` was computed but never printed by the CLI's text renderer and had zero test coverage anywhere in the codebase — both gaps were closed as a direct fix ahead of this design doc (see the commit immediately preceding this file's own history), so this doc's scope below is scoped against the *now-accurate* baseline, not the stale "🟡 per-loader histogram only" framing.

**What M13 actually closes, given that corrected baseline:**
1. Cross-loader duplicate-class detection (§2 point 1) — genuinely new logic, MAT's actual defining pattern, not present in any form today.
2. Unique-vs-shared class breakdown per loader (§2 point 2).
3. Multi-level parent-loader chain / tree (§2 point 3) — today's `parent_loader` is one edge, not a walked chain.
4. `ci-check classloader_leak_count` predicate (§2 point 4).

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [core/src/analysis/classloader.rs](../../core/src/analysis/classloader.rs) — `ClassLoaderInfo { object_id, class_name, loaded_class_count, instance_count, total_shallow_bytes, retained_bytes, parent_loader }`, `ClassLoaderLeakCandidate { object_id, class_name, retained_bytes, loaded_class_count, reason }`, `ClassLoaderReport { loaders, potential_leaks }`, `analyze_classloaders(graph, dominator)`. `parent_loader` is resolved via `read_field(loader_object, ..., "parent", ...)` — a single typed-field read, not a traversal. Grouping today is keyed by `class_info.class_loader_id` — this is the exact grouping M13's duplicate-detection reuses, just pivoted: instead of "classes per loader," M13 additionally computes "loaders per class name."
- [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `AnalyzeRequest.enable_classloaders` / `AnalyzeResponse.classloader_report`, already wired end-to-end (CLI `--classloaders`, MCP `enable_classloaders` param). `HistogramGroupBy::Classloader` already exists as a `--group-by classloader` option (separate, coarser aggregation than the dedicated classloader report).
- [core/src/policy/mod.rs](../../core/src/policy/) and its `eval.rs`/`input.rs`/`result.rs`/`render/` siblings — the existing 10-predicate `ci-check` engine (M7-2). New predicates are added by extending the predicate enum/evaluator, following the same pattern the existing 10 already establish — no new policy *engine* work, just one more predicate.
- [cli/src/main.rs](../../cli/src/main.rs) — `--classloaders` flag on `analyze`, `build_classloader_table()`, and (as of the grounding fix immediately preceding this doc) `print_classloader_leak_candidates()`.
- [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `analyze_heap`'s `enable_classloaders` param already returns `classloader_report` over MCP. No dedicated `detect_classloader_leaks` tool exists yet.

### 3.2 What MAT does

MAT's classic classloader-leak workflow: "Duplicate Classes" report lists every class name loaded by more than one classloader, grouped by class name with the list of loader instances underneath each. Combined with a retained-size view of each "orphaned" loader generation, this is the standard first move for diagnosing the Tomcat/Jetty/Spring hot-redeploy leak. M13's cross-loader duplicate detection (§4) targets this exact report shape.

### 3.3 Design principle: two independent leak signals, not one replaced by the other

M13 does **not** replace `potential_leaks` (the existing single-loader "retains a lot, loads little" heuristic) with the new cross-loader duplicate signal — they catch different failure modes and both stay. A loader can be duplicate-flagged (same class loaded elsewhere) without retaining much yet (early in a leak's life), and can retain a lot without any duplication (a legitimately large, single-instance loader that's just big). `ClassLoaderReport` gains a new field for the new signal; the old field is untouched.

## 4. Scope

In:

1. `core::analysis::classloader` extended (not replaced): new function `detect_duplicate_classes(graph: &ObjectGraph) -> Vec<DuplicateClassGroup>` — groups `graph.classes` by normalized class name, keeps only groups where `class_loader_id` differs across ≥ 2 members (a class genuinely loaded by more than one distinct loader — same class name loaded once by the bootstrap loader and once by a webapp loader is exactly the target pattern; the same class loaded twice *by the same loader* is not a duplicate in this sense and is excluded).
2. `DuplicateClassGroup { class_name: String, loader_object_ids: Vec<ObjectId>, loader_count: usize }` — new type, added to `ClassLoaderReport` as a new field (`duplicate_classes: Vec<DuplicateClassGroup>`), additive per this codebase's established discipline.
3. Unique-vs-shared breakdown: extend `ClassLoaderInfo` with `unique_class_count: usize` (classes loaded by this loader and no other) — computed as a byproduct of the same grouping pass that builds `duplicate_classes`, no second pass needed.
4. Parent-loader chain: new function `resolve_loader_chain(graph, loader_id, max_depth) -> Vec<ObjectId>` — walks `parent_loader` repeatedly (reusing the existing single-edge resolution, just iterated) up to a bounded depth (mirrors M8 Slice 8.A's `dominator_class_chain` depth-4 precedent for "bounded chain walk," though classloader hierarchies are typically shallow — 2 to 4 levels in real JVMs — so the default cap can be generous, e.g. 16, without real risk of runaway cost). Exposed as `ClassLoaderInfo.ancestor_chain: Vec<ObjectId>` (new additive field) rather than a separate call, since every loader's chain is cheap to compute alongside its existing per-loader aggregate.
5. `ci-check` predicate: `classloader_leak_count` (threshold on `duplicate_classes.len()`, following the exact shape of the existing `leak_count`/`dominator_root_count` deep-only predicates in `core::policy`).
6. CLI: extend the existing `--classloaders` output (no new flag — this is additive to the flag that already exists, matching M13's "extend, don't replace" principle from §3.3) with a "Duplicate classes across loaders" section, printed the same way the just-added "Potential classloader leaks" section is. `ci-check --policy <file>` gains the new predicate automatically once it's in the policy schema, no CLI surface change beyond that.
7. MCP: `analyze_heap`'s existing `enable_classloaders` param now populates the extended `ClassLoaderReport` automatically (additive fields, no new param needed). New standalone tool `detect_classloader_leaks` for callers who want *only* the duplicate-class signal without a full `analyze_heap` call (mirrors why `diff_heaps` exists as its own tool rather than folding into `analyze_heap` — a focused, cheaper single-purpose call has real value for an AI agent that already knows it wants exactly this).
8. Validation: synthetic fixture with a deliberately duplicated class across two loaders (the "redeployed webapp" shape) plus a legitimately-large-single-loader fixture (to confirm §3.3's two-signals-don't-collide claim), `ci-check` predicate test, parent-chain walk test with a 3-level ancestor chain.

Out:

- **UI leak-workspace panel.** Roadmap.md's original M13 scope text mentions one; per the established M8/M9/M10/M11 pattern of shipping backend surface before UI, this is downgraded to documented future work here, not implemented in this milestone.
- **Group-by-superclass histograms.** Roadmap.md's own backlog explicitly defers this (§2 "Group by class / classloader / package / superclass" row, "Missing group-by-superclass"), unrelated to classloader-leak detection specifically and out of this milestone's actual objective.
- **Live classloader unloading / triggering GC.** Same non-goal class as M11's `tune_gc` — diagnostic only, Mnemosyne never touches a live JVM.
- **Custom plugin runtime for user-defined leak heuristics.** Deferred per the existing `docs/design/m6-plugin-extension-system.md` earmark, same as M11.
- **Replacing `potential_leaks`.** §3.3 is binding — both signals ship, neither replaces the other.

## 5. Architecture overview

```
 mnemosyne analyze <heap> --classloaders
                          │
                          ▼
        core::analysis::classloader::analyze_classloaders(graph, dominator)
          [EXTENDED, not replaced]
          │
          ├─ existing per-loader aggregate pass (unchanged)
          │    → ClassLoaderInfo { ..., unique_class_count (NEW),
          │                        ancestor_chain (NEW) }
          │
          ├─ existing potential_leaks heuristic (unchanged)
          │
          └─ NEW: detect_duplicate_classes(graph)
                → group graph.classes by normalized class name
                → keep groups with ≥ 2 distinct class_loader_id values
                → DuplicateClassGroup { class_name, loader_object_ids, loader_count }
                          │
                          ▼
        ClassLoaderReport { loaders, potential_leaks,
                             duplicate_classes (NEW) }
                          │
              ┌───────────┴────────────┐
              ▼                        ▼
   CLI text renderer            core::policy
   (extend existing              classloader_leak_count predicate
    --classloaders output)       (threshold on duplicate_classes.len())
```

Module placement rationale: everything stays inside `core::analysis::classloader` — this is the same domain, extending an existing analyzer rather than introducing a new top-level module, unlike M9/M11's genuinely new cross-cutting concerns.

## 6. Data model

```rust
// core/src/analysis/classloader.rs (extended)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateClassGroup {
    pub class_name: String,
    pub loader_object_ids: Vec<ObjectId>,   // sorted ascending for determinism
    pub loader_count: usize,
}

pub struct ClassLoaderInfo {
    // ...existing fields unchanged...
    pub unique_class_count: usize,             // NEW: classes loaded only by this loader
    pub ancestor_chain: Vec<ObjectId>,          // NEW: parent, grandparent, ... up to the bound; empty if no parent
}

pub struct ClassLoaderReport {
    pub loaders: Vec<ClassLoaderInfo>,
    pub potential_leaks: Vec<ClassLoaderLeakCandidate>,   // unchanged, both signals coexist per §3.3
    pub duplicate_classes: Vec<DuplicateClassGroup>,      // NEW
}

pub fn detect_duplicate_classes(graph: &ObjectGraph) -> Vec<DuplicateClassGroup> {
    // group by normalized class name; keep groups with >= 2 distinct
    // class_loader_id values; sort loader_object_ids ascending;
    // sort the returned Vec by loader_count desc, then class_name asc
}
```

`unique_class_count` and `duplicate_classes` are computed from the same underlying grouping (class name → set of distinct loader ids) — `analyze_classloaders()` builds that grouping once and derives both outputs from it, rather than scanning `graph.classes` twice.

### 6.1 `ci-check` predicate

```toml
# policy file addition, same shape as existing predicates
[[rules]]
predicate = "classloader_leak_count"
threshold = 0
severity = "warning"
```

Deep-only (requires `ClassLoaderReport`, which requires a built `ObjectGraph`) — same category as the existing `leak_count`/`retained_size`/`dominator_root_count` deep-only predicates, evaluated against `duplicate_classes.len()`.

## 7. CLI surface

No new flags. `mnemosyne analyze <heap> --classloaders` output gains a new section:

```
ClassLoader Report:
  <existing per-loader table, now with a "Unique" column alongside "Classes">

Duplicate classes across loaders (2):
  com.example.webapp.RequestHandler  loaded by 3 loaders: 0x1000, 0x2400, 0x3800
  com.example.webapp.SessionCache    loaded by 2 loaders: 0x1000, 0x2400

Potential classloader leaks:
  <existing section, unchanged>
```

`ci-check --policy <file>` recognizes `classloader_leak_count` once present in the policy TOML — no CLI code path change beyond the predicate evaluator addition itself.

## 8. MCP surface

```jsonc
{ "name": "detect_classloader_leaks",
  "description": "Cross-loader duplicate-class detection -- the classic Tomcat/Jetty/Spring hot-redeploy leak pattern.",
  "input": [{ "name": "heap_path", "type": "string", "required": true }],
  "output_schema": "Vec<DuplicateClassGroup>" }
```

`analyze_heap`'s existing `enable_classloaders` param needs no new param — `ClassLoaderReport`'s new fields are additive and appear automatically.

## 9. Sub-slice plan

All slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice 13.A — Duplicate-class detection + unique-class-count

- **Scope:** `detect_duplicate_classes()`, `DuplicateClassGroup`, `ClassLoaderInfo.unique_class_count`, wired into `analyze_classloaders()` and `ClassLoaderReport`.
- **Files owned:** `core/src/analysis/classloader.rs` (extend), `core/tests/classloader_duplicates.rs` (new).
- **Validation gates:** a synthetic fixture with the same class name loaded by 2 distinct loaders is detected; a class loaded twice *within the same loader's declared set* (shouldn't happen in real HPROF but the code must not double-count it as a duplicate group) is excluded; `unique_class_count` sums correctly against `loaded_class_count` minus shared classes on a 3-loader fixture. Existing `potential_leaks` tests (just added) continue to pass unchanged — §3.3's two-signals-coexist claim is a regression gate, not just a design note.
- **Target size:** ~250 LOC + ~250 LOC tests.

### Slice 13.B — Parent-loader chain + CLI rendering

- **Scope:** `resolve_loader_chain()`, `ClassLoaderInfo.ancestor_chain`. Extend `cli/src/main.rs`'s `build_classloader_table`/new duplicate-classes print block per §7.
- **Files owned:** `core/src/analysis/classloader.rs` (extend), `cli/src/main.rs` (extend), `cli/tests/classloader_cli.rs` (new or extend existing classloader CLI test).
- **Validation gates:** a 3-level loader chain (child → parent → grandparent → bootstrap) resolves correctly and stops at the bound; a cyclic parent chain (malformed/adversarial HPROF) does not infinite-loop (bounded walk is the guard, verify with a test). CLI output includes the new "Duplicate classes across loaders" section only when non-empty (matches the existing `potential_leaks` empty-is-silent convention).
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 13.C — `ci-check` predicate + MCP tool

- **Scope:** `classloader_leak_count` predicate in `core::policy`. `detect_classloader_leaks` MCP tool.
- **Files owned:** `core/src/policy/eval.rs`/`input.rs` (extend), `core/src/mcp/server.rs` (extend), `core/tests/policy_classloader_predicate.rs` (new).
- **Validation gates:** predicate fires correctly against a deliberately-duplicated fixture and stays clean against a fixture with no duplicates; predicate is skipped (not errored) against an overview-mode input, matching the existing deep-only predicate skip convention. MCP tool round-trips against the same fixture the CLI test uses.
- **Target size:** ~200 LOC + ~200 LOC tests.

### Slice 13.D — Documentation sync

- **Scope:** `docs/roadmap.md` (mark M13 shipped, correct the §2 parity-matrix row per §2.1's grounding correction, scorecard, design-doc index), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md`, `docs/user-guide.md` (extend the existing `--classloaders` section).
- **Files owned:** Documentation files only.
- **Validation gates:** Full-workspace `cargo {check,test,clippy,fmt}` still green. Matches the M8/M9/M11 doc-sync precedent.
- **Target size:** Documentation-only.

## 10. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | False positives: legitimately re-loaded framework classes (some JVM/framework patterns intentionally reload a small number of classes without it being a leak) | `DuplicateClassGroup` reports the raw signal (which classes, which loaders) without a severity judgment baked in — same "give the operator the data, let them triage" philosophy as `MatchQuality` in M10 and `potential_leaks`' plain heuristic in this same file. No silent suppression of "probably fine" duplicates. |
| R2 | Parent-loader chain walk on adversarial/cyclic HPROF data infinite-loops | Bounded depth walk (§4 point 4), same discipline as M8 Slice 8.A's `ALT_PARENT_CAP`/`MAX_BACKTRACK_WORK` safety bounds. |
| R3 | `unique_class_count` computed inconsistently with `duplicate_classes` if the two ever drift to separate code paths | §6 is explicit: both derive from one grouping pass, not two — enforced by construction, not just convention. |
| R4 | Confusing two different "leak" signals (`potential_leaks` vs `duplicate_classes`) in one report without clear labeling | CLI output uses distinct section headers ("Potential classloader leaks" vs "Duplicate classes across loaders"); `docs/user-guide.md` (Slice 13.D) explicitly explains both signals and when each fires, per §3.3. |

## 11. Test strategy

- Cross-loader duplicate fixture (redeployed-webapp shape): same class name, 2-3 distinct loaders, at least one loader also carrying unique classes — exercises `duplicate_classes` and `unique_class_count` together.
- Regression: existing `potential_leaks` tests (added just before this design doc) pass unchanged, proving §3.3's two-signals-coexist claim.
- Parent-chain: 3+ level chain resolves correctly; cyclic chain does not hang (bounded-walk gate).
- `ci-check` predicate: fires on a duplicated fixture, stays clean on a non-duplicated one, skips (not errors) on overview-mode input.
- Negative: a class loaded by exactly one loader never appears in `duplicate_classes` (the "≥ 2 distinct loaders" filter is exact, not approximate).

## 12. Out-of-scope (explicit non-goals)

- UI leak-workspace panel (§4).
- Group-by-superclass histograms (§4, unrelated to this milestone's actual objective despite superficial roadmap adjacency).
- Live JVM classloader unloading / GC triggering (§4).
- Custom plugin runtime (§4).
- Replacing `potential_leaks` with `duplicate_classes` (§3.3 — both ship, permanently).

## 13. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M13 backlog entry.
- Existing analyzer being extended: [core/src/analysis/classloader.rs](../../core/src/analysis/classloader.rs).
- Sibling design (bounded-chain-walk precedent): [milestone-8-reachability-references.md](milestone-8-reachability-references.md) (M8 Slice 8.A's `ALT_PARENT_CAP`).
- Policy engine being extended: [core/src/policy/](../../core/src/policy/) (M7-2).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 13.D.

## 14. Implementation readiness verdict

**READY** — this design doc is implementation-depth. The Implementation Agent may proceed with **Slice 13.A** (duplicate-class detection) as the first task, since 13.B's CLI rendering and 13.C's policy predicate both consume `ClassLoaderReport.duplicate_classes`, which 13.A defines. 13.B and 13.C are file-disjoint after 13.A lands (`cli/src/main.rs` vs `core/src/policy/*` + `core/src/mcp/server.rs`) and may run in **separate isolated git worktrees** (per the M11 design doc's own §13 note: isolated worktrees, not just disjoint files, are the actual safety requirement for real parallelism in this repository). Slice 13.D is gated behind 13.A–13.C.

## 15. Slice 13.D closeout (documentation sync)

Slices 13.A–13.C landed as: `2ab27b4` (13.A, `detect_duplicate_classes()`, `DuplicateClassGroup`, `ClassLoaderInfo.unique_class_count`, wired into `analyze_classloaders()`/`ClassLoaderReport`), `241d423` (13.B, `resolve_loader_chain()` + `ClassLoaderInfo.ancestor_chain`, CLI `build_classloader_table`'s new "Ancestors" column, `print_classloader_duplicates()`), and `9d67109` + `4a8abff` (13.C, the `classloader_leak_count` predicate in `core::policy`, the MCP `detect_classloader_leaks` tool, plus a same-slice follow-up fix for a code-review finding: `handle_ci_check` had hardcoded `enable_classloaders: false`, so the predicate's evaluator logic was correct but never received real `ClassLoaderReport` data outside its own isolated unit tests on any actual `ci-check` invocation — `enable_classloaders` is now derived from whether the loaded policy declares a `classloader_leak_count` rule). Slice 13.D (this pass) updated: `docs/roadmap.md` (M13 rows marked shipped across the MAT parity matrix §2 — including the §2.1 grounding correction that both `potential_leaks` and `duplicate_classes` ship and coexist — the parity-matrix summary, the M13 milestone entry §5, the scorecard §7, and the design-doc index §9), `STATUS.md` (new M13 snapshot bullet plus two capability-checklist rows for duplicate-class detection and the ancestor-chain/`ci-check`/MCP surface), `CHANGELOG.md` (`[Unreleased]` entry appended below the existing M8/M9/M10 entries), `README.md` (Key Features bullet, MCP method list and MCP-commands table, `ci-check` predicate count), `ARCHITECTURE.md` (new "Shipped today" bullet for the extended `core::analysis::classloader` module, plus a project-structure-tree line for `classloader.rs` that had been missing since before M8's tree was written to this granularity), and `docs/user-guide.md` (extended `--classloaders` output example with the "Duplicate classes across loaders" section and "Ancestors" column, `classloader_leak_count` added to the `ci-check` predicate catalog with a policy TOML example, MCP section extended with `detect_classloader_leaks`).

**Scope drift found during doc-sync (this design doc's own speculative text vs. what actually shipped):**

- **The CLI table gained an "Ancestors" column, not a "Unique" column.** §7's CLI-surface sketch showed "the existing per-loader table, now with a 'Unique' column alongside 'Classes'." What shipped (`cli/src/main.rs::build_classloader_table`) added an `Ancestors` column (`loader.ancestor_chain.len()`) instead. `unique_class_count` is computed correctly (§6's data-model claim shipped exactly as specified, and it is exercised directly by `unique_class_count_excludes_shared_classes_on_three_loader_fixture` and serialized in JSON/TOON output), but it is not rendered as its own CLI text-table column. This is a real, if minor, scope gap — flagged here rather than silently claimed as met — and is now documented as an open item in `docs/user-guide.md` and `STATUS.md` rather than implied to be already CLI-visible.
- **A same-slice bug, not a design-doc/implementation mismatch, but worth recording:** 13.C's own code review caught `handle_ci_check`'s hardcoded `enable_classloaders: false` before merge (see `4a8abff`'s commit message: "A rule that never gets data to evaluate isn't a rule, it's a rumor"). The design doc's §7 ("`ci-check --policy <file>` recognizes `classloader_leak_count` once present in the policy TOML — no CLI code path change beyond the predicate evaluator addition itself") undersold this slightly: one CLI code path change *was* needed, just not a new flag — deriving `enable_classloaders` from the loaded policy's own rule set. Two new end-to-end CLI tests (`cli/tests/classloader_cli.rs`) now prove `ci-check` fires the rule for real, not just in the evaluator's isolated unit tests.
- Everything else — `detect_duplicate_classes()`'s exact grouping/filter semantics (§3.3's cross-loader-only definition, same-loader duplicates correctly excluded), the `DuplicateClassGroup`/`ClassLoaderReport` field shapes, `resolve_loader_chain()`'s bounded-depth-16 + visited-set cycle guard, the `classloader_leak_count` predicate's deep-only/skip-on-absent-report semantics, the `detect_classloader_leaks` MCP tool's name/params/output shape, and the §3.3 two-signals-coexist design principle itself (verified as a regression gate: the pre-existing `potential_leaks` tests pass unchanged) — shipped exactly as this design doc specified.

Full-workspace verification at closeout: `cargo check --workspace --all-targets` clean, `cargo test --workspace` 706 passed / 0 failed, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean.
