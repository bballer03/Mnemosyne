# Custom Agents

This folder contains workspace custom agents for VS Code Copilot.

- Files end in `.agent.md` so VS Code can discover them as custom agents.
- These agents are grounded by [.github/copilot-instructions.md](../copilot-instructions.md), [AGENTS.md](../../AGENTS.md), and [docs/agent-workflow.md](../../docs/agent-workflow.md).
- Keep agent scopes narrow and avoid overlapping file ownership.
- The Orchestration agent assigns file ownership and grants only the minimum tool access required for each task.
- Review agents should default to read-only work until orchestration explicitly approves an edit phase.
- The **Implementation Agent** is the default owner for coding tasks and terminal-based validation.
- The **GitHub Ops Agent** owns CI/CD, GitHub Actions, PR/issue, and workflow investigation tasks.
- Agents must verify runtime tool availability before execution. Report unavailable tools rather than failing silently.
- At each orchestration stage, report using: ACTIVE AGENTS, TOOLS GRANTED, PARALLEL TASKS, FILE OWNERSHIP, RESULTS SUMMARY, NEXT ACTIONS.