# Milestone 7-2 — CI Regression Policies (`mnemosyne ci-check`)

> **Status:** 🟡 Pending — design in progress, implementation not started
> **Predecessor:** M7-1 streaming overview mode ✅ shipped (commits `7e27e0e`, `8ceb43b`, `c128131`, `995f92b`, `8534f37`, `09ee9c5`, `c64b4a1`)
> **Owner (design):** Design Consulting Agent
> **Owner (implementation):** Implementation Agent (per slice)
> **Parent design:** [milestone-7-production-readiness.md](milestone-7-production-readiness.md) §7
> **Roadmap reference:** [docs/roadmap.md §4](../roadmap.md)
> **Last updated:** 2026-04-26

This addendum is the implementation-depth design for M7-2. It supersedes the framing in §7 of the parent M7 design doc for the purpose of coding. Slices defined here (M7-2.A through M7-2.E) are gated by the `READY` verdict in §19 below.

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone slice | M7-2 |
| Type | Differentiation |
| Predecessor | M7-1 (streaming overview mode) |
| Required prerequisites | `core::analysis::AnalysisMode`, `core::hprof::overview::OverviewSummary`, deep-mode `AnalyzeResponse` (all present on `main`) |
| New crates | none — additive to `core/` and `cli/` |
| Cargo.toml deltas (anticipated) | `toml` (already a dev-dep — promote to runtime in `core`); `quick-xml` for JUnit emission (`core` only); no new MSRV impact |
| Test count target | +28 to +35 net new tests (push workspace from 268 → ≥ 295) |

## 2. Objective

`mnemosyne ci-check <heap.hprof> --policy policy.toml` reads a heap dump, runs analysis (deep or overview, auto-resolved or explicit), evaluates a TOML-defined policy against the analysis result, and exits non-zero when violations meet or exceed a configurable severity threshold. Output is renderable as human-readable text, structured JSON, JUnit XML, or GitHub-Actions workflow commands so any CI server can ingest results without custom glue code.

## 3. Context

The competitive landscape for CI-driven heap regression detection is empty:

- **Eclipse MAT** ships no `ci-check` equivalent; CI usage requires hand-rolled scripting against MAT's batch-mode reports and bespoke pass/fail logic.
- **hprof-slurp** has no policy layer at all; it is a streaming summarizer.
- **YourKit / JProfiler** are GUI-first, not CI-shaped.

Real CI use cases this targets:

1. **Regression detection on JVM service heap dumps captured during integration tests.** A test harness produces an `.hprof` after a stress run; CI fails the build if total bytes, top-class growth, or specific leak suspects cross a threshold versus a checked-in baseline policy.
2. **Pre-merge gating on PR-built service images.** A short-lived JVM is dumped after a smoke run; `ci-check` enforces that no `ClassLoader` leaks appear and that `byte[]` instances stay below a budget.
3. **Nightly soak runs.** A long-running JVM is periodically dumped; `ci-check` produces JUnit XML so existing test dashboards (Jenkins, GitHub Actions test reporter) display heap-policy results alongside unit/integration tests.

This is pure differentiation territory. M7-2 widens the moat opened by M5/M6 (CI-friendly JSON, MCP, structured outputs) into a first-class, opinionated CI workflow.

## 4. Scope

In:

- New CLI subcommand `mnemosyne ci-check` with the surface defined in §7.
- New `core::policy` module (sibling of `core::analysis`, `core::report`, `core::query`):
  - `Policy` schema (TOML-backed) and parser.
  - Predicate set defined in §9.
  - Evaluator producing a `PolicyResult` (§10).
  - Mode-compatibility enforcement (§13).
- Severity ladder and `--fail-on` exit-code contract (§§11–12).
- Output formats: `text`, `json`, `junit`, `github-actions` (§14).
- Drop-in integration snippets under `docs/integrations/` for GitHub Actions and Jenkins (§15).

Out:

- **No policy-as-code DSL beyond TOML.** No Rhai, Lua, JS, or DSL evaluator.
- **No remote policy fetching.** Policies are local files. URL/HTTP loading is deferred.
- **No historical baselines or trend tracking.** That is M8-1 (object-level diff) and M8-5 (incremental leak tracking).
- **No auto-remediation.** Violations are reported, never patched.
- **No PR commenting.** Producing GitHub-Actions annotations is supported; posting PR comments is the consumer workflow's job.
- **No streaming policy evaluation.** Policy runs against a fully resolved analysis result. Streaming policy is M8-10 territory.
- **No breaking changes** to existing `parse`, `analyze`, `leaks`, `diff`, `query`, `map`, `gc-path`, `explain`, `chat`, `fix`, or `serve` subcommands.

## 5. Architecture overview

