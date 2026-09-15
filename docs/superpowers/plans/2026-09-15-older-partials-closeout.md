# Older Partials Closeout (M20 / M21 / M22.E / M23 / jsdom trial) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close older **partial** milestones (M22.E OQL corpus, M20 browser stub UI, M23 stub AI, M21 Windows-only launch, optional jsdom 30 trial) with measured evidence after v0.6.0 — docs/evidence-first, without starting M31+ or claiming MAT golden / signed installers.

**Architecture:** Docs-and-evidence-first (Approach A from the approved design). Prefer one logical PR per track (M20+M23 may share a PR if the diff stays tiny). Each track produces or updates a `docs/evidence/` file with commands, pass/fail/**NOT PROVEN** rows, Terra subagent stand-in notes, and a Sol verdict dated when evidence is actually met. Minimal product code — only what closeout evidence requires (focused stub UI tests, honesty doc edits, optional jsdom pin).

**Tech Stack:** Rust (`mnemosyne-core` corpus + CLI/MCP over shared `execute_query`), Bun/Vite UI + jsdom 24 (trial 30), Tauri packaged Windows assets from GitHub `v0.6.0`, PowerShell via WSL for Windows launch, GitHub `gh` for asset download.

**Spec:** [docs/superpowers/specs/2026-09-15-older-partials-closeout-design.md](../specs/2026-09-15-older-partials-closeout-design.md) (**Sol APPROVED** 2026-09-15)

## Global Constraints

1. **Docs/evidence heavy; minimal product code** — no new analyzers, no MAT golden dumps, no live AI provider, no signing/notarization.
2. **Honesty bar** — unit/CI green ≠ packaged GUI; browser stub ≠ Tauri invoke; stub AI ≠ live provider; Windows launch ≠ macOS/Linux launch.
3. **Never claim** MAT golden equivalency, percentage “MAT parity” beyond corpus counts, or signed installers.
4. **Sequence is binding:** M22.E → M20 → M23 → M21 → jsdom 30 (optional; failure must not block other tracks).
5. **One logical PR per track** preferred; M20+M23 may combine if small.
6. **Terra = code-review / Bugbot-style subagent** — label evidence as “Terra subagent stand-in,” not historical Terra human.
7. **Sol verdicts** — record only when evidence rows are actually met; M21 Sol is **partial** (Windows only).
8. **Host reality** — WSL for corpus/UI/docs; Windows under dual-boot (or Windows host reachable via `powershell.exe`) for M21 launch.
9. **Privacy** — never log absolute Windows user paths with secrets, heap contents, or API keys in evidence; record OS version, asset names, and SHA-256 only.
10. **Main stays protected** — merge after CI green; no force-push to main.

---

## File map

| Track | Create | Modify | Test / run |
| --- | --- | --- | --- |
| M22.E | `docs/evidence/m22-oql-compatibility.md` | `STATUS.md`, `docs/roadmap.md`, `docs/user-guide.md`, `docs/evidence/m21-m22-remaining.md`, optionally `docs/design/milestone-22-bounded-mat-oql-polish.md` status line | `cargo test -p mnemosyne-core --test oql_compatibility_corpus`; CLI/MCP query gates below |
| M20 | (optional) `ui/src/features/workbench/m20-browser-stub-smoke.test.tsx` | `docs/evidence/m20-ui-workbench.md` | Focused `bun test` on stub-bridge suites; optional `bun run dev` |
| M23 | (optional thin additions only) | `docs/evidence/m23-guided-investigation.md` | Assistant-focused `bun test`; session-ops optional |
| M21 | (ephemeral) `/tmp/mnemosyne-m21/` download + synthetic `.hprof` | `docs/evidence/m21-portable-installability.md`, `docs/evidence/m21-m22-remaining.md` | `gh release download`; `powershell.exe` install/launch |
| jsdom | none if revert | `ui/package.json`, `ui/bun.lock`, `docs/product/dependency-currency-inventory.md`, optionally `STATUS.md` M32 line | `cd ui && bun run test` with RSS log capture |

---

## Program sequence

| Order | Track | Preferred PR title |
| --- | --- | --- |
| 1 | M22.E OQL corpus closeout | `docs(m22): publish OQL corpus closeout evidence` |
| 2 | M20 browser stub closeout | `docs(m20): browser stub workbench closeout` |
| 3 | M23 stub AI closeout | `docs(m23): stub AI guided investigation closeout` (or combine with M20) |
| 4 | M21 Windows-only | `docs(m21): Windows v0.6.0 launch + Open-heap partial closeout` |
| 5 | jsdom 30 trial | `chore(ui): trial jsdom 30 under M30 RSS ceilings` **or** `docs: retain jsdom 24 after measured RSS refusal` |

---

### Task 1: M22.E — Run OQL corpus and capture counts

**Files:**
- Create: `docs/evidence/m22-oql-compatibility.md`
- Read-only inputs: `core/tests/fixtures/oql/mat-compatibility.json`, `core/tests/oql_compatibility_corpus.rs`

**Interfaces:**
- Consumes: corpus JSON `version`, `cases[]` with `id`, `status`, `equivalency`, `category`, `expect.parse` / `expect.execute`
- Produces: evidence file with exact pass/fail/**NOT PROVEN** tables for Sol

- [ ] **Step 1: Inventory corpus JSON (sanitized counts)**

Run:

```bash
cd /home/bballer09/Workspace/Mnemosyne
node -e '
const d=require("./core/tests/fixtures/oql/mat-compatibility.json");
const status={}, eq={}, cat={};
for (const c of d.cases) {
  status[c.status]=(status[c.status]||0)+1;
  eq[c.equivalency]=(eq[c.equivalency]||0)+1;
  cat[c.category]=(cat[c.category]||0)+1;
}
console.log(JSON.stringify({
  version:d.version,
  total:d.cases.length,
  status, equivalency:eq, category:cat,
  unsupported:d.cases.filter(c=>c.status==="unsupported").map(c=>c.id),
  documentation_referenced:d.cases.filter(c=>c.equivalency==="documentation-referenced").map(c=>c.id),
  non_equivalency:d.cases.filter(c=>c.equivalency==="non-equivalency").map(c=>c.id),
  mat_referenced:d.cases.filter(c=>c.equivalency==="mat-referenced").map(c=>c.id),
}, null, 2));
'
```

Expected (as of 2026-09-15 tip; re-copy actual numbers into evidence if they drift):

- `total: 13`
- `status: { shipped: 12, unsupported: 1 }`
- `equivalency: { documentation-referenced: 7, non-equivalency: 6 }` (zero `mat-referenced`)
- unsupported id: `unsupported-eval`
- **Do not** invent a MAT-parity percentage.

- [ ] **Step 2: Run the corpus test**

Run:

```bash
cargo test -p mnemosyne-core --test oql_compatibility_corpus -- --nocapture 2>&1 | tee /tmp/m22-oql-corpus-test.txt
```

Expected: all tests in that binary pass (including `only_mat_referenced_shipped_cases_are_equivalency_eligible`, which asserts **zero** `mat-referenced` golden cases). Record the `test result: ok. N passed; 0 failed` line verbatim in evidence.

- [ ] **Step 3: Create evidence file**

Write `docs/evidence/m22-oql-compatibility.md` with this structure (fill counts from Steps 1–2):

```markdown
# M22.E OQL compatibility corpus — evidence

**Date:** YYYY-MM-DD
**Branch:** <branch>
**Tip:** `git rev-parse --short HEAD`
**Corpus:** `core/tests/fixtures/oql/mat-compatibility.json` (version N)
**Runner:** `cargo test -p mnemosyne-core --test oql_compatibility_corpus`

## Commands

```bash
cargo test -p mnemosyne-core --test oql_compatibility_corpus -- --nocapture
node -e '/* inventory script from plan Task 1 Step 1 */'
```

## Pass / fail counts (corpus JSON)

| Bucket | Count | Notes |
| --- | --- | --- |
| Total cases | N | |
| `status=shipped` | N | Behavioral expectations enforced by runner |
| `status=unsupported` | N | Named gaps (see below) |
| `equivalency=documentation-referenced` | N | Handbook-linked; **not** MAT golden |
| `equivalency=non-equivalency` | N | Mnemosyne bounds / syntax deltas |
| `equivalency=mat-referenced` | 0 | **NOT PROVEN** — no recorded MAT golden dumps |

## Test binary result

- Paste: `test result: ok. …`

## Named unsupported / open rows

| Case id | Status | Why open |
| --- | --- | --- |
| `unsupported-eval` | unsupported | `eval(...)` scriptlets deferred |
| (list any other open matrix rows from design doc) | open | e.g. arbitrary-depth nesting, live attach, general `SELECT DISTINCT *` |

## CLI / MCP equivalence

| Grammar form | CLI | MCP | Notes |
| --- | --- | --- | --- |
| multi-class `FROM` | … | … | |
| 1–3 hop `OBJECTS` | … | … | |
| 4-hop reject | … | … | |
| `SELECT DISTINCT OBJECTS` | … | … | |

(Fill in Task 2.)

## Honesty

- Unit/corpus green ≠ Eclipse MAT golden equivalency.
- Do not extrapolate a percentage “MAT parity” beyond these counts.
- Packaged GUI / MAT golden remain **NOT PROVEN**.

## Terra subagent stand-in

- Reviewer: <subagent id/type>
- Focus: corpus honesty labels, no mat-referenced inflation, docs match counts
- Outcome: …

## Sol verdict

- **Scope:** M22.E corpus closeout
- **Verdict:** met | met with named remaining gaps | not met
- **Date:** YYYY-MM-DD
```

