use anyhow::{anyhow, Context, Result};
use dirs::config_dir;
use mnemosyne_core::{
    analysis::{LeakKind, LeakSeverity},
    AiConfig, AiProvider, AnalysisConfig, AppConfig, OutputFormat,
};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub data: AppConfig,
    pub origin: ConfigOrigin,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOrigin {
    Explicit,
    Environment,
    Project,
    User,
    System,
    Default,
}

impl ConfigOrigin {
    pub fn label(&self) -> &'static str {
        match self {
            ConfigOrigin::Explicit => "explicit",
            ConfigOrigin::Environment => "env",
            ConfigOrigin::Project => "project",
            ConfigOrigin::User => "user",
            ConfigOrigin::System => "system",
            ConfigOrigin::Default => "default",
        }
    }
}

struct ConfigCandidate {
    origin: ConfigOrigin,
    path: PathBuf,
}

pub fn load_app_config(cli_path: Option<&Path>) -> Result<LoadedConfig> {
    if let Some(path) = cli_path {
        return load_specific(path);
    }

    if let Ok(env_path) = env::var("MNEMOSYNE_CONFIG") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return load_from_path(Path::new(trimmed), ConfigOrigin::Environment);
        }
    }

    for candidate in discover_candidates() {
        if candidate.path.exists() {
            return load_candidate(candidate);
        }
    }

    let mut data = AppConfig::default();
    apply_env_overrides(&mut data);
    Ok(LoadedConfig {
        data,
        origin: ConfigOrigin::Default,
        path: None,
    })
}

fn load_specific(path: &Path) -> Result<LoadedConfig> {
    load_from_path(path, ConfigOrigin::Explicit)
}

fn load_from_path(path: &Path, origin: ConfigOrigin) -> Result<LoadedConfig> {
    if !path.exists() {
        return Err(anyhow!("config file '{}' does not exist", path.display()));
    }
    let candidate = ConfigCandidate {
        origin,
        path: path.to_path_buf(),
    };
    load_candidate(candidate)
}

fn load_candidate(candidate: ConfigCandidate) -> Result<LoadedConfig> {
    let mut data = parse_config(&candidate.path)?;
    apply_env_overrides(&mut data);
    Ok(LoadedConfig {
        data,
        origin: candidate.origin,
        path: Some(candidate.path),
    })
}

fn discover_candidates() -> Vec<ConfigCandidate> {
    let mut locations = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        let path = cwd.join(".mnemosyne.toml");
        locations.push(ConfigCandidate {
            origin: ConfigOrigin::Project,
            path,
        });
    }

    if let Some(mut dir) = config_dir() {
        dir.push("mnemosyne");
        dir.push("config.toml");
        locations.push(ConfigCandidate {
            origin: ConfigOrigin::User,
            path: dir,
        });
    }

    locations.push(ConfigCandidate {
        origin: ConfigOrigin::System,
        path: PathBuf::from("/etc/mnemosyne/config.toml"),
    });

    locations
}

fn parse_config(path: &Path) -> Result<AppConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file '{}':", path.display()))?;
    let file_cfg: FileConfig =
        toml::from_str(&raw).with_context(|| format!("invalid TOML in '{}':", path.display()))?;

    let mut cfg = AppConfig::default();
    apply_file_config(&mut cfg, file_cfg);
    Ok(cfg)
}

fn apply_file_config(cfg: &mut AppConfig, file: FileConfig) {
    if let Some(parser) = file.parser {
        if let Some(value) = parser.use_mmap {
            cfg.parser.use_mmap = value;
        }
        if let Some(value) = parser.threads {
            cfg.parser.threads = Some(value);
        }
        if let Some(value) = parser.max_objects {
            cfg.parser.max_objects = Some(value);
        }
    }

    if let Some(llm) = file.llm {
        apply_ai_section(&mut cfg.ai, llm);
    }

    if let Some(ai) = file.ai {
        apply_ai_section(&mut cfg.ai, ai);
    }

    if let Some(analysis) = file.analysis {
        apply_analysis_section(&mut cfg.analysis, analysis);
    }

    if let Some(output) = file.output {
        cfg.output = output;
    }

    if let Some(general) = file.general {
        if let Some(format) = general.output_format {
            cfg.output = format;
        }
        if let Some(enable_ai) = general.enable_ai {
            cfg.ai.enabled = enable_ai;
        }
    }
}

