# Agent Workflow

This document defines the execution model for Mnemosyne's multi-agent system.

Custom agent definitions live in [.github/agents](../.github/agents) and use the `.agent.md` format for VS Code discovery.

## Controller Model

The Orchestration Agent is the only controller. All other agents are sub-agents.

The orchestrator is responsible for:
- task decomposition
- sub-agent assignment and routing
- tool grant decisions (minimum required per task)
- file ownership declaration before edits begin
- non-scope protection for each batch
- parallel vs sequential decisions
- dependency ordering
- result consolidation
- implementation readiness decisions
- fail-fast when runtime capability is insufficient

The orchestrator must not become the default coder.

## Approved Batch Execution Order

Once a batch is approved, this is the only valid execution order:

1. **Scoped decomposition** — orchestrator breaks the work into sub-tasks
2. **Owner assignment** — orchestrator declares file ownership per sub-task
3. **Tool grants** — orchestrator assigns minimum tools per agent
4. **Edits** — Implementation Agent (or the explicitly assigned execution agent) performs changes
5. **Tests** — Testing Agent validates after edits
6. **Static analysis** — Static Analysis Agent performs final risk pass after tests
7. **Consolidation** — orchestrator merges results and decides next actions

Any deviation from this order requires explicit orchestration justification.

## Agent Routing Rules

| Task type | Default owner |
|---|---|
| Business logic coding | Implementation |
| Shared model changes | Implementation (with API/DB review if needed) |
| API contract docs/schemas | API Contract |
| API contract runtime code | Implementation |
| Database or persistence changes | Database Migration |
| Test work | Testing |
| Lint/build diagnosis | Static Analysis |
| Design review | Architecture Review |
| Logs/metrics/tracing | Observability |
| Cleanup after correctness is stable | Refactor |

Review agents must not become implementation owners unless orchestration explicitly reassigns ownership with justification.

## Runtime Capability Check

Before assigning execution work, the orchestrator must confirm:
- **Write-enabled?** Can the runtime edit files directly?
- **Execute-enabled?** Can the runtime run terminal, build, test, lint?

If implementation is requested but write capability is unavailable:
- stop immediately
- name the missing capability and blocked task
- do not fall back to patch-only mode unless the user explicitly asked for patches
- do not restart analysis to fill the gap

## Tool Governance

| Role | Default access |
|---|---|
| Architecture Review | read only |
| Static Analysis | read + execute (diagnostics) |
| API Contract | read only; write only when docs/schemas are explicitly assigned |
| Database Migration | read only; write + execute only for approved persistence work |
| Implementation | read + write; execute only when explicitly needed |
| Testing | read + execute; write only for test files |
| Observability | read only; write only for approved instrumentation |
| Refactor | read only; write only after correctness is stable |

Tools are granted per task, not permanently.

## File Ownership Rules

Before any edit begins, orchestration must declare:
- affected files/modules
- owning sub-agent
- parallel safety
- dependency order
- non-scope protections

Rules:
- no two writing agents on the same file simultaneously
- ownership must be explicitly transferred before a follow-up agent edits the same file
- review-only agents do not gain edit authority by reading a file

## Mandatory Handoff Contract

Every sub-agent handoff must include exactly:

1. **Task received**
2. **Scope**
3. **Non-scope**
4. **Files inspected**
5. **Files owned**
6. **Changes made or validation performed**
7. **Risks/blockers**
8. **Follow-up required**
9. **Recommended next agent**

Do not omit sections. Write `None` when a section is empty.

## Non-Scope Protection

- Every batch names its non-scope files and modules.
- Agents must not expand scope because a nearby issue looks related.
- If scope expansion is needed, the agent stops and returns to orchestration for re-scoping.

## Batch Discipline

Once a scoped batch is approved:
- agents must not rerun full repository analysis
- agents must stay within declared scope
- implementation requests must not degrade into planning when the runtime can execute
- review agents must not bounce approved work back into broad re-analysis

## Parallelism Rules

### May run in parallel
- architecture review
- API review
- database review
- observability review
- testing gap analysis
- static analysis review

### Must be sequential
- shared model edits
- core business logic edits
- shared interface changes
- coupled DB + service changes
- multiple edits to the same file
- testing after implementation edits
- static analysis after testing
- refactor after correctness is stable

### Forbidden combinations
- two agents editing the same file
- Implementation + Refactor on the same module
- Implementation + API Contract changing the same runtime file simultaneously

## Merge Readiness Checklist

- batch scope completed
- file ownership respected throughout
- tests run (or explicitly waived with documented reason)
- static analysis run (or explicitly waived with documented reason)
- contracts aligned
- no unresolved blockers
- no overlapping edits
- final consolidation complete
