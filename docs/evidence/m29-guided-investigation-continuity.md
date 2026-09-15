# M29 guided investigation continuity — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`  
**Date:** 2026-09-15  
**Plan:** [docs/superpowers/plans/2026-09-15-m29-guided-investigation-continuity.md](../superpowers/plans/2026-09-15-m29-guided-investigation-continuity.md)  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records focused store, adapter, workflow-binding, and Assistant evidence for M29.A–C on WSL. It does not infer live AI-provider behavior, packaged desktop launch, or native host interaction from unit tests or a browser production build.

## Implementation commits

| Slice | Plan | Implementation | Verified contract |
| --- | --- | --- | --- |
| M29.A findings queue | `203a5bc` | `ea6a98c` | Immutable measured findingFacts, mutable status map, source-scoped replace, stale revision rejection, policy publication |
| M29.B workflow binding | `dd8bc64` | `9aec455` | One active workspace workflow, ID-free UI adapters, recovery/close, display-safe step chrome, Assistant correlation |
| M29.C contextual Assistant | `7505abd` | `45b6b32` | Stable-selection measured projection (cap 5), selection-aware rules guidance, collapsible advisory, immutable findings |
| Follow-up | — | `a0f87c2` | TriageSummaryCard asserts ID-free workflow chrome (no raw step_result leak dumps) |

## Verified behavior

- Findings queue keeps measured facts immutable and separate from user status/notes; adapters publish artifact and policy results against stable targets only.
- Workflow start/resume/advance/close go through workspace binding; UI never pastes workflow IDs; recovery and close are display-safe; cards show current step + Continue/deep-links without dumping raw step_result payloads.
- Assistant projects matched findings from stable `leakId` / `objectId` / `classKey` only (max five), never invents a score-based default focus, and keeps measured facts outside the collapsible AI guidance region.
- History remains 12 turns by default and hard-capped at 32; every turn carries `rules` / `provider` / `fallback` provenance; stale Assistant responses after revision bumps are rejected.
- Focused UI tests mount `MemoryRouter` + page/component adapters only; they do not mount the production route tree.

## Focused local gates

Verified:

```bash
cd ui
bun test \
  src/features/investigation/investigation-store.test.ts \
  src/features/investigation/finding-adapters.test.ts \
  src/features/investigation/FindingsAdvisoryPane.test.tsx \
  src/features/policy/PolicyCheckPage.test.tsx \
  src/features/investigation/workspace-persistence.test.ts \
  src/features/investigation/workspace-actions.test.ts \
  src/features/investigation/HeapSessionBar.test.tsx \
  src/features/workflow-landing/workflow-bridge-client.test.ts \
  src/features/workflow-landing/workflow-binding.test.ts \
  src/features/workflow-landing/WorkflowCard.test.tsx \
  src/features/workflow-landing/NaturalLanguageInputBar.test.tsx \
  src/features/workflow-landing/TriageSummaryCard.test.tsx \
  src/features/assistant/assistant-context.test.ts \
  src/features/assistant/assistant-bridge-client.test.ts \
  src/features/assistant/InvestigationAssistantPage.test.tsx \
  --max-concurrency=1
bun run build
```

Results: **140 passed, 0 failed**; `tsc -b` + Vite production build exited 0. Vite emitted its existing native-config-loader advisory and chunk-size warning; neither is a compile failure or packaged-GUI evidence.

## NOT PROVEN on this WSL host

| Claim | Status / reason |
| --- | --- |
| Live AI provider chat end-to-end | **NOT PROVEN** — host bridge mocked; no live credentials or remote model calls |
| Packaged Windows/macOS/Linux GUI workflow/Assistant interaction | **NOT PROVEN** — no packaged desktop was launched on this WSL host |
| Native Tauri compile/link or installer packaging | **NOT PROVEN** — not re-run for M29 closeout; prior WSL GTK blockers remain |
| Full `bun run test` or full workspace `cargo test` | **NOT RUN** — focused M29 gates were used to avoid known WSL/Bun RSS pressure |

## Next

M30 product hardening is next ([plan](../superpowers/plans/2026-09-15-m30-product-hardening.md)). Remaining gated ecosystem / native evidence work stays under M31+.
