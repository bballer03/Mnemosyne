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

---

## Code of Conduct

This project adheres to a code of conduct that all contributors are expected to follow:

- Be respectful and inclusive
- Focus on constructive feedback
- Accept differing viewpoints gracefully
- Show empathy towards other community members

---

## Getting Started

### Prerequisites

- **Rust** 1.70 or later
- **Git**
- **A JVM** (for testing with real heap dumps)
- **Optional:** An OpenAI API key (for AI features)

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
cargo run -- parse test.hprof

# Run with debug logging
RUST_LOG=debug cargo run -- analyze test.hprof

# Run MCP server
cargo run -- serve
```

---

## Project Structure

```
mnemosyne/
│
├── core/                  # Core library crates
│   ├── hprof/            # HProf format parser
│   ├── graph/            # Object graph & dominator tree
│   ├── leaks/            # Leak detection algorithms
│   ├── mapper/           # Source code mapping
│   └── report/           # Report generation
│
├── mcp/                  # MCP server implementation
│   ├── server.rs         # Server entry point
│   └── handlers/         # MCP command handlers
│
├── cli/                  # Command-line interface
│   └── main.rs
│
├── tests/                # Integration tests
│   ├── fixtures/         # Test heap dumps
│   └── integration/
│
├── docs/                 # Documentation
│   └── examples/         # Example code and analyses
│
└── resources/            # Images, diagrams, etc.
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

    #[test]
    fn test_parse_small_heap() {
        let snapshot = parse_heap("tests/fixtures/small.hprof").unwrap();
        assert_eq!(snapshot.total_objects(), 1234);
    }

    #[test]
    fn test_detect_simple_leak() {
        let snapshot = create_test_snapshot();
        let leaks = detect_leaks(&snapshot);
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].class_name, "com.example.LeakyCache");
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
2. **At least one approval** from a maintainer
3. **All comments addressed** or discussed
4. **Conflicts resolved** with main branch

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
