# M4 Dashboard First Slice Design

> Status: approved in-session
> Date: 2026-04-14
> Parent: `docs/design/milestone-4-ui-and-usability.md`
> Visual reference: Stitch project `projects/11463443557609217785`, screen `projects/11463443557609217785/screens/e6f417ad65cc4a4e97f4bf467b02d2b4` (`Mnemosyne Triage Dashboard`)

## Goal

Ship the first real M4 UI surface by turning `mnemosyne analyze --format html` into an interactive, self-contained leak-triage dashboard that feels like a dense engineering console and reuses the current analysis output without requiring a web server.

## Why This Slice

M4 is the next full open milestone, but the full milestone is too large to build safely in one batch. The fastest evidence-backed path is to start with the already-shipped HTML report surface and make it interactive before introducing `serve --web`, routing, browser state, or upload flows.

Current runtime truth already gives this slice a strong base:

- `AnalyzeResponse` already carries the data needed for a first dashboard: summary, leaks, graph metrics, histogram, provenance, and optional secondary analysis sections
- `core::report::renderer::render_html()` already exists and is XSS-hardened through `escape_html()`
- the CLI already emits HTML through `mnemosyne analyze --format html`
- M3 delivered the analysis depth that makes a triage dashboard useful: leak suspects, grouped histogram data, graph metrics, and provenance markers

That means the first M4 step does not need new analysis algorithms or a new transport. It needs a better presentation layer.

## Approaches Considered

### 1. Pure HTML-first dashboard

Treat the first slice only as a richer report artifact and optimize purely for static-file consumption.

Pros:

- smallest implementation surface
- no new runtime or dependency model
- easiest path to a shippable first M4 outcome

Cons:

- may need later redesign when `serve --web` arrives
- risks baking report-only layout assumptions into the first screen

### 2. HTML-first implementation with a reusable app-shaped layout (recommended)

Implement the first slice as interactive HTML, but shape the layout so it can later become the dashboard view inside the local web UI with minimal redesign.

Pros:

- preserves the shortest path to value
- gives M4 a real shipped surface quickly
- reduces later redesign churn when `serve --web` lands

Cons:

- requires a little more upfront discipline around layout boundaries
- cannot express every future app interaction in the first slice

### 3. Web-app-first dashboard

Design and implement the first screen as if `serve --web` already exists, even if the initial implementation still renders from HTML.

Pros:

- most aligned with the eventual local web UI

Cons:

- pulls in route, state, and browser-app assumptions too early
- increases design and implementation complexity before the first UI artifact ships

## Chosen Approach

Use the HTML-first implementation with a reusable app-shaped layout.

In practice, that means:

- the first delivered artifact remains `mnemosyne analyze --format html`
- the dashboard is structured like the top-level screen of a future local web UI
- the slice focuses on leak triage first, with compact secondary context panels
- drill-down affordances are visible now even if deeper M4 screens land later

## Scope

### In scope

- enhanced interactive HTML output for `mnemosyne analyze --format html`
- dark, dense engineering-console presentation
- header bar with heap identity, analyzed timestamp, profile/status badges, and provenance visibility
- summary metric strip with total objects, heap size, leak count, graph nodes, and elapsed time
- leak-triage-first primary workspace
- client-side search, filter, and sorting for the leak list
- inline expandable leak details inside the dashboard
- compact graph metrics panel
- compact histogram snapshot panel
- visible drill-down actions for future M4 screens:
  - inspect leak
  - inspect object
  - trace GC path
  - open dominators
- graceful rendering for empty or partially populated data
- regression coverage for the new HTML dashboard structure

### Out of scope

- `mnemosyne serve --web`
- local web server routes or browser-open behavior
- upload/select heap flows
- live parsing progress via WebSocket or SSE
- dominator tree browser implementation
- object inspector implementation
- GC path visualizer implementation
- query console implementation
- new core analysis algorithms or response-shape changes for M4-specific data
- mobile-responsive design
- hosted or remote web access

## UX Contract

### Primary user goal

The first dashboard exists for incident-response triage: open the report, identify the most severe or highest-retained leak suspects, narrow the list quickly, expand one item for more context, and move into deeper investigation.

### Visual tone

- dark theme
- dense engineering-console styling
- high information density
- restrained color used as a signal, not decoration
- monospace accents for IDs and numeric data where useful
- sharp table-driven layout rather than card-heavy consumer SaaS styling

The intended feel is "hardened analysis console," not "friendly analytics app."

### Information architecture

