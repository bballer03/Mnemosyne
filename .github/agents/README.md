# Mnemosyne Custom Agents

Workspace custom agents for VS Code Copilot. Each `*.agent.md` file declares one agent's role, allowed tools, and handoff targets.

> **Grounded by:** [copilot-instructions.md](../copilot-instructions.md) · [AGENTS.md](../../AGENTS.md) · [docs/agent-workflow.md](../../docs/agent-workflow.md)

## Roster

| Agent | File | Default model | Primary role |
|---|---|---|---|
| Orchestration | [orchestration.agent.md](orchestration.agent.md) | Claude Opus 4.7 | Plans, routes, owns tool grants. Never codes. |
| Design Consulting | [design-consulting.agent.md](design-consulting.agent.md) | Claude Opus 4.7 | Pre-coding design gate; owns `docs/design/`. |
| Architecture Review | [architecture-review.agent.md](architecture-review.agent.md) | Claude Opus 4.7 | Read-only design review. |
| Tech PM | [tech-pm.agent.md](tech-pm.agent.md) | Claude Opus 4.7 | Roadmap, milestones; owns `docs/roadmap.md`. |
| Implementation | [implementation.agent.md](implementation.agent.md) | GPT-5.4 | Default owner for source-code edits + terminal validation. |
| Testing | [testing.agent.md](testing.agent.md) | GPT-5.4 | Adds/runs tests after implementation. |
| Static Analysis | [static-analysis.agent.md](static-analysis.agent.md) | GPT-5.4 | `cargo clippy` + `cargo fmt --check` post-test risk pass. |
| Security | [security.agent.md](security.agent.md) | GPT-5.4 | Audit-only by default; remediation only when explicitly approved. |
| API Contract | [api-contract.agent.md](api-contract.agent.md) | GPT-5.4 | CLI/MCP/config/report contract alignment. |
| Documentation Sync | [documentation-sync.agent.md](documentation-sync.agent.md) | GPT-5.4 | Impact-driven doc updater. |
| GitHub Ops | [github-ops.agent.md](github-ops.agent.md) | GPT-5.4 | CI/CD, workflows, PR/issue/branch state. |
| Refactor | [refactor.agent.md](refactor.agent.md) | GPT-5.4 | Cleanup-only after correctness is stable. |
| Observability | [observability.agent.md](observability.agent.md) | GPT-5.4 | Tracing/logging/metrics without semantic change. |
| Database Migration | [database-migration.agent.md](database-migration.agent.md) | GPT-5.4 | Reserved for future persistence layer. |

> Model defaults follow the workspace preference: GPT-5.4 for routine/low-cost tasks; Claude Opus 4.7 reserved for heavy reasoning (orchestration, design, architecture, roadmap).

## Authoring conventions

- Filename: `<kebab-case>.agent.md`. The `name:` frontmatter is the canonical display name.
- Every agent file MUST contain frontmatter with: `name`, `description`, `argument-hint`, `tools`, `model`, `target`.
- Tool entries use a single, consistent style across the file (either `read/foo` namespaced OR bare `foo` — never mix in one file).
- Every handoff entry names the receiving agent and a one-sentence prompt.

## Operating invariants

1. **Single controller.** Orchestration is the only agent that decomposes tasks and assigns file ownership.
2. **Read-only by default.** Review-class agents (Architecture Review, Static Analysis, Security in audit mode) never edit unless orchestration explicitly grants `editFiles`.
3. **No file-ownership overlap.** Two writing agents cannot own the same file in the same batch.
4. **Mandatory 9-field handoff.** Every sub-agent returns: task received, scope, non-scope, files inspected, files owned, changes made, risks/blockers, follow-up, recommended next agent.
5. **Fail fast on missing tools.** If a required runtime capability (terminal, write, MCP) is unavailable, the agent reports it and stops — never silently degrades to patch-only.
6. **Skill discipline.** When an agent's `## Skills Used` lists a skill in [`.github/skills/`](../skills/), that skill is followed verbatim.

## Reporting format

At each orchestration stage, use this structured block:

```
ACTIVE AGENTS:    <list>
TOOLS GRANTED:    <agent → tools>
PARALLEL TASKS:   <which tasks run together>
FILE OWNERSHIP:   <agent → files>
RESULTS SUMMARY:  <one-line per task>
NEXT ACTIONS:     <handoff targets + reason>
```
