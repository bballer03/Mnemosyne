use super::engine::LeakInsight;
use crate::{
    config::{AiConfig, AiMode, AiTaskDefinition, AiTaskKind},
    hprof::HeapSummary,
    llm::{complete as complete_llm, LlmCompletionRequest},
    prompts::{render_provider_instructions, ProviderPromptContext},
    CoreError, CoreResult,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use tracing::info;

fn escape_toon_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInsights {
    pub model: String,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub confidence: f32,
    pub wire: AiWireExchange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiWireExchange {
    pub format: AiWireFormat,
    pub prompt: String,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum AiWireFormat {
    #[default]
    Toon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiChatTurn {
    pub question: String,
    pub answer_summary: String,
}

/// Narrow the leak set to a specific identifier; falls back to the full list if
/// no matching leak is found so downstream callers always have context.
pub fn focus_leaks(leaks: &[LeakInsight], leak_id: Option<&str>) -> Vec<LeakInsight> {
    if leaks.is_empty() {
        return Vec::new();
    }

    if let Some(target) = leak_id {
        let matches: Vec<LeakInsight> = leaks
            .iter()
            .filter(|leak| leak.id == target || leak.class_name == target)
            .cloned()
            .collect();
        if !matches.is_empty() {
            return matches;
        }
    }

    leaks.to_vec()
}

/// Validate that a given leak ID exists in the leak set.
/// Returns an error if the ID is specified but not found.
pub fn validate_leak_id(leaks: &[LeakInsight], leak_id: &str) -> CoreResult<()> {
    if leaks
        .iter()
        .any(|leak| leak.id == leak_id || leak.class_name == leak_id)
    {
        Ok(())
    } else {
        Err(CoreError::InvalidInput(format!(
            "no leak found matching identifier '{leak_id}'"
        )))
    }
}

/// Generate a deterministic, heuristic "AI" insight so that higher layers can
/// exercise the UX before real LLM integration is available.
pub fn generate_ai_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    match config.mode {
        AiMode::Stub => Ok(generate_stub_ai_insights(summary, leaks, config)),
        AiMode::Rules => Ok(generate_rule_based_ai_insights(summary, leaks, config)),
        AiMode::Provider => generate_provider_ai_insights(summary, leaks, config),
    }
}

pub async fn generate_ai_insights_async(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    if matches!(config.mode, AiMode::Provider) {
        let summary = summary.clone();
        let leaks = leaks.to_vec();
        let config = config.clone();
        return tokio::task::spawn_blocking(move || {
            generate_ai_insights(&summary, &leaks, &config)
        })
        .await
        .map_err(|err| CoreError::Other(err.into()))?;
    }

    generate_ai_insights(summary, leaks, config)
}

pub fn generate_ai_chat_turn(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    match config.mode {
        AiMode::Stub => Ok(generate_stub_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        )),
        AiMode::Rules => Ok(generate_rule_based_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        )),
        AiMode::Provider => generate_provider_chat_insights(
            summary,
            leaks,
            question,
            history,
            focus_leak_id,
            config,
        ),
    }
}

pub async fn generate_ai_chat_turn_async(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    if matches!(config.mode, AiMode::Provider) {
        let summary = summary.clone();
        let leaks = leaks.to_vec();
        let question = question.to_string();
        let history = history.to_vec();
        let focus_leak_id = focus_leak_id.map(str::to_string);
        let config = config.clone();
        return tokio::task::spawn_blocking(move || {
            generate_ai_chat_turn(
                &summary,
                &leaks,
                &question,
                &history,
                focus_leak_id.as_deref(),
                &config,
            )
        })
        .await
        .map_err(|err| CoreError::Other(err.into()))?;
    }

    generate_ai_chat_turn(summary, leaks, question, history, focus_leak_id, config)
}

fn generate_provider_ai_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    let prompt =
        redact_provider_prompt(build_provider_toon_prompt(summary, leaks, config)?, config)?;
    emit_provider_audit_log(&prompt, config);
    let response = complete_llm(&LlmCompletionRequest {
        prompt: prompt.clone(),
        config: config.clone(),
    })?;
    parse_provider_toon_response(config, prompt, response.text)
}

fn generate_provider_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<AiInsights> {
    let prompt = redact_provider_prompt(
        build_provider_chat_toon_prompt(summary, leaks, question, history, focus_leak_id, config)?,
        config,
    )?;
    emit_provider_audit_log(&prompt, config);
    let response = complete_llm(&LlmCompletionRequest {
        prompt: prompt.clone(),
        config: config.clone(),
    })?;
    parse_provider_toon_response(config, prompt, response.text)
}

