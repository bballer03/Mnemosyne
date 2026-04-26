# M6 Phase 8 — Plugin/Extension System Design

> **Status:** Design only — implementation deferred until demonstrated demand  
> **Parent:** [milestone-6-ecosystem-and-community.md](milestone-6-ecosystem-and-community.md) §20  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-04-25

---

## 1. Status & Recommendation

**Current stance:** document, don't build.

The `mnemosyne-core` crate already exposes a public library API (`ObjectGraph`, `AnalysisConfig`, `AnalyzeResponse`, `render_report`, LLM helpers). Any Rust project can `use mnemosyne_core` today and build custom analysis on top. A formal plugin system adds discovery, registration, and (optionally) dynamic loading — none of which is justified by the current user base.

**Recommended path:**

| Phase | Trigger | Deliverable |
|-------|---------|-------------|
| **1 — Library API** (current) | Already available | Document `mnemosyne-core` as the extension mechanism |
| **2 — Trait registry** | ≥3 requests for custom analyzers/formats | `AnalyzerPlugin` + `ReportFormatterPlugin` traits, static registration |
| **3 — Dynamic loading** | Phase 2 adoption + demand for out-of-tree plugins | `cdylib` discovery via `~/.mnemosyne/plugins/` |

---

## 2. Extension Points

### 2.1 Custom Analyzer Plugin

Enables user-defined analysis passes over the parsed heap.

```rust
use mnemosyne_core::hprof::ObjectGraph;
use mnemosyne_core::config::AnalysisConfig;
use mnemosyne_core::CoreError;

/// Result returned by a plugin analyzer.
pub struct AnalyzerResult {
    pub name: String,
    pub findings: Vec<AnalyzerFinding>,
}

pub struct AnalyzerFinding {
    pub summary: String,
    pub severity: String,         // "info" | "warning" | "critical"
    pub detail: Option<String>,
}

pub trait AnalyzerPlugin: Send + Sync {
    /// Human-readable name shown in reports.
    fn name(&self) -> &str;

    /// Run the analysis pass over the object graph.
    fn analyze(
        &self,
        graph: &ObjectGraph,
        config: &AnalysisConfig,
    ) -> Result<AnalyzerResult, CoreError>;
}
```

Integration: a `PluginRegistry` would iterate registered analyzers after the built-in leak/dominator passes and append their findings to `AnalyzeResponse`.

### 2.2 Custom Report Formatter Plugin

Extends output beyond the built-in five formats (`Text`, `Toon`, `Markdown`, `Html`, `Json`).

```rust
use mnemosyne_core::analysis::AnalyzeResponse;
use mnemosyne_core::CoreError;

pub trait ReportFormatterPlugin: Send + Sync {
    /// Format identifier used in `--format custom:<name>`.
    fn format_name(&self) -> &str;

    /// MIME type of the rendered output.
    fn mime_type(&self) -> &str;

    /// Render the analysis response into a string.
    fn render(&self, response: &AnalyzeResponse) -> Result<String, CoreError>;
}
```

Integration: `render_report()` in `core/src/report/renderer.rs` would delegate to the registry when `OutputFormat` is `Custom(name)`.

### 2.3 Custom LLM Backend

The existing `AiProvider` enum (`OpenAi`, `Anthropic`, `Local`) and `llm::complete()` dispatcher already form a provider abstraction. Extending it:

```rust
pub trait LlmBackendPlugin: Send + Sync {
    fn provider_name(&self) -> &str;
    fn complete(
        &self,
        prompt: &str,
        config: &AiConfig,
    ) -> Result<String, CoreError>;
}
```

This would sit alongside `llm::complete()` as a fallback: if `AiProvider` doesn't match a built-in variant, the registry is consulted.

---

## 3. Plugin Discovery & Registration

### 3.1 Static (compile-time)

Library users call registration functions before invoking analysis:

```rust
let mut registry = PluginRegistry::new();
registry.register_analyzer(Box::new(MyCustomAnalyzer));
registry.register_formatter(Box::new(MySarifFormatter));
```

No ABI concerns. No security risk beyond normal `mnemosyne-core` usage.

### 3.2 Config-based

A `[plugins]` section in `mnemosyne.toml`:

```toml
[plugins]
analyzers = ["path/to/libmy_analyzer.so"]
formatters = ["path/to/libsarif_fmt.so"]
```

### 3.3 Directory-based

Well-known directory `~/.mnemosyne/plugins/` scanned at startup. Each `.so`/`.dylib`/`.dll` exposes a C-ABI entry point:

```rust
#[no_mangle]
pub extern "C" fn mnemosyne_plugin_init(registry: &mut PluginRegistry);
```

---

## 4. CLI Surface (future)

If plugin registration is implemented:

```
mnemosyne analyze --plugin ./libmy_analyzer.so dump.hprof
mnemosyne analyze --format custom:sarif dump.hprof
```

No changes to existing CLI commands or MCP handlers.

---

## 5. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Over-engineering for current user base | **High** | Defer to Phase 2/3; library API covers most needs now |
| ABI stability across Rust versions | **High** | Use C-ABI entry points; avoid exposing Rust-internal layouts |
| Security: loading arbitrary shared libraries | **High** | Document trust model; restrict to explicit opt-in paths |
| API churn in `ObjectGraph` / `AnalyzeResponse` | **Medium** | Stabilize core types (M3) before committing plugin ABI |
| Maintenance burden of plugin compatibility | **Medium** | Version the plugin ABI; fail fast on mismatch |

---

## 6. Non-scope

- Actual implementation of plugin loading or registry
- Changes to `Cargo.toml`, source files, or CI
- WASM-based plugin sandboxing (interesting but premature)
- Plugin marketplace or distribution infrastructure

---

## 7. Decision Record

**Decision:** Defer plugin system implementation. The `mnemosyne-core` library API is the extension mechanism for now. This document captures the design so it can be picked up when demand materialises.

**Revisit when:** three or more users/integrators request custom analyzers, output formats, or LLM backends that cannot be satisfied by the library API alone.
