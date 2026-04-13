# Roadmap Closeout Doc Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the roadmap and milestone design docs with the already shipped M3 and M5 implementation state so the remaining roadmap closeout work can be executed safely and in the correct order.

**Architecture:** This is a documentation/design-only batch. It does not add new runtime behavior. It updates roadmap truth, milestone design status, and the decomposition of the remaining work so later implementation batches operate against current reality instead of stale milestone assumptions.

**Tech Stack:** Markdown docs under `docs/`, targeted repo searches, fresh repository verification via terminal

---

### Task 1: Align `docs/roadmap.md` With Shipped M3 and M5 State

**Files:**
- Modify: `docs/roadmap.md`
- Verify: `docs/roadmap.md`

- [ ] **Step 1: Confirm the stale roadmap markers that this batch must remove**

Run:

```bash
rg "M3 — Core Heap Analysis Parity|M5 — AI / MCP / Differentiation|⚬ Pending|⚠️ Partial" docs/roadmap.md
```

Expected: the file still shows M3 broadly in progress and M5 pending/partial in places that no longer match shipped code.

- [ ] **Step 2: Update the milestone index and immediate-next-steps text**

Edit `docs/roadmap.md` so it reflects these truths:

```md
| M3 — Core Heap Analysis Parity | [milestone-3-core-heap-analysis-parity.md](design/milestone-3-core-heap-analysis-parity.md) | ⚠️ Mostly complete — targeted follow-through remains |
| M5 — AI / MCP / Differentiation | [milestone-5-ai-mcp-differentiation.md](design/milestone-5-ai-mcp-differentiation.md) | ✅ Complete for the approved milestone scope |
```

and so Section 11 no longer treats M5 as a pending milestone. Replace that sequencing text with language that identifies:

- remaining small M3 closeout work first
- M4 as the next full open milestone
- M5 as evidence-driven follow-on only
- M6 after M4 and any justified M5 follow-on work

- [ ] **Step 3: Update the backlog notes to distinguish shipped work from true remainder**

In the backlog table and milestone text, keep the existing shipped entries but tighten the remaining items so they describe only the unresolved work:

- M3: examples, badge, IntelliJ compatibility, benchmark follow-through, OQL depth, scale/streaming experiments only where still justified
- M5: broader conversation/local-provider/streaming follow-on only

- [ ] **Step 4: Verify the roadmap no longer claims M5 is broadly pending**

Run:

```bash
rg "M5 — AI / MCP / Differentiation.*Pending|finish prompt/provider/privacy/MCP hardening" docs/roadmap.md
```

Expected: no stale broad-pending M5 wording remains.

### Task 2: Align M3 Design Docs With the Live Codebase

**Files:**
- Modify: `docs/design/milestone-3-core-heap-analysis-parity.md`
- Modify: `docs/design/M3-phase2-analysis.md`
- Verify: `docs/design/milestone-3-core-heap-analysis-parity.md`
- Verify: `docs/design/M3-phase2-analysis.md`

- [ ] **Step 1: Confirm the stale M3 status lines**

Run:

```bash
rg "READY FOR IMPLEMENTATION|Implementation Pending|M3 should deliver both" docs/design/milestone-3-core-heap-analysis-parity.md docs/design/M3-phase2-analysis.md
```

Expected: the milestone docs still describe major shipped work as pending.

- [ ] **Step 2: Update `milestone-3-core-heap-analysis-parity.md` to a shipped-plus-follow-through state**

Replace the stale top matter with wording like:

```md
> **Status:** ⚠️ Mostly complete — core parity shipped; targeted follow-through remains
> **Last Updated:** 2026-04-13
```

Update the context/scope sections so they distinguish shipped work from the true remaining items:

- richer OQL/query depth beyond built-in fields
- benchmark follow-through (`hyperfine` / `heaptrack`)
- larger-tier validation follow-through as needed
- streaming overview/threaded I/O/`nom` evaluation only as evidence-driven scale levers
- README badge / examples / IntelliJ compatibility as small closeout items