fn emit_provider_audit_log(prompt: &str, config: &AiConfig) {
    if !config.privacy.audit_log {
        return;
    }

    info!(
        provider = %config.provider,
        model = %config.model,
        prompt_sha256 = %sha256_hex(prompt),
        prompt_bytes = prompt.len(),
        redact_heap_path = config.privacy.redact_heap_path,
        redact_pattern_count = config.privacy.redact_patterns.len(),
        "provider_ai_audit"
    );
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn redact_provider_prompt(prompt: String, config: &AiConfig) -> CoreResult<String> {
    let mut redacted = prompt;

    if config.privacy.redact_heap_path {
        redacted = redact_toon_key(&redacted, "heap_path", "<REDACTED>");
    }

    for pattern in &config.privacy.redact_patterns {
        let regex = Regex::new(pattern).map_err(|err| {
            CoreError::InvalidInput(format!(
                "invalid ai.privacy.redact_patterns entry `{pattern}`: {err}"
            ))
        })?;
        redacted = regex.replace_all(&redacted, "<REDACTED>").into_owned();
    }

    Ok(redacted)
}

fn redact_toon_key(prompt: &str, key: &str, replacement: &str) -> String {
    let prefix = format!("{key}=");
    prompt
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&prefix) {
                let indent_len = line.len() - trimmed.len();
                let indent = " ".repeat(indent_len);
                format!("{indent}{key}={replacement}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_stub_ai_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> AiInsights {
    let top = leaks
        .iter()
        .max_by_key(|leak| leak.retained_size_bytes)
        .cloned();

    let summary_text = match &top {
        Some(leak) => format!(
            "{class} is retaining ~{size:.2} MB via {instances} instances; prioritize freeing it to reclaim {percent:.1}% of the heap.",
            class = leak.class_name,
            size = bytes_to_mb(leak.retained_size_bytes),
            instances = leak.instances,
            percent = retained_percent(leak.retained_size_bytes, summary.total_size_bytes),
        ),
        None => format!(
            "Heap `{}` looks healthy; continue monitoring but no blockers were detected.",
            summary.heap_path
        ),
    };

    let mut recs = Vec::new();
    if let Some(leak) = &top {
        recs.push(format!(
            "Guard {} lifetimes: ensure cleanup hooks dispose unused entries.",
            leak.class_name
        ));
        recs.push("Add targeted instrumentation (counters, timers) around the suspected allocation sites.".into());
        if leak.severity >= crate::analysis::LeakSeverity::High {
            recs.push(
                "Review threading / coroutine lifecycles anchoring these objects to a GC root."
                    .into(),
            );
        }
    } else {
        recs.push("Capture a heap dump under load to validate steady-state behavior.".into());
    }

    let confidence = (0.55 + leaks.len() as f32 * 0.05 - config.temperature * 0.1).clamp(0.3, 0.92);

    AiInsights {
        model: config.model.clone(),
        summary: summary_text,
        recommendations: recs,
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt: build_toon_prompt(summary, leaks),
            response: build_toon_response(summary, &top, confidence, config),
        },
    }
}

fn generate_rule_based_ai_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> AiInsights {
    let top = leaks
        .iter()
        .max_by_key(|leak| leak.retained_size_bytes)
        .cloned();
    let context = AiTaskContext {
        summary,
        leaks,
        top_leak: top.clone(),
        config,
    };

    let mut summary_text = String::new();
    let mut recommendations = Vec::new();
    let mut confidence = (0.52 - config.temperature * 0.08).clamp(0.3, 0.9);

    for task in &config.tasks {
        if !task.enabled {
            continue;
        }

        let output = run_rule_task(task, &context);
        if summary_text.is_empty() {
            if let Some(fragment) = output.summary_fragment {
                summary_text = fragment;
            }
        }
        recommendations.extend(output.recommendations);
        confidence = (confidence + output.confidence_delta).clamp(0.3, 0.92);
    }

    if summary_text.is_empty() {
        summary_text = default_rule_summary(&context);
    }

    if recommendations.is_empty() {
        recommendations.push(default_rule_recommendation(&context));
    }

    AiInsights {
        model: config.model.clone(),
        summary: summary_text,
        recommendations,
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt: build_toon_prompt(summary, leaks),
            response: build_toon_response(summary, &top, confidence, config),
        },
    }
}

fn generate_rule_based_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> AiInsights {
    let leak_context = focus_leaks(leaks, focus_leak_id);
    let prompt = build_chat_toon_prompt(
        summary,
        &leak_context,
        question,
        history,
        focus_leak_id,
        config,
    );
    let top = leak_context
        .iter()
        .max_by_key(|leak| leak.retained_size_bytes);
    let summary_text = match top {
        Some(leak) => format!(
            "Answering question '{}': {} retains ~{:.2} MB via {} instances and should be investigated first.",
            question,
            leak.class_name,
            bytes_to_mb(leak.retained_size_bytes),
            leak.instances,
        ),
        None => format!(
            "Answering question '{}': Heap '{}' currently looks healthy based on the analyzed summary.",
            question, summary.heap_path,
        ),
    };
    let recommendations = if let Some(leak) = top {
        vec![
            format!("Inspect the owner path retaining {}.", leak.class_name),
            "Compare the active leak against the top shortlist before expanding scope.".into(),
        ]
    } else {
        vec!["Capture another heap under load to confirm the healthy-heap baseline.".into()]
    };
    let confidence = 0.68;

    AiInsights {
        model: config.model.clone(),
        summary: summary_text.clone(),
        recommendations: recommendations.clone(),
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt,
            response: build_chat_toon_response(&summary_text, &recommendations, confidence, config),
        },
    }
}

