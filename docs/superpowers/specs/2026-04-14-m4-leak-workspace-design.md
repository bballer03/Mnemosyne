# M4 Leak Workspace Design

> Status: approved in-session
> Date: 2026-04-14
> Parent: `docs/design/milestone-4-ui-and-usability.md`
> Prior slice: `docs/superpowers/specs/2026-04-14-m4-dashboard-first-slice-design.md`
> Visual reference: Stitch project `projects/11463443557609217785`, current visual lineage from `projects/11463443557609217785/screens/e6f417ad65cc4a4e97f4bf467b02d2b4` (`Mnemosyne Triage Dashboard`)
> Note: this design defines the next browser-first M4 follow-on slice after the shipped loader + triage dashboard foundation.

## Goal

Ship the next M4 screen family as a dedicated leak-detail workspace with nested subroutes for `overview`, `explain`, `gc-path`, `source-map`, and `fix`, preserving the browser-first React frontend while introducing a narrow local live-detail bridge for deeper investigation surfaces.

## Why This Slice

The first browser-first M4 slice established the shared UI foundation, local artifact loading, and a triage dashboard. That slice intentionally stopped at dashboard-level review with inline detail expansion and left the dashboard `Trace` action as the boundary to future deeper routes.

Current runtime truth supports the next step:

- the dashboard already has stable leak identities, provenance markers, retained sizes, and artifact context via serialized `AnalyzeResponse`
- the backend already exposes deeper live investigation surfaces through existing core/MCP operations:
  - `explain_leak`
  - `find_gc_path`
  - `map_to_code`
  - `propose_fix`
- the repo still does not have a browser-facing general-purpose local API layer, so the next route should widen runtime scope narrowly rather than introducing a broad app-wide RPC surface all at once
- the current dashboard is intentionally restrained; deeper investigation belongs in a dedicated workspace, not as more dashboard row expansion

That means the best next M4 step is not "add more dashboard panels." It is "promote one selected leak into its own operator workspace and bind it to existing deeper investigation flows honestly."

## Approaches Considered

### 1. Single-page leak cockpit

Build one large leak detail page that renders explain, GC path, source mapping, and fix guidance together without subroutes.

Pros:

- fastest route to a visible deep-detail screen
- fewer router changes up front

Cons:

- becomes crowded quickly
- mixes unrelated loading/failure states together
- weak foundation for later deeper M4 explorer growth

### 2. Leak workspace with nested subroutes (recommended)

Build a persistent leak workspace shell and expose child routes for `overview`, `explain`, `gc-path`, `source-map`, and `fix`.

Pros:

- best fit for a dense operational UI
- isolates mixed live/static states cleanly
- scales naturally into a broader M4 explorer family
- avoids turning the dashboard into a second app

Cons:

- more upfront route/state structure than a single page
- requires a shell-level coordination model for child views

### 3. Keep deep investigation inside the dashboard

Reuse the current dashboard route and expand rows further, or attach a side drawer/modal for all deep-detail functionality.

Pros:

- smallest navigation change from the current UI

Cons:

- overloaded dashboard responsibility
- weak fit for live-detail workflows with multiple backends
- harder to preserve context and revisit specific deep-detail surfaces

## Chosen Approach

Use the leak workspace with nested subroutes.

In practice, that means:

- dashboard `Trace` navigates into a dedicated leak workspace route
- the workspace has a persistent shell and child routes for the five detail surfaces
- the workspace remains browser-first in presentation and artifact-backed for baseline context
- only this workspace is allowed to use a narrow local live-detail bridge in this slice
- no app-wide generalized bridge or future route family is required in the same batch

## Scope

### In scope

- dedicated leak workspace route entered from the dashboard
- nested child routes:
  - `overview`
  - `explain`
  - `gc-path`
  - `source-map`
  - `fix`
- shell-level selected leak header and artifact context
- shell-level route guard for missing artifact and unknown leak IDs
- artifact-backed overview subview with dependency readiness indicators
- narrow frontend adapter boundary for live detail calls used only by the leak workspace
- subview-specific request state, caching, refresh, and fallback/unavailable/error semantics
- explicit unavailable GC-path behavior when no concrete object ID exists for the selected leak
- provenance visibility and honest fallback labeling across subviews
- docs and routing updates needed to describe the leak workspace as the next M4 route

