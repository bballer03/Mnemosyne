# M4 Artifact Explorer Breadth Design

> Status: approved by autonomous execution directive, with design work anchored in Stitch
> Date: 2026-04-15
> Parent: `docs/design/milestone-4-ui-and-usability.md`
> Prior slices:
> - `docs/superpowers/specs/2026-04-14-m4-leak-workspace-design.md`
> - `docs/superpowers/specs/2026-04-15-m4-investigation-slices-design.md`
> Stitch project: `projects/11463443557609217785`
> Slice 4 design screens:
> - `projects/11463443557609217785/screens/783bcf05d0264375a8ad5f6cabcf707c`
> - `projects/11463443557609217785/screens/60a7c166dfa24684b9edf9c87a39d204`

## Goal

Add a dedicated browser-first `Artifact Explorer` route that turns the loaded analysis artifact into a deeper exploration surface centered on histogram browsing plus optional analyzer modules, while staying fully artifact-driven and visibly honest about missing sections.

## Why This Slice Exists

The triage dashboard and leak workspace are now strong enough that the next highest-leverage browser work is breadth from the artifact itself rather than more live-bridge depth.

Current runtime truth supports that move:

- the Rust `AnalyzeResponse` already carries more than the frontend currently exposes:
  - `histogram`
  - `recommendations`
  - `string_report`
  - `collection_report`
  - `top_instances`
  - `classloader_report`
  - `unreachable`
- the current frontend artifact adapter only preserves:
  - summary
  - leaks
  - recommendations
  - graph counts
  - histogram
  - provenance
- the dashboard only uses histogram as a compact snapshot rather than as an explorer
- Stitch exploration for this slice already converged on a three-column `Artifact Explorer` workspace with:
  - analyzer rail
  - dominant histogram explorer
  - selected bucket detail panel

That means Slice 4 should not invent new analysis or depend on live runtime transport. It should widen the browser-first artifact workflow by exposing already-shipped artifact-backed sections honestly.

## Approaches Considered

### 1. Keep histogram exploration inside the dashboard

Expand the dashboard with more histogram controls and analyzer cards.

Pros:

- small routing change
- fast visible growth

Cons:

- overloads the dashboard again
- mixes triage and exploration responsibilities
- weak fit for selected-bucket detail and larger analyzer rails

### 2. Add a dedicated artifact explorer route (recommended)

Introduce `/artifacts/explorer` as a new artifact-backed workspace dedicated to histogram exploration and analyzer breadth.

Pros:

- clean separation between triage and exploration
- matches the Stitch direction already generated for this slice
- lets histogram become the dominant surface without crowding leak triage
- scales into broader explorer work while staying artifact-only

Cons:

- requires route, adapter, and new screen work together
- introduces another top-level operational path to keep synchronized

### 3. Skip breadth and jump straight to dominator/object/query views

Move immediately into heavier MAT-style explorer parity.

Pros:

- more obviously competitive on paper

Cons:

- skips the already-available artifact-backed breadth that can ship now
- risks mixing artifact-only and live/query-driven surfaces too early
- adds more complexity before the artifact explorer story is complete

## Chosen Approach

Add a dedicated `Artifact Explorer` route.

This route remains strictly artifact-driven. It uses the already loaded browser artifact only, exposes optional analyzer sections only when those fields exist in the artifact, and labels missing sections explicitly rather than faking liveness.

## Scope

### In scope

- a new artifact-backed browser route for `Artifact Explorer`
- dominant histogram explorer with:
  - search
  - sorting
  - group context
  - retained vs shallow comparison
  - selected bucket state
- analyzer rail modules for artifact-backed sections when present:
  - recommendations
  - strings
  - collections
  - top instances
  - classloaders
  - unreachable summary
- explicit absent/unavailable cards when those sections are not present in the artifact
- selected histogram bucket detail that derives only from artifact data
- navigation between dashboard, artifact explorer, and leak workspace
- frontend parser/type updates needed to carry the optional analyzer data into the UI

### Out of scope

- live local bridge work
- dominator tree explorer
- object inspector
- query console
- source map / explain / fix / gc-path controls inside the explorer
- new Rust analysis logic
- adding new artifact fields on the Rust side unless a frontend gap proves the current serialized contract is insufficient

## UX Contract

### Primary user goal

After loading an artifact and finishing initial leak triage, the user should be able to pivot into a denser explorer that answers:

- what occupies the heap by class/package/classloader grouping
- how retained and shallow sizes compare across buckets
- what optional artifact analyzers are available in this snapshot
- what details are known for one selected histogram bucket

### Honesty rules

- no live bridge controls appear in this route
- no section fabricates results when the artifact lacks that section
- missing optional artifact sections render explicit absent/unavailable cards
- selected bucket detail only displays relationships that can be derived from current artifact fields
- if the artifact cannot prove a link, the UI says so rather than implying deeper knowledge

### Layout direction

Use the Stitch-backed three-column console layout:

