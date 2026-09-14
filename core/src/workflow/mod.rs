//! Stateful, multi-step MCP workflows (M11).
//!
//! A *workflow* is a small, named state machine that orchestrates a fixed
//! sequence of steps over primitives this crate already ships and already
//! tests -- `detect_leaks`, `find_all_gc_paths`, `analyze_by_referrer`,
//! `generate_ai_insights_async`, `propose_fix_with_config`, etc. No new
//! heap-analysis logic is introduced anywhere in this module; see
//! `docs/design/milestone-11-mcp-workflow-suite.md` §3.3.
//!
//! Structural template: [`crate::mcp::session::McpSessionStore`] /
//! [`crate::snapshot::SnapshotStore`]. [`WorkflowStore`] mirrors their shape
//! exactly -- same atomic-write helper
//! ([`crate::mcp::session::replace_session_file`]), same
//! `schema_version: u32` convention, same `timestamp_now()` convention --
//! rather than reinventing the on-disk persistence shape a third time. See
//! the design doc §3.1/§6.
//!
//! Slice 11.A shipped the scaffolding (`WorkflowState` / `WorkflowStore` /
//! `StepRecord` / `WorkflowDescription`) plus the first fully-working
//! workflow kind end-to-end: [`WorkflowKind::TriageMemoryLeak`] (see
//! [`triage_memory_leak`]). Slice 11.B added two more: [`WorkflowKind::TuneGc`]
//! (see [`tune_gc`]) and [`WorkflowKind::TraverseObjectGraph`] (see
//! [`traverse_object_graph`]). Slice 11.C (this pass) adds the fourth and
//! final kind, [`WorkflowKind::CompareSnapshots`] (see
//! [`compare_snapshots`]) -- all four `WorkflowKind` variants are now fully
//! implemented in [`describe`]/[`start`]/[`advance`].
//!
//! MCP tool registration (`describe_workflow`/`start_workflow`/`next_step`/
//! `get_workflow`/`close_workflow` in `core::mcp::server`) is Slice 11.D --
//! out of scope here. [`start`] and [`advance`] are written to be directly
//! callable from a future MCP handler (see their doc comments).

pub mod classloader_leak;
pub mod compare_snapshots;
pub mod traverse_object_graph;
pub mod triage_memory_leak;
pub mod tune_gc;

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    errors::{CoreError, CoreResult},
    mcp::session::{replace_session_file, timestamp_now},
};

/// Workflow payload schema version. Bump whenever [`WorkflowState`]'s
/// serialized shape changes in a wire-incompatible way, mirroring
/// `MCP_SESSION_VERSION` / `SNAPSHOT_SCHEMA_VERSION`.
pub const WORKFLOW_SCHEMA_VERSION: u32 = 1;

/// The literal `current_step`/`StepRecord::step_name` value every workflow
/// kind uses to mean "done" -- each kind module also defines its own local
/// `STEP_COMPLETE` constant with this same value (see
/// `triage_memory_leak::STEP_COMPLETE`, `tune_gc::STEP_COMPLETE`,
/// `traverse_object_graph::STEP_COMPLETE`) for use within that module's own
/// step-sequence logic; this one exists so the kind-agnostic checks in
/// [`advance`] don't need to reach into one specific kind's module for a
/// value that is, by convention, identical across all of them.
const STEP_COMPLETE: &str = "complete";

/// The four named workflow kinds from the design doc §2/§4. Only
/// [`WorkflowKind::TriageMemoryLeak`] is implemented in Slice 11.A; the
/// other three exist as enum variants (and thus as valid wire values) ahead
/// of their own slices so `WorkflowState::kind`'s on-disk representation and
/// `describe_workflow`'s eventual `kind` dispatch never need a
/// wire-breaking enum change later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowKind {
    TriageMemoryLeak,
    TuneGc,
    TraverseObjectGraph,
    CompareSnapshots,
    ClassloaderLeak,
}

impl WorkflowKind {
    /// Stable identifier used in error messages and (in a later slice) the
    /// MCP wire format's `kind` string, mirroring `LeakKind`/`LeakSeverity`'s
    /// own `snake_case` wire convention elsewhere in this crate.
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowKind::TriageMemoryLeak => "triage_memory_leak",
            WorkflowKind::TuneGc => "tune_gc",
            WorkflowKind::TraverseObjectGraph => "traverse_object_graph",
            WorkflowKind::CompareSnapshots => "compare_snapshots",
            WorkflowKind::ClassloaderLeak => "classloader_leak",
        }
    }
}

impl std::fmt::Display for WorkflowKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One entry in [`WorkflowState::step_history`]: a record of a single step
/// execution, in the order it happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_name: String,
    pub input: Value,
    pub output_summary: Value,
    pub timestamp: String,
}

