---
name: requesting-code-review
description: "Use before handing off Mnemosyne changes to a reviewer agent (Architecture Review, API Contract, Static Analysis, Security). Produces a structured review request with self-review checklist, file diffs, scope, risks, and test evidence so the reviewer has full context."
---

# Requesting Code Review (Mnemosyne)

## Purpose

Reviewers review faster and find more when the request is structured. This skill defines the request format every Mnemosyne implementer (Implementation Agent, Security Agent in remediation, Refactor Agent) must produce before invoking a reviewer.

## When to use

- Implementation subagent finishes a task in the [subagent-driven-development](../subagent-driven-development/SKILL.md) loop and is about to hand off to spec-compliance review.
- Implementation subagent hands off to code-quality review (Static Analysis Agent).
- Refactor Agent hands off to Static Analysis after a cleanup batch.
- Security Agent hands off remediated code to Static Analysis or to a follow-up audit.

## Self-review first

Before invoking the reviewer, run this self-review. Reviewers should not be your first line of defense.

- [ ] All [tdd-cycle](../tdd-cycle/SKILL.md) steps completed.
- [ ] All [verification-before-completion](../verification-before-completion/SKILL.md) boxes checked.
- [ ] Diff is minimal — no drive-by changes.
- [ ] Function and type names match conventions used in the surrounding code.
- [ ] No dead code, no commented-out blocks, no TODO comments without an issue link.
- [ ] No `unwrap()` / `expect()` / `panic!()` on user-controlled paths.
- [ ] All public items have at least a one-line doc comment.
- [ ] Error variants are typed (`thiserror`), not stringly.

## Review request template

Save the request to `docs/superpowers/reviews/YYYY-MM-DD-<feature>-<reviewer>.md` (or include it inline in the subagent dispatch when running the autonomous loop).

```markdown
# Review Request: <Task name>

**Reviewer:** <Architecture Review | API Contract | Static Analysis | Security>
**Implementer:** <agent name>
**Plan:** [docs/superpowers/plans/YYYY-MM-DD-<feature>.md](../plans/YYYY-MM-DD-<feature>.md) — Task N
**Spec:** [docs/superpowers/specs/YYYY-MM-DD-<feature>.md](../specs/YYYY-MM-DD-<feature>.md)

## Scope of this review

- What this task changes:
- What this task explicitly does NOT change:

## Files changed

| File | Change | Lines | Reason |
|---|---|---|---|
| core/src/analysis/leak_detector.rs | Create | +47 | new behavior |
| core/src/analysis/mod.rs | Modify | +1 / -0 | re-export |
| core/tests/leak_detector.rs | Create | +28 | RED then GREEN |

Run: `git diff --stat <base>..HEAD` and paste the result.

## Behavior summary

1–3 sentences in user-visible terms ("`detect_leaks` now returns the set of classes whose retained size meets the threshold; sorted descending").

## Test evidence

```text
$ cargo test --package mnemosyne-core --test leak_detector
running 3 tests
test detects_retained_size_above_threshold ... ok
test ignores_classes_below_threshold ... ok
test handles_empty_snapshot ... ok
```

```text
$ cargo test --workspace
... 412 passed; 0 failed; 7 ignored ...
```

## Lint evidence

```text
$ cargo clippy --workspace -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.31s
```

## Contracts touched

- CLI: <unchanged | flag added: --leak-threshold>
- MCP: <unchanged | tool added>
- Config: <unchanged>
- Report: <unchanged | section added>
- Errors: <unchanged | variant added>

## Risks the reviewer should focus on

Be specific. Examples:
- "The retained-size aggregation reuses dominator-tree edges; double-counting risk if a node has multiple immediate dominators (shouldn't happen, but please sanity-check)."
- "I added a new `Leak` struct in `analysis::leak_detector`; please confirm this doesn't shadow or conflict with `analysis::leak::*` types."

## Open questions

Anything the implementer is unsure about, framed as a question. Reviewer can decide.

## Reviewer ask

- **Spec-compliance reviewer:** does the diff implement exactly what the plan task says — no more, no less?
- **Code-quality reviewer:** are there severity-tagged findings (Critical / Important / Nit) on safety, performance, or maintainability?
- **API Contract reviewer:** are CLI/MCP/config/report contracts preserved or correctly extended?
- **Architecture reviewer:** are module boundaries respected? Any new coupling that violates the architecture doc?
```

## Reviewer-specific extras

### Spec-compliance review (Architecture Review or API Contract)
Include the relevant excerpt from the plan task. The reviewer compares diff to excerpt and reports gaps OR extras (over-building counts as a finding).

### Code-quality review (Static Analysis)
Include the `cargo clippy` output and the `git diff` of the changed files. Reviewer reports findings by severity.

### Security review
Include any new input-handling, deserialization, file-path construction, command construction, or network-touching code. List any new dependencies.

## Anti-patterns

- "Please review my changes" with no context.
- Linking the whole branch — reviewers shouldn't reconstruct scope.
- Hiding risks ("nothing to flag" when you know there's a corner case).
- Asking the reviewer for a yes/no without specifying what spec to compare against.
- Skipping the self-review and offloading it to the reviewer.
