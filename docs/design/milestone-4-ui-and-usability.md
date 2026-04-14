# Milestone 4 - UI & Usability

> **Status:** In Progress  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-04-14

---

## Objective

Make Mnemosyne visually accessible to developers who prefer graphical exploration of heap data. Deliver interactive HTML reports and a browser-first local UI that leverage the analysis features built in M1-M3, widening the user base beyond CLI-only users.

## Context

Heap analysis is fundamentally a visual and exploratory task. Developers investigating memory leaks navigate dominator trees, inspect reference chains, compare object counts, and drill into specific classes - activities that map poorly to sequential text output. Eclipse MAT's success is inseparable from its GUI-based tree explorers and table views. For Mnemosyne to compete for adoption, it must offer interactive exploration while preserving the CLI-first, automation-friendly foundation.

The current reporting layer (Text, Markdown, HTML, TOON, JSON) provides a solid base. HTML output already includes XSS hardening via `escape_html()`. M4 now extends this foundation through a shared React frontend under `ui/`, starting with a local JSON artifact loader plus triage dashboard that runs directly in the browser.

## Scope

### Phase UI-2: Static Interactive HTML Reports
1. Self-contained HTML files with embedded minified JavaScript
2. Collapsible sections for leak details, object trees, reference chains
3. Client-side search/filter within the report
4. Sortable tables for histograms and leak lists
5. Provenance badges with color-coded severity
6. Object graph mini-visualization (embedded D3.js or equivalent)

### Phase UI-3: Browser-First Shared Frontend
7. Shared React frontend under `ui/`, built with Vite and run with Bun
8. Local artifact loader for serialized `AnalyzeResponse` JSON output
9. Current first slice: triage dashboard for summary metrics, provenance, histogram context, graph counts, and leak review
10. Route and state structure that can grow into deeper explorer views without introducing a live local API yet
11. Future browser views: Dominator Tree, Object Inspector, Leak Report drill-down, GC Path Viewer, Query Console

## Non-scope

- Desktop application wrapper in this slice - Tauri remains a later packaging path after the browser-first UI proves out
- Hosted web application - security and privacy concerns for heap data
- Core analysis logic changes (M3)
- AI/LLM features (M5)
- New analysis algorithms
- Replacing the current artifact-driven flow with a live local server API in the first slice

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    BROWSER-FIRST UI                         │
│                                                              │
│  Browser                                                     │
│     │                                                        │
│     ├──> Artifact Loader -> reads serialized AnalyzeResponse │
│     │                       JSON from local disk             │
│     │                                                        │
│     └──> Triage Dashboard -> summary / provenance /          │
│                              histogram / graph counts /      │
│                              leak review                     │
└──────────────────────────────────────────────────────────────┘
           │
┌──────────┼───────────────────────────────────────────────────┐
│          ▼     FRONTEND WORKSPACE                            │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  ui/                                                  │  │
│  │  React + Vite                                         │  │
│  │  Bun-supported scripts: test / build / lint / dev     │  │
│  │  Shared routes + state for later deeper explorer work │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
           │
┌──────────┼───────────────────────────────────────────────────┐
│          ▼     CORE LAYER (unchanged for this slice)         │
│  cli/ emits JSON artifacts | core/ defines AnalyzeResponse   │
└──────────────────────────────────────────────────────────────┘
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `core/src/report/renderer.rs` | Enhanced | Interactive HTML report generation with embedded JS |
| `ui/` | New | Shared browser-first React frontend for the M4 first slice |
| `ui/src/features/artifact-loader/` | New | Local `AnalyzeResponse` JSON artifact intake |
| `ui/src/features/dashboard/` | New | Triage dashboard screens and components |
| `cli/src/main.rs` | Reused | Existing CLI/report output remains the way artifacts are produced |
| `core/src/lib.rs` and shared types | Reused | Existing `AnalyzeResponse` contract feeds the frontend artifact loader |

**Workspace decision:** The first slice keeps the UI in a separate `ui/` workspace directory so browser concerns do not add frontend dependencies to the Rust crates. A later Tauri wrapper can package this same frontend without changing the current browser-first runtime model.

