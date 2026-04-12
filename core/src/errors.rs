use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {path}")]
    FileNotFound {
        path: String,
        suggestion: Option<String>,
    },

    #[error("Not a valid HPROF file: {path}")]
    NotAnHprof { path: String, detail: String },

    #[error("HPROF parse error ({phase}): {detail}")]
    HprofParseError { phase: String, detail: String },

    #[error("Configuration error: {detail}")]
    ConfigError {
        detail: String,
        suggestion: Option<String>,
    },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("AI provider request failed: {detail}")]
    AiProviderError { detail: String, status: Option<u16> },

    #[error("AI provider request timed out: {detail}")]
    AiProviderTimeout { detail: String },

    #[error("Operation not yet implemented: {0}")]
    NotImplemented(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl CoreError {
    /// Returns the user-facing suggestion/hint if available.
    pub fn suggestion(&self) -> Option<&str> {
        match self {
            CoreError::FileNotFound { suggestion, .. } => suggestion.as_deref(),
            CoreError::ConfigError { suggestion, .. } => suggestion.as_deref(),
            _ => None,
        }
    }
}
