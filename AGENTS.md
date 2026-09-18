<!-- converted from Cursor rules -->

## Cursor rule: `.cursor/rules/rawk.mdc`

_Comprehensive rules for rawk - a 100% AWK-compatible implementation in Rust_

Applies to: `["**/*.rs", "Cargo.toml", "**/*.awk"]`

# RAWK Project Rules

## Project Overview

RAWK is a 100% POSIX-compatible AWK implementation in Rust with GNU AWK (gawk) extension support. The goal is complete behavioral compatibility with existing AWK implementations.

## Architecture

### Module Structure

Organize code into these core modules:

```
src/
├── main.rs          # CLI entry point only
├── lib.rs           # Public API and module exports
├── error.rs         # Error types and Result aliases
├── lexer/
│   ├── mod.rs       # Lexer implementation
│   └── tokens.rs    # Token type definitions
├── parser/
│   ├── mod.rs       # Parser implementation
│   └── grammar.rs   # Grammar helpers if needed
├── ast.rs           # AST node definitions
├── value.rs         # AWK value type and coercion
├── interpreter/
│   ├── mod.rs       # Main interpreter loop
│   ├── expr.rs      # Expression evaluation
│   ├── stmt.rs      # Statement execution
│   └── builtins.rs  # Built-in function implementations
├── runtime/
│   ├── mod.rs       # Runtime environment
│   ├── variables.rs # Variable/symbol table
│   ├── arrays.rs    # Associative array implementation
│   └── fields.rs    # Field splitting and $n access
├── io/
│   ├── mod.rs       # I/O subsystem
│   ├── input.rs     # Record reading
│   └── output.rs    # Print/printf and redirection
└── regex.rs         # Regex handling wrapper
```

### Design Principles

1. **Separation of concerns**: Keep lexer, parser, and interpreter cleanly separated
2. **Single responsibility**: Each module handles one aspect of AWK
3. **Minimal dependencies**: Prefer stdlib; external crates only when necessary
4. **No unnecessary abstractions**: AWK is simple; the implementation should be too

## Rust Conventions

### Error Handling

- Define a custom `Error` enum in `src/error.rs`
- Use `thiserror` for error derivation if adding dependencies
- Create a type alias: `type Result<T> = std::result::Result<T, Error>`
- Errors must include location info (line, column) for parse/runtime errors
- Use `?` operator for propagation; avoid `.unwrap()` except in tests

```rust
#[derive(Debug)]
pub enum Error {
    Lexer { msg: String, line: usize, col: usize },
    Parser { msg: String, line: usize, col: usize },
    Runtime { msg: String },
    Io(std::io::Error),
}
```

### Naming Conventions

- Types: `PascalCase` (e.g., `TokenKind`, `AstNode`, `Value`)
- Functions/methods: `snake_case` (e.g., `parse_expression`, `evaluate`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_FS`, `DEFAULT_OFS`)
- Private helpers: prefix with underscore only if truly internal

### Code Style

- Maximum line length: 100 characters
- Use `rustfmt` defaults
- Prefer `match` over chains of `if let`
- Use `impl` blocks grouped by: constructors, public methods, private methods
- Document public APIs with `///` doc comments
- Use `#[must_use]` on functions returning values that shouldn't be ignored

### Testing

- Unit tests go in the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests go in `tests/` directory
- Test names should describe the behavior: `test_field_splitting_with_regex_fs`
- Use `assert_eq!` with descriptive messages
- Create test fixtures in `tests/fixtures/` for AWK scripts

## AWK Compatibility Rules

### Behavioral Accuracy

- **POSIX first**: Default behavior must match POSIX AWK specification
- **Then gawk**: Implement gawk extensions but document them as such
- **Test against real AWK**: Validate output matches `gawk` and `mawk`

### Value Semantics

AWK has unique value semantics that MUST be preserved:

1. **Uninitialized variables**: Coerce to `""` (string) or `0` (numeric) based on context
2. **Numeric strings**: `"42"` is numeric when compared; `"42abc"` has numeric value `42`
3. **Comparison rules**:
   - Both numeric → numeric comparison
   - Both strings → string comparison
   - Mixed → depends on context and "numeric string" status
4. **Concatenation**: Adjacent expressions concatenate (space is optional operator)

### Field Handling

- `$0` is the entire record
- `$1` through `$NF` are fields
- Assigning to `$n` where `n > NF` extends NF
- Modifying any field rebuilds `$0` with `OFS`
- Modifying `$0` re-splits into fields using `FS`
- `FS = " "` (default) splits on runs of whitespace and trims

### Regular Expressions

- Use POSIX Extended Regular Expressions (ERE)
- Support character classes: `[:alpha:]`, `[:digit:]`, etc.
- Replacement strings: `&` = matched text, `\1`-`\9` = captured groups
- Regex literals: `/pattern/` - disambiguate from division by context

### I/O Behavior

- `print` outputs to stdout by default (or redirected target)
- Each unique redirection target (`>file`, `>>file`, `|cmd`) opens once and stays open
- `getline` returns: 1 (success), 0 (EOF), -1 (error)
- Pipes and files closed with `close()` or at program end

## Implementation Guidelines

### Lexer

- Track position (line, column) for every token
- Handle all escape sequences in strings
- Regex vs division: `/` after value-producing expression = division
- Line continuation: `\` at end of line joins with next

### Parser

- Use recursive descent parsing
- Operator precedence (low to high):
  1. `?:` (ternary)
  2. `||`
  3. `&&`
  4. `in` (array membership)
  5. `~`, `!~` (regex match)
  6. `<`, `<=`, `>`, `>=`, `!=`, `==`
  7. Concatenation (implicit)
  8. `+`, `-`
  9. `*`, `/`, `%`
  10. `^` (exponentiation, right-associative)
  11. `!`, unary `+`, unary `-`
  12. `++`, `--` (prefix)
  13. `$` (field access)
  14. `++`, `--` (postfix)

### Interpreter

- Tree-walking interpreter is fine for AWK (most scripts are small)
- Consider bytecode compilation only if performance is critical
- Maintain proper scoping for user-defined functions
- Arrays passed by reference; scalars by value

## Performance Considerations

- Field splitting is hot path - optimize it
- Pre-compile regex patterns; cache them
- Use `Cow<str>` for strings that may or may not be modified
- Avoid allocations in the main record-processing loop where possible
- Profile with real-world AWK scripts before optimizing

## CLI Interface

Match gawk's command-line interface:

```
rawk [OPTIONS] ['program'] [file ...]
rawk [OPTIONS] -f progfile [file ...]
```

Required options:
- `-F fs` : Set field separator
- `-v var=val` : Set variable before execution
- `-f progfile` : Read program from file

Recommended options:
- `--posix` : Strict POSIX mode
- `--traditional` : Traditional AWK mode (no gawk extensions)
- `--version` : Version info
- `--help` : Usage help

## Git Practices

- Commit messages: `type: description` (e.g., `feat: implement substr()`)
- Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
- Keep commits atomic and focused
- Reference TODO.md items in commits when completing them

## Dependencies Policy

Minimize external dependencies. Approved crates:

- `regex` - Regular expressions (required)
- `clap` - CLI parsing (optional, can use manual parsing)
- `thiserror` - Error derivation (optional)

Avoid:
- Heavy frameworks
- Async runtimes (AWK is synchronous)
- Unnecessary derive macros

