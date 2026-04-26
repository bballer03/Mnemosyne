# Milestone 7-6 — v0.3.0 Release

> **Status:** 🔲 Pending — entering implementation after this design pass.
> **Predecessors:** M7-1 ✅, M7-2 ✅, M7-3 ✅, M7-4 ✅, M7-5 🟡 partial (acceptable for v0.3.0 — slices A/B/D shipped, slice C partial on WSL, reference-spec rerun user-owned).
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice), with the user as the human gate on the irreversible tag-and-push step.
> **Parent design:** [milestone-7-production-readiness.md](milestone-7-production-readiness.md)
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Release process references:** [.github/skills/finishing-a-development-branch/SKILL.md](../../.github/skills/finishing-a-development-branch/SKILL.md), [.github/prompts/release-prep.prompt.md](../../.github/prompts/release-prep.prompt.md)
> **Last updated:** 2026-04-26

---

## 1. Objective

Cut, validate, and publish **Mnemosyne v0.3.0** with the M7 production-readiness feature set:

- streaming overview mode (M7-1) — bounded-memory class-resolved triage on large dumps
- CI regression policies (M7-2) — `mnemosyne ci-check` with TOML policies and structured renderers
- allocation-site flame graphs (M7-3) — `mnemosyne flamegraph` with three rooting strategies
- OQL targeted expansion (M7-4) — `@retainedSize`, `@toString`, `@gcRootPath`, `LIKE`, `CONTAINS`, `OBJECTS`, `IS NULL`/`IS NOT NULL`
- comparative benchmark harness + partial publication (M7-5) — Linux-first scripts under `scripts/bench/`, methodology under `docs/benchmarks/`, partial WSL report at `docs/benchmarks/comparative-v0.3.0.md`

This is a **release / packaging / release-notes milestone**, not a Rust feature milestone. No new core or CLI behavior is added; this slice ships what is already on the working branch.

## 2. Context

- v0.2.0 is the current release on `main` (see [release-notes-v0.2.0.md](../release-notes-v0.2.0.md)).
- The working branch is `m6-ecosystem-roadmap-restructure` and has accumulated all of M6 plus M7-1 through M7-5 (partial) plus the M7-5 publication artifacts.
- `CHANGELOG.md` already has a comprehensive `[Unreleased]` section covering M6 and all M7 slices, including the M7-5 partial-publication caveats.
- Workspace and member crates use `version.workspace = true` (see [Cargo.toml](../../Cargo.toml), [cli/Cargo.toml](../../cli/Cargo.toml), [core/Cargo.toml](../../core/Cargo.toml)) — a single workspace bump propagates to `mnemosyne-cli` and `mnemosyne-core`. The Tauri shell is excluded from the workspace and ships independently.
- A working tag-triggered release workflow already exists at [.github/workflows/release.yml](../../.github/workflows/release.yml). It validates the tag against the workspace version, cross-compiles `mnemosyne-cli` for x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin, and x86_64-windows, builds and pushes the GHCR Docker image with `<version>`, `<major>.<minor>`, and `latest` tags, and publishes a GitHub Release using `docs/release-notes-v<tag>.md` as the body. **No workflow authoring is required in M7-6**; M7-6.B must produce the matching release-notes file or the release job will hard-fail at the "Resolve release notes path" step.
- M7-5 is intentionally partial. v0.3.0 ships the harness and the partial WSL report with explicit caveats; the reference-spec rerun remains user-owned and is documented as future work.

## 3. Scope

