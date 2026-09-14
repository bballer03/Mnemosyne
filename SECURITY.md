# Security Policy

## Supported Versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |

## Reporting a Vulnerability

If you believe you have found a security vulnerability in Mnemosyne, please do
not open a public issue.

Report it privately using one of these channels:

- Email the project maintainers at yousufsarfaraz484@gmail.com
- Open a private GitHub security advisory for the repository

When possible, include:

- A clear description of the issue and potential impact
- Affected version or commit information
- Reproduction steps, proof of concept, or a minimal test case
- Any mitigations or workarounds you have identified

## Response Expectations

The maintainers aim to:

- Acknowledge new reports within 5 business days
- Provide a status update within 10 business days when triage requires more time
- Coordinate disclosure and remediation before public discussion when a report is confirmed

## Sensitive Data Considerations

Mnemosyne processes JVM heap dumps, and heap dumps may contain sensitive data
such as credentials, tokens, personal information, and application internals.

When reporting security issues:

- Avoid posting heap dumps, raw memory contents, secrets, or production data in public
- Prefer redacted samples or minimized repro cases whenever possible
- Share sensitive artifacts only through a private channel and only when necessary for investigation

## Desktop app distribution (M16)

Tagged GitHub Releases may include Tauri desktop installers (`.msi`/`.exe` on Windows,
`.dmg`/`.app` on macOS, `.deb`/`.AppImage`/`.rpm` on Linux) alongside the CLI archives.
Post-M21 releases may also attach `Mnemosyne-<version>-windows-x64-portable.zip`
(**portable with WebView2 prerequisite** — no JVM, but not JVM-free alone).

### Code signing status

Release CI **attempts** platform signing only when the corresponding GitHub Actions
secrets are configured. **Until those secrets exist, all desktop installers ship
unsigned.** Do not assume signed or notarized artifacts unless a release explicitly
documents that signing secrets were present in CI.

| Platform | Signing gate | When unsigned (current default) |
| --- | --- | --- |
| **Windows** | `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` (optional: `WINDOWS_TIMESTAMP_URL`; CI derives the cert thumbprint and injects `bundle.windows.*` into `tauri.conf.json` before `tauri build`) | **SmartScreen** may show "Windows protected your PC" on first run. Click **More info** → **Run anyway**, or unblock the file in file Properties. |
| **macOS** | `APPLE_CERTIFICATE` (must be a **Developer ID Application** certificate — **not** Apple Development), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `KEYCHAIN_PASSWORD` | **Gatekeeper** blocks unidentified developers. **Right-click** the app (or `.dmg`) → **Open** → confirm **Open** once. Subsequent launches work normally. CI skips signing/notarization and ships unsigned when only an Apple Development certificate is configured. |
| **Linux** | None (unsigned by design) | No OS-level signing gate for `.deb`/`.AppImage`/`.rpm`. Verify downloads via the GitHub Release tag and published checksums when available. |

CI logs a clear line per platform when secrets are missing (see `.github/workflows/release.yml`,
`build-desktop` job) so maintainers know where to add certificates later.

### Auto-update (v1: skipped)

The desktop app **does not** ship an in-app auto-updater in v1. Tauri's updater plugin
requires a signing-key trust chain that overlaps with the code-signing constraints above.
Users should **re-download** new versions from [GitHub Releases](https://github.com/bballer03/mnemosyne/releases),
the same distribution model as the CLI today. Auto-update may be revisited once signing
infrastructure is in place.