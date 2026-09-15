# UI → MAT Maturity Roadmap (v0.5.0+)

**Status:** Approved for autonomous execution (product autonomy 2026-09-15) — Approach C  
**Date:** 2026-09-15  
**Supersedes for sequencing:** Informal “keep shipping power routes” habit; **does not** replace [M24 design](2026-09-14-m24-continuous-heap-investigation-design.md) (M24 remains the first product milestone)  
**Product framing:** Take the **desktop/browser UI** to Eclipse-MAT-equivalent **investigation maturity** (workflow + capability), not a pixel clone of Eclipse RCP.

---

## 1. Product definition of “MAT level”

**MAT level** for Mnemosyne means a user can complete this loop without mental context-switching across disconnected apps:

1. Open / **open another** / reopen a heap (or snapshot) / **close** with honest progress and recoverable failure  
2. Always see which heap is active (basename, mode, provenance, operation status) from a **persistent workbench chrome**  
3. Triage findings (leaks / histogram hotspots)  
4. Drill class → instance → dominator context → GC paths → inspector fields/refs  
5. Query (bounded OQL) and compare when needed  
6. Leave and return without losing workspace identity or selection  
7. Optionally ask AI for **advisory** guidance that deep-links into deterministic panes  

**Explicitly not required for “MAT level”:**

- Full Eclipse perspective/plugin model or free-form docking  
- Full MAT OQL (`eval(...)`, unbounded nesting)  
- Live JVM attachment  
- Claiming pixel or report-text equivalence without a golden corpus  
- Autonomous AI that replaces measured analysis  
- Unlimited simultaneous in-memory heaps (bounded multi-workspace is later; default is one active workspace with safe replace)  

Backend already ships most deterministic primitives. Maturity is blocked by **session lifecycle, continuity, honesty under async load, and depth of UI surfaces** — not by missing analyzers.

---

## 2. Approaches considered

### A — Feature-first (add missing MAT panes ASAP)

Ship dominator tree, fields, enrich analyzers, OQL polish on today’s route-first UI, then unify later.

- **Pros:** Visible capability wins quickly  
- **Cons:** Amplifies split-brain (artifact A + graph B), stale diffs, no cancel/progress; hard to unwind  

### B — Platform-first (session + progress + cancel before depth)

Build revisioned workspace, operation protocol, transactional open, then deepen panes.

- **Pros:** Fixes trust/stale UI first; every later pane inherits correctness  
- **Cons:** Early milestones feel “infra-heavy” to users  

### C — Hybrid spine (recommended)

Ship **M32 dependency/toolchain currency** (registry-verified latest) as a **parallel first track**, then **M24 continuous shell + identity + synced selection** as v0.5.0, with **operation IDs and stale-result rejection in the same release train**, then deepen MAT loop (M25), true progress/cancel (M26), durability/compare (M27), power tools (M28), guided continuity (M29), hardening/evidence (M30), optional ecosystem (M31+).

- **Pros:** Matches approved M24 thesis; clears deferred majors before workbench churn; surfaces product value early; does not defer async honesty past the workbench  
- **Cons:** Requires disciplined slice gates; compare may slip past v0.5.0 (already allowed by M24); major UI upgrades (React 19 / Vite 8 / Tailwind 4 / jsdom 30) need CI RSS evidence  

**Recommendation: Approach C (+ M32 first).**

---

## 3. Release mapping

| Release | Milestone gate | Ships when |
|---|---|---|
| **v0.5.0 prep / early 0.5.x** | **M32** dependency & toolchain currency | All deferred majors + registry-latest pins; Dependabot #50–#52 closed; CI green + RSS evidence |
| **v0.5.0** | **M24** (+ M26.C stale-result minimum; progress UI may be indeterminate until M26.A) | Continuous workbench: open → triage → synced core panes; token chrome; docs honest |
| **v0.5.x** | M25–M26 remaining slices | Dominator tree, fields, progress/cancel depth |
| **v0.6.0** | M27–M28 | Durable workspace + integrated compare + OQL/analyzer/export maturity |
| **v0.7.0** | M29–M30 | Findings queue, Assistant binding, a11y, native evidence closeout |
| **Later** | M31+ | Desktop OS integration, MAT golden program, plugins/JVM attach (gated) |

v0.4.x stays hotfix-only. **Do not ship M24 under 0.4.x.**

---

## 4. Architecture decisions (maintainability / debugability)

### 4.1 Single investigation session (frontend)

Introduce `ui/src/features/investigation/` as the authoritative UI aggregate:

