# M29 Guided Investigation Continuity Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind findings, workflows, and advisory Assistant turns to the active deterministic investigation without rewriting measured facts.

**Architecture:** Extend the existing M24 `FindingsAdvisoryPane`, workflow bridge, and Assistant session adapters. Findings retain immutable source facts and stable deep-link targets; workflow/AI state stores only workspace-scoped identity and provenance.

**Tech Stack:** React, Zustand, existing `core::workflow`, MCP/Tauri workflow and AI-session adapters.

**Roadmap:** [M29 — Guided investigation continuity](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m29--guided-investigation-continuity)

## Global Constraints

- AI remains collapsible, advisory, provenance-labelled, and separate from measured facts.
- Every finding deep-links through stable `classKey`/`objectId`/`leakId`; no row-index links.
- Workflow start/resume/close is workspace-scoped; users do not paste IDs.
- Never send or persist absolute paths, raw field values, API keys, or unbounded history.
- M26 stale-result/cancellation rules apply to workflow and Assistant calls.

---

## File map

- Findings: `ui/src/features/investigation/FindingsAdvisoryPane.tsx`, `investigation-store.ts`.
- Workflows: `ui/src/features/workflow-landing/`, `ui/src/host/tauri-bridge.ts`, `tauri/session-ops/src/lib.rs`.
- Assistant: `ui/src/features/assistant/InvestigationAssistantPage.tsx`, `assistant-bridge-client.ts`.

### M29.A — Unified findings queue

- [ ] Expand a full plan for leak, classloader, collection/string/array waste, and policy finding adapters.
- [ ] Define immutable fact payloads plus separate user status (`open`, `resolved`, `deferred`).
- [ ] Test every finding’s deterministic deep-link and stale workspace rejection.

### M29.B — Workflow binding

- [ ] Bind one active workflow ID/kind/current step to workspace revision.
- [ ] Auto-close or detach on workspace Close/replace; recover persisted workflows only when compatible.
- [ ] Surface current step in workbench chrome and Assistant without exposing heap paths.

### M29.C — Contextual Assistant

- [ ] Seed advisory context from current stable selection and measured findings only.
- [ ] Preserve 12-turn default/32-turn hard history bounds and provenance on every turn.
- [ ] Keep deterministic panes usable when provider/bridge is unavailable.
- [ ] Record focused rules-mode evidence and mark live-provider/native behavior `NOT PROVEN` unless run.
