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