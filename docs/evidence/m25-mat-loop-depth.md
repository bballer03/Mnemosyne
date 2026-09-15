# M25 MAT investigation-loop depth — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`  
**Date:** 2026-09-15  
**Plan:** [docs/superpowers/plans/2026-09-15-m25-mat-loop-depth.md](../superpowers/plans/2026-09-15-m25-mat-loop-depth.md)  
**Roadmap:** [docs/superpowers/specs/2026-09-15-ui-mat-maturity-roadmap-design.md](../superpowers/specs/2026-09-15-ui-mat-maturity-roadmap-design.md) §M25  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records what M25 *shipped* and what remains **NOT proven** on this WSL host. Claims below come from git history and focused automated tests — not from packaged GUI launch.

## What shipped (commit refs)

| Slice | Commit | Summary |
| --- | --- | --- |
| 25.A.1 Histogram view in investigation store | `50433dd` | Persist search/group/sort/page across route changes; reset on revision bump |
| 25.A.2 Bounded class instances host API | `66ed06a` | `list_class_instances` / bridge; default 100 / max 200; truncated pages |
| 25.A.3 Bounded sortable histogram DOM | `2d3a44f` (+ `e9608e0` test typing) | ≤100 rows/page; Showing X–Y of Z; sort before slice |
| 25.A.4 Class → instances → Inspector | `9d00e2e` | ClassInstancesPanel; `setObjectId(..., "histogram")` + encoded inspector URL |
| 25.B.1 Retain DominatorTree + children API | `8001f7c` | Session-paired graph/dominator; `getDominatorChildren`; clear on unload |
| 25.B.2 Lazy dominator tree UI | `104d62b` | Expand-on-demand; retained-% filter; objectId selection; artifact flat fallback |
| 25.C.1 Opt-in object fields | `9e587ce` | Inspect by shared objectId; field bytes only after cost disclosure |
| 25.C.2 Synchronized path/ref navigation | `76b5e5f` | Refs/referrers/dominators/GC-path nodes update store + Inspector URL |

## Bounded / honesty contracts (verified in tests)

- Histogram: ≤100 flat row buttons per page; search applies before pagination; `classKey` survives paging.
- Class instances: bounded page + truncation/total label; non-class groupings do not invent instance lists.
- Dominator tree: roots load once per filter; children only after expand; collapse removes descendant DOM; child pages default 50 / max 100.
- Fields: lean `inspectObject(..., false)` by default; CTA discloses reparse/memory cost before `retainFieldData: true`; unavailable vs empty fields are distinct.
- Navigation: real object links call `setObjectId` then navigate; synthetic GC roots remain non-clickable; relation sections disclose first-N-of-M caps.

## Local gates run for closeout

```bash
cd ui
bun run lint
bun test src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx src/features/artifact-explorer/components/ClassInstancesPanel.test.tsx --max-concurrency=1
bun test src/features/heap-explorer/components/DominatorExplorerPanel.test.tsx src/features/heap-explorer/components/ObjectInspectorPanel.test.tsx --max-concurrency=1
bun test src/features/leak-workspace/LeakGcPathPage.test.tsx --max-concurrency=1
bun run build
cd ..
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures
```

`cargo check --manifest-path tauri/Cargo.toml` may be blocked on WSL without WebKitGTK — treat full Tauri link as **CI-owned**.

`ui/run-tests.ts` keeps ClassInstances / Dominator / ObjectInspector suites in light or fresh batches to avoid Bun/jsdom RSS ceilings.

## NOT proven on this WSL host

| Claim | Status | Where it belongs |
| --- | --- | --- |
| Packaged Tauri GUI: open heap → histogram → instances → inspector → expand dominators → opt-in fields | **NOT run** | M30.C / native hosts |
| Full `bun run test` / full workspace `cargo test` as a single closeout gate | **Not claimed here** | CI on PR #95 |
| MAT golden / Eclipse report-text equivalence | **NOT proven** | M31.B |
| Overview-mode desktop analysis wiring | **Still open** | Separate track |
| In-workbench compare | **Deferred** | M24.E / M27.C |

## How to read this evidence

1. Product capability status → [ui-capability-matrix.md](../product/ui-capability-matrix.md).
2. Continuous shell (Open another / Close / selection) → M24 / v0.5.0 release notes.
3. Packaged GUI and native launch → M21/M30 native-host evidence — never infer from WSL unit green.
