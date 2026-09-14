# M21 portable installability evidence

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-14  
**Tip at write:** see `git log -1` on this branch

## Command-layer (proven on this host / in CI scripts)

| Gate | Status | Notes |
| --- | --- | --- |
| Frozen primary names | configured | verifier + README |
| Normalize Tauri → frozen | configured | AppImage/DMG rename; ditto macOS zip |
| SHA256SUMS + manifest | configured | unsigned; `launch_tested: false` |
| Zip secret/build-path inspect | fail-closed | `inspect_desktop_archives.py` |
| Info.plist / AppImage ELF probe | fail-closed when present | `probe_desktop_bundles.py` (ELF arch only after download; executable bit opt-in) |
| Name + checksum release gate | fail-closed | `verify_desktop_assets.py --require-checksums` |
| Docs consistency | green | `test_m21_docs_consistency.py` |

## Native launch matrix (not proven here)

| Platform asset | Built in CI | Attached | Signed | Launch-tested | Open heap dump |
| --- | --- | --- | --- | --- | --- |
| Windows portable zip | configured | configured | conditional secrets | **not launch-tested** (needs clean Windows + WebView2) | **not** |
| macOS aarch64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| macOS x64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| Linux x86_64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |
| Linux aarch64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |

WSL contributes **no** packaged GUI launch claim. Release dry-run against a live GitHub Actions run is still outstanding.

See also [m21-m22-remaining.md](m21-m22-remaining.md).