- [ ] **Step 4: Commit evidence draft (corpus numbers only; docs honesty in Task 3)**

```bash
git add docs/evidence/m22-oql-compatibility.md
git commit -m "$(cat <<'EOF'
docs(m22): capture OQL corpus pass/fail evidence

Publish measured corpus buckets and test binary result for 22.E closeout.
EOF
)"
```

---

### Task 2: M22.E — CLI / MCP normalized equivalence for M22 grammar forms

**Files:**
- Modify: `docs/evidence/m22-oql-compatibility.md` (CLI/MCP table)
- Test (existing): `cli/tests/integration.rs` (query cases), `core/src/mcp/server.rs` (`handle_request_query_heap_*`), `core/tests/query_parser.rs`, `core/tests/query_executor.rs`
- Create only if a gap is found: `cli/tests/oql_m22_cli_mcp_parity.rs` **or** a short note that both surfaces call `mnemosyne_core::query::execute_query` and focused gates already cover the forms — do **not** add a second query engine.

**Interfaces:**
- Consumes: same OQL strings as corpus shipped cases where fixtures allow
- Produces: evidence rows proving CLI stdout / MCP JSON agree on matched count + error class for representative forms

- [ ] **Step 1: Run focused core query + corpus gates**

```bash
cargo test -p mnemosyne-core --test oql_compatibility_corpus
cargo test -p mnemosyne-core --test query_parser -- --nocapture
cargo test -p mnemosyne-core --test query_executor multi_class objects_hop distinct -- --nocapture
```

