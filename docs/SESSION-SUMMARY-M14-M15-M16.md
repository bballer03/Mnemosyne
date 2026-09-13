# Mnemosyne: AI-Native MAT-Parity Initiative — Session Summary

**Date:** 2026-09-13
**Scope:** M14 (UI parity + AI-native design), M15 (MAT backend parity), M16 (desktop packaging)

## What Mnemosyne is

Rust JVM heap-dump analysis tool (CLI + core + web UI + Tauri desktop shell), targeting
feature/usability parity with Eclipse Memory Analyzer (MAT) while staying AI-native and
scriptable (MCP server, OQL query engine, policy-as-code CI checks).

## Why this initiative

User directive: make this a "truly AI-native and UI-friendly product," matching Eclipse MAT's
capability and adoption ease, packaged as a downloadable GUI app like MAT itself — **without
dumbing the product down**. Explicit constraint: full MAT power must stay reachable, but a simple
interface must sit on top of it, not replace it.

## Decisions made

1. **Sequencing**: MAT feature parity first, packaging last. Scope = backend AND UI interaction
   parity (not backend-only) — a user must be able to *do* in the browser UI what MAT lets them do
   in its desktop UI, not just get the same JSON from the CLI.
2. **AI-native design resolved as**: a guided landing page (`GuidedLanding`, natural-language input
   bar, triage summary, workflow cards) as the front door, sitting *alongside* — never gating —
   the existing power-user routes (heap explorer, OQL console, comparison basket, artifact
   explorer). Nothing is hidden behind the simple mode; it's a fast path in front of the full
   surface.
3. **Plugin system scoped to Phase 2 only** (M15.F): static compile-time trait registry
   (`AnalyzerPlugin`, `ReportFormatterPlugin`). Phase 3 (dynamic `cdylib` loading, filesystem
   discovery, config-driven plugin paths) stays out of scope, per the pre-existing
   `docs/design/m6-plugin-extension-system.md` gate — no third-party plugin demand exists yet to
   justify it.
4. **OQL scope bounded** to 5 concrete additions (traversal functions `outbounds`/`inbounds`/
   `dominators`, regex `=~`, one-level subqueries, `UNION`) — explicitly excluding `eval()`,
   multi-class `FROM`, and deep nesting, to keep the query engine's attack surface and complexity
   bounded.
5. **Desktop packaging (M16)** hardens the existing `tauri/` scaffold into a CI-built installer
   pipeline rather than a from-scratch rewrite. Code-signing is explicitly out of scope for now —
   this sandbox has no signing credentials, flagged up front rather than discovered mid-slice.

## Roadmap additions

`docs/roadmap.md` gained three new milestones with design docs:

- [`docs/design/milestone-14-ui-parity-ai-native.md`](../docs/design/milestone-14-ui-parity-ai-native.md) — UI backend-parity + AI-native landing.
- [`docs/design/milestone-15-mat-backend-parity.md`](../docs/design/milestone-15-mat-backend-parity.md) — OQL engine extensions + Phase-2 plugin runtime.
- [`docs/design/milestone-16-desktop-packaging.md`](../docs/design/milestone-16-desktop-packaging.md) — CI-built Tauri installers.

## What shipped this session

Every slice below followed the project's design-doc-gated, TDD, additive-only-backward-compat
process, was built by an isolated subagent in its own git worktree, and was independently
re-verified (full `cargo test`/`clippy`/`fmt` and `bun test`/`tsc`, plus manual diff review) before
merge — never trusting the agent's self-reported numbers.

**M14 — UI parity + AI-native design (5/5 slices complete)**
| Slice | What |
|---|---|
| 14.A | Comparison basket UI for object-level heap diff (`__MNEMOSYNE_COMPARISON_BRIDGE__`) |
| 14.B | Object inspector chip navigation + multi-path GC-path view |
| 14.C | Referrer, classloader, and thread explorer panels (artifact-backed, no live bridge needed) |
| 14.D | AI-guided landing page, workflow cards, natural-language input, persistent power-route nav (`__MNEMOSYNE_WORKFLOW_BRIDGE__`) |
| 14.E | Visual consistency pass + M14 documentation sync |

**M15 — MAT backend parity (6/7 slices complete; 15.G remaining)**
| Slice | What |
|---|---|
| 15.A | Duplicate primitive-array detection |
| 15.B | Group-by-superclass histogram |
| 15.C | OQL traversal functions: `outbounds`/`inbounds`/`dominators` |
| 15.D | OQL regex operator `=~` |
| 15.E | OQL one-level subqueries + `UNION` (found and fixed a real per-side-LIMIT ordering bug) |
| 15.F | Phase 2 static plugin/extension runtime (`AnalyzerPlugin`, `ReportFormatterPlugin`, `PluginRegistry`) |
| 15.G | **Not started** |

**M16 — Desktop packaging (1/4 slices complete; 16.B-D remaining)**
| Slice | What |
|---|---|
| 16.A | Tauri installer builds wired into the release pipeline (produced a real local `.msi`/`.exe`) |
| 16.B-D | **Not started** |

Current `main` tip: `75fee0f`. Rust test suite: 807 passing. UI test suite: 285 passing.
clippy/fmt/tsc all clean at time of writing.

## Notable incidents / lessons learned

- **Agent-in-shared-checkout incident**: an M15.D subagent, after a rate-limit interruption and
  resume, worked directly in the shared `D:\Mnemosyne` checkout instead of its isolated worktree
  and committed to shared `main`. It self-corrected with an unprompted `git reset --hard`.
  Independently verified via `git reflog` + full test run that `main` was exactly restored, then
  disclosed the incident to the user rather than accepting the agent's own "already resolved"
  framing. Process fix: every subsequent agent dispatch now opens with an explicit
  "verify pwd/worktree before doing anything, STOP if in the shared checkout" warning.
- **GitNexus MCP server was unavailable the entire session** (`CONNECT_TIMEOUT`), despite
  `CLAUDE.md` mandating its use for impact analysis before edits. Every agent substituted direct
  code reads, grep-based blast-radius checks, and the full verification gate, flagging the
  deviation each time. This remains a standing gap — GitNexus should be checked/reconnected before
  the next round of edits so impact analysis can resume.
- **Test-infra fixes found along the way**: a real cross-file test race on
  `MNEMOSYNE_SNAPSHOT_DIR` (two independent locks that didn't protect each other) was fixed with
  one crate-shared lock; the UI test runner was rebalanced into 4 batches after discovering the
  full combined suite reliably OOMs in this sandbox.

## What remains

- M15.G (final MAT-parity slice, not yet started/scoped in detail beyond the roadmap entry).
- M16.B-D (desktop packaging: code signing strategy, auto-update, multi-platform installer
  verification).
- Environment permanently blocks: no Eclipse MAT available in this sandbox for side-by-side
  comparison, no native Linux target (affects M12, pre-existing), no code-signing credentials
  (M16 R1).

## Process note

Per explicit instruction, autonomous dispatch stops here — no further slices (15.G, 16.B-D) were
started after this document. Resume by picking up 15.G from `docs/design/milestone-15-mat-backend-parity.md` §5.
