---
name: systematic-debugging
description: "Use when investigating a failing Mnemosyne test, panic, regression, MCP error, or unexplained behavior. Enforces a 4-phase root-cause process (reproduce, isolate, fix, verify) with mandatory failing-test reproduction before any production fix."
---

# Systematic Debugging (Mnemosyne)

## When to use

- A test that previously passed now fails.
- A panic, `unreachable!()`, or `unwrap()` blow-up in `core/`, `cli/`, or `tauri/`.
- An MCP tool returns an unexpected error or empty result.
- A heap-dump parse fails on a fixture that should work.
- A user reports wrong analysis output.
- A clippy warning surfaces a real bug, not just a style issue.

## Core principle

```
Reproduce → Isolate → Fix → Verify
```

No phase may be skipped. The fix is **never** allowed before reproduction.

## Phase 1 — Reproduce

The bug is not real to the project until a deterministic test reproduces it.

### Step 1.1 — Capture the symptom

Record exactly:
- The command or scenario that triggers it.
- The full error / panic / wrong output.
- The git SHA (run `git rev-parse HEAD`).
- The fixture or input file (path + size + relevant fields).

### Step 1.2 — Write a failing test

Pick the smallest test surface:
- Unit test if the bug is inside one function.
- Integration test in `core/tests/` or `cli/tests/` if the bug is at the API surface.
- New fixture under `resources/test-fixtures/` if needed (use [scripts/generate_synthetic_heap.sh](../../../scripts/generate_synthetic_heap.sh) for synthetic heaps).

The test asserts the **correct** behavior. It must fail today.

### Step 1.3 — Verify RED

```powershell
cargo test --package <crate> --test <name> <test_name> -- --nocapture
```

Confirm the test fails for the bug reason, not for a typo or fixture-loading error.

If you can't reproduce in a test → the bug is environmental, not code. Document the environment and stop. Do not "fix" code that has no failing test.

## Phase 2 — Isolate

### Step 2.1 — Narrow the suspect surface

Use the [GitNexus](../../../AGENTS.md) tools:
- `gitnexus_query({query: "<symptom keywords>"})` to find related execution flows.
- `gitnexus_context({name: "<suspect function>"})` for callers/callees of the prime suspect.
- `gitnexus_impact({target: "<function>", direction: "upstream"})` to check blast radius before editing.

Fall back to:
- `git log -p <file>` to see recent changes.
- `git bisect` if the regression has a clear last-known-good commit.
- `cargo test --workspace -- --nocapture` to see what else fails together.

### Step 2.2 — Form one hypothesis at a time

Write the hypothesis as a single sentence:
> "The retained-size calculation double-counts shared edges in the dominator tree."

Predict what the failing test would do if the hypothesis is true. If the prediction matches the actual failure, the hypothesis is plausible. If not, discard and try another. Don't carry stale hypotheses forward.

### Step 2.3 — Instrument minimally

If the hypothesis needs runtime evidence:
- Add **scoped** `tracing::debug!` calls inside the suspect function.
- Run the failing test with `RUST_LOG=mnemosyne_core=debug cargo test ...`.
- Read the trace; confirm or refute the hypothesis.
- Remove the temporary instrumentation before committing the fix (or hand it off to the Observability Agent if it's worth keeping).

## Phase 3 — Fix

### Step 3.1 — Run impact analysis on the change site

Before editing the suspect function, run:
```
gitnexus_impact({target: "<function>", direction: "upstream"})
```

If risk is HIGH or CRITICAL, surface it to the orchestrator before proceeding.

### Step 3.2 — Make the minimal change

The fix is the smallest diff that turns the failing test green. No drive-by refactors. No "while I'm here" cleanups (those go to the Refactor Agent later).

### Step 3.3 — Do not modify the test to make it pass

If the test asserts the correct behavior, the production code must change to match it. Modifying the test means you're papering over the bug.

The only exception: if isolating revealed the test itself encoded wrong behavior, document why in the commit message and update both.

## Phase 4 — Verify

### Step 4.1 — GREEN locally

```powershell
cargo test --package <crate> --test <name> <test_name>
cargo test --workspace
```

Confirm:
- Target test passes.
- All other tests still pass.
- No new warnings.

### Step 4.2 — Lint and contract

```powershell
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

If the bug touched a public surface (CLI, MCP, report shape), hand off to the **API Contract Agent** for contract validation before declaring done.

### Step 4.3 — Regression coverage stays

The failing test from Phase 1 stays in the suite forever. It is the regression guard.

### Step 4.4 — Run `verification-before-completion`

Apply the [verification-before-completion](../verification-before-completion/SKILL.md) checklist before reporting DONE.

## Anti-patterns

| Anti-pattern | Why it fails | Correct move |
|---|---|---|
| Patch the symptom in the call site | The bug recurs from other call sites | Fix at the root (Phase 2 isolation) |
| "I think I see the issue" → edit → run | No test, no proof | Phase 1 first, always |
| Add `if x.is_none() { return Err(...); }` and call it fixed | Hides the upstream cause | Find why `x` was unexpectedly `None` |
| Modify the test to assert the buggy output | Encodes the bug as "intended" | Production code changes; test asserts correctness |
| Mass-add `tracing::trace!` everywhere | Adds noise; doesn't isolate | Scoped instrumentation in the one suspect function |
| Skip Phase 4 because "it works on my machine" | Regressions ship | Run the full workspace test + lint |

## Final-rule

```
No fix without a failing test.
No "done" without verification.
Regression test stays in the suite.
```
