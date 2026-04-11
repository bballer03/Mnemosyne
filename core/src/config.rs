use crate::analysis::{LeakKind, LeakSeverity};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisProfile {
    #[default]
    Overview,
    IncidentResponse,
    CiRegression,
}

/// Root configuration shared across Mnemosyne surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub parser: ParserConfig,
    pub ai: AiConfig,
    pub analysis: AnalysisConfig,
    pub output: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ParserConfig {
    #[serde(default = "ParserConfig::default_use_mmap")]
    pub use_mmap: bool,
    pub threads: Option<usize>,
    pub max_objects: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: AiProvider,
    pub model: String,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisConfig {
    pub min_severity: LeakSeverity,
    pub packages: Vec<String>,
    pub leak_types: Vec<LeakKind>,
    pub accumulation_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    OpenAi,
    Anthropic,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Toon,
    Markdown,
    Html,
    Json,
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

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            min_severity: LeakSeverity::High,
            packages: Vec::new(),
            leak_types: Vec::new(),
            accumulation_threshold: 10.0,
        }
    }
}

impl ParserConfig {
    fn default_use_mmap() -> bool {
        true
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "toon" => Ok(OutputFormat::Toon),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "html" => Ok(OutputFormat::Html),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!("unsupported output format '{other}'")),
        }
    }
}

impl FromStr for AiProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "openai" => Ok(AiProvider::OpenAi),
            "anthropic" => Ok(AiProvider::Anthropic),
            "local" => Ok(AiProvider::Local),
            other => Err(format!("unsupported AI provider '{other}'")),
        }
    }
}

impl FromStr for AnalysisProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "overview" => Ok(AnalysisProfile::Overview),
            "incident-response" | "incident_response" => Ok(AnalysisProfile::IncidentResponse),
            "ci-regression" | "ci_regression" => Ok(AnalysisProfile::CiRegression),
            other => Err(format!("unsupported analysis profile '{other}'")),
        }
    }
}
