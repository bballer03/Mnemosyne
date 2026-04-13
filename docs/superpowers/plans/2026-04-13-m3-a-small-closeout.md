# M3-A Small Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining small M3 items by adding a version-qualified README badge, shipping real workflow docs under `docs/examples/`, and making `analyze --threads` emit IntelliJ-friendly Java-style stack frames.

**Architecture:** Keep this batch narrow and reuse existing surfaces. The README badge is a one-line doc change, `docs/examples/` becomes a small scenario-oriented index plus two real walkthroughs, and IntelliJ compatibility is implemented by normalizing the existing text-mode thread stack rendering in `cli/src/main.rs` rather than adding a new command or output mode.

**Tech Stack:** Markdown docs, Rust CLI, existing HPROF test fixtures, `cargo test`

---

### Task 1: Add the Version-Qualified README Badge

**Files:**
- Modify: `README.md`
- Verify: `README.md`

- [ ] **Step 1: Confirm the current badge still uses the generic alpha label**

Run:

```bash
rg "status-alpha-yellow|shields.io" README.md
```

Expected: the README still contains `status-alpha-yellow`.

- [ ] **Step 2: Update the status badge to the documented qualifier**

Edit `README.md` so the existing status badge line changes from:

```md
![status](https://img.shields.io/badge/status-alpha-yellow?style=flat-square)
```

to:

```md
![status](https://img.shields.io/badge/status-v0.2.0--alpha-yellow?style=flat-square)
```

Use the double hyphen escape in the Shields value so the rendered badge text is `v0.2.0-alpha`.

- [ ] **Step 3: Verify the README now carries the version-qualified badge**

Run:

```bash
rg "status-v0.2.0--alpha-yellow" README.md
```

Expected: one match.

### Task 2: Replace Placeholder Examples With Real Workflow Docs

**Files:**
- Modify: `docs/examples/README.md`
- Create: `docs/examples/cli-analysis-workflow.md`
- Create: `docs/examples/mcp-stdio-workflow.md`
- Verify: `docs/examples/README.md`
- Verify: `docs/examples/cli-analysis-workflow.md`
- Verify: `docs/examples/mcp-stdio-workflow.md`

- [ ] **Step 1: Confirm the examples directory is still placeholder-level**

Run:

```bash
rg "lightweight landing page|source-of-truth examples live|does not ship the older example markdown files" docs/examples/README.md
```

Expected: the current README still describes `docs/examples/` as lightweight.

- [ ] **Step 2: Rewrite `docs/examples/README.md` into a short index page**

Replace the current landing-page copy with a concise index that:

- states this directory now contains real workflow docs
- links to `cli-analysis-workflow.md`
- links to `mcp-stdio-workflow.md`
- links back to `docs/api.md`, `docs/configuration.md`, and `docs/QUICKSTART.md` for reference material

Use content in this shape:

```md
# Examples

This directory contains scenario-oriented examples for the shipped Mnemosyne CLI and MCP surface.

## Available Workflows

- [`cli-analysis-workflow.md`](cli-analysis-workflow.md) — parse, leaks, analyze, diff, explain, map, and GC-path in one operator flow
- [`mcp-stdio-workflow.md`](mcp-stdio-workflow.md) — stdio MCP requests for discovery, heap analysis, and AI-session follow-up

## Reference Docs

- [`../QUICKSTART.md`](../QUICKSTART.md)
- [`../api.md`](../api.md)
- [`../configuration.md`](../configuration.md)
```

- [ ] **Step 3: Create `docs/examples/cli-analysis-workflow.md` with a real CLI scenario**

Write a new example doc that uses only shipped commands. Include:

1. a short intro explaining the scenario (`heap.hprof` from a service incident)
2. `parse` for a quick read
3. `leaks` with `--min-severity`, `--package`, and `--leak-kind`
4. `analyze` with `--group-by package --threads --strings --collections --classloaders --top-instances --top-n 10 --min-capacity 32`
5. `diff` for before/after comparison
6. `explain`, `map`, and `gc-path` as follow-up investigation
7. a short notes section that explains:
   - graph-backed first, heuristic fallback with provenance
   - `analyze` is the richer path
   - `diff` gains class-level deltas when both snapshots build object graphs

Use command blocks in this style:

```md
## 1. Quick Parse

```bash
mnemosyne-cli parse heap.hprof
```
```

Keep the page compact and workflow-oriented rather than turning it into a full cookbook.

- [ ] **Step 4: Create `docs/examples/mcp-stdio-workflow.md` with a live-method transcript**

Write a new example doc that shows one realistic stdio MCP workflow. Include:

1. starting `mnemosyne-cli serve`
2. `list_tools`
3. `parse_heap`
4. `detect_leaks`
5. `analyze_heap`
6. one short AI-session sequence using:
   - `create_ai_session`
   - `chat_session`

Use one-line JSON request examples, for example:

```json
{"id":1,"method":"list_tools","params":{}}
```

