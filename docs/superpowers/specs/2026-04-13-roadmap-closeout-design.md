# Roadmap Closeout Design

> Status: approved via autonomous execution directive
> Date: 2026-04-13
> Scope: remaining roadmap work from M3 through M6 in ordered unattended execution

## Goal

Close the remaining roadmap work in a safe sequence that does not reopen already shipped milestones, does not mix unrelated architecture changes into one batch, and keeps roadmap, design docs, implementation, tests, and user-facing documentation aligned at every step.

## Why This Slice

The current `origin/main` codebase already contains substantially more shipped work than some roadmap and milestone design documents acknowledge. In particular:

- M3 core parity is largely complete, with only a small set of follow-through items still open.
- M5 is effectively complete for the currently approved scope, while a few evidence-driven follow-ons remain optional or partial.
- M4 and M6 are still legitimately open and large.

Attempting to execute all remaining M3, M4, M5, and M6 work as one monolithic implementation would be unsafe because it would:

- blur milestone boundaries
- mix docs-only, security-only, benchmark, UI, AI, and ecosystem work
- make testing and review attribution unclear
- increase the chance of roadmap/documentation drift while coding

The smallest correct next move is therefore a design/doc alignment batch followed by decomposed subprojects executed in strict order.

## Chosen Approach

Use a decomposed roadmap-closeout program with one design/plan/implementation cycle per subproject.

Execution order:

1. Phase 0: roadmap and milestone-design alignment
2. M3 closeout: smallest remaining items first
3. M3 deeper follow-through: benchmarking, scale, and query gaps
4. M4 interactive HTML and local web UI
5. M5 evidence-driven follow-on work only
6. M6 docs, examples, benchmarks, integrations, community, and plugin decisions

Each subproject must follow the repository's required order:

1. design gate
2. scoped decomposition
3. owner assignment
4. tool grants
5. edits
6. tests
7. static analysis
8. documentation sync
9. consolidation

## Non-Goals

- Reopening completed M1, M1.5, or M2 work
- Re-documenting already shipped behavior as future work
- Adding MCP streaming without evidence that the current request/response transport is insufficient
- Bundling M4 UI work into M3 engine changes unless a true dependency requires it
- Starting the M6 plugin system before the documentation/examples/benchmark/community foundations are in place
- Broad speculative refactors outside the currently active batch

## Subproject Breakdown

### Phase 0: Roadmap and Design Truth-Sync

Purpose:

- update roadmap and milestone design docs so they reflect shipped M3/M5 work accurately
- identify the true remaining M3/M4/M5/M6 scope
- make later autonomous implementation safe

Primary files:

- `docs/roadmap.md`
- `docs/design/milestone-3-core-heap-analysis-parity.md`
- `docs/design/M3-phase2-analysis.md`
- `docs/design/milestone-5-ai-mcp-differentiation.md`
- focused addenda under `docs/superpowers/specs/` and `docs/superpowers/plans/`

Definition of done:

- M3 and M5 statuses in roadmap/design docs match the current codebase
- the remaining work is described as decomposed subprojects instead of one undifferentiated milestone blob

### M3-A: Small Remaining Closeout Items

Scope:

- README badge version qualifier
- real usage examples in `docs/examples/`
- IntelliJ stacktrace format compatibility
- any other truly small M3 leftovers uncovered by Phase 0 truth-sync

Characteristics:

- low-risk
- small or medium effort
- good first autonomous execution batch after design alignment

### M3-B: Security and Operational Follow-Through

Scope:

- Dockerfile base-image CVE triage
- any required minimal remediation approved by security review

Characteristics:

- security-owned review first
- implementation only if a scoped remediation is approved

### M3-C: Benchmarking and Scale Follow-Through

Scope:

- `hyperfine` CLI timing automation
- `heaptrack` memory profiling automation where feasible
- remaining larger-tier validation follow-through

Characteristics:

- benchmark/documentation heavy
- may be platform-constrained

### M3-D: Remaining Deep Engine Gaps

Scope:

- richer OQL follow-through beyond built-in fields
- streaming overview mode if still required after design review
- threaded I/O pipeline if still justified by evidence
- `nom` parser evaluation only if profiling shows a real parser bottleneck

Characteristics:

- highest-risk remaining M3 work
- requires fresh design addenda before coding

### M4-A: Interactive HTML Reports

Scope:

- enhanced interactive HTML output
- collapsible sections, sorting, filtering, provenance/severity visibility

### M4-B: Local Web Server Foundation

Scope:

- `serve --web`
- localhost-only server
- read-only JSON/API surfaces for summary/leaks/navigation

### M4-C: Interactive Heap Browser

Scope:

- dominator browsing
- object inspection
- leak and GC path drill-down
- query console

### M4-D: Fast Re-Query Support

Scope:

- cache/index format for responsive UI re-query flows

### M5-Follow-On: Evidence-Driven AI Expansion

Scope:

- broader conversation/exploration semantics beyond current leak-focused chat
- native local-provider transports beyond OpenAI-compatible endpoints
- streaming only if validation proves the current contract insufficient

Important note:

M5 is not reopened as a fully pending milestone. Only the remaining evidence-driven follow-ons continue.

### M6-A: Documentation Foundation

Scope:

- user guide
- troubleshooting
- real examples in `docs/examples/`
- architecture/contributor walkthrough improvements

### M6-B: Example Projects and Sample Dumps

Scope:

- canonical leak scenario apps
- generated heap dumps
- walkthroughs and MCP/editor examples

### M6-C: Benchmark Publication and Integrations

Scope:

- published benchmark methodology and results
- GitHub Actions / Jenkins / GitLab templates

### M6-D: Community Foundations

Scope:

- contributor ladder
- good-first-issue program
- GitHub Discussions or equivalent lower-friction channel

### M6-E: Plugin System

Scope:

- only after the rest of M6 is stable
- requires a dedicated design-first pass before any runtime code is written

## Architecture and Ownership Rules

This closeout program will preserve the repository's ownership boundaries:

- Design Consulting owns milestone design docs and design references.
- Implementation owns production code.
- Testing owns test files and validation execution.
- Static Analysis owns lint/risk passes.
- Documentation Sync owns post-implementation documentation updates.
- Security owns audit-first security work.

No two writing agents may edit the same file in parallel.

## Verification Strategy

For every subproject:

- use TDD for behavior changes
- run focused tests during the batch
- run broader validation before calling the batch complete
- run `gitnexus_impact` before editing production symbols
- run `gitnexus_detect_changes()` before any commit request or milestone-complete claim

For docs/design-only batches:

- verify changed files are documentation-only
- verify cross-doc consistency with targeted searches
- run at least one fresh repository validation command before claiming the batch is complete

## Risks

- Roadmap/design drift could cause implementation on already shipped or incorrectly scoped work.
- Remaining M3 engine work is heterogeneous and should not be executed as one batch.
- M4 and M6 both contain XL subprojects; decomposition is mandatory.
- Security and benchmark items may depend on tooling or environment availability.
- Plugin-system work is a likely over-scope trap and must remain last.

## Decision

Proceed in this exact order:

1. design/doc alignment batch
2. M3 small closeout batch
3. M3 security/benchmark/engine follow-through in separate subprojects
4. M4 subprojects
5. M5 follow-on only where evidence supports more work
6. M6 foundational docs/examples/benchmarks/community work
7. M6 plugin-system work last, only after a fresh dedicated design pass