- `workspaceId`, `revision` (monotonic)  
- Heap identity: `sourceKind`, opaque `sourceId` / `snapshotKey` / artifact id, **basename only**  
- `analysis` artifact + mode + provenance + capabilities  
- `operation` (active op id, phase, progress or indeterminate)  
- `selection` (stable object/class/leak ids — **never row indices**) + back/forward stacks  
- `layout` (active pane, open panes, sizes, pane-local filters)  
- `requests` keyed by `{revision, operationId}`  

Selectors partition subscriptions so one store does not re-render every pane.

Legacy Zustand stores (`use-artifact-store`, leak/comparison) become adapters until migrated.

### 4.2 Host boundary

Unify behind `InvestigationHost` (Tauri + artifact/browser adapters). Keep today’s six bridge globals only as temporary shims.

Every response echoes `workspaceId` + `revision` (+ `operationId` when async). UI **must ignore** mismatched revisions.

### 4.3 Rust session manager

Evolve Tauri/`session-ops` toward:

- `Arc<ObjectGraph>` + retained `Arc<DominatorTree>` (stop cloning graphs / rebuilding dominators per query)  
- Revision-checked commits  
- Operation registry with cooperative cancel hooks (M26)  
- Lean vs field-data graphs never both retained indefinitely  

### 4.4 Logging correlation

Every open/analyze/query/diff/snapshot op logs to `desktop.log` with:

`operation_id`, `workspace_id`, `revision`, phase, timings, machine-readable error codes  

Never log absolute paths, field values, query bodies, or AI prompts. Rotate desktop logs daily (`desktop.YYYY-MM-DD`) with redaction helpers — shipped under M30.E (`fa33266`); ~14-day retention prune remains operational follow-up, not a never-rotate debt.

### 4.5 Anti-patterns (hard rules)

- Do not treat React cleanup `cancelled` as real cancellation  
- Do not leave artifact A visible with source B remembered  
- Do not show previous compare/query results as current during loading/error without a stale banner  
- Do not mount full production `routes` in unit tests (OOM history)  
- Do not enable field-data analyzers by default on open  
- Do not claim native launch from WSL unit green  

---

## 5. PM UX contract (no stale UI)

Every long operation (open, analyze, enrich, query, diff, snapshot, flamegraph, GC paths) must show:

| State | UI requirement |
|---|---|
| Idle | Clear CTA; previous result owned by current revision only |
| Accepted / in flight | Named phase (“Opening…”, “Building graph…”, “Computing dominators…”) or honest indeterminate; `aria-busy`; disable conflicting actions |
| Progress | Percent/bytes/records when known; else indeterminate bar + elapsed |
| Cancel | Visible when host supports it; after cancel, no late result may apply |
| Success | Atomic commit of revision; toast/status with op id (short) |
| Failure | Previous workspace intact when possible; error + recovery; never silent |
| Partial / fallback | Provenance banner; never green “ready” for skipped/heuristic |

Home remains open/import/recent only until a workspace exists (M24 IA).

---

## 6. Complete milestone catalog

### M32 — Dependency & toolchain currency (first track)

**Inventory (registry-verified):** [docs/product/dependency-currency-inventory.md](../../product/dependency-currency-inventory.md)  
**Closes deferred Dependabot:** #50 `@types/react` 19, #51 `@vitejs/plugin-react` 6, #52 `tailwindcss` 4, plus all other deferred majors called out in CHANGELOG/v0.4.3 notes.

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **32.A** UI framework majors | React 19.3 + types, react-dom 19.3, react-router-dom 7.18, Vite 8.3, `@vitejs/plugin-react` 6.1 | `bun run lint` + CI UI Build & Test green; inventory row updated | High |
| **32.B** UI CSS / table / test majors | Tailwind 4.3 (+ PostCSS adapter as required), `@tanstack/react-table` 9.2, `@testing-library/jest-dom` 7, TypeScript 7.0 | Build + tests green; no silent visual fork without token plan | High |
| **32.C** jsdom + Bun CI pin | jsdom 30.0.1; re-evaluate CI Bun beyond 1.2.5 only with RSS evidence | Peak RSS ceiling documented; no SIGTERM mid-suite | Critical |
| **32.D** Cargo majors | `toml` 1.x, `reqwest` 0.13, `dirs` 7 (unify tauri), `comfy-table` 8, clap/tokio pin refresh; plan `serde_yaml` deprecation migration | Workspace clippy/test green | High |
| **32.E** Docker + Actions | `rust:1.98.1-bookworm`; refresh Action SHAs to current GitHub latest tags | Docker Build Validation + Release dry paths green | Medium |
| **32.F** Closeout | Supersede/close Dependabot #50–#52; STATUS/CHANGELOG/inventory synced | No open deferred-major PR without dated exception | Low |

