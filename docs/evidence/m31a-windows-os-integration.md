# M31.A Windows OS integration evidence

**Branch:** `feature/mat-equivalent-wave2-windows-os`

**Date:** 2026-09-16

**Scope:** Windows-first file opening, native File menu, and recent-heap verification

**Out of scope:** macOS/Linux, signing, updater, live JVM

## Implemented contract

- `tauri/tauri.windows.conf.json` declares `.hprof` and `.bin` Tauri file associations. Associated launches reuse the existing positional / `--open` parser and opaque native `sourceId` boundary.
- The Windows native menu exposes **File → Open Heap...**. Its event is handled by the shared React shell and reuses `pick_heap_file` plus the existing lean desktop analysis path.
- `scripts/windows/register-file-associations.ps1` registers the portable executable under the current user's `Software\Classes` tree and adds only `OpenWithProgids` values. It deliberately does not overwrite Windows `UserChoice`.
- The Windows portable release packager includes the registration script and checksum-first instructions.
- The existing desktop recent-load list remains session-local and reopens by opaque `sourceId`; no absolute path is exposed to React.

## Windows host proof

Host facts:

- Windows `10.0.26200.0`
- production-protocol executable SHA-256: `d8833a03636b38fd71382aba91a7248da76f550ccdac304d74a3b1faf8fe930f`
- generated MSI SHA-256: `85e6d42cfe19a090fa838b3250c1583287c019b1f36ec9b1ee6dbb0fc1a790f0`
- generated NSIS SHA-256: `668db4017db4fd79d6dc01d489f49a8d17de82a55a738891fa19dbf2dfd53970`

The desktop was built on Windows with:

```powershell
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA "Temp\mnemosyne-m31a-windows-target"
cargo tauri build --no-bundle --ci --config <temporary-config-with-prebuilt-ui>
cargo tauri build --bundles msi,nsis --ci --config <temporary-config-with-prebuilt-ui>
```

Both commands completed. The bundle run produced one MSI and one NSIS installer. Generated WiX source contained advertised `Mnemosyne.hprof` and `Mnemosyne.bin` ProgIds with `hprof` and `bin` extensions.

### Native menu

Like [PR #105](https://github.com/bballer03/Mnemosyne/pull/105), the GUI was launched through an interactive one-shot scheduled task (`schtasks /IT`) because WSL-spawned PowerShell runs in non-interactive session 0.

A second interactive task inspected the app's Win32 menu and dispatched its command ID. Sanitized result:

```json
{"process_session":1,"main_window":true,"file_menu":true,"open_heap_item":true,"picker_command_logged":true}
```

The fresh desktop log suffix contained `pick_heap_file: opening native file dialog`, proving the menu reached the existing picker command. The dialog was cancelled during cleanup; no heap path was captured.

### Portable Open With registration

The helper was parsed and executed with Windows PowerShell using process-scoped `-ExecutionPolicy Bypass` because the test branch is unsigned. It registered both extensions, preserved the `--open "%1"` command, and then removed its entries:

```json
{"registered":{"prog_id":true,"hprof_open_with":true,"bin_open_with":true,"command_uses_open_flag":true},"unregister_clean":true}
```

The release packager was then run against the Windows executable. Its zip contained `Mnemosyne.exe`, `register-file-associations.ps1`, and `README-PORTABLE.txt`.

### Recent heaps

Focused UI execution passed 17 tests. The existing regression `reopens a desktop recent load by its opaque source id` invoked analysis twice with the same opaque ID and rendered no absolute Windows or Unix path. A new menu regression proves the same opaque picker result is used by File → Open Heap.

## Local verification

- `node scripts/tests/test_m31a_windows_os_integration.mjs` — pass
- `bun test src/features/investigation/DesktopOpenHeapMenuHandler.test.tsx src/features/artifact-loader/ArtifactLoaderPage.test.tsx` — 17 pass
- `bun run test` — pass across all bounded UI batches on the fresh rerun
- `bun run lint` — pass
- `bun run build` — pass (existing Vite native-config and chunk-size warnings)
- `cargo check --workspace` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo fmt --all -- --check` — pass
- `cargo test --manifest-path tauri/session-ops/Cargo.toml --locked` — 11 pass
- Windows `cargo tauri build --no-bundle --ci` — pass (existing Windows-only dead-code warnings in log redaction)
- Windows `cargo tauri build --bundles msi,nsis --ci` — pass; 2 bundles produced
- Linux `cargo check --locked` in `tauri/` — blocked before compiling Mnemosyne by missing system package `javascriptcoregtk-4.1`; CI installs that dependency
- A local `cargo test --workspace --features test-fixtures` attempt stalled in 16 pre-existing MCP tests after more than 15 minutes and was terminated. Root Rust production code is unchanged; pull-request CI is the required full-suite gate.

## Honesty boundary

- **PROVEN:** Windows production build; MSI/NSIS generation; generated WiX extension metadata; interactive native menu visibility and picker dispatch; per-user portable registration/unregistration; portable zip contents; opaque recent-heap UI flow.
- **INHERITED PROOF:** native positional / `--open` heap analysis in interactive Windows session 1 is recorded in [M21 evidence](m21-portable-installability.md) and [PR #105](https://github.com/bballer03/Mnemosyne/pull/105).
- **NOT PROVEN:** installing this branch's MSI/NSIS and double-clicking a real associated heap from Explorer; selecting a heap through the menu dialog and completing analysis; released artifacts containing this change.
- **NOT PROVEN / OUT OF SCOPE:** signed artifacts, macOS/Linux integration, updater, live JVM attachment.
