# M20 UI workbench — evidence summary

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-14  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)  
**Plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) (Slices 20.A–20.H)

This note records what M20 *shipped* and what remains **NOT proven** on this WSL host. Claims below come from git history and focused automated tests — not from packaged GUI launch.

## What shipped (commit refs)

| Slice / theme | Commit | Summary |
| --- | --- | --- |
| Detail panels (strings, collections, top instances, unreachable) | `6dbabb6` | Artifact-backed panels + Analyzer Rail anchors |
| Desktop open heap dump | `9af0f47` | Tauri dialog + opaque `sourceId`; absolute path stays out of React |
| Desktop analyze → artifact | `f6c627c`, `1a6227f` | Sanitized `run_desktop_analysis`; pathful errors redacted |
| Snapshot list workbench + matrix | `6669d2a` | List surface + first capability ledger |
| Investigation breadcrumbs | `5af03f9`, `281bcc5` | URL-driven investigation context |
| Policies / flamegraphs / snapshot save·remove·open | `2dbe4f8`, `352b3d5` | Workbench pages; open installs graph + opaque `sourceId` |
| Terra false-green + path findings | `9ca7a1b` | Skip ≠ pass; SHA-256 key-only remove; basename-only heap column |
| Snapshot basename regression + plan gates | `f8a4dfe` | Focused UI assertion; closes remaining Terra plan gates for 20.G |
| Status / install honesty (adjacent) | `6176878`, `138905d` | M20/M21 progress notes; portable-install prerequisites |

## Command-layer / browser evidence (observed)

- Focused React tests for policy, flamegraph, and snapshot pages landed with `2dbe4f8` / `9ca7a1b` / `f8a4dfe` / `352b3d5`.
- Session-ops / Tauri command adapters for `run_ci_check`, `generate_desktop_flamegraph`, and snapshot save/remove/open landed with `2dbe4f8` / `352b3d5` (plus follow-up fixes in `9ca7a1b`).
- Evidence class on this host: **unit / component / `cargo check` command-layer**. Treat as browser-fallback and native-adapter coverage, **not** packaged desktop GUI smoke.

## Honest caveats (still open after M20 UI slices)

- Native flamegraph **16 MiB** render-limit and **overview unavailability** parity tests remain thin; enforcement still relies primarily on core/MCP paths.
- Policy workbench: native parity vs M18 MCP `ci_check` and deeper React cases (malformed policy, overview mismatch) remain partial (20.F).
- Bounded OQL MAT corpus / 22.D operator polish remain under **M22** (not an M20 UI gap); see matrix.

## NOT proven on this WSL host

| Claim | Status | Where it belongs |
| --- | --- | --- |
| Packaged Tauri GUI smoke (launch app window, exercise workbenches) | **NOT run** — WebKitGTK/GTK bundler deps absent on this WSL environment | M21 native-host evidence |
| Clean Windows unzip → double-click on a machine without preinstalled WebView2 | **NOT proven** | M21; zip remains labeled **portable with WebView2 prerequisite** |
| Browser screenshot gallery per workbench family (synthetic fixtures) | **Not captured** in this closeout | Optional follow-up; do not invent visual proof |
| Full workspace `cargo test` / `bun run test` / Tauri session-ops suite as a single 20.H gate run | **Not re-executed** in the 20.H docs commit | Prior slice commits record focused gates; full matrix remains a release discipline item |
| Final Terra M20 milestone closeout + Sol verdict | **Not recorded** | Requires human/Sol review after this evidence note |

## How to read this evidence

1. Product capability status → [ui-capability-matrix.md](../product/ui-capability-matrix.md).
2. Install / unzip claims → README Desktop section + M21 slices; never infer launch success from WSL unit green.
3. Packaged GUI and clean-Windows portable proof → deferred to **M21** native hosts.
