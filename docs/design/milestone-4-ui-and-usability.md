# Milestone 4 — UI & Usability

> **Status:** ⚬ Pending  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Make Mnemosyne visually accessible to developers who prefer graphical exploration of heap data. Deliver interactive HTML reports and a local web UI that leverage the analysis features built in M1–M3, dramatically widening the user base beyond CLI-only users.

## Context

Heap analysis is fundamentally a visual and exploratory task. Developers investigating memory leaks navigate dominator trees, inspect reference chains, compare object counts, and drill into specific classes — activities that map poorly to sequential text output. Eclipse MAT's success is inseparable from its GUI-based tree explorers and table views. For Mnemosyne to compete for adoption, it must offer interactive exploration while preserving the CLI-first, automation-friendly foundation.

The current reporting layer (Text, Markdown, HTML, TOON, JSON) provides a solid base. HTML output already includes XSS hardening via `escape_html()`. M4 extends this foundation into interactive experiences.

## Scope

### Phase UI-2: Static Interactive HTML Reports
1. Self-contained HTML files with embedded minified JavaScript
2. Collapsible sections for leak details, object trees, reference chains
3. Client-side search/filter within the report
4. Sortable tables for histograms and leak lists
5. Provenance badges with color-coded severity
6. Object graph mini-visualization (embedded D3.js or equivalent)

### Phase UI-3: Lightweight Web UI
7. Local web server using `axum` (Tokio ecosystem)
8. Upload or select heap dump file
9. Real-time parsing progress via WebSocket or SSE
10. Interactive dominator tree browser with drill-down
11. Object graph explorer: click through reference chains
12. Histogram explorer with group-by controls
13. Query interface (OQL from M3)
14. Key screens: Dashboard, Dominator Tree, Object Inspector, Leak Report, GC Path Viewer, Query Console

## Non-scope

- Desktop application (Tauri/Electron) — evaluated but deferred to M6+
- Hosted web application — security and privacy concerns for heap data
- Core analysis logic changes (M3)
- AI/LLM features (M5)
- New analysis algorithms
- Mobile-responsive design (not a use case for heap analysis)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    USER INTERFACE                         │
│                                                         │
│  Browser ──── http://localhost:PORT ─────→ axum server  │
│     │                                          │        │
│     │    ┌──────────────┐                      │        │
│     ├───→│ Dashboard    │←── Core API ─────────┤        │
│     │    ├──────────────┤                      │        │
│     ├───→│ Dominator    │←── DominatorTree ────┤        │
│     │    │  Tree Browser│                      │        │
│     │    ├──────────────┤                      │        │
│     ├───→│ Object       │←── ObjectGraph ──────┤        │
│     │    │  Inspector   │    Navigation API    │        │
│     │    ├──────────────┤                      │        │
│     ├───→│ Leak Report  │←── detect_leaks() ──┤        │
│     │    ├──────────────┤                      │        │
│     ├───→│ GC Path      │←── find_gc_path() ──┤        │
│     │    │  Viewer      │                      │        │
│     │    ├──────────────┤                      │        │
│     └───→│ Query        │←── query_heap() ────┘        │
│          │  Console     │    (M3 OQL)                   │
│          └──────────────┘                               │
└─────────────────────────────────────────────────────────┘
           │
┌──────────┼──────────────────────────────────────────────┐
│          ▼     WEB SERVER LAYER (new)                   │
│  ┌──────────────────────────────────────────────────┐   │
│  │  axum routes                                     │   │
│  │  /api/summary     → HeapSummary JSON             │   │
│  │  /api/dominators  → DominatorTree (paginated)    │   │
│  │  /api/objects/:id → ObjectGraph navigation        │   │
│  │  /api/leaks       → detect_leaks() JSON          │   │
│  │  /api/gc-path     → find_gc_path() JSON          │   │
│  │  /api/query       → OQL execution                │   │
│  │  /ws/progress     → parsing progress stream      │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  Static assets: HTML templates, embedded JS/CSS         │
└─────────────────────────────────────────────────────────┘
           │
┌──────────┼──────────────────────────────────────────────┐
│          ▼     CORE LAYER (unchanged)                   │
│  hprof/ │ graph/ │ analysis/ │ report/ │ ...            │
└─────────────────────────────────────────────────────────┘
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `core/src/report/renderer.rs` | Enhanced | Interactive HTML report generation with embedded JS |
| `core/src/web/mod.rs` | New | Web server module (or separate `web/` crate) |
| `core/src/web/routes.rs` | New | axum route handlers |
| `core/src/web/templates/` | New | HTML templates for web UI screens |
| `core/src/web/static/` | New | Embedded JS/CSS assets |
| `cli/src/main.rs` | Updated | `mnemosyne serve --web` command |
| `cli/Cargo.toml` | Updated | axum dependency |
| `core/Cargo.toml` | Updated | axum, tower, serde_json dependencies |

