# UI capability matrix (M20–M28)

Status as of 2026-09-15 on `feature/mat-maturity-m25-plus`. Legend: **shipped** | **partial** | **planned** | **deferred**.

Evidence notes: [docs/evidence/m20-ui-workbench.md](../evidence/m20-ui-workbench.md), [docs/evidence/m23-guided-investigation.md](../evidence/m23-guided-investigation.md), [docs/evidence/m25-mat-loop-depth.md](../evidence/m25-mat-loop-depth.md), [docs/evidence/m26-async-platform.md](../evidence/m26-async-platform.md), [docs/evidence/m27-durable-investigations.md](../evidence/m27-durable-investigations.md), [docs/evidence/m28-power-tool-completeness.md](../evidence/m28-power-tool-completeness.md). No shipped heap-analysis capability below is left unclassified.

| Surface | Backend | UI | Notes |
| --- | --- | --- | --- |
| Open heap dump (`.hprof`/`.bin`) | Tauri dialog + opaque `sourceId` | **shipped** | Absolute path never enters React (`9af0f47`) |
| Desktop analyze → artifact | `analyze_heap_capturing_graph` | **shipped** | Incident defaults; path redacted in JSON (`f6c627c`) |
| JSON artifact import | parser | **shipped** | Browser path; remains available beside desktop open |
| Histogram / regroup | core + session-ops | **shipped** | Live flat regroup on desktop; superclass stays flat unless payload carries explicit parent links; M25.A: sortable ≤100-row pages + shared `histogramView` |
| Histogram class → instances → Inspector | `list_class_instances` + investigation store | **shipped** | M25.A: class grouping only; bounded pages (100/200); shared `objectId` handoff (`9d00e2e`) |
| Superclass collapsible tree | (no parent contract on regroup) | **open** | M22.D: expand/collapse only when returned entries include resolvable `parentKey`; current regroup/artifact payloads do not — no invented ancestry |
| Dominators / Inspector / OQL / Threads | core + bridges | **shipped** | Power routes; M28.A adds structured OQL locations, bounded in-memory history/examples, object-ID navigation, named syntax deferrals, and stale-response rejection. |
| Lazy dominator tree | session DominatorTree + `getDominatorChildren` | **shipped** | M25.B: expand-on-demand; retained-% filter; paired clear on unload (`8001f7c`, `104d62b`) |
| Opt-in object fields | `inspect_object` retainFieldData | **shipped** | M25.C: lean inspect by default; CTA + memory-cost disclosure; unavailable vs empty (`9e587ce`) |
| Path/ref synchronized navigation | investigation store | **shipped** | M25.C: refs/referrers/dominators/GC-path nodes set shared `objectId` (`76b5e5f`) |
| Strings / collections / top instances / unreachable | artifact sections + on-demand analysis | **shipped** | Detail panels + rail anchors (`6dbabb6`); M28.B adds one-request opt-in enrichment with field-data reparse/memory-cost disclosure and honest unavailable/provenance outcomes. |
| Duplicate arrays / plugins / classloaders | artifact sections + CLI | **shipped** | UI Loaded vs Unique (`5af03f9`); CLI Unique Classes column from existing `unique_class_count` (M22.D) |
| Compare / leak workspace | existing comparison bridge + investigation store | **shipped** | M27.C shares current/baseline pickers, strategy/top-N/leak controls, match quality, and delta results between persistent chrome and the `/compare` adapter; after-side rows open Inspector. |
| Snapshots list/save/remove/open | `list_snapshots` / `save_snapshot` / `remove_snapshot` / `open_snapshot` | **shipped** | Key-only remove and basename-only React display. M27.B atomically installs cached graph-derived facts, deep mode/capabilities, opaque snapshot identity, and compatible selection (`d6d73ef`). |
| Policies (`ci_check`) | Tauri `run_ci_check` | **shipped** | Inline TOML; skip ≠ pass; `evaluation_complete` (`2dbe4f8`, `9ca7a1b`). M28.B selects object-growth baselines by opaque source ID without replacing the current heap. |
| Flamegraphs | Tauri `generate_desktop_flamegraph` | **shipped** | M28.C exposes SVG/folded-stack/JSON and all existing roots. Only SVG is previewed through a Blob URL; returned markup/text/JSON is never mounted as HTML. |
| Workspace analysis-report exports | cached committed `AnalyzeResponse` + core report renderer | **shipped** | M28.C exposes Text/Markdown/HTML/TOON/JSON through the correlated native operation envelope. Allowlisted MIME/extensions, basename-only bounded filenames, control normalization, mode/provenance labels, and download-only HTML are covered by focused tests (`0e8bdd0`). |
| Portable Windows zip | release CI script | **partial** | Labeled WebView2 prerequisite; clean-image unzip evidence **not proven** (M21) |
| Desktop asset name/integrity verify | `verify_*` + `generate_sha256sums` + `inspect_*` + `probe_*` + `normalize_*` | **partial** | Release CI fail-closes on names/checksums/secrets/structure; unsigned checksums; **not** launch proof |
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
| Continuous heap lifecycle | investigation store + desktop host | **shipped** | M24.A/B: persistent heap identity with **Open / Open another / Close**. M27.A/B adds strict display-safe workspace metadata persistence and full transactional snapshot hydrate; failed opens preserve the current session. |
| Shared investigation selection | investigation store | **shipped** | M24.C: `objectId` / `classKey` / `leakId` selection synchronizes histogram, dominators, inspector, and deep-linked routes without stale-object precedence. |
| Findings + Assistant advisory | rules-derived findings + investigation selection | **shipped** | M24.D: collapsible, Rules-labelled advisory pane with deterministic deep-links; findings remain guidance over deterministic analysis. |
| Correlated operation progress / stale-result rejection | operation registry + Tauri envelopes + investigation store | **shipped** | M26: open/analyze/query/diff/snapshot/flamegraph/GC-path/field-data inspect echo workspace/revision/operation identity; generation-N race tests reject late progress/success. Determinate values appear only when bounded; other phases are explicitly indeterminate. |
| Cooperative Cancel | core checkpoints + registry commit guards + HeapSessionBar | **partial** | M26 proves prompt cooperative stop in parser/dominator/analysis checkpoints and prevents publication for every protocol-wired family. Query/diff/GC traversal/flame rendering/snapshot I/O have boundary/commit guards but no inner loop checkpoint. `run_ci_check`, explain/fix, source mapping, and workflow/AI lifecycle are not protocol-wired. |
| In-workbench compare | existing comparison backend + shared compare components | **shipped** | M27.C delivers the post-v0.5.0 slice in investigation chrome. `/compare` remains a thin adapter; no new analyzer was added. |

