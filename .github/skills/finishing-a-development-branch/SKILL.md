---
name: finishing-a-development-branch
description: "Use when all tasks in a Mnemosyne plan are complete and reviewed, to close the batch. Runs final verification (cargo check/test/clippy/fmt), updates CHANGELOG/STATUS where appropriate, validates commit-message style, and presents merge/PR/keep options."
---

# Finishing a Development Branch (Mnemosyne)

## Purpose

The last step in the [subagent-driven-development](../subagent-driven-development/SKILL.md) flow. Confirms the batch is genuinely done, tidies the working tree, and hands the user a clear set of next-step options without making remote changes silently.

## When to use

- Every task in the active plan has reviewer approvals.
- Documentation Sync Agent has run and reported its updates.
- Static Analysis Agent has run a final-batch review on the consolidated diff.

## When NOT to use

- Mid-batch — finishing is a single-shot at the end.
- A single direct user edit (no plan, no batch — the work is just done).

## Procedure

### 1. Final verification gate

Run the [verification-before-completion](../verification-before-completion/SKILL.md) checklist on the **whole batch**, not just the last task:

```powershell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

All four must pass. If any fail, do not proceed — return to the failing task.

### 2. Diff inspection

```powershell
git status
git diff --stat <batch-base>..HEAD
git log --oneline <batch-base>..HEAD
```

Confirm:
- Every file changed is in the plan's File Structure table.
- No accidental edits (e.g. `target/`, `.idea/`, `.vscode/settings.json`, scratch files).
- Commit messages follow the Mnemosyne style guide (see [.github/copilot-instructions.md](../../copilot-instructions.md) — Greek-mythology / memory / heap-pun friendly, but the actual change description must be clear).

### 3. CHANGELOG / STATUS sync

If the batch:
- Adds, removes, or changes a user-visible feature → confirm Documentation Sync updated [CHANGELOG.md](../../../CHANGELOG.md).
- Completes a milestone or moves a milestone forward → confirm Documentation Sync updated [STATUS.md](../../../STATUS.md) and the relevant entry in [docs/roadmap.md](../../../docs/roadmap.md).
- Touches a public surface (CLI, MCP, config, report) → confirm Documentation Sync updated [docs/api.md](../../../docs/api.md), [README.md](../../../README.md), and any `docs/QUICKSTART.md` or `docs/user-guide.md` entries.

If any of these are missing, hand back to Documentation Sync before continuing. Do not patch docs from this skill.

### 4. Run [GitNexus](../../../AGENTS.md) impact and change detection

```text
gitnexus_detect_changes({scope: "all"})
```

Confirm the affected scope matches the plan. If GitNexus reports affected symbols outside the plan, surface it. The plan may have under-specified the file structure.

### 5. Present the user with options

Surface the batch summary and let the user pick. **Do not** push, force-push, merge, or open PRs without an explicit choice.

```markdown
## Batch ready to land

**Plan:** docs/superpowers/plans/YYYY-MM-DD-<feature>.md
**Tasks:** N / N completed
**Commits:** <list of SHAs>
**Tests:** 414 passed, 0 failed, 7 ignored
**Lint:** clean
**Format:** clean
**Docs:** synced (CHANGELOG, STATUS, api.md updated)

### Options

1. **Open a PR** (you commit the GitHub Ops Agent dispatch).
2. **Push the branch** to origin without opening a PR.
3. **Merge to main locally** (only if the user explicitly opts into local merges).
4. **Keep the branch local** for further iteration in a follow-up batch.
5. **Discard** (revert all batch commits — destructive; require explicit confirmation).

Which option?
```

### 6. Honor the choice

| Choice | Action |
|---|---|
| 1 — Open a PR | Hand off to GitHub Ops Agent with commit list, summary, and review checklist. Do not bypass `gh` / GitHub MCP — explicit user action goes through that agent. |
| 2 — Push branch | Run `git push -u origin <branch>` only after re-confirming with the user. Never `--force` without explicit user opt-in. |
| 3 — Local merge | Run the merge in the user's terminal; do not push. |
| 4 — Keep local | Stop. Next batch picks up. |
| 5 — Discard | Require typed confirmation. Use `git reset --hard <batch-base>` or `git revert` per user preference. Never run `--no-verify` or destructive flags as a shortcut. |

### 7. Close the loop

Update the in-flight TODO list (`manage_todo_list`) so the batch is marked complete. Final report to the user:

```markdown
## Batch complete

- Plan: <path>
- Outcome: <PR opened #N | branch pushed | local merge done | keeping local | discarded>
- Tests: <counts>
- Follow-ups logged: <list, if any>
```

## Safety guarantees

- **No silent pushes.** No remote write happens without an explicit user choice in step 5.
- **No `--force`.** Force-push requires the user to type the option name and confirm again.
- **No `--no-verify`.** Pre-commit hooks must run.
- **No discarding the working tree** until the user types confirmation. Discard is destructive.
- **No auto-opened PRs** before tests and lint are clean.

## Anti-patterns

- Skipping the final cargo check/test/clippy/fmt because individual tasks already passed.
- Merging "while we're here" doc fixes that weren't in the plan.
- Treating the user choice prompt as optional.
- Pushing without confirming branch name and target remote.
- Closing the TODO list without the final report.
