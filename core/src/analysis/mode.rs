use serde::{Deserialize, Serialize};

/// Threshold (in bytes) at which `AnalysisMode::Auto` resolves to `Overview` instead of `Deep`.
/// Default 4 GiB. Override via `MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD` env var.
pub const OVERVIEW_AUTO_THRESHOLD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

const OVERVIEW_AUTO_THRESHOLD_ENV: &str = "MNEMOSYNE_OVERVIEW_AUTO_THRESHOLD";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisMode {
    /// File-size driven. Default. Picks Deep for small dumps and Overview for large ones.
    #[default]
    Auto,
    /// Build the full ObjectGraph and run all analyzers (v0.2.0 behavior).
    Deep,
    /// Streaming, bounded-memory triage. No graph, no retained sizes.
    Overview,
}

impl AnalysisMode {
    /// Resolves `Auto` to a concrete mode based on input size.
    /// `Deep` and `Overview` pass through unchanged.
    pub fn resolve(self, input_size_bytes: u64) -> AnalysisMode {
        match self {
            AnalysisMode::Auto => {
                if input_size_bytes >= overview_auto_threshold() {
                    AnalysisMode::Overview
                } else {
                    AnalysisMode::Deep
                }
            }
            mode => mode,
        }
    }
}

fn overview_auto_threshold() -> u64 {
    std::env::var(OVERVIEW_AUTO_THRESHOLD_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(OVERVIEW_AUTO_THRESHOLD_BYTES)
}

#[cfg(test)]
pub(crate) fn test_mode_env_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default_is_auto() {
        let _guard = test_mode_env_lock().blocking_lock();
        assert_eq!(AnalysisMode::default(), AnalysisMode::Auto);
    }

    #[test]
    fn mode_resolves_by_size() {
        let _guard = test_mode_env_lock().blocking_lock();
        let small = OVERVIEW_AUTO_THRESHOLD_BYTES - 1;
        let large = OVERVIEW_AUTO_THRESHOLD_BYTES + 1;
        assert_eq!(AnalysisMode::Auto.resolve(small), AnalysisMode::Deep);
        assert_eq!(AnalysisMode::Auto.resolve(large), AnalysisMode::Overview);
    }

    #[test]
    fn explicit_modes_pass_through() {
        let _guard = test_mode_env_lock().blocking_lock();
        assert_eq!(AnalysisMode::Deep.resolve(u64::MAX), AnalysisMode::Deep);
        assert_eq!(AnalysisMode::Overview.resolve(0), AnalysisMode::Overview);
    }

    #[test]
    fn mode_serializes_lowercase() {
        let json = serde_json::to_string(&AnalysisMode::Overview).unwrap();
        assert_eq!(json, "\"overview\"");
    }

    #[test]
    fn mode_deserializes_lowercase() {
        let mode: AnalysisMode = serde_json::from_str("\"deep\"").unwrap();
        assert_eq!(mode, AnalysisMode::Deep);
    }

    #[test]
    fn overview_threshold_uses_default_when_env_missing() {
        let _guard = test_mode_env_lock().blocking_lock();
        assert!(overview_auto_threshold() > 0);
    }
}
