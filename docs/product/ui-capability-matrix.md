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
| Snapshots list | `list_snapshots` | **partial** | List UI only; save/remove planned |
| Policies (`ci_check`) | MCP/core | **planned** | Route stub → M20.F |
| Flamegraphs | core managed artifacts | **planned** | Route stub → M20.G |
| Portable Windows zip | release CI script | **partial** | Labeled WebView2 prerequisite |
| Bounded OQL MAT corpus | — | **planned** | M22 after UI/install |
| AI-first surfaces | MCP/chat | **planned** | M23 last |

Install goal: unzip → double-click → app opens (no JVM). Windows portable zip is primary once CI publishes it; WebView2 remains a documented prerequisite until proven on a clean image.
