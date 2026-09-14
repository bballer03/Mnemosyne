# UI capability matrix (M20–M23)

Status as of 2026-09-14 on `sync/m15g-m16bcd`. Legend: **shipped** | **partial** | **planned** | **deferred**.

Evidence notes: [docs/evidence/m20-ui-workbench.md](../evidence/m20-ui-workbench.md), [docs/evidence/m23-guided-investigation.md](../evidence/m23-guided-investigation.md). No shipped heap-analysis capability below is left unclassified.

| Surface | Backend | UI | Notes |
| --- | --- | --- | --- |
| Open heap dump (`.hprof`/`.bin`) | Tauri dialog + opaque `sourceId` | **shipped** | Absolute path never enters React (`9af0f47`) |
| Desktop analyze → artifact | `analyze_heap_capturing_graph` | **shipped** | Incident defaults; path redacted in JSON (`f6c627c`) |
| JSON artifact import | parser | **shipped** | Browser path; remains available beside desktop open |
| Histogram / regroup | core + session-ops | **shipped** | Live flat regroup on desktop; superclass stays flat unless payload carries explicit parent links |
| Superclass collapsible tree | (no parent contract on regroup) | **open** | M22.D: expand/collapse only when returned entries include resolvable `parentKey`; current regroup/artifact payloads do not — no invented ancestry |
| Dominators / Inspector / OQL / Threads | core + bridges | **shipped** | Power routes |
| Strings / collections / top instances / unreachable | artifact sections | **shipped** | Detail panels + rail anchors (`6dbabb6`) |
| Duplicate arrays / plugins / classloaders | artifact sections + CLI | **shipped** | UI Loaded vs Unique (`5af03f9`); CLI Unique Classes column from existing `unique_class_count` (M22.D) |
| Compare / leak workspace | bridges | **shipped** | |
| Snapshots list/save/remove/open | `list_snapshots` / `save_snapshot` / `remove_snapshot` / `open_snapshot` | **shipped** | Key-only remove; basename in UI; open installs graph + opaque `sourceId` (`2dbe4f8`, `352b3d5`) |
| Policies (`ci_check`) | Tauri `run_ci_check` | **shipped** | Inline TOML; skip ≠ pass; `evaluation_complete` (`2dbe4f8`, `9ca7a1b`); native/MCP parity tests still thin |
| Flamegraphs | Tauri `generate_desktop_flamegraph` | **shipped** | SVG via blob URL; root selector (`2dbe4f8`); 16 MiB / overview-unavailability UI parity tests still thin (rely on core/MCP) |
| Portable Windows zip | release CI script | **partial** | Labeled WebView2 prerequisite; clean-image unzip evidence **not proven** (M21) |
| Desktop asset name/integrity verify | `verify_*` + `generate_sha256sums` + `inspect_*` + `probe_*` + `normalize_*` | **partial** | Unsigned desktop SHA256SUMS + secret inspect + Info.plist/ELF structural probe (warn); **not** strict name gate or launch proof |
| macOS app zip / Linux AppImage names | `normalize_*` + `probe_*` + `release.yml` | **partial** | Frozen names + ditto zip + ELF/Info.plist probes; native launch **not proven** |
| Bounded OQL MAT corpus | core query tests | **partial** | Handbook-linked `documentation-referenced` cases only; **no** recorded MAT golden results yet — cannot close equivalency matrix rows |
| Multi-class `FROM` | core query | **shipped** | M22.B — max 8 patterns, ID dedup (`4d477c6`) |
| Multi-hop `OBJECTS` | core query | **shipped** | M22.C — 1–3 hops; reject 4+; Terra cycle/hop fixes (`e3d18a0`, `d06f152`) |
| `SELECT DISTINCT OBJECTS` | core query | **shipped** | Bounded collapse by target id (`2b4f33c`) |
| Packaged GUI smoke (WSL) | Tauri bundle | **deferred** | Command/unit/browser evidence only here; launch → M21 native hosts |
| Investigation session (`/assistant`) | rules + optional host `chatSession` | **shipped** | Rules default; fact/AI separation; deep links (`f3ad203`, `3ab1ef5`, `22ff57b`, `d391953`) |
| Desktop AI session adapters | Tauri over MCP AI session | **shipped** | create/resume/get/close/chat; 12/32 bounds; rules fallback — **not** live-provider proof on WSL |
| Workflow get/close/resume | Tauri + WorkflowCard | **shipped** | MCP-parity get/close; resume UI (`532df7c`); basename projection on get (`d391953`) |
| AI-first guidance vs MAT analysis | advisory only | **partial** | Guidance never claims MAT-equivalent replacement; see M23 evidence NOT-proven table |

## Honesty bar

- Install goal: unzip → double-click → app opens (no JVM). Windows portable zip is primary once CI publishes it; WebView2 remains a documented prerequisite until proven on a clean image.
- This WSL host yields **command-layer / browser-fallback** evidence only. Do not treat green unit tests as packaged GUI launch proof or as a live AI provider round-trip.
- Screenshot gallery per workbench family was **not** captured in the 20.H closeout; visual proof is absent, not implied.
- M23 assistant Ask is **advisory** with explicit provenance; deterministic power tools remain the analysis path.
