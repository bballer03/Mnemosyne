# Milestone 2 — Packaging, Releases, and Developer Experience

> **Status:** ✅ Complete  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Make Mnemosyne easy to install, use, and contribute to — establishing the distribution, developer experience, and community infrastructure needed for open-source adoption.

## Context

No one adopts a tool they can't easily install. After M1 established the analytical foundation, M2 focused on everything surrounding the core analysis: how users discover, install, run, and contribute to Mnemosyne. This milestone addresses the entire developer experience from first contact to first contribution.

## Scope

- **Release automation** — GitHub Actions workflow for cross-compile + tag-validated releases
- **Binary distribution** — prebuilt archives for 5 targets (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
- **Package managers** — `cargo install mnemosyne-cli`, Homebrew formula
- **Docker image** — multi-stage Dockerfile, GHCR publishing on tagged releases
- **CLI UX** — progress spinners (indicatif), colorized output (anstream), comfy-table aligned output with truncation disclosure
- **Error messaging** — structured `CoreError` variants, `hint:` lines, nearby-file suggestions, config-fix hints
- **Community files** — CODE_OF_CONDUCT.md, SECURITY.md, issue templates, PR template
- **Documentation** — README, QUICKSTART, ARCHITECTURE, STATUS maintained in sync
- **crates.io metadata** — workspace + crate manifests ready for publish

## Non-scope

- Core analysis changes (M1/M1.5)
- AI/LLM wiring (M5)
- Web UI (M4)
- MAT-style analysis features (M3)
- Performance benchmarks (M3/M6)
- Real-world HPROF validation (M1.5)

## Architecture Overview

M2 did not change the core architecture. All changes were in the presentation and distribution layers:

```
┌────────────────────────────────────────────────────────────┐
│                    DISTRIBUTION LAYER                       │
│                                                            │
│  GitHub Releases ──── .tar.gz/.zip (5 targets)             │
│  crates.io ────────── cargo install mnemosyne-cli          │
│  Homebrew ─────────── brew install ./HomebrewFormula/...   │
│  GHCR ─────────────── docker pull ghcr.io/.../mnemosyne   │
└────────────────────────────────────────────────────────────┘
          │
┌─────────┼──────────────────────────────────────────────────┐
│         ▼        CLI PRESENTATION LAYER                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  indicatif spinners  │  anstream colors              │  │
│  │  comfy-table output  │  truncation disclosure        │  │
│  │  CoreError + hint:   │  nearby .hprof suggestions    │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
          │
┌─────────┼──────────────────────────────────────────────────┐
│         ▼        CORE (unchanged)                          │
│  hprof/ │ graph/ │ analysis/ │ mapper/ │ report/ │ fix/   │
└────────────────────────────────────────────────────────────┘
```

## Module/File Impact

| File | Change | Status |
|---|---|---|
| `.github/workflows/release.yml` | New: release automation workflow | ✅ Done |
| `.github/workflows/ci.yml` | Updated: maintained CI pipeline | ✅ Done |
| `Cargo.toml` | Updated: workspace metadata for crates.io | ✅ Done |
| `core/Cargo.toml` | Updated: crate metadata for crates.io | ✅ Done |
| `cli/Cargo.toml` | Updated: crate metadata + versioned core dependency | ✅ Done |
| `cli/src/main.rs` | Updated: spinners, colors, comfy-table, error handling | ✅ Done |
| `core/src/errors.rs` | Updated: structured CoreError variants | ✅ Done |
| `core/src/hprof/parser.rs` | Updated: validate_heap_file() with suggestions | ✅ Done |
| `Dockerfile` | New: multi-stage build + non-root runtime | ✅ Done |
| `.dockerignore` | New: Docker build context exclusions | ✅ Done |
| `HomebrewFormula/mnemosyne.rb` | New: Homebrew formula with SHA256 checksums | ✅ Done |
| `CODE_OF_CONDUCT.md` | New | ✅ Done |
| `SECURITY.md` | New | ✅ Done |
| `.github/ISSUE_TEMPLATE/` | New: bug report + feature request templates | ✅ Done |
| `cli/tests/integration.rs` | Updated: 23 integration tests (from 16) | ✅ Done |

## API/CLI/Reporting Impact

- CLI now shows progress spinners for long-running operations
- CLI output uses colorized labels and severity indicators
- `parse` and `leaks` output uses comfy-table aligned terminal tables
- Truncated values get disclosure sections showing full content
- Error messages include structured `hint:` lines with suggestions
- Missing files suggest nearby `.hprof` files
- Invalid configs surface fix hints
- No changes to core analysis API or report format structure

## Data Model Changes

### New Error Types
- `CoreError::FileNotFound` — with path and suggestions
- `CoreError::NotHprof` — with extension and guidance
- `CoreError::HprofParseFailed` — with phase context
- `CoreError::ConfigError` — with fix suggestions

No changes to analysis, graph, or report data models.

## Validation/Testing Strategy

- 87 tests total (59 core + 5 CLI unit + 23 CLI integration)
- 4 new CLI integration tests for error-path UX (M2-B6)
- 4 new CLI integration tests for table output (M2-B7)
- CI runs cargo check, test, clippy, fmt on all PRs
- Release workflow validated with actual v0.1.1 tag

## Rollout/Implementation Phases

All phases delivered:
1. ✅ M2-B1: CLI UX — spinners, colors
2. ✅ M2-B2: Release automation — cross-compile + publish workflow
3. ✅ M2-B3: Packaging — cargo install + Homebrew formula
4. ✅ M2-B4: Docker image — Dockerfile + GHCR publishing
5. ✅ M2-B5: Community files — templates, CODE_OF_CONDUCT, SECURITY
6. ✅ M2-B6: Better error messages — structured errors with hints
7. ✅ M2-B7: Table-formatted CLI output — comfy-table + truncation disclosure

## Risks and Open Questions

| Risk | Status | Resolution |
|---|---|---|
| Cross-compilation complexity | ✅ Resolved | Used cargo cross-compile targets successfully |
| crates.io name availability | ✅ Resolved | `mnemosyne-core` and `mnemosyne-cli` published |
| Homebrew SHA256 management | ✅ Resolved | Checksums populated for v0.1.1 release |
| Dockerfile base image CVEs | ⚠️ Triaged | M3-B attempted Docker Scout first and then used fallback Grype scans of saved runtime and builder-stage images when Scout auth was unavailable; the shipped runtime image had no critical findings and only `wont-fix` high findings, while the builder-stage scan was noisier but non-shipping, so no safe minimal same-family remediation was justified yet |

### Retrospective Notes
M2 was a clean success. All batches delivered without regressions. The workspace grew from 67 to 87 tests. The CLI now provides a polished experience for first-time users. Distribution channels are established and functional.
