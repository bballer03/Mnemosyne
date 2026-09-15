# M31.B MAT golden / equivalence evidence

**Date:** 2026-09-16
**Scope:** Wave 1 synthetic golden harness
**Verdict:** **MNEMOSYNE GOLDEN ESTABLISHED; ECLIPSE MAT CROSS-CHECK NOT PROVEN**

## What landed

- `core/tests/mat_golden.rs` runs deterministic core analysis and OQL against the repository's `test-fixtures` builders.
- Expected normalized JSON is versioned under `core/tests/golden/expected/`.
- Volatile analysis fields (`elapsed`, `generated_at`, temporary heap path) are removed or replaced before comparison.
- CI runs the harness through `cargo test --workspace --features test-fixtures`.
- Golden updates are explicit: review the semantic change, then run the focused test with `UPDATE_MNEMOSYNE_GOLDENS=1`.

## Corpus count

- Total golden cases: **3**
- `mnemosyne-golden`: **3**
- `mat-referenced`: **0**
- Analysis cases: **1**
- OQL cases: **2**

The cases use `build_graph_fixture()` and `build_simple_fixture()` and freeze deep-analysis, object-row projection, and retained-size query results.

## Verification

```text
cargo test -p mnemosyne-core --features test-fixtures --test mat_golden
running 1 test
test selected_analysis_and_oql_outputs_match_versioned_goldens ... ok
test result: ok. 1 passed; 0 failed
```

## Honesty boundary

No Eclipse MAT installation or recorded MAT output was available for this run. Therefore:

- no case is labeled `mat-referenced`;
- this evidence proves Mnemosyne regression stability only;
- semantic equivalence with a specific Eclipse MAT version remains **NOT PROVEN**;
- the M7-5 native reference-workstation comparison, including MAT and large fixtures, remains open.
