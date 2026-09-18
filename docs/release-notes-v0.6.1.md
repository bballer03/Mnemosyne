# Mnemosyne v0.6.1 Release Notes

> Release date: 2026-09-18
> Tag: v0.6.1
> Previous release: [v0.6.0](release-notes-v0.6.0.md)

Patch release: ship the post–v0.6.0 MAT-equivalent Wave 0–2 work into packaged builds so Windows desktop gets argv open, File → Open Heap, associations helpers, perspectives, superclass tree, and CI golden harnesses.

## Highlights

- **Desktop open path** — Positional / `--open` startup heap arguments with opaque `sourceId` flow; native **File → Open Heap** menu on Windows; portable Open With registration script.
- **Interactive Windows proof** — Argv open + analysis proven in an interactive Windows session (WSL session-0 WebView2 launches remain invalid for GUI proof).
- **Workbench perspectives** — Named presets: Leak Hunt, Dominator Browse, Compare, OQL Lab (persisted with workspace metadata).
- **Superclass histogram tree** — Collapsible tree only when graph-resolved `parent_key` links exist (no invented ancestry).
- **MAT golden harness** — Synthetic CI baselines (`mnemosyne-golden`); Eclipse MAT `mat-referenced` cross-check still **NOT PROVEN**.
- **Live AI provider path** — Loopback MCP provider round-trip evidenced; cloud provider still needs a configured API key (not claimed here).
- **Older partials closeout** — M20/M22/M23 stub/browser Sol verdicts; M21 Windows launch partial; jsdom 30 retained refusal.

## Scope / honesty

- macOS / Linux packaged launch remain **not launch-tested** (deferred).
- Installed Explorer double-click association end-to-end remains **NOT PROVEN** until exercised on this release’s installers.
- Signing / updater / live JVM (M31.D–E) remain gated.
- Not a full Eclipse MAT clone; see [eclipse-mat-equivalent program design](superpowers/specs/2026-09-15-eclipse-mat-equivalent-program-design.md).

## Breaking Changes

None for CLI, MCP, artifact, or desktop bridge contracts. Desktop argv open and File menu are additive.

## Known Limitations

Unsigned desktop artifacts remain the default unless release signing secrets are configured. Homebrew SHA-256 values must be updated after v0.6.1 darwin CLI archives publish. jsdom remains on 24 for UI test RSS safety.

## Maintainer Cut Steps

1. Merge the release-prep PR after CI is green.
2. Create tag `v0.6.1` from the merge commit; release automation validates it against the workspace version.
3. Confirm GitHub Release assets, desktop bundles, and GHCR `0.6.1` / `0.6` / `latest`.
4. Replace the Homebrew Intel and Apple Silicon SHA-256 values with hashes from the published v0.6.1 archives.
5. Optionally re-run Windows interactive `--open` / File → Open Heap against the new portable zip.
