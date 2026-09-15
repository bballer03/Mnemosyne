# M26 Operation and Async Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every long desktop operation correlated phase progress, cooperative cancellation, and revision/operation identity that prevents late results from mutating a newer workspace.

**Architecture:** Add one additive operation protocol shared by React, Tauri, and headless `session-ops`. Preserve current public core entry points, introduce controlled variants that accept a no-op-by-default observer/cancellation token, and make the investigation store the sole acceptance gate for async results.

**Tech Stack:** React 19, Zustand 5, TypeScript 7, Tauri 2 events/commands, Tokio, Rust atomics, existing Mnemosyne parser/analysis APIs.

**Spec:** [UI → MAT maturity roadmap §6 M26](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m26--operation--async-platform), [PM no-stale-UI contract](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#5-pm-ux-contract-no-stale-ui)

## Global Constraints

- React cleanup flags are not cancellation. “Cancelled” is shown only after the host acknowledges cancellation or returns the structured cancelled error.
- Every async request carries `{ workspaceId, revision, operationId }`; every event and response echoes the same values.
- A response may mutate UI state only when both revision and operation ID still match the active request.
- Cancellation checkpoints must exist inside parser/graph/analysis loops; aborting only the Tokio join handle is insufficient for `spawn_blocking`.
- Cancellation frees temporary graphs/files and cannot install graph, dominator, field-data cache, artifact, query, diff, or path results after cancellation.
- Progress is monotonic within a phase. Use bytes/records/items only when measurable; otherwise emit an explicit indeterminate phase plus elapsed time.
- Operation logs include IDs, phase, elapsed time, and machine-readable error code only. Never log absolute heap paths, field values, query bodies, or AI prompts.
- Preserve existing core APIs by delegating to controlled variants with a no-op observer.
- WSL: use focused store/protocol tests and headless Rust tests. Do not mount production routes or claim packaged/native evidence; prefer CI.

---

## File map

| Unit | Path | Responsibility |
|---|---|---|
| Operation wire types | Create `ui/src/host/operation-protocol.ts`, `.test.ts` | Validate context, progress events, response envelopes, cancelled errors |
| Investigation acceptance gate | `ui/src/features/investigation/investigation-store.ts`, `.test.ts` | `workspaceId`, revision, active operation, stale-result rejection |
| UI host adapters | `ui/src/host/tauri-bridge.ts`, `.test.ts`; existing feature clients | Add operation context to long calls; unwrap matching responses |
| Workbench status | `ui/src/features/investigation/HeapSessionBar.tsx`, `.test.tsx` | Named phase, percent/indeterminate, elapsed, Cancel |
| Native operation registry | `tauri/src/state.rs` | Active operation tokens keyed by operation ID |
| Core operation control | Create `core/src/operation.rs`; modify `core/src/lib.rs`, `core/src/errors.rs` | Dependency-neutral observer, progress snapshot, cancellation token/error |
| Shared host protocol | `tauri/session-ops/src/lib.rs` | Workspace context, Tauri wire envelopes, registry helpers, race tests |
| Native commands/events | `tauri/src/commands.rs`, `tauri/src/main.rs` | Emit `mnemosyne://operation-progress`, expose `cancel_operation` |
| Parser checkpoints | `core/src/hprof/binary_parser.rs`, parser tests | Byte/record progress and cancellation |
| Analysis checkpoints | `core/src/analysis/engine.rs`, `core/src/graph/dominator.rs`, focused tests | Phase transitions and cooperative cancellation between expensive stages |
| Long operation call sites | `run_desktop_analysis`, `load_heap_internal`, `query_heap`, `diff_objects`, `inspect_object`, `find_all_gc_paths`, snapshot/flamegraph commands | Correlation, events, cancellation, commit guard |
| Test batching/docs | `ui/run-tests.ts`, `STATUS.md`, `docs/product/ui-capability-matrix.md`, `docs/roadmap.md`, `docs/evidence/m26-async-platform.md` | Bounded validation and honest closeout |

---

### Task 26.C.1: Define the operation context and stale-result acceptance gate

**Files:**
- Create: `ui/src/host/operation-protocol.ts`
- Test: `ui/src/host/operation-protocol.test.ts`
- Modify: `ui/src/features/investigation/investigation-store.ts`
- Test: `ui/src/features/investigation/investigation-store.test.ts`

**Interfaces:**
- `OperationContext = { workspaceId: string; revision: number; operationId: string }`
- `OperationKind = "open" | "analyze" | "enrich" | "query" | "diff" | "snapshot" | "flamegraph" | "gc-path" | "inspect"`
- `OperationPhase = "accepted" | "opening" | "parsing" | "building-graph" | "computing-dominators" | "analyzing" | "rendering" | "committing" | "cancelling" | "cancelled" | "complete" | "failed"`
- `OperationEnvelope<T> = OperationContext & { data: T }`
- Store actions: `beginOperation(kind): OperationContext`, `acceptOperationResult(context): boolean`, `applyOperationResult(context, apply): boolean`, `finishOperation(context, status): boolean`.

- [ ] **Step 1: Write failing pure/store tests**

  Cover a matching response, wrong workspace, old revision, superseded operation ID, completion of a stale operation, and revision bump invalidating all outstanding requests.

- [ ] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/host/operation-protocol.test.ts src/features/investigation/investigation-store.test.ts --max-concurrency=1`

  Expected: FAIL because operation context and acceptance actions do not exist.

- [ ] **Step 3: Implement the protocol and store gate**

  Generate opaque IDs with `crypto.randomUUID()` where available and a non-secret monotonic fallback for tests. Never derive identity from heap name, path, or row position.

- [ ] **Step 4: Re-run focused tests**

  Expected: PASS; generation N cannot apply to generation N+1.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/host/operation-protocol.ts ui/src/host/operation-protocol.test.ts ui/src/features/investigation/investigation-store.ts ui/src/features/investigation/investigation-store.test.ts
  git commit -m "feat(ui): enforce operation identity"
  ```

### Task 26.A.1: Add dependency-neutral core control and host progress types

**Files:**
- Create: `core/src/operation.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/src/errors.rs`
- Modify: `tauri/session-ops/src/lib.rs`
- Test: inline `session-ops` tests
- Modify: `tauri/src/commands.rs`

**Interfaces:**
- Core `OperationPhase`, `OperationProgressSnapshot`, `CancellationToken`, and trait `OperationObserver: Send + Sync { fn progress(&self, event: OperationProgressSnapshot); fn is_cancelled(&self) -> bool; }`
- Core `NoopOperationObserver` backs unchanged CLI/MCP call sites; `CoreError::OperationCancelled` is the dependency-neutral terminal error.
- Host `OperationContext { workspace_id: String, revision: u64, operation_id: String }`
- Host `OperationProgress { context, kind, phase, completed: Option<u64>, total: Option<u64>, unit: Option<String>, indeterminate: bool, elapsed_ms: u64 }`
- Tauri event name: `mnemosyne://operation-progress`.

- [ ] **Step 1: Write failing serialization and monotonicity tests**

  Assert camelCase wire names, exact context echo, known phase strings, and rejection/coalescing of progress that moves backward within one phase.

- [ ] **Step 2: Run focused tests**

  Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml operation_progress`

  Expected: FAIL until types exist.

- [ ] **Step 3: Implement additive protocol types in the correct dependency direction**

  Core owns generic control/progress and imports neither Tauri nor `session-ops`. `session-ops` owns workspace/revision wire context. Implement the Tauri event emitter adapter in `commands.rs`.

- [ ] **Step 4: Re-run focused tests**

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add core/src/operation.rs core/src/lib.rs core/src/errors.rs tauri/session-ops/src/lib.rs tauri/src/commands.rs
  git commit -m "feat(desktop): define operation progress protocol"
  ```

### Task 26.A.2: Instrument open/analyze and expensive detail phases

**Files:**
- Modify: `core/src/hprof/binary_parser.rs`
- Test: parser unit tests in the same module
- Modify: `core/src/analysis/engine.rs`
- Test: focused engine tests
- Modify: `tauri/src/commands.rs`

**Interfaces:**
- Add controlled parser variants accepting `&dyn OperationObserver`; existing `parse_hprof*` functions call them with `NoopOperationObserver`.
- Add controlled analysis variant used by Tauri; existing `analyze_heap*` signatures remain source-compatible.
- Minimum phase sequence for deep open: accepted → opening → parsing → building-graph → computing-dominators → analyzing → committing → complete.

- [ ] **Step 1: Write failing parser progress tests**

  Parse a synthetic fixture through a recording observer. Assert at least one bounded record/byte update, monotonic completion, and the same final graph as the legacy entry point.

- [ ] **Step 2: Write failing analysis phase tests**

  Assert graph/dominator/analyzer phases occur in order and indeterminate is explicit when a total is unavailable.

- [ ] **Step 3: Run focused core tests**

  Run:

  ```bash
  cargo test -p mnemosyne-core hprof::binary_parser::tests
  cargo test -p mnemosyne-core analysis::engine::tests::operation
  ```

  Expected: FAIL until controlled variants exist.

- [ ] **Step 4: Implement minimal checkpoints and Tauri emission**

  Emit at record boundaries with throttling (for example, at most every 100 ms or 1% change), not for every object. Add phases to `run_desktop_analysis`, `load_heap_internal`, `query_heap`, `diff_objects`, `inspect_object` field reparse, `find_all_gc_paths`, snapshot, and flamegraph commands; use indeterminate events where core totals are not exposed yet.

- [ ] **Step 5: Re-run focused core and headless Tauri tests**

  Expected: PASS; legacy parser/analysis results remain equal.

- [ ] **Step 6: Commit**

  ```bash
  git add core/src/hprof/binary_parser.rs core/src/analysis/engine.rs tauri/src/commands.rs tauri/session-ops/src/lib.rs
  git commit -m "feat(core): emit bounded analysis progress"
  ```

### Task 26.A.3: Subscribe in React and render honest operation status

**Files:**
- Modify: `ui/src/host/tauri-bridge.ts`
- Test: `ui/src/host/tauri-bridge.test.ts`
- Modify: `ui/src/features/investigation/HeapSessionBar.tsx`
- Test: `ui/src/features/investigation/HeapSessionBar.test.tsx`
- Modify: feature clients for open/query/diff/snapshot/flamegraph/GC paths

**Interfaces:**
- Subscribe once to `mnemosyne://operation-progress`; validate payload through `operation-protocol.ts`.
- Ignore events failing `acceptOperationResult(context)`.
- Render phase label, short operation ID, elapsed time, and either percent/unit progress or an indeterminate progressbar with `aria-busy`.

- [ ] **Step 1: Write failing bridge/status tests**

  Inject progress events directly; assert matching updates render, stale events do not, known progress exposes `aria-valuenow`, and indeterminate progress omits it.

- [ ] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/host/tauri-bridge.test.ts src/features/investigation/HeapSessionBar.test.tsx src/host/operation-protocol.test.ts --max-concurrency=1`

  Expected: FAIL until subscription/status support exists.

- [ ] **Step 3: Implement subscription and status projection**

  Do not keep feature-local “Loading…” as a second source of truth; adapt it to the operation store or remove it after equivalent states are covered.

- [ ] **Step 4: Re-run tests and type-check**

  Expected: PASS and `bun run lint` exits 0.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/host ui/src/features/investigation ui/src/features/artifact-loader ui/src/features/heap-explorer ui/src/features/comparison ui/src/features/snapshots ui/src/features/flamegraph ui/src/features/leak-workspace
  git commit -m "feat(ui): surface correlated operation progress"
  ```

### Task 26.B.1: Add an operation registry and cancel command

**Files:**
- Modify: `tauri/src/state.rs`
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Test: focused Rust tests

**Interfaces:**
- Registry value: `Arc<AtomicBool>` keyed by operation ID, associated with workspace/revision.
- Commands register before work and remove on every terminal path.
- `cancel_operation(operation_id)` sets the token and returns `{ operation_id, accepted: bool }`; unknown/already-finished IDs return `accepted: false`.
- Structured terminal code: `operation_cancelled`.

- [ ] **Step 1: Write failing registry tests**

  Cover register, duplicate ID rejection, cancellation, unknown ID, cleanup after success/error/cancel, and isolation between two operation IDs.

- [ ] **Step 2: Run focused Rust tests**

  Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml operation_registry`

  Expected: FAIL until registry helpers exist.

- [ ] **Step 3: Implement registry and command**

  Use atomics only for the token; protect registry membership with the existing session synchronization discipline. Never expose heap paths in registry diagnostics.

- [ ] **Step 4: Register `cancel_operation` and re-run tests**

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add tauri/src/state.rs tauri/src/commands.rs tauri/src/main.rs tauri/session-ops/src/lib.rs
  git commit -m "feat(desktop): register cancellable operations"
  ```

### Task 26.B.2: Add cooperative core cancellation checkpoints

**Files:**
- Modify: `core/src/hprof/binary_parser.rs`
- Modify: `core/src/graph/dominator.rs`
- Modify: `core/src/analysis/engine.rs`
- Test: focused module tests

**Interfaces:**
- Controlled functions check `observer.is_cancelled()` at top-level record boundaries, dominator traversal boundaries, and between analyzer stages.
- Return `CoreError` mapped to machine code `operation_cancelled`; legacy no-op calls cannot produce it.

- [ ] **Step 1: Write deterministic cancellation tests**

  Use an observer that cancels after a known progress callback/check count. Assert parsing, dominator construction, and analysis stop before completion and no success payload is returned.

- [ ] **Step 2: Run focused tests**

  Run:

  ```bash
  cargo test -p mnemosyne-core cancellation
  cargo test -p mnemosyne-core operation_cancelled
  ```

  Expected: FAIL before checkpoints are implemented.

- [ ] **Step 3: Implement checkpoints without per-object callback overhead**

  Check in bounded batches. Preserve the current algorithms and outputs when the token is unset.

- [ ] **Step 4: Add a Tauri commit guard**

  Immediately before installing graph/dominator/cache or serializing success, re-check cancellation and the operation context. A cancelled operation must drop local results.

- [ ] **Step 5: Re-run focused core/session tests**

  Expected: PASS; cancellation produces no installed session state.

- [ ] **Step 6: Commit**

  ```bash
  git add core/src/hprof/binary_parser.rs core/src/graph/dominator.rs core/src/analysis/engine.rs tauri/src/commands.rs tauri/session-ops/src/lib.rs
  git commit -m "feat(core): stop work cooperatively"
  ```

### Task 26.B.3: Wire the Cancel control end to end

**Files:**
- Modify: `ui/src/host/operation-protocol.ts`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/features/investigation/HeapSessionBar.tsx`
- Test: matching UI tests

**Interfaces:**
- Bridge: `cancelOperation(operationId: string): Promise<{ operationId: string; accepted: boolean }>`
- Cancel remains visible while active and supported; first click transitions to `cancelling` and disables conflicting actions.
- Late progress/success after host cancellation is ignored by the store even if emitted accidentally.

- [ ] **Step 1: Write failing UI cancellation tests**

  Assert the correct ID is sent, UI stays `cancelling` until acknowledgement/terminal event, rejected cancellation restores in-flight status with an explanation, and late success cannot apply.

- [ ] **Step 2: Run focused tests**

  Run: `cd ui && bun test src/features/investigation/HeapSessionBar.test.tsx src/host/tauri-bridge.test.ts src/features/investigation/investigation-store.test.ts --max-concurrency=1`

  Expected: FAIL until bridge/control exist.

- [ ] **Step 3: Implement Cancel and recovery states**

  Browser/artifact adapters without cancellation support hide the button and retain honest indeterminate progress.

- [ ] **Step 4: Re-run tests and type-check**

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/host ui/src/features/investigation
  git commit -m "feat(ui): cancel active heap operations"
  ```

### Task 26.C.2: Enforce envelopes on every mutating response

**Files:**
- Modify: `tauri/src/commands.rs`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/features/investigation/workspace-actions.ts`
- Modify: query/diff/snapshot/flamegraph/GC-path/inspector clients and tests

- [ ] **Step 1: Add race tests per operation family**

  For open/analyze, query, diff, snapshot, flamegraph, GC paths, and field-data inspect: start generation N, bump revision/start N+1, resolve N last, and assert N cannot mutate store/UI.

- [ ] **Step 2: Run focused race suites**

  Run only the named files with `bun test ... --max-concurrency=1`; do not import `app/router.tsx` or the production `routes` array.

- [ ] **Step 3: Wrap native success responses**

  Return `OperationEnvelope<T>` for operation-aware bridge methods. Keep any externally documented CLI/MCP JSON contracts unchanged; this is the Tauri host boundary only.

- [ ] **Step 4: Replace ad-hoc request keys/cleanup booleans as acceptance authority**

  Local cleanup may prevent unnecessary component state work, but only the investigation store context decides whether results are current.

- [ ] **Step 5: Re-run all focused race tests**

  Expected: PASS for generation N/N+1 and cancelled-late-result cases.

- [ ] **Step 6: Commit**

  ```bash
  git add tauri/src/commands.rs ui/src/host ui/src/features
  git commit -m "fix(async): reject stale operation results"
  ```

### Task 26 closeout: Focused verification and evidence

**Files:**
- Modify: `ui/run-tests.ts`
- Modify: `STATUS.md`
- Modify: `docs/product/ui-capability-matrix.md`
- Modify: `docs/roadmap.md`
- Create: `docs/evidence/m26-async-platform.md`

- [ ] Run `bun run lint` and focused operation/store/race suites in fresh bounded processes.
- [ ] Run `cargo test -p mnemosyne-core cancellation` and `cargo test --manifest-path tauri/session-ops/Cargo.toml`.
- [ ] Run `cargo check --manifest-path tauri/Cargo.toml`; leave full workspace/native package validation to CI.
- [ ] Record event examples with sanitized IDs, phase order, cancellation cleanup, race evidence, and explicit `NOT PROVEN` native GUI rows.
- [ ] Update status/capability docs only for verified operations; list any command still indeterminate or non-cancellable.
- [ ] Commit with `docs(m26): record async platform evidence`.

## M26 completion gate

- Every long Tauri operation emits correlated phase events or an explicit indeterminate phase.
- Cancel reaches a cooperative core checkpoint, cleans registry/temp state, and cannot publish late success.
- Every mutating Tauri response echoes workspace/revision/operation identity.
- Race tests prove generation N cannot mutate generation N+1.
- CLI/MCP contracts remain compatible; no sensitive path/query/field/prompt data enters progress events or logs.
