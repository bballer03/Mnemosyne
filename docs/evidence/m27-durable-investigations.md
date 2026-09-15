# M27 durable investigations and integrated compare — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`  
**Date:** 2026-09-15  
**Plan:** [docs/superpowers/plans/2026-09-15-m27-durable-investigations-compare.md](../superpowers/plans/2026-09-15-m27-durable-investigations-compare.md)  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records focused command, store, bridge, and UI evidence for M27.A–C on WSL. It does not infer packaged-desktop launch behavior from unit tests or a browser production build.

## Implementation commits

| Slice | Commits | Verified contract |
| --- | --- | --- |
| M27.A display-safe workspace persistence | `5ec36b1`, `399bb55`, `869d78b`, `b41018a`, `03e1cb0` | Strict metadata allow-list; opaque identity; compatible selection restore; notes/bookmarks isolation |
| M27.B transactional snapshot reopen | `1b85959`, `1bbea9d`, `88463c2`, `d6d73ef` | Cached graph-derived facts; one host hydrate envelope; atomic UI commit; prior workspace preserved on failure |
| M27.C integrated compare | `0c1e06d`, `7b62ecc`, `d03140b`, `84b7f9e`, `7f6b633` | Current/baseline snapshot pickers; strategy/top-N/leak controls; shared match quality/results; after-side Inspector navigation |

## Verified behavior

- Browser persistence accepts only versioned display metadata. It rejects paths, graphs, artifacts, hostile extra fields, and incompatible selection IDs.
- Snapshot reopen reuses `open_snapshot_for_session`, derives honest partial facts from the cached graph, and commits analysis, deep-mode capabilities, snapshot identity, and compatible selection together.
- The persistent investigation chrome now exposes a collapsible current-vs-baseline comparison panel. `/compare` remains a route adapter over the same picker/results components and comparison store.
- Live comparison lists display-safe snapshot basenames, seeds the current side only from an exact snapshot persistence identity, and sends identity strategy, bounded top-N, and `crossReferenceLeaks` through the existing comparison bridge.
- The native path remains `runDiffObjects` → `diff_objects` → `diff_objects_for_session` → the existing core object-diff engine. M27 adds no analyzer.
- `MatchQualityBadge` exposes collision rate plus false-match and false-split risk for both workbench and route-adapter results.
- Added and retained-changed rows set the shared string `objectId` and open Object Inspector. Removed rows do not offer after-side navigation because the object is absent from the current heap.
- Focused tests use minimal component/router trees; none imports or mounts the full production route tree.

## Focused local gates

Verified:

```bash
cd ui
bun test \
  src/features/investigation/workspace-persistence.test.ts \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  src/features/workflow-landing/workflow-bridge-client.test.ts \
  src/features/snapshots/SnapshotManagerPage.test.tsx \
  src/features/workflow-landing/RecentHeapsList.test.tsx \
  src/features/comparison/comparison-bridge-client.test.ts \
  src/features/comparison/ComparisonPicker.test.tsx \
  src/features/comparison/ComparisonWorkbenchPanel.test.tsx \
  src/features/comparison/ComparisonPage.test.tsx \
  src/features/comparison/ObjectDeltaTable.test.tsx \
  src/host/tauri-bridge.test.ts \
  --max-concurrency=1
bun run lint
bun run build
```

Results: **118 passed, 0 failed**; `tsc --noEmit` passed; the Vite production build completed. Vite reported the existing native-config-loader advisory and chunk-size warning; neither is a compile failure or packaged-GUI launch evidence.

```bash
cargo test -p mnemosyne-core --features test-fixtures snapshot_analysis_ -- --nocapture
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures \
  open_snapshot_for_session -- --nocapture
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures \
  snapshot_hydrate_ -- --nocapture
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures \
  diff_objects_ -- --nocapture
```

Results: **1/1** graph-derived snapshot-analysis test, **3/3** snapshot-open tests, **1/1** hydrate-contract test, and **7/7** existing session object-diff tests passed.

The core filter requires `--features test-fixtures` because repository integration tests import the feature-gated fixture module. Running the same filtered command without that feature fails while compiling unrelated integration-test targets; it does not execute the intended snapshot test.

## NOT PROVEN on this WSL host

| Claim | Status / reason |
| --- | --- |
| Packaged desktop GUI: reopen persisted workspace, compare snapshots, open Inspector | **NOT PROVEN** — WSL cannot provide native packaged-GUI launch evidence |
| Native Tauri compile/link or installer packaging | **NOT RUN** — this closeout used focused core/session/UI gates; WebKitGTK/JavaScriptCoreGTK remain host-sensitive |
| Full `bun run test` or full workspace `cargo test` | **NOT RUN** — focused M27 gates were used to avoid known WSL/Bun RSS pressure |
| Native visual/a11y interaction of the expanded comparison panel | **NOT PROVEN** — component semantics are tested, but no packaged-host interaction or screenshot was captured |
| Real multi-GB snapshot comparison performance | **NOT PROVEN** — M27 reuses the bounded object-diff engine; no new performance run was made |
