//! `CompareSnapshots` workflow (M11 Slice 11.C): `resolve_snapshots` →
//! `diff` → `complete`.
//!
//! Every step below is a thin wrapper around an existing, already-tested
//! primitive -- no new heap-analysis logic is introduced here (design doc
//! §3.3):
//!
//! - `resolve_snapshots` composes [`crate::snapshot::SnapshotStore`]'s three
//!   Slice 9.A/9.B methods exactly as shipped: [`SnapshotStore::load`] when
//!   the caller already has a snapshot key, or
//!   [`SnapshotStore::find_fresh_for_heap`] (reuse if fresh) falling back to
//!   [`SnapshotStore::save`] (parse + cache) when the caller only has a raw
//!   heap path. Per design doc §4 point 4 / §4's explicit non-blocking note,
//!   this calls `core::snapshot` directly -- it does **not** go through the
//!   CLI's `--snapshot` flag or an MCP `open_snapshot` tool, so it has no
//!   dependency on M9 Slices 9.C/9.D landing first.
//! - `diff` calls [`crate::diff::run_diff`] with `DiffMode::Object`, using
//!   the same default parameter construction the `diff_heaps` MCP tool and
//!   the CLI's `diff` command already use for object-mode diffing (see
//!   `core::mcp::server`'s `diff_heaps` handler / `cli/src/main.rs`'s
//!   `handle_diff` -- both build an equivalent `DiffRequest`; this step
//!   reuses that same shape rather than guessing at defaults).
//!
//! ## Judgment call: dual heap identity vs. `WorkflowState::heap_path`
//!
//! [`WorkflowState`] has a single `heap_path: String` field (and
//! [`crate::workflow::start`]'s signature takes a single `heap_path: String`
//! parameter), but `CompareSnapshots` has *two* heap identities (before/
//! after), each of which may be given as a raw heap path or an existing
//! snapshot key. Rather than changing `start`'s call convention -- which
//! would touch the three already-shipped kinds' call sites too -- this
//! module keeps `start`'s signature untouched and treats the `heap_path`
//! argument callers pass to `start` as a caller-supplied placeholder only:
//! the real per-side identifiers travel through `start`'s existing
//! `initial_params: Value` argument (`before_heap_path`/`after_heap_path`/
//! `before_snapshot_key`/`after_snapshot_key`, matching design doc §7's
//! `start_workflow` param list exactly). Once `resolve_snapshots` has
//! resolved both sides, it overwrites `state.heap_path` with a synthetic
//! `"<before heap path> -> <after heap path>"` description for
//! human-readable display/logging, and stores the authoritative per-side
//! data (`snapshot_key`, `heap_path`, `snapshotted_now`) under
//! `state.context.before` / `state.context.after` -- this is the "synthetic
//! combined description in `heap_path`, real data in `context`" option the
//! design doc's task notes called out as the preferred, least invasive
//! choice. A future Slice 11.D `start_workflow` MCP handler can pass any
//! placeholder (e.g. an empty string) as `heap_path` for this kind, since it
//! is fully overwritten by the first step.
//!
//! ## Documented cost note: `diff` re-parses rather than reusing a
//! ## snapshot's in-memory graph
//!
//! [`crate::diff::run_diff`] takes heap file *paths* and always re-parses
//! them internally (see `core::diff::load_object_diff_graph`) -- it has no
//! parameter for handing it an already-loaded `(ObjectGraph,
//! DominatorTree)` pair. This means that even when `resolve_snapshots`
//! resolves a side via an existing snapshot key (or reuses a
//! `find_fresh_for_heap` hit), the `diff` step still re-parses that side's
//! heap file from disk -- the in-memory graph a cached `SnapshotPayload`
//! carries is not reused by the diff itself. This is a real, deliberate cost
//! (not a bug): composing `run_diff` unmodified, per §3.3's "zero new
//! analysis logic" principle, was preferred over teaching `run_diff` (or a
//! parallel code path) to accept pre-loaded graphs, which would be new
//! surface area for a single workflow step to justify. Saving a snapshot via
//! `resolve_snapshots` still has value beyond this one workflow run: it
//! populates (or refreshes) the shared on-disk snapshot cache for *future*
//! calls (this workflow's later runs, `--snapshot` CLI usage once Slice 9.C
//! lands in a given checkout, etc.), even though this run's own `diff` step
//! does not read it back.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    diff::{run_diff, DiffMode, DiffRequest, DiffResult, IdentityStrategy},
    errors::{CoreError, CoreResult},
    graph::build_dominator_tree,
    hprof::{parse_hprof_file_with_options, ParseOptions},
    snapshot::SnapshotStore,
    workflow::{
        workflow_step_input_mismatch, ParamDescription, StepDescription, StepRecord,
        WorkflowDescription, WorkflowKind, WorkflowState,
    },
};

