# Milestone 21 — Eclipse-Style Portable Installability

> **Status:** 🟡 Partial — Slice **21.A** asset-name freeze + WSL-safe verifier shipped; Windows portable packaging script/CI step exist from earlier honesty work. **Not claimed:** native Windows/macOS/Linux launch matrix, CI checksum/manifest gate on every release, or AppImage/macOS name normalization in upload.
> **Parent plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) (M21)
> **Last updated:** 2026-09-14

## Objective

Release assets support download → unzip/mount → double-click without a JVM or developer toolchain, with honest WebView/WebKit prerequisites and no claim that CI build success equals launch success.

## Frozen primary click-to-run names (21.A)

`<version>` is the release tag without a leading `v`.

| Role | Required primary filename |
| --- | --- |
| Windows x64 portable | `Mnemosyne-<version>-windows-x64-portable.zip` |
| macOS Apple Silicon app zip | `Mnemosyne-<version>-macos-aarch64-app.zip` |
| macOS Intel app zip | `Mnemosyne-<version>-macos-x64-app.zip` |
| Linux x86_64 AppImage | `Mnemosyne-<version>-linux-x86_64.AppImage` |
| Linux aarch64 AppImage | `Mnemosyne-<version>-linux-aarch64.AppImage` |

Documented **fallback** installers (allowed by the verifier, not required for the portable gate) keep today’s Tauri-ish names: `.msi` / `-setup.exe`, `.dmg`, `.deb` / `.rpm`, plus transitional `Mnemosyne_<version>_amd64.AppImage` / `_arm64.AppImage` until Slice 21.D normalizes uploads.

## Verifier (WSL-safe)

```bash
python3 scripts/release/verify_desktop_assets.py \
  --dist dist \
  --version 0.3.1 \
  --require-checksums   # optional: fail if SHA256SUMS absent
```

Tests (synthetic `dist/` only — no GUI launch):

```bash
python3 scripts/tests/test_verify_desktop_assets.py
```

The verifier flags **missing**, **duplicate** basenames, **misnamed** desktop-like files, and **checksum** gaps/mismatches when a `SHA256SUMS` file is present (or required).

## Packaging honesty already on this branch

- `scripts/release/package_windows_portable.ps1` builds `Mnemosyne-<version>-windows-x64-portable.zip` and labels it **portable with WebView2 prerequisite**.
- `release.yml` packages/uploads that zip on the Windows desktop matrix leg.
- Docs already state WSL cannot prove packaged GUI smoke.

## Remaining M21 gaps (not closed here)

- Generate SHA-256 checksums + machine-readable manifest in release CI and fail the release job when required primaries/checksums are absent.
- Normalize macOS app zip and Linux AppImage upload names to the frozen table (21.C / 21.D).
- Native-host launch matrix + evidence doc (21.F); no WSL launch claim.
- Docs polish / Terra reviews per plan slices 21.B–21.F.

## Non-goals

Mandatory signing credentials, auto-update, store packaging, or equating CI green with launch-tested.
