# Older partials closeout (M20 / M21 / M22.E / M23 / jsdom trial)

**Date:** 2026-09-15  
**Status:** Approved (conversation); pending written-spec review  
**Owner:** Sol (agent authorized to record Sol verdicts when evidence is met)  
**Terra stand-in:** Code-review / Bugbot-style subagent  
**Related:** [STATUS.md](../../../STATUS.md), [m21-m22-remaining.md](../../evidence/m21-m22-remaining.md), [ui-first plan](../plans/2026-09-14-ui-first-mat-install-ai-plan.md), [dependency inventory](../../product/dependency-currency-inventory.md)

## 1. Purpose

Close older **partial** milestones with measured evidence after v0.6.0, without starting M31+ or a new maturity arc.

## 2. Locked decisions (from requirements)

| # | Decision |
| --- | --- |
| 1 | Browser host mode **A** — stub / mock bridges only |
| 2 | UI may run on localhost / local network |
| 3 | AI for M23 = **rules / stub** (no live provider required) |
| 4 | Windows packaged smoke allowed (download/install/launch v0.6.0 + WebView2 as needed) |
| 5 | Terra reviews via **subagent** |
| 6 | M21 launch matrix = **Windows-only**; macOS/Linux rows stay **not launch-tested** |

Sol sign-off: agent may record M20/M23 (and related) Sol closeout verdicts **only when evidence rows are actually met**.

## 3. Goals

1. **M22.E** — Publish OQL corpus pass/fail counts, named unsupported rows, CLI/MCP equivalence notes; evidence + STATUS/roadmap/user-guide honesty.
2. **M20** — Browser stub-bridge smoke of workbench surfaces; evidence + Terra + Sol.
3. **M23** — Browser rules/stub guided investigation path; evidence + Terra + Sol.
4. **M21** — Windows launch + Open-heap smoke on published v0.6.0 desktop assets; macOS/Linux explicit not launch-tested; evidence + Terra + Sol **partial** closeout.
5. **jsdom 30** — Separate trial vs M30 RSS ceilings; land only if safe, else keep jsdom 24 + inventory note.

## 4. Non-goals

- M31+ (desktop OS integration, MAT golden, perspectives, plugins/live JVM, signing/updater/Cask)
- Claiming packaged GUI from browser-only runs
- Claiming macOS/Linux launch proof
- Live AI provider round-trips
- Real Eclipse MAT golden dump equivalency (corpus MAT rows remain documentation-referenced unless separately supplied)
- Percentage “MAT parity” extrapolation beyond corpus counts

## 5. Approach

**Docs-and-evidence-first** (Approach A): sequential small PRs, host-honest status updates, no product feature work beyond what closeout evidence requires.

### Sequence

1. M22.E (WSL-completeable)
2. M20 browser stub smoke
3. M23 browser stub smoke (may share PR with M20 if tiny)
4. M21 Windows-only launch matrix
5. jsdom 30 trial (optional land; revert on RSS failure)

## 6. Evidence standard

Each track produces or updates an evidence file under `docs/evidence/` with:

- Commands / URLs / asset names and hashes where applicable
- Pass / fail / **NOT PROVEN** rows
- Explicit statement that unit/CI green ≠ packaged GUI (except M21 Windows where launch is recorded)
- Terra subagent outcome summary
- Sol verdict (met / partial / not met) with date

## 7. Track details

### 7.1 M22.E — OQL corpus closeout

**Do:**

- Run full OQL compatibility corpus; capture sanitized pass/fail counts
- Confirm CLI/MCP normalized equivalence for M22 grammar forms (or document gaps)
- Create `docs/evidence/m22-oql-compatibility.md`
- Update STATUS / roadmap / user-guide: bounded/partial OQL; closed vs open matrix rows without percentage inflation
- Terra subagent; Sol closeout if gates met

**Host:** WSL or any; no GUI required.

### 7.2 M20 — UI workbench browser closeout

**Do:**

- Serve `ui` on localhost with **stub bridges**
- Smoke routes/surfaces that stub mode supports (artifact explorer, heap explorer chrome, policies/flamegraphs/snapshots as stubbed, guided landing where stubbed)
- Record what stub cannot exercise (real Tauri invoke, packaged Open-heap)
- Evidence update to `docs/evidence/m20-ui-workbench.md` (or sibling note)
- Terra; Sol

**Does not** close packaged-desktop claims.

### 7.3 M23 — Guided investigation browser closeout

**Do:**

- Same stub browser host; rules/stub AI only
- Exercise investigation session / fact vs AI separation / history bounds as stub allows
- Evidence update to `docs/evidence/m23-guided-investigation.md`
- Terra; Sol

**Does not** claim live provider proof.

### 7.4 M21 — Portable install / launch matrix (Windows-only)

**Do:**

- Download published `v0.6.0` Windows desktop assets (`.msi` / `.exe` / portable zip as available)
- Ensure WebView2; install/launch on this Windows host
- Open synthetic heap fixture once; record OS, build, asset hash
- Leave macOS/Linux rows **not launch-tested**
- Update `docs/evidence/m21-portable-installability.md` and remaining note
- Terra; Sol **partial** closeout (Windows launch-tested; other platforms open)

**Does not** require signing secrets or notarization.

### 7.5 jsdom 30 trial

**Do:**

- Branch-only bump; run UI batches under documented M30 RSS ceilings
- If pass: land + inventory update
- If fail: revert; keep jsdom 24; document measurement

## 8. Delivery

- Prefer **one PR per track** (M20+M23 may combine if small)
- Main remains protected; merge after CI green
- No force-push to main; tag retag only if a release defect is found (out of scope unless needed)

## 9. Success criteria

| Track | Success |
| --- | --- |
| M22.E | Evidence published; docs honest; Sol met or explicit remaining corpus gaps |
| M20 | Browser stub smoke evidenced; Sol met for **browser/stub** scope; packaged GUI still NOT PROVEN |
| M23 | Stub AI path evidenced; Sol met for stub scope; live provider NOT PROVEN |
| M21 | Windows launch + Open-heap recorded; macOS/Linux not launch-tested; Sol **partial** |
| jsdom | Either 30 lands under ceilings or 24 retained with measured refusal |

## 10. Risks

| Risk | Mitigation |
| --- | --- |
| Stub UI over-claims host capabilities | Evidence templates force NOT PROVEN for Tauri/packaged |
| Windows GUI automation flaky from WSL | Prefer PowerShell launch + manual one-click if needed; record honestly |
| jsdom 30 OOM | Hard abort; keep 24 |
| Terra subagent ≠ historical Terra | Label reviews as subagent Terra stand-in in evidence |

## 11. Out of scope follow-ons

- macOS/Linux launch rows when hosts appear
- Live AI provider evidence
- MAT golden / Eclipse MAT–linked corpus cases
- M31 gate opening
