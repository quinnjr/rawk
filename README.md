# awk-rs

[![CI](https://github.com/quinnjr/awk-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/quinnjr/awk-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/awk-rs.svg)](https://crates.io/crates/awk-rs)
[![Documentation](https://docs.rs/awk-rs/badge.svg)](https://docs.rs/awk-rs)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

A 100% POSIX-compatible AWK implementation in Rust with GNU AWK (gawk) extension support.

## Overview

**awk-rs** aims to be a drop-in replacement for AWK that faithfully implements the POSIX AWK specification while also supporting common GNU AWK extensions. Written in Rust for reliability and performance.

### Goals

- Complete POSIX AWK compatibility
- GNU AWK (gawk) extension support
- Identical output behavior to existing AWK implementations
- Clear, informative error messages
- Modern Rust codebase with minimal dependencies

## Installation

### From Source

```bash
git clone https://github.com/quinnjr/awk-rs.git
cd awk-rs
cargo build --release
```

The binary will be at `target/release/awk-rs`.

### From crates.io

```bash
cargo install awk-rs
```

## Usage

```bash
# Run an AWK program directly
awk-rs 'BEGIN { print "Hello, World!" }'

# Process files
awk-rs '{ print $1 }' file.txt

# Set field separator
awk-rs -F: '{ print $1 }' /etc/passwd

# Set variables before execution
awk-rs -v name="Alice" 'BEGIN { print "Hello, " name }'

# Run program from file
awk-rs -f program.awk input.txt
```

### Command-Line Options

| Option | Description |
|--------|-------------|
| `-F fs` | Set field separator (can be a regex) |
| `-v var=val` | Assign variable before program execution |
| `-f progfile` | Read AWK program from file |
| `--posix` | Strict POSIX mode (disable extensions) |
| `--version` | Print version information |
| `--help` | Print usage help |

## Examples

### Print specific columns

```bash
echo "one two three" | awk-rs '{ print $2 }'
# Output: two
```

### Sum numbers in a file

```bash
awk-rs '{ sum += $1 } END { print sum }' numbers.txt
```

### Filter lines with pattern

```bash
awk-rs '/error/ { print }' logfile.txt
```

### Field separator and formatting

```bash
awk-rs -F: '{ printf "User: %-20s Shell: %s\n", $1, $7 }' /etc/passwd
```

### Associative arrays

```bash
awk-rs '{ count[$1]++ } END { for (word in count) print word, count[word] }' words.txt
```

## Compatibility

awk-rs aims for 100% compatibility with:

- **POSIX AWK** - The baseline specification
- **gawk** - GNU AWK extensions
- **mawk** - Performance-oriented AWK
- **nawk** - One True AWK

### Supported Features

- All POSIX AWK operators and statements
- User-defined functions
- Associative arrays
- Regular expression patterns
- Field splitting and manipulation
- Printf formatting
- I/O redirection and pipes
- Built-in string and math functions
- GNU AWK extensions (optional)

## Building & Development

### Requirements

- Rust 1.85+ (2024 edition)
- Cargo

### Build

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo test           # Run tests
```

### Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Public API
├── error.rs         # Error types
├── lexer/           # Tokenization
├── parser/          # Parsing & AST
├── interpreter/     # Execution engine
├── runtime/         # Variables, arrays, fields
└── io/              # Input/output handling
```

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

Copyright © 2026 Joseph R. Quinn

## Testing

awk-rs has comprehensive test coverage:

```bash
cargo test          # Run all 639 tests
cargo test --test e2e       # E2E tests (412 tests)
cargo test --test gawk_compat  # gawk compatibility tests (34 tests)
```

### Test Coverage

- Unit tests: 170 (lexer, parser, interpreter, value system)
- E2E tests: 412 (complete AWK programs)
- CLI tests: 19 (command-line interface)
- Compatibility tests: 34 (gawk comparison)
- Doc tests: 4 (API examples)
- **Coverage: 86%** (library code)

## Benchmarking

```bash
cargo bench         # Run Criterion benchmarks
```

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Quick checklist:
1. Ensure POSIX AWK compatibility
2. Add tests for new functionality
3. Run `cargo fmt` and `cargo clippy`
4. Check [TODO.md](TODO.md) for the roadmap

## Security

See [SECURITY.md](SECURITY.md) for security policy and reporting vulnerabilities.

## Acknowledgments

- The original AWK authors: Aho, Weinberger, and Kernighan
- The GNU AWK project for comprehensive documentation
- The Rust community
