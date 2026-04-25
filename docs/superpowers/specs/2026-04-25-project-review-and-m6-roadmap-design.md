# Project Review, Doc Cleanup & M6 Roadmap Design

_Date: 2026-04-25_

## Context

M1 through M5 are complete. M4 browser-first UI shipped all planned routes and feature slices. Three residual gaps from M4 were identified in the 2026-04-15 project review — these are moved to M6 scope. This session cleans up stale docs, updates ARCHITECTURE.md with the current UI layer, and rewrites the roadmap to close M4 and define M6.

---

## What Gets Deleted

### `OVERNIGHT_SUMMARY.md`
Historical overnight work log. All content is now reflected in `STATUS.md`, `CHANGELOG.md`, and commit history. No information loss.

### `docs/superpowers/plans/` — all 23 files
All completed implementation plans. They were execution artifacts; their content is captured in commits, STATUS.md, and CHANGELOG.md. Keeping them creates false signal about open work.

### `docs/superpowers/specs/` — all 18 files (except this one, post-commit)
All completed design specs. Same rationale as plans.

**Keep:** `docs/superpowers/reviews/2026-04-15-project-review.md` — still useful as a baseline review artifact.

---

## What Gets Updated

### `ARCHITECTURE.md`

Add a **Browser-First UI Layer** section documenting `ui/`:
- React + TypeScript + Bun
- Route map: `/`, `/dashboard`, `/artifacts/explorer`, `/heap-explorer/{dominators,object-inspector,query-console}`, `/leaks/:leakId/{overview,explain,gc-path,source-map,fix}`
- Feature areas: artifact-loader, dashboard, artifact-explorer, heap-explorer, leak-workspace
- Bridge contract: `window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__` and `window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__` for host-side live detail
- Honest unavailable states when bridge absent

Add a **Mermaid architecture diagram** as inline code block (replaces outdated SVG references). Diagram covers: CLI → Core → MCP Server → Browser UI, with AI and Report layers.

Update **"Future Extension Points"**:
- Remove "GUI or Web Dashboard" — shipped as M4 browser-first UI
- Add "Tauri Desktop Packaging" as M6 item
- Add "Live Object References/Referrers" as M6 item
- Add "Heap-Explorer → Leak-Workspace Resolution" as M6 item

Update **"Still in progress"** snapshot to reflect M4 shipped, M6 open.

### `docs/roadmap.md`

**Section 8 Roadmap table:**
- M4: mark `✅ Complete` with shipped scope noted
- M5: mark `✅ Complete`
- M6: expand scope to include original ecosystem/community work + three M4 residual gaps

**Section 9 Milestones Detail — M4:**
- List shipped surfaces
- Note three residual gaps moved to M6:
  1. Live object references/referrers in Object Inspector
  2. Heap-explorer → leak-workspace object-to-leak resolution contract
  3. Tauri desktop packaging

**Section 9 Milestones Detail — M6 (rewritten):**
- Original scope: ecosystem docs, crates.io promotion, Homebrew formula maintenance, contributor docs
- Added from M4: live object refs, heap→leak jumps, Tauri packaging
- Define "M6 done" criteria

**Section 11 Recommended Immediate Next Steps:**
- Replace stale M4/M5 items with M6 starting point

---

## M4 Residual Gaps → M6 Scope

| Gap | Current state | M6 target |
|-----|--------------|-----------|
| Live object references/referrers | Object Inspector shows artifact-backed dominator data only | Add live refs/referrers via bridge contract |
| Heap-explorer → leak-workspace jumps | No object-to-leak mapping exists | Artifact-backed or host-backed resolution contract |
| Tauri desktop packaging | Explicitly deferred | Tauri wrapper over existing React UI |

---

## SVG Diagrams

`resources/architecture-overview.svg` and `resources/architecture.svg` are pre-rendered Mermaid exports with no source `.mmd` files. They predate the UI layer and do not reflect current architecture.

Decision: replace SVG `<img>` references in README and ARCHITECTURE.md with inline Mermaid code blocks. SVG files can be regenerated from the Mermaid source if needed for export.

---

## Definition of Done

- [ ] `OVERNIGHT_SUMMARY.md` deleted
- [ ] All `docs/superpowers/plans/` deleted
- [ ] All `docs/superpowers/specs/` deleted (except this file, until committed)
- [ ] `docs/superpowers/reviews/` preserved
- [ ] `ARCHITECTURE.md` updated with UI layer section + Mermaid diagram
- [ ] `docs/roadmap.md` updated: M4 closed, M6 expanded with gaps
- [ ] `README.md` architecture section updated (remove stale SVG ref, add brief UI note)
- [ ] Spec self-review passed
- [ ] Implementation plan written
