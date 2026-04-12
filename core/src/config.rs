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
    pub mode: AiMode,
    pub tasks: Vec<AiTaskDefinition>,
    pub privacy: AiPrivacyConfig,
    pub prompts: AiPromptConfig,
    pub sessions: AiSessionConfig,
    pub endpoint: Option<String>,
    pub api_key_env: Option<String>,
    pub max_tokens: Option<u32>,
    #[serde(default = "AiConfig::default_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiPrivacyConfig {
    pub redact_heap_path: bool,
    pub redact_patterns: Vec<String>,
    pub audit_log: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiPromptConfig {
    pub template_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiSessionConfig {
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AiMode {
    #[default]
    Rules,
    Stub,
    Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AiTaskDefinition {
    pub kind: AiTaskKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AiTaskKind {
    #[default]
    TopLeak,
    HealthyHeap,
    RemediationChecklist,
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
            mode: AiMode::Rules,
            tasks: vec![
                AiTaskDefinition {
                    kind: AiTaskKind::TopLeak,
                    enabled: true,
                },
                AiTaskDefinition {
                    kind: AiTaskKind::HealthyHeap,
                    enabled: true,
                },
                AiTaskDefinition {
                    kind: AiTaskKind::RemediationChecklist,
                    enabled: true,
                },
            ],
            privacy: AiPrivacyConfig::default(),
            prompts: AiPromptConfig::default(),
            sessions: AiSessionConfig::default(),
            endpoint: None,
            api_key_env: None,
            max_tokens: None,
            timeout_secs: 30,
        }
    }
}

impl Default for AiTaskDefinition {
    fn default() -> Self {
        Self {
            kind: AiTaskKind::TopLeak,
            enabled: true,
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

impl AiConfig {
    fn default_timeout_secs() -> u64 {
        30
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

impl FromStr for AiMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "rules" => Ok(AiMode::Rules),
            "stub" => Ok(AiMode::Stub),
            "provider" => Ok(AiMode::Provider),
            other => Err(format!("unsupported AI mode '{other}'")),
        }
    }
}

impl FromStr for AiTaskKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "top-leak" | "top_leak" => Ok(AiTaskKind::TopLeak),
            "healthy-heap" | "healthy_heap" => Ok(AiTaskKind::HealthyHeap),
            "remediation-checklist" | "remediation_checklist" => {
                Ok(AiTaskKind::RemediationChecklist)
            }
            other => Err(format!("unsupported AI task kind '{other}'")),
        }
    }
}

impl std::fmt::Display for AiMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            AiMode::Rules => "rules",
            AiMode::Stub => "stub",
            AiMode::Provider => "provider",
        };
        f.write_str(text)
    }
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            AiProvider::OpenAi => "openai",
            AiProvider::Anthropic => "anthropic",
            AiProvider::Local => "local",
        };
        f.write_str(text)
    }
}

impl std::fmt::Display for AiTaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            AiTaskKind::TopLeak => "top-leak",
            AiTaskKind::HealthyHeap => "healthy-heap",
            AiTaskKind::RemediationChecklist => "remediation-checklist",
        };
        f.write_str(text)
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