- [ ] **Step 3: Update `M3-phase2-analysis.md` so it no longer says implementation pending**

Replace the stale top matter with wording like:

```md
> **Status:** ✅ Implemented for the shipped Phase 2 scope; use this doc as historical architecture plus remaining-query follow-through context
> **Last Updated:** 2026-04-13
```

Add a short note near the top that thread inspection, classloader analysis, collection inspection, string analysis, top instances, and the initial OQL/query surface are already shipped, while richer query semantics remain future M3 follow-through.

- [ ] **Step 4: Verify the M3 docs no longer advertise shipped work as pending**

Run:

```bash
rg "READY FOR IMPLEMENTATION|Implementation Pending" docs/design/milestone-3-core-heap-analysis-parity.md docs/design/M3-phase2-analysis.md
```

Expected: no stale pending-status lines remain.

### Task 3: Align the M5 Design Doc With the Shipped Milestone Scope

**Files:**
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`
- Verify: `docs/design/milestone-5-ai-mcp-differentiation.md`

- [ ] **Step 1: Confirm the remaining stale follow-on wording in the M5 design doc**

Run:

```bash
rg "Streaming responses|Ollama/local backend|Remaining MCP Capabilities|Manual Testing Checklist" docs/design/milestone-5-ai-mcp-differentiation.md
```

Expected: the doc still mixes shipped work with still-open follow-on items.

- [ ] **Step 2: Tighten the M5 design doc to reflect the shipped milestone and true follow-on**

Edit `docs/design/milestone-5-ai-mcp-differentiation.md` so it clearly states:

- M5 is complete for the approved milestone scope
- request/response MCP transport is the shipped contract
- streaming remains conditional future work only if validated need appears
- local support currently means OpenAI-compatible local endpoints are shipped; provider-specific native local transports are future work
- broader conversation/exploration semantics remain a follow-on, not an unshipped core milestone promise

- [ ] **Step 3: Remove or rewrite stale manual-checklist items that imply already-shipped features are still pending**

Replace stale manual checklist wording with a short note describing the remaining post-M5 verification targets:

```md
Remaining follow-on validation should focus on broader conversation grounding, native local-provider transports, and streaming only if future evidence justifies it.
```

- [ ] **Step 4: Verify the M5 design doc now describes only true remaining follow-on work**

Run:

```bash
rg "Streaming responses|Ollama/local backend|Remaining MCP Capabilities" docs/design/milestone-5-ai-mcp-differentiation.md
```

Expected: any remaining references are explicitly marked as post-M5 follow-on rather than unshipped milestone scope.

### Task 4: Cross-Doc Consistency Verification For The Alignment Batch

**Files:**
- Verify: `docs/roadmap.md`
- Verify: `docs/design/milestone-3-core-heap-analysis-parity.md`
- Verify: `docs/design/M3-phase2-analysis.md`
- Verify: `docs/design/milestone-5-ai-mcp-differentiation.md`
- Verify: `docs/superpowers/specs/2026-04-13-roadmap-closeout-design.md`

- [ ] **Step 1: Verify the edited files are documentation-only**

Run:

```bash
git diff --name-only
```

Expected: only roadmap/design/spec-plan markdown files are changed in this batch.

- [ ] **Step 2: Run a targeted consistency search across the updated docs**

Run:

```bash
rg "M5 .*Pending|READY FOR IMPLEMENTATION|Implementation Pending|broader MCP/session work still remains future work" docs/roadmap.md docs/design/milestone-3-core-heap-analysis-parity.md docs/design/M3-phase2-analysis.md docs/design/milestone-5-ai-mcp-differentiation.md README.md
```

Expected: no contradictions remain in the updated design/roadmap set.

- [ ] **Step 3: Run fresh repository verification before calling the batch complete**

Run:

```bash
cargo test
```

Expected: PASS

- [ ] **Step 4: Record the next execution target**

After verification, the next batch should be the smallest remaining M3 implementation slice:

- `README.md` badge version qualifier
- real usage examples in `docs/examples/`
- IntelliJ stacktrace format compatibility
- any minimal doc sync needed by those changes