1. Header bar
   - heap file name
   - analyzed timestamp
   - profile or mode badge when present
   - provenance/status badge area
   - space for future dashboard-level actions
2. Summary strip
   - total objects
   - total heap size
   - leak count
   - graph nodes
   - elapsed time
3. Primary workspace
   - top leak suspects table or stacked table-like list
   - sticky filter and search controls directly above the list
   - inline expandable detail area per leak
4. Secondary context panels
   - graph metrics
   - histogram snapshot

### Interaction model

- default sort is severity first, then retained size
- search matches at least class name, leak ID, and description
- first-slice filters should cover:
  - severity
  - provenance present vs none
  - minimum retained size
- leak details expand inline rather than navigating away
- if client-side behavior fails, the page must remain readable as static HTML

## Data Model Use

This slice should consume existing `AnalyzeResponse` data rather than inventing a new dashboard-only backend shape.

Primary data surfaces:

- `AnalyzeResponse.summary`
- `AnalyzeResponse.leaks`
- `AnalyzeResponse.graph`
- `AnalyzeResponse.histogram`
- `AnalyzeResponse.provenance`

Secondary optional surfaces may remain unused in this first slice unless they naturally fit without bloating the dashboard:

- `thread_report`
- `string_report`
- `collection_report`
- `classloader_report`
- `top_instances`
- `ai`
- `unreachable`

For the approved first slice, the dashboard should stay focused on core analysis only. Optional analyzers should not be made first-class dashboard panels yet.

## File-Level Design

### Rendering surface

- `core/src/report/renderer.rs`
  - remains the public report-dispatch surface
  - continues to own `render_report()` and the format switch
  - the current static `render_html()` implementation becomes the entrypoint for the dashboard HTML slice

### Internal organization

- keep the public report surface unchanged
- allow a small extraction if needed to keep the implementation reviewable, because `renderer.rs` is already large
- preferred extraction target if the HTML dashboard code becomes unwieldy:
  - `core/src/report/html_dashboard.rs`
  - purpose: build the interactive HTML dashboard while keeping `renderer.rs` as dispatch + shared helpers

This is a maintainability choice, not a contract change. The public output path remains `OutputFormat::Html`.

### CLI surface

- `cli/src/main.rs`
  - no new command required for this slice
  - `analyze --format html` remains the delivery path
- help text or examples may be updated later if the implementation meaningfully changes how the HTML artifact is described

### Tests

- renderer unit tests in the report module should validate the new dashboard structure
- CLI integration should confirm the enhanced HTML path is wired through the existing `analyze` command

## Implementation Boundaries

### Required behavior

- the HTML artifact is self-contained and viewable locally in a browser
- leak triage is visually dominant over all other sections
- provenance remains visible and honest
- empty-leak runs render a clear "No leak suspects detected" state instead of an empty shell
- histogram and graph context appear as supporting panels, not equal peers to the leak table

### Deferred behavior

- no browser routing
- no server-backed pagination
- no route-based object detail screens
- no live progress updates
- no graph-heavy visualization dependency introduced just for the first slice

## Verification Strategy

### Renderer-level verification

- unit tests for summary strip rendering
- unit tests for leak-table rendering
- unit tests for empty leak state
- unit tests for provenance badge rendering
- unit tests confirming collapsible detail markup exists
- unit tests confirming search/filter/sort markup hooks exist
- regression tests that preserve HTML escaping and XSS hardening

### CLI verification

- integration test confirming `mnemosyne analyze --format html` emits the dashboard structure
- integration test covering the zero-leak dashboard state if a suitable fixture path already exists

### Non-goals for this slice

- browser automation is not required unless the implementation introduces behavior that cannot be safely regression-tested through rendered HTML structure alone

## Risks

- `renderer.rs` is already large, so the HTML dashboard can become hard to review if everything stays inline
- the first slice can accidentally drift toward a full app shell if too many future-screen affordances are made active immediately
- client-side interaction can obscure graceful fallback behavior if the static HTML path is not kept readable
- the UI can become generic and product-like instead of tool-like if density and signal hierarchy are not enforced during implementation
- optional analysis sections can bloat the first dashboard and dilute leak triage if they are pulled in too early

## Decision

Proceed with the first M4 slice as an interactive HTML triage dashboard in this order:

1. upgrade the HTML report into a dense dashboard shell
2. make leak triage the dominant interactive region
3. add compact summary and supporting graph/histogram context
4. keep the result self-contained and reusable as the future dashboard shape for `serve --web`