## API/CLI/Reporting Impact

### Enhanced Existing Command
- `mnemosyne analyze --format html` - continues to be the HTML reporting path

### Frontend Artifact Contract
- Current M4 first slice consumes serialized `AnalyzeResponse` JSON artifacts produced by the existing CLI/reporting surfaces
- No live local browser API is part of the shipped first slice
- Future deeper views can either keep extending the artifact model or add a local API later if evidence shows the browser-only artifact path is insufficient

## Data Model Changes

### Current first-slice contract
- Existing `AnalyzeResponse` JSON is the browser data source
- Frontend-local view models derive summary cards, dashboard tables, provenance badges, and leak triage state from that artifact

### Existing types remain unchanged
All core analysis types remain owned by the Rust crates. The current UI slice consumes the serialized output contract rather than introducing a parallel API-specific schema.

## Validation/Testing Strategy

### Functional Tests
- Interactive HTML report renders correctly in Chrome, Firefox, Safari
- Collapsible sections open/close properly
- Search/filter produces correct results
- Sortable tables sort by all columns
- Provenance badges display with correct colors

### Frontend Tests
- Artifact loader accepts valid `AnalyzeResponse` JSON and rejects malformed input with readable feedback
- Dashboard renders summary metrics, provenance, graph counts, histogram context, and leak triage tables from the loaded artifact
- Layout remains usable on narrow screens for loader and dashboard views
- Route/state resets correctly when a new artifact is loaded

### Performance Tests
- Browser remains responsive for the current triage dashboard artifact size targets
- Artifact parsing and client-side state updates remain acceptable on representative `AnalyzeResponse` payloads

### Security Tests
- XSS hardening preserved in interactive HTML and dashboard rendering
- No artifact content is uploaded off-box in the browser-first slice
- Error messages avoid dumping raw sensitive artifact content back into the UI

## Rollout/Implementation Phases

### Phase 1 - Enhanced HTML Reports (effort: Large)
1. Upgrade HTML report template with embedded minified JS
2. Collapsible section toggle for leak details
3. Sortable table headers for histograms and leak lists
4. Client-side search/filter box
5. Provenance badge styling (color + icon)

### Phase 2 - Browser-First Dashboard Foundation (effort: Large)
6. Stand up shared React frontend under `ui/`
7. Support Bun-based test/build/lint workflow
8. Implement local JSON artifact loader for serialized `AnalyzeResponse`
9. Ship the triage dashboard first slice

### Phase 3 - Interactive Views (effort: XL)
10. Extend beyond the shipped dashboard into dominator and object-inspection views
11. Add deeper cross-view navigation and drill-down
12. Decide whether artifact-only data flow remains sufficient or whether a local API is justified

### Phase 4 - Advanced Views (effort: XL)
13. Leak report view with drill-down
14. GC path visualizer (interactive path diagram)
15. Query console (if M3 OQL exists)
16. Optional Tauri wrapper if packaging evidence supports it

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Browser-only artifact flow may not cover deeper explorer needs | Medium | Medium | Reassess after dashboard slices land; add a local API only if justified |
| Browser performance with large serialized artifacts | Medium | High | Keep first slice triage-focused; add pagination/virtualization as views deepen |
| Contract drift between Rust `AnalyzeResponse` and frontend assumptions | Medium | High | Keep shared types/tests/docs synchronized and verify with frontend build/tests |
| Bun workflow mismatch for contributors | Low | Medium | Document Bun as the supported package manager/script runner for `ui/` |

### Open Questions
1. How far can the artifact-driven browser model go before a live local API is necessary?
2. Which deeper routes should land next after the shipped loader + triage dashboard?
3. When packaging is justified, should the first wrapper be Tauri or should the browser build remain standalone longer?
4. Which visualization libraries are actually needed once graph-heavy views land?

### Dependencies
- **Blocked by:** M1.5 (real-world data), M3 (analysis features to display)
- **Blocks:** M6 (community needs a polished tool to evangelize)
