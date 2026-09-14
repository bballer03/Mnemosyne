# Milestone 22 — Bounded MAT OQL and Operator Polish

**Status:** in progress (Slice 22.A corpus first slice; 22.B/22.C shipped)  
**Plan:** [docs/superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md](../superpowers/plans/2026-09-14-ui-first-mat-install-ai-plan.md) § M22  
**Branch:** `sync/m15g-m16bcd`

## Honesty rule

Only **MAT-referenced** corpus cases may close a compatibility-matrix row or support a MAT-equivalency claim. Mnemosyne-only bounds (hop caps, list-size caps, `LIMIT` truncation) are recorded as **non-equivalency** and must not be marketed as MAT parity.

## Slice 22.A — Sanitized compatibility corpus

**Artifacts:**

| Path | Role |
| --- | --- |
| `core/tests/fixtures/oql/mat-compatibility.json` | Versioned synthetic case catalog |
| `core/tests/oql_compatibility_corpus.rs` | Thin runner: parse + targeted execute checks |
| `core/tests/query_parser.rs` / `query_executor.rs` | Behavioral depth; MAT refs in comments |

**Case fields:** `id`, `status` (`shipped` \| `unsupported`), `equivalency` (`mat-referenced` \| `non-equivalency`), MAT handbook links/section names, query text, expected parse/execute/error outcome, result budget notes.

**Required coverage (first slice):** multi-class `FROM`, 1–3 hop `OBJECTS`, 4-hop rejection, duplicate targets, null/missing fields, cycles, budget exhaustion (`LIMIT` truncation).

**Non-goals for 22.A:** customer queries, live MAT golden dumps, claiming full OQL equality, a second query engine.

## MAT handbook anchors (Eclipse help)

- [Querying Heap Objects (OQL)](https://help.eclipse.org/latest/topic/org.eclipse.mat.ui.help/tasks/queryingheapobjects.html)
- [FROM Clause](https://help.eclipse.org/latest/topic/org.eclipse.mat.ui.help/reference/oqlsyntaxfrom.html) — multi-class addresses/ids; `INSTANCEOF`; `FROM OBJECTS (...)`
- [SELECT Clause](https://help.eclipse.org/latest/topic/org.eclipse.mat.ui.help/reference/oqlsyntaxselect.html) — `SELECT OBJECTS ...`; `DISTINCT OBJECTS`
- [BNF for OQL](https://help.eclipse.org/latest/topic/org.eclipse.mat.ui.help/reference/bnfofoql.html)

## Syntax deltas (explicit)

| Topic | MAT | Mnemosyne (bounded) |
| --- | --- | --- |
| Multi-class source | Comma-separated class object addresses/ids, or `UNION` | Comma-separated quoted class patterns (max 8), ID dedup |
| `SELECT OBJECTS` path | Field / expression projection; nulls skipped | 1–3 object-ref hops; 4+ structured reject |
| Result budget | GUI/result-set practical limits | Explicit `LIMIT` + truncation flag |

## Shipped vs open

- **Shipped:** 22.B multi-class `FROM`, 22.C 1–3 hop `OBJECTS` (+ 4-hop reject), bounded `SELECT DISTINCT OBJECTS` (OBJECTS-only; `SELECT DISTINCT *` rejected).
- **Open / unsupported (named gaps):** `eval(...)`, arbitrary-depth recursion, Java/JS execution, live attach, unbounded hop chains, general `SELECT DISTINCT *` / field-list DISTINCT.
