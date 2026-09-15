# M27 Durable Investigations and Compare Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore display-safe workspace state and make snapshot reopen and current-vs-baseline comparison part of the same investigation shell.

**Architecture:** Persist metadata only from `useInvestigationStore`; hydrate graph/facts transactionally through the existing snapshot and comparison bridges. Do not serialize graphs, absolute paths, or sensitive field values into browser storage.

**Tech Stack:** React, Zustand, Tauri, `SnapshotStore`, existing object-diff engine.

**Roadmap:** [M27 — Durable investigations + compare](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m27--durable-investigations--compare)

## Global Constraints

- M26 revision/op enforcement is a prerequisite for async hydration.
- Persist only layout, filters, stable selection IDs, notes/bookmarks, and opaque snapshot/source keys.
- Snapshot graph + facts + mode + compatible selection commit atomically.
- Reuse `ui/src/features/comparison/`, `diff_objects_for_session`, and `SnapshotStore`; do not add a diff analyzer.
- WSL cannot prove packaged GUI persistence; focused tests locally, full/native evidence in CI/native hosts.

---

## File map

- Workspace metadata: `ui/src/features/investigation/investigation-store.ts` plus a new focused persistence adapter/test.
- Snapshot hydration: `ui/src/features/snapshots/`, `ui/src/features/investigation/workspace-actions.ts`, `ui/src/host/tauri-bridge.ts`, `tauri/src/commands.rs`.
- Compare integration: `ui/src/features/comparison/ComparisonPage.tsx`, `ComparisonPicker.tsx`, `comparison-store.ts`, `tauri/session-ops/src/lib.rs`.

### M27.A — Workspace persistence

- [ ] Write the full slice plan with a versioned, display-safe persistence schema and migration tests.
- [ ] Persist pane/layout/filter/selection metadata; explicitly reject absolute paths and graph/artifact payloads.
- [ ] Restore only IDs compatible with the reopened workspace revision; drop stale object/class/leak selections honestly.
- [ ] Add notes/bookmarks only as metadata keyed by opaque workspace/snapshot identity.

### M27.B — Snapshot-first reopen

- [ ] Expand the plan around existing `openSnapshot`/`open_snapshot_for_session`; no parallel snapshot loader.
- [ ] Return one hydrate envelope containing snapshot identity, mode/capabilities, analysis facts, and revision/op context.
- [ ] Race-test failure/old-revision behavior so the prior workspace remains intact.

### M27.C — Integrated compare

- [ ] Move current/baseline selection into workbench chrome while preserving the standalone `/compare` route as an adapter.
- [ ] Expose identity strategy, top-N, cross-reference-leaks, and match quality.
- [ ] Make after-side object deltas set shared `objectId` and open Inspector.
- [ ] Record focused evidence and `NOT PROVEN` packaged behavior before marking M27 shipped.
