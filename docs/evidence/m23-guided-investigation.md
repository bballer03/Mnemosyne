# M23 guided investigation — usability evidence

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-14  
**Plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) § M23 (Slices 23.A–23.D)  
**Design:** [docs/design/milestone-23-ai-first-guided-investigation.md](../design/milestone-23-ai-first-guided-investigation.md)  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records what M23 *shipped* and what remains **NOT proven** on this WSL host. Claims come from git history, focused automated tests, and a **synthetic** end-to-end narrative — not from a packaged GUI launch or a live external AI provider call.

## What shipped (commit refs)

| Slice / theme | Commit | Summary |
| --- | --- | --- |
| Investigation session workspace | `f3ad203` | `/assistant` rules-mode default, fact/AI separation, deep links, basename opacity |
| Terra Important findings on 23.A | `22ff57b` | Non-finite history rejection; hide path-like source ids; no premature provider claim |
| Desktop workflow get/close/resume | `532df7c` | Thin adapters + WorkflowCard resume/close; `classloader_leak` continuity |
| Bounded chatSession providers | `3ab1ef5` | Tauri create/resume/get/close/chat over shipped MCP AI-session semantics; rules fallback |

## Synthetic end-to-end scenario (narrative)

Fixture context used by focused UI tests: artifact `fixture.json` with heap display name `fixture.hprof` (absolute path `/secret/path/fixture.hprof` never rendered), objects `42`, leaks `leak-high` (`com.example.High`, HIGH) and `leak-low`.

1. **Open heap / load facts** — User lands on Investigation session with measured facts showing basename `fixture.hprof`, object count, and focus defaulted to the highest-score leak (`leak-high`). Absolute path absent from the facts panel.
2. **Find a suspect** — Focus selector lists both leaks; changing focus updates the measured-facts panel only (no AI text mixed in).
3. **Follow a deterministic GC path** — From the session nav, open `/leaks/leak-high/gc-path` (and `/leaks/leak-high/overview`, Object Inspector, Dominators, Query Console, Dashboard). These are power-view routes, not chat answers.
4. **Ask for an explanation (rules / AI-off)** — Ask “What should I check first?” with no assistant host bridge. Answer appears only in the AI guidance region with `Provenance: rules · model rules`, citing focus metadata. Measured-facts region remains free of the question text.
5. **Provider failure remains useful** — With a bridge that times out / errors, Ask falls back to rules and shows machine-readable recovery (`recovery=rules_mode_available; …`). Offline Ask still works after “Check provider availability” reports unavailable.
6. **Return to the exact object view** — User follows Object Inspector / Leak Workspace / GC Path links for the focused leak id and continues deterministic investigation.

**Honesty:** Steps 1–6 are exercised as **component/unit** paths (React + mocked bridge + session-ops). They are **not** a recorded packaged-desktop click-through on this host.

## Synthetic transcripts (deterministic / rules / provider-failure)

### A. Deterministic power-view routing (no AI)

```
nav: Dashboard | Object Inspector | Dominators | Query Console | Leak Workspace | GC Path
focus: leak-high
assert: every Ask-adjacent action either deep-links to a power route or states unsupported
```

Observed in `InvestigationAssistantPage` + `InvestigationAssistantPage.test.tsx` (“exposes deterministic deep links…”).

### B. Rules mode (AI-off / offline default)

```
Q: What should I check first?
A: <rules summary citing leak-high / com.example.High / high severity cache>
Provenance: rules · model rules
notice: (none — no bridge required)
```

Observed in page + `assistant-bridge-client` rules-mode tests. Basename-only heap display; no `/secret/path`.

### C. Provider-failure / unavailable (rules remain useful)

```
Check provider availability → "Provider chat is unavailable without a connected assistant host bridge…"
Q: Still works offline?
A: <rules summary>
Provenance: rules

# when bridge chatSession rejects / times out:
recovery=rules_mode_available; error=<timeout|provider error>
Provenance: fallback (rules)
```

Observed in page timeout/fallback tests and `askWithProviderFallback` unit tests. **No live OpenAI/Anthropic HTTP call was made for this evidence note.**

### D. Provider success path (mocked bridge only)

```
bridge.createAiSession → session_id (opaque)
bridge.chatSession → summary + model
Provenance: provider
outbound notice: summary/focus metadata only; API keys never printed
```

Observed with an in-test `__MNEMOSYNE_ASSISTANT_BRIDGE__` stub. Treat as adapter wiring proof, **not** live-provider quality evidence.

## Assistant action → power view / unsupported map

| Action | Power-view route or state |
| --- | --- |
| Dashboard link | `/dashboard` |
| Object Inspector | `/heap-explorer/object-inspector` |
| Dominators | `/heap-explorer/dominators` |
| Query Console | `/heap-explorer/query-console` |
| Leak Workspace (when focus set) | `/leaks/:leakId/overview` |
| GC Path (when focus set) | `/leaks/:leakId/gc-path` |
| Ask (rules) | Advisory panel only — not a substitute for MAT/histogram/inspector analysis |
| Ask (provider / fallback) | Same advisory panel + provenance; rules always available on failure |
| Check provider availability | Explicit available / unavailable notice |
| Active workflow id/step in session panel | **Unsupported / not live-bound yet** — UI shows “none active in this workspace yet” |

## Command-layer evidence (observed this closeout)

- Focused UI: `bun test` on `assistant-bridge-client.test.ts`, `InvestigationAssistantPage.test.tsx`, `TopNav.test.tsx` — **24 pass**.
- Desktop session-ops: `cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures` — **38 pass** (includes AI session 12/32 eviction, provider error surfacing, workflow get/close/`classloader_leak`).
- `npx gitnexus detect_changes --scope all` — ran; docs/evidence closeout does not claim analyzer symbol changes.
- Evidence class: **unit / component / session-ops**. Not packaged GUI smoke.

## Honest caveats (do not over-claim)

- AI guidance is **advisory**. It does **not** replace MAT-equivalent histogram, dominator, inspector, OQL, or leak analysis.
- No bundled model runtime; provider mode requires configured host + credentials outside the UI.
- Live workflow id/step binding into the Investigation session panel remains open from 23.A notes.
- Full workspace `cargo test` / full `bun run test` / `cargo check --manifest-path tauri/Cargo.toml` were **not** re-run as a single green M23.D matrix on this host (Tauri check remains WebKitGTK-blocked; prior slices noted a pre-existing ArtifactExplorer multi-match failure unrelated to assistant).

## NOT proven on this WSL host

| Claim | Status | Where it belongs |
| --- | --- | --- |
| Packaged Tauri GUI smoke (launch window, open heap, click Assistant, GC Path, Ask) | **NOT run** — WebKitGTK/GTK bundler deps absent | Native-host / M21-class evidence |
| Live external AI provider round-trip (real API key, network call, production model quality) | **NOT evidenced** here — only mocked bridge + rules/fallback | Operator-configured provider validation |
| AI replaces or equals Eclipse MAT analysis depth | **False claim — do not make** | Capability matrix + honesty bar |
| Full Rust / full UI / full Tauri gate as one closeout run | **Not claimed** | Release discipline / native hosts |
| Final Terra milestone review + Sol M23 verdict | **Not recorded** | Requested in 23.D closeout commit; pending human/Sol |

## How to read this evidence

1. Product capability status → [ui-capability-matrix.md](../product/ui-capability-matrix.md).
2. Install / unzip / launch claims → README Desktop + M21; never infer GUI success from WSL unit green.
3. AI / MAT honesty → rules default, provenance labels, and power-view deep links are the product; chat is guidance only.
