# Mnemosyne Agent Guide

This repository uses both workspace custom agents and repository-wide agent guidance.

## Read First
1. [ARCHITECTURE.md](ARCHITECTURE.md)
2. [STATUS.md](STATUS.md)
3. [README.md](README.md)
4. [docs/agent-workflow.md](docs/agent-workflow.md)
5. the relevant custom agent in [.github/agents](.github/agents)

## Ground Rules
- Treat the current codebase as the source of runtime truth.
- Align runtime behavior back to the corrected architecture instead of documenting drift as intentional.
- Keep CLI, MCP, core types, reports, and docs synchronized.
- Do not make overlapping edits to the same file.
- Label all fallback, heuristic, partial-result, and stub behavior clearly.
- Prefer the smallest safe change set.

## Agent Locations
- Repo-wide standing instructions: [.github/copilot-instructions.md](.github/copilot-instructions.md)
- Shared workflow and handoff rules: [docs/agent-workflow.md](docs/agent-workflow.md)
- VS Code custom agents: [.github/agents](.github/agents)

## Mnemosyne Priorities
1. correctness of heap and class analysis
2. stable contracts across CLI, MCP, and docs
3. safe fallback and partial-result semantics
4. tests for real behavior
5. observability without sensitive-data leakage