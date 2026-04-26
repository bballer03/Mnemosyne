---
name: receiving-code-review
description: "Use when responding to review findings from another Mnemosyne agent (spec-compliance, code-quality, security, contract). Triages findings by severity, fixes critical/important items, defers nits with reasons, and triggers re-review until approved."
---

# Receiving Code Review (Mnemosyne)

## Purpose

Convert reviewer findings into a fix-or-defer decision per finding, then close the loop. The implementer's job is to **act**, not argue.

## When to use

- Static Analysis Agent returns code-quality findings.
- Architecture Review Agent returns spec-compliance gaps.
- API Contract Agent returns contract drift findings.
- Security Agent returns audit findings (when the implementer is the remediation owner).

## Severity model

Reviewers tag every finding:

| Severity | Meaning | Default action |
|---|---|---|
| **Critical** | Correctness bug, security flaw, contract violation, data loss risk, panic on user input | **Must fix** before approval |
| **Important** | Likely-bug, missing edge case, performance regression, leaky abstraction, undocumented public API | **Must fix** unless explicit waiver |
| **Nit** | Style preference, naming, minor refactor opportunity, optional clarity comment | **May defer** with reason; record in handoff |

If the reviewer didn't tag severity, ask them to before responding (do not guess).

## Triage procedure

For each finding:

1. **Read it once, fully.** Don't skim.
2. **Classify**: do you agree it's a finding? If not, you must explain — don't silently dismiss.
3. **Decide**: fix now, fix in a follow-up task (with explicit issue/task created), or waive (only Nits, with a reason).
4. **Record**: every finding gets a status in your reply.

### Reply format

```markdown
## Review response — Task N

**Reviewer:** <agent>
**Round:** 1 (or N for re-review)

### Findings

| # | Severity | Status | Notes |
|---|---|---|---|
| 1 | Critical | Fixed | core/src/analysis/leak_detector.rs:34 — corrected retained-size aggregation to use unique-edge set. New test `aggregates_shared_edges_once` covers the regression. |
| 2 | Important | Fixed | Replaced `unwrap()` with `?` and added `LeakDetectorError::MissingDominatorTree`. |
| 3 | Important | Disputed | The reviewer flagged the `O(n log n)` sort; the input is bounded by class count (≤ 65535 per JVM spec). I documented the bound in the function doc comment. Reviewer please confirm. |
| 4 | Nit | Deferred | Renamed `cnt` → `count` is a style improvement; deferred to a follow-up Refactor batch (tracked in handoff). |
| 5 | Nit | Fixed | Inline doc-comment added. |

### Fix evidence

- `cargo test --workspace` → 414 passed (was 412), 0 failed, 7 ignored
- `cargo clippy --workspace -- -D warnings` → 0 warnings
- New tests: `aggregates_shared_edges_once`, `errors_when_dominator_tree_missing`
- Commit: `<sha>`

### Re-review request

Please re-review focusing on Findings 1, 2, and 3 (the disputed item).
```

## Looping rules

- The **same** implementer subagent (or agent) addresses findings — preserves task context.
- The **same** reviewer re-reviews — preserves review context.
- Loop continues until reviewer returns ✅ Approved.
- Maximum 3 review rounds before escalating to the orchestrator. After round 3, something structural is wrong (plan, scope, or design); don't grind.

## Disputing a finding

You may dispute a finding. You must:

1. State the disagreement crisply.
2. Provide evidence (code reference, spec excerpt, benchmark numbers).
3. Mark the finding **Disputed** in the table.
4. Let the reviewer respond. Their re-review is the tiebreaker for technical findings; the orchestrator is the tiebreaker for scope or contract findings.

Do **not**:
- Silently ignore a finding.
- Rewrite the finding to a weaker version then claim it's addressed.
- Mark Disputed and proceed without reviewer response.

## Deferring (Nits only)

Acceptable reasons:
- Out of task scope (would require touching a non-scope file).
- Better handled in a coordinated refactor batch.
- Style preference where the codebase has no established rule.

Unacceptable reasons:
- "I don't have time."
- "Reviewer is wrong" without dispute.
- "It's just a nit" with no other reason.

Every deferred Nit goes into the handoff `Follow-up required` section so the orchestrator can schedule it.

## When the reviewer overruns scope

Reviewers occasionally flag issues outside the current task. Response:

- Acknowledge the finding.
- Mark **Out of scope (logged)**.
- Add to the handoff `Follow-up required` section with file path and one-line description.
- Do not fix in this task — that violates [verification-before-completion](../verification-before-completion/SKILL.md) scope discipline.

## Anti-patterns

- "Addressed" with no diff or commit reference.
- Marking everything Nit to dodge fixes.
- Disputing without evidence.
- Fixing flagged code by deleting the test that exposed it.
- Skipping re-review and self-approving.
- Silently widening scope to fix every drive-by issue the reviewer noticed.
