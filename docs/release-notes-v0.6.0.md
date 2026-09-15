# Mnemosyne v0.6.0 Release Notes

> Release date: 2026-09-15
> Tag: v0.6.0
> Previous release: [v0.5.0](release-notes-v0.5.0.md)

Product release: deepen the continuous investigation workbench to MAT-style loop maturity (M25–M30) while keeping native packaged evidence and M31+ ecosystem work explicitly gated.

## Highlights

- **MAT loop depth (M25)** — Bounded histogram → instances → Inspector; lazy dominator children; opt-in field data with cost disclosure; synchronized object navigation.
- **Async platform (M26)** — Correlated progress/cancel envelopes; stale-result rejection across mutating operations.
- **Durable investigations + compare (M27)** — Display-safe workspace persistence, transactional snapshot hydrate, in-workbench compare controls (M24.E gate delivered).
- **Power tools (M28)** — Mature OQL workbench, opt-in analyzer enrichment, safe flamegraph/report exports.
- **Guided continuity (M29)** — Immutable findings queue, workspace-bound workflows without pasted IDs, selection-aware Assistant measured context.
- **Product hardening (M30)** — Theme tokens + compact layout; UI test RSS ceilings with SIGKILL escalation; daily desktop log rotation + redaction.

## Scope Gate

- **M31+ remains gated and not started** (desktop OS integration, MAT golden/equivalence, perspectives, live JVM/plugins, signing/updater). See [m31-gated-status.md](evidence/m31-gated-status.md).
- Packaged Windows/macOS/Linux GUI open→inspect, live AI provider chat, and MAT golden corpus remain **NOT PROVEN** on WSL.

## Breaking Changes

None for CLI, MCP, artifact, or desktop bridge contracts. Investigation UX and host operation envelopes are additive.

## Known Limitations

Unsigned desktop artifacts remain the default unless release signing secrets are configured. Homebrew SHA-256 values must be updated after v0.6.0 archives are published. jsdom remains on 24 for UI test RSS safety.

## Maintainer Cut Steps

1. Merge the release-prep / maturity branch after CI is green.
2. Create tag `v0.6.0` from the approved merge commit; release automation validates it against the workspace version.
3. Confirm GitHub Release assets, desktop bundles, and GHCR `0.6.0` / `0.6` / `latest`.
4. Replace the Homebrew Intel and Apple Silicon SHA-256 values with hashes from the published v0.6.0 archives.