```
┌──────────────────────┐
│ mnemosyne ci-check   │  CLI entry: cli/src/main.rs
│  args parse (clap)   │
└──────────┬───────────┘
           │ CiCheckArgs
           ▼
┌──────────────────────┐
│ Policy load & lint   │  core::policy::load_policy(path) -> Policy
│  (TOML → typed)      │  fail-fast on schema errors → exit 2
└──────────┬───────────┘
           │ Policy
           ▼
┌──────────────────────┐
│ Mode resolution      │  core::analysis::AnalysisMode (existing)
│ (auto/deep/overview) │  + policy mode-compatibility check (§13)
└──────────┬───────────┘
           │ AnalysisMode (resolved)
           ▼
┌──────────────────────┐
│ Heap analysis        │  Deep:    core::analysis::analyze_heap
│                      │  Overview:core::hprof::parse_hprof_overview_file
└──────────┬───────────┘
           │ AnalyzeResponse OR OverviewSummary
           │ (unreadable heap → exit 3)
           ▼
┌──────────────────────┐
│ Policy evaluator     │  core::policy::evaluate(&policy, &input)
│  (per-predicate)     │  -> PolicyResult { violations, evaluations, ... }
└──────────┬───────────┘
           │ PolicyResult
           ▼
┌──────────────────────┐
│ Renderer             │  core::policy::render::{text,json,junit,github_actions}
│  (--format)          │  → stdout or --output file
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ Exit code mapping    │  --fail-on threshold → 0 / 1 / 4
│                      │  schema/IO errors    → 2 / 3
└──────────────────────┘
```

Module placement rationale:

- `core::policy` is **not** part of `core::analysis` because evaluation operates on already-finalized analysis output. It is a consumer of analysis, not a step inside the pipeline.
- `core::policy` is **not** part of `core::report` because a `PolicyResult` is itself a structured artifact that gets rendered; rendering lives in `core::policy::render`, mirroring how `core::report` owns analysis-result rendering.

## 6. CLI surface

Exact `clap` derive structure to be added to `cli/src/main.rs` (additive — does not touch the existing `Commands` enum variants):

```rust
#[derive(Subcommand, Debug)]
enum Commands {
    // ...existing variants unchanged...
    /// Evaluate a heap dump against a TOML policy and emit a CI-shaped result.
    CiCheck(CiCheckArgs),
}

#[derive(Debug, Parser)]
struct CiCheckArgs {
    /// Heap dump to analyze.
    heap: PathBuf,

    /// Policy file (TOML).
    #[arg(long, value_name = "FILE")]
    policy: PathBuf,

    /// Analysis mode. `auto` resolves by file size (existing behavior).
    #[arg(long, value_enum, default_value_t = ModeArg::Auto)]
    mode: ModeArg,

    /// Output format.
    #[arg(long, value_enum, default_value_t = CiFormat::Text)]
    format: CiFormat,

    /// Write output to a file instead of stdout.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Minimum severity that flips the exit code to non-zero.
    #[arg(long, value_enum, default_value_t = SeverityArg::Error)]
    fail_on: SeverityArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CiFormat {
    Text,
    Json,
    Junit,
    GithubActions,
}
```

`ModeArg` already exists from M7-1. `SeverityArg` is **new for `ci-check`** and lives in `cli/src/main.rs` alongside the other CLI enums; it maps 1:1 to `core::policy::Severity` (§11). It is distinct from the existing `--min-severity` enum on `leaks` (which maps to `LeakSeverity`); keeping them separate prevents accidental coupling between leak gating and policy gating.

Help text examples (rendered by clap from doc-comments above; reproduced here for review):

```text
mnemosyne ci-check <HEAP> --policy <FILE> [--mode auto|deep|overview]
                                           [--format text|json|junit|github-actions]
                                           [--output <PATH>]
                                           [--fail-on info|warning|error|critical]
```

## 7. Policy schema (TOML)

A complete, annotated example exercising every supported predicate:

