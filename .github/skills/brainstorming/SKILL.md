---
name: brainstorming
description: "Use when starting a new feature, milestone, or non-trivial change in Mnemosyne and a spec does not yet exist. Refines a rough idea into a concrete spec document via Socratic questions and saves it to docs/superpowers/specs/."
---

# Brainstorming (Mnemosyne)

## Purpose

Turn a rough request ("I want X") into a spec document concrete enough to feed `writing-plans`. This skill is invoked by the **Design Consulting Agent** before any milestone- or feature-level coding begins.

## When to use

- A new feature is requested.
- A milestone in [docs/roadmap.md](../../../docs/roadmap.md) lacks a design reference.
- A change spans multiple modules (parser + analysis + reporting + MCP, etc.).
- A user request is ambiguous about scope, error handling, or output format.

## When NOT to use

- A milestone design doc under [docs/design/](../../../docs/design/) already covers the work.
- A trivial bug fix (use `tdd-cycle` directly).
- A pure cleanup or refactor (use `refactor` agent).
- Format-only changes.

## Procedure

### 1. Ground the conversation in repo state

Before asking the user anything, read:
- [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- [STATUS.md](../../../STATUS.md)
- [docs/roadmap.md](../../../docs/roadmap.md)
- Any existing design doc under [docs/design/](../../../docs/design/) that touches the affected area.

State to the user, in one paragraph, the current relevant capability and any known gaps. This anchors brainstorming in real code, not assumed code.

### 2. Ask ≤ 5 high-leverage questions

Pick from this menu — only the ones the request leaves ambiguous:

- **Scope**: What inputs? What outputs? What is explicitly out of scope?
- **Failure modes**: What happens with malformed heap dumps, oversized files, missing classes?
- **Public surface**: New CLI flag? New MCP tool? New config field? New report section?
- **Performance budget**: Memory ceiling? Wall-clock target? Streaming vs in-memory?
- **Compatibility**: Does this break any existing CLI/MCP contract or report shape?
- **Observability**: What new tracing spans, metrics, or error variants are needed?
- **Validation strategy**: What test fixtures exist? Do new ones need to be generated via [scripts/generate_synthetic_heap.sh](../../../scripts/generate_synthetic_heap.sh)?

**Do not** ask questions whose answers are obvious from the repo. **Do not** chunk and ask for sign-off on each section — produce the whole spec, then proceed.

### 3. Draft the spec

Save to `docs/superpowers/specs/YYYY-MM-DD-<feature-name>.md` with this structure:

```markdown
# <Feature Name> Spec

**Date:** YYYY-MM-DD
**Author:** <agent name>
**Related milestone:** <link to docs/design/<milestone>.md or roadmap entry>
**Status:** Draft → Ready-for-plan

## Problem
1–3 sentences. What does the user / agent need to do that they currently cannot?

## Goals
- Bullet list of observable outcomes when this lands.

## Non-goals
- Bullet list of things we will *not* do, with one-line reasons.

## Current state
What the codebase does today in the affected modules, with file links.

## Proposed behavior
The new behavior, described in user-visible terms (CLI output shape, MCP tool schema, report section, error variant).

## Public-surface impact
| Surface | Change |
|---|---|
| CLI flags | … |
| MCP tools | … |
| Config keys | … |
| Report sections | … |
| Error variants | … |

## Module / file impact
List of files expected to be created or modified, with one-line responsibility each.

## Validation strategy
- Unit tests (modules, what behaviors)
- Integration tests (which `tests/` files, what fixtures)
- Performance / scaling tests if applicable
- Existing regressions to preserve

## Open questions
Concrete questions to resolve before plan-writing. Empty list is acceptable.

## Risks
- Risk → mitigation, one line each.
```

### 4. Hand off

After saving the spec, return to the orchestrator with:

```
SPEC READY: docs/superpowers/specs/YYYY-MM-DD-<feature>.md
NEXT: Design Consulting Agent decides whether the spec needs a milestone design doc update,
      then orchestration invokes writing-plans to break it into bite-sized tasks.
```

Do not wait for a per-section human approval. The orchestrator owns the next gate.

## Anti-patterns

- **Asking the user 12 questions.** Cap at 5; infer the rest from the codebase.
- **Treating the spec as the design doc.** Specs describe *what*, design docs describe *how at the architecture level*. The Design Consulting Agent decides if a milestone design doc also needs an update.
- **Skipping the "Current state" section.** Without it, the spec drifts into intended-vs-actual confusion.
- **Open-ended "TBD" sections.** If genuinely unknown, list it under **Open questions**, not in the body.
- **Chunking the spec for sign-off.** Mnemosyne brainstorming is single-pass; the user reviews the saved doc.
