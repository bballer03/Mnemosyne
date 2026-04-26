# Superpowers Workflow

This directory holds the **spec → plan → review** artefacts produced by Mnemosyne's skill-driven workflow.

The lifecycle is:

```
/brainstorm  →  specs/   →  /write-plan  →  plans/   →  /execute-plan  →  reviews/  →  /finish-branch
```

Each stage is gated by a mandatory skill in [.github/skills/](../../.github/skills/).

## Folder layout

| Folder | Purpose | Produced by |
|---|---|---|
| `specs/` | Single-pass Socratic specs for new features or milestones. One file per feature, named `YYYY-MM-DD-<feature>.md`. | [brainstorming](../../.github/skills/brainstorming/SKILL.md) skill, run by Design Consulting via `/brainstorm`. |
| `plans/` | Bite-sized 2–5 minute task lists with RED → GREEN → REFACTOR cycles. One file per spec, named `YYYY-MM-DD-<feature>.md`. | [writing-plans](../../.github/skills/writing-plans/SKILL.md) skill, run by Orchestration via `/write-plan`. |
| `reviews/` | Consolidated review notes for non-trivial batches: spec compliance, code quality, follow-ups. | [requesting-code-review](../../.github/skills/requesting-code-review/SKILL.md) and [receiving-code-review](../../.github/skills/receiving-code-review/SKILL.md), run inside the [subagent-driven-development](../../.github/skills/subagent-driven-development/SKILL.md) loop. |

## Worked examples

Existing end-to-end artefacts in this repository (browse [specs/](specs/), [plans/](plans/), [reviews/](reviews/) for the full set):

- Spec: [specs/2026-04-12-ai-task-runner-design.md](specs/2026-04-12-ai-task-runner-design.md)
- Plan: [plans/2026-04-15-m4-real-local-detail-bridge.md](plans/2026-04-15-m4-real-local-detail-bridge.md)
- Review notes: [reviews/2026-04-15-project-review.md](reviews/2026-04-15-project-review.md)

## Operating rules

1. **Spec before plan, plan before execution.** No skipping. If you start writing code without an artefact in `plans/` for behavior-changing work, stop and run `/write-plan`.
2. **Single-pass spec.** No chunked sign-off. The user approves the **finished** spec, not each section.
3. **Autonomous per-task review.** The user does not approve each task during execution. The user approves the spec, the plan, and the final merge/PR/keep choice. See [docs/agent-workflow.md](../agent-workflow.md).
4. **TDD inside every task.** Failing test first (Testing Agent), production code second (Implementation Agent), optional cleanup third (Refactor Agent). See [tdd-cycle](../../.github/skills/tdd-cycle/SKILL.md).
5. **No worktrees.** Mnemosyne uses regular branches. The upstream `using-git-worktrees` skill is intentionally absent.

## When to use the prompts

| Prompt | Use when |
|---|---|
| `/brainstorm <feature>` | New feature, milestone, or non-trivial change with no spec yet. |
| `/write-plan <feature>` | Spec exists; ready to produce a tactical task list. |
| `/execute-plan` | Plan exists; ready to dispatch the per-task subagent loop. |
| `/plan-and-execute` | One-shot spec → plan → execute → verify pipeline for narrow, well-understood work. |
| `/finish-branch` | All planned tasks are green; ready to merge / open PR / push / keep / discard. |

## Cross-references

- Workflow contract: [docs/agent-workflow.md](../agent-workflow.md)
- Skill catalog: [.github/skills/README.md](../../.github/skills/README.md)
- Custom agents: [.github/agents/](../../.github/agents/)
