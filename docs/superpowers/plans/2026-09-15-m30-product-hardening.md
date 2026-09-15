# M30 Product Hardening Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close visual, accessibility, test-memory, native-evidence, logging, and documentation gaps after the MAT investigation loop is functionally complete.

**Architecture:** Consolidate existing tokens/primitives and evidence pipelines; do not redesign analysis contracts. Native release claims come only from matching native hosts, while WSL remains a unit/command environment.

**Tech Stack:** React/CSS, Testing Library, Bun test batches, Tauri release CI, Rust tracing, documentation/evidence.

**Roadmap:** [M30 — Product hardening](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m30--product-hardening)

## Global Constraints

- No native-launch, MAT-golden, signing, or notarization claim without direct evidence.
- Keep full production routes out of unit tests and enforce per-batch memory ceilings.
- Never log paths, field values, query bodies, prompts, tokens, or PII; rotate `desktop.log`.
- Accessibility acceptance requires automated checks plus manual keyboard evidence.
- Documentation must describe current runtime truth and explicit `NOT PROVEN` gaps.

---

## File map

- Theme/responsive: `ui/src/app/globals.css`, shared workbench/panel components.
- Keyboard/a11y: core-loop panels in `ui/src/features/artifact-explorer/`, `heap-explorer/`, `leak-workspace/`.
- Test memory: `ui/run-tests.ts`, `.github/workflows/ci.yml`.
- Native evidence: `.github/workflows/release.yml`, `docs/evidence/`.
- Logging: `tauri/src/logging.rs`, operation logging in `tauri/src/commands.rs`.
- Drift cleanup: `STATUS.md`, `ARCHITECTURE.md`, `docs/product/ui-capability-matrix.md`, `docs/roadmap.md`.

### M30.A — Theme and responsive system

- [ ] Inventory literal color/spacing values and expand existing tokens before component migration.
- [ ] Verify Home and workbench at narrow widths without hiding primary actions.

### M30.B — Keyboard and accessibility

- [ ] Plan keyboard-complete Open → histogram → instance → Inspector → GC-path flow.
- [ ] Add automated accessible-name/state/focus tests and native manual evidence checklist.

### M30.C — Native release evidence

- [ ] Run packaged Windows/macOS/Linux open → inspect → path scenarios only on matching hosts.
- [ ] Record hashes, versions, signing state, failures, screenshots, and `NOT PROVEN` rows.
- [ ] Fold remaining M20–M23 native/Terra/Sol evidence only where actually demonstrated.

### M30.D — UI test memory safety

- [ ] Measure each `ui/run-tests.ts` batch RSS and fail above documented ceilings.
- [ ] Add store/bridge reset helpers; preserve fresh-process isolation for heavy suites.
- [ ] Treat SIGTERM/hang as a failed gate with the affected batch named.

### M30.E — Documentation and logging closeout

- [ ] Add size/daily rotation for `desktop.log` with redaction tests.
- [ ] Reconcile STATUS, ARCHITECTURE, roadmap, capability matrix, bridge list, and OQL deferrals.
- [ ] Run placeholder/contradiction scan and record remaining gated work under M31+.
