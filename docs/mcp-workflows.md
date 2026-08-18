# MCP Workflow Suite

Mnemosyne's flat MCP tool surface (`detect_leaks`, `find_gc_path`, `analyze_by_referrer`, `inspect_object`, `diff_heaps`, …) gives an AI agent every primitive it needs to triage a heap dump, but not the *sequencing knowledge* for how to chain them. Today, "find and explain the worst memory leak in this heap" requires a client to already know to call `detect_leaks`, pick a suspect, call `find_gc_path`, call `explain_leak`, and decide whether `propose_fix` is warranted — that investigative muscle memory lives only in prompt engineering on the client side.

**Workflows** move that sequencing into the server. A workflow is a small, named state machine over a fixed sequence of steps, each step a thin wrapper around an existing, already-tested primitive (no new heap-analysis logic is introduced by this feature — see [`docs/design/milestone-11-mcp-workflow-suite.md`](design/milestone-11-mcp-workflow-suite.md) §3.3). A workflow instance persists server-side between calls, so a client drives it one step at a time — `start_workflow`, then `next_step` repeatedly — without having to re-derive "what comes next" itself.

Four workflow kinds ship today:

| Kind | Step sequence | Use case |
|---|---|---|
| `triage_memory_leak` | `detect` → `investigate_suspect` → `explain` → `propose_fix` → `complete` | End-to-end leak triage: find candidates, drill into the top suspect's GC-root path and referrer profile, get an explanation, optionally get a fix suggestion. |
| `tune_gc` | `root_kind_breakdown` → `thread_local_review` → `top_retainers` → `complete` | A GC-root retention review: which root kinds retain the most memory, which threads carry the largest thread-local footprint, where the dominator tree's top retainers sit. **Diagnostic only** — Mnemosyne never touches a live JVM or applies a GC flag; the output informs a human's own manual tuning decisions. |
| `traverse_object_graph` | `inspect` ⇄ `choose_direction` → `complete` | A structured walk starting from one object: inspect it, list refs in/out, pick a direction to step into next, repeat. The one workflow kind with a real branch point — `choose_direction` loops back to `inspect` on a caller-chosen id, or ends the walk when `object_id` is omitted. |
| `compare_snapshots` | `resolve_snapshots` → `diff` → `complete` | Resolve two heaps (by path or existing snapshot key), run an M10 object-level diff between them, and surface the ranked suspects. |

## The five tools

| Tool | Purpose |
|---|---|
| `describe_workflow` | Introspect a workflow kind's fixed step sequence and each step's expected input, with **no side effects** — no workflow state is created. Useful for a client deciding whether to commit to a workflow. |
| `start_workflow` | Create a new workflow instance of a given kind, run its first step, and persist the resulting state. Returns `{ workflow_id, current_step, step_result, next_expected_input }`. |
| `next_step` | Advance an in-flight workflow: execute whatever step `current_step` currently names, using `step_input` as that step's parameters. Same response shape as `start_workflow`, or `{ current_step: "complete", ... }` once the sequence is done. |
| `get_workflow` | Read-only dump of a workflow instance's full persisted state and step history (`WorkflowState`). |
| `close_workflow` | Delete a persisted workflow instance's state. Workflow state is **not** evicted automatically — see the design doc §9 R5 — so a long-lived client should call this once it is done with a workflow. |

Errors reuse the established `error_details` envelope with four workflow-specific codes: `workflow_not_found`, `workflow_step_input_mismatch` (the caller's `step_input` doesn't match what the current step expects), `workflow_already_complete` (calling `next_step` after `current_step` is already `"complete"`), and `workflow_corrupt` (a persisted workflow file failed to parse).

## How to read the transcripts below

