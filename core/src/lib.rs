//! Core library for the Mnemosyne JVM memory debugging toolkit.
//!
//! This crate hosts the domain logic shared between the CLI
//! application and future MCP / IDE integrations.

pub mod ai;
pub mod analysis;
pub mod config;
pub mod errors;
pub mod fix;
pub mod gc_path;
pub mod graph;
pub mod heap;
pub mod mapper;
pub mod mcp;
pub mod report;

pub use ai::{focus_leaks, generate_ai_insights, AiInsights, AiWireExchange, AiWireFormat};
pub use analysis::{AnalyzeRequest, AnalyzeResponse, LeakDetectionOptions};
pub use config::{AiConfig, AppConfig, OutputFormat, ParserConfig};
pub use errors::CoreResult;
pub use fix::{propose_fix, FixRequest, FixResponse, FixStyle, FixSuggestion};
pub use gc_path::{find_gc_path, GcPathNode, GcPathRequest, GcPathResult};
pub use graph::{DominatorNode, GraphMetrics};
pub use heap::{parse_heap, HeapDiff, HeapParseJob, HeapSummary, HprofHeader};
pub use mapper::{CodeLocation, GitMetadata, MapToCodeRequest, SourceMapResult};
pub use report::{ReportArtifact, ReportRequest};
