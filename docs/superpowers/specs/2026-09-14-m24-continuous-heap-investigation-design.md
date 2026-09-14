# M24 — Continuous Heap Investigation (heap-first workbench)

**Status:** Design approved (product autonomy 2026-09-14)  
**Date:** 2026-09-14  
**Depends on:** M20 surfaces, M17/M23 bridges, Track A desktop bridge injection (`ui/src/host/tauri-bridge.ts`)  
**Supersedes for IA:** Route-first primary nav as the organizing metaphor for desktop investigation

---

## 1. Product thesis

M24 makes the **heap dump**—not routes or Eclipse-style perspectives—the primary task object. Opening a heap creates one persistent **Continuous Heap Investigation** workspace that preserves context from triage through diagnosis.

AI remains a **collapsible, provenance-labelled advisory pane**, never the destination and never a substitute for measured facts.

---

## 2. Why now

Packaged desktop already ships analysis commands, but the UX still feels like a multi-app portal (Home → Dashboard → Explorer → Assistant → leak subroutes). Users who open a `.hprof` expect an Eclipse-MAT-like continuous investigation loop, not a marketing/home screen that claims “open in the desktop app” while already inside it (fixed separately in Track A).

M24 is the product answer after that honesty fix: **one workspace, durable selection, synchronized panes.**

---

## 3. Primary user journey

1. Open or reopen a heap (desktop picker / recent / snapshot) or import an analysis JSON (browser).
2. Bounded analysis with progress and cancel; honest mode/provenance banner.
3. Prioritized findings (leaks / triage) → select class, object, or leak.
4. Inspect synchronized Histogram, Dominators, Inspector, GC Paths (and other panes as needed).
5. Leave and return without losing workspace identity or selection.
6. Optionally open Assistant for guidance that deep-links into deterministic panes.
7. Save snapshot or compare (compare may land late in the milestone; see risks).

---

## 4. Information architecture

### Home (`/`)

Only: **Open Heap**, **Recent Heaps**, **Import Artifact**. No investigation chrome until a heap/artifact is loaded.

### Workbench (after open)

- Persistent heap identity (basename + opaque `sourceId`)
- Analysis status / mode / provenance
- Findings queue
- Global search (bounded; no invented OQL)
- Breadcrumbs + resizable / tabbed investigation panes

### Panes (compose existing surfaces)

Overview · Histogram · Dominators · Inspector · GC Paths · Threads · Classloaders · Referrers · OQL · Compare

### Collapses from primary navigation

`/dashboard`, `/artifacts/explorer`, `/assistant`, and leak subroutes become **workbench panes** (or pane tabs). Preserve deep links as redirects into equivalent workbench state.

### Tools (secondary)

Policies · Snapshots · Flamegraphs — reachable without becoming the home metaphor.

### Explicitly deferred

Named “perspectives” as layout presets only (nice-to-have after M24). Not the organizing model.

---

## 5. Non-goals (M24)

- Full Eclipse perspective / plugin model or free-form docking
- New analyzers, OQL expansion, or MAT index compatibility
- Live JVM attachment
- Autonomous AI diagnosis or source modification
- Hiding deterministic tools behind chat
- Unrelated visual redesign / rebrand
- Signing, updater, or distribution-channel expansion

---

## 6. Slices

| Slice | Goal | Acceptance (one-liner) |
|---|---|---|
| **24.A** Workspace shell | Enter a persistent shell on open/import | Shell shows heap identity, mode, provenance, and honest loading/error states |
| **24.B** Open → triage | Reopen recent; progress/cancel; actionable triage | User can open/reopen, cancel bounded analysis, and get triage or recovery guidance |
| **24.C** Synchronized investigation | Shared selection across core panes | Selection + filters/sort/back-forward preserved across Histogram/Dominators/Inspector/GC Paths |
| **24.D** Guided continuity | Findings + Assistant as advisory | Assistant deep-links into deterministic panes; rules mode works offline; AI labelled |
| **24.E** Compare + proof | Two-heap compare in same shell + evidence | Match quality visible; keyboard-complete paths; packaged native open→triage evidence (not WSL-only) |

Gate **24.E compare** if single-heap continuity (24.A–D) slips; compare must not block shipping the continuous shell.

---

## 7. Honesty / provenance constraints

- Deterministic measured facts stay visually primary and separate from AI output.
- Label every AI result as `rules` / `provider` / `fallback`; rules remains offline default.
- Never imply AI or bounded queries replace MAT-equivalent analysis.
- Show partial, heuristic, unavailable, skipped, and failed states explicitly (`evaluation_complete` / skip ≠ green).
- Absolute heap paths never enter React; opaque `sourceId` + basename only.
- Preserve bounded session history and existing redaction / provider safeguards.
- Unit tests and WSL browser evidence ≠ packaged native launch or live-provider proof.

---

## 8. Prerequisites (Track A — not part of M24 scope)

Desktop must inject host bridges from the **UI bundle** (`ui/src/host/tauri-bridge.ts` + Tauri ACL `allow-desktop-commands`) so “Open heap dump” works inside the packaged app. M24 assumes that hotfix is merged.

---

## 9. Risks / open questions

1. Cross-pane selection may need a stronger shared workspace-state contract than today’s route stores.
2. Automatic triage needs strict memory, latency, cancellation, and failure bounds.
3. Pane density can recreate Eclipse complexity — progressive disclosure + a11y required.
4. Desktop vs browser-artifact capability states must stay explicit.
5. Compare may exceed M24 capacity — gate after single-heap continuity is proven.

---

## 10. Success criteria (milestone)

- Opening a heap on packaged desktop enters the workbench without a “use the desktop app” dead end.
- User can triage → inspect object → GC path → return without reloading or losing selection.
- Assistant is optional and provenance-labelled; rules path works without a provider.
- Docs (STATUS, capability matrix, roadmap pointer) describe M24 as the active UI milestone without claiming MAT UI equivalence.
