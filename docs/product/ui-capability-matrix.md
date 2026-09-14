# UI capability matrix (M20)

Status as of 2026-09-14 on `sync/m15g-m16bcd`. Legend: **shipped** | **partial** | **planned**.

| Surface | Backend | UI | Notes |
| --- | --- | --- | --- |
| Open heap dump (`.hprof`/`.bin`) | Tauri dialog + opaque `sourceId` | **shipped** | Absolute path never enters React |
| Desktop analyze → artifact | `analyze_heap_capturing_graph` | **shipped** | Incident defaults; path redacted in JSON |
| JSON artifact import | parser | **shipped** | Browser path |
| Histogram / regroup | core + session-ops | **shipped** | Live regroup on desktop |
| Dominators / Inspector / OQL / Threads | core + bridges | **shipped** | Power routes |
| Strings / collections / top instances / unreachable | artifact sections | **shipped** | Detail panels + rail anchors |
| Duplicate arrays / plugins / classloaders | artifact sections | **shipped** | Loaded vs unique columns split |
| Compare / leak workspace | bridges | **shipped** | |
| Snapshots list/save/remove | `list_snapshots` / `save_snapshot` / `remove_snapshot` | **shipped** | SHA-256 key-only remove; heap column shows basename |
| Policies (`ci_check`) | Tauri `run_ci_check` | **shipped** | Inline TOML; skip ≠ pass called out |
| Flamegraphs | Tauri `generate_desktop_flamegraph` | **shipped** | SVG via blob URL; root selector |
| Portable Windows zip | release CI script | **partial** | Labeled WebView2 prerequisite |
| Bounded OQL MAT corpus | — | **planned** | M22.A; MAT-referenced cases only close matrix rows |
| Multi-class `FROM` | core query | **shipped** | M22.B — max 8 patterns, ID dedup (`4d477c6`) |
| Multi-hop `OBJECTS` | core query | **shipped** | M22.C — 1–3 hops; reject 4+ (`e3d18a0`) |
| AI-first surfaces | MCP/chat | **planned** | M23 last |

Install goal: unzip → double-click → app opens (no JVM). Windows portable zip is primary once CI publishes it; WebView2 remains a documented prerequisite until proven on a clean image.