```toml
# Optional metadata block. Surfaces in renderer output.
[meta]
name        = "checkout-service-heap-policy"
description = "Heap regression gates for the checkout service nightly soak."
version     = "1.0"

# Defaults applied to every rule unless the rule overrides them.
[defaults]
severity = "error"   # info | warning | error | critical

# ── Overview-compatible predicates ─────────────────────────────────────────────

[[rule]]
id        = "total-heap-budget"
predicate = "total_bytes"
op        = "<="
value     = 2_147_483_648   # 2 GiB
severity  = "error"
remediation_hint = "Total heap exceeds 2 GiB budget — investigate retained roots."

[[rule]]
id        = "instance-count-budget"
predicate = "total_instances"
op        = "<="
value     = 5_000_000

[[rule]]
id        = "byte-array-instance-cap"
predicate = "class_instances"
class     = "byte[]"           # exact match; see `class_pattern` below for regex
op        = "<="
value     = 250_000

[[rule]]
id        = "string-bytes-cap"
predicate = "class_bytes"
class_pattern = "^java\\.lang\\.String$"   # regex; mutually exclusive with `class`
op        = "<="
value     = 314_572_800        # 300 MiB
severity  = "warning"

[[rule]]
id        = "loaded-class-ceiling"
predicate = "loaded_class_count"
op        = "<="
value     = 50_000

[[rule]]
id        = "thread-roots-cap"
predicate = "gc_root_count"
kind      = "thread_object"    # see GcRootCounts in core::hprof::overview
op        = "<="
value     = 200

[[rule]]
id        = "no-synthetic-provenance"
predicate = "provenance_must_not_contain"
kind      = "synthetic"        # matches ProvenanceKind variants (lowercase)
severity  = "warning"
remediation_hint = "Result contains synthetic markers — confirm fixtures are real heap dumps."

# ── Deep-only predicates ───────────────────────────────────────────────────────

[[rule]]
id        = "no-critical-leaks"
predicate = "leak_count"
severity_filter = "critical"   # only count leaks at this severity
op        = "=="
value     = 0

[[rule]]
id        = "retained-cache-cap"
predicate = "retained_size"
scope     = "class"             # "class" | "leak_suspect"
class_pattern = "^com\\.example\\.cache\\."
op        = "<="
value     = 524_288_000         # 500 MiB

[[rule]]
id        = "dominator-root-explosion"
predicate = "dominator_root_count"
op        = "<="
value     = 100_000
severity  = "warning"
```

Field-by-field semantics:

| Field | Where | Meaning |
|---|---|---|
| `[meta].name` | top-level | Identifier surfaced in JUnit `testsuite name` and text output header. |
| `[meta].description` | top-level | Free text shown in human renderer. |
| `[meta].version` | top-level | Schema-author version, not Mnemosyne version. Reserved for future migrations. |
| `[defaults].severity` | top-level | Default severity applied to rules that omit it. |
| `[[rule]].id` | per rule | **Required.** Stable identifier; surfaced in JSON, JUnit `testcase name`, and GitHub-Actions annotations. Must be unique per policy file (loader rejects duplicates). |
| `[[rule]].predicate` | per rule | **Required.** One of the names in §9. |
| `[[rule]].op` | per rule | Comparison: `<`, `<=`, `>`, `>=`, `==`. Required for numeric predicates; ignored for `provenance_must_not_contain`. |
| `[[rule]].value` | per rule | Numeric threshold for numeric predicates. |
| `[[rule]].severity` | per rule | Override of `[defaults].severity`. |
| `[[rule]].remediation_hint` | per rule | Optional text included in violation messages. |
| `[[rule]].class` | per rule | Exact class-name match. Mutually exclusive with `class_pattern`. |
| `[[rule]].class_pattern` | per rule | Regex (`regex` crate syntax) match. Loader pre-compiles and rejects invalid patterns. |
| `[[rule]].kind` | per rule | Enum value for `gc_root_count` (root kind) or `provenance_must_not_contain` (provenance kind). |
| `[[rule]].severity_filter` | `leak_count` | Only count leaks at the given severity (or higher; comparison resolved by §11 ordering). |
| `[[rule]].scope` | `retained_size` | `class` (sum across instances of matched class) or `leak_suspect` (per-suspect retained size). |

Loader rules:

- Unknown top-level keys → load error (exit 2).
- Unknown per-rule keys → load error.
- Missing required keys → load error with rule `id` (or table index when `id` is absent) in the message.
- Duplicate rule `id` → load error.
- Both `class` and `class_pattern` set → load error.
- Predicate-incompatible keys present (e.g. `class` on `total_bytes`) → load error.

## 8. Predicate catalog

