//! Phase 2 static plugin/extension runtime (M15 Slice 15.F).
//!
//! See `docs/design/m6-plugin-extension-system.md` (original design sketch)
//! and `docs/design/milestone-15-mat-backend-parity.md` §3.3/§4.4 (the
//! binding decision that this milestone ships Phase 2 -- a static trait
//! registry -- and explicitly *not* Phase 3 dynamic `cdylib` loading).
//!
//! ## What this module is
//!
//! Two extension-point traits (`AnalyzerPlugin`, `ReportFormatterPlugin`)
//! plus a `PluginRegistry` that a binary (the CLI, MCP server, or any other
//! `mnemosyne-core` consumer) constructs at startup and populates with
//! whatever plugins it was compiled with. There is no filesystem discovery,
//! no `.so`/`.dylib`/`.dll` loading, and no config-driven plugin path list --
//! registration is a plain Rust function call
//! (`registry.register_analyzer(Box::new(MyAnalyzer))`), resolved entirely
//! at compile time. This mirrors §3.1 of the m6 design doc ("Static
//! (compile-time)") and is the *only* registration mechanism this milestone
//! ships; §3.2 (config-based `.so` paths) and §3.3 (directory-based
//! discovery) remain Phase 3, deliberately unimplemented.
//!
//! ## Deviations from the m6 doc's §2.1/§2.2 sketch
//!
//! The m6 doc predates several APIs this module actually touches. Two
//! concrete adjustments, both called out as acceptable in the M15 design
//! doc's §4.4 ("adjust only if implementation finds a genuine mismatch"):
//!
//! 1. `AnalyzerPlugin::analyze` takes `Option<&DominatorTree>` instead of
//!    the sketch's `&AnalysisConfig`. The milestone-15 doc's own §4.4 shape
//!    (`fn analyze(&self, graph: &ObjectGraph, dominator: Option<&DominatorTree>)`)
//!    supersedes the m6 sketch here -- retained-size/dominator-chain
//!    analysis is the whole point of a heap-analysis plugin, and the
//!    existing pipeline (`analyze_heap_internal`) already computes the
//!    dominator tree once and threads it through every built-in analyzer
//!    (`analyze_classloaders`, `inspect_threads`, ...); a plugin should get
//!    the same look-in, not a config struct it can already reach via the
//!    graph-backed call site if it needs config. The tree is `Option`
//!    because the pipeline's heuristic (summary-only) fallback path never
//!    has one -- see `analyze_heap_internal`'s `try_build_dominator` branch.
//! 2. Both trait methods return `CoreResult<T>` (`Result<T, CoreError>`)
//!    rather than the sketch's bare `AnalyzerResult`/`String`. The m6 doc's
//!    own code sample for `AnalyzerPlugin::analyze` already returns
//!    `Result<AnalyzerResult, CoreError>` (§2.1) -- this module keeps that
//!    half of the sketch as-is and applies the same `Result`-returning
//!    shape to `ReportFormatterPlugin::render` for consistency, since a
//!    formatter rendering an arbitrary `AnalyzeResponse` can fail for the
//!    same reasons the built-in `render_report` renderers can
//!    (`CoreError::Unsupported`, serialization errors, ...).
//!
//! Everything else (trait method names, `PluginRegistry` shape, "static
//! registration only") matches both design docs as written.
//!
//! ## Pipeline wiring
//!
//! `AnalyzerPlugin` results are surfaced via
//! [`crate::analysis::analyze_heap_with_plugins`], a new, fully additive
//! entry point alongside the existing `analyze_heap`/`analyze_heap_with_graph`
//! family in `core/src/analysis/engine.rs`. It runs the same graph-backed
//! pipeline those functions already run, then -- only when a non-empty
//! registry is supplied and graph-backed analysis succeeded -- appends each
//! registered analyzer's output to `AnalyzeResponse::plugin_results`. Callers
//! that keep using `analyze_heap`/`analyze_heap_with_graph`/
//! `analyze_heap_capturing_graph` never construct a registry, so
//! `plugin_results` stays an empty, omitted-from-JSON vec for them --
//! today's `analyze` output is unchanged byte-for-byte (see
//! `analyze_heap_back_compat_byte_identical` in `engine.rs`'s test module,
//! which is unmodified by this slice and still passes).
//!
//! `ReportFormatterPlugin` hooks into the existing `OutputFormat`/
//! `render_report` mechanism in `core/src/report/renderer.rs`: a new
//! `OutputFormat::Custom(String)` variant (additive -- the five existing
//! variants, their serde representation, and `render_report`'s behavior for
//! them are untouched) is dispatched by a new
//! [`crate::report::render_report_with_plugins`] wrapper that looks the
//! name up in a `PluginRegistry` and falls back to the plain
//! `render_report` for every non-custom format. `render_report` itself gains
//! one new match arm for `OutputFormat::Custom` that returns a clear
//! `CoreError::Unsupported` (a registry-unaware caller asking for a custom
//! format it has no way to satisfy), rather than silently doing nothing.
//!
//! ## Non-scope (Phase 3, explicitly not built here)
//!
//! No dynamic loading. No `cdylib`/`.so`/`.dylib`/`.dll` discovery. No
//! `~/.mnemosyne/plugins/` directory scanning. No `[plugins]` section in
//! `mnemosyne.toml`. No C-ABI entry points. No CLI `--plugin <path>` flag.
//! Per §3.3 of the M15 design doc, Phase 3 stays gated behind "Phase 2
//! adoption + demand for out-of-tree plugins" -- evidence that does not
//! exist yet.

