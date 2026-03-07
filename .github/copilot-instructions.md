# GitHub Copilot Instructions

## Project Context

Mnemosyne is an existing Rust workspace for JVM heap analysis with:
- a CLI crate
- a shared core crate
- a stdio MCP interface
- no database or migration layer today
- partially implemented analysis, mapping, AI, and reporting features

This is **not** a greenfield project.

All work must be grounded in:
1. the current codebase
2. the completed deep review
3. the corrected architecture/design plan
4. the agent workflow in [docs/agent-workflow.md](../docs/agent-workflow.md)
5. the custom agents in [.github/agents](agents)

## Repo Priorities

1. correctness of heap/class analysis
2. stable contracts across CLI/MCP/docs
3. safe fallbacks and partial-result semantics
4. test coverage for real behavior
5. observability and lint cleanliness

## Required Review Order

Before proposing or making changes, read in this order:
1. [ARCHITECTURE.md](../ARCHITECTURE.md)
2. [STATUS.md](../STATUS.md)
3. [README.md](../README.md)
4. [docs/agent-workflow.md](../docs/agent-workflow.md)
5. your assigned custom agent in [.github/agents](agents)
6. the files listed under "Inspect First" in your assigned agent spec

---

## Agent-Enabled Execution Model

This repository uses an **orchestrated multi-agent model** where each agent has specific responsibilities, tool access, and execution capabilities.

### Key agents and their roles

| Agent | Primary role | Expected tools |
|---|---|---|
| **Implementation** | Default owner for all coding tasks. Edits source files and validates via terminal. | file edit, terminal (cargo check/test/clippy/fmt, git), codebase search, GitHub MCP (read, optional) |
| **GitHub Ops** | Owns GitHub Actions, CI/CD, PR/issue/branch, and workflow investigation. | GitHub MCP tools, terminal (git, gh CLI), codebase search, workflow file read/write |
| **Testing** | Writes and runs tests after implementation. | file edit (test files only), terminal (cargo test), codebase search |
| **Orchestration** | Routes tasks, assigns files, grants tools. Never codes. | agent dispatch, codebase search, fetch |
| **Architecture Review** | Design review only. | read only |
| **Static Analysis** | Post-test risk pass. | read + diagnostics |
| **API Contract** | Contract alignment across CLI/MCP/docs. | read; write for docs/schemas when assigned |
| **Documentation Sync** | Updates docs after successful implementation batches. | read + write for doc files only |
| **Tech PM** | Roadmap, milestones, feature planning. | read + write for `docs/roadmap.md` only |

### Routing expectations

- **Coding tasks** (edit source, fix bugs, implement features) → **Implementation Agent**
- **Build/test/lint/format execution** → **Implementation Agent** (small scope) or **Testing Agent** (broad scope)
- **CI failures, GitHub Actions issues, workflow investigation** → **GitHub Ops Agent**
- **PR/issue/branch state questions** → **GitHub Ops Agent**
- **Architecture-only review** → **Architecture Review Agent**
- **Documentation updates after code changes** → **Documentation Sync Agent**
- **Roadmap and milestone planning** → **Tech PM Agent**

### Local tool-enabled sessions

- Agent sessions running locally in VS Code with terminal access are the **preferred mode** for code edits and validation.
- The Implementation Agent should always use terminal tools (`cargo check`, `cargo test`, `cargo clippy`, `cargo fmt`) to validate changes before handoff.
- The GitHub Ops Agent should use GitHub MCP tools when available, falling back to `gh` CLI or local git inspection.
- **Agents must verify actual runtime tool availability before attempting to use them.** If a required tool is unavailable, report it and adjust approach rather than failing silently.
- Patch-only output is a fallback of last resort, only used when the user explicitly requests it.

---

## Multi-Agent Operating Model

### Controller
The Orchestration Agent is the only controller. It decomposes tasks, assigns agents, grants tools, declares ownership, sequences work, consolidates results, and decides when implementation may start. It must never become the default coder.

### Implementation ownership
Source-code edits belong to the Implementation Agent unless the task is clearly test-only, observability-only, API-docs-only, DB-only, or cleanup-only and orchestration explicitly assigns a different owner. If coding is requested and no write-capable implementation agent exists in the runtime, orchestration must fail fast.

### Review-only discipline
Architecture Review and Static Analysis agents are review-only by default. They must not take ownership of implementation work, must not bounce approved batches back into broad re-analysis, and must not produce code changes unless orchestration explicitly reassigns ownership and justifies it.

### Tool governance
- All agents get read access by default.
- Write access is granted only for the specific task.
- Execute access (terminal, build, test, lint) is granted only when the task requires it.
- Sub-agents do not automatically inherit all tools. Tools are granted per task.

