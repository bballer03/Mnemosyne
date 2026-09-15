# M21 portable installability evidence

**Branch:** `feature/mat-equivalent-wave0`
**Date:** 2026-09-15
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
| Windows portable zip | configured | configured | conditional secrets | **launch-tested 2026-09-15** (see below) | **NOT PROVEN** (UI Automation could not reach WebView content) |
| macOS aarch64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| macOS x64 app zip | configured | configured | conditional secrets | **not launch-tested** | **not** |
| Linux x86_64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |
| Linux aarch64 AppImage | configured | configured | unsigned by design | **not launch-tested** | **not** |

## 2026-09-15 Windows portable launch (partial closeout)

**Branch:** `docs/m21-windows-launch-partial`  
**Host:** Windows 10.0.26200 under WSL interop
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
| Interactive Open heap dump (UI click) | **NOT PROVEN** in the initial run; follow-up automation below also failed |
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

## 2026-09-15 Open-heap follow-up

### Packaged v0.6.0 UI Automation

The portable executable was launched through `powershell.exe`. Two Windows UI
Automation searches attempted to invoke the **Open heap dump** button:

1. search descendants associated with the Mnemosyne process;
2. search globally, accounting for WebView2 child processes.

Both attempts returned `button_not_found`; no file dialog appeared. A later
diagnostic launch recorded WebView2 error `0x80070578` (`Invalid window handle`)
before app content rendered. The process and native window remained observable,
but the web UI was unavailable to UI Automation in this desktop session.

Synthetic fixture: `fixture-simple.hprof`, 655 bytes, SHA-256
`b52b188291234428514c9bea6ad5b30764303d3ac5fecb7c65d352982b29330d`.
No absolute fixture path is retained here.

| Check | Result |
| --- | --- |
| Portable process/native window starts | **pass** |
| UI Automation finds **Open heap dump** | **fail** (`button_not_found`) |
| Native file dialog appears | **NOT PROVEN** |
| Fixture selected and analyzed through packaged UI | **NOT PROVEN** |

### Startup-argument improvement

This branch adds one-shot startup opening for both forms:

```powershell
Mnemosyne.exe fixture-simple.hprof
Mnemosyne.exe --open fixture-simple.hprof
```

Rust validates exactly one startup path, the `.hprof` extension, and file
existence, then registers the path in `HeapSession` under an opaque `sourceId`.
React receives only that ID plus the basename and reuses
`load_heap_from_source`; no absolute heap path crosses into the web UI.

| Check | Result |
| --- | --- |
| Startup argument parser unit tests | **pass** |
| Opaque startup picker bridge/UI tests | **pass** |
| Native Windows `cargo check` | **pass** (`CARGO_INCREMENTAL=0` for WSL filesystem compatibility) |
| Production UI build | **pass** |
| Updated executable starts and analyzes fixture end to end | **NOT PROVEN** — same host-session WebView2 `0x80070578` failure prevented the frontend from invoking the startup command |
| Released portable v0.6.0 supports startup args | **no** — the release predates this change |

The product path is implemented and covered below the native window boundary;
packaged end-to-end Open-heap remains an explicit native-host validation item.

See also [m21-m22-remaining.md](m21-m22-remaining.md).

