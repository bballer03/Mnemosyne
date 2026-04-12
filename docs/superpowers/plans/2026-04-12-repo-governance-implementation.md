# Repository Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Protect `main` with a solo-maintainer-friendly open source baseline and align contributor documentation with the enforced GitHub policy.

**Architecture:** This slice has two outputs: remote GitHub branch-protection state and one local documentation edit. Use `.github/workflows/ci.yml` as the source of truth for required check names, apply protection to `main` through GitHub's branch-protection API, then update `CONTRIBUTING.md` so the written PR policy matches the actual enforcement.

**Tech Stack:** GitHub branch protection API, GitHub CLI (`gh`), GitHub MCP tools, Markdown, PowerShell

---

## File Map

- `docs/superpowers/specs/2026-04-12-repo-governance-design.md`
  - Approved design reference for this batch. Read-only unless the user changes requirements.
- `.github/workflows/ci.yml`
  - Source of truth for the required status-check names: `Build & Test` and `Docker Build Validation`.
- `CONTRIBUTING.md`
  - Contributor-facing PR and review policy. This file must match the enforced GitHub settings after the branch protection change lands.
- Remote GitHub state: `bballer03/Mnemosyne` branch protection for `main`
  - Remote configuration to apply with GitHub API access. This is not a repository file, but it is part of the deliverable.

## Validation Model

This slice does not change runtime code, so there is no `cargo test` task here. Validation comes from:

- exact workflow-job name checks in `.github/workflows/ci.yml`
- GitHub API readback of the applied branch protection
- `git diff` review of `CONTRIBUTING.md`

### Task 1: Preflight GitHub Access And Baseline State

**Files:**
- Inspect: `.github/workflows/ci.yml:15-61`
- Inspect: `CONTRIBUTING.md:416-421`
- Inspect remote state: `bballer03/Mnemosyne` `main`

- [ ] **Step 1: Confirm the exact required-check names from the workflow file**

```powershell
Select-String -Path ".github/workflows/ci.yml" -Pattern "name: Build & Test|name: Docker Build Validation"
```

Expected: two matches, one for `Build & Test` and one for `Docker Build Validation`.

- [ ] **Step 2: Confirm `gh` is available in the active shell**

```powershell
Get-Command gh -ErrorAction Stop | Select-Object -ExpandProperty Source
```

Expected: an absolute path to `gh.exe`.

If this fails because the shell session predates the install, start a fresh shell session and rerun the command. If `gh` is still unavailable but GitHub MCP tools are available, use the MCP tools for the remote-state steps instead of trying to repair PATH manually.

- [ ] **Step 3: Confirm GitHub authentication works before making changes**

```powershell
gh auth status
```

Expected: authenticated account on `github.com` with no auth error.

- [ ] **Step 4: Confirm `main` is currently unprotected**

```powershell
gh api "repos/bballer03/Mnemosyne/branches/main" --jq "{name: .name, protected: .protected}"
```

Expected: `{"name":"main","protected":false}`.

### Task 2: Apply Branch Protection To `main`

**Files:**
- Inspect: `.github/workflows/ci.yml:15-61`
- Modify remote state: `bballer03/Mnemosyne` `main`

- [ ] **Step 1: Apply the exact branch-protection payload**

```powershell
@'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Build & Test",
      "Docker Build Validation"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": false,
    "require_code_owner_reviews": false,
    "required_approving_review_count": 0,
    "require_last_push_approval": false
  },
  "restrictions": null,
  "required_linear_history": false,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": true
}
'@ | gh api --method PUT "repos/bballer03/Mnemosyne/branches/main/protection" --input -
```

Expected: JSON response showing branch-protection settings for `main`.

- [ ] **Step 2: Read back the effective protection summary**

```powershell
gh api "repos/bballer03/Mnemosyne/branches/main/protection" --jq "{strict: .required_status_checks.strict, contexts: .required_status_checks.contexts, enforce_admins: .enforce_admins.enabled, required_approving_review_count: .required_pull_request_reviews.required_approving_review_count, required_conversation_resolution: .required_conversation_resolution.enabled, allow_force_pushes: .allow_force_pushes.enabled, allow_deletions: .allow_deletions.enabled}"
```

Expected:

```json
{
  "strict": true,
  "contexts": [
    "Build & Test",
    "Docker Build Validation"
  ],
  "enforce_admins": true,
  "required_approving_review_count": 0,
  "required_conversation_resolution": true,
  "allow_force_pushes": false,
  "allow_deletions": false
}
```

- [ ] **Step 3: Confirm `main` now reports as protected**

```powershell
gh api "repos/bballer03/Mnemosyne/branches/main" --jq "{name: .name, protected: .protected}"
```

Expected: `{"name":"main","protected":true}`.

### Task 3: Align `CONTRIBUTING.md` With The Enforced Policy

**Files:**
- Modify: `CONTRIBUTING.md:416-421`

- [ ] **Step 1: Replace the review-process block with the exact solo-maintainer-safe wording**

```markdown
### Review Process

1. **Automated checks** must pass (CI/CD)
2. **All pull request conversations** must be addressed or resolved
3. **Conflicts resolved** with main branch
4. **Protected main workflow:** changes land through pull requests; approval requirements may be tightened when multiple maintainers are active
```

- [ ] **Step 2: Read the updated section back from disk**

```powershell
Get-Content "CONTRIBUTING.md" | Select-Object -Index (415..421)
```

Expected: the old `At least one approval` line is gone and the new protected-main wording is present.

- [ ] **Step 3: Review the doc diff for scope control**

```powershell
git diff -- "CONTRIBUTING.md"
```

Expected: only the `### Review Process` bullets changed; no unrelated wording edits.

- [ ] **Step 4: If and only if the user explicitly asks for a commit, create a docs-only commit**

```powershell
git add "CONTRIBUTING.md" "docs/superpowers/specs/2026-04-12-repo-governance-design.md" "docs/superpowers/plans/2026-04-12-repo-governance-implementation.md"
git commit -m "docs: align protected-main policy with solo maintainer OSS flow"
```

Expected: one docs-only commit. Skip this step unless the user explicitly requests a commit.

### Task 4: Final Verification And Handoff

**Files:**
- Inspect: `.github/workflows/ci.yml:15-61`
- Inspect: `CONTRIBUTING.md:416-421`
- Inspect remote state: `bballer03/Mnemosyne` `main`

- [ ] **Step 1: Read back the full protection object one last time**

```powershell
gh api "repos/bballer03/Mnemosyne/branches/main/protection"
```

Expected: JSON contains the chosen `required_status_checks`, `enforce_admins`, `required_pull_request_reviews`, `required_conversation_resolution`, `allow_force_pushes`, and `allow_deletions` values.

- [ ] **Step 2: Re-read the final contributor-facing review block**

```powershell
Get-Content "CONTRIBUTING.md" | Select-Object -Index (415..421)
```

Expected: docs mention CI, resolved conversations, conflicts, and protected-main PR workflow; docs do not claim one approval is required.

- [ ] **Step 3: Record the tightening trigger in the handoff summary**

```text
When Mnemosyne gains a second active maintainer, raise `required_approving_review_count` from 0 to 1 and consider adding CODEOWNERS.
```

Expected: the final summary tells the user exactly when to tighten the policy later.
