# M21 portable installability evidence

**Branch:** `fix/windows-webview2-open-heap`
**Date:** 2026-09-16
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
| Windows portable zip | configured | configured | conditional secrets | **launch-tested 2026-09-15** (see below) | Packaged v0.6.0 click/selection **NOT PROVEN**; post-PR #103 local build `--open` **PROVEN** in an interactive Windows session (2026-09-16) |
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

## 2026-09-16 Windows launch-context diagnosis and argv proof

### Build and fixture

- Source: latest `main` after PR #103, commit `e5b1d5f`.
- UI: production build completed before the Windows desktop build.
- Desktop: local Windows production-protocol build through the Tauri CLI.
- Executable SHA-256:
  `ca8ce338a036c7ed06d3d7a18e33e2c7732cdf64ac02f02384e698bec87e5312`.
- Synthetic fixture: `fixture-simple.hprof`, 655 bytes, SHA-256
  `b52b188291234428514c9bea6ad5b30764303d3ac5fecb7c65d352982b29330d`.
- Executable and fixture were staged below `%LOCALAPPDATA%\Temp`; no absolute
  user path is retained here.

A plain `cargo build --release` diagnostic binary displayed a localhost
`ERR_CONNECTION_REFUSED` page because it used Tauri's development protocol.
It was discarded. All results below use the production-protocol Tauri build
with embedded UI assets.

### Session discriminator

The WSL-interoperability PowerShell process ran as the current Windows user in
session 0 with `UserInteractive=false`. The same user's Explorer shell was
running in session 1. This distinguished user identity from desktop-session
attachment before the launch methods were compared.

| Launch method | Observed session/result |
| --- | --- |
| Direct WSL-spawned PowerShell `Start-Process` | Session 0; WebView2 `0x80070578` (`Invalid window handle`); no analysis |
| `cmd.exe /c start` from WSL-spawned PowerShell | Session 0; same WebView2 `0x80070578`; no analysis |
| `explorer.exe` with the executable path | No Mnemosyne process became observable in this automation context |
| Interactive one-shot `schtasks` (`/IT`, current Windows user) | Session 1; native window and WebView content available; no `0x80070578` |

The interactive task had to execute the staged files from the current user's
local temp directory. An initial task action targeting `C:\Windows\Temp`
returned access denied and is not app evidence.

### UI and Open-heap proof

UI Automation was run by a second interactive one-shot task in the same
session as Mnemosyne. It found:

- native window title: `Mnemosyne - JVM Heap Analysis`;
- WebView document: `Mnemosyne UI - Web content`;
- accessible `Open heap dump` button.

The post-PR #103 executable was then launched in session 1 with:

```powershell
Mnemosyne.exe --open fixture-simple.hprof
```

The desktop log under
`%LOCALAPPDATA%\mnemosyne\logs\desktop.<date>` recorded, in order:

1. `startup heap source registered`;
2. `run_desktop_analysis: starting`;
3. `run_desktop_analysis: completed`.

The process remained in session 1 and no invalid-window-handle error was
recorded. This proves native argv-to-UI heap opening and analysis completion
for a latest-main local Windows build. The opaque `sourceId` boundary remains
intact; no absolute heap path is included in this evidence.

### Diagnosis and remaining limits

The reproduced WebView2 failure is launch-context dependent: WSL-spawned
PowerShell and `cmd start` remain in non-interactive session 0, while the
interactive scheduled task runs in the user's session 1 and succeeds with the
same production executable. No Tauri window-creation code change is justified
by this evidence.

Still **NOT PROVEN**:

- clicking the packaged v0.6.0 `Open heap dump` button, selecting a fixture in
  its native dialog, and completing analysis;
- released portable v0.6.0 startup argv handling (the release predates PR
  #103);
- `explorer.exe` delegation from this WSL automation context;
- MSI / setup.exe install-and-open;
- macOS and Linux packaged launches.