/// Persisted state for one in-flight (or completed) workflow instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub schema_version: u32,
    pub workflow_id: String,
    pub kind: WorkflowKind,
    pub created_at: String,
    pub updated_at: String,
    pub heap_path: String,
    /// Name of the step that will run on the *next* call to [`advance`], or
    /// the literal string `"complete"` once the workflow has finished
    /// running every step in its kind's sequence. This is the same value
    /// [`triage_memory_leak::describe`]'s `WorkflowDescription.steps` names
    /// are drawn from -- see [`triage_memory_leak`]'s module docs for the
    /// exact contract this field upholds (§6.1 of the design doc).
    pub current_step: String,
    pub step_history: Vec<StepRecord>,
    /// Kind-specific accumulated context (e.g. the chosen leak id for
    /// `TriageMemoryLeak`). Deliberately a loose `serde_json::Value` at this
    /// persistence boundary -- each kind's own module (e.g.
    /// [`triage_memory_leak`]) owns strongly-typed helpers for reading and
    /// writing its own context shape, so the looseness never leaks past
    /// this module. See design doc §9 R4.
    pub context: Value,
}

/// Static description of one named step in a workflow kind's sequence --
/// what [`triage_memory_leak::describe`] (and, in a later slice,
/// `describe_workflow`) returns per step. Mirrors the shape of `list_tools`'
/// existing `"params"` entries in `core::mcp::server`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDescription {
    pub name: String,
    pub r#type: String,
    pub required: bool,
    pub description: String,
}

impl ParamDescription {
    pub fn required(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            r#type: param_type.into(),
            required: true,
            description: description.into(),
        }
    }

    pub fn optional(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            r#type: param_type.into(),
            required: false,
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDescription {
    pub name: String,
    pub description: String,
    pub expected_input: Vec<ParamDescription>,
    /// The existing, already-tested function name(s) this step calls (e.g.
    /// `["detect_leaks"]`) -- named for transparency, so an agent (or a
    /// human debugging drift) can see exactly what a step composes.
    pub underlying_primitives: Vec<String>,
}

/// Static description of a workflow kind's step sequence. No
/// [`WorkflowState`] is created by producing this; it is pure metadata, safe
/// to call before committing to [`start`]ing a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDescription {
    pub kind: WorkflowKind,
    pub steps: Vec<StepDescription>,
}

/// On-disk store for [`WorkflowState`], keyed by `workflow_id`. Mirrors
/// [`crate::snapshot::SnapshotStore`] / `McpSessionStore`'s shape exactly:
/// same atomic-write helper, same directory-per-store convention. Config-
/// driven root resolution (a `[workflow].directory` override, mirroring
/// `[ai.sessions].directory`) is MCP wiring and lands in Slice 11.D; this
/// slice's `WorkflowStore::new` takes an explicit root, exactly like
/// `SnapshotStore::new`/`McpSessionStore::new` do today.
#[derive(Debug, Clone)]
pub struct WorkflowStore {
    root: PathBuf,
}

impl WorkflowStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_root(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn save(&self, state: &WorkflowState) -> CoreResult<()> {
        self.ensure_root()?;
        let target = self.path_for(&state.workflow_id)?;
        let temp = self.root.join(format!("{}.tmp", state.workflow_id));
        let payload = serde_json::to_vec_pretty(state)?;
        fs::write(&temp, payload)?;
        replace_session_file(&temp, &target)?;
        Ok(())
    }

    pub fn load(&self, workflow_id: &str) -> CoreResult<WorkflowState> {
        let path = self.path_for(workflow_id)?;
        let bytes = fs::read(&path).map_err(|err| map_load_error(workflow_id, err))?;
        let state: WorkflowState = serde_json::from_slice(&bytes)
            .map_err(|err| workflow_corrupt(workflow_id, &err.to_string()))?;
        if state.workflow_id != workflow_id {
            return Err(workflow_corrupt(
                workflow_id,
                &format!(
                    "embedded workflow_id '{}' does not match requested workflow_id '{workflow_id}'",
                    state.workflow_id
                ),
            ));
        }
        if state.schema_version != WORKFLOW_SCHEMA_VERSION {
            return Err(CoreError::Unsupported(format!(
                "workflow_schema_unsupported: schema_version {} is unsupported",
                state.schema_version
            )));
        }
        Ok(state)
    }

    pub fn remove(&self, workflow_id: &str) -> CoreResult<()> {
        let path = self.path_for(workflow_id)?;
        fs::remove_file(&path).map_err(|err| map_load_error(workflow_id, err))?;
        Ok(())
    }

    fn path_for(&self, workflow_id: &str) -> CoreResult<PathBuf> {
        validate_workflow_id(workflow_id)?;
        Ok(self.root.join(format!("{workflow_id}.json")))
    }
}

