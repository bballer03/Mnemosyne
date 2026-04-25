---
name: writing-plans
description: "Use when a spec or design doc exists and a multi-step Mnemosyne implementation needs to be broken into bite-sized 2-5 minute tasks. Produces a plan with exact file paths, complete Rust code, cargo commands, and TDD verification steps."
---

# Writing Plans (Mnemosyne)

## Purpose

Translate a spec ([docs/superpowers/specs/](../../../docs/superpowers/specs/)) or milestone design doc ([docs/design/](../../../docs/design/)) into a sequence of bite-sized tasks that a fresh subagent can execute one at a time without context.

## Audience assumption

Write for a skilled Rust developer who **knows nothing** about the Mnemosyne codebase, JVM heap dump format, or the project's conventions. They will only see one task at a time.

## When to use

- A spec or design doc exists and orchestration is about to start implementation.
- A change touches more than one file or more than one module.
- Subagent-driven-development is the chosen execution model.

## When NOT to use

- A single-file, single-function fix (let Implementation Agent run TDD directly).
- Pure config updates.
- Documentation-only batches (Documentation Sync Agent handles those).

## Output

Save to `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md`.

## Mandatory plan header

```markdown
# <Feature Name> Implementation Plan

> **For executing agents:** REQUIRED SKILL — Use `subagent-driven-development` to
> execute this plan task-by-task. Each task contains its own TDD cycle (RED → GREEN
> → REFACTOR) and is fully self-contained. Steps use `- [ ]` checkboxes.

**Spec:** [docs/superpowers/specs/YYYY-MM-DD-<feature>.md](../specs/YYYY-MM-DD-<feature>.md)
**Design ref:** [docs/design/<milestone>.md](../../design/<milestone>.md)
**Goal:** One sentence describing the user-visible outcome.
**Approach:** 2–3 sentences describing the implementation strategy.
**Crates touched:** `core`, `cli`, …

---
```

## File-structure section

Before tasks, list **every file** that will be created or modified, with one-line responsibility:

```markdown
## File Structure

| File | Change | Responsibility |
|---|---|---|
| `core/src/analysis/leak_detector.rs` | Create | Detects retained-size hotspots over a configurable threshold. |
| `core/src/analysis/mod.rs` | Modify | Re-export `leak_detector`. |
| `core/tests/leak_detector.rs` | Create | Integration tests against synthetic-heap fixtures. |
| `cli/src/main.rs:312-340` | Modify | Add `--leak-threshold` flag. |
| `docs/api.md:88-104` | Modify | Document the new flag. |
```

This is where decomposition decisions get locked in.

## Task structure (TDD-baked)

Every task is one logical behavior change with its own RED → GREEN → REFACTOR cycle. Tasks are 2–5 minutes of subagent work each.

````markdown
### Task N: <Behavior name>

**Files:**
- Create: `core/tests/leak_detector.rs`
- Create: `core/src/analysis/leak_detector.rs`
- Modify: `core/src/analysis/mod.rs`

**Owners:**
- Testing Agent → RED
- Implementation Agent → GREEN + REFACTOR
- Static Analysis Agent → code-quality review
- Architecture Review Agent → spec-compliance review (if module boundaries shift)

- [ ] **Step 1 — RED: write the failing test**

  In `core/tests/leak_detector.rs`:

  ```rust
  use mnemosyne_core::analysis::leak_detector::detect_leaks;

  #[test]
  fn detects_retained_size_above_threshold() {
      let snapshot = mnemosyne_core::test_fixtures::cache_leak_snapshot();
      let leaks = detect_leaks(&snapshot, 1024 * 1024);
      assert!(leaks.iter().any(|l| l.class_name == "java.util.HashMap$Node"));
  }
  ```

- [ ] **Step 2 — Verify RED**

  Run:
  ```powershell
  cargo test --package mnemosyne-core --test leak_detector detects_retained_size_above_threshold -- --nocapture
  ```

  Expected: compile error (`detect_leaks` undefined) or test failure. Both are acceptable RED states; compile error counts as "test cannot pass" which is the correct RED.

- [ ] **Step 3 — GREEN: minimal implementation**

  In `core/src/analysis/leak_detector.rs`:

  ```rust
  use crate::heap::Snapshot;

  pub struct Leak { pub class_name: String, pub retained: u64 }

  pub fn detect_leaks(snapshot: &Snapshot, threshold: u64) -> Vec<Leak> {
      snapshot.classes()
          .filter(|c| c.retained() >= threshold)
          .map(|c| Leak { class_name: c.name().to_string(), retained: c.retained() })
          .collect()
  }
  ```

  In `core/src/analysis/mod.rs`, add:
  ```rust
  pub mod leak_detector;
  ```

- [ ] **Step 4 — Verify GREEN**

  Run:
  ```powershell
  cargo test --package mnemosyne-core --test leak_detector
  cargo test --package mnemosyne-core
  ```

  Expected: target test passes; full core test suite passes.

- [ ] **Step 5 — Lint clean**

  Run:
  ```powershell
  cargo clippy --package mnemosyne-core -- -D warnings
  ```

  Expected: no warnings.

- [ ] **Step 6 — Commit**

  ```powershell
  git add core/src/analysis/leak_detector.rs core/src/analysis/mod.rs core/tests/leak_detector.rs
  git commit -m "feat(analysis): detect retained-size leaks above threshold"
  ```
````

## No placeholders

Every step must be ready to execute. **Do not** write:
- "TBD", "TODO", "fill in details"
- "Add appropriate error handling" / "validate inputs"
- "Write tests for the above" without the actual test code
- "Similar to Task N" — repeat the code in full
- References to types, functions, or imports not defined in any task or already in the codebase
- Cargo commands without the expected output described

If a step cannot be completely specified, the spec is not ready — return to brainstorming.

## Self-review (run before saving)

Walk through the spec one more time:

1. **Spec coverage** — every requirement in the spec maps to at least one task. List any gaps.
2. **Placeholder scan** — search the plan for `TODO`, `TBD`, `FIXME`, `...`, `// fill in`. Eliminate.
3. **Type and name consistency** — function names, struct fields, error variants used in Task 7 must match Task 3. A `Leak` struct in Task 3 and a `LeakInfo` struct in Task 7 is a bug.
4. **Dependency order** — Task N+1 may depend only on what Task N committed. No forward references.
5. **TDD coverage** — every behavior-changing task has RED → Verify-RED → GREEN → Verify-GREEN → REFACTOR (REFACTOR is optional; the others are not).
6. **File ownership** — no two tasks write the same file in the same step. Sequential modifications to the same file are fine if ordered.

Fix issues inline. No subagent dispatch for self-review.

## Execution handoff

After saving the plan, hand off to orchestration with:

```
PLAN READY: docs/superpowers/plans/YYYY-MM-DD-<feature>.md
TASKS: <count>
NEXT: subagent-driven-development (orchestration dispatches per task; no per-task human gate).
```

The orchestrator proceeds without waiting for confirmation unless the plan touches a contract that the user explicitly flagged as locked.

## Anti-patterns

- Vague verification steps ("run the tests").
- Tasks that bundle multiple behavior changes (split them).
- Tasks longer than ~5 minutes of subagent work (split them).
- Skipping the file-structure table.
- Writing a plan straight from a one-liner request without a spec doc.
