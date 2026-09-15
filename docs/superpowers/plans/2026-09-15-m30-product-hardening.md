# M30 Product Hardening Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close visual, accessibility, test-memory, native-evidence, logging, and documentation gaps after the MAT investigation loop is functionally complete.

**Architecture:** Consolidate existing tokens/primitives and evidence pipelines; do not redesign analysis contracts. Native release claims come only from matching native hosts, while WSL remains a unit/command environment.

**Tech Stack:** React/CSS, Testing Library, Bun test batches, Tauri release CI, Rust tracing, documentation/evidence.

**Roadmap:** [M30 — Product hardening](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m30--product-hardening)

## Global Constraints

- No native-launch, MAT-golden, signing, or notarization claim without direct evidence.
- Keep full production routes out of unit tests and enforce per-batch memory ceilings.
- Never log paths, field values, query bodies, prompts, tokens, or PII; rotate `desktop.log`.
- Accessibility acceptance requires automated checks plus manual keyboard evidence.
- Documentation must describe current runtime truth and explicit `NOT PROVEN` gaps.

---

## File map

- Theme/responsive: `ui/src/app/globals.css`, shared workbench/panel components.
- Keyboard/a11y: core-loop panels in `ui/src/features/artifact-explorer/`, `heap-explorer/`, `leak-workspace/`.
- Test memory: `ui/run-tests.ts`, `.github/workflows/ci.yml`.
- Native evidence: `.github/workflows/release.yml`, `docs/evidence/`.
- Logging: `tauri/src/logging.rs`, operation logging in `tauri/src/commands.rs`.
- Drift cleanup: `STATUS.md`, `ARCHITECTURE.md`, `docs/product/ui-capability-matrix.md`, `docs/roadmap.md`.

### M30.A — Theme and responsive system

**Inventory (2026-09-15):** `globals.css` had 9 tokens; shared workbench shells duplicated `#1e293b` panel borders, `#94a3b8` muted text, `#38bdf8` accent, and `980px` compact breakpoint across `ArtifactLoaderPage`, `DashboardPage`, `HeapExplorerLayout`, `LeakWorkspaceLayout`, `ArtifactExplorerPage`, and `ComparisonWorkbenchPanel`. Leaf panels (histogram, inspector, workflow cards) still use literals — deferred to M30.A follow-up.

**Files:**
- Modify: `ui/src/app/globals.css` — expand `--mn-*` color/spacing/radius tokens + documented `@media (max-width: 980px)` stack rule
- Create: `ui/src/app/theme-tokens.ts` — shared inline-style objects referencing CSS vars
- Create: `ui/src/app/use-compact-layout.ts` — DRY hook for the 980px breakpoint
- Migrate: workbench shell files listed above to import from `theme-tokens.ts`
- Test: `ui/src/app/theme-tokens.test.ts`, narrow-width cases in `GuidedLanding.test.tsx`, `ArtifactLoaderPage.test.tsx`, `HeapExplorerLayout.test.tsx`, `ArtifactExplorerPage.test.tsx`

- [ ] **Step 1: Write failing token contract test**

```typescript
// ui/src/app/theme-tokens.test.ts
it("exports workbench panel styles that reference CSS custom properties", () => {
  expect(workbenchPanelStyle.border).toContain("var(--mn-border-subtle)");
  expect(COMPACT_LAYOUT_MAX_WIDTH).toBe(980);
});
```

- [ ] **Step 2: Run test — expect FAIL** (`bun test ui/src/app/theme-tokens.test.ts`)

- [ ] **Step 3: Expand globals.css tokens and add theme-tokens.ts + use-compact-layout.ts**

- [ ] **Step 4: Run token test — expect PASS**

- [ ] **Step 5: Write failing narrow-width primary-action tests**

```typescript
// GuidedLanding.test.tsx — workflow cards remain queryable at 720px
// ArtifactLoaderPage.test.tsx — Open heap dump button visible at 720px
// ArtifactExplorerPage.test.tsx — histogram grid stacks to one column at 720px
```

- [ ] **Step 6: Migrate shared workbench shells to theme tokens + compactGridColumns**

