# Repository Governance Design

> Status: approved in conversation; corrected during spec review for solo-maintainer compatibility
> Date: 2026-04-12
> Scope: baseline GitHub protections for public open source collaboration on `main`

## Goal

Protect `main` from direct merges, accidental history rewrites, and unreviewed breakage while preserving a normal open source fork-and-PR workflow and a workable path for a solo maintainer.

## Current State

- `bballer03/Mnemosyne` is a public repository with forks enabled and `main` as the default branch.
- `main` is currently unprotected.
- The repository already has contributor-facing GitHub hygiene in place: issue templates, a pull request template, CI, and release automation.
- `.github/workflows/ci.yml` currently publishes two merge-relevant job names that can be used as required status checks:
  - `Build & Test`
  - `Docker Build Validation`

## Options Considered

### 1. Team-style strict protection

Require pull requests, passing checks, and at least one approving review before merge.

Rejected for now. This is common for multi-maintainer repositories, but it is a bad fit for Mnemosyne today because GitHub does not count self-approval toward required approving reviews. With one maintainer, that policy can block legitimate maintenance work.

### 2. Maintainer-bypass protection

Require pull requests and passing checks for contributors, but let admins bypass the rules.

Rejected as the default. It is operationally easy, but it weakens the governance signal and does not satisfy the goal that nobody should be able to merge directly into `main`.

### 3. Solo-maintainer OSS baseline

Require pull requests, require passing checks, require resolved conversations, block direct pushes, block force-pushes, block branch deletion, and require zero approving reviews for now.

Chosen. This matches the common pattern for smaller public repositories with a single active maintainer: contributors still use forks and pull requests, `main` stays protected, and the maintainer can merge a green pull request without needing a second human who does not exist yet.

## Chosen Approach

Use GitHub branch protection on `main` as the primary enforcement mechanism.

The protection should apply to administrators too, so the repository owner cannot bypass the policy with a direct push. The owner must use a short-lived branch and merge through a pull request like everyone else.

Because Mnemosyne currently has a solo maintainer, the policy should not require approving reviews yet. Once a second active maintainer exists, the policy should be tightened to require one approving review and, if useful, CODEOWNERS coverage.

## Scope

- Configure branch protection for `main`
- Require a pull request before merge
- Require CI status checks before merge
- Require the branch to be up to date before merge
- Require conversation resolution before merge
- Apply branch protection to administrators
- Disallow force-pushes to `main`
- Disallow deletion of `main`
- Keep the existing public fork-and-PR contribution model
- Align contribution docs with the actual protection policy if the current wording is stricter than the enforced rule

## Non-Scope

- Two-review or code-owner-review enforcement
- Signed-commit enforcement
- Linear-history enforcement
- Merge queue or auto-merge configuration
- Rulesets for tags or non-`main` branches
- Organization/team permission design
- Release-branch governance

## Enforcement Details

### Branch target

- Protected branch: `main`

### Pull request requirements

- Require a pull request before merging: enabled
- Required approving review count: `0` for now
- Require review from code owners: disabled
- Require approval of the most recent push: disabled
- Dismiss stale reviews: disabled for now because no review count is being enforced
- Require conversation resolution before merging: enabled

### Status checks

- Require status checks to pass before merging: enabled
- Require branches to be up to date before merging: enabled
- Required checks:
  - `Build & Test`
  - `Docker Build Validation`

### Branch safety controls

- Apply restrictions to administrators: enabled
- Allow force-pushes: disabled
- Allow deletions: disabled

## Documentation Alignment

`CONTRIBUTING.md` currently says pull requests need at least one approval from a maintainer. That does not match the solo-maintainer-safe policy above.

If this design is implemented, the contribution guide should be updated so it says:

- pull requests must pass automated checks
- pull request conversations must be resolved before merge
- `main` is protected and changes land through pull requests
- approval requirements may be tightened when multiple maintainers are active

No README policy rewrite is required for this slice because the README already describes a fork-and-PR workflow at a high level without locking in a specific approval count.

## Validation Strategy

1. Use `gh api repos/bballer03/Mnemosyne/branches/main/protection` after applying the policy to confirm the remote settings.
2. Confirm `gh api repos/bballer03/Mnemosyne/branches/main` reports `protected: true`.
3. Verify the required status-check names exactly match the current workflow job names.
4. Re-read `CONTRIBUTING.md` after any doc edit to ensure the written contribution policy matches the enforced GitHub settings.
5. Do not test by attempting a direct push to `main`; the GitHub protection API is the source of truth for this slice.

## Risks

- Status-check names can drift if workflow job names change later. Mitigation: treat branch protection and workflow-name changes as a linked update.
- Zero required approvals is weaker than the usual team-maintainer baseline. Mitigation: raise it to `1` as soon as there is another active maintainer.
- Admin-enforced protection removes the fastest emergency path. Mitigation: use a short-lived hotfix branch and merge through a pull request.

## File And State Impact

- Remote GitHub state: `main` branch protection settings
- Documentation: `CONTRIBUTING.md`

## Decision

Adopt a solo-maintainer-friendly open source baseline for `main`: pull-request-only merges, required CI, required conversation resolution, no direct pushes, no force-pushes, no deletion, and no mandatory approving review until Mnemosyne has more than one active maintainer.
