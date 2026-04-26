# Mnemosyne Roadmap — Path to MAT

> **Last updated:** 2026-04-26
> **Owner:** Tech PM Agent  
> **Goal:** Reach Eclipse MAT-level analysis depth and quality while maintaining Mnemosyne's unique differentiators
> **Historical archive:** [roadmap-archive.md](roadmap-archive.md)

Mnemosyne has closed M1 through M6 and shipped v0.2.0 across releases, Docker, crates.io, and Homebrew. The active roadmap is now deliberately narrow: close the highest-value MAT gaps, solve large-dump credibility, and widen the project's CI/AI/MCP moat rather than reopening completed milestone detail.

---

## 1. Where We Are (v0.2.0)

v0.2.0 is not a prototype foundation anymore. The core parser, object-graph analysis, dominator tree, UI, MCP server, and AI integration are shipped. The remaining question is not whether Mnemosyne works, but which gaps matter most on the path from a strong alpha to a credible MAT alternative.

| Area | Status | Key Metric |
|---|---|---|
| Parser | ✅ Production | 2.25 GiB/s streaming, 90 MiB/s binary |
| Analysis | ✅ Production | Dominator tree, retained sizes, MAT-style suspects, 8 analyzer surfaces |
| Query | ⚠️ Basic | Instance-field projection, `INSTANCEOF`; still narrow vs MAT OQL |
| AI | ✅ Shipped | Rules/stub/provider modes, CLI chat, MCP sessions |
| MCP | ✅ Shipped | 14 methods, `list_tools`, `error_details`, session lifecycle |
| UI | ✅ Shipped | Browser-first React, artifact/heap/leak explorer flows |
| Desktop | ⚠️ Scaffold | Tauri shell exists, not yet distributable |
| Distribution | ✅ Full | 4 end-user channels plus automated release pipeline |
| Testing | ✅ Solid | 377 Rust tests + 143 UI tests |
| Scale | ⚠️ In progress | Deep mode is validated to ~2 GB dense tiers; M7-1 overview mode now covers 10 GB+ streaming triage, and M7-5 still needs published head-to-head proof |

**Active roadmap rule:** M1-M6 are complete and archived. The only active milestone is M7, followed by the M8+ backlog.

## 2. The MAT Gap — What's Missing

Mnemosyne is already ahead of MAT on provenance, AI assistance, MCP-native IDE workflows, and automation-friendly structured output. The remaining gaps are concentrated in scale, query depth, and a small number of high-value investigation workflows.

### Critical gaps

- **Published large-dump proof (10 GB+):** M7-1 overview mode now provides bounded-memory streaming triage for very large dumps, but M7-5 still needs to publish the comparative benchmark evidence that makes those claims defensible.
- **Full OQL depth:** the shipped query engine covers high-value basics, not MAT's full predicate, function, and traversal depth.

### Important gaps

- **Object-level heap diff:** Mnemosyne has class-level deltas today, not stable per-object change tracking across snapshots.

### Nice-to-have gaps

- **Incremental leak tracking:** 3+ snapshot trend analysis is valuable, but it depends on the M7 scale story first.
- **IDE-native memory annotations:** a powerful MCP/LSP extension of the current moat, but not needed for v0.3.0 credibility.
- **Persistent indexes / cache:** parse-once, query-many is a real MAT strength, but it is a longer-tail architectural investment than M7 requires.

## 3. Competitive Positioning

Mnemosyne does not need to beat both competitors at their own game. It needs to close the few blocking gaps, then win on the workflows they cannot match.

| Dimension | Mnemosyne | Eclipse MAT | hprof-slurp |
|---|---|---|---|
| Core analysis depth | Strong and shipped; query/diff still narrower | Best today | Shallow triage only |
| Large-dump handling | M7-1 overview mode shipped; M7-5 benchmarks still pending | Disk-index capable but heavyweight | Best-in-class streaming / low RSS |
| Automation / CI | Strong JSON/TOON story plus shipped CI policy gate | Weak | Basic scripting only |
| IDE / AI integration | MCP + AI + provenance | Eclipse-only, no AI | None |
| User experience | Modern CLI + browser-first UI | Mature but dated desktop UX | Fast CLI, minimal exploration |

**Where Mnemosyne wins now:** provenance, MCP-native IDE integration, AI-assisted diagnosis, browser-first UI, and distribution breadth.

**Where Mnemosyne still loses:** published large-dump benchmark proof, full OQL depth, and object-level diff.

**Where Mnemosyne is already becoming unique:** CI regression policies and allocation-site flame graphs widen a category neither MAT nor hprof-slurp currently owns.

## 4. Milestone 7 — Production Readiness & Scale