Expected: pass (adjust filter tokens if local test names differ; prefer `--list` then re-run named tests rather than inventing names).

- [ ] **Step 2: Run CLI query integration samples**

```bash
cargo test -p mnemosyne-cli --test integration test_query_command -- --nocapture
```

Expected: pass. If multi-class / multi-hop forms lack CLI coverage, add **one** focused CLI test that writes `build_graph_fixture()` (or corpus-equivalent fixture) and asserts:

1. multi-class `FROM "A", "B"` succeeds with JSON or text match count ≥ 1
2. `SELECT OBJECTS a.b.c` (3-hop) succeeds or matches corpus expectation
3. 4-hop form returns structured reject (non-zero exit / error substring)

Show the exact assertion strings in the PR; do not claim MAT golden.

- [ ] **Step 3: Run MCP `query_heap` unit tests**

```bash
cargo test -p mnemosyne-core handle_request_query_heap -- --nocapture
```

Expected: pass. Document that MCP `query_heap` and CLI `query` both normalize through `parse_query` + `execute_query`.

- [ ] **Step 4: Optional thin parity proof (only if Step 2–3 leave a grammar form unproven)**

If needed, add `cli/tests/oql_m22_cli_smoke.rs` that runs the three forms above and records exit codes + `Matched:` lines. For MCP, prefer extending an existing `#[tokio::test]` in `core/src/mcp/server.rs` rather than a new crate. Keep the diff ≤ ~80 lines.

- [ ] **Step 5: Update evidence CLI/MCP table**

Fill the Task 1 evidence table with: command run, pass/fail, and “normalized via shared `execute_query`” note. Mark any untested form **NOT PROVEN** rather than inventing coverage.

- [ ] **Step 6: Commit**

```bash
git add docs/evidence/m22-oql-compatibility.md
# plus any new/changed test files from Steps 2–4
git commit -m "$(cat <<'EOF'
test(m22): record CLI/MCP OQL grammar equivalence notes

Document shared execute_query path and focused gates for M22 forms.
EOF
)"
```

---

### Task 3: M22.E — STATUS / roadmap / user-guide honesty

**Files:**
- Modify: `STATUS.md` (M22 bullet ~line 23)
- Modify: `docs/roadmap.md` (UI-first status blurb + any M22 rows that still say “Pending” under old numbering — prefer a short honesty insert pointing at evidence, do not rewrite the entire archive)
- Modify: `docs/user-guide.md` § `query` — especially the stale “Named deferrals” paragraph that still lists multi-class `FROM` and multi-hop `OBJECTS` as deferrals (those shipped in 22.B/22.C; keep `eval(...)`, arbitrary-depth nesting, live attach as open)
- Modify: `docs/evidence/m21-m22-remaining.md` — mark 22.E closed or “closed with named open rows”
- Modify: `docs/design/milestone-22-bounded-mat-oql-polish.md` status line to reflect 22.E evidence published

- [ ] **Step 1: Patch STATUS.md M22 bullet**

Replace the “Remaining: corpus Terra/closeout (22.E)” clause with language like:

```markdown
- 🟡 **M22 bounded MAT OQL (partial closeout)** — **22.A–22.D** shipped; **22.E** corpus evidence published ([docs/evidence/m22-oql-compatibility.md](docs/evidence/m22-oql-compatibility.md)): N shipped / N unsupported cases; **0** `mat-referenced` golden rows (**NOT PROVEN**). OQL remains bounded/partial. Named open: `eval(...)`, arbitrary-depth nesting, live attach, MAT golden dumps.
```

Use the real N values from Task 1.

- [ ] **Step 2: Patch user-guide named deferrals**

In `docs/user-guide.md` around the query “Named deferrals” paragraph, change to:

```markdown
Named deferrals (not silent gaps): `eval(...)` scriptlets, arbitrary-depth subquery nesting, Java/JavaScript execution, and live JVM attach. Multi-class `FROM` (bounded) and 1–3 hop `OBJECTS` (4+ rejected) shipped under M22 — see [evidence/m22-oql-compatibility.md](evidence/m22-oql-compatibility.md) and [design/milestone-22-bounded-mat-oql-polish.md](design/milestone-22-bounded-mat-oql-polish.md). Do not claim full Eclipse MAT OQL equivalence.
```

Also fix any nearby sentence that still says “`OBJECTS` is intentionally single-hop only” if multi-hop 1–3 is shipped — replace with “`OBJECTS` supports 1–3 object-ref hops; 4+ hops return a structured reject.”

