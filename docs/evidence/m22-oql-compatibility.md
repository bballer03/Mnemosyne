# M22.E OQL compatibility corpus — evidence

**Date:** 2026-09-15  
**Branch:** `docs/older-partials-closeout-spec`  
**Tip:** `81a75b8`
**Corpus:** `core/tests/fixtures/oql/mat-compatibility.json` (version 1)  
**Runner:** `cargo test -p mnemosyne-core --test oql_compatibility_corpus`  
**Spec:** [docs/superpowers/specs/2026-09-15-older-partials-closeout-design.md](../superpowers/specs/2026-09-15-older-partials-closeout-design.md)  
**Plan:** [docs/superpowers/plans/2026-09-15-older-partials-closeout.md](../superpowers/plans/2026-09-15-older-partials-closeout.md)

## Commands

```bash
cargo test -p mnemosyne-core --test oql_compatibility_corpus -- --nocapture
# inventory script (status/equivalency/category buckets from corpus JSON)
node -e '/* see plan Task 1 Step 1 */'
cargo test -p mnemosyne-core --test query_parser
cargo test -p mnemosyne-cli --test integration test_query_command
cargo test -p mnemosyne-core handle_request_query_heap
```

## Pass / fail counts (corpus JSON)

| Bucket | Count | Notes |
| --- | --- | --- |
| Total cases | 13 | |
| `status=shipped` | 12 | Behavioral expectations enforced by runner |
| `status=unsupported` | 1 | `unsupported-eval` |
| `equivalency=documentation-referenced` | 7 | Handbook-linked; **not** MAT golden |
| `equivalency=non-equivalency` | 6 | Mnemosyne bounds / syntax deltas |
| `equivalency=mat-referenced` | 0 | **NOT PROVEN** — no recorded MAT golden dumps |

### Category breakdown

| Category | Count |
| --- | --- |
| multi-class-from | 1 |
| objects-hops | 3 |
| objects-hop-limit | 1 |
| duplicates | 1 |
| distinct-objects | 2 |
| null-missing | 2 |
| cycles | 1 |
| budget-exhaustion | 1 |
| unsupported-syntax | 1 |

## Test binary result

- `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- Includes `only_mat_referenced_shipped_cases_are_equivalency_eligible` (asserts **zero** `mat-referenced` golden cases).

## Named unsupported / open rows

| Case id / row | Status | Why open |
| --- | --- | --- |
| `unsupported-eval` | unsupported | `eval(...)` scriptlets deferred |
| Arbitrary-depth subquery nesting | open (named deferral) | Beyond one-level M15 nesting |
| Live JVM attach / MAT `.index` interchange | open | Out of M22 scope |
| Eclipse MAT golden dump equivalency | **NOT PROVEN** | No `mat-referenced` corpus cases with versioned MAT results |

## CLI / MCP equivalence

Both CLI `query` (`cli/src/main.rs` → `execute_query`) and MCP `query_heap` (`core/src/mcp/server.rs` → `execute_query`) normalize through shared `parse_query` + `execute_query`. No second query engine.

| Grammar form | Corpus / core | CLI | MCP | Notes |
| --- | --- | --- | --- | --- |
| multi-class `FROM` | shipped corpus + parser/executor | shared engine | shared engine | Corpus case `multi-class-from-two-literals` |
| 1–3 hop `OBJECTS` | shipped corpus | shared engine | shared engine | `objects-one/two/three-hop` |
| 4-hop reject | shipped (non-equivalency) | shared engine | shared engine | `objects-four-hop-reject` |
| `SELECT DISTINCT OBJECTS` | shipped corpus | shared engine | shared engine | distinct collapse + reject-without-OBJECTS |
| CLI `query` smoke | — | **pass** (`test_query_command_*` 2/2) | — | Existing integration path |
| MCP `query_heap` smoke | — | — | **pass** (`handle_request_query_heap_*`) | Existing unit path |

## Honesty

- Unit/corpus green ≠ Eclipse MAT golden equivalency.
- Do not extrapolate a percentage “MAT parity” beyond these counts (13 cases; 0 mat-referenced).
- Packaged GUI / MAT golden remain **NOT PROVEN**.

## Terra subagent stand-in

- Reviewer: [cavecrew-reviewer](09dbd3fb-0ba9-4817-aa65-aa275bf519fb) (Terra stand-in)
- Focus: corpus honesty labels, no mat-referenced inflation, docs match counts, stale deferral text
- First pass: **CHANGES REQUIRED** (stale M15/roadmap deferrals; design equivalency enum; tip/metadata)
- Follow-up: all 🔴/🟡 findings applied in the same closeout commit set
- Outcome: **APPROVED** after fixes (Sol confirms honesty bar met)

## Sol verdict

- **Scope:** M22.E corpus closeout (bounded OQL evidence publish)
- **Verdict:** **met with named remaining gaps** (`eval(...)`, arbitrary-depth nesting, 0 mat-referenced / MAT golden **NOT PROVEN**)
- **Date:** 2026-09-15
- **Authority:** Sol sign-off authorized by product owner for this closeout program