| Item | Notes |
|---|---|
| Workspace version bump from `0.2.0` → `0.3.0` | Single edit in workspace `[workspace.package].version` (member crates inherit). |
| `CHANGELOG.md` curation | Move `[Unreleased]` block to a new `[0.3.0] - <date>` section; tighten wording, deduplicate, group by Added / Changed / Fixed; preserve M7-5 partial caveat. |
| `docs/release-notes-v0.3.0.md` | New hand-crafted release notes (required by `release.yml`). |
| Final docs sweep | README install snippets reference v0.3.0 archive paths; `STATUS.md` reflects shipped state with M7-5 partial caveat; `docs/roadmap.md` shows M7 5/6 done with M7-5 partial caveat and v0.3.0 cut. |
| Final validation | All four cargo gates (`check`, `test --workspace --all-targets`, `clippy --workspace --all-targets -- -D warnings`, `fmt --all -- --check`) green on top of branch; smoke test the release-profile binary against a small HPROF fixture. |
| PR merge | Open / refresh the PR for `m6-ecosystem-roadmap-restructure` → `main`; squash or merge per repository convention; ensure `main` matches the validated branch tip. |
| Tag and push | `git tag -a v0.3.0 -m "..."` + `git push origin v0.3.0` from `main`. **Irreversible.** Triggers `release.yml`. |
| GitHub Release artifact verification | Confirm five archives uploaded, GHCR image tags `<version>`/`<major>.<minor>`/`latest` published, release body matches the hand-crafted notes. |
| Homebrew formula version + SHA bump | Update `HomebrewFormula/mnemosyne.rb` `version` and the two `sha256` placeholders against the published darwin tarballs. |
| Docker tag verification | Pull `ghcr.io/<owner>/mnemosyne:0.3.0` and `:latest`, run `--version` smoke. |
| Post-release sanity | Install via published GitHub release archive on at least one target; run `parse` and `analyze --mode auto` on a small fixture; confirm version string. |

## 4. Non-scope

- New features, refactors, or behavior changes. No code edits beyond version strings and packaging metadata.
- The native-Linux reference-spec benchmark rerun (M7-5 slice C remainder). Stays user-owned. Documented in release notes as future work.
- Eclipse MAT integration in the comparative report. Stays user-owned.
- M7+1 milestones (M8 work). Tech PM territory.
- UI / Tauri release. The Tauri shell is excluded from the workspace and ships independently; no Tauri bundles are published in v0.3.0.
- crates.io publication. Not part of v0.3.0 channels (workflow does not publish; doing so would require an explicit, separate decision).
- New release infrastructure, cross-compilation targets, or signing. The existing `release.yml` is sufficient.

## 5. Architecture overview

There are no architecture changes. The release purely packages the architecture frozen at the tip of `m6-ecosystem-roadmap-restructure`:

- Two-mode parity (deep + overview) frozen as v0.3.0 contract.
- `AnalysisMode { auto, deep, overview }` as the public mode contract across CLI / MCP / core.
- `core::policy` as the post-analysis policy layer.
- `core::report::flamegraph` as the flame-graph projection layer.
- `core::query` as the targeted-OQL surface.
- `core::hprof::overview` as the streaming triage parser.
- The deep-mode JSON contract for `AnalyzeResponse` is byte-compatible with v0.2.0 on existing fixtures (M7 made all additions optional / `skip_serializing_if`).

## 6. Module / file impact

| Path | Change kind |
|---|---|
| `Cargo.toml` (workspace) | `[workspace.package].version` → `0.3.0`. |
| `cli/Cargo.toml`, `core/Cargo.toml` | No edit needed — they inherit via `version.workspace = true`. |
| `Cargo.lock` | Regenerated by `cargo check` after version bump. |
| `CHANGELOG.md` | `[Unreleased]` → `[0.3.0] - <YYYY-MM-DD>`; new empty `[Unreleased]` placeholder. |
| `docs/release-notes-v0.3.0.md` | New file. Required by `release.yml`. |
| `STATUS.md` | Snapshot bullet updated to "v0.3.0 is the current release"; M7 5/6 done with M7-5 partial caveat. |
| `docs/roadmap.md` | M7 entry shows v0.3.0 cut, M7-5 partial caveat preserved, M7-6 done. |
| `README.md` | Install snippet version refs (Homebrew, Docker, archive URLs) bumped to v0.3.0. |
| `HomebrewFormula/mnemosyne.rb` | `version "0.3.0"` plus two `sha256` lines populated from published darwin tarballs (post-tag, M7-6.D). |
| `Dockerfile` | No edit. Image tag is parameterized by the workflow's `VERSION` build-arg. |
| `.github/workflows/release.yml` | No edit. Already wired. |

## 7. API / CLI / reporting impact

None. v0.3.0 is the version stamp on the contracts already shipped through M7-1..M7-5. No breaking changes are expected; if any are discovered during M7-6.A audit, they must be flagged as a hard release blocker and routed back to design.

## 8. Data model changes

None.

## 9. Validation / testing strategy

