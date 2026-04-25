# Mnemosyne Skills Library

Skills are **mandatory workflow gates** that Mnemosyne agents invoke during their work. Each skill is a self-contained procedure with bundled context. Agents reference required skills in their `## Skills Used` section and must follow them when triggered.

## What lives here

Each subfolder is one skill with a `SKILL.md`. The frontmatter `description: "Use when ..."` is the trigger phrase — when the matching condition occurs, the agent must load and follow the skill.

## Skill index

| Skill | Trigger (when to use) | Owning lifecycle stage |
|---|---|---|
| [brainstorming](brainstorming/SKILL.md) | Use when starting a new feature or non-trivial change and a spec does not yet exist. | Pre-design |
| [writing-plans](writing-plans/SKILL.md) | Use when a spec or design doc exists and a multi-step implementation needs a bite-sized task plan. | Pre-implementation |
| [tdd-cycle](tdd-cycle/SKILL.md) | Use when implementing any new behavior or fixing any bug in `core/`, `cli/`, or `tauri/`. | Implementation |
| [subagent-driven-development](subagent-driven-development/SKILL.md) | Use when executing an approved plan with two or more independent tasks. | Implementation |
| [systematic-debugging](systematic-debugging/SKILL.md) | Use when investigating a failing test, panic, regression, or unexplained behavior. | Debugging |
| [verification-before-completion](verification-before-completion/SKILL.md) | Use before any agent declares a task DONE. | Quality gate |
| [requesting-code-review](requesting-code-review/SKILL.md) | Use before handing off changes to a reviewer agent. | Handoff |
| [receiving-code-review](receiving-code-review/SKILL.md) | Use when responding to review findings from another agent. | Handoff |
| [finishing-a-development-branch](finishing-a-development-branch/SKILL.md) | Use when all plan tasks are complete and the batch is ready to land. | Wrap-up |

## Lifecycle (Mnemosyne batch flow)

```
Brainstorm  →  Design Gate  →  Write Plan  →  Subagent-driven loop  →  Verify  →  Finish
   ↓                ↓               ↓                  ↓                   ↓          ↓
brainstorming  (Design Consulting) writing-plans  subagent-driven-     verification- finishing-a-
  + spec doc                                       development         before-       development-
                                                  + tdd-cycle          completion    branch
                                                  + requesting/                       
                                                  receiving-code-                     
                                                  review                              
                                                  + systematic-                       
                                                  debugging (when                     
                                                  stuck)                              
```

## Mandatory invariants

1. **No production Rust without a failing test first.** `tdd-cycle` enforces this. Code written before the test is reverted.
2. **No DONE without verification.** `verification-before-completion` runs before any agent reports completion.
3. **Plan tasks run autonomously.** `subagent-driven-development` does **not** insert human checkpoints between tasks. Only hard blockers escalate.
4. **Skills are not suggestions.** When an agent's `## Skills Used` section lists a skill, that skill must be loaded and followed for the matching task.

## Authoring conventions

- `name` must match the folder name (kebab-case, lowercase).
- `description` starts with `"Use when ..."` so triggers are discoverable.
- Keep each `SKILL.md` under 500 lines. Move long examples to `references/` siblings.
- Reference repo files with workspace-relative markdown links, not bare paths.
- Mention the actual cargo / git / repo commands the agent should run.

## Storage of artifacts

| Artifact | Location |
|---|---|
| Specs from `brainstorming` | `docs/superpowers/specs/YYYY-MM-DD-<feature>.md` |
| Plans from `writing-plans` | `docs/superpowers/plans/YYYY-MM-DD-<feature>.md` |
| Reviews from `requesting/receiving-code-review` | `docs/superpowers/reviews/YYYY-MM-DD-<feature>.md` |
| Milestone design docs (Design Consulting) | `docs/design/<milestone>.md` |
