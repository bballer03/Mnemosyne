use crate::{config::AiConfig, CoreError, CoreResult};
use serde::Deserialize;
use std::{fs, path::PathBuf};

const DEFAULT_PROVIDER_INSIGHTS_TEMPLATE: &str = include_str!("defaults/provider-insights.yaml");

#[derive(Debug, Clone)]
pub struct ProviderPromptContext<'a> {
    pub model: &'a str,
    pub provider: &'a str,
    pub heap_path: &'a str,
    pub total_bytes: u64,
    pub total_objects: u64,
    pub leak_sampled: usize,
}

#[derive(Debug, Deserialize)]
struct PromptTemplateFile {
    version: u32,
    instructions: Vec<PromptInstruction>,
}

#[derive(Debug, Deserialize)]
struct PromptInstruction {
    key: String,
    value: String,
}

pub fn render_provider_instructions(
    config: &AiConfig,
    context: &ProviderPromptContext<'_>,
) -> CoreResult<Vec<(String, String)>> {
    let raw = load_provider_template(config)?;
    let parsed: PromptTemplateFile =
        serde_yaml::from_str(&raw).map_err(|err| CoreError::ConfigError {
            detail: format!("invalid YAML in provider prompt template: {err}"),
            suggestion: Some(
                "Fix the YAML syntax in provider-insights.yaml or remove the override directory."
                    .into(),
            ),
        })?;

    if parsed.version != 1 {
        return Err(CoreError::ConfigError {
            detail: format!(
                "unsupported provider prompt template version '{}'",
                parsed.version
            ),
            suggestion: Some("Use a version 1 provider-insights.yaml template.".into()),
        });
    }

    Ok(parsed
        .instructions
        .into_iter()
        .map(|entry| (entry.key, render_value(&entry.value, context)))
        .collect())
}

fn load_provider_template(config: &AiConfig) -> CoreResult<String> {
    let Some(dir) = &config.prompts.template_dir else {
        return Ok(DEFAULT_PROVIDER_INSIGHTS_TEMPLATE.to_string());
    };

    let path = PathBuf::from(dir).join("provider-insights.yaml");
    fs::read_to_string(&path).map_err(|err| CoreError::ConfigError {
        detail: format!(
            "failed to read provider prompt template '{}': {err}",
            path.display()
        ),
        suggestion: Some(
            "Ensure [ai.prompts].template_dir points to a readable directory containing provider-insights.yaml."
                .into(),
        ),
    })
}

fn render_value(value: &str, context: &ProviderPromptContext<'_>) -> String {
    value
        .replace("{{model}}", context.model)
        .replace("{{provider}}", context.provider)
        .replace("{{heap_path}}", context.heap_path)
        .replace("{{total_bytes}}", &context.total_bytes.to_string())
        .replace("{{total_objects}}", &context.total_objects.to_string())
        .replace("{{leak_sampled}}", &context.leak_sampled.to_string())
}