**Design references:** [design/milestone-7-production-readiness.md](design/milestone-7-production-readiness.md), [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md), and [design/milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md)  
**Status:** ⚠️ In progress

M7 is the active milestone. Its job is not to reopen the M1-M6 foundation, but to make Mnemosyne credible on the remaining production blockers: scale, focused parity, and differentiated workflows that MAT cannot match.

| # | Item | Type | Priority | Effort |
|---|---|---|---|---|
| M7-1 | Streaming overview mode | Parity + Diff | P1 | L |
| M7-2 | CI regression policies | Differentiation | P1 | M |
| M7-3 | Allocation-site flame graphs | Differentiation | P1 | M |
| M7-4 | OQL targeted expansion (5-6 predicates) | Parity | P1 | M |
| M7-5 | Comparative benchmarks vs MAT/hprof-slurp | Credibility | P2 | M |
| M7-6 | v0.3.0 release | Release | P1 | S |

**Status detail:** ✅ M7-1 through M7-3 are complete (3/6 slices). Streaming overview mode shipped against [design/milestone-7-production-readiness.md](design/milestone-7-production-readiness.md), CI regression policies shipped against [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md), and allocation-site flame graphs shipped against [design/milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md). M7-4 through M7-6 remain planned.

### Phase 1 — Foundation

**M7-1 is complete.** Streaming overview mode is now the enabling layer for large-dump credibility, fair benchmark comparisons, and differentiated workflows on real production-scale data. The next active slices are M7-4 through M7-6.

### Phase 2 — Differentiation

**M7-2 and M7-3 are complete.** `mnemosyne-cli ci-check` gives Mnemosyne a one-command CI gate with policy TOML, severity-aware exit codes, and text/JSON/JUnit/GitHub Actions outputs, and `mnemosyne-cli flamegraph` now exports retained-size SVG/folded-stack/JSON artifacts from deep-mode heap analysis. Deliver **M7-4** next so the roadmap moves from differentiated workflows back to targeted parity work.

### Phase 3 — Depth

Deliver **M7-4** after the scale and differentiation slices land. The goal is targeted expansion, not the full MAT OQL treadmill: add the highest-value predicates only.

### Phase 4 — Validation & Release

Close with **M7-5** and **M7-6**. Publish comparative evidence, then ship `v0.3.0` with a clear story: Mnemosyne is now credible at larger scale and differentiated in automation and visualization.

### Success criteria

- `mnemosyne parse --mode overview` processes a 10 GB dump in under 60 seconds with under 1 GB RSS.
- `mnemosyne ci-check <heap.hprof> --policy policy.toml` exits non-zero on threshold violations.
- `mnemosyne flamegraph <heap.hprof> -o flame.svg` produces a retained-size flame graph.
- `mnemosyne query "SELECT * FROM java.lang.String WHERE @toString LIKE '%password%'"` works.
- Comparative benchmark results versus MAT and hprof-slurp are published with reproducible methodology.
- `v0.3.0` ships to all current release channels.

### Definition of done

- All P1 items in M7 are delivered and covered by tests.
- Comparative benchmark publication exists before the release tag is cut.
- Test count remains at or above 260 without regressing the existing suite (already at 377 after M7-3).
- Roadmap, STATUS, and release notes align on the shipped M7 scope.
- M1-M6 remain archive-only and are not reopened through scope creep.

## 5. M8+ Backlog — The Long Road to Full MAT Parity

M8+ absorbs the valuable work that is real, but not required for the M7 credibility jump. This is where the long tail of parity, indexing, richer time-series workflows, and desktop hardening belongs.

| # | Item | Origin | Priority | Notes |
|---|---|---|---|---|
| M8-1 | Object-level heap diff | M7 deferred | P1 | Requires stable object identity heuristics |
| M8-2 | Full OQL expansion | M7 deferred | P2 | Treadmill risk; scope carefully |
| M8-3 | Property-based parser testing | M7 deferred | P2 | `proptest` for binary parser robustness |
| M8-4 | Byte-accurate progress bars | M7 deferred | P3 | Polish |
| M8-5 | Incremental leak tracking (3+ snapshots) | D3 | P2 | Requires M7-1 streaming |
| M8-6 | IDE-native memory annotations | D4 | P3 | Requires VS Code extension / new codebase |
| M8-7 | Smart heap reduction advisor | D5 | P3 | Builds on existing analysis |
| M8-8 | Persistent index/cache | Backlog #48 | P3 | Parse-once, query-many |
| M8-9 | Tauri desktop release | M6 follow-on | P3 | Code signing, file associations, auto-update |
| M8-10 | Streaming responses (MCP) | M5 follow-on | P3 | Only if evidence shows need |

## 6. MAT Feature Parity Scorecard

