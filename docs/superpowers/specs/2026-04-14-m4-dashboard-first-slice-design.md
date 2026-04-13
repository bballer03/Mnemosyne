# M4 Dashboard First Slice Design

> Status: approved in-session
> Date: 2026-04-14
> Parent: `docs/design/milestone-4-ui-and-usability.md`
> Visual reference: Stitch project `projects/11463443557609217785`, screen `projects/11463443557609217785/screens/e6f417ad65cc4a4e97f4bf467b02d2b4` (`Mnemosyne Triage Dashboard`)
> Note: this design supersedes the earlier HTML-embedded first-slice direction after the stack decision changed to a shared React frontend with later Tauri packaging.

## Goal

Ship the first real M4 UI surface as a browser-first React application that loads Mnemosyne analysis JSON artifacts, presents a leak-triage dashboard based on the approved Stitch design, and establishes the shared frontend foundation that can later be reused in a Tauri desktop shell with minimal UI rewrites.

## Why This Slice

The original first-slice idea was to enhance `analyze --format html` into an interactive HTML dashboard. That would have been the narrowest path, but it conflicts with the new architectural goal: a maintainable shared UI that can support both browser and desktop app delivery.

Current runtime truth supports the revised direction:

- `AnalyzeResponse` already contains the dashboard data we need: summary, leaks, graph metrics, histogram, provenance, and optional secondary analyzers
- the JSON output path already exists via `OutputFormat::Json` and serializes `AnalyzeResponse` directly
- CLI integration tests already exercise `analyze --format json`, so the data artifact path is real rather than speculative
- the repo does not yet have a frontend workspace, so the first slice must define that boundary clearly instead of layering React awkwardly into the Rust crates

That means the best first M4 step is not "enhance embedded report JS." It is "create the shared frontend foundation and render the first real dashboard from existing JSON output."

## Approaches Considered

### 1. Keep the HTML-first implementation

Continue with the embedded HTML/CSS/JS dashboard and treat React/Tauri as a later rewrite.

Pros:

- smallest short-term implementation surface
- no new frontend toolchain yet

Cons:

- duplicates UI effort later
- poor maintainability for the desired long-term product
- conflicts with the explicit preference to avoid hand-maintained plain HTML/CSS/JS UI logic

### 2. Browser-first shared frontend with later Tauri shell (recommended)

Build a React/TypeScript frontend now, use existing JSON artifacts as the first data source, and keep Tauri as a later packaging/runtime layer around the same frontend.

Pros:

- one maintainable UI codebase for web and desktop
- no premature Rust API/server work required in the first slice
- aligns with the approved stack and future desktop direction
- gives M4 a real product foundation instead of a throwaway report UI

Cons:

- introduces a frontend workspace and build toolchain now
- first slice needs explicit file and packaging boundaries because the repo is currently Rust-only

### 3. Build browser and Tauri targets from day one

Set up both the browser app and the Tauri shell in the first slice.

Pros:

- proves web/desktop parity immediately

Cons:

- increases scope too early
- mixes frontend-foundation work with desktop packaging concerns
- slows delivery of the first usable dashboard

## Chosen Approach

Use the browser-first shared frontend with later Tauri shell.

In practice, that means:

- the first shipped M4 slice is a browser app, not a desktop app yet
- the frontend package manager and script runner are locked to Bun
- the frontend stack is locked to:
  - React
  - TypeScript
  - Vite
  - React Router
  - TanStack Table
  - TanStack Query
  - Zustand
  - Tailwind + shadcn/ui
  - Bun
- the first data source is a Mnemosyne analysis JSON artifact generated from existing CLI/report output
- the frontend is structured so Tauri can later wrap it without rewriting the dashboard UI

## Scope

### In scope

- new browser-first frontend workspace for M4
- app shell and route foundation
- artifact loader for local Mnemosyne analysis JSON files
- dashboard screen matching the approved Stitch design direction
- leak-triage-first workflow
- summary strip using existing `AnalyzeResponse.summary`
- leak table using existing `AnalyzeResponse.leaks`
- compact graph metrics panel using existing `AnalyzeResponse.graph`
- compact histogram snapshot using existing `AnalyzeResponse.histogram`
- provenance visibility using existing response and leak provenance markers
- frontend state for selected artifact, filters, sorting, and expanded leak details
- validation and graceful failure for malformed or incompatible JSON artifacts
- docs and repo structure updates needed to introduce the frontend app honestly
- Bun-based frontend scripts and developer workflow docs

### Out of scope

- Tauri packaging in this first slice
- local Rust API server
- `serve --web`
- object inspector route
- dominator tree route
- GC path viewer route
- query console route
- live parsing progress
- upload-to-backend workflow
- new analysis algorithms or dashboard-only backend data models
- hosted multi-user web deployment

## UX Contract

### Primary user goal

The first dashboard exists for incident-response triage: open a real Mnemosyne analysis artifact in the browser, identify the most severe or highest-retained leak suspects, narrow the set quickly, expand one row for more context, and prepare to drill into deeper M4 screens later.

### Visual tone

- dark engineering-console aesthetic
- high information density
- restrained color used as signal rather than decoration
- table-forward layout rather than soft card-heavy admin UI
- technical typography and clear data hierarchy

The intent remains "serious heap analysis tool," not generic SaaS dashboard.

### Information architecture

1. App shell
   - top-level title and current artifact identity
   - left navigation or compact shell navigation for future M4 routes
   - current route focused on dashboard only in the first slice
