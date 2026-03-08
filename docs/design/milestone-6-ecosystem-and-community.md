# Milestone 6 — Ecosystem & Community

> **Status:** ⚬ Pending  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Build the documentation, examples, benchmarks, community infrastructure, and ecosystem integrations that make Mnemosyne self-sustaining as an open-source project — moving from "interesting alpha" to "tool that developers recommend to each other."

## Context

Open-source success requires more than good code. It requires comprehensive documentation, real-world examples, reproducible benchmarks, contributor onboarding paths, and community engagement channels. Mnemosyne has strong technical foundations (Rust performance, provenance system, MCP integration) but currently lacks: API documentation with real content, example projects, benchmark comparisons, a contributor ladder, community channels, and the content that drives organic discovery (blog posts, conference talks, case studies).

M6 is intentionally the last milestone because it benefits most from having a mature, feature-complete tool to showcase. But some elements (API docs, basic examples) should be started earlier as dependencies allow.

## Scope

### Documentation
1. **API documentation** — comprehensive rustdoc with examples, published to docs.rs
2. **User guide** — tutorial-style walkthrough from installation to leak resolution
3. **MCP API reference** — real docs/api.md with JSON-RPC signatures, schemas, wire-format examples for all 7+ handlers
4. **Troubleshooting guide** — common errors, unsupported HPROF variants, limitations, FAQ
5. **Architecture walkthrough** — annotated dive into the codebase for contributors

### Examples & Sample Data
6. **Example Java projects** — 3-5 sample apps with known memory issues:
   - Cache leak (unbounded HashMap)
   - Thread leak (spawned threads not joined)
   - ClassLoader leak (dynamic class loading without cleanup)
   - Large string duplication
   - HTTP response body leak
7. **Sample heap dumps** — pre-generated .hprof files from each example app
8. **CLI workflow walkthroughs** — step-by-step tutorials using sample dumps
9. **MCP integration examples** — VS Code, Cursor, Zed configuration samples

### Benchmarks
10. **Criterion benchmark suite** — parser throughput, graph construction, dominator computation, OQL evaluation
11. **Hyperfine CLI timing** — end-to-end analysis time at dump size tiers (10MB, 100MB, 1GB, 10GB)
12. **Heaptrack memory profiling** — RSS measurement at each size tier
13. **Comparative benchmarks** — Mnemosyne vs hprof-slurp (speed) vs MAT (features) where fair comparison exists
14. **Published benchmark results** — in README and dedicated docs/benchmarks.md

### Community Infrastructure
15. **Contributor guide enhancement** — architecture overview, module ownership, development setup, coding standards
16. **Good First Issues program** — labeled issues with clear scope, mentorship notes
17. **Contributor ladder** — documented path from first contribution to maintainer
18. **Discord/Discussions** — community channel for questions, feedback, showcases
19. **Integration examples** — GitHub Actions workflow, Jenkins pipeline, GitLab CI templates

### Ecosystem
20. **Plugin/extension system** — custom analyzers, output formats, LLM backends
21. **Case studies** — real-world usage stories from early adopters
22. **Content creation** — blog posts, conference talk proposals

## Non-scope

- Core analysis features (M3)
- AI/LLM implementation (M5)
- Web UI features (M4)
- Packaging/distribution changes (M2)
- Breaking API changes
- Hosted/SaaS infrastructure

## Architecture Overview

M6 does not change the core architecture. It adds surrounding assets:

```
┌───────────────────────────────────────────────────────────────┐
│                    ECOSYSTEM LAYER (new)                       │
│                                                               │
│  docs/                                                        │
│  ├── api.md            ← MCP JSON-RPC reference              │
│  ├── user-guide.md     ← Installation → analysis tutorials   │
│  ├── troubleshooting.md← Common errors + workarounds         │
│  ├── benchmarks.md     ← Published performance data          │
│  ├── examples/                                                │
│  │   ├── cache-leak/   ← Example Java app + .hprof + walkthru│
│  │   ├── thread-leak/                                         │
│  │   ├── classloader-leak/                                    │
│  │   ├── string-dup/                                          │
│  │   └── http-leak/                                           │
│  └── integrations/                                            │
│      ├── github-actions.md                                    │
│      ├── jenkins.md                                           │
│      └── vscode-mcp.md                                        │
│                                                               │
│  benches/                                                     │
│  ├── parser_bench.rs   ← criterion: parser throughput        │
│  ├── graph_bench.rs    ← criterion: graph construction       │
│  └── dominator_bench.rs← criterion: dominator computation    │
│                                                               │
│  examples/             ← Example Java source projects        │
│  ├── cache-leak-app/                                          │
│  ├── thread-leak-app/                                         │
│  └── ...                                                      │
└───────────────────────────────────────────────────────────────┘
          │
┌─────────┼─────────────────────────────────────────────────────┐
│         ▼        CORE + CLI + MCP (unchanged)                 │
│  All existing modules preserved as-is                         │
└───────────────────────────────────────────────────────────────┘
```

### Plugin System Architecture (conceptual)

