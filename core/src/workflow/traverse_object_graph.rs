//! `TraverseObjectGraph` workflow (M11 Slice 11.B): `inspect` →
//! `choose_direction` → loops back to `inspect` on the chosen id, or ends at
//! `complete`.
//!
//! This is the one workflow kind with a real branch point (design doc §4
//! point 4 / §8 Slice 11.B): `choose_direction`'s caller-supplied
//! `object_id` must be one of the ids the *prior* `inspect` step actually
//! offered (its `references_out`/`referrers_in`), and whether the walk
//! loops back to `inspect` or ends at `complete` depends on that same
//! step's input (omit `object_id` to end the walk). See
//! [`run_step`]/[`run_choose_direction`] below for how this differs
//! structurally from `triage_memory_leak`'s/`tune_gc`'s purely linear
//! `next_step_name(current) -> next` shape (this kind cannot compute its
//! next step from `current_step` alone -- it also depends on the step's
//! *outcome*).
//!
//! Every step is a thin wrapper around one existing, already-tested
//! primitive (design doc §3.3): both `inspect` and `choose_direction` call
//! [`crate::analysis::inspect_object`] (M8 Slice 8.C) -- `choose_direction`
//! itself does not call it directly, but the walk it enables is entirely
//! `inspect_object` calls chained together via this workflow's own
//! bookkeeping (`state.context`), not any new graph-walking algorithm.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    analysis::inspect_object,
    errors::CoreResult,
    graph::build_dominator_tree,
    hprof::{parse_hprof_file_with_options, ParseOptions},
    workflow::{
        workflow_step_input_mismatch, ParamDescription, StepDescription, StepRecord,
        WorkflowDescription, WorkflowKind, WorkflowState,
    },
};

pub(crate) const STEP_INSPECT: &str = "inspect";
pub(crate) const STEP_CHOOSE_DIRECTION: &str = "choose_direction";
pub(crate) const STEP_COMPLETE: &str = "complete";

/// Static step-sequence description for this workflow kind. No side
/// effects, no `WorkflowState` created.
///
/// Note this lists each *distinct* step name exactly once -- `inspect` and
/// `choose_direction` -- even though a live run can visit both repeatedly
/// (see the contract-test note in
/// `core/tests/workflow_traverse_object_graph.rs` for how the §6.1
/// contract test is adapted to account for that).
pub fn describe() -> WorkflowDescription {
    WorkflowDescription {
        kind: WorkflowKind::TraverseObjectGraph,
        steps: vec![
            StepDescription {
                name: STEP_INSPECT.into(),
                description: "Inspect one object: identity, size, dominator context, and refs in/out. The very first call (via start_workflow) must supply object_id; later calls (after a choose_direction step picked a direction) reuse that chosen id automatically and object_id may be omitted.".into(),
                expected_input: vec![ParamDescription::optional(
                    "object_id",
                    "string",
                    "The object id to inspect. Required on the first call; inferred from the prior choose_direction step otherwise.",
                )],
                underlying_primitives: vec!["inspect_object".into()],
            },
            StepDescription {
                name: STEP_CHOOSE_DIRECTION.into(),
                description: "Pick one of the just-inspected object's references_out or referrers_in ids to step into next -- loops back to the inspect step on that id. Omit object_id to end the walk and complete instead.".into(),
                expected_input: vec![ParamDescription::optional(
                    "object_id",
                    "string",
                    "One of the ids from the prior inspect step's references_out/referrers_in. Omit to end the walk.",
                )],
                underlying_primitives: vec!["inspect_object".into()],
            },
        ],
    }
}