Add a short note that responses remain single-line JSON over stdio and that `docs/api.md` is the wire-format source of truth.

- [ ] **Step 5: Verify the new example docs are linked and scenario-oriented**

Run:

```bash
rg "cli-analysis-workflow|mcp-stdio-workflow|create_ai_session|chat_session|mnemosyne-cli diff" docs/examples
```

Expected: matches in the new files and index.

### Task 3: Make `analyze --threads` Emit IntelliJ-Friendly Stack Frames

**Files:**
- Modify: `cli/tests/integration.rs`
- Modify: `cli/src/main.rs`
- Modify: `core/src/hprof/test_fixtures.rs` (only if needed for a fixture with real stack frames)
- Verify: `cli/tests/integration.rs`
- Verify: `cli/src/main.rs`

- [ ] **Step 1: Add a failing CLI integration test for Java-style stack-frame output**

In `cli/tests/integration.rs`, add a new test near the existing thread-report coverage. Use a fixture that includes a real thread root plus stack frames. The test should:

1. run `mnemosyne-cli analyze <fixture> --threads`
2. assert success
3. inspect ANSI-stripped stdout
4. assert the thread report contains canonical Java-style frame lines such as:

```text
at com.example.WorkerThread.run(WorkerThread.java:123)
at com.example.WorkerThread.mainLoop(Unknown Source)
```

If the existing `build_graph_fixture()` is too shallow, use a new exported fixture from `core::hprof::test_fixtures` dedicated to thread stacks.

- [ ] **Step 2: Run the new test and verify it fails for the expected reason**

Run:

```bash
cargo test -p mnemosyne-cli test_analyze_with_threads_flag_emits_java_style_stack_frames -- --exact
```

Expected: FAIL because the current renderer prints a bare `at com.example.WorkerThread.mainLoop` line without `(Unknown Source)`.

- [ ] **Step 3: Add a dedicated fixture with real stack frames if the current graph fixture does not exercise this path**

If needed, extend `core/src/hprof/test_fixtures.rs` with a new helper such as `build_thread_stack_fixture()` that:

- uses the existing stack-frame and stack-trace record helpers
- creates a `java.lang.Thread`-compatible object graph with one thread root
- includes one frame with a positive line number and one frame with an unknown line number

Prefer reusing the current test-builder helpers (`add_stack_frame`, `add_stack_trace`, `add_gc_root_thread_obj`) instead of inventing a second fixture style.

- [ ] **Step 4: Implement the minimal renderer change in `print_thread_stacks()`**

Update `cli/src/main.rs` so `print_thread_stacks()` keeps the current thread header but normalizes frame lines as follows:

- `(Some(source_file), line) if line > 0` → `at Class.method(File.java:123)`
- `(Some(source_file), -2)` → `at Class.method(Compiled Method)`
- `(Some(source_file), _)` → `at Class.method(File.java)`
- `(None, -2)` → `at Class.method(Compiled Method)`
- `(None, _)` → `at Class.method(Unknown Source)`

Keep the implementation local to `print_thread_stacks()` unless a tiny helper is clearly cleaner.

- [ ] **Step 5: Run the focused test and verify it passes**

Run:

```bash
cargo test -p mnemosyne-cli test_analyze_with_threads_flag_emits_java_style_stack_frames -- --exact
```

Expected: PASS.

- [ ] **Step 6: Re-run the existing thread-report integration test**

Run:

```bash
cargo test -p mnemosyne-cli test_analyze_with_threads_flag -- --exact
```

Expected: PASS.

### Task 4: Final Batch Verification and Minimal Doc Sync

**Files:**
- Verify: `README.md`
- Verify: `docs/examples/README.md`
- Verify: `docs/examples/cli-analysis-workflow.md`
- Verify: `docs/examples/mcp-stdio-workflow.md`
- Verify: `cli/src/main.rs`
- Verify: `cli/tests/integration.rs`
- Verify: `core/src/hprof/test_fixtures.rs` (if changed)

- [ ] **Step 1: Verify the worktree scope for this batch**

Run:

```bash
git diff --name-only
```

Expected: the branch may still contain earlier Phase 0 doc edits, but the new M3-A files should be limited to the README/examples/thread-stack paths for this batch.

- [ ] **Step 2: Run focused searches for the three closeout items**

Run:

```bash
rg "v0.2.0-alpha|cli-analysis-workflow|mcp-stdio-workflow|Unknown Source|Compiled Method" README.md docs/examples cli/src/main.rs cli/tests/integration.rs core/src/hprof/test_fixtures.rs
```

Expected: matches confirm the badge, examples, and stacktrace formatting landed.

- [ ] **Step 3: Run the full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Record the next execution target**

After this batch passes, continue with the next M3 follow-through batch rather than jumping ahead to M4:

- M3-B security / Dockerfile CVE triage
- or M3-C benchmark / scale follow-through

Choose the next batch based on current branch priorities and environment readiness.