- [ ] **Step 3: Patch roadmap + remaining note**

In `docs/roadmap.md` status update block, add one sentence that UI-first **M22.E** evidence is published and OQL stays partial. In `docs/evidence/m21-m22-remaining.md`, move “Recorded MAT golden dump equivalency” to remain blocked; remove “corpus Terra/closeout” as the M22 blocker if Sol met.

- [ ] **Step 4: Commit**

```bash
git add STATUS.md docs/roadmap.md docs/user-guide.md \
  docs/evidence/m21-m22-remaining.md \
  docs/design/milestone-22-bounded-mat-oql-polish.md \
  docs/evidence/m22-oql-compatibility.md
git commit -m "$(cat <<'EOF'
docs(m22): align STATUS and guides with corpus closeout honesty

Retire stale multi-class/multi-hop deferral wording; keep MAT golden NOT PROVEN.
EOF
)"
```

---

### Task 4: M22.E — Terra subagent + Sol verdict + PR

**Files:**
- Modify: `docs/evidence/m22-oql-compatibility.md` (Terra + Sol sections)

- [ ] **Step 1: Run Terra subagent stand-in**

Dispatch a Bugbot / code-review style subagent on the M22.E diff with this prompt focus:

- Corpus counts match JSON + test output
- No `mat-referenced` inflation / no percentage parity claim
- user-guide deferrals match shipped vs open rows
- CLI/MCP notes do not over-claim

Paste a 3–6 line summary into the evidence Terra section; label **Terra subagent stand-in**.

- [ ] **Step 2: Record Sol verdict**

Only if gates met: `Sol verdict: met` (or `met with named remaining gaps`) + date. If gaps remain (e.g. CLI form unproven), use `not met` or explicit remaining rows — do not rubber-stamp.

- [ ] **Step 3: Open PR (Track 1)**

```bash
git push -u origin HEAD
gh pr create --title "docs(m22): OQL corpus closeout evidence (22.E)" --body "$(cat <<'EOF'
## Summary
- Publish `docs/evidence/m22-oql-compatibility.md` with corpus pass/fail buckets and test output
- Document CLI/MCP shared-query equivalence for M22 grammar forms
- Align STATUS / roadmap / user-guide honesty (bounded OQL; no MAT golden claim)

## Test plan
- [ ] `cargo test -p mnemosyne-core --test oql_compatibility_corpus`
- [ ] Focused CLI/MCP query gates listed in the evidence file
- [ ] Docs reviewed for no percentage MAT-parity claim

EOF
)"
```

---

### Task 5: M20 — Focused browser stub-bridge UI tests

**Files:**
- Prefer modify existing tests under:
  - `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`
  - `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`
  - `ui/src/features/policy/PolicyCheckPage.test.tsx`
  - `ui/src/features/snapshots/SnapshotManagerPage.test.tsx`
  - `ui/src/features/flamegraph/FlamegraphPage.test.tsx`
  - `ui/src/features/heap-explorer/HeapDominatorPage.test.tsx`
  - `ui/src/app/TopNav.test.tsx`
- Create only if missing a single cross-surface smoke: `ui/src/features/workbench/m20-browser-stub-smoke.test.tsx`
- Modify: `ui/run-tests-config.ts` only if a new file must join a batch (add to `m20-surfaces` files list)

**Interfaces:**
- Consumes: `window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__`, `__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__`, and other keys from `ui/src/test/reset-test-state.ts`
- Produces: green focused tests proving stub mode can render workbench chrome without Tauri

- [ ] **Step 1: Run existing M20 surface suites (baseline)**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun test \
  src/features/policy/PolicyCheckPage.test.tsx \
  src/features/snapshots/SnapshotManagerPage.test.tsx \
  src/features/flamegraph/FlamegraphPage.test.tsx \
  src/features/artifact-loader/ArtifactLoaderPage.test.tsx \
  src/features/heap-explorer/HeapDominatorPage.test.tsx \
  src/app/TopNav.test.tsx
```

Expected: pass (or note any pre-existing failures separately — do not silently expand scope).

- [ ] **Step 2: Add or extend one stub-smoke assertion set (minimal)**

If Step 1 already proves stub bridges for policies/snapshots/flamegraphs/open-heap chrome, **do not** add a new file — document those tests as the M20 browser evidence in Task 7.

If a gap exists (e.g. no single test that asserts TopNav reaches `/workbench/policies`, `/workbench/snapshots`, `/workbench/flamegraphs`, `/artifacts/explorer`, `/assistant` under stub), create `ui/src/features/workbench/m20-browser-stub-smoke.test.tsx`:

```tsx
import "../../test/setup";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { TopNav } from "../../app/TopNav";
import { resetAllTestState } from "../../test/reset-test-state";

afterEach(() => {
  cleanup();
  resetAllTestState();
});

