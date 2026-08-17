# Milestone 11 — MCP Workflow Suite

> **Status:** 🔲 Pending — design authored 2026-04-27, awaiting Implementation Agent pickup of Slice 11.A.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M11
> **Predecessors:** M8 (Reachability & References) ✅ shipped, M10 (Object-Level Diff) ✅ mostly shipped, M9 (Snapshot Persistence) 🟡 in progress (Slice 9.A shipped; this design assumes 9.A–9.D land before Slice 11.D, which is the only slice that needs the finished snapshot CLI/MCP surface — see §9).
> **Last updated:** 2026-04-27

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M11 |
| Type | Pure differentiator (MAT has no equivalent — Eclipse-only GUI, no agent-facing workflow surface at all) |
| Touched crates | `core` only. No `cli`/`tauri`/`ui` change — this is an MCP-only surface, matching roadmap.md's own framing ("MCP Workflow Suite," not a CLI feature). |
| Test count target | +25 to +35 net new Rust tests. |
| Memory budget | Workflows compose existing primitives; no new graph-walking algorithms, no new peak-RSS profile beyond what the composed primitives already cost individually. |

## 2. Objective

After M11, an AI agent (or IDE) talking to Mnemosyne over MCP can complete a full triage session — leak investigation, GC-root retention review, object-graph exploration, or two-snapshot comparison — through a **small, guided, stateful tool sequence**, instead of having to already know Mnemosyne's full 20+-tool surface and improvise the right call order itself.

Today, an AI agent doing "find and explain the worst memory leak in this heap" must independently know to call `detect_leaks`, pick a leak id, call `find_gc_path`, call `explain_leak`, and decide whether `propose_fix` is warranted — the *sequencing knowledge* lives only in prompt engineering on the client side, nowhere in Mnemosyne itself. M11 moves that sequencing into the server as four **named workflows**, each a small state machine over existing primitives:

1. **`triage_memory_leak`** — detect leaks, drill into the top suspect's GC-root path and referrer profile, get an AI explanation, optionally get a fix suggestion.
2. **`tune_gc`** — a GC-root retention review: which root *kinds* (thread locals, JNI globals, sticky classes, …) retain the most memory, which threads carry the largest thread-local footprint, where the dominator tree's top retainers sit relative to GC roots. Informs a human's manual GC-tuning decisions (heap sizing, generation ratios, root-cause of retention); Mnemosyne never touches a live JVM or applies any GC flag itself — the name matches roadmap.md's workflow list, but the deliverable is diagnostic data, not live tuning action, and the workflow's own `describe_workflow` output says so explicitly (honesty-contract discipline, same as every `ProvenanceKind` surface elsewhere in this codebase).
3. **`traverse_object_graph`** — a structured walk starting from one object: inspect it, list refs in/out, let the caller pick a direction to step into next, repeat. Replaces ad hoc `inspect_object`/`gc_root_path` improvisation with a session that remembers where you are.
4. **`compare_snapshots`** — open two M9 snapshots (or two heap paths, snapshotting them first if needed), run an M10 object-level diff between them, surface the ranked suspects.