fn generate_stub_chat_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> AiInsights {
    let mut ai =
        generate_rule_based_chat_insights(summary, leaks, question, history, focus_leak_id, config);
    ai.confidence = 0.55;
    let leak_context = focus_leaks(leaks, focus_leak_id);
    ai.wire.prompt = build_chat_toon_prompt(
        summary,
        &leak_context,
        question,
        history,
        focus_leak_id,
        config,
    );
    ai.wire.response =
        build_chat_toon_response(&ai.summary, &ai.recommendations, ai.confidence, config);
    ai
}

struct AiTaskContext<'a> {
    summary: &'a HeapSummary,
    leaks: &'a [LeakInsight],
    top_leak: Option<LeakInsight>,
    config: &'a AiConfig,
}

struct AiTaskOutput {
    summary_fragment: Option<String>,
    recommendations: Vec<String>,
    confidence_delta: f32,
}

fn run_rule_task(task: &AiTaskDefinition, context: &AiTaskContext<'_>) -> AiTaskOutput {
    match task.kind {
        AiTaskKind::TopLeak => run_top_leak_task(context),
        AiTaskKind::HealthyHeap => run_healthy_heap_task(context),
        AiTaskKind::RemediationChecklist => run_remediation_task(context),
    }
}

fn run_top_leak_task(context: &AiTaskContext<'_>) -> AiTaskOutput {
    let Some(leak) = &context.top_leak else {
        return AiTaskOutput {
            summary_fragment: None,
            recommendations: Vec::new(),
            confidence_delta: 0.0,
        };
    };

    AiTaskOutput {
        summary_fragment: Some(format!(
            "{class} is retaining ~{size:.2} MB via {instances} instances; prioritize freeing it to reclaim {percent:.1}% of the heap.",
            class = leak.class_name,
            size = bytes_to_mb(leak.retained_size_bytes),
            instances = leak.instances,
            percent = retained_percent(leak.retained_size_bytes, context.summary.total_size_bytes),
        )),
        recommendations: Vec::new(),
        confidence_delta: 0.12,
    }
}

fn run_healthy_heap_task(context: &AiTaskContext<'_>) -> AiTaskOutput {
    if context.top_leak.is_some() {
        return AiTaskOutput {
            summary_fragment: None,
            recommendations: Vec::new(),
            confidence_delta: 0.0,
        };
    }

    AiTaskOutput {
        summary_fragment: Some(format!(
            "Heap `{}` looks healthy; continue monitoring but no blockers were detected.",
            context.summary.heap_path
        )),
        recommendations: vec![
            "Capture a heap dump under load to validate steady-state behavior.".into(),
        ],
        confidence_delta: 0.05,
    }
}

fn run_remediation_task(context: &AiTaskContext<'_>) -> AiTaskOutput {
    if let Some(leak) = &context.top_leak {
        let mut recommendations = vec![format!(
            "Guard {} lifetimes: ensure cleanup hooks dispose unused entries.",
            leak.class_name
        )];
        recommendations.push(
            "Add targeted instrumentation (counters, timers) around the suspected allocation sites."
                .into(),
        );
        if leak.severity >= crate::analysis::LeakSeverity::High {
            recommendations.push(
                "Review threading / coroutine lifecycles anchoring these objects to a GC root."
                    .into(),
            );
        }

        return AiTaskOutput {
            summary_fragment: None,
            recommendations,
            confidence_delta: 0.08,
        };
    }

    AiTaskOutput {
        summary_fragment: None,
        recommendations: vec![
            "Capture a heap dump under load to validate steady-state behavior.".into(),
        ],
        confidence_delta: 0.03,
    }
}

fn default_rule_summary(context: &AiTaskContext<'_>) -> String {
    if context.top_leak.is_some() {
        format!(
            "Heap `{}` shows {} leak candidates, but no enabled task claimed primary ownership of the summary.",
            context.summary.heap_path,
            context.leaks.len()
        )
    } else {
        format!(
            "Heap `{}` looks stable under the current rule set.",
            context.summary.heap_path
        )
    }
}

fn default_rule_recommendation(context: &AiTaskContext<'_>) -> String {
    if context.top_leak.is_some() {
        format!(
            "Review the enabled AI task set for model `{}` to include a remediation-oriented rule.",
            context.config.model
        )
    } else {
        "Capture another heap sample under representative load before changing the AI task set."
            .into()
    }
}

