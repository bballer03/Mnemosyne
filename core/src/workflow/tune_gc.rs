//! `TuneGc` workflow (M11 Slice 11.B): `root_kind_breakdown` →
//! `thread_local_review` → `top_retainers` → `complete`.
//!
//! ## Honesty constraint (design doc §2 point 2, §4, §9 R2)
//!
//! **Mnemosyne never touches a live JVM or applies a GC flag.** This
//! workflow is a *diagnostic* GC-root retention review only -- which root
//! *kinds* retain the most memory, which threads carry the largest
//! thread-local footprint, where the dominator tree's top retainers sit.
//! The name `tune_gc` matches roadmap.md's workflow list, but the
//! deliverable is data for a *human* to act on manually (heap sizing,
//! generation ratios, retention root-causing) -- never live tuning action.
//! This is stated explicitly in [`describe`]'s own returned step
//! descriptions below (not just in this doc comment), per the design doc's
//! explicit requirement that the honesty constraint live in the
//! agent/user-facing `describe_workflow` output itself.
//!
//! Every step below is a thin wrapper around an existing, already-tested
//! primitive -- no new heap-analysis logic (design doc §3.3):
//!
//! - `root_kind_breakdown` groups [`crate::hprof::ObjectGraph::gc_roots`] by
//!   [`crate::hprof::GcRootKind`] (via
//!   [`crate::graph::gc_root_path::gc_root_kind`], the exact same
//!   classification `core::graph::gc_root_path` already uses for GC-root-path
//!   lookups -- made `pub(crate)` specifically for this reuse, so this step
//!   does not duplicate that 9-arm match a second time) and sums
//!   [`crate::graph::DominatorTree::retained_size`] per root. The grouping
//!   *aggregation* itself (loop + `BTreeMap` accumulation) is simple enough
//!   glue to write inline here rather than a new top-level analyzer -- no
//!   new retained-size math is introduced, only composition of two existing
//!   primitives (design doc §4 point 2 / §3.3's "zero new analysis logic").
//! - `thread_local_review` calls [`crate::analysis::inspect_threads`] as-is.
//! - `top_retainers` calls [`crate::graph::DominatorTree::top_retained`] as-is.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    analysis::inspect_threads,
    errors::CoreResult,
    graph::{build_dominator_tree, gc_root_path::gc_root_kind},
    hprof::{parse_hprof_file_with_options, GcRootKind, ParseOptions},
    workflow::{
        workflow_step_input_mismatch, ParamDescription, StepDescription, StepRecord,
        WorkflowDescription, WorkflowKind, WorkflowState,
    },
};

pub(crate) const STEP_ROOT_KIND_BREAKDOWN: &str = "root_kind_breakdown";
pub(crate) const STEP_THREAD_LOCAL_REVIEW: &str = "thread_local_review";
pub(crate) const STEP_TOP_RETAINERS: &str = "top_retainers";
pub(crate) const STEP_COMPLETE: &str = "complete";

/// Default "top N" size for the `thread_local_review`/`top_retainers`
/// steps' optional `top_n` input, matching `AnalyzeRequest::top_n`'s own
/// default (`core::analysis::engine`) -- this crate's established default
/// for "how many top-N entries" absent an explicit caller preference.
const DEFAULT_TOP_N: usize = 10;

/// The fixed step order this workflow kind runs in. Linear, like
/// `TriageMemoryLeak` -- `TuneGc` has no branch point (that's
/// `TraverseObjectGraph`'s distinction, see design doc §8 Slice 11.B).
const STEP_ORDER: [&str; 3] = [
    STEP_ROOT_KIND_BREAKDOWN,
    STEP_THREAD_LOCAL_REVIEW,
    STEP_TOP_RETAINERS,
];

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
        kind: WorkflowKind::TuneGc,
        steps: vec![
            StepDescription {
                name: STEP_ROOT_KIND_BREAKDOWN.into(),
                description: "Group GC roots by kind (thread locals, JNI globals, sticky classes, monitor-used, etc.) and sum retained size per kind. Diagnostic data only -- Mnemosyne never touches a live JVM or applies a GC flag itself; this is meant to inform a human's own manual GC-tuning decisions (heap sizing, generation ratios, retention root-causing), not to perform any tuning action.".into(),
                expected_input: vec![],
                underlying_primitives: vec![
                    "ObjectGraph::gc_roots".into(),
                    "DominatorTree::retained_size".into(),
                ],
            },
            StepDescription {
                name: STEP_THREAD_LOCAL_REVIEW.into(),
                description: "Review per-thread retained size and thread-local object counts. Diagnostic only -- no live JVM interaction, no GC flags applied.".into(),
                expected_input: vec![ParamDescription::optional(
                    "top_n",
                    "integer",
                    "How many top-retaining threads to highlight. Defaults to 10.",
                )],
                underlying_primitives: vec!["inspect_threads".into()],
            },
            StepDescription {
                name: STEP_TOP_RETAINERS.into(),
                description: "List the dominator tree's top retainers by retained size, to cross-reference against the root-kind breakdown. Diagnostic only -- Mnemosyne does not tune a live JVM's garbage collector.".into(),
                expected_input: vec![ParamDescription::optional(
                    "top_n",
                    "integer",
                    "How many top retainers to return. Defaults to 10.",
                )],
                underlying_primitives: vec!["DominatorTree::top_retained".into()],
            },
        ],
    }
}

