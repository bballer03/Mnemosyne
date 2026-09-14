//! `ClassloaderLeak` workflow (M19 Slice 19.D): `detect` → `select` →
//! `inspect_retention` → `explain` → `complete`.
//!
//! Orchestration only over shipped M13/M8 primitives -- no new classloader
//! heuristics:
//!
//! - `detect` calls [`crate::analysis::analyze_classloaders`] (duplicate
//!   classes + potential_leaks + loader chains).
//! - `select` validates a duplicate class name and/or loader object id
//!   against the detect output.
//! - `inspect_retention` calls [`crate::analysis::inspect_object`] and
//!   [`crate::graph::find_all_gc_paths`] for the chosen loader.
//! - `explain` composes a deterministic, offline summary from prior step
//!   context (no live AI provider).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    analysis::{analyze_classloaders, inspect_object, ClassLoaderReport, DuplicateClassGroup},
    errors::CoreResult,
    graph::{build_dominator_tree, find_all_gc_paths, AllPathsRequest},
    hprof::{parse_hprof_file_with_options, ObjectId, ParseOptions},
    workflow::{
        workflow_step_input_mismatch, ParamDescription, StepDescription, StepRecord,
        WorkflowDescription, WorkflowKind, WorkflowState,
    },
};

pub(crate) const STEP_DETECT: &str = "detect";
pub(crate) const STEP_SELECT: &str = "select";
pub(crate) const STEP_INSPECT_RETENTION: &str = "inspect_retention";
pub(crate) const STEP_EXPLAIN: &str = "explain";
pub(crate) const STEP_COMPLETE: &str = "complete";

const STEP_ORDER: [&str; 4] = [
    STEP_DETECT,
    STEP_SELECT,
    STEP_INSPECT_RETENTION,
    STEP_EXPLAIN,
];

fn next_step_name(current: &str) -> &'static str {
    match STEP_ORDER.iter().position(|&name| name == current) {
        Some(idx) if idx + 1 < STEP_ORDER.len() => STEP_ORDER[idx + 1],
        _ => STEP_COMPLETE,
    }
}

pub fn describe() -> WorkflowDescription {
    WorkflowDescription {
        kind: WorkflowKind::ClassloaderLeak,
        steps: vec![
            StepDescription {
                name: STEP_DETECT.into(),
                description: "Run classloader analysis to surface duplicate classes (cross-loader) and single-loader potential-leak candidates.".into(),
                expected_input: vec![],
                underlying_primitives: vec!["analyze_classloaders".into()],
            },
            StepDescription {
                name: STEP_SELECT.into(),
                description: "Select a duplicate class and/or a loader object id from the detect step for deeper retention inspection.".into(),
                expected_input: vec![
                    ParamDescription::optional(
                        "class_name",
                        "string",
                        "Duplicate class name from detect.duplicate_classes (slash or dotted form).",
                    ),
                    ParamDescription::optional(
                        "loader_object_id",
                        "string",
                        "Loader object id (decimal or 0x-hex) from a duplicate group or potential_leaks entry.",
                    ),
                ],
                underlying_primitives: vec![],
            },
            StepDescription {
                name: STEP_INSPECT_RETENTION.into(),
                description: "Inspect the chosen loader object and enumerate bounded GC paths retaining it.".into(),
                expected_input: vec![],
                underlying_primitives: vec![
                    "inspect_object".into(),
                    "find_all_gc_paths".into(),
                ],
            },
            StepDescription {
                name: STEP_EXPLAIN.into(),
                description: "Compose an offline explanation of the selected classloader leak signal from prior step context.".into(),
                expected_input: vec![],
                underlying_primitives: vec![],
            },
        ],
    }
}

