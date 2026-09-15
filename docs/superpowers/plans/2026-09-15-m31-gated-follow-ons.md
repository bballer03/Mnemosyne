# M31+ Gated Follow-ons Stub Plan

> **For agentic workers:** Do not implement these items from this stub. When a gate is met and product scope is approved, use superpowers:brainstorming and superpowers:writing-plans to create a dedicated implementation plan.

**Goal:** Keep optional ecosystem work visible without allowing it to block MAT post-mortem investigation maturity.

**Architecture:** No architecture is approved by this stub. Each item requires its named product, security, licensing, resource, or native-host gate before design begins.

**Tech Stack:** Gated; depends on the approved slice.

**Roadmap:** [M31+ — Optional / gated](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m31--optional--gated)

## Global Constraints

- Every item below is a labelled stub, not an implementation-ready task.
- M24–M27 stability and M30 evidence take priority.
- Live JVM attachment and dynamic plugins require security and resource-model review.
- WSL cannot close native OS integration, signing, updater, notarization, or MAT equivalence gates.
- Optional work must not block the deterministic post-mortem MAT loop.

---

### 31.A — Desktop OS integration — GATED

- [ ] Gate: M24–M27 lifecycle/persistence is stable and native host owners are available.
- [ ] Host-blocked: verify menus, file associations, recent heaps, and deep links on Windows, macOS, and Linux.

**2026-09-15 assessment:** Gate **NOT MET** on WSL. Persistence stability is evidenced (M27), but native host owners / packaged apps are absent. See [m31-gated-status.md](../../evidence/m31-gated-status.md).

### 31.B — MAT golden/equivalence program — GATED

- [ ] Gate: approve fixture licensing, MAT versions, expected-output storage, and reference workstation.
- [ ] Host/tool-blocked: run versioned Eclipse MAT outputs; do not infer equivalence from Mnemosyne-only fixtures or WSL.

**2026-09-15 assessment:** Gate **NOT MET**. Licensing + MAT reference workstation not approved.

### 31.C — Perspectives as layout presets — GATED

- [ ] Gate: M24 workbench density and M27 persisted layout evidence demonstrate a real preset need.
- [ ] Scope remains named presets, not Eclipse-style free-form docking.

**2026-09-15 assessment:** Gate **NOT MET**. No product decision that named presets are required beyond M27 persistence.

### 31.D — Phase-3 plugins / live JVM — GATED

- [ ] Gate: documented out-of-tree plugin or live-attach demand.
- [ ] Required reviews: security boundary, ABI/versioning, sandboxing, authentication, cancellation, memory/CPU budgets, and failure isolation.
- [ ] Must not weaken heap privacy or block post-mortem analysis.

**2026-09-15 assessment:** Gate **NOT MET**. No security/resource review or demand record.

### 31.E — Signing / updater / Homebrew Cask — GATED

- [ ] Gate: signing/notarization secrets plus an explicit updater/channel product decision.
- [ ] Host-blocked: validate signed installers, update rollback, and platform warnings on matching native hosts.
- [ ] Never label artifacts signed/notarized/updatable based on CI configuration alone.

**2026-09-15 assessment:** Gate **NOT MET**. No secrets/product decision; do not claim signed from CI alone.
