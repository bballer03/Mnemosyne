# Contributing to Mnemosyne

Thank you for your interest in contributing to Mnemosyne! We appreciate your help in making this AI-powered JVM memory analysis tool even better.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Pull Request Process](#pull-request-process)
- [Issue Reporting](#issue-reporting)
- [Architecture for Contributors](#architecture-for-contributors)
- [Contributor Ladder](#contributor-ladder)
- [Community](#community)

---

## Code of Conduct

This project adheres to a code of conduct that all contributors are expected to follow:

- Be respectful and inclusive
- Focus on constructive feedback
- Accept differing viewpoints gracefully
- Show empathy towards other community members

---

## Getting Started

Mnemosyne is maintained by **bballer03**. Contribute against the upstream repository at https://github.com/bballer03/mnemosyne.

### Prerequisites

- **Rust** 1.70 or later
- **Git**
- **A JVM** (for testing with real heap dumps)
- **Optional:** AI provider credentials if you want to exercise provider-backed AI flows (`OPENAI_API_KEY` for OpenAI-compatible mode, `ANTHROPIC_API_KEY` for the in-progress Anthropic path)

### First-Time Setup

1. **Fork the repository** on GitHub
2. **Clone your fork:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/mnemosyne
   cd mnemosyne
   ```

3. **Add upstream remote:**
   ```bash
   git remote add upstream https://github.com/bballer03/mnemosyne
   ```

4. **Install dependencies:**
   ```bash
   cargo build
   ```

5. **Run tests to verify setup:**
   ```bash
   cargo test
   ```

---

## Development Setup

### Building the Project

```bash
# Development build (faster, with debug symbols)
cargo build

# Release build (optimized)
cargo build --release

# Build with all features
cargo build --all-features
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Linting and Formatting

```bash
# Format code
cargo fmt

# Check formatting (CI-friendly)
cargo fmt -- --check

# Run Clippy (linter)
cargo clippy -- -D warnings

# Fix auto-fixable issues
cargo clippy --fix
```

### Running the Development Version

```bash
# Parse a heap dump
cargo run -p mnemosyne-cli -- parse test.hprof

# Run with debug logging
RUST_LOG=debug cargo run -p mnemosyne-cli -- analyze test.hprof

# Run MCP server
cargo run -p mnemosyne-cli -- serve
```

---

## Project Structure

```
mnemosyne/
├── Cargo.toml              # Workspace root
├── cli/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs         # CLI entry point
│   │   └── config_loader.rs
│   └── tests/
│       └── integration.rs  # CLI integration coverage
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            # Public API re-exports
│       ├── hprof/            # parser.rs, binary_parser.rs, object_graph.rs, test_fixtures.rs
│       ├── graph/            # dominator.rs, gc_path.rs, metrics.rs
│       ├── analysis/         # engine.rs, ai.rs
│       ├── mapper/           # source.rs
│       ├── report/           # renderer.rs
│       ├── fix/              # generator.rs
│       ├── mcp/              # server.rs
│       ├── config.rs         # Configuration types
│       └── errors.rs         # CoreError types
├── docs/                    # Documentation
├── resources/
│   └── test-fixtures/       # Test fixture documentation
├── HomebrewFormula/
│   └── mnemosyne.rb         # macOS Homebrew formula
├── .github/
│   └── workflows/           # CI + release workflows
└── Dockerfile               # Multi-stage Docker build
```

---

## Coding Standards

### Rust Style Guide

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and use `rustfmt` for formatting.

#### Key Principles

1. **Safety First**
   - Minimize use of `unsafe` code
   - Document any `unsafe` blocks with safety invariants
   - Prefer safe abstractions

2. **Error Handling**
   - Use `Result` for recoverable errors
   - Use `anyhow` for application errors
   - Use `thiserror` for library errors
   - Provide meaningful error messages

3. **Documentation**
   - Add doc comments (`///`) to all public items
   - Include examples in doc comments
   - Document panics, safety, and errors

4. **Naming Conventions**
   - `snake_case` for functions, variables, modules
   - `PascalCase` for types, traits, enums
   - `SCREAMING_SNAKE_CASE` for constants

### Example Code Style

```rust
/// Parses an HPROF heap dump file.
///
/// # Arguments
///
/// * `path` - Path to the heap dump file
///
/// # Returns
///
/// A `HeapSnapshot` containing the parsed data
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The HPROF format is invalid
/// - The file is corrupted
///
/// # Example
///
/// ```
/// use mnemosyne::parse_heap;
///
/// let snapshot = parse_heap("heap.hprof")?;
/// println!("Total objects: {}", snapshot.total_objects());
/// ```
pub fn parse_heap(path: impl AsRef<Path>) -> Result<HeapSnapshot> {
    let file = File::open(path.as_ref())
        .context("Failed to open heap dump file")?;
    
    Parser::new(file)
        .parse()
        .context("Failed to parse heap dump")
}
```

---

## Testing Guidelines

### Writing Tests

1. **Unit Tests**
   - Place in the same file as the code (`#[cfg(test)]` module)
   - Test individual functions and methods
   - Mock external dependencies

2. **Integration Tests**
   - Place in `tests/` directory
   - Test complete workflows
   - Use real (but small) heap dumps

3. **Property Tests**
   - Use `proptest` or `quickcheck` for property-based testing
   - Great for parsers and algorithms

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;

      #[cfg(feature = "test-fixtures")]
      use mnemosyne_core::test_fixtures::{build_graph_fixture, build_simple_fixture};

    #[test]
      #[cfg(feature = "test-fixtures")]
    fn test_parse_small_heap() {
            let fixture = build_simple_fixture();
            assert!(fixture.len() > 0);
    }

    #[test]
      #[cfg(feature = "test-fixtures")]
      fn test_build_graph_fixture() {
            let fixture = build_graph_fixture();
            assert!(fixture.len() > 0);
    }

    #[test]
    #[should_panic(expected = "Invalid HPROF magic number")]
    fn test_invalid_format() {
        parse_heap("tests/fixtures/invalid.bin").unwrap();
    }
}
```

### Test Coverage

We aim for:
- **80%+ code coverage** overall
- **90%+ coverage** for critical paths (parser, leak detection)
- **100% coverage** for unsafe code

Run coverage reports:
```bash
cargo tarpaulin --out Html --output-dir coverage
```

---

## Commit Message Guidelines

We follow a fun but informative commit style! See [.github/copilot-instructions.md](.github/copilot-instructions.md) for details.

### Format

```
<type>: <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `docs`: Documentation changes
- `test`: Test additions or changes
- `chore`: Build/tooling changes
- `style`: Code style changes (formatting)

### Examples

**Good:**
```
feat: add coroutine leak detection to AI engine

Mnemosyne now remembers to check for suspended coroutines
that Zeus forgot to clean up. Includes dominator tree analysis
and GC root tracing for Kotlin coroutines.

Closes #42
```

**Also Good (with humor):**
```
fix: stopped the heap from forgetting to free itself

The parser was hoarding objects like a digital dragon.
Now it properly releases memory as it goes.

Fixes #128
```

**Avoid:**
```
fix stuff
update code
WIP
```

---

## Pull Request Process

### Before Submitting

1. **Update from upstream:**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run the full test suite:**
   ```bash
   cargo test --all-features
   cargo clippy -- -D warnings
   cargo fmt -- --check
   ```

3. **Update documentation:**
   - Add/update doc comments
   - Update README.md if needed
   - Add examples if introducing new features

4. **Add tests:**
   - All new code should have tests
   - Ensure existing tests pass

### PR Guidelines

1. **Title:** Clear, descriptive title following commit message format
2. **Description:** Explain what, why, and how
3. **Link issues:** Use "Fixes #123" or "Closes #456"
4. **Keep PRs focused:** One feature/fix per PR
5. **Request review:** Tag relevant maintainers

### PR Template

```markdown
## Description
Brief description of changes

## Motivation
Why is this change needed?

## Changes Made
- Item 1
- Item 2

## Testing
How was this tested?

## Checklist
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Clippy passes
- [ ] Formatting checked
- [ ] CHANGELOG.md updated (if needed)

Fixes #(issue)
```

### Review Process

1. **Automated checks** must pass (CI/CD)
2. **All pull request conversations** must be addressed or resolved
3. **Conflicts resolved** with main branch
4. **Protected main workflow:** changes land through pull requests; approval requirements may be tightened when multiple maintainers are active

---

## Issue Reporting

### Bug Reports

Use the bug report template and include:

- **Description:** Clear description of the bug
- **Steps to Reproduce:** Numbered steps
- **Expected Behavior:** What should happen
- **Actual Behavior:** What actually happens
- **Environment:**
  - OS and version
  - Rust version (`rustc --version`)
  - Mnemosyne version
- **Heap Dump Info:** Size, JVM version (if applicable)
- **Logs:** Relevant error messages or stack traces

### Feature Requests

Use the feature request template and include:

- **Problem:** What problem does this solve?
- **Proposed Solution:** Your suggested approach
- **Alternatives:** Other solutions you've considered
- **Additional Context:** Screenshots, examples, etc.

### Questions

For questions:
- Check existing issues and documentation first
- Use GitHub Discussions for general questions
- Tag with `question` label

---

## Architecture for Contributors

If you are picking up your first issue, start with the high-level module layout in [ARCHITECTURE.md](ARCHITECTURE.md) and the tree in [Project Structure](#project-structure). The walkthrough below is the practical version contributors usually need when deciding where a change belongs.

### How a Heap Dump Flows Through the System

1. `core::hprof::parser` reads the HPROF header and record tags in a streaming pass. This is the fast path for quick triage and summary-oriented commands.
2. `core::hprof::binary_parser` performs the full binary parse and builds the `ObjectGraph` used by deep analysis.
3. `core::graph::dominator` computes the Lengauer-Tarjan dominator tree and retained sizes on top of that graph.
4. `core::analysis::engine` runs `analyze_heap()` and `detect_leaks()`, preferring graph-backed analysis and falling back to heuristics when the full graph path is unavailable.
5. `core::graph::gc_path` traces GC root paths with `ObjectGraph` BFS plus documented fallbacks when the heap data is incomplete.
6. `core::report::renderer` turns analysis results into the supported output formats: Text, Markdown, HTML, TOON, and JSON.
7. `cli::main` is the CLI entry point that wires user-facing commands onto the shared core APIs.
8. `core::mcp::server` exposes the same analysis capabilities over stdio JSON-RPC for editor and IDE integration.

### Module Ownership Guide

| Module | What it does | Good first issues |
| --- | --- | --- |
| `hprof/` | HPROF parsing, record decoding, and object graph construction | fixture coverage, parser edge cases, clearer parse errors, tag handling fixes |
| `analysis/` | leak detection, AI integration, and investigation analyzers | ranking tweaks, new analyzers, provenance fixes, small request/response improvements |
| `graph/` | dominator tree, GC paths, and graph metrics | traversal bugs, retained-size correctness checks, metrics polish, fallback behavior cleanup |
| `report/` | output formatting and escaping | renderer consistency, table/layout polish, escaping fixes, output parity across formats |
| `config.rs` | configuration types, defaults, and config plumbing | default-value cleanup, validation improvements, config documentation alignment |
| `fix/` | fix suggestion generation | better heuristics, output shaping, provenance labeling, safer fallback messaging |
| `mcp/` | stdio server transport and tool wiring | request validation, error contract cleanup, method coverage, transport robustness |

### Key Types to Understand

- `ObjectGraph` is the central heap model shared by parsing, dominator analysis, GC-path tracing, and higher-level investigation code.
- `AnalyzeRequest` and `AnalyzeResponse` define the main analysis contract used by the CLI, reports, and MCP surfaces.
- `LeakInsight` represents one leak candidate, including severity, retained-size context, and provenance markers.
- `AppConfig` is the configuration hierarchy that controls parsing, analysis, AI, and output behavior.
- `ProvenanceMarker` labels fallback, partial, synthetic, and placeholder data so contributors can preserve honest output semantics.

### Development Flow

The full setup is already covered in [Development Setup](#development-setup), [Testing Guidelines](#testing-guidelines), and [Pull Request Process](#pull-request-process). For day-to-day work, the short loop is:

- Run `cargo check` frequently while developing.
- Run `cargo test --workspace` before pushing.
- Run `cargo clippy --workspace --all-targets -- -D warnings` to catch lint regressions.
- Run `cargo fmt --all -- --check` to verify formatting.

---

## Contributor Ladder

Mnemosyne welcomes small first fixes just as much as deeper subsystem work. The ladder below is not a rigid program; it is the usual path from a first PR to broader project responsibility.

### First-Time Contributor

Start with issues labeled `good first issue` or `help wanted`. These are the best entry points for learning the codebase, the review expectations, and the testing loop without having to understand every subsystem at once.

- Pick a focused issue, fork the repository, and open a pull request.
- If you are unsure whether an issue is still available or where a change should land, comment on the issue and ping the maintainer before you invest heavily.
- Draft pull requests are welcome for first contributions, especially when you want feedback on direction early.

### Regular Contributor

After a few merged pull requests, contributors usually become comfortable moving between parser, analysis, graph, and reporting work.

- Expect to take on slightly broader changes that may touch more than one module.
- Keep pull requests focused, test behavior changes, and update docs when the user-facing contract moves.
- Participate in review discussions by responding promptly and explaining tradeoffs clearly.

### Trusted Contributor

Trusted contributors have a track record of consistent, high-signal changes and good review hygiene.

- You may be asked to take on larger features or trickier bug investigations.
- Reviews at this stage should show strong attention to correctness, fallback behavior, and contract stability across CLI, MCP, and docs.
- Trusted contributors are often the first people maintainers look to for milestone follow-through and cross-module cleanup.

### Maintainer

Maintainers are responsible for more than code changes. They help keep the project healthy and predictable for everyone else.

- Typical maintainer responsibilities include issue triage, pull request review, release management, and keeping roadmap work aligned with the shipped behavior.
- Maintainer access is earned through a demonstrated track record of high-quality contributions, thoughtful review participation, and reliable follow-through over time.
- There is no shortcut here: the clearest path is steady, correct work and good collaboration across multiple contribution cycles.

### Review and Mentorship Notes

- Code review expectations are the same at every level: clear problem statement, focused changes, tests for behavior changes when practical, and responsiveness to feedback.
- For your first PR, it is fine to ask for guidance explicitly. A short note in the issue or PR description is enough.
- If you want help choosing a starter task, begin with the issue labels above and use [Getting Help](#getting-help) when you need clarification.

---

## Community

For questions, ideas, and feedback, use GitHub Discussions. For bugs and feature requests, use the issue tracker and follow the guidance in [Issue Reporting](#issue-reporting). For private security disclosures, please follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

---

## Development Tips

### Useful Commands

```bash
# Watch and auto-rebuild on changes
cargo watch -x build

# Run specific example
cargo run --example parse_heap

# Generate documentation
cargo doc --open

# Check dependencies for updates
cargo outdated

# Audit dependencies for security
cargo audit
```

### Debugging

```bash
# Enable debug logging
RUST_LOG=debug cargo run -- analyze heap.hprof

# Use rust-gdb or lldb for debugging
rust-gdb target/debug/mnemosyne

# Profile with perf (Linux)
cargo build --release
perf record target/release/mnemosyne parse large.hprof
perf report
```

### Performance Testing

```bash
# Benchmark
cargo bench

# Flamegraph (requires cargo-flamegraph)
cargo flamegraph -- parse heap.hprof
```

---

## Getting Help

- **GitHub Issues:** Bug reports and feature requests
- **GitHub Discussions:** Questions and general discussion
- **Documentation:** Check [ARCHITECTURE.md](ARCHITECTURE.md)
- **Code Comments:** Read inline documentation

---

## License

By contributing to Mnemosyne, you agree that your contributions will be licensed under the Apache License 2.0.

---

Thank you for contributing to Mnemosyne! May the goddess of memory bless your code. 🏛️✨
