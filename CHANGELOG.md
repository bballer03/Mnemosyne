# Changelog

All notable changes to Mnemosyne will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure
- Documentation (README, ARCHITECTURE, CONTRIBUTING)
- Copilot instructions for fun commit messages
- Architecture diagrams (SVG)
- Rust workspace scaffolding (`mnemosyne-core` + `mnemosyne-cli`) with stub CLI commands and core APIs
- Basic HPROF header parsing with CLI wiring for `parse`, `leaks`, and `analyze`
- Record-level HPROF scanning with CLI summaries (top tags, record counts, heuristics-driven leak severity)
- Functional MCP stdio server that handles `parse_heap` and `detect_leaks` requests
- Graph module with synthetic dominator summaries included in analysis reports
- Source mapping module with `mnemosyne map` CLI command, MCP `map_to_code` handler, and leak identifiers surfaced in reports
- GC path tracing scaffolding with CLI `gc-path` subcommand and MCP `find_gc_path` endpoint
- AI Insights heuristics powering `--ai` analysis output with model/confidence metadata in CLI, reports, and JSON responses

### Coming Soon
- HPROF parser implementation
- Basic leak detection
- CLI interface
- MCP server

---

## [0.1.0] - TBD (Alpha Release)

### Planned Features
- Basic heap dump parsing
- Class histogram generation
- Dominator tree computation
- Simple leak detection
- CLI tool with basic commands
- MCP server for IDE integration

---

## Version History

### Alpha Phase (Current)
- **0.1.0**: Initial alpha release (planned)
  - Core parsing functionality
  - Basic analysis features
  - MCP integration

### Future Phases

#### Beta Phase
- **0.2.0**: Enhanced analysis
  - AI-powered insights
  - Source code mapping
  - Git integration

#### Version 1.0
- **1.0.0**: Production-ready release
  - Stable API
  - Complete documentation
  - Performance optimizations
  - Comprehensive test coverage

---

## Release Notes Template

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New features and capabilities

### Changed
- Changes to existing functionality

### Deprecated
- Features that will be removed in future versions

### Removed
- Features that have been removed

### Fixed
- Bug fixes

### Security
- Security-related changes
```

---

[Unreleased]: https://github.com/bballer03/mnemosyne/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bballer03/mnemosyne/releases/tag/v0.1.0