### Out of scope

- dominator browser route
- object inspector route
- query console route
- generalized browser-wide RPC client for every future M4 feature
- a resolver that invents or guesses an object ID from a leak ID
- widening the first dashboard artifact contract just to force GC path to be live in this slice
- Tauri packaging
- hosted/browser-server deployment
- deeper Stitch-to-code automation beyond the existing design lineage

## UX Contract

### Primary user goal

After selecting a suspect from the dashboard, the user should enter a focused leak workspace that preserves the selected leak's identity, exposes deeper investigation surfaces through separate modes, and makes it obvious which panels are artifact-backed, live, unavailable, or fallback-driven.

### Visual tone

- same dark engineering-console lineage as the loader and triage dashboard
- hard-edged, route-workspace styling rather than consumer-detail tabs
- dense left-heavy information architecture
- severity, provenance, and readiness shown as operational signal
- technical typography with monospace treatment for IDs, timestamps, paths, and code-oriented data

The intent remains "operator workspace for leak investigation," not "pretty details page."

### Information architecture

1. Persistent workspace shell
   - back to dashboard action
   - artifact identity
   - selected leak identity and severity
   - retained/shallow size, score, provenance, and last-refresh signal
2. Workspace mode navigation
   - `overview`
   - `explain`
   - `gc-path`
   - `source-map`
   - `fix`
3. Shared dependency/status rail
   - bridge availability
   - `projectRoot` status
   - provider/source/object-target readiness
4. Active subview workspace
   - overview previews or live detail output depending on route

### Interaction model

- `/leaks/:leakId` redirects to `/leaks/:leakId/overview`
- live subviews load on demand when opened rather than all at once
- cached results may be reused until the user explicitly refreshes the subview
- shell context persists while changing child routes
- failure in one subview does not collapse the whole workspace

## Route Design

Recommended route tree:

- `/leaks/:leakId`
- `/leaks/:leakId/overview`
- `/leaks/:leakId/explain`
- `/leaks/:leakId/gc-path`
- `/leaks/:leakId/source-map`
- `/leaks/:leakId/fix`

Behavior:

- if no artifact is loaded, redirect to the artifact loader route
- if the leak ID is unknown in the current artifact, render a shell-level invalid-selection state with a `Back to dashboard` action
- the dashboard inline `Inspect` expansion remains as-is for lightweight context and is not replaced by the workspace

## Data Flow

This slice mixes artifact-backed context with route-specific live calls:

1. user loads a serialized `AnalyzeResponse` artifact in the browser
2. dashboard renders from that artifact
3. user selects dashboard `Trace`
4. router navigates to `/leaks/:leakId/overview`
5. workspace resolves the selected leak from the existing artifact store
6. `overview` renders immediately from artifact-backed data
7. child routes invoke live detail actions only when opened

This preserves the browser-first artifact workflow while allowing deeper live follow-through only where the workspace needs it.

## Live Detail Bridge Contract

The leak workspace introduces a narrow local bridge contract used only inside this feature.

Browser-facing operations for this slice:

- `explainLeak(leakId, heapPath | session)`
- `findGcPath(objectId, heapPath)`
- `mapToCode(leakId, className, projectRoot)`
- `proposeFix(leakId, heapPath | session, projectRoot, style)`

Design rules:

- view components do not receive raw transport payloads
- the adapter normalizes transport payloads into workspace-specific typed results and status enums
- if no bridge exists, the workspace shell still renders and live panels become unavailable rather than crashing
- this slice does not create a generalized app-wide browser RPC layer

## Frontend Boundaries

### Leak workspace shell

Responsible for:

- selected leak resolution from route params
- shell layout and persistent context
- subroute navigation
- shell-level invalid/missing-artifact states

### Overview subview

Responsible for:

- artifact-backed narrative summary
- dependency readiness matrix
- compact preview blocks for explain, GC path, source map, and fix

### Explain subview