| Predicate | TOML key | Mode requirement | Inputs read from analysis result | Operators | Severity sources | Notes |
|---|---|---|---|---|---|---|
| Total payload bytes | `total_bytes` | overview ✅ / deep ✅ | Deep: `summary.total_size_bytes`. Overview: `OverviewSummary.total_size_bytes`. | `<` `<=` `>` `>=` `==` | rule | The same field name is used in both modes; renderer notes which mode produced it. |
| Total instances | `total_instances` | overview ✅ / deep ✅ | Deep: `summary.total_instances`. Overview: `OverviewSummary.total_instances + total_object_arrays` (+ primitive arrays counted as 1 each — see open question §18-Q3). | `<` `<=` `>` `>=` `==` | rule | |
| Per-class instance count | `class_instances` | overview ✅ / deep ✅ | Deep: histogram entry for matched class. Overview: matching entry in `top_classes_by_instances`. | `<` `<=` `>` `>=` `==` | rule | If overview top-N does not contain the matched class, evaluator emits an `info`-severity `Evaluation` noting the class fell below the top-N cutoff and the rule is **skipped** (not violated). |
| Per-class byte count | `class_bytes` | overview ✅ / deep ✅ | Deep: histogram entry. Overview: `top_classes_by_bytes` `approx_shallow_bytes`. | `<` `<=` `>` `>=` `==` | rule | Overview values are **approximate shallow bytes** (HPROF record payload). Renderer always labels overview-sourced numbers as approximate. |
| Loaded class count | `loaded_class_count` | overview ✅ / deep ✅ | Deep: derived from class table size. Overview: `OverviewSummary.loaded_class_count`. | `<` `<=` `>` `>=` `==` | rule | |
| GC root count by kind | `gc_root_count` | overview ✅ / deep ✅ | Deep: graph metrics `gc_roots`. Overview: `GcRootCounts.<kind>`. | `<` `<=` `>` `>=` `==` | rule | `kind` enum: `jni_global`, `jni_local`, `java_frame`, `native_stack`, `sticky_class`, `thread_block`, `monitor_used`, `thread_object`, `other`. |
| Provenance must not contain | `provenance_must_not_contain` | overview ✅ / deep ✅ | `AnalyzeResponse.provenance` (and any leak-level provenance for deep). | n/a | rule | `kind` enum mirrors `core::analysis::ProvenanceKind` lowercase variants. Violation if any marker of that kind is present. |
| Leak count | `leak_count` | **deep only** | `AnalyzeResponse.leaks`, optionally filtered by `severity_filter`. | `<` `<=` `>` `>=` `==` | rule | Severity comparison uses §11 ordering. |
| Retained size | `retained_size` | **deep only** | `scope = "class"` → sum of `LeakInsight.retained_size_bytes` for matched class; `scope = "leak_suspect"` → max `retained_size_bytes` across matched leaks. | `<` `<=` `>` `>=` `==` | rule | Operates on leak-derived retained sizes (already ranked); does not re-walk the dominator tree. |
| Dominator root count | `dominator_root_count` | **deep only** | `AnalyzeResponse.graph.dominators` filtered to entries with `immediate_dominator == None` (true dominator roots). | `<` `<=` `>` `>=` `==` | rule | The existing `GraphMetrics` struct exposes `dominators: Vec<DominatorNode>` and `node_count`/`edge_count`; no new field is required on `GraphMetrics` itself. |

Examples:

```toml
[[rule]]
id = "no-critical-leaks"
predicate = "leak_count"
severity_filter = "critical"
op = "=="
value = 0
severity = "critical"
```

```toml
[[rule]]
id = "byte-array-bytes-cap"
predicate = "class_bytes"
class = "byte[]"
op = "<="
value = 1_073_741_824
```

## 9. Result types

```rust
// core/src/policy/result.rs
use serde::{Deserialize, Serialize};
use crate::analysis::{AnalysisMode, ProvenanceKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    pub policy_name: Option<String>,
    pub heap_path: String,
    pub mode_requested: AnalysisMode,
    pub mode_used: AnalysisMode,
    pub evaluations: Vec<Evaluation>,
    pub violations: Vec<Violation>,
    pub skipped: Vec<SkippedRule>,
    pub provenance: Vec<ProvenanceMarker>, // mirror analysis-level provenance
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    pub policy_id: String,
    pub predicate: String,
    pub actual: ActualValue,
    pub expected: ExpectedConstraint,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub policy_id: String,
    pub severity: Severity,
    pub message: String,
    pub actual: ActualValue,
    pub expected: ExpectedConstraint,
    pub remediation_hint: Option<String>,
    pub source_line: Option<u32>, // line in the policy TOML file
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedRule {
    pub policy_id: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Class targeted by `class` / `class_pattern` not present in overview top-N.
    ClassBelowTopN { class_pattern: String },
    /// Mode auto-resolved to overview but rule requires deep.
    DeepOnlyPredicateInOverviewMode { predicate: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActualValue {
    Integer(u64),
    Float(f64),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedConstraint {
    pub op: String,        // "<=", "==", etc; "must_not_contain" for non-numeric
    pub value: ActualValue,
}
```

## 10. Severity ladder