This scorecard keeps the roadmap honest: it shows what already matches MAT, what M7 should close, and what stays in the longer-tail backlog.

| MAT Feature | Mnemosyne Status | Closes In |
|---|---|---|
| Dominator tree | ✅ | — |
| Retained sizes | ✅ | — |
| Leak suspects | ✅ | — |
| GC root paths | ✅ | — |
| Histogram grouping | ✅ | — |
| Thread inspection | ✅ | — |
| String analysis | ✅ | — |
| Collection inspection | ✅ | — |
| Full OQL | ⚠️ 20% | M7-4 → M8-2 |
| Object-level diff | ❌ | M8-1 |
| Large dump handling | ⚠️ M7-1 shipped; comparative validation still pending | M7-5 |
| Persistent indexes | ❌ | M8-8 |
| CI/CD automation | ✅ Better than MAT | M7-2 widens lead |
| AI-assisted diagnosis | ✅ MAT has none | Unique moat |
| Provenance tracking | ✅ MAT has none | Unique moat |
| MCP/IDE integration | ✅ MAT has none | Unique moat |

## 7. Risk Register

Only active risks belong here. Resolved risks and milestone-history detail live in the archive.

| Risk | Why it matters | Mitigation |
|---|---|---|
| Large-dump architecture still lacks 10 GB+ proof | M7 credibility depends on it | Ship overview mode first and benchmark it on larger fixtures |
| OQL scope creep | Full MAT parity is an endless tail | Keep M7 to targeted predicates; defer full expansion to M8-2 |
| Comparative claims remain unproven without head-to-head data | Performance messaging is only as good as published evidence | Make M7-5 a release gate |
| Provider-specific AI quality drift | Multi-provider UX can erode trust | Keep strict wire contracts, provider tests, and rules fallback |
| Desktop hardening could consume roadmap bandwidth early | Native packaging is not the main blocker to MAT credibility | Keep desktop release work in M8-9 until adoption data justifies it |
| Post-M7 scope sprawl | Reopening M1-M6 detail would stall delivery | Treat archive content as historical; keep M7 exit criteria explicit |
| Rare HPROF edge cases may still surface on new real-world heaps | Parser correctness risk grows with broader fixture variety | Expand fixture corpus and property-based testing in M8-3 |

## 8. Design Documents Index

These design documents remain the reference trail for completed milestones and supporting design work. Historical milestone detail lives in [roadmap-archive.md](roadmap-archive.md).

| Scope | Design Doc | Status |
|---|---|---|
| M1 — Stability & Trust | [design/milestone-1-stability-and-trust.md](design/milestone-1-stability-and-trust.md) | ✅ |
| M1.5 — Real-World Hardening | [design/milestone-1.5-real-world-hardening.md](design/milestone-1.5-real-world-hardening.md) | ✅ |
| M2 — Packaging, Releases, and DX | [design/milestone-2-packaging-releases-dx.md](design/milestone-2-packaging-releases-dx.md) | ✅ |
| M3 — Core Heap Analysis Parity | [design/milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md) | ✅ |
| M3 supporting batch | [design/m3-p1-b2-core-analysis-features.md](design/m3-p1-b2-core-analysis-features.md) | ✅ |
| M3 supporting batch | [design/M3-phase2-analysis.md](design/M3-phase2-analysis.md) | ✅ |
| M4 — UI & Usability | [design/milestone-4-ui-and-usability.md](design/milestone-4-ui-and-usability.md) | ✅ |
| M5 — AI / MCP / Differentiation | [design/milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md) | ✅ |
| M6 — Ecosystem and Community | [design/milestone-6-ecosystem-and-community.md](design/milestone-6-ecosystem-and-community.md) | ✅ |
| M6 supporting design | [design/m6-tauri-desktop-packaging.md](design/m6-tauri-desktop-packaging.md) | ✅ |
| M6 supporting design | [design/m6-plugin-extension-system.md](design/m6-plugin-extension-system.md) | ✅ |
| Scaling support design | [design/memory-scaling.md](design/memory-scaling.md) | ✅ |
| M7 supporting design | [design/milestone-7-2-ci-regression-policies.md](design/milestone-7-2-ci-regression-policies.md) | ✅ |
| M7 supporting design | [design/milestone-7-3-allocation-site-flame-graphs.md](design/milestone-7-3-allocation-site-flame-graphs.md) | ✅ |
| M7 — Production Readiness & Scale | [design/milestone-7-production-readiness.md](design/milestone-7-production-readiness.md) | ⚠️ In progress |

---

For milestone history, completed-batch detail, prior backlog tables, the original M7 proposal, and the full v0.2.0 state snapshot, see [roadmap-archive.md](roadmap-archive.md).
