# Milestone 21 — Eclipse-Style Portable Installability

> **Status:** 🟡 Partial — **21.A** verifier; **21.B** Windows portable + unsigned desktop `SHA256SUMS`/manifest provenance + zip secret inspect; **21.C/D** normalize AppImage/DMG + **ditto-only** macOS `.app` zip (Python zipfile refused for release). Verifier still **warn mode**. **Not claimed:** native launch matrix, strict fail-closed gate, or GUI smoke.
> **Parent plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) (M21)
> **Last updated:** 2026-09-14

## Objective

Release assets support download → unzip/mount → double-click without a JVM or developer toolchain, with honest WebView/WebKit prerequisites and no claim that CI build success equals launch success.

## Frozen primary click-to-run names (21.A / 21.C / 21.D)

`<version>` is the release tag without a leading `v`.

| Role | Required primary filename |
| --- | --- |
| Windows x64 portable | `Mnemosyne-<version>-windows-x64-portable.zip` |
| macOS Apple Silicon app zip | `Mnemosyne-<version>-macos-aarch64-app.zip` |
| macOS Intel app zip | `Mnemosyne-<version>-macos-x64-app.zip` |
| Linux x86_64 AppImage | `Mnemosyne-<version>-linux-x86_64.AppImage` |
| Linux aarch64 AppImage | `Mnemosyne-<version>-linux-aarch64.AppImage` |

### Frozen fallback installers (allowed, not required for the portable gate)

| Role | Frozen fallback filename | Typical Tauri source (normalized away) |
| --- | --- | --- |
| macOS Apple Silicon DMG | `Mnemosyne-<version>-macos-aarch64.dmg` | `Mnemosyne_<version>_aarch64.dmg` |
| macOS Intel DMG | `Mnemosyne-<version>-macos-x64.dmg` | `Mnemosyne_<version>_x64.dmg` |
| Windows MSI / NSIS | `Mnemosyne_<version>_x64_en-US.msi`, `Mnemosyne_<version>_x64-setup.exe` | (unchanged Tauri names) |
| Linux deb / rpm | `Mnemosyne_<version>_amd64.deb` / `_arm64.deb`, `Mnemosyne-<version>-1.x86_64.rpm` / `.aarch64.rpm` | (unchanged) |

`scripts/release/normalize_desktop_assets.py` renames AppImage + DMG sources above and, when `--macos-arch` is set, zips `Mnemosyne.app` → the frozen macOS app zip **via Apple `ditto -c -k --sequesterRsrc --keepParent`**. Python `zipfile` is refused unless `--allow-unsafe-python-zip` (synthetic tests only). Transitional Tauri AppImage/DMG basenames remain **allowed** by the verifier if a normalize miss leaves them on disk.

## Verifier (WSL-safe)

```bash
python3 scripts/release/verify_desktop_assets.py \
  --dist dist \
  --version 0.3.1 \
  --require-checksums   # optional: fail if SHA256SUMS absent

# CI warn mode (does not fail the release job):
python3 scripts/release/verify_desktop_assets.py \
  --dist dist \
  --version 0.3.1 \
  --allow-warnings
```

Normalize (Tauri → frozen; no launch claim):

```bash
# After a local/CI Tauri build tree:
python3 scripts/release/normalize_desktop_assets.py \
  --version 0.3.1 \
  --search-root tauri/target \
  --out-dir dist-normalized \
  --macos-arch aarch64   # or x64; omit on Linux

# In-place rename under an assembled release dist/:
python3 scripts/release/normalize_desktop_assets.py \
  --version 0.3.1 \
  --dist dist \
  --in-place
```

Checksum generation (desktop-heuristic scope; unsigned; no launch claim):

```bash
python3 scripts/release/generate_sha256sums.py \
  --dist dist \
  --desktop-only \
  --version 0.3.1 \
  --manifest auto
```

Archive member inspect (credential / absolute build-path denylist; fail-closed in CI):

```bash
python3 scripts/release/inspect_desktop_archives.py --dist dist
```

Tests (synthetic trees only — no GUI launch):

```bash
python3 scripts/tests/test_verify_desktop_assets.py
python3 scripts/tests/test_generate_sha256sums.py
python3 scripts/tests/test_normalize_desktop_assets.py
python3 scripts/tests/test_inspect_desktop_archives.py
```

The verifier flags **missing**, **duplicate** basenames, **misnamed** desktop-like files, and **checksum** gaps/mismatches when a `SHA256SUMS` file is present (or required). With `--allow-warnings`, findings are printed but exit status stays 0. `SHA256SUMS` header comments + `asset-manifest.json` declare `scope=desktop-heuristic`, `signed: false`, and optional CI provenance fields — not a Sigstore attestation.

## Packaging honesty already on this branch

- `scripts/release/package_windows_portable.ps1` builds `Mnemosyne-<version>-windows-x64-portable.zip` and labels it **portable with WebView2 prerequisite**.
- `scripts/release/normalize_desktop_assets.py` + `release.yml` normalize AppImage/DMG names; macOS app zips use **ditto** on macOS runners.
- `release.yml` runs zip secret inspect, generates unsigned desktop `SHA256SUMS` + provenance manifest, and warn-mode verify before attaching `dist/*` to the GitHub Release.
- Docs already state WSL cannot prove packaged GUI smoke. See [docs/evidence/m21-m22-remaining.md](../evidence/m21-m22-remaining.md).

## Remaining M21 gaps (not closed here)

- Fail the release job when required frozen primaries / checksums are absent (strict gate — 21.F).
- Native-host launch matrix + evidence doc (21.F); no WSL launch claim.
- Full macOS bundle integrity checks (Info.plist / executable bits) and AppImage arch metadata probes beyond rename.
- Signed/attested checksums (Sigstore/GPG) — provenance fields today are informational only.

## Non-goals

Mandatory signing credentials, auto-update, store packaging, or equating CI green with launch-tested.
