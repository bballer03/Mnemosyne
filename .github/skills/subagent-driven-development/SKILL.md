---
name: subagent-driven-development
description: "Use when executing a Mnemosyne implementation plan with two or more independent tasks. Dispatches a fresh subagent per task with two-stage autonomous review (spec compliance then code quality). No human checkpoints between tasks; only hard blockers escalate."
---

# Subagent-Driven Development (Mnemosyne)

## Purpose

Execute an approved plan in [docs/superpowers/plans/](../../../docs/superpowers/plans/) by dispatching a fresh subagent per task, with two automated review stages between tasks. The orchestrator stays at the controller level; subagents do the work in isolated context.

## Core principle

```
Fresh subagent per task  +  spec-compliance review  +  code-quality review  =  autonomous progress
```

**Why subagents:** Tasks delegated to specialized subagents with isolated context stay focused. The controller curates exactly what each subagent needs and never forwards its own session history. The controller's context stays clean for coordination.

## When to use

- A plan exists in [docs/superpowers/plans/](../../../docs/superpowers/plans/).
- The plan has two or more largely independent tasks.
- The orchestrator is running the batch (not a one-off direct user edit request).

## When NOT to use

- Single-task work — the Implementation Agent runs `tdd-cycle` directly.
- Tasks tightly coupled to a single shared file across all of them — execute inline with one Implementation Agent invocation instead.
- Brainstorming or design work (no plan yet — use `brainstorming` and `writing-plans` first).

## Autonomous-review policy

- **No human checkpoint between tasks.** The orchestrator runs spec-compliance review then code-quality review automatically.
- **Hard blockers escalate.** Compile errors that no fix-pass resolves, contract violations the user flagged as locked, or external-system failures stop the loop and surface to the user with a structured report.
- **Soft findings loop autonomously.** Reviewer issues feed back into the same Implementation subagent for fixes, then the same reviewer runs again.

## Reviewer mapping

| Stage | Default reviewer | When to swap |
|---|---|---|
| Implementer | **Implementation Agent** (subagent dispatch) | Always |
| RED writer | **Testing Agent** (subagent dispatch) | Always |
| Spec-compliance review | **Architecture Review Agent** for architecture-touching tasks; **API Contract Agent** for contract-touching tasks; otherwise the **plan author** (Design Consulting) | Pick by task scope |
| Code-quality review | **Static Analysis Agent** (always) | Never swap |
| Final batch review | **Static Analysis Agent** | Once at end of plan |

## Procedure

### 1. Read the plan once, extract all tasks

The orchestrator reads the plan file once. For each task, extract:
- Task number and name
- Files (Create / Modify)
- All steps with their full code blocks
- Owners (which agent runs which step)

Build a TODO list (use `manage_todo_list`) of all tasks before dispatching the first one.

### 2. Per-task loop

```
for task in plan.tasks:
    1. Dispatch Testing Agent (subagent) with:
       - The full RED step text
       - Spec excerpt for the behavior
       - Repo paths for fixtures
       Subagent writes failing test, verifies RED via `cargo test`, returns
       failing-test path + failure message.

    2. Dispatch Implementation Agent (subagent) with:
       - The failing test path + message
       - The full GREEN step text
       - Files allowed to modify
       Subagent writes minimal code, runs `cargo test` (verify GREEN), runs
       `cargo clippy` if step requires, self-reviews, returns DONE / DONE_WITH_CONCERNS / NEEDS_CONTEXT / BLOCKED.

    3. Handle implementer status (see table below).

    4. Dispatch spec-compliance reviewer (subagent) with:
       - Task spec
       - Files changed
       Reviewer returns ✅ Spec compliant OR list of gaps / extras.

       If gaps: re-dispatch Implementation subagent with the specific gaps; loop.

    5. Once spec-compliance ✅: dispatch Static Analysis (subagent) with:
       - Files changed
       Reviewer runs `cargo clippy -- -D warnings` and inspects diffs, returns
       ✅ Approved OR severity-tagged findings.

       Critical/Important findings: re-dispatch Implementation subagent; loop.
       Nit findings: record in handoff, do not block.

    6. Mark task complete in TODO list. Move to next task.

after all tasks:
    7. Dispatch final-batch reviewer (Static Analysis) on the full diff.
    8. Hand off to Documentation Sync Agent with the impact-driven payload.
    9. Hand off to `finishing-a-development-branch`.
```

### 3. Implementer status handling

| Status | Action |
|---|---|
| **DONE** | Proceed to spec-compliance review. |
| **DONE_WITH_CONCERNS** | Read concerns. If correctness/scope issue, address before review. If observation ("this file is getting large"), record and proceed. |
| **NEEDS_CONTEXT** | Provide the missing context. Re-dispatch the same subagent. |
| **BLOCKED** | Diagnose: context gap → provide and retry; reasoning gap → upgrade model and retry; task too large → split into sub-tasks; plan is wrong → escalate to user. |

Never silently retry the same subagent with the same prompt after BLOCKED.

## Subagent dispatch contract

Every dispatch must provide the subagent:

1. **Role**: which Mnemosyne agent it is acting as (Testing, Implementation, Static Analysis, …).
2. **Task scope**: the full task text from the plan, copied — do not tell the subagent to "read task N from the plan".
3. **Files allowed**: explicit allow-list. Subagents must not edit anything outside it.
4. **Files non-scope**: explicit deny-list of nearby-but-untouched files.
5. **Required commands**: cargo invocations to run with expected output.
6. **Return format**: structured handoff per [docs/agent-workflow.md](../../../docs/agent-workflow.md).

The orchestrator constructs every dispatch from the plan; subagents do not inherit the controller's session.

## Model selection

Use the least-capable model that can handle the task:

- **Mechanical implementation** (1–2 files, complete spec, clear test): cheap/fast model.
- **Integration tasks** (multi-file, pattern-matching across modules): standard model.
- **Architecture-touching, design-judgment, broad codebase reasoning**: most-capable model.

Mnemosyne signals to upgrade model:
- Task touches `core/src/hprof/` parser internals.
- Task changes module boundaries.
- Task introduces a new public type that other tasks consume.

## Red flags

**Never:**
- Skip either review stage.
- Run code-quality review before spec-compliance is ✅.
- Dispatch multiple Implementation subagents in parallel on overlapping files.
- Make a subagent read the plan file (always provide full task text).
- Forward your controller-session history to a subagent.
- Accept "close enough" on spec compliance.
- Mark a task complete with reviewer issues open.
- Insert a human approval prompt between tasks (this is the autonomy contract).

**If reviewer finds issues:**
- Same Implementation subagent fixes them (preserves task context).
- Same reviewer reviews again.
- Repeat until approved.
- Do not skip the re-review.

**If a subagent fails three times:**
- Stop. Escalate to the user with: failed task, what was tried, suspected root cause.
- Do not paper over with manual fixes from the controller.

## Final-batch handoff

After the last task's reviews pass:

```
BATCH COMPLETE: <plan-file>
TASKS COMPLETED: N / N
COMMITS: <list of SHAs>
TESTS: cargo test --workspace ✅
LINT: cargo clippy --workspace -- -D warnings ✅
NEXT: Documentation Sync Agent (impact-driven), then finishing-a-development-branch.
```