describe("M20 browser stub smoke", () => {
  it("exposes workbench routes without a Tauri bridge", () => {
    // Intentionally no window.__MNEMOSYNE_* assignment — browser host mode A.
    const router = createMemoryRouter(
      [{ path: "/", element: <TopNav /> }],
      { initialEntries: ["/"] },
    );
    const view = render(<RouterProvider router={router} />);
    for (const name of [/policies/i, /snapshots/i, /flamegraphs/i, /assistant/i]) {
      expect(view.getByRole("link", { name })).toBeTruthy();
    }
  });
});
```

Adjust accessible names to match the real `TopNav` labels (read `ui/src/app/TopNav.tsx` first). Add the file to the `m20-surfaces` batch in `ui/run-tests-config.ts`.

- [ ] **Step 3: Re-run focused tests**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun test src/features/workbench/m20-browser-stub-smoke.test.tsx \
  src/features/policy/PolicyCheckPage.test.tsx \
  src/features/snapshots/SnapshotManagerPage.test.tsx \
  src/features/flamegraph/FlamegraphPage.test.tsx
```

Expected: pass.

- [ ] **Step 4: Commit (tests only if any)**

```bash
git add ui/src/features/workbench/m20-browser-stub-smoke.test.tsx ui/run-tests-config.ts
# or only modified existing tests
git commit -m "$(cat <<'EOF'
test(m20): browser stub-bridge smoke for workbench surfaces

Exercise stub-host routes without claiming packaged Tauri GUI.
EOF
)"
```

Skip this commit if no code changed (docs-only path in Task 7).

---

### Task 6: M20 — Optional localhost Vite smoke (non-Tauri)

**Files:**
- Modify: `docs/evidence/m20-ui-workbench.md` (smoke subsection)

- [ ] **Step 1: Start Vite**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun run dev -- --host 127.0.0.1 --port 5173
```

Expected log: local URL `http://127.0.0.1:5173/`.

- [ ] **Step 2: Curl routes that work without Tauri**

```bash
for path in / /dashboard /artifacts/explorer /workbench/policies \
  /workbench/snapshots /workbench/flamegraphs /assistant \
  /heap-explorer/dominators /heap-explorer/query-console; do
  code=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5173$path")
  echo "$code $path"
done
```

Expected: `200` for SPA shell (Vite may return 200 for all client routes). Record that **interactive** stub behavior still depends on injected bridges / loaded artifacts — HTTP 200 ≠ Open-heap success.

- [ ] **Step 3: Stop the dev server** (Ctrl-C / kill the Vite pid). Do not leave it running.

- [ ] **Step 4: Explicit NOT PROVEN rows to record**

| Claim | Status |
| --- | --- |
| Real `invoke` / Tauri IPC | **NOT PROVEN** in browser stub mode |
| Packaged Open-heap | **NOT PROVEN** |
| Packaged GUI smoke | **NOT PROVEN** (belongs to M21) |

No commit yet — fold into Task 7 evidence update.

---

### Task 7: M20 — Evidence update + Terra + Sol (browser/stub scope only)

**Files:**
- Modify: `docs/evidence/m20-ui-workbench.md`
- Modify: `STATUS.md` M20 bullet (browser/stub Sol only; packaged still pending/NOT PROVEN)

- [ ] **Step 1: Append closeout section to evidence**

Add dated section:

```markdown
## Browser stub closeout (YYYY-MM-DD)

**Host mode:** A — stub / mock bridges only (`window.__MNEMOSYNE_*`)
**Commands:** (paste Task 5 bun test + Task 6 curl)

### Pass
- Focused stub UI tests: … pass
- Localhost Vite route smoke (optional): … 

### NOT PROVEN
- Packaged Tauri GUI
- Real desktop Open-heap / analyze invoke
- Clean Windows portable without WebView2

### Terra subagent stand-in
- …

### Sol verdict (browser/stub scope only)
- **Verdict:** met | not met
- Packaged GUI remains **NOT PROVEN**
- **Date:** …
```

- [ ] **Step 2: Update STATUS.md M20 line** to say browser/stub Sol met (if true) while packaged GUI remains NOT PROVEN / deferred to M21 evidence.

- [ ] **Step 3: Terra subagent** on M20 docs+tests; paste outcome.

- [ ] **Step 4: Commit + PR (Track 2; may wait to combine with M23)**

```bash
git add docs/evidence/m20-ui-workbench.md STATUS.md
git commit -m "$(cat <<'EOF'
docs(m20): browser stub workbench closeout evidence

Record stub-bridge tests and keep packaged GUI NOT PROVEN.
EOF
)"
```

---

### Task 8: M23 — Stub AI closeout tests + evidence

**Files:**
- Test: `ui/src/features/assistant/assistant-bridge-client.test.ts`
- Test: `ui/src/features/assistant/InvestigationAssistantPage.test.tsx`
- Modify: `docs/evidence/m23-guided-investigation.md`
- Modify: `STATUS.md` M23 bullet
- Optional: `cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures` (command-layer only; still ≠ live provider)

**Interfaces:**
- Consumes: rules-mode default + stub `__MNEMOSYNE_ASSISTANT_BRIDGE__`
- Produces: evidence that stub AI path is exercised; live provider **NOT PROVEN**

- [ ] **Step 1: Run assistant-focused tests**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun test \
  src/features/assistant/assistant-bridge-client.test.ts \
  src/features/assistant/InvestigationAssistantPage.test.tsx
