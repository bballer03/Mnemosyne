# Task 2 Report — Slice 16.B: Conditional code-signing + auto-update decision

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-13  
**Status:** Complete

## Goal

Wire release CI so desktop signing steps check for GitHub Actions secrets and no-op loudly when absent; record explicit v1 auto-update skip decision; document SmartScreen/Gatekeeper caveats.

## Changes

### `.github/workflows/release.yml` (`build-desktop` job)

Added steps **before** the existing `Build Tauri bundle` step (16.A path unchanged):

1. **Desktop auto-update (v1 decision)** — always logs that auto-update is SKIPPED for v1; users re-download from GitHub Releases.
2. **Resolve code-signing (Linux)** — logs that Linux bundles ship unsigned by design (no secrets needed).
3. **Resolve code-signing (Windows)** — checks `WINDOWS_CERTIFICATE` + `WINDOWS_CERTIFICATE_PASSWORD`; sets `win-signing.enabled` output; logs ENABLED or NOT configured + SmartScreen caveat.
4. **Import Windows code-signing certificate** — runs only when `win-signing.enabled == true`; imports base64 `.pfx` into cert store for Tauri Authenticode signing.
5. **Resolve code-signing (macOS)** — checks `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `KEYCHAIN_PASSWORD`; logs ENABLED or NOT configured + Gatekeeper workaround.
6. **Import Apple Developer certificate** — runs only when `mac-signing.enabled == true`; creates CI keychain, imports `.p12`, exports `APPLE_SIGNING_IDENTITY` to `GITHUB_ENV`.

Extended **Build Tauri bundle** `env` with Apple signing/notarization secrets (empty when absent → unsigned build). Windows signing uses cert store import; no extra env vars required beyond Tauri's default `bundle.windows` behavior when a cert is present.

**Not changed:** `build` (CLI matrix), `docker`, `release` jobs — unsigned CLI path and artifact attachment from 16.A remain intact.

### `SECURITY.md`

New section **Desktop app distribution (M16)** covering:

- Conditional signing table (Windows/macOS/Linux secrets + unsigned user experience)
- SmartScreen and Gatekeeper workarounds
- Explicit v1 auto-update skip with rationale

### `README.md`

Short pointer under Installation / desktop scaffold to `SECURITY.md` for signing caveats and auto-update skip.

## Auto-update decision

**SKIP for v1.** No Tauri updater plugin wired. Overlaps with code-signing trust chain (update artifacts must be signed). Users re-download from GitHub Releases, matching CLI distribution. Documented in CI log step, `SECURITY.md`, and `README.md`.

## Validation

| Gate | Result |
| --- | --- |
| Workflow YAML parses | ✅ `python3 -c "import yaml; yaml.safe_load(...)"` |
| Signing steps gated on secrets | ✅ `win-signing` / `mac-signing` outputs + conditional import steps |
| Clear CI log when secrets absent | ✅ explicit `echo` lines per platform (not silent skip) |
| Unsigned fallback documented | ✅ `SECURITY.md` + `README.md` pointer |
| Auto-update decision explicit | ✅ CI step + docs |
| CLI release path preserved | ✅ no edits to `build` or `release` jobs |
| Signed artifacts claimed | ❌ **Not claimed** — no signing credentials in this environment |

## Secrets reference (for future maintainers)

| Secret | Platform | Purpose |
| --- | --- | --- |
| `WINDOWS_CERTIFICATE` | Windows | Base64-encoded `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows | PFX export password |
| `APPLE_CERTIFICATE` | macOS | Base64-encoded `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | P12 export password |
| `APPLE_ID` | macOS | Notarization Apple ID |
| `APPLE_PASSWORD` | macOS | App-specific password for notarytool |
| `KEYCHAIN_PASSWORD` | macOS | Ephemeral CI keychain password |
| `APPLE_TEAM_ID` | macOS | Optional team ID override |

## Commit

One Conventional Commit on `sync/m15g-m16bcd` for Slice 16.B.
