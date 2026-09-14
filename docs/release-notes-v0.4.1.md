# Mnemosyne v0.4.1 Release Notes

> Release date: 2026-09-14
> Tag: v0.4.1
> Previous release: [v0.4.0](release-notes-v0.4.0.md)

Mnemosyne v0.4.1 is a **desktop hotfix** on the v0.4.0 line. Packaged apps now inject host bridges from the UI bundle so **Open heap dump** works inside the Tauri shell instead of claiming the feature is only available in the desktop app.

The **MAT-equivalent continuous workbench UI** remains reserved for **v0.5.0** (M24 Continuous Heap Investigation) — not this patch.

## Highlights

- **Desktop bridge injection (Track A):** `ui/src/host/tauri-bridge.ts` injects `__MNEMOSYNE_*_BRIDGE__` before React mounts; `@tauri-apps/api` is a UI dependency; Tauri ACL `allow-desktop-commands` grants invoke for desktop IPC.
- **Honest unavailable copy:** Browser vs “Tauri running but bridge missing” messaging on Home → Open heap dump.
- **STATUS conflict cleanup:** Removes accidental merge conflict markers from `STATUS.md` introduced during the Track A merge.

## Breaking Changes

None. Additive fix only.

## Known Limitations

- Same as v0.4.0: unsigned desktop by default; native launch matrix / MAT golden / Sol closeouts still open — [docs/evidence/m21-m22-remaining.md](evidence/m21-m22-remaining.md).
- Homebrew SHA-256 values are placeholders until this tag’s macOS CLI archives publish; fill in a follow-up commit.
- This release does **not** ship the heap-first workbench IA (that is **v0.5.0 / M24**).

## Upgrade Instructions

- **GitHub Release:** download CLI archives and desktop bundles from the `v0.4.1` release page (prefer this over `v0.4.0` for desktop Open heap dump).
- **Docker:** `docker pull ghcr.io/bballer03/mnemosyne:0.4.1`
- **Source:** `git checkout v0.4.1 && cargo build --release --workspace`
- **Homebrew:** after SHA follow-up, `brew upgrade mnemosyne`.

## Links

- Design / next: [M24 Continuous Heap Investigation](superpowers/specs/2026-09-14-m24-continuous-heap-investigation-design.md) → target product release **v0.5.0**
- Changelog: [CHANGELOG.md](../CHANGELOG.md)
- Evidence leftovers: [m21-m22-remaining.md](evidence/m21-m22-remaining.md)