use crate::{errors::CoreResult, graph::DominatorTree, hprof::ObjectGraph};
use serde::{Deserialize, Serialize};

/// A single finding produced by an [`AnalyzerPlugin`] pass.
///
/// Derives `Serialize`/`Deserialize` solely so [`AnalyzerResult`] (and, in
/// turn, `AnalyzeResponse::plugin_results`) can participate in
/// `AnalyzeResponse`'s existing `Serialize`/`Deserialize` derive -- plugin
/// authors do not need to implement serde themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerFinding {
    /// One-line human-readable summary of the finding.
    pub summary: String,
    /// Free-form severity label (e.g. `"info"`, `"warning"`, `"critical"`).
    /// Kept as a plain `String` (not a shared enum with the built-in
    /// `LeakSeverity`) per the m6 doc's §2.1 sketch: third-party plugins
    /// are not required to map onto Mnemosyne's own severity taxonomy.
    pub severity: String,
    /// Optional extended detail (multi-line explanation, reference chain,
    /// suggested remediation, ...).
    pub detail: Option<String>,
}

impl AnalyzerFinding {
    /// Convenience constructor for a finding with no extended detail.
    pub fn new(summary: impl Into<String>, severity: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            severity: severity.into(),
            detail: None,
        }
    }

    /// Convenience constructor for a finding that also carries detail text.
    pub fn with_detail(
        summary: impl Into<String>,
        severity: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            summary: summary.into(),
            severity: severity.into(),
            detail: Some(detail.into()),
        }
    }
}

/// Result returned by a single [`AnalyzerPlugin`] run.
///
/// This is the type `AnalyzeResponse::plugin_results` (a `Vec` of these, one
/// per registered analyzer) is made of; the M15 design doc's own field
/// sketch calls the vec-of-these shape `PluginAnalyzerResult` -- this crate
/// uses `AnalyzerResult` directly as that element type rather than adding a
/// redundant type alias, since it is also the exact return type
/// [`AnalyzerPlugin::analyze`] itself produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerResult {
    /// The plugin's own `name()`, copied in so a consumer of
    /// `AnalyzeResponse::plugin_results` doesn't need the original
    /// `Box<dyn AnalyzerPlugin>` to know which plugin produced which entry.
    pub name: String,
    pub findings: Vec<AnalyzerFinding>,
}

/// Extension point for a user-defined analysis pass over a parsed heap.
///
/// Implementations are registered into a [`PluginRegistry`] at process
/// startup (static, compile-time registration only -- see the module-level
/// docs). `analyze` receives the same `ObjectGraph` and, when graph-backed
/// analysis succeeded, the same `DominatorTree` that every built-in analyzer
/// in `core::analysis` already receives from
/// [`crate::analysis::analyze_heap_with_plugins`].
pub trait AnalyzerPlugin: Send + Sync {
    /// Human-readable name shown in reports and used to attribute
    /// `AnalyzeResponse::plugin_results` entries back to this plugin.
    fn name(&self) -> &str;

