# CI Integration Guides

These guides show practical ways to run Mnemosyne in CI with the current v0.2.0 CLI.

Mnemosyne returns exit code `0` when analysis succeeds, even if it reports leak suspects or growth. If you want CI to fail on findings, add an explicit threshold check in your workflow or pipeline.

For the full CLI reference, see [the user guide](../user-guide.md). For installation channels and release artifacts, see [the repository README](../../README.md).

- [GitHub Actions](github-actions.md) - GitHub Actions workflows for JSON analysis, leak gates, diff artifacts, Docker runs, matrix builds, and artifact uploads.
- [Jenkins](jenkins.md) - Jenkins pipeline examples for release-binary installs, Docker-based runs, Groovy JSON leak gates, and archived HTML reports.