fn validate_workflow_id(workflow_id: &str) -> CoreResult<()> {
    if workflow_id.is_empty()
        || !workflow_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(CoreError::InvalidInput(format!(
            "invalid workflow_id: {workflow_id}"
        )));
    }
    Ok(())
}

fn map_load_error(workflow_id: &str, err: std::io::Error) -> CoreError {
    if err.kind() == std::io::ErrorKind::NotFound {
        return workflow_not_found(workflow_id);
    }
    CoreError::Io(err)
}

/// Structured, greppable error identifiers, following the same
/// `CoreError::Unsupported("<code>: <detail>")` convention `core::snapshot`
/// and `core::graph::gc_path` already use.
fn workflow_not_found(workflow_id: &str) -> CoreError {
    CoreError::Unsupported(format!(
        "workflow_not_found: no workflow found for id '{workflow_id}'"
    ))
}

fn workflow_corrupt(workflow_id: &str, detail: &str) -> CoreError {
    CoreError::Unsupported(format!(
        "workflow_corrupt: failed to load workflow '{workflow_id}': {detail}"
    ))
}

pub(crate) fn workflow_step_input_mismatch(detail: impl Into<String>) -> CoreError {
    CoreError::Unsupported(format!("workflow_step_input_mismatch: {}", detail.into()))
}

fn workflow_already_complete(workflow_id: &str) -> CoreError {
    CoreError::Unsupported(format!(
        "workflow_already_complete: workflow '{workflow_id}' has already reached the 'complete' step"
    ))
}

// `workflow_kind_not_implemented` (Slices 11.A/11.B's placeholder error for
// `WorkflowKind::CompareSnapshots`) is removed as of Slice 11.C: all four
// `WorkflowKind` variants are now implemented, so `describe`/`start`/
// `advance`'s `match` arms are exhaustive without a fallback arm, and the
// compiler -- not a runtime error path -- is what now guarantees every kind
// is handled.

fn new_workflow_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("wf-{nanos}")
}

/// Static, side-effect-free step-sequence lookup for `kind`. No
/// [`WorkflowState`] is created. All four [`WorkflowKind`] variants are
/// implemented as of Slice 11.C.
pub fn describe(kind: WorkflowKind) -> CoreResult<WorkflowDescription> {
    match kind {
        WorkflowKind::TriageMemoryLeak => Ok(triage_memory_leak::describe()),
        WorkflowKind::TuneGc => Ok(tune_gc::describe()),
        WorkflowKind::TraverseObjectGraph => Ok(traverse_object_graph::describe()),
        WorkflowKind::CompareSnapshots => Ok(compare_snapshots::describe()),
        WorkflowKind::ClassloaderLeak => Ok(classloader_leak::describe()),
    }
}

/// Create a new workflow instance of `kind`, run its first step using
/// `initial_params`, persist the resulting state via `store`, and return it.
///
/// This is the function a future MCP `start_workflow` tool handler (Slice
/// 11.D) calls directly: `kind`/`heap_path`/`initial_params` map 1:1 onto
/// that tool's wire parameters (see design doc §7), and `store` is
/// constructed by the handler the same way `session_store(config)` already
/// builds an `McpSessionStore` for the existing session-lifecycle tools.
///
/// All four [`WorkflowKind`] variants are implemented as of Slice 11.C.
pub async fn start(
    store: &WorkflowStore,
    kind: WorkflowKind,
    heap_path: String,
    initial_params: Value,
) -> CoreResult<WorkflowState> {
    fn new_state(kind: WorkflowKind, heap_path: String, first_step: &str) -> WorkflowState {
        let now = timestamp_now();
        WorkflowState {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            workflow_id: new_workflow_id(),
            kind,
            created_at: now.clone(),
            updated_at: now,
            heap_path,
            current_step: first_step.to_string(),
            step_history: Vec::new(),
            context: json!({}),
        }
    }

    let mut state = match kind {
        WorkflowKind::TriageMemoryLeak => {
            let mut state = new_state(kind, heap_path, triage_memory_leak::STEP_DETECT);
            triage_memory_leak::run_step(&mut state, initial_params).await?;
            state
        }
        WorkflowKind::TuneGc => {
            let mut state = new_state(kind, heap_path, tune_gc::STEP_ROOT_KIND_BREAKDOWN);
            tune_gc::run_step(&mut state, initial_params).await?;
            state
        }
        WorkflowKind::TraverseObjectGraph => {
            let mut state = new_state(kind, heap_path, traverse_object_graph::STEP_INSPECT);
            traverse_object_graph::run_step(&mut state, initial_params).await?;
            state
        }
        WorkflowKind::CompareSnapshots => {
            // `heap_path` is a caller-supplied placeholder for this kind --
            // see `compare_snapshots`'s module doc comment ("dual heap
            // identity" section). It is overwritten by the first step once
            // both sides are resolved.
            let mut state = new_state(kind, heap_path, compare_snapshots::STEP_RESOLVE_SNAPSHOTS);
            compare_snapshots::run_step(&mut state, initial_params).await?;
            state
        }
        WorkflowKind::ClassloaderLeak => {
            let mut state = new_state(kind, heap_path, classloader_leak::STEP_DETECT);
            classloader_leak::run_step(&mut state, initial_params).await?;
            state
        }
    };

    state.updated_at = timestamp_now();
    store.save(&state)?;
    Ok(state)
}

