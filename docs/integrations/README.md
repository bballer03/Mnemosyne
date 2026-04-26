# CI Integration Guides

These guides show practical ways to run Mnemosyne in CI with the current v0.3.0 CLI.

Mnemosyne returns exit code `0` when `analyze` succeeds, even if it reports leak suspects or growth. If you want Mnemosyne itself to own the policy gate, use `mnemosyne-cli ci-check` and the dedicated heap-regression sections linked below.

For the full CLI reference, see [the user guide](../user-guide.md). For installation channels and release artifacts, see [the repository README](../../README.md).

- [GitHub Actions](github-actions.md) - GitHub Actions workflows for JSON analysis, leak gates, diff artifacts, Docker runs, matrix builds, artifact uploads, and `ci-check` heap regression policies with workflow annotations plus JUnit upload.
- [Jenkins](jenkins.md) - Jenkins pipeline examples for release-binary installs, Docker-based runs, Groovy JSON leak gates, archived HTML reports, and `ci-check` JUnit/XML policy gates.