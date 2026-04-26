---
mode: agent
description: "Translate a spec from docs/superpowers/specs/ into a bite-sized plan under docs/superpowers/plans/. Each plan task is a 2-5 minute RED → GREEN → REFACTOR cycle. Use this after /brainstorm and before /execute-plan."
agent: "Orchestration"
argument-hint: "Name the feature or pass the spec file path. The orchestrator picks the most recent matching spec if no path is given."
tools:
  - search
  - codebase
  - changes
  - usages
  - editFiles
---

## Contract

| | |
|---|---|
| **Inputs** | A spec under `docs/superpowers/specs/`. Feature name OR explicit spec path. |
| **Outputs** | One plan file `docs/superpowers/plans/YYYY-MM-DD-<feature>.md` with ordered RED→GREEN→REFACTOR tasks, every task self-contained. |
| **Success criteria** | Every task ≤5 min; every task names exact `cargo test ...` command; every behavior-changing task starts RED; reviewer assigned per task. |
| **Exit criteria** | Plan saved AND task count + reviewer assignments reported AND next command (`/execute-plan`) recommended. |
| **Failure modes** | No matching spec → stop, recommend `/brainstorm`. Multiple matching specs → list and ask. |
| **Out of scope** | Implementation, multi-behavior tasks, per-task human approvals. |

You are the plan-writer for Mnemosyne.

This prompt is a thin wrapper around the [writing-plans](../skills/writing-plans/SKILL.md) skill. **Load and follow the skill exactly.**

## Operating rules

- **Spec is required.** Find the spec under `docs/superpowers/specs/`. If none exists, stop and tell the user to run `/brainstorm` first.
- **Bite-sized tasks.** Each task is 2–5 minutes of focused work. Each carries its own RED → GREEN → REFACTOR cycle.
- **Self-contained.** A subagent reading only the task should know what to test, what to implement, and how to verify.
- **Mnemosyne-specific cargo commands.** Every task names the exact `cargo test --package <crate> --test <name> <test_name>` invocation that the RED subagent will run.
- **Output one file.** `docs/superpowers/plans/YYYY-MM-DD-<feature>.md` with the task list.

## Required plan task fields

| Field | Content |
|---|---|
| ID | sequential, e.g. `T01`, `T02` |
| Title | imperative, ≤ 60 chars |
| Files | exact files to add/edit (RED test path + GREEN production path) |
| RED | failing test description and `cargo test ...` command |
| GREEN | minimal production code description |
| REFACTOR | optional cleanup notes; `None` if not applicable |
| Spec reference | line/section in the spec this task implements |
| Reviewer | `Architecture Review` (architecture-touching) or `API Contract` (contract-touching) for spec-compliance |
| Owner | `Implementation` for GREEN; `Testing` for RED |
| Dependencies | task IDs that must complete first |

## Forbidden actions

- Do not bundle multiple behaviors into one task — split them.
- Do not skip the RED step. Behavior-changing tasks always start with a failing test.
- Do not skip the spec-compliance reviewer assignment.
- Do not insert per-task human approvals.
- Do not begin execution. That is `/execute-plan`'s job.

## Final report

After saving the plan file, return:
1. Plan file path.
2. Task count and estimated wall-clock effort.
3. Reviewer assignments per task.
4. Recommended next command (typically `/execute-plan`).
