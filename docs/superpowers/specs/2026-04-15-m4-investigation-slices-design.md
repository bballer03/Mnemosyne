# M4 Investigation Slices Design

> Status: approved by autonomous execution directive
> Date: 2026-04-15
> Parent: `docs/design/milestone-4-ui-and-usability.md`
> Prior slices:
> - `docs/superpowers/specs/2026-04-14-m4-dashboard-first-slice-design.md`
> - `docs/superpowers/specs/2026-04-14-m4-leak-workspace-design.md`
> Note: this design turns the remaining browser-first UI work into an ordered slice program aimed at making Mnemosyne more effective than MAT/VisualVM for leak triage and code-aware follow-through.

## Goal

Advance the browser-first UI from a good triage shell into a genuinely strong leak-investigation workflow by prioritizing operator throughput, code-aware follow-through, and honest evidence display over broad but shallow screen count.

## Why This Design Exists

The current UI is no longer just a dashboard: it already ships a loader, a triage route, and a leak workspace with `overview`, `explain`, `gc-path`, `source-map`, and `fix` routes. The problem is that the current gap is now obvious:

- the route family exists
- the shared shell exists
- the store exists
- the deeper tabs are wired
- but normal browser use still does not seed the local context needed to activate `source-map`, `fix`, or `gc-path`

That means the next UI work should not be "add more screens quickly." It should be "turn the current investigation path into a real operator workflow first."

If Mnemosyne is going to beat MAT and VisualVM, it does not need to clone every surface immediately. It needs to win on the path users hit first:

1. identify the suspect quickly
2. carry the suspect into a focused workspace
3. see what evidence is available right now
4. activate deeper investigation with as little friction as possible
5. connect leak evidence to source and fix follow-through faster than incumbent tools

## Approaches Considered

### 1. Breadth-first UI expansion

Add many new routes now: histogram explorer, dominator browser, object inspector, query console, recommendations, analyzer views.

Pros:

- visible feature growth quickly
- looks closer to MAT on paper

Cons:

- leaves the current workspace half-activated
- spreads effort across many shallow surfaces
- risks shipping more tabs that still dead-end on missing context

### 2. Depth-first investigation activation (recommended)

Strengthen the existing leak workspace first: activate its missing local context, then make source/fix/gc-path genuinely useful, then widen into broader explorer views.

Pros:

- directly improves the operator path that matters most
- keeps the architecture honest and narrow
- builds a stronger competitive story: less friction from suspect to explanation, source, path, and fix
- avoids adding superficial route count without real investigative depth

Cons:

- some broader explorer screens land later
- requires disciplined sequencing instead of grabbing many visible wins at once

### 3. Desktop-first follow-through now

Shift effort into Tauri or a broader local runtime shell so the bridge problem can be solved more aggressively.

Pros:

- may eventually unlock richer local integration

Cons:

- mixes packaging/runtime work with unresolved workflow questions
- slows iteration on the browser-first UI that already exists
- does not improve current operator throughput as directly as depth-first slice work

## Chosen Approach

Use the depth-first investigation activation path.

That means the browser-first UI program now proceeds in slices that make the current leak workspace genuinely more useful before widening into broader explorer coverage.

## Slice Program

### Slice 1: Investigation Context Activation

Add shell-level workspace context controls that let the operator activate missing local context intentionally rather than waiting for it to appear magically.

This slice introduces:

- route-seeded workspace identity (`leakId`, `heapPath`)
- shell-level local context controls for `projectRoot` and `objectId`
- explicit apply/clear actions instead of guessing values from weak artifact hints
- readiness display that reflects the real current workspace context
- route-level activation of `source-map`, `fix`, and `gc-path` once the user provides honest local inputs

This is the first slice because it converts three currently half-dead tabs into operator-activatable workflows without introducing a generalized app-wide bridge.

### Slice 2: Real Local Detail Bridge

Replace placeholder live-detail behavior with a real narrow bridge for the current leak workspace only.

Priority order inside this slice:

1. `source-map`
2. `fix`
3. `explain`
4. `gc-path`

This slice remains feature-local by design. It should not become a generalized browser RPC layer for every future screen.

### Slice 3: GC Path Workflow Hardening

Turn `gc-path` from a raw target-dependent tab into a workflow that helps the user provide and revisit real object targets safely.

Likely additions:

- recent object target recall inside the workspace
- stronger path rendering once real bridge data exists
- explicit provenance and refresh controls

### Slice 4: Artifact-Driven Explorer Breadth

Add broader browser-first explorer views that can be powered honestly from the existing artifact contract.

Priority candidates:

- histogram explorer
- recommendations / AI insight surfaces
- optional analyzer views for strings, collections, top instances, classloaders, and unreachable summaries when present in the artifact

This slice is where Mnemosyne gains breadth, but only after the core investigation path is already strong.

### Slice 5: Competitive Explorer Surfaces

Only after the prior slices are in place should the UI invest in heavyweight MAT-class surfaces:

- dominator explorer
- object inspector
- query console
- richer diff/explorer cross-navigation

These routes matter for competitive parity, but they are not the highest-leverage next step until the current leak workspace is fully activated and its narrow bridge strategy is proven.

## Slice 1 Detailed Design

### Goal

Make the existing leak workspace routes materially more useful in normal browser use by letting the operator provide the exact local context that the current subviews need.

### In scope

- seed `leakId` and `heapPath` into the workspace store from the active route/artifact
- add shell-level controls for:
  - `projectRoot`
  - `objectId`
- use explicit apply/clear actions so the user controls when downstream views refresh
- expose readiness copy in the shell so users can see what each local input unlocks
- update `overview` so it reflects current workspace context rather than always claiming everything is missing
- add route-level tests proving that shell context activates the currently shipped subviews honestly

### Out of scope

- local storage persistence for these values
- broad app-wide settings infrastructure
- guessing an object target from provenance strings or leak IDs
- real backend bridge transport work
- new explorer routes

### UX contract

The shell should tell the truth and help the operator act:

- `projectRoot` unlocks `source-map` and `fix`
- `objectId` unlocks `gc-path`
- missing context stays visible as missing
- nothing is auto-guessed from weak hints
- changing leak identity resets dependent context as needed

### Data flow

1. dashboard enters `/leaks/:leakId/overview`
2. workspace resolves the current leak from the artifact store
3. workspace seeds `leakId` and `heapPath` into the workspace store
4. operator optionally provides `projectRoot` and/or `objectId`
5. store updates only when the operator explicitly applies the change
6. dependent subviews consume that context and switch from `unavailable` to live/fallback behavior honestly

### Files affected

- `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
- `ui/src/features/leak-workspace/leak-workspace-store.ts`

### Verification

- targeted leak-workspace route tests
- full frontend `bun test`
- full frontend `bun run build`
- full frontend `bun run lint`

## Competitive Design Rule

Mnemosyne does not beat MAT or VisualVM by copying every screen immediately. It beats them by giving the operator a faster and more truthful route from suspect leak to actionable follow-through.

For the next UI batches, prefer:

- stronger evidence chains over more route count
- code-aware follow-through over cosmetic parity
- explicit readiness and provenance over fake liveness
- narrow, composable bridge boundaries over broad premature transport layers

## Decision

Proceed in this order:

1. ship Slice 1: Investigation Context Activation
2. use that slice as the base for a real local detail bridge
3. widen into broader artifact-driven explorer views only after the leak workspace is genuinely activated
4. tackle dominator/object/query parity after the activated workflow proves out
