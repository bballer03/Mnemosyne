# Milestone 17 — Desktop Guided UX Bridge Completion

> **Status:** 🟡 In progress — Slice 17.A shipped; Slices 17.B–17.D pending.
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M17
> **Predecessors:** M14 (UI bridge contracts), M16 (desktop packaging)
> **Last updated:** 2026-09-14

---

## 1. Objective

Wire the M14 host bridge methods the React UI already calls into native Tauri commands backed by existing `mnemosyne_core` behavior — no new analyzers, no bridge API redesign.

## 2. Required desktop bridge interface

See [post-M16 product plan excerpt](../../.superpowers/sdd/briefs/task-m17-plan-excerpt.md) for the exact camelCase host signatures and acceptance gates.

## 3. Slice status

| Slice | Scope | Status |
|---|---|---|
| **17.A** | `inspectObject`, `findAllGcPaths` | ✅ Shipped |
| **17.B** | `diffObjects` (comparison bridge) | 🔲 Pending |
| **17.C** | Workflow bridge (`describeWorkflow`, `startWorkflow`, `nextStep`, `listSnapshots`) | 🔲 Pending |
| **17.D** | Packaged-desktop integration evidence | 🔲 Pending |

### Slice 17.A — Inspector and multi-path commands

**Files:** `tauri/src/commands.rs`, `tauri/src/bridge.ts`, `tauri/src/main.rs`, `tauri/session-ops/`

- [x] `inspect_object` Tauri command maps to `core::analysis::inspect_object` against the loaded session graph, with optional `retain_field_data` and `inspect_object_id_not_found` errors matching MCP/CLI.
- [x] `find_all_gc_paths` Tauri command maps to `core::graph::find_all_gc_paths_in_graph` with bounded `max_paths`, preserving `all_paths` and `truncated` on the existing `GcPathResult` wire shape.
- [x] `bridge.ts` injects `inspectObject` on `__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__` and `findAllGcPaths` on `__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__` without removing pre-M14 methods.
- [x] Unit tests cover session wrappers with `core` synthetic fixtures (`cargo test --features test-fixtures` in `tauri/session-ops/`).

**Smoke path (manual):** Launch the desktop app, `load_heap` on a known fixture, open Object Inspector and Leak GC Path pages — dominator chips and multi-path enumeration should render instead of unavailable states.

#### Slice 17.A implementation report (Terra review fixes)

**Findings addressed:**

1. **`retainFieldData` never populated fields** — `load_heap` parsed with default `retain_field_data: false`, so `inspect_object(..., retainFieldData=true)` could never surface instance fields. Fix: when `retain_field_data` is requested and the session graph has no retained field bytes, `inspect_object` re-parses the loaded heap with `ParseOptions { retain_field_data: true }` and caches the result in `HeapSession.field_data_graph` (lean session graph unchanged for other commands). Matches CLI/MCP semantics: caller flag gates the fields section; graph must actually carry field bytes.

2. **Malformed object ids returned ad-hoc errors** — `parse_object_id` surfaced `"Invalid object id '…'"` on inspect paths. Fix: `inspect_object_for_session` now uses `parse_inspect_object_id` (mirroring MCP/CLI) and maps parse failures to `inspect_object_id_not_found: object id '…' was not found in heap dump '…'`.

**Verification:** `cargo test --features test-fixtures` in `tauri/session-ops/` — 7/7 pass (includes new retain-field-data and malformed-id cases).

## 4. Out of scope (M17)

- Slices 17.B–17.D until scheduled.
- `getWorkflow` / `closeWorkflow` (no current `ui/src` callers).
- New analysis capability, signing credentials, auto-update.
