# M3-A Small Closeout Design

> Status: approved via autonomous execution directive
> Date: 2026-04-13
> Parent: `docs/superpowers/specs/2026-04-13-roadmap-closeout-design.md`

## Goal

Close the remaining small M3 items with the smallest safe change set: a version-qualified README status badge, real usage examples under `docs/examples/`, and IntelliJ-friendly thread stack formatting for the existing `analyze --threads` text output.

## Why This Slice

The active roadmap already narrows M3 follow-through to a short list of low-risk closeout items before deeper benchmark, query, or scaling work. These items are intentionally smaller than the later M3 follow-through batches and do not require reopening M4, M5, or M6 architecture decisions.

The current codebase shows three concrete gaps:

- `README.md` still uses a generic `alpha` status badge instead of the already-documented `v0.2.0-alpha` qualifier.
- `docs/examples/` is still a lightweight landing page rather than a set of real CLI/MCP walkthroughs.
- `mnemosyne-cli analyze --threads` already prints stack frames, but frames without source metadata do not consistently use canonical Java stacktrace syntax, which weakens IntelliJ "Analyze Stacktrace" compatibility.

## Chosen Approach

### 1. Badge qualifier

Update the existing README status badge only. The new displayed value should be `v0.2.0-alpha`, matching the current roadmap and milestone-design references.

This stays intentionally narrow:

- no release-process changes
- no new version source of truth
- no new badge types or extra badges

### 2. Real examples

Turn `docs/examples/` into a small but real examples area instead of a placeholder. Keep the batch focused on two example documents plus an updated index page:

- `docs/examples/cli-analysis-workflow.md`
- `docs/examples/mcp-stdio-workflow.md`
- `docs/examples/README.md`

The CLI workflow example should cover a realistic operator path across already-shipped commands: `parse`, `leaks`, `analyze`, `diff`, `explain`, `map`, and `gc-path`, with short guidance on when to use each and explicit provenance/fallback notes.

The MCP example should stay stdio-first and use only live methods. It should show `list_tools`, basic heap analysis calls, and one small AI-session follow-up example so the examples reflect the actual shipped MCP surface.

To keep M3-A small, this batch should not add sample heap dumps, example applications, or a large cookbook. Those remain M6-scale work.

### 3. IntelliJ stacktrace compatibility

Improve the existing thread-stack rendering in `mnemosyne-cli analyze --threads` rather than adding a new command or output mode.

The minimal compatibility target is canonical Java-style frame lines:

- `at pkg.Class.method(File.java:123)` when source file and positive line number are known
- `at pkg.Class.method(File.java)` when the source file is known but the line number is not usable
- `at pkg.Class.method(Unknown Source)` when no source file is known
- `at pkg.Class.method(Compiled Method)` when `line_number == -2`, matching the existing internal M3 Phase 2 design note

The surrounding thread-report header can remain Mnemosyne-specific. The compatibility goal is the frame-line syntax itself, because that is what IntelliJ consumes.

## Scope

### In scope

- `README.md` status badge qualifier
- new real workflow docs under `docs/examples/`
- `docs/examples/README.md` as a lightweight index to those workflows
- `cli/src/main.rs` stack-frame text formatting for `analyze --threads`
- any minimal fixture/test additions needed to verify the new thread-stack rendering
- minimal documentation sync required by these changes

### Out of scope

- new CLI commands or flags
- new MCP methods
- HTML/JSON/TOON stacktrace formatting changes
- sample `.hprof` binaries or example applications
- broader thread-dump or explorer UX work
- OQL/query, benchmark, scale, Docker/security, or UI milestone work

## File-Level Design

### Docs

- `README.md`
  - change the existing status badge value to `v0.2.0-alpha`
- `docs/examples/README.md`
  - convert from landing page to short index
- `docs/examples/cli-analysis-workflow.md`
  - new CLI walkthrough using shipped commands only
- `docs/examples/mcp-stdio-workflow.md`
  - new MCP stdio walkthrough using live methods only

### CLI / test surface

- `cli/src/main.rs`
  - keep `print_thread_stacks()` as the single rendering point for text-mode thread stacks
  - normalize missing-source and compiled-method cases to Java-style parenthesized suffixes
- `cli/tests/integration.rs`
  - add a focused assertion that `analyze --threads` emits IntelliJ-friendly stack lines
- `core/src/hprof/test_fixtures.rs`
  - add one dedicated thread-stack-capable fixture if the current graph fixture is too shallow for the new CLI assertion

## Verification Strategy

### Behavior verification

- add/extend a CLI integration test that asserts rendered stack lines contain canonical `at ...(...)` syntax
- keep the change limited to text output of `analyze --threads`

### Batch verification

- targeted CLI integration test for the new thread-stack rendering
- broader `cargo test`
- targeted doc searches for the new example/index links and badge qualifier

## Risks

- Duplicating command examples across `README.md`, `docs/QUICKSTART.md`, and `docs/examples/` can reintroduce drift. The new examples should stay scenario-oriented and avoid re-copying every reference snippet verbatim.
- Thread-stack compatibility should not overreach into unsupported special line-number semantics. This batch should only implement the cases already supported by current parsed data and the existing M3 Phase 2 design note.
- `docs/QUICKSTART.md` already has unrelated MCP-session drift; only touch it if a direct contradiction becomes necessary to resolve in this batch.

## Decision

Proceed with one M3-A batch in this order:

1. README badge qualifier
2. real `docs/examples/` workflow docs
3. IntelliJ-friendly thread-stack rendering via the existing `analyze --threads` text path
4. minimal follow-up doc sync and verification
