# M3 Final Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the remaining evidence-backed M3 work by adding benchmark automation wrappers, wiring the already-modeled deeper query slice, and updating milestone docs so M3 can be closed honestly.

**Architecture:** Keep this batch narrow. Reuse the existing benchmark scripts rather than replacing them, add optional-tool wrappers that fail soft when `hyperfine` or `heaptrack` are missing, and wire the query executor only for capabilities already represented in the parser/types and object-graph helpers. Close the milestone docs around the remaining evidence-only scale levers instead of implementing them speculatively.

**Tech Stack:** Bash scripts, Rust query engine, existing object-graph helpers, cargo tests, shell smoke tests, milestone/roadmap markdown

---

### Task 1: Add Optional-Tool Benchmark Automation

**Files:**
- Create: `scripts/run_hyperfine_bench.sh`
- Create: `scripts/run_heaptrack_profile.sh`
- Create: `scripts/tests/test_run_hyperfine_bench.sh`
- Create: `scripts/tests/test_run_heaptrack_profile.sh`
- Modify: `docs/performance/memory-scaling.md`
- Modify: `README.md` (only if benchmark usage guidance needs one short update)

- [ ] **Step 1: Write a shell smoke test for the missing-`hyperfine` path**
- [ ] **Step 2: Run it and verify it fails because the script does not exist yet**
- [ ] **Step 3: Implement `scripts/run_hyperfine_bench.sh` as a thin wrapper**
Requirements:
  - detect whether `hyperfine` is installed
  - print a clear skip/explanation and exit successfully when it is absent
  - when present, benchmark a small shipped command matrix against a supplied heap path
  - keep it wrapper-level; do not rewrite existing RSS scripts
- [ ] **Step 4: Re-run the shell test and verify it passes**
- [ ] **Step 5: Repeat the same red/green cycle for `scripts/run_heaptrack_profile.sh`**
Requirements:
  - detect whether `heaptrack` is installed
  - print a clear skip/explanation and exit successfully when it is absent
  - when present, profile a supplied command/heap combination and preserve the output path
- [ ] **Step 6: Add one compact doc update describing the new wrappers and their optional-tool behavior**

### Task 2: Deepen the Query Engine With Real Executor Support

**Files:**
- Modify: `core/tests/query_parser.rs`
- Modify: `core/tests/query_executor.rs`
- Modify: `core/src/query/executor.rs`
- Modify: `core/src/query/types.rs` (only if needed)
- Verify: `cli/src/main.rs`
- Verify: `core/src/mcp/server.rs`
- Modify: `docs/api.md` or `README.md` only if the supported query subset needs clarification

- [ ] **Step 1: Add a failing parser or executor test for `FROM INSTANCEOF ...` matching subclasses**
- [ ] **Step 2: Run the focused test and verify it fails for the expected reason**
- [ ] **Step 3: Add a failing executor test for projecting/filtering a real instance field from fixture data**
- [ ] **Step 4: Run the focused test and verify it fails because instance fields currently resolve to `Null`**
- [ ] **Step 5: Implement the minimal executor changes**
Requirements:
  - class matching must walk superclass links for `FROM INSTANCEOF "..."`
  - field projection must use the existing object-graph field readers when data is available
  - field filtering must compare supported scalar/object-ref-backed values honestly
  - `WHERE <field> INSTANCEOF "..."` should work only when the left side resolves to an object reference and class metadata is present
  - preserve the existing built-in-field behavior
- [ ] **Step 6: Re-run the focused tests and verify they pass**
- [ ] **Step 7: Re-run the existing query parser/executor tests and the CLI/MCP query-facing tests that cover this path**
- [ ] **Step 8: Update query docs/examples only if the supported subset changed in a user-visible way**

### Task 3: Close M3 Docs Around Evidence-Only Levers

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/design/milestone-3-core-heap-analysis-parity.md`
- Modify: `STATUS.md` (if milestone state summary needs one small update)

- [ ] **Step 1: Update the M3 remaining-work wording to remove already-closed small closeout items**
- [ ] **Step 2: Record that benchmark automation and the deeper query slice are now the final M3 follow-through items for this batch**
- [ ] **Step 3: Record that overview mode, threaded I/O, and `nom` stay backlog levers only if future profiling justifies them**
- [ ] **Step 4: Ensure no doc still implies those evidence-only levers are required before M3 can be considered complete**

### Task 4: Final Verification and M3 Completion Decision

**Files:**
- Verify: all touched files for this batch

- [ ] **Step 1: Run the new shell smoke tests**
- [ ] **Step 2: Run focused query tests**
- [ ] **Step 3: Run `cargo check`**
- [ ] **Step 4: Run `cargo test`**
- [ ] **Step 5: Run `cargo clippy --workspace --all-targets -- -D warnings`**
- [ ] **Step 6: Run `cargo fmt --all -- --check`**
- [ ] **Step 7: Verify changed-file scope with `git diff --name-only`**
- [ ] **Step 8: If all verification is green, update the remaining M3 milestone wording so M3 is treated as complete and later work starts at M4, with only future evidence-driven items called out explicitly if they remain**