pub(crate) const STEP_RESOLVE_SNAPSHOTS: &str = "resolve_snapshots";
pub(crate) const STEP_DIFF: &str = "diff";
pub(crate) const STEP_COMPLETE: &str = "complete";

/// The fixed step order this workflow kind runs in. Linear, like
/// `TriageMemoryLeak`/`TuneGc` -- no branch point (that's
/// `TraverseObjectGraph`'s distinction).
const STEP_ORDER: [&str; 2] = [STEP_RESOLVE_SNAPSHOTS, STEP_DIFF];

fn next_step_name(current: &str) -> &'static str {
    match STEP_ORDER.iter().position(|&name| name == current) {
        Some(idx) if idx + 1 < STEP_ORDER.len() => STEP_ORDER[idx + 1],
        _ => STEP_COMPLETE,
    }
}

/// Static step-sequence description for this workflow kind. No side
/// effects, no `WorkflowState` created.
pub fn describe() -> WorkflowDescription {
    WorkflowDescription {
        kind: WorkflowKind::CompareSnapshots,
        steps: vec![
            StepDescription {
                name: STEP_RESOLVE_SNAPSHOTS.into(),
                description: "Resolve the before/after heap identities into two snapshots. A heap path is resolved via an existing fresh cached snapshot if one exists, otherwise the heap is parsed and a new snapshot is saved. A snapshot key is loaded as-is.".into(),
                expected_input: vec![
                    ParamDescription::optional(
                        "before_heap_path",
                        "string",
                        "Path to the 'before' heap dump. Mutually exclusive with before_snapshot_key.",
                    ),
                    ParamDescription::optional(
                        "after_heap_path",
                        "string",
                        "Path to the 'after' heap dump. Mutually exclusive with after_snapshot_key.",
                    ),
                    ParamDescription::optional(
                        "before_snapshot_key",
                        "string",
                        "An existing snapshot's key for the 'before' heap. Mutually exclusive with before_heap_path.",
                    ),
                    ParamDescription::optional(
                        "after_snapshot_key",
                        "string",
                        "An existing snapshot's key for the 'after' heap. Mutually exclusive with after_heap_path.",
                    ),
                    ParamDescription::optional(
                        "snapshot_dir",
                        "string",
                        "Override the snapshot cache directory (defaults to the same cache root the CLI's --snapshot flag uses). Primarily useful for tests.",
                    ),
                ],
                underlying_primitives: vec![
                    "SnapshotStore::find_fresh_for_heap".into(),
                    "SnapshotStore::save".into(),
                    "SnapshotStore::load".into(),
                ],
            },
            StepDescription {
                name: STEP_DIFF.into(),
                description: "Run an object-level diff (DiffMode::Object) between the two resolved heap files and surface the ranked suspects.".into(),
                expected_input: vec![],
                underlying_primitives: vec!["run_diff".into()],
            },
        ],
    }
}

/// Execute whatever step `state.current_step` currently names, using `input`
/// as that step's parameters. Mirrors
/// [`crate::workflow::triage_memory_leak::run_step`]'s shape exactly -- same
/// append-to-`step_history`-then-advance-`current_step` contract.
pub(crate) async fn run_step(state: &mut WorkflowState, input: Value) -> CoreResult<Value> {
    let step_name = state.current_step.clone();
    let output = match step_name.as_str() {
        STEP_RESOLVE_SNAPSHOTS => run_resolve_snapshots(state, &input)?,
        STEP_DIFF => run_diff_step(state, &input).await?,
        other => {
            return Err(workflow_step_input_mismatch(format!(
                "unknown compare_snapshots step '{other}'"
            )))
        }
    };

    state.step_history.push(StepRecord {
        step_name: step_name.clone(),
        input,
        output_summary: output.clone(),
        timestamp: crate::mcp::session::timestamp_now(),
    });
    state.current_step = next_step_name(&step_name).to_string();

    Ok(output)
}

