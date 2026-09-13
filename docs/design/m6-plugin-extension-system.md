# M6 Phase 8 — Plugin/Extension System Design

> **Status:** Phase 2 shipped (M15 Slice 15.F); Phase 3 still gated  
> **Parent:** [milestone-6-ecosystem-and-community.md](milestone-6-ecosystem-and-community.md) §20  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-09-13 (M15 Slice 15.G doc-sync)

---

## 1. Status & Recommendation

**Current stance:** Phase 2 is shipped; Phase 3 remains deferred.

The `mnemosyne-core` crate exposes a public library API (`ObjectGraph`, `AnalysisConfig`, `AnalyzeResponse`, `render_report`, LLM helpers). M15 Slice 15.F added a formal **Phase-2 static trait registry** (`core::plugin`) so Rust consumers can register custom analyzers and formatters at compile time. Dynamic loading (Phase 3) is still gated — no third-party plugin demand exists yet to justify `cdylib` ABI stability work.

**Recommended path:**

| Phase | Trigger | Deliverable | Status |
|-------|---------|-------------|--------|
| **1 — Library API** | Already available | Document `mnemosyne-core` as the extension mechanism | ✅ Shipped |
| **2 — Trait registry** | ≥3 requests for custom analyzers/formats | `AnalyzerPlugin` + `ReportFormatterPlugin` traits, static registration | ✅ Shipped (M15 Slice 15.F) |
| **3 — Dynamic loading** | Phase 2 adoption + demand for out-of-tree plugins | `cdylib` discovery via `~/.mnemosyne/plugins/` | ⏳ Gated |

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
        dominator: Option<&DominatorTree>,
    ) -> CoreResult<AnalyzerResult>;
}
```

**Shipped integration (M15):** `PluginRegistry::register_analyzer()` plus `analyze_heap_with_plugins()` appends each registered analyzer's output to `AnalyzeResponse::plugin_results`. Existing `analyze_heap()` callers are unchanged (empty `plugin_results`).

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

**Shipped integration (M15):** `OutputFormat::Custom(String)` plus `render_report_with_plugins()` in `core/src/report/renderer.rs` delegates to a registered `ReportFormatterPlugin`. Plain `render_report()` returns `CoreError::Unsupported` for custom formats when no registry is supplied.

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

Phase 2 ships **§3.1 only**. §3.2–3.3 describe proposed Phase 3 behavior — not implemented.

### 3.1 Static (compile-time) — shipped (Phase 2)

Library users call registration functions before invoking analysis:

```rust
let mut registry = PluginRegistry::new();
registry.register_analyzer(Box::new(MyCustomAnalyzer));
registry.register_formatter(Box::new(MySarifFormatter));
```

No ABI concerns. No security risk beyond normal `mnemosyne-core` usage.

### 3.2 Config-based (Phase 3 — proposed, not shipped)

A future `[plugins]` section in `mnemosyne.toml` would allow:

```toml
[plugins]
analyzers = ["path/to/libmy_analyzer.so"]
formatters = ["path/to/libsarif_fmt.so"]
```

### 3.3 Directory-based (Phase 3 — proposed, not shipped)

A future well-known directory `~/.mnemosyne/plugins/` would be scanned at startup. Each `.so`/`.dylib`/`.dll` would expose a C-ABI entry point:

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

## 6. Non-scope (Phase 3 — still deferred)

- Dynamic `cdylib`/`.so`/`.dylib`/`.dll` loading or filesystem discovery
- Config-driven plugin paths (`[plugins]` in `mnemosyne.toml`, `~/.mnemosyne/plugins/`)
- CLI `--plugin <path>` flag
- WASM-based plugin sandboxing (interesting but premature)
- Plugin marketplace or distribution infrastructure

Phase 2 (static registry) **is** implemented — see `core/src/plugin/mod.rs`.

---

## 7. Decision Record

**Decision (2026-04-25):** Defer dynamic plugin loading; library API is sufficient for now.

**Update (2026-09-13, M15 Slice 15.F):** Phase 2 shipped — `AnalyzerPlugin`, `ReportFormatterPlugin`, and `PluginRegistry` with compile-time static registration. Phase 3 remains gated behind "Phase 2 adoption + demand for out-of-tree plugins."

**Revisit Phase 3 when:** real out-of-tree plugin authors appear and compile-time linking is insufficient.