fn build_provider_toon_prompt(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> CoreResult<String> {
    let mut body = build_request_section(summary, leaks, "explain_leak", config);
    let instructions = render_provider_instructions(
        config,
        &ProviderPromptContext {
            model: &config.model,
            provider: &config.provider.to_string(),
            heap_path: &summary.heap_path,
            total_bytes: summary.total_size_bytes,
            total_objects: summary.total_objects,
            leak_sampled: leaks.len(),
        },
    )?;
    body.push_str("section instructions\n");
    for (key, value) in instructions {
        push_kv(&mut body, 2, &key, value);
    }
    Ok(body)
}

fn build_provider_chat_toon_prompt(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> CoreResult<String> {
    let leak_context = focus_leaks(leaks, focus_leak_id);
    let mut body = build_chat_toon_prompt(
        summary,
        &leak_context,
        question,
        history,
        focus_leak_id,
        config,
    );
    let instructions = render_provider_instructions(
        config,
        &ProviderPromptContext {
            model: &config.model,
            provider: &config.provider.to_string(),
            heap_path: &summary.heap_path,
            total_bytes: summary.total_size_bytes,
            total_objects: summary.total_objects,
            leak_sampled: leak_context.len(),
        },
    )?;
    body.push_str("section instructions\n");
    for (key, value) in instructions {
        push_kv(&mut body, 2, &key, value);
    }
    Ok(body)
}

fn build_request_section(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    intent: &str,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    push_kv(&mut body, 2, "intent", intent);
    push_kv(&mut body, 2, "heap_path", &summary.heap_path);
    push_kv(&mut body, 2, "total_bytes", summary.total_size_bytes);
    push_kv(&mut body, 2, "total_objects", summary.total_objects);
    push_kv(&mut body, 2, "leak_sampled", leaks.len());

    body.push_str("section leaks\n");
    if leaks.is_empty() {
        push_kv(&mut body, 2, "status", "empty");
        return body;
    }

    let leak_limit = provider_leak_limit(config);
    if leak_limit < leaks.len().min(3) {
        push_kv(&mut body, 2, "context_truncated", "true");
    }

    for (idx, leak) in leaks.iter().enumerate().take(leak_limit) {
        body.push_str(&format!("  leak#{idx}\n"));
        push_kv(&mut body, 4, "id", &leak.id);
        push_kv(&mut body, 4, "class", &leak.class_name);
        push_kv(&mut body, 4, "kind", format!("{:?}", leak.leak_kind));
        push_kv(&mut body, 4, "severity", format!("{:?}", leak.severity));
        push_kv(
            &mut body,
            4,
            "retained_mb",
            format!("{:.2}", bytes_to_mb(leak.retained_size_bytes)),
        );
        push_kv(&mut body, 4, "instances", leak.instances);
        push_kv(
            &mut body,
            4,
            "description",
            truncate_prompt_description(&leak.description, config),
        );
    }

    body
}

fn provider_leak_limit(config: &AiConfig) -> usize {
    match config.max_tokens {
        Some(limit) if limit <= 256 => 1,
        _ => 3,
    }
}

fn build_chat_toon_prompt(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    question: &str,
    history: &[AiChatTurn],
    focus_leak_id: Option<&str>,
    config: &AiConfig,
) -> String {
    let leak_context = focus_leaks(leaks, focus_leak_id);
    let mut body = build_request_section(summary, &leak_context, "chat_leak_follow_up", config);
    body.push_str("section chat\n");
    push_kv(&mut body, 2, "question", question);
    if let Some(focus) = focus_leak_id {
        push_kv(&mut body, 2, "active_leak_id", focus);
    }
    push_chat_history(&mut body, history, config);
    body
}

fn push_chat_history(buf: &mut String, history: &[AiChatTurn], config: &AiConfig) {
    let keep = match config.max_tokens {
        Some(limit) if limit <= 256 => 0,
        _ => history.len().min(3),
    };
    if keep == 0 {
        return;
    }

    buf.push_str("section conversation\n");
    for (idx, turn) in history[history.len() - keep..].iter().enumerate() {
        buf.push_str(&format!("  turn#{idx}\n"));
        push_kv(buf, 4, "question", &turn.question);
        push_kv(buf, 4, "answer_summary", &turn.answer_summary);
    }
}

fn build_chat_toon_response(
    summary_text: &str,
    recommendations: &[String],
    confidence: f32,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section response\n");
    push_kv(&mut body, 2, "model", &config.model);
    push_kv(
        &mut body,
        2,
        "confidence_pct",
        format!("{:.0}", confidence * 100.0),
    );
    push_kv(&mut body, 2, "summary", summary_text);
    body.push_str("section recommendations\n");
    for (idx, item) in recommendations.iter().enumerate() {
        push_kv(&mut body, 2, &format!("item#{idx}"), item);
    }
    body
}

fn truncate_prompt_description(description: &str, config: &AiConfig) -> String {
    let max_chars = match config.max_tokens {
        Some(limit) if limit <= 256 => 240,
        _ => 1200,
    };

    if description.chars().count() <= max_chars {
        return description.to_string();
    }

    let truncated: String = description
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect();
    format!("{truncated}...")
}

fn parse_provider_toon_response(
    config: &AiConfig,
    prompt: String,
    response: String,
) -> CoreResult<AiInsights> {
    if !response.starts_with("TOON v1") {
        return Err(CoreError::InvalidInput(
            "provider returned malformed TOON output".into(),
        ));
    }

    let mut model = None;
    let mut confidence = None;
    let mut summary = None;
    let mut recommendations = Vec::new();
    let mut section = "";

    for line in response.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("section ") {
            section = rest;
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = unescape_toon_value(value);
        match (section, key) {
            ("response", "model") => model = Some(value),
            ("response", "confidence_pct") => {
                let parsed = value.parse::<f32>().map_err(|_| {
                    CoreError::InvalidInput("provider returned invalid confidence_pct".into())
                })?;
                confidence = Some((parsed / 100.0).clamp(0.0, 1.0));
            }
            ("response", "summary") => summary = Some(value),
            ("recommendations", key) if key.starts_with("item#") => recommendations.push(value),
            _ => {}
        }
    }

    let model = model.unwrap_or_else(|| config.model.clone());
    let summary = summary.ok_or_else(|| {
        CoreError::InvalidInput("provider TOON output missing response summary".into())
    })?;
    let confidence = confidence.unwrap_or(0.5);

    Ok(AiInsights {
        model,
        summary,
        recommendations,
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt,
            response,
        },
    })
}

fn build_toon_prompt(summary: &HeapSummary, leaks: &[LeakInsight]) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    push_kv(&mut body, 2, "intent", "explain_leak");
    push_kv(&mut body, 2, "heap_path", &summary.heap_path);
    push_kv(&mut body, 2, "total_bytes", summary.total_size_bytes);
    push_kv(&mut body, 2, "total_objects", summary.total_objects);
    push_kv(&mut body, 2, "leak_sampled", leaks.len());

    body.push_str("section leaks\n");
    if leaks.is_empty() {
        push_kv(&mut body, 2, "status", "empty");
    } else {
        for (idx, leak) in leaks.iter().enumerate().take(3) {
            body.push_str(&format!("  leak#{idx}\n"));
            push_kv(&mut body, 4, "id", &leak.id);
            push_kv(&mut body, 4, "class", &leak.class_name);
            push_kv(&mut body, 4, "kind", format!("{:?}", leak.leak_kind));
            push_kv(&mut body, 4, "severity", format!("{:?}", leak.severity));
            push_kv(
                &mut body,
                4,
                "retained_mb",
                format!("{:.2}", bytes_to_mb(leak.retained_size_bytes)),
            );
            push_kv(&mut body, 4, "instances", leak.instances);
            push_kv(&mut body, 4, "description", &leak.description);
        }
    }

    body
}

