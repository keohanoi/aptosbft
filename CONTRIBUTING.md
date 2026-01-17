# Contributing to AptosBFT

Thank you for your interest in contributing to AptosBFT! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Participants are expected to be respectful, collaborative, and constructive.

## Getting Started

### Prerequisites

- **Rust**: Install via [rustup](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Tooling**:
  ```bash
  cargo install cargo-tarpaulin  # For coverage reports
  cargo install cargo-watch       # For development
  ```

### Repository Setup

```bash
# Fork the repository on GitHub
git clone https://github.com/YOUR_USERNAME/aptosbft.git
cd aptosbft
git remote add upstream https://github.com/aptos-labs/aptosbft.git
```

### Building

```bash
# Build all workspace members
cargo build --workspace

# Run tests to verify setup
cargo test --workspace
```

## Development Workflow

### 1. Create a Branch

```bash
git checkout main
git pull upstream main
git checkout -b feature/my-feature
```

### 2. Make Changes

Following the coding standards below, make your changes with appropriate tests.

### 3. Test Your Changes

```bash
# Run all tests
cargo test --workspace

# Run tests with coverage
cargo tarpaulin --workspace --out Html --output-dir target/tarpaulin

# Run clippy
cargo clippy --workspace -- -D warnings
```

### 4. Commit Your Changes

Follow the [Commit Message Guidelines](#commit-message-guidelines) below.

### 5. Push and Create Pull Request

```bash
git push origin feature/my-feature
# Then create a PR on GitHub
```

## Coding Standards

### Rust Style Guide

Follow the official [Rust style guide](https://rust-lang.github.io/rustfmt/):

```bash
# Format all code
cargo fmt --workspace
```

### Code Organization

- **Modules**: Organize code into logical modules with clear responsibilities
- **Traits**: Use traits to define generic interfaces
- **Documentation**: Document all public APIs with rustdoc comments
- **Error Handling**: Use `Result<T, Error>` for fallible operations

### Documentation Requirements

All public APIs must have rustdoc documentation:

```rust
/// Creates a new safety rules instance.
///
/// # Parameters
///
/// * `epoch` - The initial epoch number
///
/// # Returns
///
/// A new `TwoChainSafetyRules` instance
///
/// # Example
///
/// ```rust
/// use consensus_core::safety_rules::TwoChainSafetyRules;
///
/// let safety_rules = TwoChainSafetyRules::<MyBlock, MyVote>::new(0);
/// ```
pub fn new(epoch: u64) -> Self {
    // ...
}
```

### Naming Conventions

- **Types**: `PascalCase` (e.g., `TwoChainSafetyRules`)
- **Functions**: `snake_case` (e.g., `safe_to_vote`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `GENESIS_EPOCH`)
- **Acronyms**: Treat as regular words (e.g., `QuorumCert` not `Qc`)

## Testing Guidelines

### Test Coverage

We maintain **85%+ test coverage**. All new code must include tests that maintain this standard.

```bash
# Check coverage
cargo tarpaulin --workspace --out Html --output-dir target/tarpaulin
```

### Unit Tests

Write unit tests in the same module using the `#[cfg(test)]` attribute:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_to_vote_normal_progression() {
        let safety_rules = TwoChainSafetyRules::<TestBlock, TestVote>::new(0);
        let proposal = create_test_proposal(5, 4);

        assert!(safety_rules.safe_to_vote(&proposal, None).is_ok());
    }
}
```

### Integration Tests

Place integration tests in the `tests/` directory:

```rust
// tests/safety_integration.rs
use consensus_core::safety_rules::TwoChainSafetyRules;

#[test]
fn test_2_chain_commit_rule() {
    // Integration test for commit rule
}
```

### Safety-Critical Tests

When modifying safety-critical code (voting rules, commit rules, etc.):

1. Add comprehensive tests for all edge cases
2. Include tests for overflow scenarios (u64::MAX)
3. Test error conditions explicitly
4. Verify the 2-chain commit rule invariants

### Mock Implementations

Use the mock implementations in `testing/` module for testing:

```rust
use consensus_core::testing::{
    MockBlock, MockVote, MockValidator,
    MockHash, MockSignature,
};

#[test]
fn test_with_mock_types() {
    let block = MockBlock::genesis();
    let validator = MockValidator::new(1, 100);
    // ...
}
```

## Commit Message Guidelines

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **test**: Test additions or changes
- **refactor**: Code refactoring (no functional changes)
- **perf**: Performance improvements
- **style**: Code style changes (formatting, etc.)
- **chore**: Build process, tooling, etc.

### Examples

```
feat(safety_rules): add 3-chain commit rule variant

Implement an alternative commit rule that requires 3 consecutive
certified blocks for enhanced safety in high-risk environments.

Closes #123
```

```
fix(votes): prevent duplicate votes from same validator

Add tracking of voted validators to prevent duplicate votes from
being counted multiple times in quorum formation.

Fixes #456
```

## Pull Request Process

### Before Submitting

1. **Code Review**: Review your own code first
2. **Tests**: Ensure all tests pass
3. **Coverage**: Maintain 85%+ coverage
4. **Docs**: Update documentation as needed
5. **Clippy**: Fix all clippy warnings

### PR Title

Use a clear, descriptive title following conventional commits format:

```
feat(safety_rules): add timeout certificate voting support
fix(votes): resolve race condition in vote aggregation
docs: add API documentation for RoundManager
```

### PR Description

Include:

- **Summary**: Brief description of changes
- **Motivation**: Why this change is needed
- **Testing**: How the changes were tested
- **Breaking Changes**: Any breaking changes (if applicable)
- **Related Issues**: Links to related issues

### PR Checklist

- [ ] Tests pass for all workspace members
- [ ] Coverage is maintained at 85%+
- [ ] Documentation is updated
- [ ] Clippy produces no warnings
- [ ] Code is formatted with `cargo fmt`
- [ ] Commits follow conventional commits format
- [ ] PR description is clear and comprehensive

### Review Process

1. **Automated Checks**: CI runs tests, coverage, and linting
2. **Manual Review**: Maintainers review the code
3. **Address Feedback**: Make requested changes
4. **Approval**: PR is approved and merged

## Safety-Critical Code

When modifying safety-critical components:

### Safety Rules (`safety_rules/`)

- **High Review Required**: All changes require thorough review
- **Formal Verification**: Consider formal verification for complex changes
- **Test Coverage**: Must have 100% coverage for modified code paths
- **Documentation**: Must include detailed safety invariants

### Vote Processing (`votes/`)

- **Correctness**: Verify vote counting logic is correct
- **Edge Cases**: Test with duplicate votes, equivocation, etc.
- **Performance**: Ensure O(1) or O(log n) operations

### Timeout Handling (`timeout/`)

- **Liveness**: Ensure timeouts don't cause deadlocks
- **Recovery**: Verify crash recovery works correctly
- **Edge Cases**: Test with rapid timeout sequences

## Running Tests Locally

### All Tests

```bash
cargo test --workspace
```

### Specific Crate

```bash
cargo test -p consensus-core
cargo test -p consensus-traits
```

### Specific Test

```bash
cargo test -p consensus-core test_safe_to_vote
```

### Watch Mode (Development)

```bash
cargo watch -x 'cargo test --workspace'
```

## Documentation

### Building Docs

```bash
cargo doc --workspace --no-deps
```

### Building Docs with Private Items

```bash
cargo doc --workspace --document-private-items
```

### Opening Docs Locally

```bash
cargo doc --workspace --no-deps --open
```

## Performance Considerations

When optimizing for performance:

1. **Benchmark First**: Measure before optimizing
2. **Profile**: Use `cargo flamegraph` to identify hot paths
3. **Test**: Ensure optimizations don't break correctness
4. **Document**: Explain why optimizations were needed

## Asking for Help

### Communication Channels

- **GitHub Issues**: For bugs and feature requests
- **GitHub Discussions**: For general questions and ideas
- **Pull Requests**: For code reviews and contributions

### When Asking Questions

1. **Search First**: Check existing issues and documentation
2. **Be Specific**: Include code examples and error messages
3. **Provide Context**: Explain what you're trying to accomplish
4. **Show Effort**: Demonstrate what you've already tried

## License

By contributing to AptosBFT, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).

## Recognition

Contributors are recognized in the [CONTRIBUTORS.md](CONTRIBUTORS.md) file.

Thank you for contributing to AptosBFT! 🎉
