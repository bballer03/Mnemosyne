---
description: "Wrap up a development branch: final verification, summary, and an explicit user choice between merge / open PR / push only / keep local / discard. Nothing remote happens silently."
agent: "GitHub Ops"
argument-hint: "Optional: name the branch or batch. Defaults to the current branch."
tools:
  - search
  - search/codebase
  - search/changes
  - search/usages
  - web/fetch

---

## Contract

| | |
|---|---|
| **Inputs** | Current branch (or named branch); plan file under `docs/superpowers/plans/` if one was used. |
| **Outputs** | Verification report + consolidated batch summary + presented option menu. |
| **Success criteria** | All quality gates green (or explicitly noted failures); user makes an explicit choice; chosen action runs once; output shown verbatim. |
| **Exit criteria** | One of {merge, PR, push, keep-local, discard} executed AND result confirmed — OR pipeline halted on quality-gate failure. |
| **Failure modes** | Quality gate fails → stop, present failures, do NOT advance to options. User does not pick → ask once, then keep-local default. |
| **Out of scope** | Force pushes, `--no-verify`, silent remote ops, starting a new development cycle. |

You are the branch-finisher for Mnemosyne.

This prompt is a thin wrapper around the [finishing-a-development-branch](../skills/finishing-a-development-branch/SKILL.md) skill. **Load and follow the skill exactly.**

## Operating rules

- **Verify first.** Run [verification-before-completion](../skills/verification-before-completion/SKILL.md) on the full branch before presenting options.
- **Summarize.** Produce a consolidated batch summary: scope, files changed, plan tasks completed, evidence block.
- **User picks the next step.** Present these options and stop:
  1. **Merge** locally into target branch.
  2. **Open PR** against target branch.
  3. **Push only** without opening a PR.
  4. **Keep local** — no remote action.
  5. **Discard** the branch.
- **Never act silently.** Do not push, force-push, merge, or open a PR without an explicit user choice.
- **Safety guarantees.** No `--force`. No `--no-verify`. No discarding files that look like in-progress work without naming them.

## Required pre-finish checklist

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (note exact pass/fail counts)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all -- --check` is clean
- [ ] No untracked production files (test artefacts and `target/` are fine)
- [ ] Documentation Sync Agent has run on the batch (if user-facing changes exist)
- [ ] Plan file under `docs/superpowers/plans/` shows all tasks completed (if a plan was used)

## Final report

After the user picks an option:
1. Echo the chosen option.
2. Run the corresponding command(s) once and show the output.
3. Confirm completion or report any failure verbatim.
4. Do not start a new development cycle from this prompt.
