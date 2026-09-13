# Post-M16 AI-Native MAT Product Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Mnemosyne's desktop and MCP investigation loops first, surface already-shipped analysis in the guided UI, then buy the highest-value remaining MAT compatibility and credibility improvements before considering deep OQL or dynamic plugins.

**Architecture:** Preserve the Rust core as the single analysis implementation. New desktop, MCP, CLI, and React behavior must be thin adapters over existing core functions wherever those functions already exist; backend work is limited to bounded additions such as the classloader workflow and later OQL slices. Delivery is incremental: every slice is independently testable, additive to existing contracts, and small enough to revert without a cross-product rewrite.

**Tech Stack:** Rust workspace (`mnemosyne-core`, CLI, MCP, Tauri v2), React/TypeScript/Bun, JSON/TOON/JUnit/GitHub Actions contracts, Criterion, GitHub Actions.

**Planning baseline:** [gap inventory](../../../.superpowers/sdd/reports/gap-inventory.md), [roadmap invariants and MAT matrix](../../roadmap.md), [M14 UI bridge design](../../design/milestone-14-ui-parity-ai-native.md), and [M14–M16 session summary](../../SESSION-SUMMARY-M14-M15-M16.md). The inventory is the source of truth for what is not shipped.

## Global Constraints

1. **Provenance-first:** every new response preserves or adds structured provenance. Overview, partial, fallback, heuristic, and unavailable states must never look like deep verified results.
2. **Overview honesty:** a capability without a valid streaming implementation must fail with structured `feature_unavailable_in_overview_mode`; it must not allocate a full `ObjectGraph` behind an overview request.
3. **MCP-first:** every newly user-facing analysis capability must have a structured MCP path in the same milestone. UI and Tauri can adapt that path but cannot become the only implementation.
4. **CLI/automation stability:** existing JSON, TOON, JUnit, GitHub Actions, exit-code, and default-output contracts remain backward compatible. New flags and fields are additive and default off when they add cost.
5. **No JVM dependency:** retain the Rust-native, single-binary product. MAT compatibility does not justify embedding Eclipse, Java, or a JVM.
6. **No duplicated analysis logic:** Tauri and MCP handlers call core functions; React renders typed contracts. Do not reimplement heap analysis in TypeScript or host-command glue.
7. **Bounded execution:** path counts, query depth, result rows, provider context, artifact size, and persisted state stay explicitly capped.
8. **Security and privacy:** do not log heap values, prompts, paths, API keys, snapshot contents, or plugin payloads. Validate local paths and keep provider redaction/audit behavior intact.
9. **Honest release claims:** M12 remains environment-blocked until a native-Linux reference workstation with Eclipse MAT and the 10 GiB fixture produces evidence. Desktop signing, notarization, platform launch, and tagged artifact claims require captured evidence.
10. **Cost-aware delivery:** prefer adapters, additive parameters, focused panels, and contract tests. No bridge rewrite, MCP transport rewrite, UI redesign, query-engine rewrite, or plugin big bang.
11. **Design gate before code:** each milestone first creates its linked `docs/design/milestone-*.md`, maps exact symbols/files, runs required GitNexus impact checks, and records a READY verdict.
12. **Verification:** each implementation slice ends with focused tests; each milestone ends with the relevant full gates:

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cd ui && bun run test
cd ui && bun run lint
```

## Success Metrics

- All four M14 live capability groups work in the packaged Tauri desktop: object inspection, all GC paths, object diff, and workflow/snapshot operations.
- Agents can perform policy checks with an optional baseline, request leak-annotated diffs, create/remove snapshots, and generate flamegraph artifacts through MCP without shelling out.
- Duplicate-array results, static-plugin findings, and superclass regrouping are visible in the React UI from real current JSON contracts.
- A fifth `classloader_leak` workflow completes end to end through core, MCP, browser UI, and Tauri using the shipped M13 analyzers.
- CLI and MCP AI conversations retain at least 12 recent turns by default under one shared bounded policy; no unbounded session growth is introduced.
- The bounded MAT migration slice adds multi-class `FROM` and bounded multi-hop `OBJECTS` without weakening parser errors, result budgets, or existing query behavior.
- Snapshot-load and post-M16 desktop artifact claims have reproducible evidence. The full MAT/10 GiB claim remains explicitly partial until M12 can run.
- No milestone is called shipped until focused tests, full applicable gates, docs, and captured contract/benchmark artifacts are committed.

## Non-Goals Through M21

- Live JVM attach, heap capture agents, CPU sampling, or replacing `jmap`.
- Eclipse MAT `.index` read/write or cross-machine snapshot interchange.
- MCP token streaming or an MCP transport rewrite without measured latency/user evidence.
- Bundling Ollama, LM Studio, or another local model runtime.
- Raw Rust trait-object `cdylib` loading, third-party plugin marketplaces, or executable custom UI code.
- Full MAT OQL, Java/JavaScript script execution, or unbounded query recursion.
- Auto-update, package-manager expansion, or mandatory code signing.
- At-rest snapshot encryption and field-aware PII inference; these require a separate enterprise security proposal.

## File and Ownership Map

- `docs/design/milestone-17-desktop-guided-ux-bridges.md` — M17 design and bridge contract.
- `docs/design/milestone-18-mcp-agent-loop-completion.md` — M18 MCP tool schemas and parity rules.
- `docs/design/milestone-19-guided-analysis-completion.md` — M19 UI/workflow/session contract.
- `docs/design/milestone-20-bounded-mat-migration-polish.md` — M20 bounded OQL/operator scope.
- `docs/design/milestone-21-credibility-release-evidence.md` — M21 benchmark/release evidence protocol.
- `docs/design/milestone-22-advanced-oql-compatibility.md` — conditional deep-OQL design.
- `docs/design/milestone-23-extension-runtime-custom-inspectors.md` — conditional extension design.
- Runtime ownership follows the repository workflow: one implementation owner per slice, then Testing, Static Analysis, and Documentation Sync. No two slices may edit `core/src/mcp/server.rs`, `core/src/workflow/`, `ui/src/lib/analysis-types.ts`, `ui/src/app/router.tsx`, or Tauri bridge files concurrently.

## Sequencing

1. **M17 — Desktop Guided UX Bridge Completion**
2. **M18 — MCP Agent and IDE Loop Completion**
3. **M19 — Guided Analysis and Investigation Continuity**
4. **M20 — Bounded MAT Migration Polish**
5. **M21 — Credibility and Release Evidence**
6. **M22 — Advanced OQL Compatibility** (conditional on a real saved-query corpus)
7. **M23 — Extension Runtime and Custom Inspectors** (conditional on Phase-2 adoption and external demand)

**Parallel blocked track:** M12 remains the reference-workstation benchmark milestone. Run it whenever the specified hardware, Eclipse MAT, and 10 GiB fixture are available, but do not let its blocked environment stop M17–M20 and do not remove the partial-evidence caveat before it passes.

---

## M17 — Desktop Guided UX Bridge Completion

**Status:** 🔲 Pending
**Why now:** M16 distributes the M14 UI, but the desktop host injects only the two pre-M14 bridges. The shipped desktop therefore advertises guided and power surfaces that deliberately render unavailable. This is the most direct usefulness gap and reuses already-tested core behavior.

**Product outcome:** A desktop user with valid heap inputs can use every M14 live surface without switching to a browser mock, MCP client, or CLI.

**In scope:**
- Native Tauri commands and TypeScript injection for `inspectObject` and `findAllGcPaths`.
- The dedicated comparison bridge's `diffObjects`.
- The workflow bridge's `describeWorkflow`, `startWorkflow`, `nextStep`, and `listSnapshots`; add `getWorkflow`/`closeWorkflow` only if the existing UI lifecycle requires them.
- Contract normalization at the bridge boundary only; Rust response truth remains unchanged.
- Synthetic-fixture command tests plus a packaged-desktop smoke path.

**Out of scope:**
- New analyzers, new workflow kinds, M15 UI panels, signing credentials, auto-update, or bridge API redesign.
- Pretending browser-only artifact views are live Tauri commands.

### Slice 17.A — Inspector and multi-path commands

**Likely files:** `tauri/src/commands.rs`, `tauri/src/bridge.ts`, Tauri command tests.

- [ ] Map `inspectObject` to the existing core object-inspection path with the same typed field opt-in and structured unknown-object error.
- [ ] Map `findAllGcPaths` to the existing bounded all-paths request, preserving `max_paths` and `truncated`.
- [ ] Prove old bridge methods are byte-compatible when the new methods are unused.

### Slice 17.B — Comparison command

**Likely files:** `tauri/src/commands.rs`, `tauri/src/bridge.ts`, comparison bridge tests.

- [ ] Implement `diffObjects` over the existing `DiffMode::Object` path.
- [ ] Preserve `MatchQuality`, identity strategy, fingerprint budget, and opt-in leak cross-reference defaults.
- [ ] Test pure-add, pure-remove, retained-change, collision, and invalid-input cases.

### Slice 17.C — Workflow and snapshot commands

**Likely files:** `tauri/src/commands.rs`, `tauri/src/bridge.ts`, workflow bridge tests.

- [ ] Bind workflow description/start/next to the existing `WorkflowStore` and state transitions.
- [ ] Bind `listSnapshots` to the existing snapshot store and preserve stale/corrupt distinctions.
- [ ] Keep workflow IDs and heap paths out of logs.

### Slice 17.D — Desktop integration evidence

**Likely files:** `tauri/tests/`, UI bridge contract tests, M17 design/closeout docs.

- [ ] Exercise all four capability groups against synthetic fixtures from a packaged or production-mode desktop build.
- [ ] Verify a browser with no bridge still renders the existing honest unavailable states.
- [ ] Capture Windows evidence; capture macOS/Linux only when actually run on those platforms.

**Dependencies:** M8–M11, M13–M16 shipped; no dependency on M12.

**Risks and mitigations:**
- Host/core JSON casing drift — assert against real serialized responses, not hand-authored mocks.
- UI thread blocking — keep expensive commands asynchronous and expose progress/error state without changing analysis semantics.
- Path access expansion — use Tauri's existing local-file validation and never pass paths to a shell.

**Acceptance gates:**
- All M14 bridge methods present in the desktop injection and backed by native commands.
- Valid fixture calls render results; invalid and overview-incompatible calls render structured errors.
- Existing browser fallback tests, Tauri checks, Rust gates, and UI gates pass.
- STATUS/README still say unsigned and platform-unverified wherever evidence is absent.

---

## M18 — MCP Agent and IDE Loop Completion

**Status:** 🔲 Pending
**Why now:** Mnemosyne's automation moat is strongest in the CLI, but agents still must shell out for policy gates, baseline growth, flamegraphs, and snapshot mutation. Closing these gaps makes the MCP server a complete investigation/CI backend and directly benefits IDE integrations.

**Product outcome:** An MCP client can complete the same high-value CI, comparison, snapshot, and visualization loop as the CLI using structured calls.

**In scope:**
- New `ci_check` MCP tool with policy input, mode, severity threshold, output-independent structured result, and optional baseline.
- Additive `cross_reference_leaks` on `diff_heaps`, default `false`.
- `save_snapshot` and `remove_snapshot` tools complementing `open_snapshot`/`list_snapshots`.
- A `generate_flamegraph` tool exposing the existing three rooting strategies and formats with bounded artifact handling.
- Tool schemas, `list_tools`, structured errors, captured transcripts, and CLI/MCP equivalence tests.

**Out of scope:**
- A second policy engine, direct shell execution, arbitrary output paths, streamed SVG/tokens, or a CI-provider-specific service.
- Changing existing CLI exit codes or diff defaults.

### Slice 18.A — Single-heap policy checks

**Likely files:** `core/src/mcp/server.rs`, MCP tests, `docs/mcp-tools.md` or current MCP reference.

- [ ] Register `ci_check` and reuse policy loading/evaluation from `core::policy`.
- [ ] Return the full structured policy result plus the equivalent recommended CLI exit classification.
- [ ] Preserve skipped-rule reasons and overview/deep mismatch errors.

### Slice 18.B — Baseline object growth

- [ ] Add optional baseline input to `ci_check`.
- [ ] Reuse object diff once per request when an `object_growth_threshold` rule needs it.
- [ ] Return `object_growth_threshold_requires_baseline` when required, never a silent skip.

### Slice 18.C — Leak-progress diff cross-reference

- [ ] Add `cross_reference_leaks: boolean` to `diff_heaps`.
- [ ] Keep omitted/false behavior byte-compatible.
- [ ] Assert annotated and unannotated object deltas against the CLI path.

### Slice 18.D — Snapshot mutation

- [ ] Add `save_snapshot` and `remove_snapshot` with the existing schema, freshness, corruption, and not-found errors.
- [ ] Restrict mutations to the configured snapshot store; no arbitrary delete path.
- [ ] Verify open/list/save/remove lifecycle with a synthetic heap.

### Slice 18.E — Flamegraph artifact generation

- [ ] Expose dominator, class-hierarchy, and GC-root-path rooting and SVG/folded/JSON formats.
- [ ] Reuse deep-mode analysis and return structured overview-mode unavailability.
- [ ] Enforce response/artifact budgets and avoid logging rendered heap labels.

### Slice 18.F — Agent-loop transcripts and docs

- [ ] Capture one real MCP transcript for policy-with-baseline and one for snapshot-to-diff-to-flamegraph.
- [ ] Update tool discovery docs from actual registered schemas.
- [ ] Confirm no transcript contains local absolute paths or heap-derived sensitive values.

**Dependencies:** M10-B and M7 policy/flamegraph paths shipped; M9 snapshot store shipped. M17 is sequenced first for immediate desktop usefulness but is not a code dependency.

**Risks and mitigations:**
- MCP/CLI semantic drift — parity tests compare normalized core results, not renderer text.
- Large flamegraph payloads — enforce limits and prefer managed artifacts over unbounded inline SVG.
- Snapshot deletion abuse — accept store keys only and resolve within the configured root.

**Acceptance gates:**
- `list_tools` publishes complete schemas for all new/additive fields.
- An MCP-only integration test completes policy gate → baseline growth → annotated diff → snapshot lifecycle → flamegraph.
- Omitted additive parameters preserve prior responses.
- Full Rust gates pass; docs include real captured exchanges.

---

## M19 — Guided Analysis and Investigation Continuity

**Status:** 🔲 Pending
**Why now:** M15 shipped useful backend data that has no React surface, and M13 classloader detection lacks an M11-style guided workflow. These are low-analysis-risk additions that convert existing backend value into a complete guided experience. The three-turn conversation cap is also too short for the multi-step workflows already shipped.

**Product outcome:** Browser and desktop users can see M15 results, launch classloader triage, and continue a bounded investigation without losing context after three turns.

**In scope:**
- React types and panels for duplicate primitive arrays and static `plugin_results`.
- Interactive regrouping to the shipped superclass histogram mode; flat grouped results first.
- A fifth `classloader_leak` workflow over existing M13 primitives, exposed through MCP and the generic workflow bridge.
- Shared bounded AI history policy: default 12 recent turns, configurable up to a hard maximum of 32 for both CLI and MCP.
- Workflow-card/UI integration and compatibility with M17's generic Tauri workflow bridge.

**Out of scope:**
- New classloader heuristics, dynamic plugins, executable custom inspector UI, MAT's full collapsible superclass tree, user-authored workflows, streaming, or unbounded history.

### Slice 19.A — Duplicate-array panel

**Likely files:** `ui/src/lib/analysis-types.ts`, `ui/src/features/artifact-explorer/`, focused UI tests.

- [ ] Type the real snake_case array report emitted by current CLI JSON.
- [ ] Render element type, length, duplicate count, and wasted bytes with absent-field compatibility.
- [ ] Verify an older artifact without `array_report` does not crash or show fabricated zeroes.

### Slice 19.B — Superclass regroup control

- [ ] Add a superclass option to the existing histogram grouping interaction.
- [ ] Add an optional `regroupHistogram(groupBy)` host method backed by MCP `analyze_heap.histogram_group_by`, and wire the same adapter into Tauri without duplicating grouping logic.
- [ ] Clearly label a live regroup versus a precomputed artifact grouping.
- [ ] Keep the list flat in this slice; do not imply a hierarchy tree.

### Slice 19.C — Static plugin findings panel

- [ ] Type and render existing `plugin_results` as provenance-bearing findings.
- [ ] Treat plugin name/formatter data as untrusted display text.
- [ ] Preserve a clean absence state for standard builds with no registered plugins.

### Slice 19.D — Classloader workflow core

**Likely files:** `core/src/workflow/`, `core/src/mcp/server.rs`, workflow contract tests.

- [ ] Add `classloader_leak` as orchestration only over duplicate classes, loader chains, object inspection, and GC paths.
- [ ] Define a bounded sequence: detect → select duplicate class/loader → inspect retention → explain → complete.
- [ ] Add full-run, invalid-selection, resume, and close tests plus a real transcript.

### Slice 19.E — Guided workflow card

- [ ] Add the workflow to discovery and the guided landing without hiding existing power routes.
- [ ] Run it through browser and M17 Tauri generic workflow bridges without a classloader-specific host API.
- [ ] Deep-link findings to the shipped classloader and object-inspector panels.

### Slice 19.F — Bounded conversation continuity

**Likely files:** `core/src/mcp/session.rs`, shared AI config, CLI chat code/tests.

- [ ] Replace separate hard-coded three-turn windows with one shared policy: 12 turns by default, hard maximum 32.
- [ ] Preserve focus state and provider redaction when old turns are evicted.
- [ ] Add deterministic tests at 12 and 32 turns and confirm persisted session compatibility.

**Dependencies:** M17 generic desktop bridges, M13 classloader analyzer, M15 backend fields. M18 is desirable for agent completeness but not required for UI artifact panels.

**Risks and mitigations:**
- UI contract mismatch — capture real CLI JSON fixtures before typing fields.
- Workflow drift — extend the M11 description/runtime transition contract tests.
- Context cost growth — hard cap both turn count and existing provider prompt budget; no unbounded persisted prompt.

**Acceptance gates:**
- Duplicate arrays and static plugin findings render real artifacts and degrade cleanly on old artifacts; superclass regroup runs through MCP and desktop against a real heap.
- Classloader workflow runs start-to-complete in core, MCP, browser, and desktop.
- CLI and MCP share the same 12/32 history policy.
- Rust/UI/Tauri full gates pass; no streaming claim is introduced.

---

## M20 — Bounded MAT Migration Polish

**Status:** 🔲 Pending
**Why now:** After the product loops are complete, the remaining cheap MAT migration friction is concentrated in saved queries and small operator polish. This milestone deliberately takes bounded, high-ROI pieces and leaves scriptlets and arbitrary recursion for later.

**Product outcome:** More real MAT OQL queries migrate with predictable budgets, while classloader and grouped-histogram data become easier for power users to read.

**In scope:**
- Multi-class `FROM` lists with explicit union/deduplication semantics.
- Multi-hop `OBJECTS` chains capped at three field segments and existing row/result budgets.
- CLI rendering of already-computed `unique_class_count`.
- Optional collapsible superclass presentation only if it consumes already-returned grouped data; otherwise retain flat regroup and defer the tree.
- Query compatibility corpus and structured unsupported/error cases.

**Out of scope:**
- `eval(...)`, arbitrary-depth subqueries, recursive graph patterns, Java semantics, query-engine rewrite, or Phase-3 plugins.

### Slice 20.A — MAT saved-query corpus

- [ ] Add a sanitized, synthetic corpus representing shipped, newly targeted, and explicitly deferred syntax.
- [ ] Record expected parse/execute/error outcomes; never commit customer heap values or queries.
- [ ] Use corpus results to freeze the exact M20 grammar boundary.

### Slice 20.B — Multi-class `FROM`

- [ ] Parse a bounded class list and reuse current class-pattern resolution per entry.
- [ ] Deduplicate by object ID before ordering/limit.
- [ ] Preserve existing single-class AST serialization and results.

### Slice 20.C — Bounded multi-hop `OBJECTS`

- [ ] Support one to three field segments with cycle-safe traversal and existing result limits.
- [ ] Reject more than three segments with a structured limit error.
- [ ] Test missing fields, nulls, cycles, duplicate targets, and budget exhaustion.

### Slice 20.D — Classloader/operator polish

- [ ] Render `unique_class_count` in the CLI classloader table.
- [ ] Evaluate the superclass tree as a UI-only projection; ship it only if no new backend traversal or contract is needed.
- [ ] Keep provenance and flat fallback visible.

**Dependencies:** M15 query engine and M19 regroup UI. No dependency on M12.

**Risks and mitigations:**
- Grammar ambiguity — corpus-first parser tests before executor changes.
- Traversal explosion — three-segment cap, visited set, and row/result budgets.
- False "full MAT OQL" claim — docs list `eval` and deeper nesting as unsupported after M20.

**Acceptance gates:**
- Target corpus passes with explicit expected outcomes and no regression in current query tests.
- Query budgets and overview-mode errors are enforced.
- CLI/MCP query paths return equivalent results for new syntax.
- Documentation continues to describe OQL as bounded/partial.

---

## M21 — Credibility and Release Evidence

**Status:** 🔲 Pending
**Why now:** Product loops and the highest-ROI parity work should precede expensive evidence gathering, but the remaining performance and desktop caveats must be measured before broader claims. This milestone closes evidence that is runnable in normal CI and preserves M12 as an explicit external blocker.

**Product outcome:** Snapshot speed and post-M16 desktop artifacts have reproducible evidence; unsupported signing/platform/MAT claims remain visibly caveated.

**In scope:**
- The missing Criterion cold-parse versus snapshot-load benchmark.
- Tagged-release dry run or real post-M16 tag evidence for desktop artifact production and attachment.
- Platform-specific install/launch evidence on every actually available runner.
- Published raw artifacts, methodology, versions, and caveats.
- Trigger M12 only when its reference environment exists.

**Out of scope:**
- Invented benchmark numbers, WSL results labeled native Linux, unsigned artifacts labeled signed, or macOS/Linux bundles labeled launch-tested without a launch.
- Making M12 a blocker for M17–M20.

### Slice 21.A — Snapshot-load benchmark

**Likely files:** `core/benches/snapshot_load.rs`, benchmark fixture helpers, benchmark docs.

- [ ] Measure cold binary parse, fresh snapshot save, and warm snapshot load on the same fixture and build.
- [ ] Publish raw Criterion output and state that timing does not alter correctness guarantees.
- [ ] Keep generated large fixtures out of git.

### Slice 21.B — Desktop release artifact proof

- [ ] Run the tagged-release desktop matrix or a non-publishing equivalent at the exact release workflow revision.
- [ ] Verify expected bundle types and release attachment logic per OS.
- [ ] Record unsigned fallback lines when credentials are absent.

### Slice 21.C — Platform launch evidence

- [ ] Launch-test Windows, Linux, and macOS only on matching hosts; record host/OS/build hash.
- [ ] Keep any unavailable platform marked not launch-tested.
- [ ] Verify one M17 bridge-backed workflow in each successfully launched build.

### Slice 21.D — M12 handoff

- [ ] Check the M12 prerequisites exactly as defined by the reference spec.
- [ ] If available, run the existing M7-5/M12 methodology unchanged and publish MAT, `mnemo-deep`, `mnemo-overview`, `hprof-slurp`, Jaccard, RSS, and 10 GiB results.
- [ ] If unavailable, leave M12 🟡 blocked and retain every partial-evidence caveat.

**Dependencies:** M17 for bridge-backed desktop smoke. M12 has an external hardware/tool dependency and may remain blocked after M21's runnable slices close.

**Risks and mitigations:**
- Non-comparable benchmarks — pin fixture, hardware, build profile, tool versions, and commands.
- Release overclaim — documentation derives claims from captured artifacts, not workflow configuration.
- Credential absence — signing remains optional and is never a functional gate.

**Acceptance gates:**
- Snapshot benchmark and raw evidence are committed and reproducible.
- Desktop build/launch matrix distinguishes configured, built, attached, signed, and launch-tested.
- M12 caveat is removed only after the full reference method succeeds.

---

## M22 — Advanced OQL Compatibility

**Status:** 🔲 Pending, conditional
**Scheduling gate:** At least 20 sanitized, currently failing real-world saved queries show that deeper nesting or `eval(...)` blocks migration after M20. Without that evidence, keep structured deferrals and do not start.

**Why later:** Deep query semantics expand parser, evaluator, performance, and security risk while current triage OQL already covers the high-value majority.

**In scope if scheduled:**
- Nested subqueries capped at depth 8 with explicit execution budgets.
- A Rust-native, allowlisted expression form for the smallest evidenced `eval(...)` subset; no Java, JavaScript, shell, reflection, I/O, network, or dynamic code execution.
- Compatibility reporting against the gated corpus.

**Out of scope:**
- Arbitrary code execution, unlimited recursion, full Java expression semantics, or silent approximation.

### Slices

- [ ] **22.A:** Threat model and corpus classification; reject unsafe/unbounded requirements before grammar work.
- [ ] **22.B:** Depth-8 nested subqueries with cumulative row/work budgets.
- [ ] **22.C:** Implement only the allowlisted pure-expression subset evidenced by the corpus.
- [ ] **22.D:** CLI/MCP parity, fuzz/property tests, docs, and explicit remaining incompatibilities.

**Dependencies:** M20 corpus and bounded grammar. Security review is mandatory before 22.C.

**Risks:** denial of service, semantic mismatch with MAT, and code-injection expectations. Mitigate with parser-only ASTs, no runtime code loader, cumulative budgets, fuzzing, and explicit partial-compatibility labels.

**Acceptance gates:**
- Gate corpus demonstrates demand and target coverage.
- No expression can access filesystem, network, environment, process, or host language execution.
- Depth/work limit errors are structured and deterministic.
- Docs do not call the result full MAT OQL unless the entire corpus and named matrix gaps are actually closed.

---

## M23 — Extension Runtime and Custom Inspectors

**Status:** 🔲 Pending, conditional
**Scheduling gate:** At least two maintained out-of-tree analyzers or formatters are blocked by static registration, with named owners willing to test version upgrades.

**Why last:** The current Phase-2 registry has no evidenced third-party demand. Dynamic native loading adds ABI, supply-chain, crash-isolation, and UI trust risks that are larger than the current product benefit.

**In scope if scheduled:**
- A versioned manifest and compatibility negotiation.
- An ABI/security spike comparing a narrow C ABI with WASI or an out-of-process protocol. Raw Rust trait-object ABI is not acceptable across compiler/version boundaries.
- Explicit filesystem discovery and CLI/config opt-in.
- Structured custom-inspector view models rendered by trusted built-in React components; plugins do not ship arbitrary JavaScript.
- Signature/hash metadata and clear trusted/untrusted provenance.

**Out of scope:**
- JVM/JAR plugins, automatic internet installation, arbitrary executable UI, marketplace hosting, or loading untrusted native code by default.

### Slices

- [ ] **23.A:** Adoption evidence, threat model, ABI/protocol decision, and no-go criteria.
- [ ] **23.B:** Versioned manifest plus discovery/list/validate commands with loading disabled by default.
- [ ] **23.C:** One isolated analyzer/formatter proof using the selected boundary and failure containment.
- [ ] **23.D:** MCP metadata/results with plugin identity, version, trust, and provenance.
- [ ] **23.E:** Declarative custom-inspector schema and built-in safe renderers.
- [ ] **23.F:** CLI opt-in, upgrade compatibility tests, security review, and operator docs.

**Dependencies:** Phase-2 registry adoption, M19 static plugin-results UI, and explicit security review.

**Risks:** unstable Rust ABI, arbitrary native execution, dependency confusion, data exfiltration, host crashes, and incompatible custom UI. Mitigate through an explicit stable boundary, default-off loading, allowlisted roots, integrity metadata, isolation where feasible, and declarative rendering.

**Acceptance gates:**
- Scheduling demand gate is documented with real maintainers.
- A plugin built against the oldest supported extension ABI passes current compatibility tests or fails before execution with a structured incompatibility error.
- Disabled/default installations execute no third-party code.
- Plugin findings and custom inspector data carry plugin identity and provenance through CLI, MCP, reports, browser, and desktop.

---

## Explicitly Deferred Discovery Lane

The following are not milestones until evidence changes their ranking:

- **Live JVM attach/capture:** keep external `jmap`/artifact workflows. Reconsider only after MCP/IDE loops show repeated capture friction and a no-JVM design remains possible.
- **MCP streaming:** instrument provider/tool latency first. Reconsider only when p95 response time or payload size causes measured abandonment/timeouts that chunked artifacts cannot solve.
- **MAT `.index` interchange:** reconsider only with cross-tool migration demand; Mnemosyne snapshots remain local and versioned.
- **Composable/user-defined workflows:** add a declarative format only after at least three requested workflows cannot be served by small built-in state machines.
- **Enterprise snapshot encryption and field-aware PII redaction:** require a separate threat model, key-management design, and enterprise owner; do not bolt cryptography onto the cache opportunistically.

## Milestone Closeout Checklist

- [ ] Design doc linked from `docs/roadmap.md` and marked READY before runtime edits.
- [ ] GitNexus impact analysis run for every modified symbol; HIGH/CRITICAL blast radius disclosed before edits.
- [ ] Focused red tests precede implementation for behavior changes.
- [ ] MCP schema and structured-error contract updated for every user-facing capability.
- [ ] Overview/provenance behavior tested, including unavailable paths.
- [ ] CLI/default backward compatibility tested for additive fields and flags.
- [ ] Security/privacy tests cover paths, logs, heap-derived text, and artifact limits.
- [ ] Full applicable Rust, UI, and Tauri gates pass.
- [ ] `gitnexus_detect_changes()` confirms expected affected symbols and flows before commit.
- [ ] STATUS, roadmap, user guide, MCP docs, CHANGELOG, and design closeout reflect actual evidence.
- [ ] Milestone remains 🔲 Pending until every acceptance gate is evidenced; partial slices are reported as partial, never shipped.
