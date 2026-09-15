# M26 operation and async platform — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`
**Date:** 2026-09-15
**Plan:** [docs/superpowers/plans/2026-09-15-m26-async-platform.md](../superpowers/plans/2026-09-15-m26-async-platform.md)
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records the M26 behavior verified by focused tests on WSL. It does not infer packaged-GUI, native-host, full-suite, or performance evidence from unit tests.

## Implementation commits

| Slice | Commit | Verified contract |
| --- | --- | --- |
| Operation identity/store gate | `02060c2` | Workspace, revision, and operation ID acceptance |
| Shared core/host protocol | `fc9b28c` | Camel-case envelopes, phases, monotonic progress |
| Parser/analysis progress | `5737ef7` | Controlled variants and bounded checkpoints |
| React status projection | `ad3b81d` | Correlated determinate/indeterminate status |
| Native operation registry | `9603739` | Registration, cancellation, cleanup, isolation |
| Cooperative checkpoints | `d46828d` | Parser, dominator, analysis-stage cancellation |
| Cancel control | `af00934` | Cancelling/acknowledgement/rejection UI states |
| Mutating response envelopes/races | `c840171` | Late generation N cannot publish into N+1 |

## Sanitized wire examples

Progress events use `mnemosyne://operation-progress` and contain correlation and progress metadata only:

```json
{
  "context": {
    "workspaceId": "workspace-demo",
    "revision": 7,
    "operationId": "operation-demo"
  },
  "kind": "analyze",
  "phase": "building-graph",
  "completed": 25,
  "total": 100,
  "unit": "objects",
  "indeterminate": false,
  "elapsedMs": 1250
}
```

Operation-aware success responses flatten the same identity beside `data`:

```json
{
  "workspaceId": "workspace-demo",
  "revision": 7,
  "operationId": "operation-demo",
  "data": {
    "result": "sanitized-example"
  }
}
```

No heap path, query body, field value, or AI prompt belongs to either wire shape. An explicit log-capture audit is listed as **NOT PROVEN** below.

## Verified behavior

- Deep open/analyze phase order is `accepted → opening → parsing → building-graph → computing-dominators → analyzing → committing → complete`; failures and acknowledged cancellation terminate as `failed` or `cancelled`.
- Query, diff, GC-path, and non-reparse inspect work expose an explicit indeterminate `analyzing` phase rather than fabricated percentages.
- Snapshot save/open and flamegraph work expose correlated parse/open, graph/dominator, rendering/commit phases where available; unknown totals stay indeterminate.
- Parser byte/record checkpoints, dominator traversal checkpoints, and analysis stage boundaries observe cancellation. Registry tests prove cleanup after success/error/cancel, operation-ID isolation, and commit rejection after cancellation.
- The store rejects wrong-workspace, old-revision, superseded-operation, and post-cancellation progress/success. Bridge race tests cover open, analyze, query, diff, snapshot, flamegraph, GC paths, and field-data inspect.
- The UI renders determinate progress with `aria-valuenow`, indeterminate progress without a percentage, and remains `cancelling` until host acknowledgement or a structured `operation_cancelled` terminal error.
- `ui/run-tests.ts` runs the M26 status mount in a fresh bounded process with the pure operation-protocol suite.

## Focused local gates

Verified:

```bash
cd ui
bun run lint
bun test src/host/operation-protocol.test.ts src/features/investigation/investigation-store.test.ts --max-concurrency=1
bun test src/host/tauri-bridge.test.ts --max-concurrency=1
bun test src/features/investigation/HeapSessionBar.test.tsx --max-concurrency=1
bun test src/features/investigation/workspace-actions.test.ts --max-concurrency=1
cd ..
cargo test -p mnemosyne-core --lib cancellation
cargo test -p mnemosyne-core --lib operation_cancelled
cargo test --manifest-path tauri/session-ops/Cargo.toml
```

Results: TypeScript check passed; 49 focused UI tests passed; 3 focused core cancellation tests passed; 11 session-operation tests passed.

The plan's literal `cargo test -p mnemosyne-core cancellation` command is not a supported focused invocation in this worktree: Cargo also builds integration tests that import the feature-gated `test_fixtures` module. It failed during compilation before running the filter. Adding `--lib` isolated and passed the intended parser/dominator cancellation tests; the separately filtered analysis-stage cancellation test also passed.

Attempted but environment-blocked:

```bash
cargo check --manifest-path tauri/Cargo.toml
```

The check reached native Tauri dependencies, then stopped because WSL lacks `javascriptcoregtk-4.1` and `webkit2gtk-4.1` pkg-config packages.

## Cancellation depth and remaining indeterminate work

| Operation family | Correlated envelope/events | Cancellation evidence | Remaining limitation |
| --- | --- | --- | --- |
| Open / analyze | Verified | Cooperative parser, dominator, and analyzer checkpoints plus commit guards | Packaged-host timing not measured |
| Field-data inspect | Verified | Controlled reparse/inspection and field-cache commit guard | Non-reparse work may finish before acknowledgement |
| Snapshot save | Verified | Cooperative parse/dominator work; stale artifact removal guard | Snapshot-store write has no inner cancellation checkpoint |
| Snapshot open | Verified | Boundary checks and session commit guard | Snapshot read/deserialization has no inner checkpoint |
| Flamegraph | Verified | Cooperative analysis plus render/session commit guards | Collapse/render is indeterminate and not internally interruptible |
| Query | Verified | Pre/post checks and late-result rejection | Query execution loop has no observer checkpoint |
| Diff | Verified | Registry/commit guard and late-result rejection | Diff engine has no observer checkpoint |
| GC paths | Verified | Pre/post checks and late-result rejection | Traversal loop has no observer checkpoint |

Cancellation is therefore proven to prevent publication for all protocol-wired families, but prompt CPU interruption is proven only in the controlled parser/dominator/analysis paths. `run_ci_check`, leak explain/fix, source mapping, and workflow/AI lifecycle commands are not operation-protocol-wired by M26 and must not be described as cancellable.

## NOT PROVEN on this WSL host

| Claim | Status / reason |
| --- | --- |
| Packaged Tauri GUI progress and Cancel flow | **NOT PROVEN** — packaged GUI was not launched |
| Native Tauri compile/link | **NOT PROVEN** — missing JavaScriptCoreGTK/WebKitGTK development packages |
| Full `bun run test`, full workspace Cargo tests, or native packaging | **NOT RUN** — closeout intentionally used focused bounded gates |
| Prompt cancellation latency inside query/diff/GC traversal/flame rendering/snapshot I/O | **NOT PROVEN** — publication guards pass; those loops lack inner observer checkpoints |
| End-to-end sensitive-log audit | **NOT PROVEN** — wire shapes are sanitized, but no captured-log assertion was run |
| Full CLI/MCP regression compatibility | **NOT PROVEN by this closeout** — public core APIs remain additive/no-op compatible, but full CLI/MCP suites were not run |
