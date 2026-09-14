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
    /// Max retained chat turns for CLI/MCP sessions. Default 12; hard-capped at 32 (M19.F).
    pub history_max_turns: Option<usize>,
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
    /// A plugin-provided format, selected via `custom:<name>` (e.g.
    /// `--format custom:sarif`) and dispatched by
    /// `report::render_report_with_plugins` to whichever
    /// `ReportFormatterPlugin` in a `PluginRegistry` has a matching
    /// `format_name()` (M15 Slice 15.F). Additive: the five formats above
    /// are unchanged and `render_report`'s behavior for them is untouched.
    Custom(String),
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
        // "custom:<name>" uses an exact-case, lowercase "custom:" prefix --
        // unlike the built-in formats below, the plugin-supplied name after
        // the colon keeps its original case (not lowercased), since a
        // `ReportFormatterPlugin::format_name()` is matched by exact string
        // equality in `PluginRegistry::find_formatter`.
        if let Some(name) = s.strip_prefix("custom:") {
            return if name.is_empty() {
                Err(format!("unsupported output format '{s}'"))
            } else {
                Ok(OutputFormat::Custom(name.to_string()))
            };
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_from_str_existing_variants_unchanged() {
        assert!(matches!(
            "text".parse::<OutputFormat>(),
            Ok(OutputFormat::Text)
        ));
        assert!(matches!(
            "TOON".parse::<OutputFormat>(),
            Ok(OutputFormat::Toon)
        ));
        assert!(matches!(
            "md".parse::<OutputFormat>(),
            Ok(OutputFormat::Markdown)
        ));
        assert!(matches!(
            "html".parse::<OutputFormat>(),
            Ok(OutputFormat::Html)
        ));
        assert!(matches!(
            "json".parse::<OutputFormat>(),
            Ok(OutputFormat::Json)
        ));
        assert!("nonsense".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn output_format_from_str_parses_custom_plugin_format() {
        match "custom:sarif".parse::<OutputFormat>() {
            Ok(OutputFormat::Custom(name)) => assert_eq!(name, "sarif"),
            other => panic!("expected OutputFormat::Custom(\"sarif\"), got {other:?}"),
        }
    }

    #[test]
    fn output_format_from_str_preserves_custom_name_case() {
        match "custom:MixedCase-Name".parse::<OutputFormat>() {
            Ok(OutputFormat::Custom(name)) => assert_eq!(name, "MixedCase-Name"),
            other => panic!("expected OutputFormat::Custom(\"MixedCase-Name\"), got {other:?}"),
        }
    }

    #[test]
    fn output_format_from_str_rejects_empty_custom_name() {
        assert!("custom:".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn output_format_serde_round_trip_for_custom_variant() {
        let format = OutputFormat::Custom("sarif".to_string());
        let json = serde_json::to_string(&format).expect("should serialize");
        let round_tripped: OutputFormat = serde_json::from_str(&json).expect("should deserialize");
        assert!(matches!(round_tripped, OutputFormat::Custom(name) if name == "sarif"));
    }

    #[test]
    fn output_format_default_is_unaffected_by_custom_variant() {
        assert!(matches!(OutputFormat::default(), OutputFormat::Text));
    }
}
