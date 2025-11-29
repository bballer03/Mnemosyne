//! Core library for the Mnemosyne JVM memory debugging toolkit.
//!
//! This crate hosts the domain logic shared between the CLI
//! application and future MCP / IDE integrations.

pub mod analysis;
pub mod config;
pub mod errors;
pub mod heap;
pub mod mcp;
pub mod report;

pub use analysis::{AnalyzeRequest, AnalyzeResponse, LeakDetectionOptions};
pub use config::{AiConfig, AppConfig, OutputFormat, ParserConfig};
pub use errors::CoreResult;
pub use heap::{parse_heap, HeapDiff, HeapParseJob, HeapSummary, HprofHeader};
pub use report::{ReportArtifact, ReportRequest};