## Honesty bar

- Install goal: unzip → double-click → app opens (no JVM). Windows portable zip is primary once CI publishes it; WebView2 remains a documented prerequisite until proven on a clean image.
- This WSL host yields **command-layer / browser-fallback** evidence only. Do not treat green unit tests as packaged GUI launch proof or as a live AI provider round-trip.
- Screenshot gallery per workbench family was **not** captured in the 20.H closeout; visual proof is absent, not implied.
- M23 assistant Ask is **advisory** with explicit provenance; deterministic power tools remain the analysis path.
- M24 findings remain **Rules-derived advisory** content. M27 comparison is integrated into persistent chrome, but packaged-GUI interaction remains **NOT proven** on WSL.
- M26 focused tests prove correlation, rejection, cleanup, and controlled core checkpoints. They do **not** prove packaged-GUI behavior, native Tauri compilation on WSL, or prompt interruption inside every indeterminate operation.
- M27 focused tests prove display-safe persistence, atomic snapshot hydrate, shared compare rendering, option forwarding, and after-side selection. They do **not** prove packaged-GUI behavior or real multi-GB comparison performance.
- M28 focused tests prove OQL/analyzer contracts, format selection, operation-envelope wiring, filename/content normalization, provenance labels, and that hostile report/flamegraph bodies are absent from the DOM. Packaged GUI generation/download and native save behavior remain **NOT PROVEN** on WSL.