    /// Run the analysis pass over the object graph.
    ///
    /// `dominator` is `None` when the pipeline fell back to heuristic
    /// (summary-only) analysis -- e.g. HPROF parsing into a full object
    /// graph failed. A plugin that requires dominator-tree data should
    /// return an empty `AnalyzerResult` (or a finding noting the
    /// limitation) rather than panicking in that case.
    fn analyze(
        &self,
        graph: &ObjectGraph,
        dominator: Option<&DominatorTree>,
    ) -> CoreResult<AnalyzerResult>;
}

/// Extension point for a custom report output format.
///
/// Selected via `OutputFormat::Custom(name)` (see `core::config`) and
/// [`crate::report::render_report_with_plugins`], which looks the plugin up
/// in a [`PluginRegistry`] by [`format_name`](ReportFormatterPlugin::format_name).
pub trait ReportFormatterPlugin: Send + Sync {
    /// Format identifier matched against `OutputFormat::Custom(name)`.
    fn format_name(&self) -> &str;

    /// MIME type of the rendered output, copied into the resulting
    /// [`crate::report::ReportArtifact::mime_type`].
    fn mime_type(&self) -> &str;

    /// Render the analysis response into a string.
    fn render(&self, response: &crate::analysis::AnalyzeResponse) -> CoreResult<String>;
}

/// Static (compile-time) registry of [`AnalyzerPlugin`]s and
/// [`ReportFormatterPlugin`]s.
///
/// A binary constructs one of these at startup and registers whatever
/// plugins it was compiled with -- there is no discovery mechanism.
/// Mnemosyne's own CLI and MCP binaries ship with an **empty** registry
/// today (per the design doc's "zero built-in flagship plugins" directive
/// -- see §4.4); the only `AnalyzerPlugin`/`ReportFormatterPlugin`
/// implementations in this codebase are the test-only demos in this
/// module's `tests` submodule, which exist purely to prove the trait shape
/// works end-to-end.
#[derive(Default)]
pub struct PluginRegistry {
    analyzers: Vec<Box<dyn AnalyzerPlugin>>,
    formatters: Vec<Box<dyn ReportFormatterPlugin>>,
}

impl PluginRegistry {
    /// An empty registry. Equivalent to `PluginRegistry::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an analyzer plugin. Returns `&mut Self` for chaining, e.g.
    /// `PluginRegistry::new().register_analyzer(..).register_formatter(..)`.
    pub fn register_analyzer(&mut self, plugin: Box<dyn AnalyzerPlugin>) -> &mut Self {
        self.analyzers.push(plugin);
        self
    }

    /// Register a report formatter plugin. Returns `&mut Self` for chaining.
    pub fn register_formatter(&mut self, plugin: Box<dyn ReportFormatterPlugin>) -> &mut Self {
        self.formatters.push(plugin);
        self
    }

    /// `true` when no analyzers and no formatters are registered -- the
    /// state Mnemosyne's own binaries ship in today.
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty() && self.formatters.is_empty()
    }

    /// Number of registered analyzer plugins.
    pub fn analyzer_count(&self) -> usize {
        self.analyzers.len()
    }

    /// Number of registered formatter plugins.
    pub fn formatter_count(&self) -> usize {
        self.formatters.len()
    }

    /// Run every registered analyzer over `graph`/`dominator`, in
    /// registration order. A plugin whose `analyze` call returns `Err` is
    /// skipped (logged at `warn` level) rather than failing the whole
    /// pipeline -- one misbehaving third-party plugin should not take down
    /// `analyze_heap_with_plugins` for every other registered plugin or for
    /// the built-in analysis it wraps.
    pub fn run_analyzers(
        &self,
        graph: &ObjectGraph,
        dominator: Option<&DominatorTree>,
    ) -> Vec<AnalyzerResult> {
        self.analyzers
            .iter()
            .filter_map(|plugin| match plugin.analyze(graph, dominator) {
                Ok(result) => Some(result),
                Err(error) => {
                    tracing::warn!(
                        plugin = plugin.name(),
                        %error,
                        "plugin analyzer failed; skipping its output"
                    );
                    None
                }
            })
            .collect()
    }

    /// Find a registered formatter by [`format_name`](ReportFormatterPlugin::format_name).
    pub fn find_formatter(&self, name: &str) -> Option<&dyn ReportFormatterPlugin> {
        self.formatters
            .iter()
            .find(|formatter| formatter.format_name() == name)
            .map(|boxed| boxed.as_ref())
    }
}