**Policy:** Targets are always re-queried from npm/crates.io/Docker Hub/GitHub Releases at slice start — never from memory.

---

### M24 — Continuous Heap Investigation (v0.5.0)

Approved product thesis: [M24 design](2026-09-14-m24-continuous-heap-investigation-design.md).

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **24.A** Workspace shell + identity | Persistent shell; heap identity/mode/provenance; **Open / Open another / Close** always reachable | Header shows basename+revision; Open another available after load; Close clears workspace; TopNav/chrome survives investigation | High |
| **24.B** Transactional open/reopen | Open, open-another, import, snapshot reopen are atomic; wire `unload_heap` | Failure leaves prior workspace intact; success replaces artifact+graph+caps together; Close invokes host unload + clears all heap-bound stores; picker cancel is neutral | High |
| **24.C** Synchronized core selection | Shared class/object/leak selection across Histogram / Dominators / Inspector / GC Paths | Select in one pane updates others; back/forward restores selection | High |
| **24.D** Guided continuity | Findings queue + Assistant as collapsible advisory | Deep-links into deterministic panes; rules offline; AI labelled | Medium |
| **24.E** Compare + proof (gateable) | Two-heap compare in shell + packaged evidence | Match quality visible; keyboard path; native open→triage evidence | High — **gate if 24.A–D slip** |

**M24 also absorbs (minimum):**

- **P0:** After open, user can Open another / Close without a Home-only dead end  
- Fix snapshot-open split-brain (graph B + artifact A)  
- Fix failed-open remembered-source vs displayed artifact  
- Make Recent Loads / Recent Heaps **actionable** (Open), not display-only  
- Expose native `unload_heap` through desktop heap bridge  
- Operation IDs + revision echo on open/analyze (even if progress is indeterminate)  
- Token system foundation in `globals.css` + shared status/provenance primitives  

Theme principles already in M24 §8 remain binding.

---

### M25 — MAT investigation-loop depth

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **25.A** Histogram drill-down | Sortable/virtualized histogram; class → bounded instances | Instance pick opens Inspector without losing histogram filters | Medium |
| **25.B** Dominator tree | Lazy expandable tree (not flat list only) | Expand parent/children; retained % filter; no full-tree DOM | High |
| **25.C** Object-centric inspector + paths | Render fields; path nodes navigable; synced refs/referrers | Fields shown when `retainFieldData` requested with cost disclosure; truncation/fallback honest | Medium–High |

---

### M26 — Operation / async platform

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **26.A** Progress protocol | Host events: phase + bounded progress | Open/analyze/diff/query/expensive detail emit events or explicit indeterminate | High |
| **26.B** True cancellation | UI → Tauri → cooperative core checkpoints | Cancel stops work, frees temps, cannot publish after cancel | High |
| **26.C** Stale-result enforcement | Revision+op id on all mutating responses | Race tests: gen N cannot mutate gen N+1 | Medium |

---

### M27 — Durable investigations + compare

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **27.A** Workspace persistence | Persist layout/filters/selection metadata (display-safe) | Return restores pane+selection; no absolute paths / full graphs in storage | Medium |
| **27.B** Snapshot-first reopen | Snapshot open = full workspace hydrate | Graph + facts + mode + compatible selection atomic | High |
| **27.C** Integrated compare | Current vs baseline pickers; drill to inspector | Strategy/topN/match quality; after-side navigation | High |

---

### M28 — Power-tool completeness

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **28.A** OQL workbench | History, examples, structured errors, object navigation | Honest about named OQL deferrals | Medium |
| **28.B** On-demand analyzers | Strings/collections/arrays/threads/referrers/classloaders from workspace | Cost/memory disclosure; lean default preserved | High |
| **28.C** Viz + export | Flamegraph formats + report export with provenance | No unsafe HTML; exports labelled | Medium |

Also: policy baseline picker; compare identity strategy UI; recommendations content (not just count).

---

### M29 — Guided investigation continuity

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **29.A** Unified findings queue | Leaks + classloader + waste + policy findings | Every finding deep-links; resolved/deferred without rewriting facts | Medium |
| **29.B** Workflow binding | Workspace-scoped start/resume/close (no pasted IDs) | Current workflow/step visible in shell + Assistant | Medium |
| **29.C** Contextual Assistant | Selection-aware collapsible advisory | Provenance on every turn; never replaces tools | Medium–High |

