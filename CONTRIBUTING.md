# Contributing to awk-rs

Thank you for your interest in contributing to awk-rs! This document provides guidelines and information for contributors.

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for everyone.

## Getting Started

### Prerequisites

- Rust 1.85.0 or later (for Rust 2024 edition support)
- Git
- A working knowledge of AWK

### Setting Up the Development Environment

```bash
# Clone the repository
git clone https://github.com/quinnjr/rawk.git
cd rawk

# Build the project
cargo build

# Run tests
cargo test

# Run with an AWK program
cargo run -- 'BEGIN { print "Hello, World!" }'
```

## How to Contribute

### Reporting Bugs

1. Check existing issues to avoid duplicates
2. Use the bug report template
3. Include:
   - The AWK program that triggers the bug
   - Input data (if applicable)
   - Expected output (ideally from gawk)
   - Actual awk-rs output
   - Your environment details

### Suggesting Features

1. Check the TODO.md to see if the feature is already planned
2. Use the feature request template
3. Explain the use case and how it fits with AWK standards

### Submitting Code

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes
4. Ensure tests pass: `cargo test`
5. Ensure code is formatted: `cargo fmt`
6. Ensure no clippy warnings: `cargo clippy`
7. Commit with a descriptive message
8. Push and create a pull request

## Development Guidelines

### Code Style

- Follow Rust conventions and idioms
- Use `rustfmt` for formatting
- Address all `clippy` warnings
- Write doc comments for public APIs
- Keep functions focused and small

### Commit Messages

Use conventional commit format:

```
type: description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance tasks

Examples:
```
feat: implement gsub() built-in function
fix: correct field splitting with regex FS
docs: add examples for getline usage
```

### Testing

- Add tests for new features
- Add regression tests for bug fixes
- Ensure compatibility with gawk where applicable
- Run the full test suite before submitting

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### AWK Compatibility

awk-rs aims for 100% POSIX AWK compatibility with GNU AWK extensions:

1. **POSIX First**: Core functionality must match POSIX spec
2. **gawk Extensions**: Clearly document as extensions
3. **Test Against gawk**: Verify output matches gawk

### Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Public API
├── error.rs         # Error types
├── lexer/           # Tokenizer
├── parser/          # Parser and AST
├── ast.rs           # AST definitions
├── value.rs         # AWK value type
└── interpreter/     # Runtime
    ├── mod.rs       # Main interpreter
    ├── expr.rs      # Expression evaluation
    ├── stmt.rs      # Statement execution
    └── builtins.rs  # Built-in functions

tests/
├── e2e.rs           # End-to-end tests
├── gawk_compat.rs   # gawk compatibility tests
└── fixtures/        # Test data files
```

## Review Process

1. All PRs require at least one review
2. CI must pass (tests, clippy, fmt)
3. Documentation must be updated if needed
4. Changes should include tests

## Getting Help

- Open an issue for questions
- Check existing issues and discussions
- Reference the GNU AWK manual for behavior questions

## Recognition

Contributors will be recognized in:
- GitHub contributors list
- Release notes (for significant contributions)

Thank you for contributing to awk-rs!