/// Per-side result of `resolve_snapshots`: the resolved snapshot key, the
/// heap file path `diff` should re-parse, and whether this run created a new
/// snapshot (`false` when an existing snapshot -- key-supplied or a fresh
/// `find_fresh_for_heap` hit -- was reused instead).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResolvedSide {
    snapshot_key: String,
    heap_path: String,
    snapshotted_now: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ResolveSnapshotsInput {
    #[serde(default)]
    before_heap_path: Option<String>,
    #[serde(default)]
    after_heap_path: Option<String>,
    #[serde(default)]
    before_snapshot_key: Option<String>,
    #[serde(default)]
    after_snapshot_key: Option<String>,
    #[serde(default)]
    snapshot_dir: Option<String>,
}

fn run_resolve_snapshots(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<ResolveSnapshotsInput>(
        input,
        "resolve_snapshots step expects an object with before_heap_path/after_heap_path or before_snapshot_key/after_snapshot_key fields",
    )?;

    let root = parsed
        .snapshot_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(default_snapshot_store_root);
    let store = SnapshotStore::new(root);

    let before = resolve_side(
        &store,
        parsed.before_heap_path,
        parsed.before_snapshot_key,
        "before",
    )?;
    let after = resolve_side(
        &store,
        parsed.after_heap_path,
        parsed.after_snapshot_key,
        "after",
    )?;

    // See module doc comment's "dual heap identity" section: overwrite the
    // placeholder `heap_path` `start` was called with, now that both sides
    // are known.
    state.heap_path = format!("{} -> {}", before.heap_path, after.heap_path);
    state.context = json!({ "before": before, "after": after });

    Ok(json!({ "before": before, "after": after }))
}

/// Resolve one side ("before" or "after") of the comparison: exactly one of
/// `heap_path`/`snapshot_key` must be given.
fn resolve_side(
    store: &SnapshotStore,
    heap_path: Option<String>,
    snapshot_key: Option<String>,
    side: &str,
) -> CoreResult<ResolvedSide> {
    match (heap_path, snapshot_key) {
        (Some(_), Some(_)) => Err(workflow_step_input_mismatch(format!(
            "resolve_snapshots step: provide only one of {side}_heap_path or {side}_snapshot_key, not both"
        ))),
        (None, None) => Err(workflow_step_input_mismatch(format!(
            "resolve_snapshots step requires {side}_heap_path or {side}_snapshot_key"
        ))),
        (None, Some(key)) => {
            let payload = store.load(&key)?;
            Ok(ResolvedSide {
                snapshot_key: key,
                heap_path: payload.manifest.heap_path,
                snapshotted_now: false,
            })
        }
        (Some(path), None) => {
            if let Some(payload) = store.find_fresh_for_heap(&path)? {
                return Ok(ResolvedSide {
                    snapshot_key: payload.manifest.heap_sha256,
                    heap_path: path,
                    snapshotted_now: false,
                });
            }

            // No fresh cached snapshot for this heap path: parse it now and
            // save a new one via `SnapshotStore::save` (design doc §4 point
            // 4 / §8 Slice 11.C). See this module's doc comment for why the
            // `diff` step below still re-parses `path` itself rather than
            // reusing `graph`/`dominator` here -- this parse's value is
            // populating the shared snapshot cache for future runs, not
            // avoiding a re-parse later in *this* run.
            let graph = parse_hprof_file_with_options(&path, ParseOptions::default())?;
            let dominator = build_dominator_tree(&graph);
            let manifest = store.save(&path, &graph, &dominator)?;
            Ok(ResolvedSide {
                snapshot_key: manifest.heap_sha256,
                heap_path: path,
                snapshotted_now: true,
            })
        }
    }
}

async fn run_diff_step(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    // This step takes no parameters of its own -- both heap identities were
    // already fixed by `resolve_snapshots`. Allow null/`{}`, reject anything
    // with actual content (same convention as
    // `triage_memory_leak::run_explain`).
    let is_empty_object = input.as_object().is_some_and(|obj| obj.is_empty());
    if !input.is_null() && !is_empty_object {
        return Err(workflow_step_input_mismatch(
            "diff step takes no input; pass null or {}",
        ));
    }

    let before = resolved_side(state, "before")?;
    let after = resolved_side(state, "after")?;

    let request = default_object_diff_request(before.heap_path, after.heap_path);
    let diff = match run_diff(request).await? {
        DiffResult::Object(diff) => diff,
        // `mode` above is always `DiffMode::Object`; `run_diff` only returns
        // `DiffResult::Class` for `DiffMode::Class` requests.
        DiffResult::Class(_) => unreachable!("compare_snapshots always requests DiffMode::Object"),
    };

    let value = serde_json::to_value(&diff)?;
    state.context["diff"] = value.clone();
    Ok(value)
}

