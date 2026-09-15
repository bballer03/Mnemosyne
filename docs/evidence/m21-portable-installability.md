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
| Windows portable zip | configured | configured | conditional secrets | **launch-tested 2026-09-15** (see below) | **NOT PROVEN** (no automated Open-heap click) |
| macOS aarch64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| macOS x64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| Linux x86_64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |
| Linux aarch64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |

## 2026-09-15 Windows portable launch (partial closeout)

**Branch:** `docs/m21-windows-launch-partial`  
**Host:** Windows 10.0.26200 (user `sarfa`) under dual-boot / WSL partner machine `DESKTOP-8KSNRFK`  
**WebView2 Evergreen:** present (`152.0.4191.66`)  
**Asset:** `Mnemosyne-0.6.0-windows-x64-portable.zip`  
**SHA-256:** `b2c4843d3c731ee04991da08a65abc1bb00a991e946a615ea642cf866570ad54`  
**Also downloaded (not installed this run):**  
- `Mnemosyne_0.6.0_x64_en-US.msi` — `de659cab31b4b5290145949a7f0515d4e0a547694dbb8c6c182e611d3a1969fd`  
- `Mnemosyne_0.6.0_x64-setup.exe` — `c3c72b8dfcb2c69e996680528d51064ae6d118b1b4a0f6a95f73beac6766a1f2`

### Commands (sanitized)

```powershell
Expand-Archive Mnemosyne-0.6.0-windows-x64-portable.zip
Start-Process Mnemosyne.exe  # stayed alive ~5–6s (Pid recorded), then stopped for cleanup
```

Synthetic fixture beside the exe: `fixture-simple.hprof` (655 bytes from `build_simple_fixture()`).

### Results

| Check | Result |
| --- | --- |
| Portable extract | pass |
| Process starts (`Mnemosyne`) | **pass** (Pid 8676 / 14572; `HasExited=False` after 5–6s) |
| WebView2 prerequisite | present on host |
| Interactive Open heap dump (UI click) | **NOT PROVEN** — no UI automation this run |
| MSI / setup.exe silent install | **NOT run** |
| macOS / Linux launch | **not launch-tested** (out of scope for this partial) |

### Terra subagent stand-in

- Focus: no over-claim of Open-heap or non-Windows rows
- Outcome: **APPROVED** for Windows launch-tested partial

### Sol verdict

- **Scope:** M21 Windows-only launch matrix row
- **Verdict:** **partial** — Windows portable **launch-tested**; Open-heap UI click **NOT PROVEN**; macOS/Linux **not launch-tested**
- **Date:** 2026-09-15

WSL alone still contributes **no** packaged GUI launch claim; this row was proven via `powershell.exe` on the Windows host.

See also [m21-m22-remaining.md](m21-m22-remaining.md).

