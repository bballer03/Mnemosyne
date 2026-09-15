# Mnemosyne v0.5.0 Release Notes

> Release date: Pending human/CI cut
> Tag: Not created by release-prep PR
> Previous release: [v0.4.3](release-notes-v0.4.3.md)

Product release: make heap investigation continuous across the desktop workbench while keeping advisory findings and comparison scope explicit.

## Highlights

- **Continuous workbench lifecycle (M24.A/B)** — A loaded heap keeps one persistent identity across investigation routes, with **Open**, **Open another**, and **Close** always available. Replacement opens are transactional: a failed open does not discard the current session. Recent loads and snapshots can be reopened without split-brain state.
- **Shared investigation selection (M24.C)** — Histogram, dominator, inspector, and leak-oriented routes share stable `objectId`, `classKey`, and `leakId` selection so drill-down context follows the investigator.
- **Findings advisory (M24.D)** — A collapsible, Rules-labelled Findings + Assistant pane provides deterministic deep-links while remaining clearly advisory over measured heap facts.
- **Dependency currency (M32)** — React 19.3, React Router 7, Vite 8, Tailwind 4.3, TypeScript 7, Cargo dependencies, the Rust Docker image, and GitHub Actions pins were refreshed within the validated compatibility envelope.

## Scope Gate

- **M24.E in-workbench compare is gated to post-v0.5.0 / the next slice.** It does not block this release.
- The existing standalone `/compare` route remains available as the separate comparison surface; this release does not claim compare is integrated into the continuous workbench shell.

## Dependency Exception

- **jsdom remains on 24.1.1.** Attempts to move through jsdom 29/30 correlated with multi-GB Bun RSS and focused-test hangs in the constrained environment. jsdom 30 is explicitly deferred until an RSS ceiling is demonstrated; no jsdom bump is included in v0.5.0.

## Breaking Changes

None for CLI, MCP, artifact, or desktop bridge contracts. M24 changes desktop investigation continuity and presentation without removing the standalone power routes.

## Known Limitations

Unsigned desktop artifacts remain the default unless release signing secrets are configured. Native launch-matrix and MAT golden-corpus evidence remain open. Overview-mode desktop analysis is still not wired. Homebrew SHA-256 values must be updated after v0.5.0 archives are published.

## Maintainer Cut Steps

1. Merge this release-prep change after CI is green.
2. Create tag `v0.5.0` from the approved merge commit; release automation validates it against the workspace version.
3. Confirm GitHub Release assets, desktop bundles, and GHCR `0.5.0` / `0.5` / `latest`.
4. Replace the Homebrew Intel and Apple Silicon SHA-256 values with hashes from the published v0.5.0 archives.