/// Load the workflow identified by `workflow_id`, run whatever step
/// `WorkflowState::current_step` currently names using `step_input`,
/// persist the advanced state via `store`, and return it.
///
/// Mirrors [`start`]'s MCP-readiness: this is exactly what a future
/// `next_step` MCP tool handler (Slice 11.D) calls.
///
/// Errors (never panics): `workflow_not_found` if `workflow_id` doesn't
/// resolve, `workflow_already_complete` if the workflow's `current_step` is
/// already `"complete"`, `workflow_step_input_mismatch` if `step_input`
/// doesn't match what the current step expects.
pub async fn advance(
    store: &WorkflowStore,
    workflow_id: &str,
    step_input: Value,
) -> CoreResult<WorkflowState> {
    let mut state = store.load(workflow_id)?;
    if state.current_step == STEP_COMPLETE {
        return Err(workflow_already_complete(workflow_id));
    }

    match state.kind {
        WorkflowKind::TriageMemoryLeak => {
            triage_memory_leak::run_step(&mut state, step_input).await?;
        }
        WorkflowKind::TuneGc => {
            tune_gc::run_step(&mut state, step_input).await?;
        }
        WorkflowKind::TraverseObjectGraph => {
            traverse_object_graph::run_step(&mut state, step_input).await?;
        }
        WorkflowKind::CompareSnapshots => {
            compare_snapshots::run_step(&mut state, step_input).await?;
        }
        WorkflowKind::ClassloaderLeak => {
            classloader_leak::run_step(&mut state, step_input).await?;
        }
    }

    state.updated_at = timestamp_now();
    store.save(&state)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state(workflow_id: &str) -> WorkflowState {
        let now = timestamp_now();
        WorkflowState {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            workflow_id: workflow_id.into(),
            kind: WorkflowKind::TriageMemoryLeak,
            created_at: now.clone(),
            updated_at: now,
            heap_path: "heap.hprof".into(),
            current_step: "detect".into(),
            step_history: Vec::new(),
            context: json!({}),
        }
    }

    #[test]
    fn workflow_store_round_trips_state() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(temp.path().to_path_buf());
        let state = sample_state("wf-123");

        store.save(&state).unwrap();
        let loaded = store.load("wf-123").unwrap();

        assert_eq!(loaded.workflow_id, state.workflow_id);
        assert_eq!(loaded.current_step, "detect");
    }

    #[test]
    fn workflow_store_load_missing_id_returns_workflow_not_found() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(temp.path().to_path_buf());

        let err = store.load("does-not-exist").unwrap_err();

        assert!(err.to_string().contains("workflow_not_found"));
    }

    #[test]
    fn workflow_store_load_corrupt_file_returns_structured_error() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(temp.path().to_path_buf());
        store.ensure_root().unwrap();
        std::fs::write(temp.path().join("bad.json"), b"{ not valid json").unwrap();

        let err = store.load("bad").unwrap_err();

        assert!(err.to_string().contains("workflow_corrupt"));
    }

    #[test]
    fn workflow_store_rejects_path_traversal_ids() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkflowStore::new(temp.path().to_path_buf());

        let err = store.load("../escaped").unwrap_err();

        assert!(err.to_string().contains("invalid workflow_id"));
    }

    #[test]
    fn describe_succeeds_for_all_workflow_kinds() {
        for kind in [
            WorkflowKind::TriageMemoryLeak,
            WorkflowKind::TuneGc,
            WorkflowKind::TraverseObjectGraph,
            WorkflowKind::CompareSnapshots,
            WorkflowKind::ClassloaderLeak,
        ] {
            let description = describe(kind)
                .unwrap_or_else(|err| panic!("describe({kind}) should succeed, got error: {err}"));
            assert_eq!(description.kind, kind);
            assert!(
                !description.steps.is_empty(),
                "describe({kind}) should return at least one step"
            );
        }
    }
}
