# M24 — Continuous Heap Investigation (heap-first workbench)

**Status:** Design approved (product autonomy 2026-09-14)  
**Date:** 2026-09-14  
**Product release target:** **v0.5.0** (do not ship under 0.4.x; v0.4.1 is the desktop bridge hotfix only)  
**Depends on:** M20 surfaces, M17/M23 bridges, Track A desktop bridge injection (`ui/src/host/tauri-bridge.ts`) — landed in v0.4.1  
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
- **Open another** and **Close** always available in workbench chrome (not Home-only)
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
| **24.A** Workspace shell | Enter a persistent shell on open/import; Open another / Close reachable | Shell shows heap identity, mode, provenance, Open another/Close, and honest loading/error states |
| **24.B** Open → triage | Reopen recent; progress/cancel; actionable triage; transactional replace | User can open/reopen/open-another/close, cancel bounded analysis, and get triage or recovery guidance |
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

## 8. Theme & visual consistency (v0.5.0)

M24 must feel like **one modern workbench**, not a collage of independently styled routes.

### Principles

1. **One token system** — Extend `ui/src/app/globals.css` with CSS variables for surface, border, text, accent, danger, warning, success, and provenance/AI advisory. Prefer tokens over hard-coded hex in new components.
2. **Preserve the existing dark workbench base** — Keep the current charcoal canvas (`#0a0c10` / Inter stack) as the product DNA for continuity with M14–M23. Evolve density and hierarchy; do not rebrand to a new “AI purple” or cream-serif look.
3. **Facts vs advisory** — Measured heap data uses primary text/surfaces. AI / rules / provider / fallback content uses a distinct but quiet advisory treatment (border/label), never a louder hero palette than the data.
4. **Consistent chrome** — Shared shell for heap identity, mode/provenance banner, pane tabs, and status. Same spacing scale, heading levels, and control sizes across panes.
5. **Progressive density** — Prefer calm empty states and progressive disclosure over Eclipse-style clutter. Cards only when they bound an interaction.
6. **Motion with purpose** — Soft pane focus / selection transitions (2–3 intentional motions), not decorative noise.
7. **No MAT skin-copy** — Visual language is Mnemosyne’s; equivalence is **workflow and capability**, not cloning Eclipse chrome.

### Non-goals for theme

- Light-mode redesign in M24
- Per-pane custom color themes
- Marketing landing redesign unrelated to investigation

---

## 9. Prerequisites (Track A — shipped in v0.4.1)

Desktop injects host bridges from the **UI bundle** (`ui/src/host/tauri-bridge.ts` + Tauri ACL `allow-desktop-commands`). M24 assumes v0.4.1+.

---

## 10. Risks / open questions

1. Cross-pane selection may need a stronger shared workspace-state contract than today’s route stores.
2. Automatic triage needs strict memory, latency, cancellation, and failure bounds.
3. Pane density can recreate Eclipse complexity — progressive disclosure + a11y required.
4. Desktop vs browser-artifact capability states must stay explicit.
5. Compare may exceed M24 capacity — gate after single-heap continuity is proven.
6. Theme token migration must not fork styles between legacy routes and the new shell during the transition.

---

## 11. Success criteria (milestone → v0.5.0)

- Opening a heap on packaged desktop enters the workbench without a “use the desktop app” dead end (prerequisite: v0.4.1).
- User can triage → inspect object → GC path → return without reloading or losing selection.
- Assistant is optional and provenance-labelled; rules path works without a provider.
- Visual chrome is token-consistent across Home → workbench panes.
- Docs (STATUS, capability matrix, roadmap) describe M24 / **v0.5.0** without claiming MAT UI pixel-equivalence.