/// Test-only fixtures shared by this module's own unit tests *and* by
/// `core::analysis::engine`'s integration-level pipeline tests (M15 Slice
/// 15.F's validation gate: "a test-only `AnalyzerPlugin` implementation
/// registers and its `analyze()` output appears in a full pipeline run").
/// Gated the same way `crate::hprof::test_fixtures` is (compiled for both
/// `cfg(test)` and the `test-fixtures` feature) so `engine.rs`'s
/// `#[cfg(test)] mod tests` can reach it via `crate::plugin::test_support`.
///
/// This holds the *one* test-only demo `AnalyzerPlugin` this slice adds
/// (per the design doc's "avoid inventing a flagship plugin nobody asked
/// for" -- see the module-level doc comment above). It is never
/// registered by any production binary in this workspace.
#[cfg(any(test, feature = "test-fixtures"))]
pub(crate) mod test_support {
    use super::{AnalyzerFinding, AnalyzerPlugin, AnalyzerResult, ReportFormatterPlugin};
    use crate::{
        analysis::AnalyzeResponse, errors::CoreResult, graph::DominatorTree, hprof::ObjectGraph,
    };

    /// The one test-only demo `AnalyzerPlugin` this slice ships. Not
    /// registered by the CLI, MCP server, or any other production binary --
    /// exists purely to prove the trait shape works end-to-end. Counts
    /// objects whose class name contains "leak" (case-insensitive) and
    /// emits one finding per match.
    pub struct DemoLeakNameAnalyzer;

    impl AnalyzerPlugin for DemoLeakNameAnalyzer {
        fn name(&self) -> &str {
            "demo-leak-name-analyzer"
        }

        fn analyze(
            &self,
            graph: &ObjectGraph,
            _dominator: Option<&DominatorTree>,
        ) -> CoreResult<AnalyzerResult> {
            let findings = graph
                .objects
                .values()
                .filter_map(|object| graph.class_name(object.class_id))
                .filter(|name| name.to_ascii_lowercase().contains("leak"))
                .map(|name| AnalyzerFinding::new(format!("suspicious class name: {name}"), "info"))
                .collect();
            Ok(AnalyzerResult {
                name: self.name().to_string(),
                findings,
            })
        }
    }

    /// Test-only demo `ReportFormatterPlugin`. Renders a minimal
    /// pipe-delimited summary line -- proves the `OutputFormat::Custom` /
    /// `render_report_with_plugins` dispatch path, nothing more. Not
    /// registered by any production binary.
    pub struct DemoPipeSummaryFormatter;

    impl ReportFormatterPlugin for DemoPipeSummaryFormatter {
        fn format_name(&self) -> &str {
            "demo-pipe-summary"
        }

        fn mime_type(&self) -> &str {
            "text/plain"
        }