fn build_toon_response(
    summary: &HeapSummary,
    top: &Option<LeakInsight>,
    confidence: f32,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section response\n");
    push_kv(&mut body, 2, "model", &config.model);
    push_kv(
        &mut body,
        2,
        "confidence_pct",
        format!("{:.0}", confidence * 100.0),
    );

    match top {
        Some(leak) => {
            push_kv(
                &mut body,
                2,
                "summary",
                format!(
                    "{class} retains ~{size:.2} MB via {instances} instances (severity {severity:?}).",
                    class = leak.class_name,
                    size = bytes_to_mb(leak.retained_size_bytes),
                    instances = leak.instances,
                    severity = leak.severity
                ),
            );
            body.push_str("section remediation\n");
            push_kv(&mut body, 2, "priority", "high");
            push_kv(
                &mut body,
                2,
                "retained_percent",
                format!(
                    "{:.1}",
                    retained_percent(leak.retained_size_bytes, summary.total_size_bytes)
                ),
            );
        }
        None => {
            push_kv(
                &mut body,
                2,
                "summary",
                format!("Heap `{}` currently looks healthy.", summary.heap_path),
            );
            body.push_str("section remediation\n");
            push_kv(&mut body, 2, "priority", "observe");
        }
    }

    body
}

fn push_kv<T: std::fmt::Display>(buf: &mut String, indent: usize, key: &str, value: T) {
    for _ in 0..indent {
        buf.push(' ');
    }
    let raw = value.to_string();
    let _ = writeln!(buf, "{}={}", key, escape_toon_value(&raw));
}

