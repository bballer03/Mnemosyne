# MAT UI polish evidence

**Date:** 2026-09-16
**Scope:** Superclass histogram hierarchy, perspective discoverability, and operator labels
**Verdict:** **FOCUSED CORE/UI CONTRACT MET; FULL ECLIPSE MAT CLONE AND PACKAGED GUI NOT PROVEN**

## What changed

- `HistogramEntry` now has an additive optional `parent_key` field. Superclass regroup derives it only from parsed `ClassInfo.super_class_id` data.
- A parent link is emitted only when the resolved parent is another returned bucket. Missing classes, absent parent buckets, conflicting relations, self-links, and cyclic relations remain flat.
- The existing histogram UI already parsed `parent_key` and rendered a collapsible forest. Live superclass regroup can now activate that path without client-side ancestry inference.
- The Wave 1 `PerspectiveSwitcher` is mounted by `InvestigationChromeLayout`, which wraps every current application route. Focused component coverage also confirms its visible heading and fixed-layout controls.
- Operator-facing labels now use the concise MAT vocabulary **Histogram**, **Dominator Tree**, and **Object Inspector**. Route paths and underlying contracts are unchanged.

## Focused verification

The implementation was developed against a failing core parent-link assertion, then verified with:

```text
cargo test -p mnemosyne-core --lib histogram_
8 passed; 0 failed

bun test src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx --max-concurrency=1
7 passed; 0 failed

bun test src/features/heap-explorer/heap-explorer-query-client.test.ts --max-concurrency=1
36 passed; 0 failed

bun test src/features/heap-explorer/components/ModeRail.test.tsx --max-concurrency=1
1 passed; 0 failed

bun test src/app/InvestigationBreadcrumbs.test.tsx --max-concurrency=1
2 passed; 0 failed

bun test src/features/investigation/PerspectiveSwitcher.test.tsx --max-concurrency=1
2 passed; 0 failed
```

Repository-wide verification is recorded in the pull request checks.

## Honesty boundary

This closes the parent-contract gap for graph-backed superclass regroup and exercises the existing React expand/collapse behavior. It does not synthesize ancestry for flat artifacts, claim arbitrary Eclipse MAT docking or report parity, or prove a packaged Tauri GUI on WSL.