fn resolved_side(state: &WorkflowState, side: &str) -> CoreResult<ResolvedSide> {
    let value = state.context.get(side).ok_or_else(|| {
        workflow_step_input_mismatch(format!(
            "diff step requires resolve_snapshots to have run first (missing context.{side})"
        ))
    })?;
    serde_json::from_value(value.clone()).map_err(CoreError::from)
}

/// Builds the same `DiffRequest` shape the `diff_heaps` MCP tool / CLI
/// `diff` command already use for object-mode diffing with default
/// parameters (see `core::mcp::server`'s `diff_heaps` handler and
/// `cli/src/main.rs`'s `handle_diff`) -- reused here rather than guessed at,
/// per the design doc's explicit instruction.
fn default_object_diff_request(before_path: String, after_path: String) -> DiffRequest {
    DiffRequest {
        before_path,
        after_path,
        mode: DiffMode::Object,
        identity_strategy: IdentityStrategy::default(),
        retained_bucket_bits: 10,
        min_retained_bytes: crate::diff::object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
        retained_change_threshold: crate::diff::object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
        top_n: crate::diff::object::types::DEFAULT_OBJECT_DIFF_TOP_N,
        retain_field_data: false,
        cross_reference_leaks: false,
    }
}

/// `MNEMOSYNE_SNAPSHOT_DIR` overrides the default snapshot cache root. This
/// mirrors `cli/src/main.rs`'s `default_snapshot_dir()` (Slice 9.C) exactly
/// -- same env var, same `dirs::cache_dir()`-then-temp-dir fallback shape --
/// so a `compare_snapshots` run sees (and populates) the same on-disk cache
/// the CLI's `--snapshot` flag does. It is duplicated here rather than
/// shared because `core` cannot depend on `cli` (the dependency points the
/// other way), and per this milestone's explicit design constraint (§4):
/// `CompareSnapshots` must not depend on the CLI/MCP wiring layers (Slices
/// 9.C/9.D) to function.
const SNAPSHOT_DIR_ENV: &str = "MNEMOSYNE_SNAPSHOT_DIR";

fn default_snapshot_store_root() -> PathBuf {
    if let Ok(dir) = std::env::var(SNAPSHOT_DIR_ENV) {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(mut dir) = dirs::cache_dir() {
        dir.push("mnemosyne");
        return dir;
    }

    let mut fallback = std::env::temp_dir();
    fallback.push("mnemosyne");
    fallback.push("snapshots");
    fallback
}

fn parse_step_input<T>(input: &Value, mismatch_message: &str) -> CoreResult<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if input.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(input.clone())
        .map_err(|err| workflow_step_input_mismatch(format!("{mismatch_message}: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_step_names_match_step_order() {
        let description = describe();
        let names: Vec<String> = description.steps.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, STEP_ORDER.to_vec());
    }

    #[test]
    fn next_step_name_walks_full_order_then_complete() {
        assert_eq!(next_step_name(STEP_RESOLVE_SNAPSHOTS), STEP_DIFF);
        assert_eq!(next_step_name(STEP_DIFF), STEP_COMPLETE);
        assert_eq!(next_step_name(STEP_COMPLETE), STEP_COMPLETE);
        assert_eq!(next_step_name("bogus"), STEP_COMPLETE);
    }

    #[tokio::test]
    async fn default_snapshot_store_root_honors_env_override() {
        // This crate's unified `mnemosyne_core` lib test binary also runs
        // `core::mcp::server`'s snapshot-backed tests, which mutate this
        // same `MNEMOSYNE_SNAPSHOT_DIR` env var -- a plain, unlocked mutation
        // here raced against those (reproduced intermittently). Holding the
        // crate-shared `crate::snapshot::test_snapshot_env_lock()` for the
        // duration serializes against every other test touching this var,
        // not just tests in this file.
        let _env_lock = crate::snapshot::test_snapshot_env_lock().lock().await;
        let previous = std::env::var(SNAPSHOT_DIR_ENV).ok();
        std::env::set_var(SNAPSHOT_DIR_ENV, "C:/tmp/mnemosyne-test-snapshots");
        let root = default_snapshot_store_root();
        match previous {
            Some(value) => std::env::set_var(SNAPSHOT_DIR_ENV, value),
            None => std::env::remove_var(SNAPSHOT_DIR_ENV),
        }
        assert_eq!(root, PathBuf::from("C:/tmp/mnemosyne-test-snapshots"));
    }
}