```

Expected: pass. Capture pass count (e.g. `N pass`).

- [ ] **Step 2: Optional session-ops gate (not live provider)**

```bash
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures
```

Expected: pass or document skip reason. Still label as command-layer.

- [ ] **Step 3: Update evidence**

Append to `docs/evidence/m23-guided-investigation.md`:

```markdown
## Stub AI closeout (YYYY-MM-DD)

**AI mode:** rules / stub only (Decision 3)
**Commands:** (paste bun test lines + counts)

### Pass
- Rules-mode Ask + fact/AI separation
- Provider-failure → rules fallback (mocked bridge)
- History bounds / opaque session ids as covered by existing tests

### NOT PROVEN
- Live external AI provider round-trip
- Packaged GUI Assistant click-through
- AI replaces MAT analysis (**false claim — do not make**)

### Terra subagent stand-in
- …

### Sol verdict (stub scope only)
- **Verdict:** met | not met
- Live provider remains **NOT PROVEN**
- **Date:** …
```

Also flip the prior “Final Terra milestone review + Sol M23 verdict | Not recorded” row to recorded for **stub scope**.

- [ ] **Step 4: STATUS.md** — M23: stub Sol met (if true); live provider still NOT PROVEN.

- [ ] **Step 5: Commit**

```bash
git add docs/evidence/m23-guided-investigation.md STATUS.md
git commit -m "$(cat <<'EOF'
docs(m23): stub AI guided investigation closeout evidence

Record rules/stub assistant gates; keep live provider NOT PROVEN.
EOF
)"
```

---

### Task 9: M23 — Terra + Sol; open PR (Track 3 or combined with M20)

- [ ] **Step 1: Terra subagent** focused on stub-vs-live honesty and fact/AI separation.

- [ ] **Step 2: Sol stub-scope verdict** in evidence (only if met).

- [ ] **Step 3: PR**

If combining with M20:

```bash
git push -u origin HEAD
gh pr create --title "docs(m20/m23): browser stub UI + stub AI closeout" --body "$(cat <<'EOF'
## Summary
- M20 browser stub workbench evidence (+ optional focused stub tests)
- M23 rules/stub assistant evidence
- Packaged GUI and live AI provider remain NOT PROVEN

## Test plan
- [ ] `cd ui && bun test` focused M20/M23 files listed in evidence
- [ ] Optional `bun run dev` curl smoke recorded
- [ ] Docs: no packaged/live-provider inference

EOF
)"
```

If separate, use titles from the Program sequence table.

---

### Task 10: M21 — Download v0.6.0 Windows desktop assets

**Files:**
- Ephemeral download dir: `/tmp/mnemosyne-m21/v0.6.0/` (not committed)
- Modify later: `docs/evidence/m21-portable-installability.md`

- [ ] **Step 1: List release assets**

```bash
gh release view v0.6.0 --repo "$(gh repo view --json nameWithOwner -q .nameWithOwner)" --json assets --jq '.assets[].name'
```

Expected names (frozen primaries / fallbacks from README):

- `Mnemosyne-0.6.0-windows-x64-portable.zip` (preferred)
- and/or `Mnemosyne_0.6.0_x64_en-US.msi`, `Mnemosyne_0.6.0_x64-setup.exe`
- `SHA256SUMS` if attached

- [ ] **Step 2: Download Windows assets + checksums**

```bash
mkdir -p /tmp/mnemosyne-m21/v0.6.0
cd /tmp/mnemosyne-m21/v0.6.0
gh release download v0.6.0 \
  --pattern 'Mnemosyne*windows*' \
  --pattern 'Mnemosyne*0.6.0*x64*' \
  --pattern 'SHA256SUMS*' \
  --clobber
ls -la
sha256sum Mnemosyne-0.6.0-windows-x64-portable.zip \
  Mnemosyne_0.6.0_x64_en-US.msi \
  Mnemosyne_0.6.0_x64-setup.exe 2>/dev/null | tee SHA256-local.txt
