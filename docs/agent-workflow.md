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

1. **Design gate** — Orchestrator invokes the Design Consulting Agent to inspect roadmap, review existing design docs, create or update the relevant design artifact, link it in `roadmap.md`, and return an implementation readiness verdict (**READY** / **READY AFTER DOC UPDATE** / **BLOCKED UNTIL DESIGN COMPLETES**). If blocked, stop here.
2. **Scoped decomposition** — orchestrator breaks the work into sub-tasks
3. **Owner assignment** — orchestrator declares file ownership per sub-task
4. **Tool grants** — orchestrator assigns minimum tools per agent
5. **Edits** — Implementation Agent (or the explicitly assigned execution agent) performs changes, using the design doc as the source of truth
6. **Tests** — Testing Agent validates after edits
7. **Static analysis** — Static Analysis Agent performs final risk pass after tests
8. **Documentation sync** — Documentation Sync Agent receives the impact-driven handoff payload and auto-determines which docs to update
9. **Consolidation** — orchestrator merges results and decides next actions

Any deviation from this order requires explicit orchestration justification.

### Post-security-remediation execution order
After security remediation is approved and applied:
1. **Security Agent** applies approved fixes (remediation mode) or hands off code changes to **Implementation Agent**
2. **Testing Agent** runs `cargo check` + `cargo test` to validate remediation
3. **Static Analysis Agent** runs `cargo clippy` for post-fix risk pass
4. **Documentation Sync Agent** receives the impact-driven handoff payload and auto-determines which docs to update
5. **Security Agent** (optional) confirms findings are resolved via follow-up audit

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
| Pre-coding design gate | Design Consulting |
| Technical design creation / milestone design docs | Design Consulting |
| Architecture/design doc ownership | Design Consulting |
| Design review | Architecture Review |
| Logs/metrics/tracing | Observability |
| Cleanup after correctness is stable | Refactor |
| Security audit / review | Security |
| Dependency vulnerability review | Security (review) → Implementation (upgrade) |
| Workflow / CI security review | Security |
| Approved security remediation | Security (review + approve) → Implementation (code fixes) |
| Secret / credential scanning | Security |

### Security routing rules
- Security audits and vulnerability reviews always route to the **Security Agent** first.
- The Security Agent owns the review. The Implementation Agent owns code fixes if remediation is approved.
- If approved remediation changes user-visible behavior or security guidance, hand off to **Documentation Sync Agent**.
- If a security fix changes module boundaries or trust boundaries, consult **Architecture Review Agent** before implementation.

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

If security remediation is requested but `editFiles` is unavailable:
- stop immediately
- name the missing capability (`editFiles` / write access) and blocked task
- do not fall back to patch-only mode unless the user explicitly asked for patches

If validation is required after remediation but terminal tools are unavailable:
- report the limitation
- do not claim the remediation is validated

## Tool Governance

| Role | Default access |
|---|---|
| Design Consulting | read + write for architecture/design docs (`ARCHITECTURE.md`, `docs/design/*`, `docs/roadmap.md` design refs, `STATUS.md` design sections) |
| Architecture Review | read only |
| Static Analysis | read + execute (diagnostics), terminal (cargo clippy, cargo fmt --check, cargo check) |
| API Contract | read only; write only when docs/schemas are explicitly assigned |
| Database Migration | read only; write + execute only for approved persistence work |
| Implementation | read + write; execute only when explicitly needed |
| Testing | read + execute; write only for test files |
| Observability | read only; write only for approved instrumentation |
| Refactor | read only; write only after correctness is stable |
| Documentation Sync | read + write for docs only; operates in impact-driven mode (auto-selects impacted docs) |
| Security | read + search + codebase + changes + usages; write (editFiles) only in remediation mode when explicitly approved |

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
- design consulting (read-only inspection phase)
- API review
- database review
- observability review
- testing gap analysis
- static analysis review
- security audit (read-only mode)

### Must be sequential
- design gate before any implementation edits
- shared model edits
- core business logic edits
- shared interface changes
- coupled DB + service changes
- multiple edits to the same file
- testing after implementation edits
- static analysis after testing
- refactor after correctness is stable
- security remediation followed by testing then static analysis

### Forbidden combinations
- two agents editing the same file
- Implementation + Refactor on the same module
- Implementation + API Contract changing the same runtime file simultaneously
- Security remediation + Implementation on the same file without explicit ownership transfer

## Merge Readiness Checklist

- batch scope completed
- file ownership respected throughout
- tests run (or explicitly waived with documented reason)
- static analysis run (or explicitly waived with documented reason)
- contracts aligned
- no unresolved blockers
- no overlapping edits
- final consolidation complete
