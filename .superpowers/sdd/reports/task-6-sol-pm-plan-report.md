# Task 6 — Post-M16 Product Plan Report

## Status

Complete. Authored the post-M16 product plan and patched roadmap §§5–6 with pending candidates only. No candidate is described as shipped.

## Inputs reviewed

- `.superpowers/sdd/reports/gap-inventory.md`
- `docs/roadmap.md`, including invariants and the MAT parity matrix
- `STATUS.md`, including every open ⚠️/🟡 caveat
- `docs/design/milestone-14-ui-parity-ai-native.md`
- `docs/SESSION-SUMMARY-M14-M15-M16.md`

## Product decisions

1. **M17 completes the packaged desktop before adding depth.** M16 made the UI installable, but M14's object-inspection, all-paths, comparison, workflow, and snapshot-list bridges remain unavailable in Tauri. Thin native adapters over shipped core behavior are the highest-value and lowest-analysis-risk next step.
2. **M18 closes the agent/IDE loop.** MCP receives the CLI-first policy/baseline, diff leak cross-reference, snapshot mutation, and flamegraph surfaces. This removes shell-outs and restores the roadmap's MCP-first invariant.
3. **M19 converts shipped M13/M15 backend value into guided UX.** Duplicate arrays, superclass regrouping, and static plugin findings gain UI surfaces; classloader analysis gains a fifth fixed workflow. CLI/MCP conversation history becomes shared and bounded rather than independently hard-coded to three turns.
4. **M20 takes only bounded MAT migration wins.** Multi-class `FROM`, three-segment `OBJECTS` chains, and small operator polish precede deeper query semantics. `eval(...)`, arbitrary nesting, and query-engine rewrites remain out.
5. **M21 separates reproducible evidence from feature work.** Snapshot-load and post-M16 desktop artifact/launch evidence are runnable without pretending M12 is unblocked. Signing remains optional and claim-based on artifacts, not configuration.
6. **M22/M23 are conditional, not commitments.** Advanced OQL requires a corpus of at least 20 blocked real-world saved queries. Dynamic extensions require at least two maintained out-of-tree adopters. Raw Rust trait-object `cdylib` loading is rejected as a stable ABI strategy.
7. **Live attach, MAT index interchange, and MCP streaming remain discovery items.** Agent/IDE and artifact workflows have higher demonstrated value; no speculative milestone is assigned.

## Ranking rationale

- **First:** repair visible desktop gaps and MCP shell-outs where core behavior already exists.
- **Second:** surface M15 results and add classloader orchestration, because these are additive UI/workflow slices rather than new analyzers.
- **Third:** close common, bounded MAT migration friction without claiming full OQL parity.
- **Fourth:** publish benchmark/release evidence, while preserving M12's native-Linux/MAT/10 GiB blocker.
- **Last:** deep OQL and dynamic/custom-view extensions, gated by evidence because they have the highest security, ABI, complexity, and maintenance cost.

## Guardrails carried forward

- Structured provenance and honest overview-mode errors on every new surface.
- MCP-first, CLI/automation-compatible, Rust-native, no JVM dependency.
- Additive parameters and adapters; no bridge, transport, UI, or query big-bang rewrite.
- Bounded paths, queries, history, artifacts, and persisted state.
- No signed, notarized, launch-tested, benchmarked, or MAT-parity claim without captured evidence.

## Deliverables

- Product plan: `docs/superpowers/plans/2026-09-13-post-m16-ai-native-mat-plan.md`
- Roadmap: `docs/roadmap.md` §§5–6
- This report: `.superpowers/sdd/reports/task-6-sol-pm-plan-report.md`

## Verification note

This batch changes Markdown only. Verification should therefore check document structure, links, milestone statuses/order, and git scope; runtime test suites are unnecessary unless a repository hook requires them.