2. Dashboard header
   - heap file identity
   - analyzed timestamp
   - profile/status badges
   - provenance summary indicator
3. Summary strip
   - total objects
   - total heap size
   - leak count
   - graph nodes
   - elapsed time
4. Primary workspace
   - top leak suspects table
   - search and filter controls
   - inline expandable leak details
5. Secondary context panels
   - graph metrics
   - histogram snapshot

### Interaction model

- browser-first, stateful dashboard experience
- default sort: severity, then retained size
- search covers class name, leak ID, and description
- initial filters cover:
  - severity
  - provenance present vs none
  - minimum retained size
- leak detail expands inline in the dashboard rather than navigating away
- artifact load errors render a clear, user-readable invalid-artifact state

## Data Flow

The first slice uses a file/artifact flow rather than a live API:

1. Mnemosyne CLI produces analysis JSON from existing `AnalyzeResponse`
2. User opens the browser app
3. User selects a local JSON artifact
4. Frontend validates and parses the artifact
5. Frontend normalizes the parsed JSON into dashboard view state
6. Dashboard renders the approved first-slice panels

This keeps the first slice grounded in real data while avoiding premature server work.

## Frontend Boundaries

### App shell

Responsible for:

- route container
- page framing
- top-level empty/loading/error states
- future route slots for later M4 screens

### Artifact loader

Responsible for:

- selecting a local JSON artifact
- parsing file contents in browser
- validating the JSON shape against the first-slice contract
- returning typed frontend data or a clear error state

### Dashboard route

Responsible for:

- assembling the triage screen from typed data
- owning dashboard-level filters and sort state
- coordinating summary strip, leak table, and secondary panels

### Presentation components

Responsible for:

- summary strip
- leak table
- leak detail expansion
- graph metrics panel
- histogram panel
- provenance badges

### State layer

Responsible for:

- selected artifact metadata
- parsed analysis payload
- filter state
- sort state
- expanded leak row state

## Data Model Strategy

The frontend should not define a brand-new product-specific backend contract in the first slice. It should consume the existing JSON shape and create a narrow typed adapter layer around it.

### Canonical source

- existing serialized `AnalyzeResponse`

### First-slice required fields

- `summary`
- `leaks`
- `graph`
- `histogram`
- `provenance`
- `elapsed`

### Optional fields

- `thread_report`
- `string_report`
- `collection_report`
- `classloader_report`
- `top_instances`
- `ai`
- `unreachable`

These optional fields should be tolerated by the loader but do not need first-class dashboard panels in this slice.

## File-Level Design

### Workspace shape

Because the repo currently has no frontend workspace, the first slice should add one explicitly rather than mixing frontend build assets into the Rust crates.

Recommended structure:

- `ui/`
  - browser-first React app
- `ui/package.json`
  - frontend package manifest executed through Bun
- `ui/bun.lock`
  - committed frontend lockfile
- `ui/src/main.tsx`
  - React entrypoint
- `ui/src/app/router.tsx`
  - route setup
- `ui/src/app/providers.tsx`
  - TanStack Query and other global providers
- `ui/src/features/artifact-loader/`
  - local JSON selection, parsing, and validation
- `ui/src/features/dashboard/`
  - dashboard route and presentation components
- `ui/src/lib/analysis-types.ts`
  - typed adapter for the JSON artifact shape
- `ui/src/lib/formatting.ts`
  - dashboard display helpers
- `ui/src/state/`
  - Zustand stores for artifact and dashboard UI state

### Frontend tooling policy

- Bun is the only supported frontend package manager and script runner for this slice
- frontend setup, install, dev, test, and build commands use `bun` / `bunx`
- docs should not dual-document npm, pnpm, or yarn commands for this workspace
- frontend runtime code should remain standard browser/React code rather than using Bun-specific runtime APIs, so the app stays portable to browser and later Tauri builds

### Rust-side contract surface

- `core/src/report/renderer.rs`
  - JSON output remains the current artifact path
- `cli/src/main.rs`
  - may need small doc/help/example updates later to make the frontend artifact workflow discoverable

No live API/server layer is required in this slice.

## Verification Strategy

### Frontend verification

- unit tests for artifact validation/parsing
- component tests for summary strip and leak table rendering
- component tests for empty, invalid, and malformed artifact states
- component tests for search/filter/sort behavior
- route-level test for opening the dashboard after loading a valid artifact

### Rust-side verification

- keep existing JSON analysis CLI coverage green
- add or update one integration test if needed to ensure the frontend-facing artifact path remains truthful and documented

### Non-goals for this slice

- no end-to-end Tauri testing yet
- no browser-to-Rust live API tests yet
- no requirement for the full later M4 route set

## Risks

- the first slice can become too large if it tries to add live APIs or Tauri in the same batch
- the frontend can drift from the real Rust data model if the typed adapter layer becomes a second contract instead of a thin normalization layer
- using a heavy generic component library aesthetic would undermine the desired engineering-console tone
- file-import UX can become clumsy if artifact loading and invalid-state handling are not designed clearly

## Decision

Proceed with the first M4 slice in this order:

1. create the browser-first shared frontend workspace
2. add local JSON artifact loading and typed adaptation of `AnalyzeResponse`
3. ship the dashboard route and app shell from the approved Stitch direction
4. defer Tauri packaging and live Rust APIs until after the browser-first slice is stable
