//! `TriageMemoryLeak` workflow (M11 Slice 11.A): `detect` →
//! `investigate_suspect` → `explain` → `propose_fix` → `complete`.
//!
//! Every step below is a thin wrapper around an existing, already-tested
//! primitive -- no new heap-analysis logic is introduced here (design doc
//! §3.3):
//!
//! - `detect` calls [`crate::analysis::detect_leaks`], exactly the same call
//!   shape the `detect_leaks` MCP tool handler uses today
//!   (`core/src/mcp/server.rs`).
//! - `investigate_suspect` calls [`crate::graph::find_all_gc_paths`] (M8
//!   Slice 8.A) and [`crate::analysis::analyze_by_referrer`] (M8 Slice 8.B).
//!   **Judgment call:** the design doc §4 leaves the exact primitive
//!   combination up to the implementer ("drill into the top suspect's
//!   GC-root path and referrer profile"). `find_all_gc_paths` is called with
//!   `by_class: Some(leak.class_name)` rather than resolving a specific
//!   `object_id` -- deliberately, because [`crate::analysis::LeakInsight`]
//!   (what the `detect` step returns) does not carry a raw object id, only a
//!   `class_name`, and `by_class` is the exact existing enumeration mode
//!   `find_all_gc_paths` already supports for "every live instance of this
//!   class" (see its own doc comment). `analyze_by_referrer` ranks the
//!   *whole* graph by referrer count, so to get "the referrer profile for
//!   the chosen suspect's class" specifically, this step requests a `top_n`
//!   wide enough to cover every object in the graph and then filters the
//!   ranked entries down to the ones whose `class_name` matches the chosen
//!   leak -- still the same unmodified `analyze_by_referrer` function, just
//!   called with a large `top_n` and filtered afterward, not a new ranking
//!   algorithm.
//! - `explain` calls [`crate::analysis::analyze_heap`] (AI disabled) plus
//!   [`crate::analysis::generate_ai_insights_async`], the same two calls the
//!   `explain_leak` MCP tool's `heap_path` branch makes today.
//!   **Judgment call:** to keep this workflow (and its test suite) usable
//!   without live AI provider credentials, this step force-sets
//!   `AiConfig::mode` to [`crate::config::AiMode::Rules`] -- this crate's
//!   documented offline-safe default (`AiMode`'s own `#[default]`) -- rather
//!   than trusting whatever mode a caller-supplied config might carry. Rules
//!   mode never makes a network call, so `explain` is deterministic and safe
//!   to exercise in CI.
//! - `propose_fix` calls [`crate::fix::propose_fix_with_config`], the same
//!   function the `propose_fix` MCP tool's `heap_path` branch calls, again
//!   forced into `AiMode::Rules` for the same offline-safety reason. This
//!   step is optional: passing `{"skip": true}` as its `step_input` skips
//!   straight to `complete` without generating a suggestion (design doc §4).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    analysis::{
        analyze_by_referrer, analyze_heap, detect_leaks, focus_leaks, generate_ai_insights_async,
        validate_leak_id, AnalyzeRequest, LeakDetectionOptions, LeakInsight, LeakKind,
        LeakSeverity,
    },
    config::{AiMode, AppConfig},
    errors::{CoreError, CoreResult},
    fix::{propose_fix_with_config, FixRequest, FixStyle},
    graph::{build_dominator_tree, find_all_gc_paths, AllPathsRequest},
    hprof::{parse_hprof_file_with_options, ParseOptions},
    workflow::{
        workflow_step_input_mismatch, ParamDescription, StepDescription, StepRecord,
        WorkflowDescription, WorkflowKind, WorkflowState,
    },
    HistogramGroupBy,
};

pub(crate) const STEP_DETECT: &str = "detect";
pub(crate) const STEP_INVESTIGATE_SUSPECT: &str = "investigate_suspect";
pub(crate) const STEP_EXPLAIN: &str = "explain";
pub(crate) const STEP_PROPOSE_FIX: &str = "propose_fix";
pub(crate) const STEP_COMPLETE: &str = "complete";

