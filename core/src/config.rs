use serde::{Deserialize, Serialize};

/// Root configuration shared across Mnemosyne surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub parser: ParserConfig,
    pub ai: AiConfig,
    pub output: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    pub use_mmap: bool,
    pub threads: Option<usize>,
    pub max_objects: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: AiProvider,
    pub model: String,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AiProvider {
    #[default]
    OpenAi,
    Anthropic,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Toon,
    Markdown,
    Html,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            parser: ParserConfig::default(),
            ai: AiConfig::default(),
            output: OutputFormat::Text,
        }
    }
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            use_mmap: true,
            threads: None,
            max_objects: None,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProvider::OpenAi,
            model: "gpt-4.1-mini".into(),
            temperature: 0.2,
        }
    }
}
