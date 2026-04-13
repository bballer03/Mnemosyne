# M3 Final Closeout Design

> Status: approved via autonomous execution directive
> Date: 2026-04-13
> Parent: `docs/superpowers/specs/2026-04-13-roadmap-closeout-design.md`

## Goal

Finish the remaining evidence-backed M3 work by shipping the missing benchmark automation and the smallest useful query-depth expansion, then close the milestone docs around the remaining scale levers as explicitly not justified by current evidence.

## Why This Slice

After M3-A and M3-B, the remaining M3 backlog is narrower than the historical milestone design still suggests.

Current runtime truth shows:

- benchmark infrastructure already exists in pieces: Criterion benches, RSS measurement scripts, dense synthetic validation scripts, and published performance docs
- the shipped query surface already parses more than it executes: `FieldRef::InstanceField` and `ComparisonOp::InstanceOf` exist in the parser/types, but the executor currently returns `Null` for instance fields and does not honor hierarchy-aware `INSTANCEOF`
- overview mode, threaded I/O, and `nom` are now backlog levers that should only move if future profiling proves the current architecture insufficient

That means the remaining M3 work is no longer “finish all historical M3 proposals.” It is “complete the still-missing shipped-scope follow-through and close the rest honestly.”

## Approaches Considered

### 1. Docs-only closure

Mark the remaining items as future work and end M3 without any more code changes.

Pros:

- smallest possible change set
- no new runtime risk

Cons:

- leaves the query engine in a visibly half-wired state
- leaves benchmark automation still missing even though the roadmap still tracks it as an M3 follow-through item

### 2. Minimal evidence-backed closeout (recommended)

Ship only the pieces that are already clearly scaffolded in the codebase and docs:

- benchmark automation wrappers around the existing scripts, with graceful tool detection for `hyperfine` and `heaptrack`
- query depth limited to two already-modeled capabilities: instance field projection/filtering and hierarchy-aware `INSTANCEOF`
- explicit milestone-doc closure for the evidence-only scale levers

Pros:

- finishes the remaining concrete M3 gaps
- keeps the change set small and coherent
- avoids reopening superseded overview-mode architecture debates

Cons:

- still leaves fuller MAT-style OQL as future work
- benchmark automation may be partially tool-gated on non-Linux/non-profiling environments

### 3. Broad M3 expansion

Try to implement richer OQL, overview mode, threaded I/O, and parser evaluation together before closing M3.

Pros:

- would reduce future backlog size

Cons:

- reopens evidence-driven items as if they were required scope
- too large and risky for a final closeout batch

## Chosen Approach

Use the minimal evidence-backed closeout approach.

This batch has three parts:

1. **Benchmark automation** — keep existing `measure_rss.sh` and `run_step11_scaling_validation.sh`, but add lightweight automation wrappers/documentation for `hyperfine` and `heaptrack` only when the tools are present. Missing tools must be reported clearly rather than treated as failures.
2. **Query follow-through** — wire the shipped query parser/types through the executor for instance field access and hierarchy-aware `INSTANCEOF`, with focused tests and CLI/MCP contract updates.
3. **M3 doc closure** — update milestone/roadmap docs so richer future OQL remains future work, and overview mode/threaded-I/O/`nom` are explicitly recorded as not currently justified by evidence.

## Scope

### In scope

- shell automation around existing benchmark scripts
- tool detection and truthful fallback behavior for `hyperfine` / `heaptrack`
- documentation of benchmark automation usage and current environment limitations
- query executor support for instance field projection/filtering when field data is available
- hierarchy-aware `INSTANCEOF` matching in query evaluation
- focused parser/executor/CLI/MCP tests for the deeper query slice
- milestone/roadmap/doc sync needed to close M3 honestly

### Out of scope

- overview/deep/auto mode implementation
- threaded I/O pipeline work
- parser-engine replacement or `nom` migration
- object-level diffing
- general interactive explorer work (M4)
- broad MAT-complete OQL semantics beyond the chosen minimal slice
- mandatory installation of `hyperfine` or `heaptrack`

## File-Level Design

### Benchmark automation surface

- `scripts/measure_rss.sh`
  - keep current RSS measurement behavior intact
- `scripts/run_step11_scaling_validation.sh`
  - keep current dense synthetic validation flow intact
- new helper script(s) under `scripts/`
  - wrap `hyperfine` for end-to-end CLI timing when available
  - wrap `heaptrack` profiling when available and clearly skip otherwise
- shell smoke tests under `scripts/tests/`
  - verify the automation scripts degrade cleanly when the optional tools are absent

### Query surface

- `core/src/query/executor.rs`
  - resolve `FieldRef::InstanceField` using the existing `read_field` / `read_all_fields` object-graph helpers
  - implement hierarchy-aware `INSTANCEOF` matching for `FROM INSTANCEOF ...` and `WHERE <field> INSTANCEOF ...` where the left side resolves to an object reference
- `core/src/query/types.rs`
  - extend `CellValue` only if needed to faithfully compare or render object-reference-backed field values
- `core/tests/query_parser.rs`
  - add parser coverage for `INSTANCEOF` forms that are meant to be supported
- `core/tests/query_executor.rs`
  - add execution coverage for instance fields and hierarchy-aware matching
- `core/src/mcp/server.rs` and `docs/api.md`
  - update only if result-shape or documented query semantics need clarification after the executor change
- `cli/src/main.rs` and CLI docs
  - only if the user-facing query help/examples need to reflect the deeper supported subset

## Verification Strategy

### Behavior verification

- TDD for the new query behavior: failing tests first, then minimal executor changes
- focused query parser/executor tests
- existing CLI query path remains green

### Batch verification

- targeted script smoke tests for the benchmark wrappers
- `cargo check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

## Risks

- field access can only work when the parsed object retains field data; query behavior must stay truthful when data is absent
- `INSTANCEOF` semantics can be misleading if they silently degenerate to exact-match behavior; tests and docs must make the supported subset explicit
- benchmark automation can easily become environment-fragile; optional-tool paths must skip cleanly and explain why
- milestone docs must not overclaim full MAT-equivalent OQL just because this batch deepens the shipped subset

## Decision

Proceed with one final M3 closeout batch in this order:

1. benchmark automation wrappers plus graceful tool detection
2. minimal real query-depth expansion via TDD
3. final M3 verification and milestone-doc closure