/// The fixed step order this workflow kind runs in. [`describe`]'s
/// `WorkflowDescription.steps` is generated from this same list, so the two
/// can never drift silently (the contract test in
/// `core/tests/workflow_triage_memory_leak.rs` asserts a live run's observed
/// step sequence equals `describe().steps.map(|s| s.name)`).
const STEP_ORDER: [&str; 4] = [
    STEP_DETECT,
    STEP_INVESTIGATE_SUSPECT,
    STEP_EXPLAIN,
    STEP_PROPOSE_FIX,
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
        kind: WorkflowKind::TriageMemoryLeak,
        steps: vec![
            StepDescription {
                name: STEP_DETECT.into(),
                description: "Run leak detection over the heap dump to find candidate leaks."
                    .into(),
                expected_input: vec![
                    ParamDescription::optional(
                        "min_severity",
                        "string",
                        "Lowest severity to report: LOW, MEDIUM, HIGH, or CRITICAL. Defaults to LOW so small heaps still surface candidates.",
                    ),
                    ParamDescription::optional(
                        "package",
                        "string",
                        "Optional package filter, matched against candidate class names.",
                    ),
                    ParamDescription::optional(
                        "leak_types",
                        "array",
                        "Optional explicit leak kinds to restrict detection to.",
                    ),
                ],
                underlying_primitives: vec!["detect_leaks".into()],
            },
            StepDescription {
                name: STEP_INVESTIGATE_SUSPECT.into(),
                description:
                    "Drill into the chosen suspect's GC-root path(s) and referrer profile."
                        .into(),
                expected_input: vec![ParamDescription::required(
                    "leak_id",
                    "string",
                    "The id of one of the leaks returned by the detect step.",
                )],
                underlying_primitives: vec![
                    "find_all_gc_paths".into(),
                    "analyze_by_referrer".into(),
                ],
            },
            StepDescription {
                name: STEP_EXPLAIN.into(),
                description:
                    "Generate an explanation of the chosen leak (rules-based/offline by default)."
                        .into(),
                expected_input: vec![],
                underlying_primitives: vec![
                    "analyze_heap".into(),
                    "generate_ai_insights_async".into(),
                ],
            },
            StepDescription {
                name: STEP_PROPOSE_FIX.into(),
                description: "Optionally generate a fix suggestion for the chosen leak. Pass {\"skip\": true} to skip straight to completion.".into(),
                expected_input: vec![
                    ParamDescription::optional(
                        "skip",
                        "boolean",
                        "Skip fix generation and complete the workflow.",
                    ),
                    ParamDescription::optional(
                        "style",
                        "string",
                        "Minimal, Defensive, or Comprehensive. Defaults to Minimal.",
                    ),
                ],
                underlying_primitives: vec!["propose_fix_with_config".into()],
            },
        ],
    }
}

