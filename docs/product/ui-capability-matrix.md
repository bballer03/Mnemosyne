# UI capability matrix (M20)

Status as of 2026-09-14 on `sync/m15g-m16bcd`. Legend: **shipped** | **partial** | **planned** | **deferred**.

Evidence note: [docs/evidence/m20-ui-workbench.md](../evidence/m20-ui-workbench.md). No shipped heap-analysis capability below is left unclassified.

| Surface | Backend | UI | Notes |
| --- | --- | --- | --- |
| Open heap dump (`.hprof`/`.bin`) | Tauri dialog + opaque `sourceId` | **shipped** | Absolute path never enters React (`9af0f47`) |
| Desktop analyze → artifact | `analyze_heap_capturing_graph` | **shipped** | Incident defaults; path redacted in JSON (`f6c627c`) |
| JSON artifact import | parser | **shipped** | Browser path; remains available beside desktop open |
| Histogram / regroup | core + session-ops | **shipped** | Live regroup on desktop |
| Dominators / Inspector / OQL / Threads | core + bridges | **shipped** | Power routes |
| Strings / collections / top instances / unreachable | artifact sections | **shipped** | Detail panels + rail anchors (`6dbabb6`) |
| Duplicate arrays / plugins / classloaders | artifact sections | **shipped** | Loaded vs unique columns split; breadcrumbs (`5af03f9`) |
| Compare / leak workspace | bridges | **shipped** | |
| Snapshots list/save/remove | `list_snapshots` / `save_snapshot` / `remove_snapshot` | **shipped** | SHA-256 key-only remove; heap column shows basename (`2dbe4f8`, `9ca7a1b`) |
| Snapshot open (load graph into session from key) | store load | **deferred** | 20.G explicit deferral; list/save/remove only |
| Policies (`ci_check`) | Tauri `run_ci_check` | **shipped** | Inline TOML; skip ≠ pass called out (`2dbe4f8`, `9ca7a1b`); native/MCP parity tests still thin |
| Flamegraphs | Tauri `generate_desktop_flamegraph` | **shipped** | SVG via blob URL; root selector (`2dbe4f8`); 16 MiB / overview-unavailability UI parity tests still thin (rely on core/MCP) |
| Portable Windows zip | release CI script | **partial** | Labeled WebView2 prerequisite; clean-image unzip evidence **not proven** (M21) |
| Bounded OQL MAT corpus | core query tests | **partial** | M22.A first slice: MAT-referenced catalog + thin runner; only mat-referenced cases close rows |
| Multi-class `FROM` | core query | **shipped** | M22.B — max 8 patterns, ID dedup (`4d477c6`) |
| Multi-hop `OBJECTS` | core query | **shipped** | M22.C — 1–3 hops; reject 4+ (`e3d18a0`) |
| Packaged GUI smoke (WSL) | Tauri bundle | **deferred** | Command/unit/browser evidence only here; launch → M21 native hosts |
| AI-first surfaces | MCP/chat | **planned** | M23 last |

## Honesty bar

- Install goal: unzip → double-click → app opens (no JVM). Windows portable zip is primary once CI publishes it; WebView2 remains a documented prerequisite until proven on a clean image.
- This WSL host yields **command-layer / browser-fallback** evidence only. Do not treat green unit tests as packaged GUI launch proof.
- Screenshot gallery per workbench family was **not** captured in the 20.H closeout; visual proof is absent, not implied.