Each workflow is a thin **orchestration layer**: it calls existing MCP-internal functions (the same Rust functions the existing tools already call — `detect_leaks`, `find_all_gc_paths`, `analyze_by_referrer`, `inspect_object`, `run_diff`, snapshot store methods) in a fixed, documented order, persists where the caller is in that order, and hands back one step's result plus what to call next. No new analysis logic is introduced anywhere in this milestone.

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [core/src/mcp/server.rs](../../core/src/mcp/server.rs) — `list_tools` currently registers ~20 flat, independent tools (`parse_heap`, `detect_leaks`, `analyze_heap`, `query_heap`, `find_gc_path`, `map_leak`, `explain_leak`, `chat_session`, `fix_leak`, `inspect_object`, `diff_heaps`, …). Every tool is a one-shot call; there is no existing multi-step *workflow* concept anywhere in this codebase before M11.
- [core/src/mcp/session.rs](../../core/src/mcp/session.rs) — `McpSessionStore` / `PersistedAiSession` already implement exactly the state-persistence shape M11 needs (JSON file per session, atomic write via `replace_session_file`, `session_version: u32` schema versioning, `created_at`/`updated_at` timestamps via `timestamp_now()`). M9's `SnapshotStore` (Slice 9.A, shipped) already reuses this same atomic-write helper rather than reinventing it. **M11's `WorkflowStore` is the third consumer of this exact pattern** — by this point it is clearly the established persistence idiom for this crate, not a one-off.
- **Existing session-lifecycle MCP tools** (`create_ai_session`, `resume_ai_session`, `get_ai_session`, `close_ai_session`, `chat_session` — confirmed in `core/src/mcp/server.rs` `list_tools`) are the direct structural precedent for M11's `start_workflow`/`next_step`/`describe_workflow`/`get_workflow`/`close_workflow` lifecycle. M11 does not invent a new lifecycle shape; it copies this one.
- Primitives every workflow composes, all already shipped and independently tested: `detect_leaks()` (M1/M3), `find_all_gc_paths()`/`find_gc_path()` (M8 Slice 8.A), `analyze_by_referrer()` (M8 Slice 8.B), `inspect_object()` (M8 Slice 8.C), `inspect_threads()` (M3 Phase 2, extended with frame-locals in M8 Slice 8.D), `run_diff()` (M10), `SnapshotStore` (M9 Slice 9.A, with `find_fresh_for_heap`/`list`/`remove` landing in Slice 9.B). `explain_leak`/`propose_fix` (M5 AI pipeline).

### 3.2 What MAT does

MAT has no equivalent. It is a GUI-driven, human-in-the-loop tool with no agent-facing API at all — a human clicks through the same investigation steps M11 automates. This is not a parity gap; it's a category Mnemosyne is defining, consistent with roadmap.md's framing of M11 as "Pure Differentiator." The design goal is not "match what MAT's GUI wizards do" (MAT does have some multi-step wizards, e.g. "Leak Suspects Report," but they are presentation-layer report generators, not stateful, resumable, agent-drivable sessions) — it's "give an AI agent the same investigative muscle memory a human MAT expert has, expressed as a small number of composable server-side workflows."

### 3.3 Design principle: workflows are orchestration, not new analysis

Every workflow step's *content* (what data comes back) is produced by an existing, already-tested function. M11 adds zero new heap-analysis logic. This keeps the milestone's risk profile low (same "composition, not new algorithms" pattern M8's own risk assessment used) and means a workflow step's output is exactly as trustworthy as the underlying tool's own existing test coverage — a workflow cannot introduce a *new* correctness bug in, say, GC-path enumeration, because it calls the same `find_all_gc_paths()` M8 already shipped and tested.

### 3.4 State machine shape

Each workflow kind defines a fixed, small sequence of named steps (not a general-purpose graph — deliberately simple, since the roadmap goal is "an agent can complete an entire triage session," not "a workflow scripting language"). A workflow instance is:

```
WorkflowState {
  workflow_id, kind, created_at, updated_at,
  heap_path (or snapshot key),
  current_step: <step name>,
  step_history: [ { step name, input, output_summary, timestamp } ],
  context: <workflow-kind-specific accumulated data, e.g. the chosen leak_id, the current traversal object_id>
}
```

`next_step` validates that the caller-supplied input matches what the *current* step expects, executes that step's underlying primitive call(s), advances `current_step` to whatever the state machine defines next (workflow kinds with a branch point — e.g. `traverse_object_graph`'s "which ref to follow" — take the branch choice as part of the step input), and returns the step's result plus the *name* of the next step (or `"complete"`). This mirrors `chat_session`'s existing shape (a persisted session, one call per turn, server-side state) rather than introducing a fundamentally new interaction model.

## 4. Scope

In:

