---
name: verification-before-completion
description: "Use before any Mnemosyne agent declares a task DONE. Evidence-over-claims checklist that requires cargo check, cargo test, cargo clippy, and contract/doc alignment proofs before the handoff is allowed to report completion."
---

# Verification Before Completion (Mnemosyne)

## Purpose

Stop "I think it's done" handoffs. Every agent must produce **evidence** for the claims in its handoff: actual cargo command output, file paths, test counts, and contract checks.

## When to use

- Before any agent writes its mandatory handoff with `Status: DONE`.
- Before the orchestrator marks a plan task complete.
- Before the `finishing-a-development-branch` skill runs.
- Before reporting batch completion to the user.

## Mandatory checklist

Each item is **evidence-backed**, not opinion-backed. If you cannot produce the evidence, the task is not done.

### Build & lint

- [ ] `cargo check --workspace` ran and **passed**. Paste the final line.
- [ ] `cargo test --workspace` ran. Record `<passed>` / `<failed>` / `<ignored>`. Failed must be 0 unless explicitly waived in the task.
- [ ] `cargo clippy --workspace -- -D warnings` ran and **passed**. Paste the final line.
- [ ] `cargo fmt --check` ran and **passed**, OR `cargo fmt` was applied and the diff is part of the commit.

### Behavior

- [ ] Every new behavior has a test that previously failed (per [tdd-cycle](../tdd-cycle/SKILL.md)). List the test names.
- [ ] Every fixed bug has a regression test in the suite. List it.
- [ ] No test was modified to make it pass without an explanation in the commit message.

### Contracts

- [ ] CLI flags / arguments unchanged, OR change documented in [docs/api.md](../../../docs/api.md) and [README.md](../../../README.md).
- [ ] MCP tool schemas unchanged, OR change documented in [docs/api.md](../../../docs/api.md) and reflected in `core/src/mcp/`.
- [ ] Config keys unchanged, OR change documented in [docs/configuration.md](../../../docs/configuration.md).
- [ ] Report output shape unchanged, OR change documented and any snapshot tests updated.
- [ ] Error variants added/changed, OR no `errors.rs` change.

### Scope discipline

- [ ] Files modified are within the orchestration-assigned scope.
- [ ] No file outside the assigned scope was edited.
- [ ] No `docs/roadmap.md` edits unless this is a Tech PM batch.
- [ ] No production source edits unless this is an Implementation/Security/Refactor/Observability/Database Migration batch.

### Observability & safety

- [ ] No raw heap contents, secrets, file paths from user filesystem, or PII added to logs.
- [ ] Any new fallback / partial-result / heuristic behavior is labeled in code (comment or doc) and in the handoff.
- [ ] No new `unwrap()` / `expect()` / `panic!()` on user-controlled paths in production code, unless explicitly justified.

### Tracking

- [ ] If the task came from a plan, the corresponding TODO entry is marked complete.
- [ ] If the task touched a milestone, the milestone status reflects reality (handed off to Documentation Sync if needed).

## Handoff evidence template

The agent's mandatory handoff `Changes made or validation performed` section must include this block:

```markdown
### Verification evidence

**Build:**
- `cargo check --workspace` → ok
- `cargo clippy --workspace -- -D warnings` → 0 warnings
- `cargo fmt --check` → clean (or `cargo fmt` applied; included in commit `<sha>`)

**Tests:**
- `cargo test --workspace` → 412 passed, 0 failed, 7 ignored
- New tests added: `core/tests/leak_detector.rs::detects_retained_size_above_threshold`
- Regression tests added: <list or N/A>

**Contracts:**
- CLI: unchanged
- MCP: unchanged
- Config: unchanged
- Report: unchanged
- Errors: added `HprofError::TruncatedRecord { offset, needed }`; documented in core/src/errors.rs and propagated.

**Scope:**
- Files modified: core/src/analysis/leak_detector.rs, core/src/analysis/mod.rs, core/tests/leak_detector.rs
- Files outside scope edited: none

**Observability:**
- No new sensitive data in logs.
- New fallback behavior: none.
- New panics: none.
```

## Verdict rules

- All boxes checked → DONE is allowed.
- Any unchecked box without an explicit waiver in the task scope → status is **NOT_DONE**, hand back to the implementer (or escalate to the orchestrator if blocked).
- Waivers must be quoted from the task or orchestrator instruction. "I think it's fine" is not a waiver.

## Anti-patterns

- "Tests pass on my machine" without paste.
- "Lint mostly clean" — no, clippy is binary.
- "Docs are mostly aligned" — call Documentation Sync.
- Marking DONE while a clippy warning, fmt diff, or contract drift is open.
- Treating verification as a formality — it's the only thing standing between this batch and a regression.
