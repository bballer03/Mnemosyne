# Credible Eclipse MAT–equivalent product program

**Date:** 2026-09-15  
**Status:** **Sol APPROVED by product-owner autonomy** — gates for M31+/MAT golden/live AI/Open-heap opened for this program. macOS/Linux launch **explicitly deferred**.  
**Owner:** Sol (agent autonomous execution)  
**Related:** [STATUS.md](../../../STATUS.md), [roadmap §2 MAT parity](../../roadmap.md), [m31-gated-status.md](../../evidence/m31-gated-status.md), [older-partials design](2026-09-15-older-partials-closeout-design.md)

## 1. Purpose

Make Mnemosyne a **credible Eclipse MAT–equivalent investigation product** for post-mortem HPROF analysis — comparable capability and UI workflow — while preserving Mnemosyne differentiators (provenance, overview mode, MCP, ci-check, single-binary desktop).

This is **not** a pixel-perfect Eclipse RCP clone and does **not** require Java/Eclipse plugins.

## 2. Product-owner decisions (locked)

| Decision | Value |
| --- | --- |
| macOS / Linux launch matrix | **Leave open** (deferred) |
| Windows packaged Open-heap click | **In scope — required** |
| Live AI provider | **In scope** (config via env; never paste secrets in chat) |
| MAT golden / equivalence | **In scope** — versioned expected outputs + optional Eclipse MAT when available |
| M31+ | **Gates opened** for slices below; security model still required before live JVM (31.D) |
| Signing / updater (31.E) | **Deferred** until secrets exist |
| Autonomy | Agent may design, implement, PR, merge CI-green work |

## 3. Definition of “Eclipse equivalent” (success bar)

**Met when all of the following are true:**

1. **Parity matrix honesty:** Every MAT capability row in `docs/roadmap.md` §2 is ✅ for Mnemosyne **or** an explicit remaining named deferral with rationale (no silent gaps).
2. **UI loop:** Operator can open a dump, histogram → instances → inspect → GC paths → compare → OQL → export without leaving the workbench (Windows packaged proof for open path).
3. **Golden program:** At least one versioned golden corpus comparing Mnemosyne outputs to recorded expected results; Eclipse MAT cross-check when MAT is installed, else `mat-pending` label.
4. **Live AI:** Provider mode succeeds for one redacted chat/explain round-trip when API key env is present; rules fallback remains default.
5. **Perspectives:** Named layout presets (MAT-like) beyond free-form M27 persistence.
6. **Honesty:** Never claim signed artifacts, macOS/Linux launch, or full OQL `eval(...)` without evidence.

## 4. Approaches considered

| Approach | Pros | Cons |
| --- | --- | --- |
| **A. Credible product + proof (recommended)** | Matches roadmap; shippable; preserves differentiators | Not an Eclipse clone |
| B. Eclipse RCP lookalike | Familiar to MAT users | Huge UI rewrite; wrong stack |
| C. Only docs / marketing parity | Fast | Dishonest |

**Selected: A.**

## 5. Program sequence (binding)

### Wave 0 — Proof gaps (immediate)

1. **Windows packaged Open-heap** — automate or scripted UI proof on `v0.6.0` portable; record evidence. Add argv/`--open` / file-drop open if needed for reliability and ship in next release.
2. **Live AI provider** — verify provider path end-to-end with env-based keys or local provider; evidence note; improve UX if broken.

### Wave 1 — MAT equivalence core

3. **MAT golden harness** — expected-output fixtures; CI-runnable; mark `mat-referenced` only with real MAT outputs.
4. **UI perspectives (31.C)** — named presets: e.g. Leak Hunt, Dominator Browse, Compare, OQL Lab.
5. **Remaining MAT UI gaps** — superclass collapsible tree when `parentKey` exists; histogram/report polish to match MAT operator expectations.

### Wave 2 — Desktop OS integration (31.A, Windows-first)

6. File associations, Open With, recent heaps, menus — Windows first.

### Explicitly deferred

- macOS/Linux launch
- 31.D live JVM / Phase-3 dynamic plugins (needs security design first)
- 31.E signing/updater without secrets
- Full OQL `eval(...)` / arbitrary-depth nesting (named deferrals unless separately approved)

## 6. Gate status updates

| Slice | Prior | Now |
| --- | --- | --- |
| 31.A Desktop OS (Windows) | NOT MET | **OPEN** for implementation |
| 31.B MAT golden | NOT MET | **OPEN** (licensing: synthetic + optional MAT) |
| 31.C Perspectives | NOT MET | **OPEN** |
| 31.D Live JVM / plugins | NOT MET | **still gated** (security) |
| 31.E Signing | NOT MET | **still gated** (secrets) |

## 7. Evidence standard

Same honesty bar as older-partials: commands, hashes, **NOT PROVEN** rows, no unit→GUI inference. Windows packaged Open-heap requires recorded interaction (UI Automation or argv open exercised in packaged binary).

## 8. Delivery

- One PR per wave slice where practical
- Merge after CI green
- Update STATUS / roadmap / m31-gated-status as slices land