```
┌─────────────────────────────────────────────────┐
│  Plugin Registry                                │
│  ┌──────────────┐  ┌──────────────────────────┐ │
│  │  Analyzer    │  │  Output Formatter        │ │
│  │  Plugins     │  │  Plugins                 │ │
│  │              │  │                          │ │
│  │  trait       │  │  trait ReportFormatter { │ │
│  │  Analyzer {  │  │    fn format(data) ->    │ │
│  │    fn run()  │  │      String;             │ │
│  │  }           │  │  }                       │ │
│  └──────────────┘  └──────────────────────────┘ │
│  ┌──────────────┐                               │
│  │  LLM Backend │                               │
│  │  Plugins     │                               │
│  │              │                               │
│  │  (reuses     │                               │
│  │   LlmProvider│                               │
│  │   trait)     │                               │
│  └──────────────┘                               │
└─────────────────────────────────────────────────┘
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `docs/api.md` | Rewritten | Real MCP API reference (currently placeholder) |
| `docs/user-guide.md` | New | Tutorial-style user guide |
| `docs/troubleshooting.md` | New | Common errors and workarounds |
| `docs/benchmarks.md` | New | Published benchmark results |
| `docs/examples/` | Rewritten | Real CLI + MCP workflow examples (currently placeholder) |
| `docs/integrations/` | New | CI/CD integration templates |
| `benches/parser_bench.rs` | New | Criterion parser benchmarks |
| `benches/graph_bench.rs` | New | Criterion graph benchmarks |
| `benches/dominator_bench.rs` | New | Criterion dominator benchmarks |
| `examples/` | New | Example Java projects with known leaks |
| `CONTRIBUTING.md` | Enhanced | Architecture walkthrough, module guide |
| `README.md` | Updated | Benchmark results, additional badges |
| `Cargo.toml` | Updated | Criterion dev-dependency, bench targets |

## API/CLI/Reporting Impact

### Plugin System (if implemented)
- New `--plugin` flag for loading custom analyzers
- New `--format custom:<plugin>` for custom output formats
- Plugin discovery via config file or well-known directory

### No changes to existing API
All existing CLI commands, MCP handlers, and report formats remain unchanged.

## Data Model Changes

### New Types (if plugin system implemented)
- `AnalyzerPlugin` (trait) — custom analysis plugin interface
- `ReportFormatterPlugin` (trait) — custom output format interface
- `PluginRegistry` — discovery and lifecycle management

### No changes to existing types
All core data model types remain unchanged.

## Validation/Testing Strategy

### Documentation
- All code examples in docs must compile and run
- API docs include wire-format examples that match actual MCP output
- Tutorials tested end-to-end against sample heap dumps

### Benchmarks
- Criterion benchmarks run in CI (but don't block on regression — track only)
- Benchmark results reproducible on standard hardware (document specs)
- Comparative benchmarks run with pinned versions of competitors

### Examples
- Each example Java project compiles and generates heap dumps
- Heap dumps parse correctly through Mnemosyne's pipeline
- Walkthrough steps produce the documented output

### Community
- Issue templates validated by creating test issues
- Contributor guide tested by a fresh contributor (ideally)
- CI integration templates validated in example repositories

## Rollout/Implementation Phases

### Phase 1 — Documentation Foundation (effort: Medium)
1. Rewrite docs/api.md with real MCP handler documentation
2. Create docs/troubleshooting.md
3. Real examples in docs/examples/ (CLI workflows, MCP integration)
4. Enhanced CONTRIBUTING.md with architecture walkthrough

### Phase 2 — Benchmarks (effort: Medium)
5. Criterion benchmark suite setup
6. Hyperfine scripts for CLI timing
7. Heaptrack integration for memory profiling
8. Publish initial benchmark results

### Phase 3 — Example Projects (effort: Medium-Large)
9. Create 3-5 example Java apps with known memory issues
10. Generate and commit sample heap dumps
11. Write step-by-step tutorial for each example
12. Create MCP integration example (VS Code config)

### Phase 4 — Community (effort: Medium)
13. Set up Discord or GitHub Discussions
14. Create Good First Issues with mentorship notes
15. Document contributor ladder
16. CI integration templates (GitHub Actions, Jenkins, GitLab)

### Phase 5 — Ecosystem (effort: Large)
17. Plugin system design and implementation
18. Blog post: "Introduction to Mnemosyne"
19. Case study: first real-world usage story
20. Conference talk proposal

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Documentation becomes stale as features evolve | High | Medium | Automate doc generation where possible; doc-sync agent |
| Benchmark results may show unfavorable comparison to hprof-slurp | Medium | Medium | Be honest about tradeoffs; position as "analysis depth vs raw speed" |
| Plugin system may over-engineer for current user base | Medium | Medium | Defer until demonstrated demand; start with documentation, not code |
| Example Java apps require JVM setup for regeneration | Low | Low | Commit generated .hprof files; provide Dockerfile for regeneration |
| Community channels may not reach critical mass | Medium | Medium | Focus on GitHub Discussions first (lower friction than Discord) |

### Open Questions
1. Should example heap dumps be large files in-repo or downloadable? (Recommendation: small <10MB in-repo, large downloadable)
2. Is a plugin system worth the complexity? (Recommendation: defer; document extension points via library API first)
3. Discord vs GitHub Discussions? (Recommendation: Discussions first, Discord when >50 active contributors)
4. Should benchmarks run in CI? (Recommendation: yes, track-only, no blocking regression gates initially)

### Success Metrics
| Metric | 6-month target | 12-month target |
|---|---|---|
| GitHub stars | 500 | 2,000 |
| Monthly crates.io downloads | 200 | 1,000 |
| Contributors | 5 | 15 |
| Open issues (healthy) | 20 | 50 |
| Test count | 100 | 300 |
| Doc coverage | 80% | 95% |

### Dependencies
- **Blocked by:** M1-M5 (needs a mature tool to showcase)
- **Partial overlap:** API docs (Phase 1) can start after M1.5; benchmarks (Phase 2) can start after M1.5; examples need M3 features
