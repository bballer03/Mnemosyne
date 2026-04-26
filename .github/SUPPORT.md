# Getting Help with Mnemosyne

Pick the channel that matches what you need.

## I think I found a bug

- **Heap-dump bug** (parser failures, wrong leak/dominator results, OQL crashes) → [open a Heap Dump Bug issue](https://github.com/bballer03/Mnemosyne/issues/new?template=heap_dump_bug.yml)
- **Other bug** (CLI crash, MCP error, install failure, regression) → [open a Bug Report](https://github.com/bballer03/Mnemosyne/issues/new?template=bug_report.yml)

Before filing: check [open and closed issues](https://github.com/bballer03/Mnemosyne/issues?q=is%3Aissue) and confirm you are on the latest release or recent `main`.

## I want to propose a new capability

[Open a Feature Request](https://github.com/bballer03/Mnemosyne/issues/new?template=feature_request.yml). Include the user problem, sketched UX, and acceptance criteria. Cross-check against [`docs/roadmap.md`](../docs/roadmap.md) first.

## I have a question or want to discuss design

Use [GitHub Discussions](https://github.com/bballer03/Mnemosyne/discussions) — not the issue tracker.

## I think I found a security vulnerability

**Do NOT open a public issue.** Use [GitHub Security Advisories](https://github.com/bballer03/Mnemosyne/security/advisories/new). See [`SECURITY.md`](../SECURITY.md) for full policy and supported versions.

## I want to contribute code

1. Read [`ARCHITECTURE.md`](../ARCHITECTURE.md), [`STATUS.md`](../STATUS.md), [`docs/roadmap.md`](../docs/roadmap.md), and [`docs/agent-workflow.md`](../docs/agent-workflow.md).
2. Check the [agent workflow](copilot-instructions.md) — Mnemosyne enforces a pre-coding design gate, mandatory tests, and structured handoffs.
3. Open a small PR. Fill the [PR template](PULL_REQUEST_TEMPLATE.md) end-to-end — risk, contract, performance, rollback all matter.

## I want to know what's coming next

- Roadmap: [`docs/roadmap.md`](../docs/roadmap.md)
- Status snapshot: [`STATUS.md`](../STATUS.md)
- Recent changes: [`CHANGELOG.md`](../CHANGELOG.md)