```

If a pattern misses, download by exact name from Step 1. Record hashes in evidence; prefer verifying against published `SHA256SUMS` when present:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

- [ ] **Step 3: Confirm WebView2 plan**

Note in evidence: Windows portable zip is **portable with WebView2 prerequisite**. If launch fails with a runtime error, install Evergreen WebView2 from Microsoft on the Windows host, then retry. Do not claim offline single-file drop-in.

No commit yet.

---

### Task 11: M21 — Install / launch / Open heap (Windows via powershell.exe)

**Files:**
- Ephemeral synthetic heap: `/tmp/mnemosyne-m21/synthetic-simple.hprof`
- Evidence updates in Task 12

- [ ] **Step 1: Materialize synthetic heap fixture (WSL)**

In `cli/tests/integration.rs`, temporarily add (near other `build_simple_fixture` tests; `build_simple_fixture` is already imported in that file):

```rust
#[test]
fn m21_write_synthetic_simple_fixture_to_tmp() {
    let bytes = build_simple_fixture();
    let path = std::path::Path::new("/tmp/mnemosyne-m21/synthetic-simple.hprof");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &bytes).unwrap();
    assert!(path.metadata().unwrap().len() > 0);
}
```

Run:

```bash
mkdir -p /tmp/mnemosyne-m21
cd /home/bballer09/Workspace/Mnemosyne
cargo test -p mnemosyne-cli --test integration m21_write_synthetic_simple_fixture_to_tmp -- --exact --nocapture
ls -la /tmp/mnemosyne-m21/synthetic-simple.hprof
sha256sum /tmp/mnemosyne-m21/synthetic-simple.hprof
```

**Do not commit** the temporary test — revert it before the M21 docs PR (`git checkout -- cli/tests/integration.rs`). Keep the `.hprof` only under `/tmp` / `C:\Temp`.

Copy the `.hprof` to a Windows-visible path (example if `/mnt/c` is available):

```bash
mkdir -p /mnt/c/Temp/mnemosyne-m21
cp /tmp/mnemosyne-m21/synthetic-simple.hprof /mnt/c/Temp/mnemosyne-m21/
cp /tmp/mnemosyne-m21/v0.6.0/Mnemosyne-0.6.0-windows-x64-portable.zip /mnt/c/Temp/mnemosyne-m21/
```

- [ ] **Step 2: Unzip / install via powershell.exe**

Portable zip path (preferred):

```bash
powershell.exe -NoProfile -Command "
  \$ErrorActionPreference = 'Stop'
  \$src = 'C:\\Temp\\mnemosyne-m21\\Mnemosyne-0.6.0-windows-x64-portable.zip'
  \$dst = 'C:\\Temp\\mnemosyne-m21\\portable'
  if (Test-Path \$dst) { Remove-Item -Recurse -Force \$dst }
  Expand-Archive -Path \$src -DestinationPath \$dst
  Get-ChildItem -Recurse \$dst | Select-Object FullName, Length
  \$exe = Join-Path \$dst 'Mnemosyne.exe'
  if (-not (Test-Path \$exe)) { throw \"Mnemosyne.exe not found under \$dst\" }
  Write-Output \"EXE=\$exe\"
"
```

MSI alternative (if zip missing):

```bash
powershell.exe -NoProfile -Command "
  Start-Process msiexec.exe -ArgumentList '/i C:\\Temp\\mnemosyne-m21\\Mnemosyne_0.6.0_x64_en-US.msi /qn' -Wait
"
```

Prefer portable zip for MAT-like unzip→click; record which asset was used.

- [ ] **Step 3: Launch**

```bash
powershell.exe -NoProfile -Command "
  \$exe = 'C:\\Temp\\mnemosyne-m21\\portable\\Mnemosyne.exe'
  \$p = Start-Process -FilePath \$exe -PassThru
  Start-Sleep -Seconds 5
  Write-Output \"PID=\$(\$p.Id) HasExited=\$(\$p.HasExited)\"
  Get-Process -Id \$p.Id -ErrorAction SilentlyContinue | Format-List Id,ProcessName,MainWindowTitle
"
```

If automation cannot complete Open-heap, perform **one manual** Open heap of `C:\Temp\mnemosyne-m21\synthetic-simple.hprof` and record honestly (“manual click”). Do not invent success.

- [ ] **Step 4: Record OS identity**

```bash
powershell.exe -NoProfile -Command "
  [System.Environment]::OSVersion.VersionString
  (Get-CimInstance Win32_OperatingSystem).Caption
  (Get-CimInstance Win32_OperatingSystem).Version
"
```

- [ ] **Step 5: Stop the app** when done (`Stop-Process` by PID). Leave macOS/Linux **not launch-tested**.

---

### Task 12: M21 — Evidence + Sol partial + PR

**Files:**
- Modify: `docs/evidence/m21-portable-installability.md`
- Modify: `docs/evidence/m21-m22-remaining.md`
- Modify: `STATUS.md` M21 bullet

- [ ] **Step 1: Update launch matrix rows**

In `docs/evidence/m21-portable-installability.md`, change the Windows portable (and MSI/exe if used) row:

| Platform asset | Launch-tested | Open heap dump |
| --- | --- | --- |
| Windows portable zip / MSI / setup (record which) | **yes** — OS caption/version, asset SHA-256, date | **yes** — synthetic `synthetic-simple.hprof` (manual or scripted) |
| macOS aarch64/x64 | **not launch-tested** | **not** |
| Linux AppImage x86_64/aarch64 | **not launch-tested** | **not** |

Add commands, hashes, WebView2 notes, and:

```markdown
## Sol verdict (partial)
- Windows launch + Open-heap: met
- macOS/Linux: open / not launch-tested
- Signing/notarization: **NOT PROVEN** / out of scope
- Date: …
```

Label Terra subagent stand-in if a docs review ran; M21 is primarily host evidence.

- [ ] **Step 2: STATUS.md** — M21 partial closeout: Windows launch-tested; other platforms open.

- [ ] **Step 3: Revert any temporary `m21_write_synthetic_*` test** before push.

- [ ] **Step 4: Commit + PR (Track 4)**

```bash
git add docs/evidence/m21-portable-installability.md \
  docs/evidence/m21-m22-remaining.md STATUS.md
git commit -m "$(cat <<'EOF'
docs(m21): Windows v0.6.0 launch and Open-heap partial evidence

Record OS/hash for published Windows assets; leave macOS/Linux untested.
EOF
)"
git push -u origin HEAD
gh pr create --title "docs(m21): Windows-only portable launch partial closeout" --body "$(cat <<'EOF'
## Summary
- Download/install/launch v0.6.0 Windows desktop asset
- Open synthetic heap once; record OS + SHA-256
- macOS/Linux remain not launch-tested; Sol partial