        fn render(&self, response: &AnalyzeResponse) -> CoreResult<String> {
            Ok(format!(
                "objects={}|leaks={}",
                response.summary.total_objects,
                response.leaks.len()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{DemoLeakNameAnalyzer, DemoPipeSummaryFormatter};
    use super::*;
    use crate::analysis::AnalyzeResponse;
    use crate::errors::CoreError;

    /// Plain test fixture (not "the" demo plugin -- see
    /// `test_support::DemoLeakNameAnalyzer` for that) used only to prove
    /// `PluginRegistry::run_analyzers` isolates a misbehaving plugin
    /// instead of propagating its error.
    struct FailingTestAnalyzer;

    impl AnalyzerPlugin for FailingTestAnalyzer {
        fn name(&self) -> &str {
            "failing-test-analyzer"
        }

        fn analyze(
            &self,
            _graph: &ObjectGraph,
            _dominator: Option<&DominatorTree>,
        ) -> CoreResult<AnalyzerResult> {
            Err(CoreError::InvalidInput(
                "test fixture intentionally fails".into(),
            ))
        }
    }

    fn fixture_graph() -> ObjectGraph {
        let bytes = crate::hprof::test_fixtures::build_graph_fixture();
        crate::hprof::parse_hprof(&bytes).expect("fixture heap should parse")
    }

    /// Minimal `AnalyzeResponse` for exercising `ReportFormatterPlugin`
    /// without a real heap parse -- mirrors the same-purpose helpers in
    /// `core/src/report/renderer.rs`'s and `core/src/analysis/engine.rs`'s
    /// own test modules.
    fn sample_response_for_formatter_test() -> AnalyzeResponse {
        use crate::graph::GraphMetrics;
        use crate::hprof::HeapSummary;
        use std::time::{Duration, SystemTime};

        AnalyzeResponse {
            mode: crate::analysis::AnalysisMode::Deep,
            overview: None,
            summary: HeapSummary {
                heap_path: "test.hprof".into(),
                total_objects: 10,
                total_size_bytes: 1024,
                classes: Vec::new(),
                generated_at: SystemTime::UNIX_EPOCH,
                header: None,
                total_records: 0,
                record_stats: Vec::new(),
            },
            leaks: Vec::new(),
            recommendations: Vec::new(),
            elapsed: Duration::from_secs(0),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            classloader_report: None,
            collection_report: None,
            string_report: None,
            array_report: None,
            top_instances: None,
            referrer_report: None,
            plugin_results: Vec::new(),
            provenance: Vec::new(),
        }
    }

    #[test]
    fn registry_starts_empty() {
        let registry = PluginRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.analyzer_count(), 0);
        assert_eq!(registry.formatter_count(), 0);
    }

    #[test]
    fn registered_analyzer_runs_and_returns_expected_findings() {
        let graph = fixture_graph();
        let mut registry = PluginRegistry::new();
        registry.register_analyzer(Box::new(DemoLeakNameAnalyzer));
        assert!(!registry.is_empty());
        assert_eq!(registry.analyzer_count(), 1);

        let results = registry.run_analyzers(&graph, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "demo-leak-name-analyzer");
        // The fixture graph's `com/example/BigCache` class doesn't contain
        // "leak"; this just proves the plugin ran over the real graph and
        // produced a well-formed (possibly empty) result rather than
        // panicking or being skipped.
        for finding in &results[0].findings {
            assert!(finding.summary.to_ascii_lowercase().contains("leak"));
        }
    }

    #[test]
    fn failing_analyzer_is_skipped_not_propagated() {
        let graph = fixture_graph();
        let mut registry = PluginRegistry::new();
        registry.register_analyzer(Box::new(FailingTestAnalyzer));
        registry.register_analyzer(Box::new(DemoLeakNameAnalyzer));

        // Must not panic and must not lose the second (working) plugin's
        // output just because the first one errored.
        let results = registry.run_analyzers(&graph, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "demo-leak-name-analyzer");
    }

    #[test]
    fn registered_formatter_is_found_by_name_and_renders() {
        let mut registry = PluginRegistry::new();
        registry.register_formatter(Box::new(DemoPipeSummaryFormatter));
        assert_eq!(registry.formatter_count(), 1);

        let formatter = registry
            .find_formatter("demo-pipe-summary")
            .expect("formatter should be registered under its format_name");
        assert_eq!(formatter.mime_type(), "text/plain");

        let response = sample_response_for_formatter_test();
        let rendered = formatter.render(&response).unwrap();
        assert_eq!(rendered, "objects=10|leaks=0");
    }

    #[test]
    fn unknown_formatter_name_is_not_found() {
        let registry = PluginRegistry::new();
        assert!(registry.find_formatter("does-not-exist").is_none());
    }
}
