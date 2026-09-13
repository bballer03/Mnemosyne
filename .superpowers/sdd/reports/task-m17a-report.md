# Task M17.A Report — Inspector and multi-path Tauri commands

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-14  
**Status:** ✅ Complete (Slice 17.A only)

## Summary

Wired native Tauri commands and TypeScript bridge injection for M14's `inspectObject` and `findAllGcPaths` host methods. Both delegate to existing `mnemosyne_core` APIs against the loaded heap session graph; no new analyzers.

## Changes

| Area | Files |
|---|---|
| Tauri commands | `tauri/src/commands.rs`, `tauri/src/main.rs` |
| Bridge injection | `tauri/src/bridge.ts` |
| Testable session wrappers | `tauri/session-ops/` (`mnemosyne-desktop-session` crate) |
| Design doc | `docs/design/milestone-17-desktop-guided-ux-bridges.md` |

### Commands

- `inspect_object(objectId, retainFieldData?)` → `core::analysis::inspect_object` + dominator tree; unknown id → `inspect_object_id_not_found: ...` (MCP/CLI shape).
- `find_all_gc_paths(objectId, maxPaths?)` → `core::graph::find_all_gc_paths_in_graph`; preserves `all_paths`, `truncated`, and core unknown-id errors.

### Bridge

- `__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__.inspectObject` added; existing `queryHeap` / `getReferences` / `getReferrers` unchanged.
- `__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__.findAllGcPaths` added; existing leak-workspace methods unchanged.

## Validation

| Check | Result |
|---|---|
| `cargo test --features test-fixtures` in `tauri/session-ops/` | ✅ 5/5 passed |
| Full `mnemosyne-desktop` Tauri build | ⏭ Skipped — WSL host lacks WebKit/GTK (`javascriptcoregtk-4.1`, `webkit2gtk-4.1`) |
| UI tests (`bun run test`) | ⏭ Skipped — `bun` not installed in agent environment |
| GitNexus impact / detect_changes | ⏭ MCP tools unavailable in session |

## Smoke path (manual)

1. Launch desktop app; `load_heap` on a known HPROF.
2. Open Object Inspector — dominator parent/children chips should render via `inspectObject`.
3. Open Leak GC Path with multi-path selector — enumerated paths + truncation notice via `findAllGcPaths`.

## Out of scope (unchanged)

Slices 17.B (`diffObjects`), 17.C (workflow bridge), 17.D (packaged evidence).

## Commit

See git log on `sync/m15g-m16bcd` for the M17.A conventional commit.

---

## Terra review fixes (2026-09-14)

### Findings addressed

1. **`retainFieldData` never populated fields** — `load_heap` parsed with default `retain_field_data: false`, so `inspect_object(..., retainFieldData=true)` could never surface instance fields. **Fix:** when `retain_field_data` is requested and the session graph has no retained field bytes, `inspect_object` re-parses the loaded heap with `ParseOptions { retain_field_data: true }` and caches the result in `HeapSession.field_data_graph` (lean session graph unchanged for other commands). Matches CLI/MCP semantics.

2. **Malformed object ids returned ad-hoc errors** — `parse_object_id` surfaced `"Invalid object id '…'"` on inspect paths. **Fix:** `inspect_object_for_session` uses `parse_inspect_object_id` (mirroring MCP/CLI) and maps parse failures to `inspect_object_id_not_found: object id '…' was not found in heap dump '…'`.

### Files changed

- `tauri/session-ops/src/lib.rs` — `graph_has_field_data`, `parse_inspect_object_id`, inspect not-found semantics, tests
- `tauri/src/state.rs` — `field_data_graph` cache
- `tauri/src/commands.rs` — lazy reparse + cache on `inspect_object`
- `docs/design/milestone-17-desktop-guided-ux-bridges.md` — Terra fix notes

### Validation

| Check | Result |
|---|---|
| `cargo test --features test-fixtures` in `tauri/session-ops/` | ✅ 7/7 passed |