## Test plan
- [ ] Evidence hashes match `gh release download` assets
- [ ] No signed-installer claim
- [ ] No macOS/Linux launch inference

EOF
)"
```

---

### Task 13: jsdom 30 trial (optional; separate PR)

**Files:**
- Modify: `ui/package.json` (`"jsdom": "30.0.1"` exact trial pin)
- Modify: `ui/bun.lock`
- Modify: `docs/product/dependency-currency-inventory.md`
- Optionally: `STATUS.md` M32 jsdom line
- Reference ceilings: `ui/run-tests-config.ts` (`rssCeilingMiB` per batch)

- [ ] **Step 1: Branch-only bump**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
# record baseline first
bun run test 2>&1 | tee /tmp/jsdom24-rss.txt
# bump
bun add -d jsdom@30.0.1
```

- [ ] **Step 2: Measure RSS under M30 ceilings**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun run test 2>&1 | tee /tmp/jsdom30-rss.txt
rg '\[ui-test-rss\]|exceeded RSS ceiling|error:' /tmp/jsdom30-rss.txt
```

Expected decision rule:

- **Land** only if every batch prints `peak=…` under its `rssCeilingMiB` and exit code 0 within timeouts.
- **Refuse** on hang (>2 min after last progress), SIGKILL, or any `exceeded RSS ceiling` — revert immediately.

- [ ] **Step 3a: Land path**

```bash
# keep jsdom 30.0.1
# update inventory row to current; remove M32.C exception
git add ui/package.json ui/bun.lock docs/product/dependency-currency-inventory.md STATUS.md
git commit -m "$(cat <<'EOF'
chore(ui): land jsdom 30 under documented RSS ceilings

Measured batch peaks remain within M30 ui-test-rss gates.
EOF
)"
```

Attach `/tmp/jsdom30-rss.txt` peak lines into a short note under `docs/evidence/m30-product-hardening.md` or inventory “Evidence” section.

- [ ] **Step 3b: Refuse path**

```bash
cd /home/bballer09/Workspace/Mnemosyne/ui
bun add -d jsdom@24.1.1
# restore lock if needed: git checkout -- ui/bun.lock && bun install
```

Update inventory exception with **new measurement date** and peak/hang observation. Keep jsdom `^24.1.1`.

```bash
git add docs/product/dependency-currency-inventory.md
git commit -m "$(cat <<'EOF'
docs(m32): retain jsdom 24 after measured jsdom 30 RSS refusal

Record trial peaks/hang; do not block other closeout tracks.
EOF
)"
```

- [ ] **Step 4: PR (Track 5)** — either land or measured refusal; never leave the tree half-bumped.

---

## Execution notes

- Prefer stacked or sequential PRs in program order; do not open M31 work.
- Before each commit on code-touching tasks, run applicable focused tests only; full workspace suites are optional on constrained WSL.
- After merges that change symbols, refresh GitNexus if required by repo hooks (`npx gitnexus analyze`); this closeout is mostly docs — skip unless Rust/UI symbols changed.
- Label every evidence Terra section as **subagent stand-in**.

---

## Self-review vs approved spec

Checklist run after writing this plan (writing-plans skill self-review):

| Spec requirement | Plan coverage |
| --- | --- |
| §2 Decision 1 — browser stub bridges only | Tasks 5–7 |
| §2 Decision 2 — localhost UI allowed | Task 6 |
| §2 Decision 3 — rules/stub AI | Tasks 8–9 |
| §2 Decision 4 — Windows packaged smoke | Tasks 10–12 |
| §2 Decision 5 — Terra via subagent | Tasks 4, 7, 9, 12 |
| §2 Decision 6 — Windows-only M21 matrix | Tasks 11–12 (macOS/Linux explicit not launch-tested) |
| §3 / §7.1 M22.E corpus + docs honesty | Tasks 1–4 |
| §3 / §7.2 M20 browser stub | Tasks 5–7 |
| §3 / §7.3 M23 stub AI | Tasks 8–9 |
| §3 / §7.4 M21 Windows launch + Open-heap | Tasks 10–12 |
| §3 / §7.5 jsdom 30 RSS trial | Task 13 |
| §4 Non-goals (no M31+, no MAT golden claim, no live AI, no macOS/Linux claim) | Global Constraints + evidence NOT PROVEN rows |
| §5 Sequence order | Program sequence table |
| §6 Evidence standard fields | Each track’s evidence steps |
| §8 One PR per track (M20+M23 optional combine) | Program sequence + Tasks 7/9 |
| §9 Success criteria | Sol verdict steps per track |
| §10 Risks (stub over-claim, WSL GUI flaky, jsdom OOM, Terra label) | Global Constraints + Task 6/11/13 mitigations |
| Placeholder scan | No TBD/TODO; temporary fixture test explicitly reverted |
| Type/name consistency | Bridge keys match `reset-test-state.ts`; asset names match README frozen primaries |

**Gaps found during self-review:** none blocking. Optional localhost smoke (Task 6) and temporary fixture dump test (Task 11) are explicitly optional/reverted so they cannot leave product debt.