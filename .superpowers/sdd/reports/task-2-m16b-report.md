# Task 2 — M16.B Terra Review Fix Report

## Findings addressed

### 1. Windows — PFX import alone does not sign

**Problem:** Importing the PFX into `Cert:\CurrentUser\My` does not configure Tauri to invoke `signtool`. Tauri requires `bundle.windows.certificateThumbprint`, `digestAlgorithm`, and `timestampUrl` in `tauri.conf.json`.

**Fix:**
- Renamed resolve step output to `secrets_present` (no premature "ENABLED" log).
- Merged import + configure into `Import Windows code-signing certificate and wire Tauri signing`:
  - Import PFX into cert store (for signtool).
  - Derive SHA-1 thumbprint via `Get-PfxData`.
  - Patch `tauri/tauri.conf.json` with `certificateThumbprint`, `digestAlgorithm=sha256`, and `timestampUrl` (default `http://timestamp.comodoca.com`, overridable via optional `WINDOWS_TIMESTAMP_URL` secret).
  - Log **ENABLED** only after thumbprint is resolved and config is patched.

### 2. macOS — `APPLE_TEAM_ID` required for Apple-ID notarization

**Problem:** CI logged signing ENABLED without `APPLE_TEAM_ID`, but Tauri CLI requires it when `APPLE_ID` + `APPLE_PASSWORD` are set for notarization.

**Fix:**
- Gate `mac-signing` `enabled=true` on full secret set including `APPLE_TEAM_ID`.
- Updated missing-secrets log to list `APPLE_TEAM_ID` as required (Apple-ID path).
- Pass Apple env vars to `tauri-action` only when `mac-signing.outputs.enabled == 'true'`.
- Updated `SECURITY.md` — removed "(optional: APPLE_TEAM_ID)" from macOS signing gate table.

## Constraints preserved

- Unsigned fallback + loud no-op when secrets absent (unchanged for Linux; Windows/macOS log clearly when skipping).
- Auto-update still SKIPPED for v1 (unchanged step).
- CLI `build` job untouched.
- Docs still state v1 default is unsigned; no claim that signed artifacts exist today.

## Files changed

- `.github/workflows/release.yml`
- `SECURITY.md`
