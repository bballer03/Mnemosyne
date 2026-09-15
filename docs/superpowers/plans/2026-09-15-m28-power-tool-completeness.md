# M28 Power-Tool Completeness Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mature the existing query, analyzer, policy, visualization, and export surfaces without widening the bounded backend scope.

**Architecture:** Treat each power tool as an on-demand projection of the active investigation. Reuse shipped query/analyzer/flamegraph/policy APIs and M26 operation envelopes; preserve lean open and provenance.

**Tech Stack:** React, Tauri, `core::query`, existing analysis modules, policy engine, flamegraph/report renderers.

**Roadmap:** [M28 — Power-tool completeness](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m28--power-tool-completeness)

## Global Constraints

- Keep named OQL deferrals explicit: `eval(...)`, arbitrary-depth nesting, and other unscheduled grammar.
- Strings/collections/arrays/threads/referrers/classloaders remain opt-in with memory/cost disclosure.
- Reuse `AnalyzeRequest` flags and existing analyzer reports; no duplicate frontend analyzer.
- Preserve HTML escaping and provenance on every export.
- Use focused component/client tests; never mount the production route tree in WSL tests.

---

## File map

- OQL: `ui/src/features/heap-explorer/HeapQueryConsolePage.tsx`, `components/QueryConsolePanel.tsx`, `heap-explorer-query-client.ts`, `core/src/query/`.
- Analyzers: `ui/src/features/artifact-explorer/components/*Panel.tsx`, `tauri/src/commands.rs`, `tauri/session-ops/src/lib.rs`.
- Policy/compare controls: `ui/src/features/policy/`, `ui/src/features/comparison/`.
- Visualization/export: `ui/src/features/flamegraph/`, `core/src/report/`, existing desktop flamegraph command.

### M28.A — OQL workbench

- [ ] Expand into a full plan for bounded history, vetted examples, structured error locations, and object-ID navigation.
- [ ] Keep query text out of logs/progress events and stale-result guard every response.
- [ ] Display supported syntax and named deferrals beside the editor.

### M28.B — On-demand analyzers

- [ ] Plan one host enrichment request over existing analyzer flags with field-data cost preview.
- [ ] Commit results only to the matching revision/op and label partial/fallback/unavailable sections.
- [ ] Add policy baseline picker, compare identity strategy controls, and recommendation content rather than count-only cards.

### M28.C — Visualization and export

- [ ] Expose existing flamegraph formats and report exports from the workspace.
- [ ] Sanitize filenames/content, preserve provenance/mode labels, and never inject untrusted HTML.
- [ ] Add focused export contract tests and honest native/download evidence.