fn apply_ai_section(cfg: &mut AiConfig, section: PartialAiConfig) {
    if let Some(value) = section.enabled {
        cfg.enabled = value;
    }
    if let Some(value) = section.provider {
        cfg.provider = value;
    }
    if let Some(value) = section.model {
        cfg.model = value;
    }
    if let Some(value) = section.temperature {
        cfg.temperature = value;
    }
}

fn apply_analysis_section(cfg: &mut AnalysisConfig, section: PartialAnalysisConfig) {
    if let Some(value) = section.min_severity {
        cfg.min_severity = value;
    }
    if let Some(value) = section.packages {
        cfg.packages = value;
    }
    if let Some(value) = section.leak_types {
        cfg.leak_types = value;
    }
}

fn apply_env_overrides(cfg: &mut AppConfig) {
    if let Ok(value) = env::var("MNEMOSYNE_OUTPUT_FORMAT") {
        match value.parse::<OutputFormat>() {
            Ok(format) => cfg.output = format,
            Err(err) => warn!("Ignoring MNEMOSYNE_OUTPUT_FORMAT: {}", err),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_USE_MMAP") {
        if let Some(parsed) = parse_bool(&value) {
            cfg.parser.use_mmap = parsed;
        } else {
            warn!("Ignoring MNEMOSYNE_USE_MMAP: expected boolean");
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_THREADS") {
        match value.parse::<usize>() {
            Ok(num) => cfg.parser.threads = Some(num),
            Err(_) => warn!("Ignoring MNEMOSYNE_THREADS: expected integer"),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_MAX_OBJECTS") {
        match value.parse::<u64>() {
            Ok(num) => cfg.parser.max_objects = Some(num),
            Err(_) => warn!("Ignoring MNEMOSYNE_MAX_OBJECTS: expected integer"),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_AI_ENABLED") {
        if let Some(parsed) = parse_bool(&value) {
            cfg.ai.enabled = parsed;
        } else {
            warn!("Ignoring MNEMOSYNE_AI_ENABLED: expected boolean");
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_AI_PROVIDER") {
        match value.parse::<AiProvider>() {
            Ok(provider) => cfg.ai.provider = provider,
            Err(err) => warn!("Ignoring MNEMOSYNE_AI_PROVIDER: {}", err),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_AI_MODEL") {
        if !value.trim().is_empty() {
            cfg.ai.model = value;
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_AI_TEMPERATURE") {
        match value.parse::<f32>() {
            Ok(temp) => cfg.ai.temperature = temp,
            Err(_) => warn!("Ignoring MNEMOSYNE_AI_TEMPERATURE: expected float"),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_MIN_SEVERITY") {
        match value.parse::<LeakSeverity>() {
            Ok(sev) => cfg.analysis.min_severity = sev,
            Err(err) => warn!("Ignoring MNEMOSYNE_MIN_SEVERITY: {}", err),
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_PACKAGES") {
        let packages: Vec<String> = value
            .split(',')
            .map(|segment| segment.trim().to_string())
            .filter(|segment| !segment.is_empty())
            .collect();
        if !packages.is_empty() {
            cfg.analysis.packages = packages;
        }
    }

    if let Ok(value) = env::var("MNEMOSYNE_LEAK_TYPES") {
        let mut kinds = Vec::new();
        for item in value.split(',') {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.parse::<LeakKind>() {
                Ok(kind) => kinds.push(kind),
                Err(err) => warn!("Ignoring leak type '{}': {}", trimmed, err),
            }
        }
        if !kinds.is_empty() {
            cfg.analysis.leak_types = kinds;
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    general: Option<GeneralSection>,
    parser: Option<PartialParserConfig>,
    ai: Option<PartialAiConfig>,
    #[serde(rename = "llm")]
    llm: Option<PartialAiConfig>,
    analysis: Option<PartialAnalysisConfig>,
    output: Option<OutputFormat>,
}

#[derive(Debug, Default, Deserialize)]
struct GeneralSection {
    #[serde(rename = "output_format")]
    output_format: Option<OutputFormat>,
    enable_ai: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialParserConfig {
    use_mmap: Option<bool>,
    threads: Option<usize>,
    max_objects: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialAiConfig {
    enabled: Option<bool>,
    provider: Option<AiProvider>,
    model: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
struct PartialAnalysisConfig {
    min_severity: Option<LeakSeverity>,
    packages: Option<Vec<String>>,
    leak_types: Option<Vec<LeakKind>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_output_from_general_section() {
        let toml = r#"
[general]
output_format = "toon"
"#;
        let file_cfg: FileConfig = toml::from_str(toml).unwrap();
        let mut cfg = AppConfig::default();
        apply_file_config(&mut cfg, file_cfg);
        assert!(matches!(cfg.output, OutputFormat::Toon));
    }

    #[test]
    fn prefer_ai_section_over_llm() {
        let toml = r#"
[llm]
model = "llm-model"

[ai]
model = "ai-model"
"#;
        let file_cfg: FileConfig = toml::from_str(toml).unwrap();
        let mut cfg = AppConfig::default();
        apply_file_config(&mut cfg, file_cfg);
        assert_eq!(cfg.ai.model, "ai-model");
    }

    #[test]
    fn env_overrides_take_effect() {
        let mut cfg = AppConfig::default();
        env::set_var("MNEMOSYNE_OUTPUT_FORMAT", "toon");
        env::set_var("MNEMOSYNE_USE_MMAP", "false");
        env::set_var("MNEMOSYNE_AI_MODEL", "env-model");
        apply_env_overrides(&mut cfg);
        assert!(matches!(cfg.output, OutputFormat::Toon));
        assert!(!cfg.parser.use_mmap);
        assert_eq!(cfg.ai.model, "env-model");
        env::remove_var("MNEMOSYNE_OUTPUT_FORMAT");
        env::remove_var("MNEMOSYNE_USE_MMAP");
        env::remove_var("MNEMOSYNE_AI_MODEL");
    }

    #[test]
    fn analysis_section_updates_config() {
        let toml = r#"
[analysis]
min_severity = "CRITICAL"
packages = ["com.example", "org.demo"]
leak_types = ["CACHE", "THREAD"]
"#;
        let file_cfg: FileConfig = toml::from_str(toml).unwrap();
        let mut cfg = AppConfig::default();
        apply_file_config(&mut cfg, file_cfg);
        assert_eq!(cfg.analysis.min_severity, LeakSeverity::Critical);
        assert_eq!(cfg.analysis.packages, vec!["com.example", "org.demo"]);
        assert_eq!(
            cfg.analysis.leak_types,
            vec![LeakKind::Cache, LeakKind::Thread]
        );
    }

    #[test]
    fn env_overrides_analysis_settings() {
        let mut cfg = AppConfig::default();
        env::set_var("MNEMOSYNE_MIN_SEVERITY", "low");
        env::set_var("MNEMOSYNE_PACKAGES", "com.foo, org.bar");
        env::set_var("MNEMOSYNE_LEAK_TYPES", "cache,thread,unknown");
        apply_env_overrides(&mut cfg);
        assert_eq!(cfg.analysis.min_severity, LeakSeverity::Low);
        assert_eq!(cfg.analysis.packages, vec!["com.foo", "org.bar"]);
        assert_eq!(
            cfg.analysis.leak_types,
            vec![LeakKind::Cache, LeakKind::Thread, LeakKind::Unknown]
        );
        env::remove_var("MNEMOSYNE_MIN_SEVERITY");
        env::remove_var("MNEMOSYNE_PACKAGES");
        env::remove_var("MNEMOSYNE_LEAK_TYPES");
    }
}
