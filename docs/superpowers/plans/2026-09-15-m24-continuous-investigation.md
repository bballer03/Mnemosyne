# M24 Continuous Heap Investigation — Implementation Plan (v0.5.0)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a persistent investigation workbench where users Open / **Open another** / **Close** heaps transactionally, see one authoritative identity, and share selection across core panes — product release **v0.5.0**.

**Architecture:** Revisioned `investigation` Zustand aggregate + `InvestigationHost` facade over Tauri/bridges; atomic commit of artifact+graph+capabilities; routes become projections of workbench state.

**Tech Stack:** React UI, Zustand, Tauri commands, session-ops.

**Specs:** [M24 design](../specs/2026-09-14-m24-continuous-heap-investigation-design.md), [maturity roadmap](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md)

## Global Constraints

- Absolute paths never enter React; opaque `sourceId` + basename only.
- Lean first-open remains default.
- Do not claim MAT golden / native launch without evidence.
- WSL: no full production-route test mounts; use minimal fixtures; prefer CI.
- Must expose Open another + Close after first open (P0).

---

## File map

| Unit | Path |
|---|---|
| Types + store | `ui/src/features/investigation/*` |
| Host facade | `ui/src/host/investigation-host.ts` (+ adapters) |
| Shell chrome | `ui/src/features/investigation/WorkbenchShell.tsx` (or app layout) |
| Bridge unload | `ui/src/host/tauri-bridge.ts`, `desktop-heap-client.ts`, `tauri/src/commands.rs` (already has unload) |
| Open flow | `ArtifactLoaderPage.tsx` → migrate to investigation actions |
| Tests | `ui/src/features/investigation/*.test.tsx`, ArtifactLoader tests |

---

### Task 24.A.1 — Investigation types + store skeleton

- [ ] Add `investigation-types.ts`, `investigation-store.ts` with workspaceId, revision, heap identity, operation, selection stubs
- [ ] Unit tests for revision bump + ignore stale revision
- [ ] Commit

### Task 24.A.2 — Workbench chrome: Open another / Close / identity

- [x] Persistent header (`HeapSessionBar`) on investigation routes
- [x] Tests for Open another / Close

### Task 24.B.1 — Bridge `unloadHeap` + atomic Close

- [x] `unloadHeap` on desktop bridge; Close clears heap-bound stores

### Task 24.B.2 — Transactional open / open-another

- [x] Remember source only after successful lean open

### Task 24.B.3 — Actionable recent + snapshot open hydrate (minimum)

- [x] Recent Loads / Recent Heaps Open (`6839e83`)
- [x] Snapshot open clears artifact (no split-brain) via `applyOpenedSnapshotSession`

### Task 24.C — Shared selection across core panes

- [ ] Stable object/class/leak ids in investigation store
- [ ] Wire Histogram / Dominators / Inspector / GC Paths to shared selection
- [ ] Tests
- [ ] Commit

### Task 24.D / 24.E

- [ ] Findings + Assistant as pane (can follow 24.C)
- [ ] Gate compare if needed; cut v0.5.0 when 24.A–C (+ D if ready) meet success criteria

---

## v0.5.0 cut criteria

- Packaged desktop: open → investigate → open another → close without split-brain
- Docs: STATUS, capability matrix, release notes
- Version bump to 0.5.0 across workspace when cutting tag
