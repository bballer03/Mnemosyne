# M3 Phase 3 ClassLoader + Query Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining M3 MAT-parity gap by shipping classloader analysis first, then the OQL-style query surface, while preserving existing CLI/MCP/report contracts.

**Architecture:** Reuse the existing `ObjectGraph`, `ClassInfo.class_loader_id`, field readers, and `DominatorTree` as the single analysis substrate. Deliver the new work in thin layers: standalone classloader analyzer, then `analyze_heap` integration, then CLI/report/MCP surfaces, then a separate query module that executes over the same immutable graph.

**Tech Stack:** Rust workspace (`mnemosyne-core`, `mnemosyne-cli`), serde, clap, existing graph/dominator infrastructure, existing CLI integration test patterns.

---

### Task 1: Standalone ClassLoader Analyzer Core

**Files:**
- Create: `core/src/analysis/classloader.rs`
- Modify: `core/src/analysis/mod.rs`
- Test: `core/tests/classloader_analysis.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
use mnemosyne_core::{
    analysis::analyze_classloaders,
    graph::build_dominator_tree,
    hprof::{
        field_types, ClassInfo, FieldDescriptor, GcRoot, GcRootType, HeapObject, ObjectGraph,
        ObjectKind,
    },
};

#[test]
fn analyze_classloaders_reports_non_bootstrap_loaders() {
    let graph = build_classloader_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_classloaders(&graph, Some(&dominator));

    assert_eq!(report.loaders.len(), 1);
    assert_eq!(report.loaders[0].loaded_class_count, 2);
    assert_eq!(report.loaders[0].instance_count, 3);
}

#[test]
fn analyze_classloaders_flags_loader_with_large_retained_graph_and_few_classes() {
    let graph = build_leaky_classloader_graph();
    let dominator = build_dominator_tree(&graph);

    let report = analyze_classloaders(&graph, Some(&dominator));

    assert_eq!(report.potential_leaks.len(), 1);
    assert!(report.potential_leaks[0]
        .reason
        .contains("loads only 1 classes"));
}
```

- [ ] **Step 2: Run the new test target and verify it fails**

Run: `cargo test -p mnemosyne-core --test classloader_analysis`
Expected: FAIL because `analysis::analyze_classloaders` and the new classloader report types do not exist yet.