Responsible for:

- live explain request lifecycle
- rendering AI explanation content
- honest error vs fallback distinction

### GC path subview

Responsible for:

- rendering a live GC path when a concrete object target exists
- rendering an explicit unavailable state when no object target exists
- never inventing a synthetic object target in the browser just to make the panel appear active

### Source map subview

Responsible for:

- live mapping request lifecycle
- code-location results, snippets, and optional git metadata
- synthetic/unmapped fallback labeling

### Fix subview

Responsible for:

- live fix request lifecycle
- diff preview and suggestion context
- fallback labeling when provider/source-aware fix generation drops to heuristic guidance

### Workspace store

Responsible for:

- selected leak lookup derived from route params
- `projectRoot`
- bridge availability
- per-subview status values:
  - `idle`
  - `loading`
  - `ready`
  - `error`
  - `unavailable`
  - `fallback`
- per-subview cached result payloads
- explicit refresh triggers per subview

## File-Level Design

Recommended additions:

- `ui/src/features/leak-workspace/`
  - `LeakWorkspaceLayout.tsx`
  - `LeakWorkspaceOverview.tsx`
  - `LeakExplainPage.tsx`
  - `LeakGcPathPage.tsx`
  - `LeakSourceMapPage.tsx`
  - `LeakFixPage.tsx`
  - `leak-workspace-store.ts`
  - `live-detail-client.ts`
  - `types.ts`

Router updates:

- add nested leak workspace routes to `ui/src/app/router.tsx`
- wire the dashboard `Trace` action to navigate into the new workspace

The existing artifact store and dashboard store remain in place; the leak workspace adds a focused feature-local state layer rather than widening the dashboard store further.

## Failure and Fallback Semantics

### Overview

- never depends on live backends

### Explain

- `error` for live explain transport/runtime failure
- `fallback` only if the backend explicitly reports fallback/provenance semantics

### Source map

- `unavailable` when `projectRoot` is missing
- `fallback` when the backend returns synthetic/unmapped results
- `error` for transport/runtime failure

### GC path

- `unavailable` when no object ID exists for the selected leak in the current runtime context
- `error` for live call failure
- `fallback` only when backend provenance says the path is fallback/synthetic

### Fix

- `unavailable` when required local context is absent
- `fallback` when provider/source-aware generation drops to heuristic guidance
- `error` for transport/runtime failure

These distinctions must remain visible in the UI; do not collapse `error`, `fallback`, and `unavailable` into one generic empty state.

## Verification Strategy

### Frontend verification

- route tests for dashboard-to-workspace navigation
- route tests for missing artifact redirect and unknown leak handling
- workspace tests for shell persistence across child routes
- overview tests for artifact-backed rendering without bridge access
- adapter tests for payload normalization and honest status mapping
- subview tests for success, error, fallback, and unavailable states
- GC-path tests that assert explicit unavailable behavior when no object target exists

### Rust-side verification

- keep existing `explain_leak`, `find_gc_path`, `map_to_code`, and `propose_fix` tests green
- add or update one integration test only if this slice requires a newly documented runtime contract for the bridge boundary

### Verification commands

- `npx --yes bun test`
- `npx --yes bun run build`
- `npx --yes bun run lint`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

## Risks

- the workspace can turn into a kitchen sink if it tries to solve dominator browsing, object inspection, and query execution in the same batch
- mixed artifact/live freshness can confuse users if the UI does not label availability and fallback states clearly
- the GC-path route is structurally constrained by object-ID availability; forcing it to look live when it is not would be dishonest
- a broad app-wide transport layer would increase scope and couple future routes prematurely

## Decision

Proceed with the next M4 slice in this order:

1. add the leak workspace shell and nested routes
2. wire dashboard `Trace` into the workspace and keep inline `Inspect` intact
3. ship the artifact-backed `overview` subroute and shared workspace store
4. add the narrow live-detail adapter boundary
5. land `explain`, `source-map`, and `fix` subviews
6. land `gc-path` as a best-effort subview with explicit unavailable behavior when no object target exists
7. update docs/status and verify the full frontend and Rust workspace again
