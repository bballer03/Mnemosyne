# M31.C workbench perspectives evidence

**Date:** 2026-09-16
**Scope:** Wave 1 named layout presets
**Verdict:** **FOCUSED UI CONTRACT MET; PACKAGED GUI NOT PROVEN**

## What landed

The persistent investigation chrome now exposes four fixed MAT-like workbench presets:

1. **Leak Hunt** — suspects, histogram, and GC-path follow-through
2. **Dominator Browse** — retained-size tree, inspector, and references
3. **Compare** — snapshot pair, match quality, and object deltas
4. **OQL Lab** — query editor, history, and results

Applying a preset switches to its fixed route-backed layout. This is deliberately not free-form docking.

The selected `PerspectiveId` is stored in the existing versioned, display-safe workspace metadata under `layout.perspectiveId`. Persistence remains session-scoped because the existing workspace metadata uses `sessionStorage`; it is isolated by opaque workspace/snapshot identity and contains no heap path or graph payload.

## Verification

```text
bun test \
  src/features/investigation/PerspectiveSwitcher.test.tsx \
  src/features/investigation/workspace-persistence.test.ts \
  src/features/investigation/investigation-store.test.ts \
  --max-concurrency=1

37 pass
0 fail
```

Dedicated perspective coverage: **2 tests** for apply/switch behavior and workspace-metadata restoration. The remaining **35** tests protect the surrounding investigation store and persistence contract.

## Honesty boundary

The tests prove React routing, preset selection, and session metadata behavior under jsdom. They do not prove a packaged Tauri interaction, native window layout, or cross-session disk persistence. Packaged GUI behavior remains **NOT PROVEN** on WSL.