---

### M30 — Product hardening

| Slice | Goal | Acceptance | Risk |
|---|---|---|---|
| **30.A** Theme + responsive system | Shared tokens/primitives; remove hex sprawl | Home + workbench audited; narrow widths usable | Medium |
| **30.B** Keyboard + a11y | Core loop keyboard-complete | Automated a11y + manual keyboard evidence | Medium–High |
| **30.C** Native release evidence | Packaged Win/macOS/Linux open→inspect→path | Evidence docs with NOT-PROVEN sections | High (host-blocked on WSL) |
| **30.D** UI test memory safety | Per-batch RSS ceilings; reset helpers | CI fails on memory regression, not only SIGTERM | Medium |
| **30.E** Doc drift cleanup | STATUS / ARCHITECTURE / matrix / roadmap sync | No contradictory bridge/OQL claims | Low |

Close leftover Terra/Sol items from M20–M23 as part of 30.C/30.E where evidence allows.

---

### M31+ — Optional / gated

| Slice | Goal | Gate |
|---|---|---|
| **31.A** Desktop OS integration | Menus, file associations, recent heaps, deep links | After M24–M27 stability |
| **31.B** MAT golden / equivalence program | Versioned MAT reference outputs | Fixture licensing + MAT versions |
| **31.C** Perspectives as layout presets | Named pane layouts only | After M24 density proven |
| **31.D** Phase-3 plugins / live JVM | Dynamic loading / attach | Security + resource model; **must not** block MAT post-mortem UI |
| **31.E** Signing / updater / Homebrew Cask | Distribution maturity | Secrets + product decision |

---

## 7. Highest-priority defects (fix in/near M24.A–B)

1. **No Open another / Close heap after load** — Open lives only on Home; TopNav disappears; `unload_heap` exists in Tauri but is not bridged  
2. Snapshot open installs graph B while artifact A remains displayed  
3. Failed desktop open can leave remembered source B with artifact A  
4. Recent Loads / Recent Heaps are display-only (cannot reopen)  
5. No operation ID / progress / cancel on open-analyze  
6. Comparison / query / flamegraph can render previous results during loading/error  
7. No authoritative workspace generation across host calls  
8. Object fields parsed but not rendered (M25.C)  
9. Analysis mode discarded in artifact UI model  
10. Native packaged workflow evidence absent (M30.C)  

---

## 7b. Thorough missed-case catalog (MAT vs Mnemosyne UI)

Evidence from code + MAT Basic Tutorial / Workbench expectations. **“Shipped” in the capability matrix often means a route exists**, not a continuous MAT loop.

### Session lifecycle (P0 spine)

| Case | Today | Target owner |
|---|---|---|
| Open heap | Home only → navigates to `/dashboard` | M24.A–B |
| **Open another** while investigating | **Missing** | **M24.A–B** |
| Close / unload | Native `unload_heap` unwired; no UI | M24.B |
| Persistent chrome / heap header | TopNav only on `/` | M24.A |
| Actionable recent heaps | Display-only lists | M24.B / M27.B |
| Snapshot open hydrates React workspace | Graph-only / split-brain | M24.B / M27.B |
| Transactional replace (fail keeps prior) | Non-transactional | M24.B + M26.C |
| Drag-drop `.hprof` / OS file association | JSON drop only; no association | M31.A |
| Multi-heap tabs | Single artifact + single graph | M27 / M31.A (bounded) |

### Core MAT loop

| Case | Today | Target owner |
|---|---|---|
| Histogram → instances → inspector | Aggregate bucket only | M25.A |
| Dominator tree (lazy expand) | Flat shortlist + row index | M25.B |
| Inspector fields / arrays | Parsed, **not rendered**; no field-data request | M25.C |
| GC paths from any object; clickable nodes | Leak-route + manual object ID | M24.C + M25.C |
| Shared selection + back/forward | Fragmented stores | M24.C / M27.A |
| Overview top-consumers / recommendations content | Count / static cards | M24.D / M25 |
| Threads / classloaders / unreachable drill-down | Mostly terminal tables | M25.C / M28.B |
| Compare with pickers + strategy + after-side nav | Opaque keys; stale diff risk | M27.C |
| OQL workbench | Textarea + raw table | M28.A |

### Async / honesty