Defined in `core::policy::Severity`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,      // < default fail-on
    Warning,   // < default fail-on
    Error,     // == default fail-on (fails build by default)
    Critical,  // > default fail-on
}
```

Ordering is total: `Info < Warning < Error < Critical`. `--fail-on <S>` flips the exit code to `1` whenever any violation has severity `>= S`. Default `--fail-on error` matches conventional CI policy gates.

This enum is **distinct** from `core::analysis::LeakSeverity` (`Low | Medium | High | Critical`). They live in separate crates' subsystems and serialize differently (lowercase vs SCREAMING_SNAKE_CASE). The `severity_filter` key on the `leak_count` predicate accepts the `LeakSeverity` vocabulary (`low | medium | high | critical`); the policy-level `severity` key accepts the `Severity` vocabulary (`info | warning | error | critical`). Loader validates both with explicit enum-name parsers and rejects cross-vocabulary values.

## 11. Exit codes

| Code | Meaning | When |
|---|---|---|
| `0` | Clean | No violations, OR all violations below `--fail-on`. |
| `1` | Policy violation | At least one violation with severity `>= --fail-on`. |
| `2` | Policy file invalid | TOML parse error; schema error; unknown key; duplicate rule id; invalid regex; both `class` and `class_pattern` set; etc. |
| `3` | Heap unreadable | HPROF parse failure; file not found; permission denied. |
| `4` | Mode mismatch | `--mode overview` (explicit) and policy contains deep-only predicates. See §13. |

Exit code mapping is performed by the CLI handler, not the core evaluator. The evaluator returns a `PolicyResult` and a `Result<(), CoreError>` for IO/schema errors. The CLI maps these onto exit codes deterministically and does not panic on any user-input failure.

## 12. Mode interaction

Two-axis decision:

| `--mode` request | Policy contains deep-only predicates? | Behavior |
|---|---|---|
| `deep` | yes | Run deep, evaluate all rules. |
| `deep` | no | Run deep, evaluate all rules. |
| `overview` (explicit) | **yes** | **Fail loud at policy load:** exit 4 with message listing the offending rule ids. Do not parse the heap. |
| `overview` (explicit) | no | Run overview, evaluate all rules. |
| `auto` → resolves to deep | yes | Run deep, evaluate all rules. |
| `auto` → resolves to deep | no | Run deep, evaluate all rules. |
| `auto` → resolves to overview | yes | Run overview, **warn-and-skip** deep-only rules: emit `SkippedRule { reason: DeepOnlyPredicateInOverviewMode }` per skipped rule, evaluate the rest. Exit `0` or `1` based on remaining violations vs `--fail-on`. |
| `auto` → resolves to overview | no | Run overview, evaluate all rules. |

Rationale:

- **Explicit overview + deep predicates → fail loud.** The user intentionally asked for triage mode but wrote a policy that requires deep analysis. This is a configuration mistake; surfacing it early is cheaper than running analysis and silently skipping.
- **Auto-resolved overview + deep predicates → warn and skip.** Auto mode is a size-driven convenience. The user did not explicitly choose overview, and skipping deep-only checks on a 10 GB dump is the lesser evil compared to forcing a deep analysis that may exhaust memory. Skipped rules are surfaced in every output format so the result remains honest.

Mode-compatibility check happens **at policy-load time** when the resolved mode is already known (i.e. after CLI resolution but before heap parsing). The check is a pure function over `Policy` and `AnalysisMode` and is unit-testable independent of any heap.

## 13. Output formats

### `text`

Human-readable summary. Sections in order:

1. Header: policy name, heap path, mode requested → mode used, elapsed ms.
2. Skipped rules (if any), grouped reason-first.
3. Violations grouped by severity (Critical → Error → Warning → Info), one line per violation with `id`, message, `actual` vs `expected`, optional remediation hint.
4. Pass summary: `N passed, M violated, K skipped`.
5. Final line: `RESULT: PASS` or `RESULT: FAIL (fail-on=<sev>)`.

Coloring uses the existing `console` styling already in `cli/src/main.rs`; falls back to plain text when stdout is not a TTY.

### `json`

`serde_json::to_string_pretty(&PolicyResult)` against the result type in §9. Schema is stable from M7-2 onward; new fields added in future minor versions are additive.

### `junit`

JUnit XML using `quick-xml`. One `<testsuite>` per policy, one `<testcase>` per rule. Violations emit `<failure>` elements; skipped rules emit `<skipped>` elements. Schema:

```xml
<testsuite name="<meta.name | 'policy'>"
           tests="<rule_count>"
           failures="<violation_count_at_or_above_fail_on>"
           skipped="<skipped_count>"
           time="<elapsed_ms / 1000>">
  <testcase classname="mnemosyne.policy" name="<rule.id>" time="0">
    <!-- on violation: -->
    <failure type="<severity>" message="<one-line summary>">
      <!-- multi-line: actual, expected, remediation hint -->
    </failure>
    <!-- on skip: -->
    <skipped message="<reason>"/>
  </testcase>
</testsuite>
```

The `failures` attribute counts only violations at or above `--fail-on` so the JUnit report agrees with the exit code.

### `github-actions`

Workflow-command lines on stdout, one per violation, using the documented syntax. Heap dumps have no source `file:line`, so file is the **policy** TOML path and line is the rule's TOML source line (captured during load by walking the `toml` value spans).

```text
::error file=policy.toml,line=42::[no-critical-leaks] expected leak_count == 0, got 3 (critical) — investigate <hint>
::warning file=policy.toml,line=51::[string-bytes-cap] expected class_bytes <= 314572800, got 412006432 (warning)
::notice file=policy.toml,line=22::[ci-check] policy 'checkout-service-heap-policy' evaluated 9 rules: 7 passed, 2 violated, 0 skipped
```

Severity → workflow-command mapping: `critical` and `error` → `::error`; `warning` → `::warning`; `info` → `::notice`. A trailing `::notice` summary line is always emitted last.

## 14. CI integration snippets

Two new docs under `docs/integrations/`:

### `docs/integrations/github-actions.md` (new section appended)

```yaml
- name: Run Mnemosyne CI policy
  run: |
    mnemosyne ci-check "$HEAP_FILE" \
      --policy .mnemosyne/policy.toml \
      --format github-actions \
      --fail-on error
