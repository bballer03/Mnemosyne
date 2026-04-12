use crate::{
    config::{AiConfig, AiProvider},
    CoreError, CoreResult,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};

#[derive(Debug, Clone)]
pub struct LlmCompletionRequest {
    pub prompt: String,
    pub config: AiConfig,
}

#[derive(Debug, Clone)]
pub struct LlmCompletionResponse {
    pub text: String,
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    messages: Vec<OpenAiMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicMessageRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

pub fn complete(request: &LlmCompletionRequest) -> CoreResult<LlmCompletionResponse> {
    match request.config.provider {
        AiProvider::OpenAi | AiProvider::Local => complete_openai_compatible(request),
        AiProvider::Anthropic => complete_anthropic(request),
    }
}

fn map_provider_error(err: reqwest::Error) -> CoreError {
    if err.is_timeout() {
        return CoreError::AiProviderTimeout {
            detail: err.to_string(),
        };
    }

    CoreError::AiProviderError {
        detail: err.to_string(),
        status: err.status().map(|status| status.as_u16()),
    }
}

fn complete_openai_compatible(request: &LlmCompletionRequest) -> CoreResult<LlmCompletionResponse> {
    let endpoint = resolved_endpoint(&request.config)?;
    let api_key = resolved_api_key(&request.config)?;
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(request.config.timeout_secs))
        .build()
        .map_err(|err| CoreError::AiProviderError {
            detail: err.to_string(),
            status: None,
        })?;

    let body = OpenAiChatRequest {
        model: request.config.model.clone(),
        temperature: request.config.temperature,
        max_tokens: request.config.max_tokens,
        messages: vec![OpenAiMessage {
            role: "user",
            content: request.prompt.clone(),
        }],
    };

    let mut builder = client.post(url).json(&body);
    if let Some(key) = api_key {
        builder = builder.bearer_auth(key);
    }

    let response = builder.send().map_err(map_provider_error)?;
    let response = response.error_for_status().map_err(map_provider_error)?;
    let payload: OpenAiChatResponse = response.json().map_err(map_provider_error)?;
    let text = payload
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| CoreError::InvalidInput("provider returned no completion content".into()))?;

    Ok(LlmCompletionResponse { text })
}

fn complete_anthropic(request: &LlmCompletionRequest) -> CoreResult<LlmCompletionResponse> {
    let endpoint = resolved_endpoint(&request.config)?;
    let api_key = resolved_api_key(&request.config)?
        .ok_or_else(|| CoreError::InvalidInput("Anthropic provider requires an API key".into()))?;
    let url = format!("{}/messages", endpoint.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(request.config.timeout_secs))
        .build()
        .map_err(|err| CoreError::AiProviderError {
            detail: err.to_string(),
            status: None,
        })?;

    let body = AnthropicMessageRequest {
        model: request.config.model.clone(),
        max_tokens: request.config.max_tokens.unwrap_or(2000),
        temperature: request.config.temperature,
        messages: vec![AnthropicMessage {
            role: "user",
            content: request.prompt.clone(),
        }],
    };

    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .map_err(map_provider_error)?;
    let response = response.error_for_status().map_err(map_provider_error)?;
    let payload: AnthropicMessageResponse = response.json().map_err(map_provider_error)?;
    let text = payload
        .content
        .into_iter()
        .filter_map(|block| (block.kind == "text").then_some(block.text).flatten())
        .collect::<Vec<_>>()
        .join("\n");

    if text.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "provider returned no completion content".into(),
        ));
    }

    Ok(LlmCompletionResponse { text })
}

fn resolved_endpoint(config: &AiConfig) -> CoreResult<String> {
    match config.provider {
        AiProvider::OpenAi => Ok(config
            .endpoint
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into())),
        AiProvider::Local => config.endpoint.clone().ok_or_else(|| {
            CoreError::InvalidInput(
                "local provider mode requires `ai.endpoint` to be configured".into(),
            )
        }),
        AiProvider::Anthropic => Ok(config
            .endpoint
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".into())),
    }
}

fn resolved_api_key(config: &AiConfig) -> CoreResult<Option<String>> {
    let env_name = config.api_key_env.as_deref().or(match config.provider {
        AiProvider::OpenAi => Some("OPENAI_API_KEY"),
        AiProvider::Anthropic => Some("ANTHROPIC_API_KEY"),
        AiProvider::Local => None,
    });

    let Some(env_name) = env_name else {
        return Ok(None);
    };

    match env::var(env_name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        _ => Err(CoreError::InvalidInput(format!(
            "missing AI provider API key in environment variable `{env_name}`"
        ))),
    }
}