- [ ] **Step 3: Implement the minimal classloader analyzer**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderInfo {
    pub object_id: ObjectId,
    pub class_name: String,
    pub loaded_class_count: usize,
    pub instance_count: usize,
    pub total_shallow_bytes: u64,
    pub retained_bytes: Option<u64>,
    pub parent_loader: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderLeakCandidate {
    pub object_id: ObjectId,
    pub class_name: String,
    pub retained_bytes: u64,
    pub loaded_class_count: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLoaderReport {
    pub loaders: Vec<ClassLoaderInfo>,
    pub potential_leaks: Vec<ClassLoaderLeakCandidate>,
}

pub fn analyze_classloaders(
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> ClassLoaderReport {
    // gather non-bootstrap loaders from ClassInfo.class_loader_id,
    // aggregate class counts + instance counts + shallow bytes,
    // derive retained bytes from the loader object when available,
    // then flag suspicious loaders with very high retained bytes per loaded class.
}
```

- [ ] **Step 4: Re-run the targeted test target and verify it passes**

Run: `cargo test -p mnemosyne-core --test classloader_analysis`
Expected: PASS with both new classloader tests green.

### Task 2: Integrate ClassLoader Analysis Into `analyze_heap`

**Files:**
- Modify: `core/src/analysis/engine.rs`
- Test: `core/tests/classloader_analysis.rs`

- [ ] **Step 1: Add a failing contract test for `AnalyzeResponse` integration**

```rust
#[tokio::test]
async fn analyze_heap_emits_classloader_report_when_enabled() {
    // create fixture-backed heap input
    // assert response.classloader_report.is_some()
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p mnemosyne-core analyze_heap_emits_classloader_report_when_enabled -- --exact`
Expected: FAIL because `AnalyzeRequest`/`AnalyzeResponse` do not carry the new classloader surface yet.

- [ ] **Step 3: Extend `AnalyzeRequest`/`AnalyzeResponse` and wire the analyzer**

```rust
pub struct AnalyzeRequest {
    pub enable_classloaders: bool,
}

pub struct AnalyzeResponse {
    pub classloader_report: Option<ClassLoaderReport>,
}

let classloader_report = request
    .enable_classloaders
    .then(|| analyze_classloaders(obj_graph, Some(dom)));
```

- [ ] **Step 4: Re-run the focused test target and verify it passes**

Run: `cargo test -p mnemosyne-core classloader -- --nocapture`
Expected: PASS for standalone and integration coverage.

### Task 3: CLI + Text Output Surface

**Files:**
- Modify: `cli/src/main.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Add a failing CLI integration test**

```rust
#[test]
fn test_analyze_classloaders_prints_loader_report() {
    let output = cmd.args(["analyze", fixture, "--classloaders"]).output().unwrap();
    assert!(normalized_stdout(&output.stdout).contains("ClassLoader Report"));
}
```

- [ ] **Step 2: Run the focused CLI test and verify it fails**

Run: `cargo test -p mnemosyne-cli test_analyze_classloaders_prints_loader_report -- --exact`
Expected: FAIL because the CLI flag and output section do not exist.

- [ ] **Step 3: Add the CLI flag and text rendering**

```rust
#[arg(long)]
classloaders: bool,

if let Some(classloaders) = &response.classloader_report {
    println!("{}", bold_label("ClassLoader Report:"));
    println!("{}", build_classloader_table(classloaders));
}
```

- [ ] **Step 4: Re-run the focused CLI test and verify it passes**

Run: `cargo test -p mnemosyne-cli test_analyze_classloaders_prints_loader_report -- --exact`
Expected: PASS.

### Task 4: MCP + Contract Alignment For ClassLoader Analysis

**Files:**
- Modify: `core/src/mcp/server.rs`
- Modify: `docs/api.md`
- Modify: `README.md`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Add/adjust a failing MCP contract test or JSON assertion**
- [ ] **Step 2: Run the contract check and verify it fails**
- [ ] **Step 3: Extend the MCP analyze/explain path to preserve the new response field**
- [ ] **Step 4: Update docs to reflect the real response shape**

Run: `cargo test -p mnemosyne-cli --test integration`
Expected: PASS with no contract drift in CLI/MCP-visible behavior.

### Task 5: OQL AST + Parser Foundation

**Files:**
- Create: `core/src/query/mod.rs`
- Create: `core/src/query/types.rs`
- Create: `core/src/query/parser.rs`
- Modify: `core/src/lib.rs`
- Test: `core/tests/query_parser.rs`

- [ ] **Step 1: Write failing parser tests for exact match, glob match, LIMIT, and invalid syntax**
- [ ] **Step 2: Run `cargo test -p mnemosyne-core --test query_parser` and watch it fail**
- [ ] **Step 3: Implement the minimal AST and recursive-descent parser**
- [ ] **Step 4: Re-run the parser tests and verify they pass**

### Task 6: OQL Executor + CLI/MCP Query Surface

**Files:**
- Create: `core/src/query/executor.rs`
- Modify: `cli/src/main.rs`
- Modify: `core/src/mcp/server.rs`
- Test: `core/tests/query_executor.rs`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Write failing executor tests against a synthetic object graph**
- [ ] **Step 2: Run the targeted executor tests and confirm failure**
- [ ] **Step 3: Implement object-scan execution for built-in fields first (`@objectId`, `@className`, `@shallowSize`, `@retainedSize`)**
- [ ] **Step 4: Add `mnemosyne query` and MCP `query_heap` on top of that executor**
- [ ] **Step 5: Re-run core + CLI query tests and verify they pass**

### Task 7: Analysis Profiles + Finish M3 Phase 3

**Files:**
- Modify: `core/src/config.rs`
- Modify: `cli/src/main.rs`
- Modify: `docs/roadmap.md`
- Modify: `STATUS.md`
- Modify: `README.md`
- Test: `cli/tests/integration.rs`

- [ ] **Step 1: Write failing config/CLI tests for `--profile` defaults**
- [ ] **Step 2: Run the focused tests and verify they fail**
- [ ] **Step 3: Implement profile selection with the smallest useful set (`incident-response`, `ci-regression`, `overview`)**
- [ ] **Step 4: Re-run tests and verify they pass**
- [ ] **Step 5: Run final validation for the milestone slice**

Run:
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

Expected: all commands pass before claiming M3 Phase 3 is complete.