Every transcript in this document is a **real, captured** request/response pair — produced by driving the actual MCP dispatch handler (`core::mcp::server::handle_request`, the same function `mnemosyne-cli serve`'s stdio loop calls per line) against a synthetic HPROF fixture from `core::hprof::test_fixtures` (the same fixture builders this codebase's own workflow test suites use). Nothing below is hand-written or aspirational. Two things vary run-to-run and are called out inline rather than treated as part of the contract: `workflow_id` (a `wf-<nanoseconds>` token) and the temp-file heap paths. Some large, repetitive response fields (e.g. `compare_snapshots`' full object-diff payload) are trimmed with an explicit `// … trimmed` marker for readability — the JSON shown around each trim point is otherwise verbatim.

---

## `triage_memory_leak`

Fixture: `build_graph_fixture()` — a small heap with a `com/example/BigCache` instance rooted via a Java-frame GC root, referencing one `java/lang/Object` instance. `detect_leaks` flags `BigCache` as a `CACHE`-kind leak.

**1. Introspect the workflow before committing to it:**

```json
// request
{"id":1,"method":"describe_workflow","params":{"kind":"triage_memory_leak"}}
```

```json
// response (result only)
{
  "kind": "TRIAGE_MEMORY_LEAK",
  "steps": [
    {
      "name": "detect",
      "description": "Run leak detection over the heap dump to find candidate leaks.",
      "expected_input": [
        { "name": "min_severity", "type": "string", "required": false, "description": "Lowest severity to report: LOW, MEDIUM, HIGH, or CRITICAL. Defaults to LOW so small heaps still surface candidates." },
        { "name": "package", "type": "string", "required": false, "description": "Optional package filter, matched against candidate class names." },
        { "name": "leak_types", "type": "array", "required": false, "description": "Optional explicit leak kinds to restrict detection to." }
      ],
      "underlying_primitives": ["detect_leaks"]
    },
    {
      "name": "investigate_suspect",
      "description": "Drill into the chosen suspect's GC-root path(s) and referrer profile.",
      "expected_input": [
        { "name": "leak_id", "type": "string", "required": true, "description": "The id of one of the leaks returned by the detect step." }
      ],
      "underlying_primitives": ["find_all_gc_paths", "analyze_by_referrer"]
    },
    {
      "name": "explain",
      "description": "Generate an explanation of the chosen leak (rules-based/offline by default).",
      "expected_input": [],
      "underlying_primitives": ["analyze_heap", "generate_ai_insights_async"]
    },
    {
      "name": "propose_fix",
      "description": "Optionally generate a fix suggestion for the chosen leak. Pass {\"skip\": true} to skip straight to completion.",
      "expected_input": [
        { "name": "skip", "type": "boolean", "required": false, "description": "Skip fix generation and complete the workflow." },
        { "name": "style", "type": "string", "required": false, "description": "Minimal, Defensive, or Comprehensive. Defaults to Minimal." }
      ],
      "underlying_primitives": ["propose_fix_with_config"]
    }
  ]
}
```

**2. Start the workflow (runs `detect`):**

```json
// request
{"id":2,"method":"start_workflow","params":{"kind":"triage_memory_leak","heap_path":"C:\\Users\\...\\.tmpEIJOvM"}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438305986200",
  "current_step": "investigate_suspect",
  "next_expected_input": [
    { "name": "leak_id", "type": "string", "required": true, "description": "The id of one of the leaks returned by the detect step." }
  ],
  "step_result": {
    "leak_count": 2,
    "leaks": [
      {
        "id": "com/example/BigCache::2058784d37470eae",
        "class_name": "com/example/BigCache",
        "leak_kind": "CACHE",
        "severity": "LOW",
        "instances": 2,
        "shallow_size_bytes": 4,
        "retained_size_bytes": 4,
        "suspect_score": 4.0,
        "description": "com/example/BigCache retains 4 bytes with shallow size 4 (ratio 1.00, score 4.00, 1 dominated objects,). Chain: GC Root -> com/example/BigCache"
      },
      {
        "id": "java/lang/Object::1c9d012d71529c63",
        "class_name": "java/lang/Object",
        "leak_kind": "UNKNOWN",
        "severity": "LOW",
        "instances": 1,
        "shallow_size_bytes": 0,
        "retained_size_bytes": 0,
        "suspect_score": 0.0,
        "description": "java/lang/Object retains 0 bytes with shallow size 0 (ratio 0.00, score 0.00, 0 dominated objects,). Chain: GC Root -> com/example/BigCache -> java/lang/Object"
      }
    ],
    "top_leak_ids": ["com/example/BigCache::2058784d37470eae", "java/lang/Object::1c9d012d71529c63"]
  }
}
```

**3. Advance to `investigate_suspect`, choosing the top suspect:**

```json
// request
{"id":3,"method":"next_step","params":{"workflow_id":"wf-1787011438305986200","step_input":{"leak_id":"com/example/BigCache::2058784d37470eae"}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438305986200",
  "current_step": "explain",
  "next_expected_input": [],
  "step_result": {
    "leak_id": "com/example/BigCache::2058784d37470eae",
    "class_name": "com/example/BigCache",
    "gc_path_length": 1,
    "gc_path_truncated": false,
    "referrer_entry_count": 1,
    "referrer_entries": [
      { "object_id": "0x00001000", "class_name": "com/example/BigCache", "referrer_count": 0, "retained_size": 4, "top_referrer_classes": [] }
    ]
  }
}
```

**4. Advance to `explain` (no input required):**

```json
// request
{"id":4,"method":"next_step","params":{"workflow_id":"wf-1787011438305986200","step_input":null}}
```

```json
// response (result only, current_step advances to propose_fix)
{
  "workflow_id": "wf-1787011438305986200",
  "current_step": "propose_fix",
  "next_expected_input": [
    { "name": "skip", "type": "boolean", "required": false, "description": "Skip fix generation and complete the workflow." },
    { "name": "style", "type": "string", "required": false, "description": "Minimal, Defensive, or Comprehensive. Defaults to Minimal." }
  ],
  "step_result": {
    "summary": "com/example/BigCache is retaining ~0.00 MB via 2 instances; prioritize freeing it to reclaim 1.3% of the heap.",
    "confidence": 0.7039999961853027,
    "model": "gpt-4.1-mini",
    "recommendations": [
      "Guard com/example/BigCache lifetimes: ensure cleanup hooks dispose unused entries.",
      "Add targeted instrumentation (counters, timers) around the suspected allocation sites."
    ]
    // … "wire" (raw prompt/response trace) trimmed
  }
}
```

**5. Skip fix generation and complete:**

```json
// request
{"id":5,"method":"next_step","params":{"workflow_id":"wf-1787011438305986200","step_input":{"skip":true}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438305986200",
  "current_step": "complete",
  "next_expected_input": [],
  "step_result": { "skipped": true }
}
```

**6. Close the workflow once finished with it:**

```json
// request
{"id":6,"method":"close_workflow","params":{"workflow_id":"wf-1787011438305986200"}}
```

```json
// response (result only)
{ "workflow_id": "wf-1787011438305986200", "closed": true }
```

---

## `tune_gc`

Fixture: `build_tune_gc_fixture()` — a heap with two distinct GC-root kinds: a `ROOT_THREAD_OBJECT`-rooted `java/lang/Thread` (`0x00005000`) holding a thread-local `com/example/ThreadLocalValue` (`0x00005500`), and a `ROOT_STICKY_CLASS`-rooted `com/example/CacheHolder` (`0x00006000`) holding a `com/example/Entry` (`0x00006500`).

**1. Start the workflow (runs `root_kind_breakdown`; this step takes no input):**

```json
// request
{"id":2,"method":"start_workflow","params":{"kind":"tune_gc","heap_path":"C:\\Users\\...\\.tmpBNmPPj"}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438359329100",
  "current_step": "thread_local_review",
  "next_expected_input": [
    { "name": "top_n", "type": "integer", "required": false, "description": "How many top-retaining threads to highlight. Defaults to 10." }
  ],
  "step_result": {
    "total_roots": 2,
    "root_kinds": [
      { "kind": "StickyClass", "root_count": 1, "retained_bytes": 44 },
      { "kind": "ThreadObject", "root_count": 1, "retained_bytes": 28 }
    ]
  }
}
```

Note the honesty-contract text in `describe_workflow("tune_gc")`'s own step description (not shown again here — see the `triage_memory_leak` section above for the `describe_workflow` request/response shape): *"Diagnostic data only -- Mnemosyne never touches a live JVM or applies a GC flag itself."*

**2. Advance to `thread_local_review` with an explicit `top_n`:**

```json
// request
{"id":3,"method":"next_step","params":{"workflow_id":"wf-1787011438359329100","step_input":{"top_n":5}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438359329100",
  "current_step": "top_retainers",
  "next_expected_input": [
    { "name": "top_n", "type": "integer", "required": false, "description": "How many top retainers to return. Defaults to 10." }
  ],
  "step_result": {
    "total_thread_count": 1,
    "total_thread_retained": 28,
    "threads": [
      { "name": "Thread-1", "object_id": 20480, "daemon": false, "retained_bytes": 28, "thread_local_count": 1, "thread_local_bytes": 24, "stack_trace": null }
    ]
  }
}
```

**3. Advance to `top_retainers`, which completes the workflow:**

```json
// request
{"id":4,"method":"next_step","params":{"workflow_id":"wf-1787011438359329100","step_input":{"top_n":5}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438359329100",
  "current_step": "complete",
  "next_expected_input": [],
  "step_result": {
    "top_retainers": [
      { "object_id": "0x00006000", "class_name": "com.example.CacheHolder", "retained_bytes": 44 },
      { "object_id": "0x00006500", "class_name": "com.example.Entry", "retained_bytes": 40 },
      { "object_id": "0x00005000", "class_name": "java.lang.Thread", "retained_bytes": 28 },
      { "object_id": "0x00005500", "class_name": "com.example.ThreadLocalValue", "retained_bytes": 24 }
    ]
  }
}
```

```json
// request
{"id":5,"method":"close_workflow","params":{"workflow_id":"wf-1787011438359329100"}}
```

```json
// response (result only)
{ "workflow_id": "wf-1787011438359329100", "closed": true }
```

---

## `traverse_object_graph`

Fixture: `build_simple_fixture()` — a reference chain `0x2001 --next--> 0x2002 --next--> 0x2003`, plus an object array `0x3000` referencing both `0x2001` and `0x2002` (so `0x2001`/`0x2002` have real `referrers_in` entries, not just `references_out`).

**1. Start the workflow at a chosen object (runs `inspect`):**

```json
// request
{"id":2,"method":"start_workflow","params":{"kind":"traverse_object_graph","heap_path":"C:\\Users\\...\\.tmp8FNgzZ","object_id":"0x0000000000002001"}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438391241200",
  "current_step": "choose_direction",
  "next_expected_input": [
    { "name": "object_id", "type": "string", "required": false, "description": "One of the ids from the prior inspect step's references_out/referrers_in. Omit to end the walk." }
  ],
  "step_result": {
    "object_id": "0x0000000000002001",
    "class_name": "com.example.Node",
    "shallow_size": 16,
    "retained_size": 0,
    "dominator_parent": null,
    "dominator_children": [],
    "references_out": [ { "object_id": "0x0000000000002002", "class_name": "com.example.Node" } ],
    "referrers_in": [ { "object_id": "0x0000000000003000", "class_name": "<unknown>" } ]
  }
}
```

**2. `choose_direction`: step into the referenced node — loops back to `inspect`:**

```json
// request
{"id":3,"method":"next_step","params":{"workflow_id":"wf-1787011438391241200","step_input":{"object_id":"0x0000000000002002"}}}
```

```json
// response (result only — the workflow looped back to "inspect")
{
  "workflow_id": "wf-1787011438391241200",
  "current_step": "inspect",
  "next_expected_input": [
    { "name": "object_id", "type": "string", "required": false, "description": "The object id to inspect. Required on the first call; inferred from the prior choose_direction step otherwise." }
  ],
  "step_result": { "chosen_object_id": "0x0000000000002002" }
}
```

**3. `inspect` runs again automatically against the chosen id (`object_id` omitted — inferred from context):**

```json
// request
{"id":4,"method":"next_step","params":{"workflow_id":"wf-1787011438391241200","step_input":{}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438391241200",
  "current_step": "choose_direction",
  "next_expected_input": [
    { "name": "object_id", "type": "string", "required": false, "description": "One of the ids from the prior inspect step's references_out/referrers_in. Omit to end the walk." }
  ],
  "step_result": {
    "object_id": "0x0000000000002002",
    "class_name": "com.example.Node",
    "shallow_size": 16,
    "retained_size": 0,
    "dominator_parent": null,
    "dominator_children": [],
    "references_out": [ { "object_id": "0x0000000000002003", "class_name": "com.example.Node" } ],
    "referrers_in": [
      { "object_id": "0x0000000000002001", "class_name": "com.example.Node" },
      { "object_id": "0x0000000000003000", "class_name": "<unknown>" }
    ]
  }
}
```

**4. End the walk by omitting `object_id` from `choose_direction`:**

```json
// request
{"id":5,"method":"next_step","params":{"workflow_id":"wf-1787011438391241200","step_input":{}}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438391241200",
  "current_step": "complete",
  "next_expected_input": [],
  "step_result": { "ended": true }
}
```

```json
// request
{"id":6,"method":"close_workflow","params":{"workflow_id":"wf-1787011438391241200"}}
```

```json
// response (result only)
{ "workflow_id": "wf-1787011438391241200", "closed": true }
```

---

## `compare_snapshots`

Fixture: `build_graph_fixture()` as the "before" heap, `build_tune_gc_fixture()` as the "after" heap — two structurally different synthetic heaps, so `resolve_snapshots` has to save two fresh snapshots and the `diff` step has real class-level deltas to report (`BigCache` disappears, `CacheHolder`/`Entry`/`Thread`/`ThreadLocalValue` appear).

**1. Start the workflow with raw heap paths (runs `resolve_snapshots`; no pre-existing snapshots, so both sides get parsed and cached):**

```json
// request
{"id":2,"method":"start_workflow","params":{"kind":"compare_snapshots","before_heap_path":"C:\\Users\\...\\.tmpQDME2l","after_heap_path":"C:\\Users\\...\\.tmpE5Fhtx"}}
```

```json
// response (result only)
{
  "workflow_id": "wf-1787011438429190200",
  "current_step": "diff",
  "next_expected_input": [],
  "step_result": {
    "before": { "snapshot_key": "157636311c6d282bc2d8b4228817b94bfd83a47665996ebf5f420f19c181a8a4", "heap_path": "C:\\Users\\...\\.tmpQDME2l", "snapshotted_now": true },
    "after":  { "snapshot_key": "5fa1856c9b52966ec06f3bf302a9d65d3d0c37bfd1f7a123ce079da6fa499ff4", "heap_path": "C:\\Users\\...\\.tmpE5Fhtx",  "snapshotted_now": true }
  }
}
```

Note `heap_path` in `get_workflow`'s persisted `WorkflowState` is overwritten at this point to a synthetic `"<before> -> <after>"` description — the authoritative per-side data lives in `step_result`/`WorkflowState.context` above, per this kind's "dual heap identity" design note (see [`core/src/workflow/compare_snapshots.rs`](../core/src/workflow/compare_snapshots.rs)'s module doc comment).

**2. Advance to `diff` (no input required), which completes the workflow:**

```json
// request
{"id":3,"method":"next_step","params":{"workflow_id":"wf-1787011438429190200","step_input":null}}
```

```json
// response (result only, object_diff internals trimmed — see diff_heaps in docs/api.md for the full ObjectDiffReport shape)
{
  "workflow_id": "wf-1787011438429190200",
  "current_step": "complete",
  "next_expected_input": [],
  "step_result": {
    "before": "C:\\Users\\...\\.tmpQDME2l",
    "after": "C:\\Users\\...\\.tmpE5Fhtx",
    "delta_objects": 7,
    "delta_bytes": 371,
    "class_diff": [
      { "class_name": "com/example/CacheHolder", "before_instances": 0, "after_instances": 1, "after_retained_bytes": 44, "after_shallow_bytes": 4 },
      { "class_name": "com/example/Entry", "before_instances": 0, "after_instances": 1, "after_retained_bytes": 40, "after_shallow_bytes": 40 },
      { "class_name": "java/lang/Thread", "before_instances": 0, "after_instances": 1, "after_retained_bytes": 28, "after_shallow_bytes": 4 },
      { "class_name": "com/example/ThreadLocalValue", "before_instances": 0, "after_instances": 1, "after_retained_bytes": 24, "after_shallow_bytes": 24 },
      { "class_name": "com/example/BigCache", "before_instances": 1, "after_instances": 0, "before_retained_bytes": 4, "before_shallow_bytes": 4 },
      { "class_name": "java/lang/Object", "before_instances": 1, "after_instances": 0, "before_retained_bytes": 0, "before_shallow_bytes": 0 }
    ],
    "object_diff": {
      "strategy": "ClassDominator",
      "retained_bucket_bits": 10,
      "retained_change_threshold": 1048576,
      "added": [],
      "removed": [],
      "retained_changed": []
      // … "match_quality"/"totals" trimmed
    }
    // … "changed_classes" (raw HPROF record-tag byte deltas) trimmed
  }
}
```

```json
// request
{"id":4,"method":"close_workflow","params":{"workflow_id":"wf-1787011438429190200"}}
```

```json
// response (result only)
{ "workflow_id": "wf-1787011438429190200", "closed": true }
```

---

## Error shapes

Two of the four workflow error codes, captured the same way as the transcripts above:

**Unknown `workflow_id`:**

```json
// request
{"id":1,"method":"next_step","params":{"workflow_id":"does-not-exist"}}
```

```json
// response
{
  "id": 1,
  "success": false,
  "result": null,
  "error": "workflow_not_found: no workflow found for id 'does-not-exist'",
  "error_details": {
    "code": "workflow_not_found",
    "message": "workflow_not_found: no workflow found for id 'does-not-exist'",
    "details": { "detail": "workflow_not_found: no workflow found for id 'does-not-exist'" }
  }
}
```

**Malformed `step_input`** (`investigate_suspect` requires `{"leak_id": <string>}`; a caller sends an unrelated field instead):

```json
// request
{"id":2,"method":"next_step","params":{"workflow_id":"wf-...","step_input":{"not_a_field":true}}}
```

```json
// response
{
  "id": 2,
  "success": false,
  "result": null,
  "error": "workflow_step_input_mismatch: investigate_suspect step requires {\"leak_id\": <string>}: missing field `leak_id`",
  "error_details": {
    "code": "workflow_step_input_mismatch",
    "message": "workflow_step_input_mismatch: investigate_suspect step requires {\"leak_id\": <string>}: missing field `leak_id`",
    "details": { "detail": "investigate_suspect step requires {\"leak_id\": <string>}: missing field `leak_id`" }
  }
}
```

## See also

- [`docs/design/milestone-11-mcp-workflow-suite.md`](design/milestone-11-mcp-workflow-suite.md) — the full design doc (architecture, data model, risks).
- [`docs/examples/mcp-stdio-workflow.md`](examples/mcp-stdio-workflow.md) — the flat-tool-surface stdio walkthrough this document's transcripts follow the same convention as.
- [`docs/api.md`](api.md) — source of truth for the live wire format of every MCP tool, including the five workflow tools.
