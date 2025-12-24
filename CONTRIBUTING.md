# Contributing to Desk CLI

Thank you for your interest in contributing to Desk CLI! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

### Finding Issues

- Look for issues labeled [`good first issue`](https://github.com/your-org/desk-cli/labels/good%20first%20issue) for beginner-friendly tasks
- Check [`help wanted`](https://github.com/your-org/desk-cli/labels/help%20wanted) for issues where we need assistance
- Feel free to ask questions on any issue before starting work

### Before You Start

1. Check if an issue already exists for your proposed change
2. If not, create an issue to discuss the change before implementing
3. Wait for feedback from maintainers before starting significant work

## Development Setup

### Prerequisites

- **Rust**: 1.75 or later ([rustup.rs](https://rustup.rs))
- **Git**: For version control
- **Just**: Task runner (optional but recommended)

### Clone and Build

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/desk-cli.git
cd desk-cli

# Add upstream remote
git remote add upstream https://github.com/your-org/desk-cli.git

# Install development tools
rustup component add rustfmt clippy

# Build the project
cargo build

# Run tests
cargo test
```

### Recommended Tools

```bash
# Install just (task runner)
cargo install just

# Install cargo-watch for auto-rebuild
cargo install cargo-watch

# Watch for changes and rebuild
cargo watch -x build
```

## Making Changes

### Branch Naming

Create a branch from `develop` with a descriptive name:

```bash
git checkout develop
git pull upstream develop
git checkout -b <type>/<description>
```

Branch types:
- `feature/` — New features
- `bugfix/` — Bug fixes
- `docs/` — Documentation changes
- `refactor/` — Code refactoring
- `test/` — Test additions or fixes

Examples:
- `feature/add-workspace-export`
- `bugfix/fix-git-stash-restore`
- `docs/update-installation-guide`

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:**
- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation only
- `style` — Code style (formatting, semicolons, etc.)
- `refactor` — Code change that neither fixes a bug nor adds a feature
- `perf` — Performance improvement
- `test` — Adding or updating tests
- `build` — Build system or dependencies
- `ci` — CI/CD configuration
- `chore` — Other changes

**Examples:**
```
feat(workspace): add export command for sharing workspaces

fix(git): handle detached HEAD state correctly

docs: update README with new installation options
```

### Code Changes

1. Write clean, readable code following our [coding standards](#coding-standards)
2. Add tests for new functionality
3. Update documentation as needed
4. Ensure all tests pass locally

```bash
# Run all checks before committing
just check

# Or manually:
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Pull Request Process

### Before Submitting

1. Rebase your branch on latest `develop`:
   ```bash
   git fetch upstream
   git rebase upstream/develop
   ```

2. Run all checks:
   ```bash
   just check
   ```

3. Ensure your commits are clean and atomic

### Submitting

1. Push your branch to your fork
2. Create a Pull Request against `develop` branch
3. Fill out the PR template completely
4. Link any related issues

### Review Process

1. Maintainers will review your PR
2. Address any requested changes
3. Once approved, a maintainer will merge your PR

### After Merge

- Delete your feature branch
- Pull the latest `develop` to stay up to date

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting (configuration in `rustfmt.toml`)
- Address all `clippy` warnings
- Write documentation for public APIs

### Code Organization

```
cli/src/
├── main.rs          # Entry point
├── cli/             # CLI argument parsing and commands
├── core/            # Core business logic
├── integrations/    # External integrations (git, editors, etc.)
├── tui/             # Terminal UI components
└── utils/           # Utility functions
```

### Error Handling

- Use `anyhow::Result` for application errors
- Use `thiserror` for library errors with custom types
- Provide helpful error messages for users

### Logging

- Use `tracing` for structured logging
- Use appropriate log levels:
  - `error!` — Errors that affect functionality
  - `warn!` — Potential issues
  - `info!` — High-level operations
  - `debug!` — Detailed debugging info
  - `trace!` — Very verbose tracing

## Testing

### Test Organization

```
cli/
├── src/             # Source code with unit tests
│   └── *.rs         # #[cfg(test)] mod tests { ... }
└── tests/           # Integration tests
    ├── integration/ # Integration test files
    └── fixtures/    # Test fixtures and data
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests with coverage
cargo llvm-cov
```

### Writing Tests

- Write unit tests for individual functions
- Write integration tests for command workflows
- Use descriptive test names
- Test both success and error cases

## Documentation

### Code Documentation

- Document all public APIs with rustdoc
- Include examples in documentation
- Use `///` for item documentation
- Use `//!` for module documentation

```rust
/// Creates a new workspace with the given name.
///
/// # Arguments
///
/// * `name` - The workspace name (e.g., "PROJ-1234")
///
/// # Examples
///
/// ```
/// let workspace = Workspace::new("my-workspace");
/// ```
///
/// # Errors
///
/// Returns an error if the workspace already exists.
pub fn new(name: &str) -> Result<Self> {
    // ...
}
```

### User Documentation

- Update docs in the `docs/` directory
- Keep README.md up to date
- Add changelog entries for user-facing changes

## Questions?

- Open a [Discussion](https://github.com/your-org/desk-cli/discussions)
- Ask in issue comments
- Reach out to maintainers

Thank you for contributing!