- All four cargo gates on top of the merged branch:
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace --all-targets`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
- Release-profile build: `cargo build --release -p mnemosyne-cli`.
- Smoke test the release binary on a small HPROF fixture: `target/release/mnemosyne-cli parse <fixture>` and `target/release/mnemosyne-cli analyze --mode auto <fixture>`.
- Tag-version validation is enforced by the workflow's `validate-version` job — failure there is the safety net for a mis-bumped manifest.
- Post-release: download a published archive, install on at least one OS, run `mnemosyne-cli --version` and a basic `parse` / `analyze` smoke against a small fixture.

The handoff to a Testing Agent is **not required** for M7-6 because no behavior changes ship; the validation here is gating, not feature-coverage. Implementation Agent runs the gates inline per slice.

## 10. Release readiness gates (hard preconditions before tagging)

- [ ] All four cargo gates green on top of branch (Implementation Agent reports actual command output).
- [ ] `CHANGELOG.md` `[Unreleased]` section is comprehensive, accurate, deduplicated, and the M7-5 partial caveat is preserved verbatim.
- [ ] `STATUS.md` reflects post-M7 state (M7 5/6 done with M7-5 partial caveat, v0.3.0 as current release).
- [ ] `docs/roadmap.md` shows v0.3.0 as cut and M7-5 partial caveat is loud.
- [ ] `docs/release-notes-v0.3.0.md` exists, leads with the M7-5 partial caveat, and links the comparative report.
- [ ] Comparative benchmark report (`docs/benchmarks/comparative-v0.3.0.md`) is reachable from README and release notes; relative links resolve.
- [ ] No open critical issues against M7 work.
- [ ] PR for `m6-ecosystem-roadmap-restructure` → `main` is approved and mergeable; CI green on the merge candidate.
- [ ] Workspace version is `0.3.0`; tag to be pushed is `v0.3.0`.
- [ ] User has explicitly confirmed the irreversible tag step (M7-6.C gate).

## 11. Slice breakdown

The work is split into four focused slices. Each slice is a single PR-sized batch with the four cargo gates run inline.

### Slice M7-6.A — Version bump + CHANGELOG curation + STATUS/roadmap final sweep

- **Files owned:**
  - `Cargo.toml` (workspace `version` → `0.3.0`)
  - `Cargo.lock` (regenerated by `cargo check`)
  - `CHANGELOG.md` (`[Unreleased]` → `[0.3.0] - <date>`; tighten wording; new empty `[Unreleased]` placeholder)
  - `STATUS.md` (top snapshot bullet)
  - `docs/roadmap.md` (M7 row + v0.3.0 status)
  - `README.md` (install snippet version refs only — Homebrew/Docker/archive URLs)
- **Files explicitly not owned in this slice:** `HomebrewFormula/mnemosyne.rb` (deferred to M7-6.D after tarball SHAs are available), `docs/release-notes-v0.3.0.md` (Slice B), workflow files.
- **Validation:** all four cargo gates green; `grep -rn "0\.2\.0" --include='*.toml' --include='*.md'` shows only historical references (CHANGELOG history, release-notes-v0.2.0.md).
- **Target size:** ~10 file edits, mostly mechanical. No code edits.
- **Done means:** branch tip carries `0.3.0` workspace version, CHANGELOG curated, STATUS/roadmap/README updated, all gates green, ready for Slice B.

### Slice M7-6.B — Release notes (`docs/release-notes-v0.3.0.md`)

- **Files owned:** `docs/release-notes-v0.3.0.md` (new file).
- **Required structure** (mirroring [release-notes-v0.2.0.md](../release-notes-v0.2.0.md)):
  - Header: release date, previous release pointer (v0.2.0).
  - Highlights — five subsections, one per M7 slice, each labeled honestly.
  - **Prominent M7-5 partial-status callout** — first or second highlight section, not buried.
  - Performance / benchmark section — cite the partial WSL report at `docs/benchmarks/comparative-v0.3.0.md`, name the absent comparisons (MAT, deep-mode, xlarge, equivalence) explicitly.
  - Upgrade notes — explicit "no breaking API/CLI/MCP/report changes".
  - Known limitations — M7-5 reference-spec rerun pending, MAT integration pending, no Tauri release artifacts, no crates.io publication.
  - Roadmap progress — M7 5/6 shipped, v0.3.0 cut, link to roadmap.
- **Honesty contract** (binding for the writer):
  - Do NOT claim "MAT-comparable depth" without MAT in the comparative report.
  - Do NOT claim "10 GB heap support" without 10 GB measurement.
  - DO say "credible streaming behavior on 6 GB fixtures (WSL execution; reference-spec re-run pending)".
  - The M7-5 partial status must be loud, not buried.
- **Validation:** workflow's "Resolve release notes path" step would find this file at the expected path; relative links resolve when previewed.
- **Target size:** ~150–250 lines of markdown. No code edits.

### Slice M7-6.C — Final validation + PR merge + tag and push **(irreversible gate)**

This slice uses [.github/skills/finishing-a-development-branch/SKILL.md](../../.github/skills/finishing-a-development-branch/SKILL.md).

- **Files owned:** none (no source edits in this slice; `.git` operations only).
- **Steps, in order:**
  1. Re-run all four cargo gates on the branch tip; quote command output.
  2. Verify all readiness gates in §10 are checked.
  3. Open / refresh the PR for `m6-ecosystem-roadmap-restructure` → `main`; ensure CI is green; obtain approval per repository policy.
  4. Merge to `main` (squash or merge per repository convention; preserve a clean v0.3.0 commit on `main`).
  5. Pull `main`, confirm tip matches expectations.
  6. **STOP. Hard gate.** Surface the planned tag commands to the user verbatim and obtain explicit confirmation:
     ```
     git tag -a v0.3.0 -m "Mnemosyne v0.3.0"
     git push origin v0.3.0
     ```
     Do not execute these without an explicit user "go".
  7. After confirmation, run the tag and push.
  8. Monitor `release.yml` runs to completion (validate-version → build matrix → docker → release).
  9. Verify the GitHub Release page lists all five archives, the GHCR Docker image has `0.3.0`, `0.3`, and `latest` tags, and the release body matches `docs/release-notes-v0.3.0.md`.
- **Irreversibility rule:** once `v0.3.0` is pushed and the workflow publishes artifacts, the tag is immutable. Do not force-update or delete-and-repush a published release tag. Any post-tag fix is a new patch release (`v0.3.1`), not a re-tag.
- **Rollback if pre-push:** if a blocker is discovered between merge and push, no rollback is needed — simply do not push the tag; fix on `main`; restart Slice C.
- **Rollback if post-push but pre-publish:** the tag exists but artifacts have not yet uploaded — let the workflow finish (artifacts will be wrong/missing); then cut `v0.3.1` with the fix; document in CHANGELOG.
- **Rollback if post-publish:** see §13.

### Slice M7-6.D — Post-release packaging

- **Files owned:**
  - `HomebrewFormula/mnemosyne.rb` (bump `version "0.3.0"`; replace both `sha256` lines with the SHA-256 of the two darwin tarballs from the published GitHub release).
- **Steps:**
  1. Download the published `mnemosyne-cli-aarch64-apple-darwin.tar.gz` and `mnemosyne-cli-x86_64-apple-darwin.tar.gz` from the GitHub Release.
  2. Compute SHA-256 for each.
  3. Update the formula; commit on `main` with a `release(homebrew): bump formula to v0.3.0` Conventional Commit.
  4. Verify the GHCR image tags exist: `ghcr.io/<owner>/mnemosyne:0.3.0`, `:0.3`, `:latest`.
  5. Run an install smoke test: `docker run --rm ghcr.io/<owner>/mnemosyne:0.3.0 --version` (expect `mnemosyne-cli 0.3.0`).
  6. Run an archive install smoke test: download a platform archive, extract, run `--version` and a small-fixture `parse`.
  7. Confirm README install snippets work as written.
- **Validation:** Homebrew formula install (`brew install --formula ./HomebrewFormula/mnemosyne.rb`) on a darwin host is the gold-standard check; if no darwin host is available, at minimum verify the SHAs match and the URLs resolve.
- **Target size:** one file edit + verification logs. No code edits.
- **Done means:** all v0.3.0 distribution channels (GitHub Release, GHCR, Homebrew formula) are aligned.

## 12. Risks and open questions

| Risk | Mitigation |
|---|---|
| Release workflow fails on a cross-compile target | The workflow is unchanged from v0.2.0 and known good. If a target regresses, fix on `main` and cut `v0.3.1`; do not re-tag `v0.3.0`. |
| Tag-triggered build takes time / requires monitoring | M7-6.C explicitly includes monitoring `release.yml` to completion before declaring the slice done. |
| Homebrew formula update may need a separate PR or upstream tap interaction | The formula in `HomebrewFormula/` is in-repo; bump is a normal commit on `main` post-release. No upstream tap is in use today. |
| Reference-spec rerun caveat may be buried in release notes | M7-6.B treats the M7-5 partial status as a top-level highlight, not a footnote. Reviewer must confirm placement. |
| User assumes "10 GB heap support" because the design doc mentioned it | Release notes must say "credible streaming on 6 GB fixtures; 10 GB tier pending reference-workstation rerun" verbatim. |
| Cross-compilation environment drift on GitHub-hosted runners | Workflow pins action SHAs and uses `cargo` / `cross` at known versions; if a target build breaks at tag time, treat as a v0.3.1 candidate, not a v0.3.0 re-tag. |
| Manifest version mis-bumped (workspace says 0.2.0, tag says v0.3.0) | The workflow's `validate-version` job hard-fails before any artifact uploads; this is the safety net. |
| Missing `docs/release-notes-v0.3.0.md` at tag time | The workflow's "Resolve release notes path" step hard-fails with a clear message; M7-6.B is the gate. |
| `Cargo.lock` churn not committed | Slice M7-6.A explicitly runs `cargo check` after the version bump and commits the regenerated lockfile. |
| User pushes `v0.3.0` before `main` is up to date | Slice C step 5 mandates pulling `main` and confirming tip before tagging. |

**Open question:** should v0.3.0 also publish to crates.io? Decision: **no, deferred.** Not in scope; would require a separate workflow change and explicit user decision. Document in release notes' "Known limitations" / "Distribution channels" section.

## 13. Honesty contract for the release

This is binding for M7-6.B and any docs-sweep work in M7-6.A:

- **Do not** claim "MAT-comparable depth" anywhere in release notes, README, or roadmap. MAT is not in the published comparative report.
- **Do not** claim "10 GB heap support" or "10 GB validated". 10 GB is in the design as a target; the published evidence is on 6 GB fixtures only.
- **Do** say "credible streaming behavior on 6 GB fixtures (WSL execution; reference-spec re-run pending)" or equivalent wording.
- **Do** mark the M7-5 partial status prominently in release notes — not in a footnote, not in an appendix.
- **Do** name the absent comparisons explicitly: no MAT row, no deep-mode row, no xlarge tier, no equivalence comparison.
- **Do** preserve the existing `Partial` provenance markers in user-facing docs as-is; they are already honest and machine-readable.

## 14. Reproducibility / rollback plan

- **Tags are immutable in practice.** Once `v0.3.0` is pushed and the GitHub Release is published, do not force-update, delete-and-repush, or otherwise rewrite the tag. Package managers and downstream caches treat the tag → archive → SHA mapping as fixed.
- **Post-tag regression discovered:**
  1. Open an issue describing the regression and severity.
  2. If a packaging mirror that supports yanking is involved (currently: none — crates.io is not used), yank the affected version.
  3. Cut `v0.3.1` with the fix (this addendum's process applies recursively to a patch release: bump workspace to `0.3.1`, curate `[Unreleased]`, write `release-notes-v0.3.1.md`, validate, merge, tag, verify, bump Homebrew formula).
  4. Document the regression and remediation in the `v0.3.1` CHANGELOG entry.
- **Reproducibility:** the build is reproducible from the tag — the workflow and pinned actions/runners encode the toolchain. Anyone with access to the tagged SHA can `cargo build --release -p mnemosyne-cli` and reproduce the binary modulo platform-specific differences already documented in benchmarks/QA notes.

## 15. Implementation readiness verdict

**READY** — this addendum supersedes the §11 framing in the parent M7 doc for implementation purposes. The Implementation Agent may begin with **Slice M7-6.A** immediately. Slices B, C, and D follow in strict order. **Slice M7-6.C contains the only irreversible step in M7 (tag-and-push) and must be gated on explicit user confirmation before `git push origin v0.3.0`** — the orchestrator is responsible for surfacing that gate verbatim and waiting for "go" before the tag is pushed.