1. New top-level module `core::workflow` (sibling of `mcp`, `snapshot`, `diff`, `policy` — matching this crate's flat domain-module layout; not nested inside `mcp` because the state machine logic itself has no MCP-protocol dependency, same separation-of-concerns reasoning that keeps `core::snapshot` out of `core::mcp`).
2. `WorkflowKind` enum: `TriageMemoryLeak`, `TuneGc`, `TraverseObjectGraph`, `CompareSnapshots`.
3. `WorkflowStore` — reuses `crate::mcp::session::replace_session_file` (already `pub(crate)`, made so by M9 Slice 9.A specifically for this kind of reuse) for atomic writes; own directory under the same base-dir convention as `McpSessionStore`/`SnapshotStore` (`[workflow].directory` config override, same override-key naming convention as `[ai.sessions].directory`).
4. Each workflow kind's step sequence, implemented as a small internal state machine (§6):
   - `TriageMemoryLeak`: `detect` → `investigate_suspect` (gc-path + referrer profile for the chosen leak's dominant object) → `explain` (AI) → `propose_fix` (optional, AI) → `complete`.
   - `TuneGc`: `root_kind_breakdown` (group GC roots by kind, retained size per kind) → `thread_local_review` (reuse `inspect_threads()`'s existing thread-local counts/retained bytes, M8 Slice 8.D's frame-locals where present) → `top_retainers` (dominator top-N, reuse `DominatorTree::top_retained`) → `complete`.
   - `TraverseObjectGraph`: `inspect` (current object) → `choose_direction` (caller picks a specific reference-out or referrer-in id from the just-returned list to inspect next, or ends the walk) → loops back to `inspect` on the chosen id, or `complete`.
   - `CompareSnapshots`: `resolve_snapshots` (accept either two existing snapshot keys, or two heap paths that get snapshotted first via `SnapshotStore::save` if not already cached) → `diff` (run M10's `run_diff` with `DiffMode::Object`) → `complete`.
5. MCP tools: `describe_workflow(kind)` (static: returns the step sequence and each step's expected input/output shape, no side effects, no persisted state — lets an agent introspect before committing to a workflow), `start_workflow(kind, params)` (creates a `WorkflowState`, runs the first step, returns `workflow_id` + first step's result), `next_step(workflow_id, step_input)`, `get_workflow(workflow_id)` (read-only state/history dump), `close_workflow(workflow_id)` (delete persisted state — mirrors `close_ai_session`).
6. Companion documentation `docs/mcp-workflows.md` with one worked example transcript per workflow kind (per roadmap.md's own M11 scope text: "reproducible AI-agent transcripts").
7. Validation: one scripted-agent-style integration test per workflow kind (call `start_workflow`, then `next_step` repeatedly to `complete`, assert the final state and that every intermediate step's data matches what calling the underlying primitive directly would have returned) plus contract tests for `describe_workflow`'s static schema staying in sync with the actual step sequence (roadmap.md's own risk register calls out "workflow drift if underlying analyzers change without updating workflow contracts — mitigate with contract tests," so this is a named requirement, not incidental coverage).

Out:

- **No new analyzers.** §3.3 is binding — every step's data comes from an already-shipped function.
- **No general-purpose workflow scripting / user-defined workflows.** Four fixed kinds only, per roadmap.md's explicit scope list. A plugin-style extensible workflow runtime is exactly the kind of scope creep `docs/design/m6-plugin-extension-system.md` already earmarks as deferred future work, not M11 territory.
- **No live-JVM interaction for `tune_gc`** (§2 point 2) — diagnostic data only, never applies a GC flag or touches a running process. This is a hard non-goal, not a "future work" item — Mnemosyne's whole architecture is post-mortem dump analysis, and staying honest about that in a workflow literally named `tune_gc` matters enough to state explicitly here.
- **No UI/Tauri surfacing.** MCP-only per roadmap.md's own framing of this milestone.
- **No classloader-leak workflow.** Roadmap.md explicitly defers this to "once classloader explorer ships in M13" — out of scope until M13 lands.
- **`CompareSnapshots` does not depend on M9 Slices 9.C/9.D (CLI/MCP wiring)** — it calls `SnapshotStore` methods directly from `core::workflow`, the same way it calls `run_diff` directly; it does not go through the `--snapshot` CLI flag or the `open_snapshot` MCP tool. This keeps M11 implementable without a hard blocking dependency on M9's later slices landing first (only Slice 9.A/9.B's `core::snapshot` API surface is needed, both already shipped or in flight — see predecessor note at the top of this doc).

## 5. Architecture overview

```
 MCP client (AI agent / IDE)
        │
        │ describe_workflow("triage_memory_leak")
        ▼
 core::mcp::server  →  core::workflow::describe(WorkflowKind::TriageMemoryLeak)
        │                (static step-schema lookup, no state created)
        │
        │ start_workflow("triage_memory_leak", { heap_path })
        ▼
 core::mcp::server  →  core::workflow::start(kind, params)
                          │
                          ▼
                core::workflow::triage_memory_leak::run_step("detect", ctx)
                          │  calls the SAME detect_leaks() the `detect_leaks`
                          │  MCP tool already calls -- no new logic
                          ▼
                WorkflowState persisted via WorkflowStore
                (same atomic-write shape as McpSessionStore / SnapshotStore)
                          │
                          ▼
                { workflow_id, current_step: "investigate_suspect",
                  step_result: <leak list>, next_expected_input: {...} }
        │
        │ next_step(workflow_id, { leak_id: "..." })
        ▼
 core::mcp::server  →  core::workflow::advance(workflow_id, input)
                          │  loads WorkflowState, dispatches to the kind's
                          │  step-sequence function for "investigate_suspect",
                          │  which calls find_all_gc_paths() + analyze_by_referrer()
                          │  (again: existing M8 primitives, unchanged)
                          ▼
                updated WorkflowState persisted
                          │
                          ▼
                { current_step: "explain", step_result: <gc-path + referrer data>, ... }
```

Module placement rationale: `core::workflow` is a new top-level sibling for the same reason `core::snapshot` and `core::diff` are — it is a cross-cutting orchestration concern, not a sub-concern of any single existing domain module, and nesting it under `mcp` would wrongly imply the state-machine logic has an MCP-protocol dependency (it doesn't; the *tools* that expose it do).

## 6. Data model

```rust
// core/src/workflow/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowKind {
    TriageMemoryLeak,
    TuneGc,
    TraverseObjectGraph,
    CompareSnapshots,
}

pub const WORKFLOW_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub schema_version: u32,
    pub workflow_id: String,
    pub kind: WorkflowKind,
    pub created_at: String,        // timestamp_now(), same convention as SnapshotManifest/PersistedAiSession
    pub updated_at: String,
    pub heap_path: String,
    pub current_step: String,
    pub step_history: Vec<StepRecord>,
    /// Kind-specific accumulated context (e.g. the chosen leak_id for
    /// TriageMemoryLeak, the current object_id for TraverseObjectGraph).
    /// A `serde_json::Value` map rather than a per-kind Rust struct --
    /// deliberately dynamic since each kind's context shape differs and a
    /// sum-type-of-structs would fight serde's tagging story for little
    /// benefit over a documented, tested JSON shape per kind.
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_name: String,
    pub input: serde_json::Value,
    pub output_summary: serde_json::Value,
    pub timestamp: String,
}

/// Static description of a workflow kind's step sequence -- what
/// `describe_workflow` returns. No `WorkflowState` is created by calling
/// this; it is pure metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDescription {
    pub kind: WorkflowKind,
    pub steps: Vec<StepDescription>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDescription {
    pub name: String,
    pub description: String,
    pub expected_input: Vec<ParamDescription>,   // same shape as list_tools' existing "params" entries
    pub underlying_primitives: Vec<String>,       // e.g. ["detect_leaks", "find_all_gc_paths"] -- named for transparency, so an agent (or a human debugging drift) can see exactly what a step calls
}
```

`WorkflowStore` mirrors `SnapshotStore`/`McpSessionStore` exactly: `new(root: PathBuf)`, `ensure_root()`, `save(&WorkflowState)`, `load(workflow_id) -> WorkflowState`, `remove(workflow_id)`. No `list()` requirement in scope (unlike `SnapshotStore`, there's no roadmap-stated need to enumerate all in-flight workflows — `get_workflow` on a known id is sufficient; add `list()` only if Slice 11.A's own implementer finds it trivially cheap to include, not a hard requirement).

### 6.1 Contract-test discipline (roadmap-mandated)

Per roadmap.md's M11 risk register entry ("Workflow drift if underlying analyzers change without updating workflow contracts. Mitigate with contract tests."): each workflow kind's `WorkflowDescription` (the static step list `describe_workflow` returns) must have a test asserting it matches the *actual* step names the runtime state machine transitions through — i.e., a test that runs a full workflow to completion and asserts the sequence of `current_step` values observed equals `WorkflowDescription.steps.map(|s| s.name)` in order. This is the one piece of "process" this milestone imposes beyond normal test coverage, specifically because the roadmap named this exact risk.

## 7. MCP surface

```jsonc
{ "name": "describe_workflow",
  "input": [{ "name": "kind", "type": "string", "required": true, "description": "One of: triage_memory_leak, tune_gc, traverse_object_graph, compare_snapshots." }],
  "output_schema": "WorkflowDescription" }
{ "name": "start_workflow",
  "input": [
    { "name": "kind", "type": "string", "required": true },
    { "name": "heap_path", "type": "string", "required": false, "description": "Required for triage_memory_leak/tune_gc/traverse_object_graph. Not used by compare_snapshots (see params below)." },
    { "name": "object_id", "type": "string", "required": false, "description": "traverse_object_graph only: the starting object." },
    { "name": "before_heap_path", "type": "string", "required": false, "description": "compare_snapshots only." },
    { "name": "after_heap_path", "type": "string", "required": false, "description": "compare_snapshots only." },
    { "name": "before_snapshot_key", "type": "string", "required": false, "description": "compare_snapshots only, alternative to before_heap_path." },
    { "name": "after_snapshot_key", "type": "string", "required": false, "description": "compare_snapshots only, alternative to after_heap_path." }
  ],
  "output_schema": "{ workflow_id: string, current_step: string, step_result: object, next_expected_input: object }" }
{ "name": "next_step",
  "input": [
    { "name": "workflow_id", "type": "string", "required": true },
    { "name": "step_input", "type": "object", "required": false, "description": "Shape depends on the current step -- see describe_workflow." }
  ],
  "output_schema": "same shape as start_workflow's output, or { current_step: \"complete\", ... } when done" }
{ "name": "get_workflow",
  "input": [{ "name": "workflow_id", "type": "string", "required": true }],
  "output_schema": "WorkflowState" }
{ "name": "close_workflow",
  "input": [{ "name": "workflow_id", "type": "string", "required": true }],
  "output_schema": "{ closed: true }" }
```

Error envelopes reuse the established `error_details` pattern: `workflow_not_found`, `workflow_step_input_mismatch` (caller's `step_input` doesn't match what the current step expects — e.g. calling `next_step` with a `leak_id` the `detect` step never returned), `workflow_already_complete` (calling `next_step` after `current_step` is already `"complete"`).

## 8. Sub-slice plan

All slices end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice 11.A — `core::workflow` core: state machine + `WorkflowStore` + `TriageMemoryLeak`

- **Scope:** `WorkflowState`/`WorkflowStore`/`StepRecord`/`WorkflowDescription` core types (§6). First workflow kind implemented end-to-end: `TriageMemoryLeak`, proving the whole state-machine shape works before replicating it three more times.
- **Files owned:** `core/src/workflow/mod.rs` (new), `core/src/workflow/triage_memory_leak.rs` (new), `core/tests/workflow_triage_memory_leak.rs` (new).
- **Validation gates:** full run (`start` → `next_step` × N → `complete`) on a synthetic leaky-heap fixture produces the same leak/gc-path/referrer data the equivalent direct tool calls would. Contract test (§6.1) passes. Malformed `step_input` returns `workflow_step_input_mismatch`, not a panic.
- **Target size:** ~450 LOC + ~350 LOC tests.

### Slice 11.B — `TuneGc` + `TraverseObjectGraph`

- **Scope:** Two more workflow kinds on top of the now-proven state-machine shape. `TraverseObjectGraph` is the one kind with a real branch point (§4 point 4) — its `choose_direction` step's input validation (the caller-picked id must be one of the refs/referrers the prior `inspect` step actually returned) is the interesting new logic here, everything else is composition.
- **Files owned:** `core/src/workflow/tune_gc.rs` (new), `core/src/workflow/traverse_object_graph.rs` (new), `core/tests/workflow_tune_gc.rs` (new), `core/tests/workflow_traverse_object_graph.rs` (new).
- **Validation gates:** `TuneGc`'s root-kind breakdown matches an independently-computed reference grouping. `TraverseObjectGraph` rejects a `choose_direction` input naming an id that wasn't actually offered (`workflow_step_input_mismatch`), and correctly loops `inspect` → `choose_direction` → `inspect` across at least 3 hops in a test. Both kinds' contract tests (§6.1) pass.
- **Target size:** ~400 LOC + ~350 LOC tests.

### Slice 11.C — `CompareSnapshots`

- **Scope:** Fourth workflow kind, composing `SnapshotStore` (M9 Slice 9.A/9.B) and `run_diff`/`DiffMode::Object` (M10). Handles both snapshot-key and raw-heap-path inputs (snapshotting on the fly for the latter via `SnapshotStore::save`).
- **Files owned:** `core/src/workflow/compare_snapshots.rs` (new), `core/tests/workflow_compare_snapshots.rs` (new).
- **Validation gates:** given two heap paths (no pre-existing snapshots), the workflow snapshots both and produces the same object-diff `run_diff` would produce directly. Given two already-cached snapshot keys, skips the save step. Contract test passes.
- **Target size:** ~300 LOC + ~250 LOC tests.

### Slice 11.D — MCP integration + `docs/mcp-workflows.md`

- **Scope:** Register all five tools (§7) in `core::mcp::server`'s `list_tools` + dispatch handlers. Write `docs/mcp-workflows.md` with one worked transcript per workflow kind (roadmap.md's explicit ask). If M9 Slices 9.C/9.D have landed by this point, `compare_snapshots`'s `start_workflow` params can additionally accept the `--snapshot`-flag-style key format for parity with the CLI surface — otherwise this slice proceeds using only the `core::snapshot` API directly (§4's explicit non-blocking-dependency note), and a follow-up slice reconciles the two only if real drift shows up.
- **Files owned:** `core/src/mcp/server.rs` (extend), `docs/mcp-workflows.md` (new).
- **Validation gates:** `list_tools` includes all five new tools with the schemas in §7. Each of the four worked transcripts in the companion doc is a real, captured (not hand-written/aspirational) transcript from running the actual MCP tools against a synthetic fixture — same "reproducible AI-agent transcripts" bar roadmap.md's own success criteria set for this milestone.
- **Target size:** ~250 LOC + ~200 LOC tests + documentation.

### Slice 11.E — Documentation sync

- **Scope:** `docs/roadmap.md` (mark M11 shipped, scorecard, design-doc index), `STATUS.md`, `CHANGELOG.md`, `README.md`, `ARCHITECTURE.md` (new `core::workflow` module in the project-structure tree + MCP section).
- **Files owned:** Documentation files only.
- **Validation gates:** Full-workspace `cargo {check,test,clippy,fmt}` still green. Matches the M8 Slice 8.E / M10 Slice H doc-sync precedent exactly.
- **Target size:** Documentation-only.

## 9. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | Workflow drift — an underlying analyzer's output shape changes without the workflow's `WorkflowDescription`/step contract being updated to match | §6.1's mandatory contract tests, per roadmap.md's own named risk-register entry for this exact milestone. |
| R2 | `TuneGc`'s name implies live GC tuning capability that doesn't exist | `describe_workflow("tune_gc")`'s own returned description text states the diagnostic-only, no-live-JVM-interaction scope explicitly (§4 non-goal is enforced in the user/agent-facing description, not just this doc). |
| R3 | `CompareSnapshots` implemented against `core::snapshot` directly (not the CLI/MCP-wired surface) could drift from M9's eventual `--snapshot`/`open_snapshot` semantics once 9.C/9.D land | §8 Slice 11.D explicitly calls for a reconciliation pass only if real drift is observed — avoids blocking M11 on M9's full completion while staying honest that a follow-up check is owed. |
| R4 | `WorkflowState.context`'s `serde_json::Value` typing trades compile-time safety for flexibility — a workflow-kind's step function could read the wrong key and get a silent `None`/type-mismatch instead of a compile error | Each workflow kind's own module (`triage_memory_leak.rs`, etc.) owns strongly-typed helper functions for reading/writing its own context shape, so the `Value` looseness is contained to `core::workflow::mod.rs`'s persistence boundary and never leaks into a kind's actual step logic — same discipline as how `AnalyzeResponse`'s optional fields stay strongly typed even though the overall JSON contract is additive/loose at the wire level. |
| R5 | Unbounded workflow-state accumulation on disk (analogous to M9's R4 snapshot-cache-growth risk) | Same answer as M9 R4: manual `close_workflow` only in this milestone; automatic eviction is documented future work, not silently implemented. |

## 10. Test strategy

- One full-run (`start` → `next_step`* → `complete`) integration test per workflow kind, each asserting intermediate step data matches what calling the underlying primitive directly would return (§3.3's "zero new analysis logic" claim is exactly what these tests verify).
- Contract tests (§6.1) for all four kinds.
- Negative tests: `workflow_not_found` on an unknown id, `workflow_step_input_mismatch` on malformed/out-of-band step input, `workflow_already_complete` on advancing past completion.
- `TraverseObjectGraph`'s branch-point validation (§8 Slice 11.B) gets dedicated coverage since it's the one kind with real caller-driven branching logic.

## 11. Out-of-scope (explicit non-goals)

- New analyzers or graph algorithms (§3.3).
- General-purpose/user-defined workflows (§4).
- Live-JVM GC tuning (§4, §9 R2).
- Classloader-leak workflow (deferred to post-M13 per roadmap.md).
- UI/Tauri surfacing.
- Automatic workflow-state eviction (§9 R5).

## 12. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M11 backlog entry.
- Sibling design (slice-breakdown template, additive-only-surface discipline): [milestone-8-reachability-references.md](milestone-8-reachability-references.md) (M8), [milestone-9-snapshot-persistence.md](milestone-9-snapshot-persistence.md) (M9, state-persistence template this milestone's `WorkflowStore` reuses a third time), [milestone-8-1-object-level-diff.md](milestone-8-1-object-level-diff.md) (M10, diff engine `CompareSnapshots` composes).
- Existing session-lifecycle MCP precedent: [core/src/mcp/session.rs](../../core/src/mcp/session.rs), and `create_ai_session`/`resume_ai_session`/`get_ai_session`/`close_ai_session`/`chat_session` in [core/src/mcp/server.rs](../../core/src/mcp/server.rs).
- Architecture: [ARCHITECTURE.md](../../ARCHITECTURE.md) — to be updated in Slice 11.E.

## 13. Implementation readiness verdict

**READY** — this design doc is implementation-depth. The Implementation Agent may proceed with **Slice 11.A** (core state machine + `TriageMemoryLeak`) as the first task, since it establishes the shared `core::workflow` scaffolding every later slice builds on. Slices 11.B and 11.C are independent of each other once 11.A lands (different files: `tune_gc.rs`/`traverse_object_graph.rs` vs. `compare_snapshots.rs`) and may run in parallel in **separate isolated worktrees** — this repository's own session history has already demonstrated that two implementation agents sharing one working directory can collide on branch/checkout state even when their file sets don't overlap; isolated worktrees (not just "different files") are the actual safety requirement for real parallelism here. Slice 11.D is gated behind 11.A–11.C. Slice 11.E is gated behind 11.D.