pub(crate) async fn run_step(state: &mut WorkflowState, input: Value) -> CoreResult<Value> {
    let step_name = state.current_step.clone();
    let output = match step_name.as_str() {
        STEP_DETECT => run_detect(state, &input)?,
        STEP_SELECT => run_select(state, &input)?,
        STEP_INSPECT_RETENTION => run_inspect_retention(state, &input)?,
        STEP_EXPLAIN => run_explain(state, &input)?,
        other => {
            return Err(workflow_step_input_mismatch(format!(
                "unknown classloader_leak step '{other}'"
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

fn reject_non_empty_input(input: &Value, step: &str) -> CoreResult<()> {
    let is_empty_object = input.as_object().is_some_and(|obj| obj.is_empty());
    if !input.is_null() && !is_empty_object {
        return Err(workflow_step_input_mismatch(format!(
            "{step} step takes no input; pass null or {{}}"
        )));
    }
    Ok(())
}

fn run_detect(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    reject_non_empty_input(input, STEP_DETECT)?;

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);
    let report = analyze_classloaders(&graph, Some(&dominator));

    let duplicate_class_names: Vec<String> = report
        .duplicate_classes
        .iter()
        .map(|group| group.class_name.clone())
        .collect();

    let output = json!({
        "loader_count": report.loaders.len(),
        "duplicate_class_count": report.duplicate_classes.len(),
        "potential_leak_count": report.potential_leaks.len(),
        "duplicate_classes": report.duplicate_classes,
        "potential_leaks": report.potential_leaks,
        "duplicate_class_names": duplicate_class_names,
        "loaders": report.loaders,
    });

    state.context = json!({ "classloader_report": report });
    Ok(output)
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SelectInput {
    #[serde(default)]
    class_name: Option<String>,
    #[serde(default)]
    loader_object_id: Option<String>,
}

fn normalize_class_name(name: &str) -> String {
    name.trim().replace('.', "/")
}

fn parse_object_id_input(raw: &str) -> CoreResult<ObjectId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(workflow_step_input_mismatch(
            "loader_object_id must not be empty",
        ));
    }

    let parsed = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else if trimmed
        .chars()
        .any(|character| matches!(character, 'A'..='F' | 'a'..='f'))
    {
        u64::from_str_radix(trimmed, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    };

    parsed.ok_or_else(|| {
        workflow_step_input_mismatch(format!(
            "loader_object_id '{trimmed}' is not a valid object id"
        ))
    })
}

fn context_report(state: &WorkflowState) -> CoreResult<ClassLoaderReport> {
    let value = state.context.get("classloader_report").ok_or_else(|| {
        workflow_step_input_mismatch(
            "classloader_report missing from workflow context; run detect first",
        )
    })?;
    serde_json::from_value(value.clone()).map_err(|err| {
        workflow_step_input_mismatch(format!(
            "classloader_report in workflow context is malformed: {err}"
        ))
    })
}

fn find_duplicate_group<'a>(
    report: &'a ClassLoaderReport,
    class_name: &str,
) -> Option<&'a DuplicateClassGroup> {
    let normalized = normalize_class_name(class_name);
    report
        .duplicate_classes
        .iter()
        .find(|group| normalize_class_name(&group.class_name) == normalized)
}

fn run_select(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed: SelectInput = serde_json::from_value(if input.is_null() {
        json!({})
    } else {
        input.clone()
    })
    .map_err(|err| {
        workflow_step_input_mismatch(format!(
            "select step expects {{\"class_name\"?: string, \"loader_object_id\"?: string}}: {err}"
        ))
    })?;

    if parsed.class_name.is_none() && parsed.loader_object_id.is_none() {
        return Err(workflow_step_input_mismatch(
            "select step requires at least one of class_name or loader_object_id",
        ));
    }

    let report = context_report(state)?;

    let selected_group = if let Some(class_name) = parsed.class_name.as_deref() {
        Some(find_duplicate_group(&report, class_name).ok_or_else(|| {
            workflow_step_input_mismatch(format!(
                "class_name '{class_name}' was not returned by the detect step's duplicate_classes"
            ))
        })?)
    } else {
        None
    };

    let loader_object_id = if let Some(raw) = parsed.loader_object_id.as_deref() {
        let id = parse_object_id_input(raw)?;
        let in_duplicates = report
            .duplicate_classes
            .iter()
            .any(|group| group.loader_object_ids.contains(&id));
        let in_leaks = report
            .potential_leaks
            .iter()
            .any(|leak| leak.object_id == id);
        let in_loaders = report.loaders.iter().any(|loader| loader.object_id == id);
        if !(in_duplicates || in_leaks || in_loaders) {
            return Err(workflow_step_input_mismatch(format!(
                "loader_object_id '{raw}' was not present in the detect step output"
            )));
        }
        if let Some(group) = selected_group {
            if !group.loader_object_ids.contains(&id) {
                return Err(workflow_step_input_mismatch(format!(
                    "loader_object_id '{raw}' is not one of the loaders for selected class '{}'",
                    group.class_name
                )));
            }
        }
        id
    } else {
        let group = selected_group.ok_or_else(|| {
            workflow_step_input_mismatch(
                "select step requires loader_object_id when class_name is omitted",
            )
        })?;
        *group.loader_object_ids.first().ok_or_else(|| {
            workflow_step_input_mismatch(
                "selected duplicate class has an empty loader_object_ids list",
            )
        })?
    };

    let loader_info = report
        .loaders
        .iter()
        .find(|loader| loader.object_id == loader_object_id)
        .cloned();

    let class_name = selected_group
        .map(|group| group.class_name.clone())
        .or_else(|| {
            report
                .duplicate_classes
                .iter()
                .find(|group| group.loader_object_ids.contains(&loader_object_id))
                .map(|group| group.class_name.clone())
        });

    let output = json!({
        "class_name": class_name,
        "loader_object_id": loader_object_id,
        "loader": loader_info,
        "duplicate_group": selected_group,
    });

    state.context["selection"] = output.clone();
    Ok(output)
}

fn run_inspect_retention(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    reject_non_empty_input(input, STEP_INSPECT_RETENTION)?;

    let selection = state.context.get("selection").ok_or_else(|| {
        workflow_step_input_mismatch("selection missing from workflow context; run select first")
    })?;
    let loader_object_id = selection
        .get("loader_object_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            workflow_step_input_mismatch("selection.loader_object_id missing from workflow context")
        })?;

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);
    let inspection =
        inspect_object(&graph, Some(&dominator), loader_object_id, false).ok_or_else(|| {
            workflow_step_input_mismatch(format!(
                "loader object id {loader_object_id} was not found in heap dump '{}'",
                state.heap_path
            ))
        })?;

    let id_width = usize::from(graph.identifier_size) * 2;
    let object_id_hex = format!("0x{loader_object_id:0id_width$X}");
    let gc_paths = find_all_gc_paths(&AllPathsRequest {
        heap_path: state.heap_path.clone(),
        object_id: Some(object_id_hex),
        by_class: None,
        max_paths: AllPathsRequest::DEFAULT_MAX_PATHS,
        max_depth: None,
    })?;

    let ancestor_chain = selection
        .get("loader")
        .and_then(|loader| loader.get("ancestor_chain"))
        .cloned()
        .unwrap_or_else(|| json!([]));

    let output = json!({
        "loader_object_id": loader_object_id,
        "inspection": inspection,
        "gc_paths": gc_paths,
        "ancestor_chain": ancestor_chain,
    });

    state.context["retention"] = output.clone();
    Ok(output)
}

fn run_explain(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    reject_non_empty_input(input, STEP_EXPLAIN)?;

    let selection = state.context.get("selection").cloned().unwrap_or(json!({}));
    let retention = state.context.get("retention").cloned().unwrap_or(json!({}));
    let report = context_report(state)?;

    let class_name = selection
        .get("class_name")
        .and_then(Value::as_str)
        .unwrap_or("(unspecified class)");
    let loader_object_id = selection
        .get("loader_object_id")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let path_count = retention
        .get("gc_paths")
        .and_then(|paths| paths.get("all_paths"))
        .and_then(Value::as_array)
        .map(|paths| paths.len())
        .unwrap_or(0);
    let truncated = retention
        .get("gc_paths")
        .and_then(|paths| paths.get("truncated"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let summary = format!(
        "Classloader investigation for '{class_name}' via loader object {loader_object_id}: \
         detect found {} duplicate class group(s) and {} potential leak candidate(s). \
         Retention inspection enumerated {path_count} GC path(s){} to the selected loader. \
         Cross-check duplicate classes and loader ancestor chains in the classloader explorer; \
         use the object inspector on the loader id for field-level retention detail.",
        report.duplicate_classes.len(),
        report.potential_leaks.len(),
        if truncated { " (truncated)" } else { "" },
    );

    let output = json!({
        "summary": summary,
        "class_name": class_name,
        "loader_object_id": loader_object_id,
        "duplicate_class_count": report.duplicate_classes.len(),
        "potential_leak_count": report.potential_leaks.len(),
        "gc_path_count": path_count,
        "gc_paths_truncated": truncated,
        "mode": "rules_offline",
    });

    state.context["explanation"] = output.clone();
    Ok(output)
}