```

The annotations show up inline on the PR's "Files changed" view (against the policy file lines). Exit code `1` fails the job; `2`/`3`/`4` also fail and surface the underlying error.

### `docs/integrations/jenkins.md` (new section appended)

```groovy
stage('Heap policy') {
    steps {
        sh '''
            mnemosyne ci-check "$HEAP_FILE" \
              --policy ci/policy.toml \
              --format junit \
              --output build/heap-policy.xml \
              --fail-on error
        '''
    }
    post {
        always {
            junit 'build/heap-policy.xml'
        }
    }
}
```

Both snippets are appended to the existing integration docs without disturbing the M5/M6 examples already present.

## 15. Test plan (TDD-cycle compatible)

All tests are written **before** implementation per `.github/skills/tdd-cycle`.

### Unit tests in `core/src/policy/`

| Test | Asserts |
|---|---|
| `policy_loads_full_example_toml` | The §7 example loads without error and round-trips key fields. |
| `policy_load_rejects_unknown_top_level_key` | Unknown `[foo]` returns load error. |
| `policy_load_rejects_unknown_rule_key` | Unknown per-rule key returns load error with rule id. |
| `policy_load_rejects_duplicate_rule_id` | Two rules with same `id` returns load error. |
| `policy_load_rejects_both_class_and_pattern` | Setting `class` and `class_pattern` on the same rule errors. |
| `policy_load_rejects_invalid_regex` | Bad `class_pattern` errors at load, not evaluation. |
| `policy_load_captures_source_line_per_rule` | Each parsed rule carries the TOML line number of its `[[rule]]` header. |
| `severity_total_ordering` | `Info < Warning < Error < Critical`. |
| `mode_compatibility_explicit_overview_with_deep_rule_errors` | §13 row 3 → `Err(ModeMismatch)`. |
| `mode_compatibility_auto_overview_with_deep_rule_skips` | §13 row 7 → returns skip list, no error. |
| `evaluator_total_bytes_le_pass_and_fail` | Op semantics. |
| `evaluator_total_bytes_eq_zero_only_when_zero` | Equality op. |
| `evaluator_class_instances_overview_below_topn_skips` | Class not in overview top-N → `SkippedRule { ClassBelowTopN }`. |
| `evaluator_class_pattern_matches_multiple_classes_aggregates` | Regex matches several classes; counts/bytes are summed. |
| `evaluator_gc_root_count_per_kind_overview` | One rule per kind asserts the right field is read. |
| `evaluator_provenance_must_not_contain_synthetic_violates` | Synthetic marker present → violation; absent → pass. |
| `evaluator_leak_count_with_severity_filter_critical` | Only critical leaks are counted. |
| `evaluator_retained_size_class_scope_aggregates` | Sums retained sizes across matching leaks. |
| `evaluator_retained_size_leak_suspect_scope_takes_max` | Takes max instead of sum. |
| `evaluator_dominator_root_count` | Counts entries in `graph.dominators` whose `immediate_dominator` is `None`. |
| `exit_code_clean_is_zero` | No violations → 0. |
| `exit_code_violation_at_fail_on_is_one` | Violation at `--fail-on` → 1. |
| `exit_code_violation_below_fail_on_is_zero` | Warning violation with `--fail-on error` → 0. |
| `exit_code_invalid_policy_is_two` | Schema error → 2. |
| `exit_code_unreadable_heap_is_three` | IO error → 3. |
| `exit_code_mode_mismatch_is_four` | Explicit overview + deep rule → 4. |

### Integration tests in `cli/tests/integration.rs`

| Test | Asserts |
|---|---|
| `ci_check_text_format_on_passing_fixture` | Exit 0, "RESULT: PASS" in stdout. |
| `ci_check_text_format_on_failing_fixture` | Exit 1, "RESULT: FAIL" in stdout, violation listed. |
| `ci_check_json_format_round_trips` | `serde_json::from_slice::<PolicyResult>` succeeds on stdout. |
| `ci_check_junit_format_writes_file` | `--output build/result.xml` exists and contains expected `<testcase>` rows. |
| `ci_check_github_actions_format_emits_workflow_commands` | Stdout contains `::error file=`, `::warning file=`, `::notice` lines. |
| `ci_check_invalid_policy_exits_two` | Malformed TOML fixture exits 2. |
| `ci_check_missing_heap_exits_three` | Non-existent heap path exits 3. |
| `ci_check_explicit_overview_with_deep_predicate_exits_four` | Exit 4, error message names offending rule ids. |
| `ci_check_auto_mode_resolves_overview_skips_deep_rules` | Skipped rules surfaced; remaining rules evaluated. |

### Snapshot tests

Use `insta` if already in the workspace; otherwise inline `assert_eq!` against checked-in golden files under `core/tests/snapshots/policy/`.

| Snapshot | Locks |
|---|---|
| `junit-passing.xml` | JUnit envelope on a clean run. |
| `junit-failing-mixed-severity.xml` | JUnit with `<failure>` and `<skipped>`. |
| `github-actions-mixed.txt` | Workflow-command output with all three severity tiers. |
| `text-failing.txt` | Human-readable failure layout. |

### Regression

| Test | Asserts |
|---|---|
| Existing 268-test workspace baseline | All pre-M7-2 tests pass unchanged. |
| `analyze` and `parse` outputs unchanged | `ci-check` is purely additive; existing renderers and JSON envelopes are byte-identical to pre-M7-2 captures. |

## 16. Slice breakdown

Five TDD-friendly slices, each ending with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean.

### Slice M7-2.A — `core::policy` skeleton + Policy/Predicate types + TOML parser

- **Files affected:**
  - `core/src/policy/mod.rs` (new — module root, re-exports)
  - `core/src/policy/schema.rs` (new — `Policy`, `Rule`, `Predicate`, `Op`, `Severity` types)
  - `core/src/policy/load.rs` (new — `load_policy(path) -> CoreResult<Policy>` + line-span capture)
  - `core/src/lib.rs` (re-export `pub mod policy;`)
  - `core/Cargo.toml` (promote `toml` from dev to runtime; add `regex` if not already runtime)
- **Tests:** all `policy_load_*`, `severity_total_ordering`.
- **Out of scope:** evaluation, CLI, renderers.
- **Target size:** ~250 LOC + ~150 LOC tests.

### Slice M7-2.B — Predicate evaluators (overview-compatible) + Violation/PolicyResult types

- **Files affected:**
  - `core/src/policy/result.rs` (new — `PolicyResult`, `Violation`, `Evaluation`, `SkippedRule`, `ActualValue`, `ExpectedConstraint`)
  - `core/src/policy/eval.rs` (new — `evaluate(&Policy, &PolicyInput) -> PolicyResult`)
  - `core/src/policy/input.rs` (new — `enum PolicyInput { Deep(AnalyzeResponse), Overview(OverviewSummary) }` + accessor traits)
- **Tests:** every `evaluator_*` test in §15 covering the overview-compatible predicates (`total_bytes`, `total_instances`, `class_instances`, `class_bytes`, `loaded_class_count`, `gc_root_count`, `provenance_must_not_contain`).
- **Out of scope:** deep-only predicates, CLI, renderers.
- **Target size:** ~350 LOC + ~250 LOC tests.

### Slice M7-2.C — Deep-only predicate evaluators + mode-compatibility enforcement

- **Files affected:**
  - `core/src/policy/eval.rs` (extend with deep-only predicates: `leak_count`, `retained_size`, `dominator_root_count`)
  - `core/src/policy/mode_check.rs` (new — `check_mode_compatibility(&Policy, AnalysisMode) -> Result<Vec<SkippedRule>, ModeMismatch>`)
- **Tests:** `evaluator_leak_count_*`, `evaluator_retained_size_*`, `evaluator_dominator_root_count`, `mode_compatibility_*`.
- **Out of scope:** CLI, renderers.
- **Target size:** ~250 LOC + ~200 LOC tests.

### Slice M7-2.D — CLI `ci-check` subcommand + text/json renderers + exit-code mapping

- **Files affected:**
  - `cli/src/main.rs` (new `CiCheck(CiCheckArgs)` variant, `CiFormat` enum, `SeverityArg` enum, handler that wires policy load → mode resolution → analysis → evaluator → renderer → exit code)
  - `core/src/policy/render/text.rs` (new)
  - `core/src/policy/render/json.rs` (new — `serde_json` based)
  - `core/src/policy/render/mod.rs` (new — re-exports)
  - `cli/tests/integration.rs` (add `ci_check_text_*`, `ci_check_json_*`, `ci_check_invalid_policy_exits_two`, `ci_check_missing_heap_exits_three`, `ci_check_explicit_overview_with_deep_predicate_exits_four`, `ci_check_auto_mode_resolves_overview_skips_deep_rules`)
- **Tests:** all CLI integration tests except JUnit/GitHub-Actions snapshots; all exit-code unit tests.
- **Out of scope:** JUnit and GitHub-Actions renderers; integration docs.
- **Target size:** ~300 LOC + ~250 LOC tests.

### Slice M7-2.E — JUnit + GitHub-Actions renderers + CI integration docs + snapshot tests

- **Files affected:**
  - `core/src/policy/render/junit.rs` (new — `quick-xml` based)
  - `core/src/policy/render/github_actions.rs` (new — workflow-command emission)
  - `core/Cargo.toml` (add `quick-xml` as runtime dep)
  - `core/tests/snapshots/policy/*` (new golden files per §15)
  - `cli/tests/integration.rs` (add `ci_check_junit_*`, `ci_check_github_actions_*`)
  - `docs/integrations/github-actions.md` (append M7-2 section per §14)
  - `docs/integrations/jenkins.md` (append M7-2 section per §14)
- **Tests:** snapshot tests; remaining CLI integration tests for the two formats.
- **Out of scope:** none — this slice closes M7-2.
- **Target size:** ~300 LOC + ~200 LOC tests + ~80 lines of doc.

After Slice E, M7-2 is complete. Documentation Sync should run an impact-driven pass against `STATUS.md`, `CHANGELOG.md`, `docs/roadmap.md` (mark M7-2 complete; move next-action to M7-3), `README.md` (CLI surface table), and `docs/user-guide.md` (new `ci-check` section).

## 17. Risks and open questions

| # | Risk / question | Mitigation / current answer |
|---|---|---|
| R1 | Approximate shallow bytes from overview drift from MAT/deep shallow on `class_bytes` rules → false positives or false negatives | Renderer always labels overview-sourced numbers as approximate; M7-5 publishes the divergence number; user-guide warns CI policy authors to tune `class_bytes` thresholds against an overview-mode baseline run, not a deep-mode one. |
| R2 | TOML schema evolution breaks pinned policies in user repos | Enforce strict unknown-key rejection from the start (no quiet acceptance); reserve `[meta].version` for explicit migrations; document additive-only minor versions. |
| R3 | `class_pattern` regex DoS on adversarial policies | The `regex` crate is linear-time by construction; loader still rejects patterns that fail to compile. Policies are local files trusted by the CI owner, so this is a low risk. |
| R4 | JUnit renderer schema disagreements between Jenkins, GitHub Actions test reporter, and others | Target the lowest-common-denominator subset (no `<system-out>`, no nested suites); snapshot test locks the schema. |
| Q1 | Should `ci-check` accept a directory of `.toml` policies and merge them? | **Defer.** Single-file v1 covers the use case; merging is straightforward to add later without breaking the schema. |
| Q2 | Should violations carry a JSON path into the analysis result for tooling? | **Defer.** The predicate name + `actual` value is enough for v1; add JSON-path attribution if a real consumer asks. |
| Q3 | Counting semantics for `total_instances` in overview mode when primitive arrays are present | **Open.** Overview currently exposes `total_instances`, `total_object_arrays`, and `total_primitive_array_bytes` (no per-array count). Slice B must either (a) extend `OverviewSummary` with `total_primitive_arrays: u64` (preferred, ~5 LOC change to overview parser) or (b) document `total_instances` for overview mode as instances + object arrays only. **Recommended decision:** option (a), tracked as a sub-task of Slice M7-2.B. |
| Q4 | How does `ci-check` interact with `--profile ci-regression` documented in `docs/integrations/github-actions.md` for the `analyze` subcommand? | The existing `--profile ci-regression` pre-dates `ci-check` and only tunes `analyze`'s output. They are complementary: `analyze --profile ci-regression` produces the artifact; `ci-check` enforces policy. The user-guide section added in Slice E must call this out explicitly so users do not assume one supersedes the other. |
| Q5 | Should `ci-check` emit AI insights when `--mode deep` is used and `LLM_API_KEY` is set? | **No** for v1. AI insights are a separate workflow (`analyze --ai`); `ci-check` stays deterministic and offline-friendly so it can run on air-gapped runners. |

## 18. Cross-references

- Parent design: [milestone-7-production-readiness.md](milestone-7-production-readiness.md) §7 (framing) and §6 (M7-1 overview-mode design that this addendum builds on).
- Roadmap: [docs/roadmap.md §4](../roadmap.md) (M7-2 row).
- Existing analysis types consumed by the evaluator:
  - [core/src/analysis/engine.rs](../../core/src/analysis/engine.rs) — `AnalyzeResponse`, `LeakInsight`, `LeakSeverity`.
  - [core/src/analysis/mode.rs](../../core/src/analysis/mode.rs) — `AnalysisMode`.
  - `core/src/hprof/overview.rs` — `OverviewSummary`, `GcRootCounts`, `OverviewClassStat`.
  - `core/src/graph/metrics.rs` — `GraphMetrics { node_count, edge_count, dominators: Vec<DominatorNode> }`; `dominator_root_count` is derived in the evaluator from `dominators.iter().filter(|d| d.immediate_dominator.is_none()).count()`.
- CLI integration target: [cli/src/main.rs](../../cli/src/main.rs).
- CI integration docs to extend: [docs/integrations/github-actions.md](../integrations/github-actions.md), [docs/integrations/jenkins.md](../integrations/jenkins.md).

## 19. Implementation readiness verdict

**READY** — this addendum is implementation-depth. The Implementation Agent may proceed with **Slice M7-2.A (`core::policy` skeleton + Policy/Predicate types + TOML parser)** as the first task. Slices B → E are gated behind their predecessors and must each end with `cargo {check, test, clippy --workspace --all-targets -- -D warnings, fmt --all -- --check}` clean before handing off to the next slice.
