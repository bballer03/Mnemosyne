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
pub mod llm;
pub mod mapper;
pub mod mcp;
pub mod prompts;
pub mod query;
pub mod report;

pub use analysis::{
    focus_leaks, generate_ai_insights, generate_ai_insights_async, AiInsights, AiWireExchange,
    AiWireFormat, AnalysisMode, AnalyzeRequest, AnalyzeResponse, LeakDetectionOptions, LeakSuspect,
    ProvenanceKind, ProvenanceMarker, OVERVIEW_AUTO_THRESHOLD_BYTES,
};
pub use config::{
    AiConfig, AiMode, AiPromptConfig, AiProvider, AiSessionConfig, AiTaskDefinition, AiTaskKind,
    AnalysisConfig, AnalysisProfile, AppConfig, OutputFormat, ParserConfig,
};
pub use errors::{CoreError, CoreResult};
pub use fix::{
    propose_fix, propose_fix_with_config, FixRequest, FixResponse, FixStyle, FixSuggestion,
};
pub use graph::{
    build_dominator_tree, build_histogram, find_gc_path, find_unreachable_objects, DominatorNode,
    DominatorTree, GcPathNode, GcPathRequest, GcPathResult, GraphMetrics, HistogramEntry,
    HistogramGroupBy, HistogramResult, UnreachableClassEntry, UnreachableSet, VIRTUAL_ROOT_ID,
};
pub use hprof::{
    parse_heap, parse_hprof, parse_hprof_file, parse_hprof_file_with_options,
    parse_hprof_with_options, ClassLevelDelta, GcRootKind, HeapDiff, HeapParseJob, HeapSummary,
    HprofHeader, OverviewClassStat, OverviewClassStats, OverviewInstanceStat, OverviewOptions,
    OverviewSummary, OverviewThreadFrame, ParseOptions, DEFAULT_MAX_CLASS_TABLE_SIZE,
    DEFAULT_MAX_THREAD_FRAMES, DEFAULT_TOP_N_CLASSES, DEFAULT_TOP_N_INSTANCES,
};
pub use mapper::{CodeLocation, GitMetadata, MapToCodeRequest, SourceMapResult};
pub use report::{ReportArtifact, ReportRequest};

#[cfg(any(test, feature = "test-fixtures"))]
pub use hprof::test_fixtures;