/// Execute whatever step `state.current_step` currently names, using
/// `input` as that step's parameters. Unlike
/// [`crate::workflow::triage_memory_leak::run_step`] /
/// [`crate::workflow::tune_gc::run_step`], the *next* `current_step` is not
/// a pure function of the step just executed -- `choose_direction`'s
/// outcome (did the caller pick a valid id, or end the walk?) decides
/// whether the next step is `inspect` (loop) or `complete` (end). See
/// [`run_choose_direction`]'s `{"ended": true}` vs `{"chosen_object_id":
/// ...}` output shapes, which this function reads back to make that call.
pub(crate) async fn run_step(state: &mut WorkflowState, input: Value) -> CoreResult<Value> {
    let step_name = state.current_step.clone();
    let output = match step_name.as_str() {
        STEP_INSPECT => run_inspect(state, &input)?,
        STEP_CHOOSE_DIRECTION => run_choose_direction(state, &input)?,
        other => {
            return Err(workflow_step_input_mismatch(format!(
                "unknown traverse_object_graph step '{other}'"
            )))
        }
    };

    state.step_history.push(StepRecord {
        step_name: step_name.clone(),
        input,
        output_summary: output.clone(),
        timestamp: crate::mcp::session::timestamp_now(),
    });

    state.current_step = match step_name.as_str() {
        STEP_INSPECT => STEP_CHOOSE_DIRECTION.to_string(),
        STEP_CHOOSE_DIRECTION => {
            if output.get("ended").and_then(Value::as_bool) == Some(true) {
                STEP_COMPLETE.to_string()
            } else {
                STEP_INSPECT.to_string()
            }
        }
        _ => STEP_COMPLETE.to_string(),
    };

    Ok(output)
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct InspectInput {
    #[serde(default)]
    object_id: Option<String>,
}

fn run_inspect(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<InspectInput>(
        input,
        "inspect step expects an object with an optional object_id field (required on the first call)",
    )?;

    let object_id_str = parsed
        .object_id
        .or_else(|| {
            state
                .context
                .get("current_object_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| {
            workflow_step_input_mismatch(
                "inspect step requires an object_id (either as step input on the first call, \
                 or from a prior choose_direction step)",
            )
        })?;

    let object_id = parse_object_id(&object_id_str).ok_or_else(|| {
        workflow_step_input_mismatch(format!(
            "object_id '{object_id_str}' is not a valid object id"
        ))
    })?;

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);

    let inspection =
        inspect_object(&graph, Some(&dominator), object_id, false).ok_or_else(|| {
            workflow_step_input_mismatch(format!(
                "object_id '{object_id_str}' was not found in heap dump '{}'",
                state.heap_path
            ))
        })?;

    let value = serde_json::to_value(&inspection)?;
    state.context["current_object_id"] = json!(inspection.object_id);
    state.context["references_out"] = value["references_out"].clone();
    state.context["referrers_in"] = value["referrers_in"].clone();

    Ok(value)
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ChooseDirectionInput {
    #[serde(default)]
    object_id: Option<String>,
}

/// The branch point (module doc comment / design doc §8 Slice 11.B): the
/// caller-picked `object_id` must be one of the ids the prior `inspect`
/// step actually returned (`references_out`/`referrers_in`, recorded into
/// `state.context` by [`run_inspect`]) -- rejecting an out-of-band id with
/// `workflow_step_input_mismatch` rather than silently inspecting an
/// unrelated object is the "interesting new logic" this slice adds.
/// Omitting `object_id` ends the walk instead of looping.
fn run_choose_direction(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<ChooseDirectionInput>(
        input,
        "choose_direction step expects an object with an optional object_id field (omit to end the walk)",
    )?;

    match parsed.object_id {
        None => Ok(json!({ "ended": true })),
        Some(chosen_id) => {
            let offered = offered_object_ids(state)?;
            if !offered.contains(&chosen_id) {
                return Err(workflow_step_input_mismatch(format!(
                    "object_id '{chosen_id}' was not among the references_out/referrers_in the prior inspect step returned"
                )));
            }
            state.context["current_object_id"] = json!(chosen_id.clone());
            Ok(json!({ "chosen_object_id": chosen_id }))
        }
    }
}

/// The set of object ids the most recent `inspect` step offered (its
/// `references_out` + `referrers_in`, as recorded into `state.context` by
/// [`run_inspect`]) -- what a `choose_direction` input is validated against.
fn offered_object_ids(state: &WorkflowState) -> CoreResult<HashSet<String>> {
    let mut ids = HashSet::new();
    for key in ["references_out", "referrers_in"] {
        if let Some(list) = state.context.get(key).and_then(Value::as_array) {
            for entry in list {
                if let Some(id) = entry.get("object_id").and_then(Value::as_str) {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    if ids.is_empty() {
        return Err(workflow_step_input_mismatch(
            "choose_direction step requires the inspect step to have run first",
        ));
    }
    Ok(ids)
}

/// Parse an `object_id` string (`0x...` hex or bare decimal), mirroring the
/// parsing convention `core::graph::gc_path` / `core::mcp::server` each
/// already duplicate locally for the same purpose -- this workflow module
/// owns its own copy rather than reaching into either of those, matching
/// the established per-module-ownership convention (design doc §9 R4).
fn parse_object_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }
    if trimmed.chars().any(|c| matches!(c, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed, 16).ok();
    }
    trimmed.parse::<u64>().ok()
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
    fn describe_lists_inspect_then_choose_direction_once_each() {
        let description = describe();
        let names: Vec<String> = description.steps.iter().map(|s| s.name.clone()).collect();
        assert_eq!(
            names,
            vec![STEP_INSPECT.to_string(), STEP_CHOOSE_DIRECTION.to_string()]
        );
    }

    #[test]
    fn parse_object_id_accepts_hex_and_decimal() {
        assert_eq!(parse_object_id("0x1A"), Some(26));
        assert_eq!(parse_object_id("0X1a"), Some(26));
        assert_eq!(parse_object_id("26"), Some(26));
        assert_eq!(parse_object_id(""), None);
        assert_eq!(parse_object_id("not-a-number"), None);
    }
}