1. left analyzer rail
2. dominant center histogram explorer
3. right selected bucket detail panel

This matches screens:

- `projects/11463443557609217785/screens/783bcf05d0264375a8ad5f6cabcf707c`
- `projects/11463443557609217785/screens/60a7c166dfa24684b9edf9c87a39d204`

## Data Model And Runtime Truth

The current Rust `AnalyzeResponse` already includes these optional sections:

- `histogram`
- `unreachable`
- `thread_report`
- `classloader_report`
- `collection_report`
- `string_report`
- `top_instances`
- `recommendations`

For this slice, the browser artifact contract should add frontend parsing/types for the sections the new explorer will surface directly:

- `string_report`
- `collection_report`
- `top_instances`
- `classloader_report`
- `unreachable`

`thread_report` remains out of scope for this slice because the chosen Stitch direction and slice program focus on histogram breadth plus analyzer modules, not thread investigation.

## Route Design

Add:

- `/artifacts/explorer`

Behavior:

- if no artifact is loaded, redirect to the loader route
- if artifact is loaded, render the explorer immediately from browser state
- dashboard should link into the explorer
- leak workspace remains separate and entered through leak-specific actions

## Screen Responsibilities

### Artifact Explorer route shell

Responsible for:

- route guard for missing artifact
- artifact identity context
- selected histogram bucket state
- top-level navigation between dashboard, artifact explorer, and leak workspace

### Histogram explorer panel

Responsible for:

- rendering all histogram entries, not just a snapshot
- search/filter/sort interactions
- retained vs shallow comparison bars
- selected-row emphasis
- group-by context already present in the artifact

### Analyzer rail

Responsible for compact summaries of:

- recommendations
- strings
- collections
- top instances
- classloaders
- unreachable summary

Each card must show one of:

- artifact-backed summary
- explicit absent/unavailable state

### Selected bucket detail panel

Responsible for:

- selected histogram key
- instance and size context
- any leak relationships derivable from artifact leak rows
- provenance/readiness notes for what can and cannot be inferred from the artifact

## Derivation Rules

The selected bucket detail panel may derive limited artifact-backed context by matching current histogram keys against existing artifact leak rows.

Allowed:

- exact class-name matches when histogram is grouped by class
- package-prefix related leaks when histogram is grouped by package and the prefix clearly matches leak class names
- counts and size summaries taken directly from the selected histogram entry

Not allowed:

- inferring object graphs, dominator paths, or source mappings
- claiming leak relationships when grouping semantics do not support them clearly
- inventing selected-object details from top instances unless the artifact explicitly provides them

## Failure And Absent-State Semantics

### Histogram

- route should still load if histogram is absent
- the center panel should show an explicit artifact-missing state instead of empty table chrome

### Analyzer modules

- `present` when the parsed artifact section exists
- `absent` when the artifact omits the section entirely
- `empty` when the section exists but contains zero meaningful rows

The UI must keep `absent` and `empty` visibly distinct.

## File-Level Design

### Existing files to extend

- `ui/src/lib/analysis-types.ts`
  - parse and type the optional artifact-backed analyzer reports needed for Slice 4
- `ui/src/lib/analysis-types.test.ts`
  - verify those sections parse correctly and remain optional
- `ui/src/features/dashboard/DashboardPage.tsx`
  - add navigation into the artifact explorer
- `ui/src/features/dashboard/DashboardPage.test.tsx`
  - verify route access into the explorer
- `ui/src/app/router.tsx`
  - register the new route

### New frontend area

- `ui/src/features/artifact-explorer/`
  - route shell
  - histogram explorer panel
  - analyzer rail components
  - selected bucket detail panel
  - tests for absent/present artifact sections

## Verification Strategy

### Frontend verification

- artifact parser tests for new optional analyzer sections
- route tests for dashboard to artifact explorer navigation
- explorer tests for:
  - missing artifact redirect
  - histogram search/sort/filter
  - selected bucket detail rendering
  - present vs absent analyzer cards
- full frontend `bun test`
- full frontend `bun run build`
- full frontend `bun run lint`

### Rust verification

- keep existing Rust tests green
- do not add Rust-side work unless the frontend discovers a concrete artifact-contract gap

### Commands

- `npx --yes bun test`
- `npx --yes bun run build`
- `npx --yes bun run lint`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

## Risks

- the explorer can drift into fake insight if selected-bucket detail overreaches beyond artifact truth
- extending the frontend artifact type too broadly can create unnecessary parser work for sections not yet surfaced
- the route can become a second dashboard unless histogram exploration is allowed to dominate the center workspace

## Decision

Proceed with Slice 4 as:

1. extend the frontend artifact adapter for the already-shipped optional analyzer sections needed here
2. add a dedicated `Artifact Explorer` route
3. make histogram exploration the main center surface
4. add compact analyzer rail cards with honest absent/empty states
5. keep all behavior browser-first and artifact-only