/// Execute whatever step `state.current_step` currently names, using
/// `input` as that step's parameters. Mirrors
/// [`crate::workflow::triage_memory_leak::run_step`]'s shape exactly --
/// same append-to-`step_history`-then-advance-`current_step` contract.
pub(crate) async fn run_step(state: &mut WorkflowState, input: Value) -> CoreResult<Value> {
    let step_name = state.current_step.clone();
    let output = match step_name.as_str() {
        STEP_ROOT_KIND_BREAKDOWN => run_root_kind_breakdown(state, &input)?,
        STEP_THREAD_LOCAL_REVIEW => run_thread_local_review(state, &input)?,
        STEP_TOP_RETAINERS => run_top_retainers(state, &input)?,
        other => {
            return Err(workflow_step_input_mismatch(format!(
                "unknown tune_gc step '{other}'"
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

fn run_root_kind_breakdown(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    reject_non_empty_input(input, STEP_ROOT_KIND_BREAKDOWN)?;

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);

    // Thin composition, not a new analyzer (module doc comment / design doc
    // §3.3): `gc_root_kind` and `DominatorTree::retained_size` already exist
    // and are already tested; this loop only aggregates their outputs.
    let mut totals: BTreeMap<GcRootKind, (usize, u64)> = BTreeMap::new();
    for root in &graph.gc_roots {
        let kind = gc_root_kind(root.root_type.clone());
        let entry = totals.entry(kind).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += dominator.retained_size(root.object_id);
    }

    let root_kinds: Vec<Value> = totals
        .into_iter()
        .map(|(kind, (root_count, retained_bytes))| {
            json!({
                "kind": format!("{kind:?}"),
                "root_count": root_count,
                "retained_bytes": retained_bytes,
            })
        })
        .collect();

    let output = json!({
        "root_kinds": root_kinds,
        "total_roots": graph.gc_roots.len(),
    });

    state.context["root_kind_breakdown"] = output.clone();
    Ok(output)
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TopNInput {
    #[serde(default)]
    top_n: Option<usize>,
}

fn run_thread_local_review(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<TopNInput>(
        input,
        "thread_local_review step expects an object with an optional top_n field",
    )?;
    let top_n = parsed.top_n.unwrap_or(DEFAULT_TOP_N);

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);
    let report = inspect_threads(&graph, Some(&dominator), top_n);

    let value = serde_json::to_value(&report)?;
    state.context["thread_local_review"] = value.clone();
    Ok(value)
}

fn run_top_retainers(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<TopNInput>(
        input,
        "top_retainers step expects an object with an optional top_n field",
    )?;
    let top_n = parsed.top_n.unwrap_or(DEFAULT_TOP_N);

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);
    let id_size = graph.identifier_size as usize;

    let top_retainers: Vec<Value> = dominator
        .top_retained(top_n)
        .into_iter()
        .map(|(object_id, retained_bytes)| {
            let class_name = graph
                .get_object(object_id)
                .and_then(|obj| graph.class_name(obj.class_id))
                .map(|name| name.replace('/', "."))
                .unwrap_or_else(|| "<unknown>".to_string());
            let width = id_size * 2;
            json!({
                "object_id": format!("0x{object_id:0width$X}"),
                "class_name": class_name,
                "retained_bytes": retained_bytes,
            })
        })
        .collect();

    let output = json!({ "top_retainers": top_retainers });
    state.context["top_retainers"] = output.clone();
    Ok(output)
}

/// `root_kind_breakdown` takes no parameters (design doc §4 point 2 lists
/// none) -- allow `null`/`{}`, reject anything with actual content, same
/// convention `triage_memory_leak::run_explain` established for its own
/// parameterless step.
fn reject_non_empty_input(input: &Value, step: &str) -> CoreResult<()> {
    let is_empty_object = input.as_object().is_some_and(|obj| obj.is_empty());
    if !input.is_null() && !is_empty_object {
        return Err(workflow_step_input_mismatch(format!(
            "{step} step takes no input; pass null or {{}}"
        )));
    }
    Ok(())
}

/// Same small `null`-defaults-to-`T::default()` convenience
/// `triage_memory_leak::parse_step_input` uses -- duplicated per-module
/// rather than shared, matching this crate's established convention of each
/// workflow kind owning its own strongly-typed step-input helpers (design
/// doc §9 R4).
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
        assert_eq!(
            next_step_name(STEP_ROOT_KIND_BREAKDOWN),
            STEP_THREAD_LOCAL_REVIEW
        );
        assert_eq!(next_step_name(STEP_THREAD_LOCAL_REVIEW), STEP_TOP_RETAINERS);
        assert_eq!(next_step_name(STEP_TOP_RETAINERS), STEP_COMPLETE);
        assert_eq!(next_step_name(STEP_COMPLETE), STEP_COMPLETE);
        assert_eq!(next_step_name("bogus"), STEP_COMPLETE);
    }

    #[test]
    fn describe_states_no_live_jvm_constraint_explicitly() {
        // §9 R2 / §2 point 2: the honesty constraint must be in the
        // returned description TEXT, not just this module's doc comment.
        let description = describe();
        let root_kind_step = &description.steps[0];
        assert!(root_kind_step
            .description
            .contains("never touches a live JVM"));
        assert!(root_kind_step.description.contains("GC flag"));
    }
}