| Case | Today | Target owner |
|---|---|---|
| Phase + progress + elapsed | Opening…/Analyzing… labels | M24 min → M26.A |
| True cancel | React cleanup only | M26.B |
| Revision-safe responses | Partial (JSON import only) | M26.C |
| On-demand enrich (not “rerun CLI”) | Lean open; CLI copy | M28.B |
| Mode/capability envelope in UI | Mode discarded | M24.A |

### Power / guided / export

| Case | Today | Target owner |
|---|---|---|
| Export / clipboard / flamegraph formats | Minimal | M28.C |
| Unified findings queue | Dispersed | M29.A |
| Workflow without pasted IDs | Paste resume | M29.B |
| Contextual Assistant pane | Separate `/assistant` | M29.C |
| Policy baseline picker | Unsupported in UI | M28 |
| Notes / bookmarks | Absent | M27.A |

### Explicitly later / gated

Component report, finalizer overview, weak-ref exclusion presets, MAT golden corpus, free-form docking, live JVM, Phase-3 plugins — **M31+** only after spine proven.

---

## 8. Coverage of previously skipped / partial work

| Skipped / partial item | Milestone |
|---|---|
| No Open another / Close / unload after load | **M24.A–B** |
| Deferred UI majors (Tailwind 4 / Vite 8 / React 19 / jsdom / RR7 / table 9 / TS7) | **M32** |
| Cargo majors (`toml` 1 / `reqwest` 0.13 / `dirs` 7 / `comfy-table` 8) + Docker rust 1.98 | **M32** |
| Dependabot #50–#52 | **M32.F** |
| M24 implementation | M24 |
| Superclass collapsible tree (needs parentKey contract) | M25.A / backend contract if required |
| Snapshot reopen hydration | M24.B / M27.B |
| Progress + cancel | M24 minimum IDs → M26 |
| Dominator tree UI | M25.B |
| Inspector fields | M25.C |
| On-demand enrich analyzers | M28.B |
| OQL workbench maturity | M28.A |
| Integrated compare / strategy UI | M24.E gate or M27.C |
| Workflow ↔ Assistant binding | M29 |
| Theme tokens | M24 foundation → M30.A |
| M21 native launch proof | M30.C (host-blocked here) |
| M22 MAT golden OQL | M31.B |
| M20/M23 Terra·Sol closeout | M30.C / M30.E |
| UI test OOM hardening | M30.D |
| ARCHITECTURE/STATUS drift | M30.E (ongoing each milestone) |

---

## 9. Documentation obligations (every milestone)

Before a milestone is “shipped”:

1. Update `STATUS.md` snapshot + capability checklist rows  
2. Update `docs/product/ui-capability-matrix.md`  
3. Update `docs/roadmap.md` active section  
4. Add/extend evidence under `docs/evidence/` with **NOT-PROVEN** honesty  
5. Keep `ARCHITECTURE.md` current for session/host contracts when they change  
6. Release notes only when cutting a version tag  

---

## 10. Execution order (first 90 days)

1. **Approve this roadmap** + keep M24 design as product thesis  
2. **M32** — registry-refresh inventory → 32.A→32.F (CI-validated; close Dependabot majors)  
3. Write M24 implementation plan (`docs/superpowers/plans/…`) with bite-sized tasks  
4. Branch `feature/m24-continuous-investigation` from `main` after M32 green (or parallel only if deps PRs do not touch the same files)  
5. Implement **24.A → 24.B → 24.C** (do not start deep MAT trees before transactional open)  
6. Parallel doc sync + UI race tests for stale results  
7. **24.D**; gate **24.E**  
8. Cut **v0.5.0** when M24 success criteria met  
9. Continue M25/M26 on `0.5.x` without blocking the first workbench release  

---

## 11. Success criteria (program)

- User can **Open → investigate → Open another / Close** without losing trust or chrome  
- User never sees two heaps’ identities mixed in one shell  
- Long ops never leave a silently stale primary view  
- Core MAT loop works as one workspace on packaged desktop  
- AI remains advisory with provenance  
- Dependencies match registry `latest` (or dated exception) per M32 inventory  
- Docs never claim MAT golden / native launch without evidence  

---

## 12. Open decision for approval

**Confirm Approach C:** M32 (registry-latest deps) first → M24 workbench as **v0.5.0** (must include Open another / Close) → M25–M31+ as mapped.

Alternates:
- **C′:** Slip v0.5.0 until M25.C (fields + dominator tree) — stronger MAT feel, slower first continuous shell  
- **C″:** Start M24 shell before finishing all of M32 (only if deps PRs do not collide; still must land M32 before claiming toolchain currency)