- [ ] **Step 7: Run focused tests + `bun run build` in ui/**

Run: `cd ui && bun test src/app/theme-tokens.test.ts src/features/workflow-landing/GuidedLanding.test.tsx src/features/artifact-loader/ArtifactLoaderPage.test.tsx src/features/heap-explorer/HeapExplorerLayout.test.tsx src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`

- [ ] **Step 8: Commit** `feat(ui): expand theme tokens for hardening`

**Leftover gaps (explicit):**
- Leaf panel components still carry literal hex colors (ClassInstancesPanel, QueryConsolePanel, WorkflowCard, etc.)
- App.tsx / TopNav.tsx shell chrome not yet tokenized
- No visual regression screenshots; narrow-width acceptance is test + documented CSS rule only

### M30.B — Keyboard and accessibility

- [ ] Plan keyboard-complete Open → histogram → instance → Inspector → GC-path flow.
- [ ] Add automated accessible-name/state/focus tests and native manual evidence checklist.

### M30.C — Native release evidence

- [ ] Run packaged Windows/macOS/Linux open → inspect → path scenarios only on matching hosts.
- [ ] Record hashes, versions, signing state, failures, screenshots, and `NOT PROVEN` rows.
- [ ] Fold remaining M20–M23 native/Terra/Sol evidence only where actually demonstrated.

### M30.D — UI test memory safety

#### M30.D file map

- Modify `ui/run-tests-config.ts` — named batches, per-batch RSS ceilings (MiB), and preserved fresh-process splits.
- Create `ui/run-tests-rss.ts` — `/proc` peak-RSS parsing, ceiling comparison, and failure formatting.
- Create `ui/run-tests-runner.ts` — async batch runner with timeout/SIGTERM handling and RSS polling.
- Modify `ui/run-tests.ts` — thin entry that runs batches through the runner and exits non-zero on gate failure.
- Create `ui/src/test/reset-test-state.ts` — `resetAllTestState()` for Zustand stores and `__MNEMOSYNE_*` bridges.
- Modify `ui/src/test/setup.ts` — call `resetAllTestState()` from global `afterEach`.
- Create `ui/run-tests.test.ts` and `ui/src/test/reset-test-state.test.ts` — harness and helper unit tests only.
- Modify `.github/workflows/ci.yml` — keep Bun pin aligned; UI job already runs `bun run test` (RSS gate is in-runner).

#### Documented RSS ceilings (Bun 1.2.5 / jsdom 24.1.1 baseline, ~50% headroom)

| Batch id | Ceiling (MiB) | Notes |
|---|---:|---|
| `leak-workspace-core` | 512 | Observed ~220 MiB |
| `heap-explorer-core` | 512 | Observed ~192 MiB |
| `comparison-panels` | 512 | Observed ~204 MiB |
| `bridge-light` | 512 | Observed ~171 MiB |
| `workflow-card` | 384 | Isolated; observed ~133 MiB |
| `workflow-cards` | 384 | Observed ~121 MiB |
| `investigation-assistant` | 768 | Highest; observed ~438 MiB |
| `workflow-landing` | 512 | Observed ~155 MiB |
| `m20-surfaces` | 512 | Observed ~164 MiB |
| `artifact-explorer-instances` | 512 | Observed ~211 MiB |
| `dominator-explorer` | 512 | Observed ~210 MiB |
| `object-inspector` | 512 | Observed ~193 MiB |
| `operation-status` | 384 | Observed ~140 MiB |
| `findings-advisory` | 512 | Observed ~156 MiB |

#### Task M30.D.1: RSS gate utilities (TDD)

- [x] **Step 1: Write failing harness tests** in `ui/run-tests.test.ts` for `/proc` parsing, ceiling comparison, and batch-named failures (timeout, SIGTERM, RSS).
- [x] **Step 2: Run focused tests and verify RED**

```bash
cd ui
bun test run-tests.test.ts --max-concurrency=1
```

- [x] **Step 3: Implement** `ui/run-tests-rss.ts` and minimal exports until tests pass.
- [x] **Step 4: Run focused tests and verify GREEN**

#### Task M30.D.2: Batch runner with RSS polling (TDD)

- [x] **Step 1: Extend harness tests** to assert `classifyBatchResult` prefers timeout/signal over exit code and fails RSS after exit 0.
- [x] **Step 2: Implement** `ui/run-tests-runner.ts` with async spawn, 120s timeout, SIGTERM kill, and `/proc/<pid>/status` polling.
- [x] **Step 3: Split batch list** into `ui/run-tests-config.ts`; keep existing fresh-process isolation (WorkflowCard, Assistant, dominator/inspector, M29 pane, etc.).
- [x] **Step 4: Wire** `ui/run-tests.ts` to the runner; log `[ui-test-rss] batch=… peak=… ceiling=…` per batch on stderr.
- [x] **Step 5: Run full UI gate**

```bash
cd ui
bun run test
```

Expected: all batches pass; stderr shows peak RSS under each ceiling.

#### Task M30.D.3: Store/bridge reset helpers (TDD)

- [x] **Step 1: Write failing tests** in `ui/src/test/reset-test-state.test.ts` for store and bridge cleanup.
- [x] **Step 2: Implement** `resetAllTestState()` and hook it from `ui/src/test/setup.ts` global `afterEach`.
- [x] **Step 3: Run helper tests**

```bash
cd ui
bun test src/test/reset-test-state.test.ts --max-concurrency=1
```

- [x] **Step 4: Re-run full UI gate** to confirm no regressions from global reset.

#### Task M30.D.4: CI alignment

- [x] Confirm `.github/workflows/ci.yml` `ui-check` job runs `bun run test` (no separate RSS step required).
- [x] Keep Bun `1.2.5` pin comment referencing batch isolation + RSS gate.
- [x] Do **not** mount production route trees in unit tests; heavy suites stay in dedicated batches.

#### Acceptance

- [x] Each `ui/run-tests.ts` batch logs peak RSS and fails above its documented ceiling.
- [x] Timeout or SIGTERM names the affected batch id and file list (no silent job cancel).
- [x] Global test setup resets stores and host bridges between tests within a batch.
- [x] Fresh-process isolation for WorkflowCard, InvestigationAssistant, dominator/inspector, and findings pane is unchanged.

### M30.E — Documentation and logging closeout

- [ ] Add size/daily rotation for `desktop.log` with redaction tests.
- [ ] Reconcile STATUS, ARCHITECTURE, roadmap, capability matrix, bridge list, and OQL deferrals.
- [ ] Run placeholder/contradiction scan and record remaining gated work under M31+.