**Crate structure decision:** The web UI may warrant a separate `web/` crate in the workspace to isolate the axum/frontend dependency from the core analysis library. This prevents users who only need the core library from pulling in web framework dependencies.

## API/CLI/Reporting Impact

### New CLI Command
- `mnemosyne serve --web [--port PORT]` — start local web server and open browser

### Enhanced Existing Command
- `mnemosyne analyze --format html` — now produces interactive HTML (collapsible, searchable, sortable) instead of static HTML

### New REST API (web server only)
- `GET /api/summary` — heap summary JSON
- `GET /api/dominators?page=N&size=M` — paginated dominator tree
- `GET /api/objects/:id` — single object with fields, references, referrers
- `GET /api/leaks` — leak suspects JSON
- `GET /api/gc-path?object_id=ID&max_depth=N` — GC path JSON
- `POST /api/query` — OQL query execution (if M3 OQL exists)
- `WS /ws/progress` — parsing progress WebSocket stream

## Data Model Changes

### New Types
- `WebConfig` — port, bind address, auto-open browser, progress streaming
- `DominatorPage` — paginated dominator tree response for large heaps
- `ObjectDetail` — expanded view of a single object (fields, references, referrers, class info, size)
- `ProgressEvent` — parsing progress for WebSocket streaming (phase, progress_pct, message)

### Existing types remain unchanged
All core types (`ObjectGraph`, `DominatorTree`, `LeakInsight`, etc.) are consumed as-is through JSON serialization.

## Validation/Testing Strategy

### Functional Tests
- Interactive HTML report renders correctly in Chrome, Firefox, Safari
- Collapsible sections open/close properly
- Search/filter produces correct results
- Sortable tables sort by all columns
- Provenance badges display with correct colors

### Web Server Tests
- Each API endpoint returns correct JSON
- Paginated endpoints handle edge cases (empty, single page, many pages)
- WebSocket progress events fire during parsing
- Server handles concurrent requests
- Server gracefully handles invalid object IDs

### Performance Tests
- Browser remains responsive with 100K+ objects (virtual scrolling)
- Dominator tree lazy-loading works for deep trees
- Web server response time <100ms for API calls with pre-parsed data

### Security Tests
- XSS hardening preserved in interactive HTML
- Web server binds only to localhost by default
- No sensitive heap data leaked in error responses
- CSP headers on HTML responses

## Rollout/Implementation Phases

### Phase 1 — Enhanced HTML Reports (effort: Large)
1. Upgrade HTML report template with embedded minified JS
2. Collapsible section toggle for leak details
3. Sortable table headers for histograms and leak lists
4. Client-side search/filter box
5. Provenance badge styling (color + icon)

### Phase 2 — Web Server Foundation (effort: Large)
6. Create web server module (axum, tower)
7. `mnemosyne serve --web` command
8. API routes for summary, dominators, leaks
9. Static asset serving (HTML templates, JS, CSS)

### Phase 3 — Interactive Views (effort: XL)
10. Dashboard screen (summary metrics, top consumers, leak count)
11. Dominator tree browser (expand/collapse, retained size bars)
12. Object inspector (click object → fields, references, referrers)

### Phase 4 — Advanced Views (effort: XL)
13. Leak report view with drill-down
14. GC path visualizer (interactive path diagram)
15. Query console (if M3 OQL exists)
16. Parsing progress via WebSocket

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Frontend complexity may exceed Rust-ecosystem tooling | Medium | High | Consider htmx for progressive enhancement instead of full SPA |
| axum dependency adds significant compile time | Medium | Medium | Isolate in separate crate; feature-gate |
| Browser performance with 1M+ objects | High | High | Virtual scrolling, server-side pagination, lazy tree loading |
| Embedding JS in Rust binary bloats binary size | Medium | Low | Minify and compress; consider lazy download |
| Security: web server opens local port | Low | Medium | Bind localhost only; document security model |

### Open Questions
1. htmx vs React SPA? (Recommendation: start with htmx for simplicity, upgrade if needed)
2. Separate `web/` crate or module in core? (Recommendation: separate crate to isolate deps)
3. Should the web UI support remote access? (Recommendation: no, localhost-only for security)
4. D3.js for graph visualization or simpler alternatives? (Recommendation: evaluate vis.js or cytoscape.js)

### Dependencies
- **Blocked by:** M1.5 (real-world data), M3 (analysis features to display)
- **Blocks:** M6 (community needs a polished tool to evangelize)
