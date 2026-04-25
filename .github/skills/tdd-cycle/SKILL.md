---
name: tdd-cycle
description: "Use when implementing any new behavior, bug fix, or behavior change in Mnemosyne Rust source (core/, cli/, tauri/). Enforces RED-GREEN-REFACTOR with cargo test. The Iron Law: no production Rust without a failing test first."
---

# Test-Driven Development Cycle (Mnemosyne)

## The Iron Law

```
NO PRODUCTION RUST
WITHOUT A FAILING TEST FIRST
```

Code written before the test is **deleted**. Don't keep it as "reference". Don't "adapt" it. Implement fresh from the test.

## When to use

**Always:**
- New features in `core/src/` or `cli/src/`
- Bug fixes (the failing test reproduces the bug)
- Behavior changes
- Refactors that change observable behavior

**Exceptions (do not skip lightly — record a reason in the handoff):**
- Pure formatting / rename refactors with no behavior change
- Generated code
- Configuration files (`Cargo.toml`, `tauri.conf.json`)
- Documentation-only changes
- Throwaway prototypes that never land

## Role split

In Mnemosyne the cycle spans two agents to keep file-ownership boundaries clean:

| Phase | Agent | Output |
|---|---|---|
| RED — write failing test | Testing Agent | Failing test in `core/tests/` or `cli/tests/` or inline `#[cfg(test)] mod tests` if assigned |
| Verify RED | Testing Agent | `cargo test <name>` output showing the right failure |
| GREEN — minimal code | Implementation Agent | Production code that makes the test pass |
| Verify GREEN | Implementation Agent | `cargo test` output showing all passing |
| REFACTOR | Implementation Agent (or Refactor Agent) | Clean code, still green |

The handoff carries the failing test, the failure message, and the scope.

## Red-Green-Refactor for Rust

### RED — write the failing test

One behavior. Real types. Clear name.

```rust
#[test]
fn parses_truncated_hprof_record_returns_partial_error() {
    let bytes = include_bytes!("fixtures/truncated_record.hprof");
    let result = parse_record(bytes);
    assert!(matches!(result, Err(HprofError::TruncatedRecord { .. })));
}
```

Requirements:
- Tests **real code**, not mocks (mocks only when unavoidable — heap parsing rarely needs them).
- Name describes the behavior, not the implementation.
- Asserts on observable outcomes (return values, error variants, side effects), not internals.
- One assertion target per test (multiple `assert!`s are fine if they describe the same behavior).

### Verify RED — watch it fail (MANDATORY)

```powershell
cargo test --package mnemosyne-core --test <test_file> <test_name> -- --nocapture
```

Confirm:
- Test **fails**, not compile-errors (compile errors mean fix imports first, then re-run).
- Failure message matches the expected behavior gap.
- Failure is because the feature is missing, not because of a typo.

If the test passes immediately → you're testing existing behavior. Rewrite the test.

### GREEN — minimal code

Write the simplest Rust that makes this one test pass. No extra fields, no extra error variants, no speculative `Option<T>` parameters.

```rust
pub fn parse_record(bytes: &[u8]) -> Result<Record, HprofError> {
    if bytes.len() < MIN_RECORD_LEN {
        return Err(HprofError::TruncatedRecord { offset: 0, needed: MIN_RECORD_LEN });
    }
    // ...
}
```

YAGNI applies hard. Don't add features the test doesn't require.

### Verify GREEN — watch it pass (MANDATORY)

```powershell
cargo test --package mnemosyne-core
```

Confirm:
- Target test passes.
- All other tests still pass.
- No new warnings introduced (run `cargo clippy -- -D warnings` if lint cleanliness matters).

If other tests break → fix the production code, not the tests, unless the breakage proves the old tests encoded wrong behavior. Document any test deletions in the handoff.

### REFACTOR — clean up

Stay green:
- Extract helpers
- Improve names
- Remove duplication
- Simplify match arms

Do not add behavior in this phase.

## Common rationalizations (all wrong)

| Excuse | Reality |
|---|---|
| "Rust's type system already proves correctness" | Type-correct code can still violate semantics. The test pins behavior. |
| "It's just a small parser tweak" | Parser bugs are the highest-impact bugs in this codebase. Test it. |
| "I'll add tests after the heap fixture is parsing" | Tests-after pass on first run; they prove nothing. |
| "Tests-after achieve same coverage" | Coverage ≠ proof of failure detection. |
| "Already manually tested with the real .hprof" | Manual testing isn't repeatable. |
| "Deleting working code is wasteful" | Sunk cost. Untested production Rust is technical debt. |

## Bug-fix flow

A bug **is** a missing test. The cycle for a reported bug:

1. Reproduce in a failing test that asserts the buggy output.
2. Verify RED with `cargo test`.
3. Fix the production code minimally.
4. Verify GREEN.
5. Refactor if needed.
6. Commit with the test and fix together.

This is the only way to prevent regressions.

## Verification checklist

Before marking a TDD cycle complete:

- [ ] Failing test was written before the production change.
- [ ] `cargo test` was run with the test alone and showed the expected failure.
- [ ] Production code is minimal — nothing the test doesn't require.
- [ ] `cargo test --workspace` runs clean (or every failure is documented).
- [ ] No new `cargo clippy -- -D warnings` regressions.
- [ ] Test name describes the behavior, not the implementation.

## Red flags — STOP and start over

- Wrote the production code first and then a test that passed.
- Test passes immediately on first run.
- Cannot explain why the test failed before the fix.
- "I already manually tested with a real heap dump."
- "Type checker caught it, no test needed."
- Test asserts on internals (private struct fields, log strings) instead of behavior.
- Multiple unrelated assertions in one test.

## Final rule

```
Production Rust  →  test exists and failed first
Otherwise        →  not TDD; revert and restart
```