fn unescape_toon_value(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn retained_percent(retained: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (retained as f64 / total as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{LeakKind, LeakSeverity};
    use crate::config::{AiMode, AiProvider, AiTaskDefinition, AiTaskKind};
    use crate::hprof::HeapSummary;
    use std::time::SystemTime;

    fn sample_chat_summary() -> HeapSummary {
        HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 128,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::UNIX_EPOCH,
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        }
    }

    fn sample_chat_leak() -> LeakInsight {
        LeakInsight {
            id: "com.example.CacheLeak::deadbeef".into(),
            class_name: "com.example.CacheLeak".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "Cache entries stay reachable from a singleton owner.".into(),
            provenance: Vec::new(),
        }
    }

    #[test]
    fn generates_summary_with_leak() {
        let summary = HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leak = LeakInsight {
            id: "com.example.Leak::deadbeef".into(),
            class_name: "com.example.Leak".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "Half the heap".into(),
            provenance: Vec::new(),
        };
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[leak], &config).unwrap();
        assert!(insights.summary.contains("com.example.Leak"));
        assert!(insights.recommendations.len() >= 2);
        assert!(insights.confidence > 0.5);
        assert!(insights.wire.prompt.starts_with("TOON v1"));
        assert!(insights.wire.response.contains("section response"));
    }

    #[test]
    fn handles_empty_leaks() {
        let summary = HeapSummary::placeholder("heap");
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[], &config).unwrap();
        assert!(insights.summary.contains("looks healthy"));
        assert!(!insights.recommendations.is_empty());
        assert_eq!(insights.wire.format, AiWireFormat::Toon);
    }

    #[test]
    fn generates_summary_with_configured_tasks() {
        let summary = HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leak = LeakInsight {
            id: "com.example.Leak::deadbeef".into(),
            class_name: "com.example.Leak".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "Half the heap".into(),
            provenance: Vec::new(),
        };
        let config = AiConfig {
            mode: AiMode::Rules,
            tasks: vec![
                AiTaskDefinition {
                    kind: AiTaskKind::TopLeak,
                    enabled: true,
                },
                AiTaskDefinition {
                    kind: AiTaskKind::RemediationChecklist,
                    enabled: true,
                },
            ],
            ..AiConfig::default()
        };

        let insights = generate_ai_insights(&summary, &[leak], &config).unwrap();
        assert!(insights.summary.contains("com.example.Leak"));
        assert!(insights
            .recommendations
            .iter()
            .any(|item| item.contains("cleanup") || item.contains("instrumentation")));
    }

    #[test]
    fn disabled_top_leak_task_changes_summary() {
        let summary = HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leak = LeakInsight {
            id: "com.example.Leak::deadbeef".into(),
            class_name: "com.example.Leak".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "Half the heap".into(),
            provenance: Vec::new(),
        };
        let config = AiConfig {
            mode: AiMode::Rules,
            tasks: vec![AiTaskDefinition {
                kind: AiTaskKind::TopLeak,
                enabled: false,
            }],
            ..AiConfig::default()
        };

        let insights = generate_ai_insights(&summary, &[leak], &config).unwrap();
        assert!(!insights.summary.contains("prioritize freeing"));
    }

    #[test]
    fn provider_mode_requires_api_key() {
        let summary = HeapSummary::placeholder("heap.hprof");
        let mut config = AiConfig::default();
        config.mode = AiMode::Provider;
        config.provider = crate::config::AiProvider::OpenAi;
        config.api_key_env = Some("MNEMOSYNE_TEST_MISSING_KEY".into());
        config.endpoint = Some("https://api.openai.com/v1".into());

        std::env::remove_var("MNEMOSYNE_TEST_MISSING_KEY");

        let err = generate_ai_insights(&summary, &[], &config).unwrap_err();
        assert!(err.to_string().contains("MNEMOSYNE_TEST_MISSING_KEY"));
    }

    #[test]
    fn provider_mode_parses_openai_compatible_toon_response() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "TOON v1\nsection response\n  model=test-provider\n  confidence_pct=81\n  summary=Provider-backed summary\nsection recommendations\n  item#0=Do the cleanup\n  item#1=Add instrumentation\n"
                    }
                }
            ]
        })
        .to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 4096];
            let _ = stream.read(&mut buf).unwrap();
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let summary = HeapSummary::placeholder("heap.hprof");
        let mut config = AiConfig::default();
        config.mode = AiMode::Provider;
        config.provider = crate::config::AiProvider::Local;
        config.endpoint = Some(format!("http://{addr}/v1"));
        config.api_key_env = Some("MNEMOSYNE_TEST_LOCAL_KEY".into());
        config.timeout_secs = 2;
        std::env::set_var("MNEMOSYNE_TEST_LOCAL_KEY", "dummy-key");

        let insights = generate_ai_insights(&summary, &[], &config).unwrap();
        assert_eq!(insights.model, "test-provider");
        assert_eq!(insights.summary, "Provider-backed summary");
        assert_eq!(
            insights.recommendations,
            vec!["Do the cleanup", "Add instrumentation"]
        );
        assert!((insights.confidence - 0.81).abs() < f32::EPSILON);
        assert_eq!(insights.wire.response, "TOON v1\nsection response\n  model=test-provider\n  confidence_pct=81\n  summary=Provider-backed summary\nsection recommendations\n  item#0=Do the cleanup\n  item#1=Add instrumentation\n");

        std::env::remove_var("MNEMOSYNE_TEST_LOCAL_KEY");
        server.join().unwrap();
    }

    #[test]
    fn provider_mode_parses_anthropic_toon_response() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": "TOON v1\nsection response\n  model=claude-test\n  confidence_pct=79\n  summary=Anthropic-backed summary\nsection recommendations\n  item#0=Trim retained cache state\n  item#1=Add request lifecycle instrumentation\n"
                }
            ]
        })
        .to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 4096];
            let read = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..read]).into_owned();
            assert!(request.contains("POST /v1/messages HTTP/1.1"), "{request}");
            assert!(request.contains("x-api-key: dummy-key"), "{request}");
            assert!(
                request.contains("anthropic-version: 2023-06-01"),
                "{request}"
            );

            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let summary = HeapSummary::placeholder("heap.hprof");
        let mut config = AiConfig::default();
        config.mode = AiMode::Provider;
        config.provider = crate::config::AiProvider::Anthropic;
        config.model = "claude-test".into();
        config.endpoint = Some(format!("http://{addr}/v1"));
        config.api_key_env = Some("MNEMOSYNE_TEST_ANTHROPIC_KEY".into());
        config.max_tokens = Some(512);
        config.timeout_secs = 2;
        std::env::set_var("MNEMOSYNE_TEST_ANTHROPIC_KEY", "dummy-key");

        let insights = generate_ai_insights(&summary, &[], &config).unwrap();
        assert_eq!(insights.model, "claude-test");
        assert_eq!(insights.summary, "Anthropic-backed summary");
        assert_eq!(
            insights.recommendations,
            vec![
                "Trim retained cache state",
                "Add request lifecycle instrumentation"
            ]
        );
        assert!((insights.confidence - 0.79).abs() < f32::EPSILON);

        std::env::remove_var("MNEMOSYNE_TEST_ANTHROPIC_KEY");
        server.join().unwrap();
    }

    #[test]
    fn provider_mode_redacts_prompt_before_send() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "TOON v1\nsection response\n  model=privacy-test-model\n  confidence_pct=80\n  summary=Redacted prompt accepted\nsection recommendations\n  item#0=Keep secrets out of provider prompts\n"
                    }
                }
            ]
        })
        .to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 8192];
            let read = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..read]).into_owned();
            assert!(request.contains("heap_path=<REDACTED>"), "{request}");
            assert!(request.contains("token=<REDACTED>"), "{request}");
            assert!(!request.contains("customer-42"), "{request}");
            assert!(!request.contains("secret-token-123"), "{request}");

            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let summary = HeapSummary {
            heap_path: "C:/captures/customer-42/heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leak = LeakInsight {
            id: "Leak::1".into(),
            class_name: "com.example.SecretCache".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "token=secret-token-123".into(),
            provenance: Vec::new(),
        };
        let config = AiConfig {
            mode: AiMode::Provider,
            provider: AiProvider::Local,
            model: "privacy-test-model".into(),
            endpoint: Some(format!("http://{addr}/v1")),
            timeout_secs: 2,
            privacy: crate::config::AiPrivacyConfig {
                redact_heap_path: true,
                redact_patterns: vec!["secret-token-[0-9]+".into(), "customer-[0-9]+".into()],
                ..Default::default()
            },
            ..AiConfig::default()
        };

        let insights = generate_ai_insights(&summary, &[leak], &config).unwrap();
        assert!(insights.wire.prompt.contains("heap_path=<REDACTED>"));
        assert!(insights.wire.prompt.contains("token=<REDACTED>"));
        assert!(!insights.wire.prompt.contains("customer-42"));
        assert!(!insights.wire.prompt.contains("secret-token-123"));

        server.join().unwrap();
    }

    #[test]
    fn provider_mode_truncates_leak_context_when_max_tokens_is_small() {
        let summary = HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leaks = vec![
            LeakInsight {
                id: "Leak::1".into(),
                class_name: "com.example.LeakOne".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::High,
                retained_size_bytes: 256 * 1024 * 1024,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 42,
                description: "A".repeat(800),
                provenance: Vec::new(),
            },
            LeakInsight {
                id: "Leak::2".into(),
                class_name: "com.example.LeakTwo".into(),
                leak_kind: LeakKind::Thread,
                severity: LeakSeverity::Medium,
                retained_size_bytes: 128 * 1024 * 1024,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 7,
                description: "B".repeat(800),
                provenance: Vec::new(),
            },
        ];
        let config = AiConfig {
            mode: AiMode::Provider,
            max_tokens: Some(1),
            ..AiConfig::default()
        };

        let prompt = build_provider_toon_prompt(&summary, &leaks, &config).unwrap();

        assert!(prompt.contains("section instructions"), "{prompt}");
        assert!(prompt.contains("response_format="), "{prompt}");
        assert!(prompt.contains("context_truncated=true"), "{prompt}");
        assert!(!prompt.contains("Leak::2"), "{prompt}");
    }

    #[test]
    fn provider_chat_prompt_includes_question_focus_and_recent_history() {
        let summary = sample_chat_summary();
        let leak = sample_chat_leak();
        let config = AiConfig {
            mode: AiMode::Provider,
            ..AiConfig::default()
        };
        let history = vec![AiChatTurn {
            question: "What should I inspect first?".into(),
            answer_summary: "Start with the singleton cache owner.".into(),
        }];

        let prompt = build_provider_chat_toon_prompt(
            &summary,
            &[leak],
            "Why is this leaking?",
            &history,
            Some("com.example.CacheLeak"),
            &config,
        )
        .unwrap();

        assert!(prompt.contains("intent=chat_leak_follow_up"));
        assert!(prompt.contains("active_leak_id=com.example.CacheLeak"));
        assert!(prompt.contains("question=Why is this leaking?"));
        assert!(prompt.contains("section conversation"));
        assert!(prompt.contains("What should I inspect first?"));
        assert!(prompt.contains("Start with the singleton cache owner."));
    }

    #[test]
    fn provider_chat_prompt_trims_history_before_selected_leak_context() {
        let summary = sample_chat_summary();
        let leak = sample_chat_leak();
        let config = AiConfig {
            mode: AiMode::Provider,
            max_tokens: Some(256),
            ..AiConfig::default()
        };
        let history = vec![
            AiChatTurn {
                question: "q1".into(),
                answer_summary: "a1".into(),
            },
            AiChatTurn {
                question: "q2".into(),
                answer_summary: "a2".into(),
            },
            AiChatTurn {
                question: "q3".into(),
                answer_summary: "a3".into(),
            },
            AiChatTurn {
                question: "q4".into(),
                answer_summary: "a4".into(),
            },
        ];

        let prompt = build_provider_chat_toon_prompt(
            &summary,
            &[leak],
            "What keeps this alive?",
            &history,
            Some("com.example.CacheLeak::deadbeef"),
            &config,
        )
        .unwrap();

        assert!(prompt.contains("question=What keeps this alive?"));
        assert!(prompt.contains("class=com.example.CacheLeak"));
        assert!(prompt.contains("active_leak_id=com.example.CacheLeak::deadbeef"));
        assert!(!prompt.contains("section conversation"));
        assert!(!prompt.contains("q1"));
        assert!(!prompt.contains("q2"));
        assert!(!prompt.contains("q3"));
        assert!(!prompt.contains("q4"));
        assert!(!prompt.contains("a1"));
        assert!(!prompt.contains("a2"));
        assert!(!prompt.contains("a3"));
        assert!(!prompt.contains("a4"));
    }

    #[test]
    fn rules_chat_focus_uses_selected_leak_for_answer() {
        let summary = sample_chat_summary();
        let leaks = vec![
            LeakInsight {
                id: "com.example.BigLeak::1".into(),
                class_name: "com.example.BigLeak".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::High,
                retained_size_bytes: 300 * 1024 * 1024,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 90,
                description: "Large unrelated leak.".into(),
                provenance: Vec::new(),
            },
            LeakInsight {
                id: "com.example.SmallLeak::2".into(),
                class_name: "com.example.SmallLeak".into(),
                leak_kind: LeakKind::Thread,
                severity: LeakSeverity::Medium,
                retained_size_bytes: 32 * 1024 * 1024,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 4,
                description: "Smaller focused leak.".into(),
                provenance: Vec::new(),
            },
        ];
        let config = AiConfig {
            mode: AiMode::Rules,
            ..AiConfig::default()
        };

        let insights = generate_ai_chat_turn(
            &summary,
            &leaks,
            "Why is this leaking?",
            &[],
            Some("com.example.SmallLeak::2"),
            &config,
        )
        .unwrap();

        assert!(insights.summary.contains("com.example.SmallLeak"));
        assert!(!insights.summary.contains("com.example.BigLeak"));
    }

    #[test]
    fn provider_mode_rejects_invalid_redaction_pattern() {
        let summary = HeapSummary::placeholder("heap.hprof");
        let config = AiConfig {
            mode: AiMode::Provider,
            provider: AiProvider::Local,
            endpoint: Some("http://127.0.0.1:9/v1".into()),
            privacy: crate::config::AiPrivacyConfig {
                redact_patterns: vec!["(".into()],
                ..Default::default()
            },
            ..AiConfig::default()
        };

        let err = generate_ai_insights(&summary, &[], &config).unwrap_err();
        assert!(err.to_string().contains("redact_patterns"));
    }

    #[test]
    fn focuses_on_matching_leak() {
        let leaks = vec![
            LeakInsight {
                id: "LeakA::1".into(),
                class_name: "LeakA".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::Low,
                retained_size_bytes: 1,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 1,
                description: String::new(),
                provenance: Vec::new(),
            },
            LeakInsight {
                id: "LeakB::2".into(),
                class_name: "LeakB".into(),
                leak_kind: LeakKind::Thread,
                severity: LeakSeverity::High,
                retained_size_bytes: 2,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 2,
                description: String::new(),
                provenance: Vec::new(),
            },
        ];

        let focused = focus_leaks(&leaks, Some("LeakB::2"));
        assert_eq!(focused.len(), 1);
        assert_eq!(focused[0].class_name, "LeakB");

        // Fallback to all leaks when no match.
        let fallback = focus_leaks(&leaks, Some("missing"));
        assert_eq!(fallback.len(), leaks.len());
    }

    #[test]
    fn validates_matching_leak_id() {
        let leaks = vec![LeakInsight {
            id: "LeakA::1".into(),
            class_name: "LeakA".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::Low,
            retained_size_bytes: 1,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 1,
            description: String::new(),
            provenance: Vec::new(),
        }];

        assert!(validate_leak_id(&leaks, "LeakA::1").is_ok());
        assert!(validate_leak_id(&leaks, "LeakA").is_ok());
    }

    #[test]
    fn rejects_unknown_leak_id() {
        let leaks = vec![LeakInsight {
            id: "LeakA::1".into(),
            class_name: "LeakA".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::Low,
            retained_size_bytes: 1,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 1,
            description: String::new(),
            provenance: Vec::new(),
        }];

        let err = validate_leak_id(&leaks, "missing").unwrap_err();
        assert!(err
            .to_string()
            .contains("no leak found matching identifier"));
    }
}