/// Execute whatever step `state.current_step` currently names, using
/// `input` as that step's parameters. Appends a [`StepRecord`] to
/// `state.step_history`, updates `state.context` with whatever the step
/// produced, and advances `state.current_step` to the next step name (or
/// `"complete"` if this was the last one).
///
/// Called by [`crate::workflow::start`] (for the first, `detect`, step) and
/// [`crate::workflow::advance`] (for every subsequent step).
pub(crate) async fn run_step(state: &mut WorkflowState, input: Value) -> CoreResult<Value> {
    let step_name = state.current_step.clone();
    let output = match step_name.as_str() {
        STEP_DETECT => run_detect(state, &input).await?,
        STEP_INVESTIGATE_SUSPECT => run_investigate_suspect(state, &input)?,
        STEP_EXPLAIN => run_explain(state, &input).await?,
        STEP_PROPOSE_FIX => run_propose_fix(state, &input).await?,
        other => {
            return Err(workflow_step_input_mismatch(format!(
                "unknown triage_memory_leak step '{other}'"
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

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DetectInput {
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    leak_types: Option<Vec<LeakKind>>,
}

async fn run_detect(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<DetectInput>(
        input,
        "detect step expects an object with optional min_severity/package/leak_types fields",
    )?;

    let mut options = LeakDetectionOptions::new(parsed.min_severity.unwrap_or(LeakSeverity::Low));
    if let Some(package) = parsed.package {
        options.package_filters = vec![package];
    }
    if let Some(leak_types) = parsed.leak_types {
        options.leak_types = leak_types;
    }

    let leaks = detect_leaks(&state.heap_path, options).await?;

    state.context = json!({ "leaks": leaks });

    let top_leak_ids: Vec<String> = leaks.iter().take(5).map(|leak| leak.id.clone()).collect();
    Ok(json!({
        "leak_count": leaks.len(),
        "leaks": leaks,
        "top_leak_ids": top_leak_ids,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvestigateSuspectInput {
    leak_id: String,
}

fn run_investigate_suspect(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed: InvestigateSuspectInput = serde_json::from_value(input.clone()).map_err(|err| {
        workflow_step_input_mismatch(format!(
            "investigate_suspect step requires {{\"leak_id\": <string>}}: {err}"
        ))
    })?;

    let leaks = context_leaks(state)?;
    validate_leak_id(&leaks, &parsed.leak_id).map_err(|_| {
        workflow_step_input_mismatch(format!(
            "leak_id '{}' does not match any leak returned by the detect step",
            parsed.leak_id
        ))
    })?;
    let chosen = focus_leaks(&leaks, Some(parsed.leak_id.as_str()))
        .into_iter()
        .next()
        .expect("validate_leak_id above guarantees a match exists");

    // `LeakInsight::class_name` (from `detect_leaks`/`graph_backed_leaks`) is
    // the *raw* internal-form class name (e.g. `com/example/BigCache`,
    // slash-separated, straight from `ObjectGraph::class_name`).
    // `find_all_gc_paths`'s `by_class` mode, by contrast, matches against
    // the *prettified* dotted form (see `resolve_live_instances_by_class`'s
    // own doc comment: "matched against the prettified class name, e.g.
    // `com.example.Foo`"). These two existing functions were never composed
    // together before this workflow, so bridging that naming-convention gap
    // here -- not inventing a new matching rule -- is the actual "new" code
    // this step adds.
    let dotted_class_name = chosen.class_name.replace('/', ".");

    let gc_path = find_all_gc_paths(&AllPathsRequest {
        heap_path: state.heap_path.clone(),
        object_id: None,
        by_class: Some(dotted_class_name),
        max_paths: AllPathsRequest::DEFAULT_MAX_PATHS,
        max_depth: None,
    })?;

    let graph = parse_hprof_file_with_options(&state.heap_path, ParseOptions::default())?;
    let dominator = build_dominator_tree(&graph);
    // See module doc comment: analyze_by_referrer ranks the whole graph, so
    // request every object and filter to the suspect's class afterward
    // rather than trusting a small top_n to happen to include it.
    let referrer_top_n = graph.objects.len().max(1);
    let referrer_report = analyze_by_referrer(&graph, Some(&dominator), referrer_top_n);
    let referrer_entries: Vec<_> = referrer_report
        .entries
        .into_iter()
        .filter(|entry| entry.class_name == chosen.class_name)
        .collect();

    state.context["chosen_leak_id"] = json!(parsed.leak_id);
    state.context["chosen_class_name"] = json!(chosen.class_name);
    state.context["gc_path"] = serde_json::to_value(&gc_path)?;
    state.context["referrer_entries"] = serde_json::to_value(&referrer_entries)?;

    Ok(json!({
        "leak_id": parsed.leak_id,
        "class_name": chosen.class_name,
        "gc_path_length": gc_path.path_length,
        "gc_path_truncated": gc_path.truncated,
        "referrer_entry_count": referrer_entries.len(),
        "referrer_entries": referrer_entries,
    }))
}

async fn run_explain(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    // This step takes no parameters of its own; allow null or `{}` but
    // reject anything with actual content, which is more likely a caller
    // mistake (e.g. echoing a previous step's input) than intentional.
    let is_empty_object = input.as_object().is_some_and(|obj| obj.is_empty());
    if !input.is_null() && !is_empty_object {
        return Err(workflow_step_input_mismatch(
            "explain step takes no input; pass null or {}",
        ));
    }

    let leak_id = required_context_string(state, "chosen_leak_id", STEP_EXPLAIN)?;

    let mut analyze_config = AppConfig::default();
    analyze_config.ai.enabled = false;
    let leak_options = LeakDetectionOptions::new(LeakSeverity::Low);

    let analysis = analyze_heap(AnalyzeRequest {
        heap_path: state.heap_path.clone(),
        config: analyze_config.clone(),
        leak_options,
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await?;

    validate_leak_id(&analysis.leaks, &leak_id)?;
    let focused = focus_leaks(&analysis.leaks, Some(leak_id.as_str()));

    // Offline-safe default (see module doc comment): force AI on with
    // AiMode::Rules regardless of AppConfig::default()'s own settings, so
    // this step never requires live provider credentials.
    let mut ai_config = analyze_config.ai.clone();
    ai_config.enabled = true;
    ai_config.mode = AiMode::Rules;

    let insights = generate_ai_insights_async(&analysis.summary, &focused, &ai_config).await?;

    state.context["explanation_summary"] = json!(insights.summary);

    serde_json::to_value(&insights).map_err(CoreError::from)
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ProposeFixInput {
    #[serde(default)]
    skip: bool,
    #[serde(default)]
    style: Option<String>,
}

async fn run_propose_fix(state: &mut WorkflowState, input: &Value) -> CoreResult<Value> {
    let parsed = parse_step_input::<ProposeFixInput>(
        input,
        "propose_fix step expects an object with optional skip/style fields",
    )?;

    if parsed.skip {
        state.context["fix"] = json!({ "skipped": true });
        return Ok(json!({ "skipped": true }));
    }

    let leak_id = required_context_string(state, "chosen_leak_id", STEP_PROPOSE_FIX)?;

    let style = match parsed.style.as_deref() {
        Some("Defensive") => FixStyle::Defensive,
        Some("Comprehensive") => FixStyle::Comprehensive,
        _ => FixStyle::Minimal,
    };

    // Offline-safe default, same reasoning as `run_explain` above.
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.mode = AiMode::Rules;

    let response = propose_fix_with_config(
        FixRequest {
            heap_path: state.heap_path.clone(),
            leak_id: Some(leak_id),
            style,
            project_root: None,
        },
        &config,
    )
    .await?;

    let value = serde_json::to_value(&response)?;
    state.context["fix"] = value.clone();
    Ok(value)
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

fn context_leaks(state: &WorkflowState) -> CoreResult<Vec<LeakInsight>> {
    let leaks_value = state.context.get("leaks").ok_or_else(|| {
        workflow_step_input_mismatch(
            "investigate_suspect step requires the detect step to have run first",
        )
    })?;
    serde_json::from_value(leaks_value.clone()).map_err(CoreError::from)
}

fn required_context_string(state: &WorkflowState, key: &str, step: &str) -> CoreResult<String> {
    state
        .context
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            workflow_step_input_mismatch(format!(
                "{step} step requires investigate_suspect to have run first (missing context.{key})"
            ))
        })
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
        assert_eq!(next_step_name(STEP_DETECT), STEP_INVESTIGATE_SUSPECT);
        assert_eq!(next_step_name(STEP_INVESTIGATE_SUSPECT), STEP_EXPLAIN);
        assert_eq!(next_step_name(STEP_EXPLAIN), STEP_PROPOSE_FIX);
        assert_eq!(next_step_name(STEP_PROPOSE_FIX), STEP_COMPLETE);
        assert_eq!(next_step_name(STEP_COMPLETE), STEP_COMPLETE);
        assert_eq!(next_step_name("bogus"), STEP_COMPLETE);
    }
}