### File ownership
Before any edit, orchestration must declare affected files, the owning agent, parallel safety, and dependency order. No two writing agents may own the same file at the same time. Ownership must be explicitly transferred before a follow-up agent edits the same file.

### Batch discipline
Once a scoped batch is approved by orchestration:
1. Agents must stay within the declared scope and non-scope boundaries.
2. Agents must not restart full-repo analysis unless new evidence invalidates the batch.
3. Implementation requests must not degrade into planning when the runtime can execute.
4. Execution order is: decomposition → ownership → tool grants → edits → tests → static analysis → consolidation.

### Runtime capability awareness
- Before assigning execution, orchestration must confirm the runtime has the required capability.
- If implementation is requested but write capability is unavailable, fail fast. Name the missing capability and the blocked task. Do not fall back to patch-only output unless the user explicitly asked for patches.
- If test execution is required but unavailable, report it and stop.

### Mandatory handoff contract
Every sub-agent must return exactly these fields:
1. **Task received** — the task as assigned
2. **Scope** — approved boundaries
3. **Non-scope** — protected files/modules
4. **Files inspected**
5. **Files owned** — files authorized for editing, or `Review-only` if none
6. **Changes made or validation performed**
7. **Risks/blockers**
8. **Follow-up required**
9. **Recommended next agent**

### Non-scope protection
Every batch must name its non-scope items. Agents must not expand scope because a nearby issue looks related. If scope must change, the agent stops and returns to orchestration for re-scoping.

### Execution sequencing
- **May run in parallel:** architecture review, API review, DB review, observability review, testing gap analysis, static analysis review
- **Must be sequential:** implementation edits, shared-model changes, same-file edits, testing after edits, static analysis after testing, refactor after correctness is stable
- **Forbidden:** two writing agents on the same file; Implementation + Refactor on the same module; implementation + contract review changing the same runtime file simultaneously

---

## Operating Rules

- Do not invent a new architecture unless the orchestration plan explicitly approves it.
- Preserve working behavior where possible.
- Replace only code that is incorrect, unsafe, misleading, incomplete, or incompatible with the corrected design.
- Do not start business-logic implementation until the assigned agent scope allows it.
- Do not edit files outside your assigned scope.
- Do not make parallel edits to the same file across multiple agents.
- Prefer the smallest safe change sets.
- Keep CLI, MCP, core types, and docs aligned.
- Any partial or fallback behavior must be explicitly labeled in code and docs.
- Any API/documentation update must reflect actual runtime behavior.
- Any feature gated by placeholder/stub behavior must remain clearly marked until fully implemented.
- When execution is requested and capability exists, execute. Do not answer with plans only unless planning was explicitly requested.

## Architecture Alignment Rules

- Shared core remains the source of truth for CLI and MCP behavior.
- Parser, analysis, graph, mapper, fix, AI, reporting, and MCP transport remain separate concerns.
- Runtime truth wins over stale docs, but runtime must be brought back into alignment with the corrected design.
- Record-tag summaries must not be mislabeled as real class-level analysis.
- Any future persistence or cache layer must be optional and must not become required for correctness.

## Safe Editing Rules

Before editing:
- confirm file ownership assigned by orchestration
- confirm the assigned agent is the correct execution owner for this task
- confirm there is no parallel overlap
- confirm dependency conditions are satisfied
- review the existing implementation before changing it

After editing:
- hand off to Testing Agent for test coverage
- hand off to Static Analysis Agent for risk pass
- use the mandatory handoff contract
- summarize changed files, risks, and required follow-up

## Testing and Validation Rules

- Testing runs after implementation edits, not before.
- Static analysis runs after testing, not before.
- All business-logic changes require tests or a documented reason why tests were not added.
- Contract changes require CLI/MCP/docs alignment checks.
- Fallback behavior, partial results, and error paths must be tested.
- Existing passing behavior must be preserved unless the orchestrator explicitly approves a breaking change.

## Logging and Observability Expectations

- Use `tracing` intentionally, not noisily.
- Add visibility for request lifecycle, parse phases, fallbacks, truncation budgets, and major error boundaries.
- Do not log sensitive heap contents or raw secrets.
- Observability changes must not alter business logic semantics.

## Commit Messages

When generating commit messages, keep them professional but add a touch of humor related to:
- Greek mythology (especially memory-related deities)
- Memory management puns
- Heap dump jokes
- AI/LLM references

Examples:
- "feat: Mnemosyne remembers things now (unlike my production server)"
- "fix: stopped the heap from forgetting to free itself"
- "refactor: taught the parser to remember where it left off"
- "docs: added wisdom from the goddess of memory herself"
- "perf: made heap analysis faster than Zeus's lightning bolt"

Keep it light and fun, but ensure the actual change description is clear and informative.
