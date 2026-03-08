//! Core library for the Mnemosyne JVM memory debugging toolkit.
//!
//! This crate hosts the domain logic shared between the CLI
//! application and future MCP / IDE integrations.

pub mod analysis;
pub mod config;
pub mod errors;
pub mod fix;
pub mod graph;
pub mod hprof;
pub mod mapper;
pub mod mcp;
pub mod report;

pub use analysis::{
    focus_leaks, generate_ai_insights, AiInsights, AiWireExchange, AiWireFormat, AnalyzeRequest,
    AnalyzeResponse, LeakDetectionOptions, LeakSuspect, ProvenanceKind, ProvenanceMarker,
};
pub use config::{AiConfig, AiProvider, AnalysisConfig, AppConfig, OutputFormat, ParserConfig};
pub use errors::{CoreError, CoreResult};
pub use fix::{propose_fix, FixRequest, FixResponse, FixStyle, FixSuggestion};
pub use graph::{
    build_dominator_tree, build_histogram, find_gc_path, find_unreachable_objects, DominatorNode,
    DominatorTree, GcPathNode, GcPathRequest, GcPathResult, GraphMetrics, HistogramEntry,
    HistogramGroupBy, HistogramResult, UnreachableClassEntry, UnreachableSet, VIRTUAL_ROOT_ID,
};
pub use hprof::{
    parse_heap, parse_hprof, ClassLevelDelta, HeapDiff, HeapParseJob, HeapSummary, HprofHeader,
};
pub use mapper::{CodeLocation, GitMetadata, MapToCodeRequest, SourceMapResult};
pub use report::{ReportArtifact, ReportRequest};

#[cfg(any(test, feature = "test-fixtures"))]
pub use hprof::test_fixtures;
