# Changelog

All notable changes to awk-rs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-08-20

Conformance release: a full claim-vs-implementation audit against POSIX and
gawk, with every finding reconciled and verified against gawk output.

### Fixed

#### POSIX conformance
- `BEGIN`/`END` now run once per invocation instead of once per input file;
  `FILENAME` and `FNR` are tracked per file, `NR` accumulates across files
- Bare `getline` and `getline var` now read from the main input stream
  (previously always returned EOF without consuming anything)
- `OFMT`/`CONVFMT` are honored end-to-end: `print`, concatenation,
  comparisons, and array subscripts (previously ignored entirely)
- String literals are pure strings, never numeric strings, so
  `("10" > "9")` is a string comparison, matching gawk
- `$` accepts unary operands: `$-1`, `$+x`, `$++i`, `$!x`
- `-v` assignments process escape sequences (`\t`, `\n`, octal, `\x` hex)
- `system()` flushes buffered output (stdout and open files/pipes) before
  running the command, so output interleaves correctly

#### printf/sprintf
- `%c` with a string argument prints its first character instead of NUL
- `*` width/precision pulled from the argument list
- `+`, space, and `#` flags applied across all numeric conversions;
  width/alignment honored for `%e %E %g %G %o %x %X %c`
- C-style `%e`/`%g` formatting and a proper `%u` conversion

#### Builtins
- `sub`/`gsub`/`gensub` honor `\&` (literal ampersand) and `\\` in
  replacements; `gensub` gains `\1`-`\9` capture-group references
- `length(array)` returns the element count, including arrays passed
  by reference into functions
- `split(s, a, "")` splits into one element per character
- `getline` targets accept array elements and fields
  (`getline arr[i] < file`, `cmd | getline $2`); target subscripts are
  evaluated even at EOF, matching gawk

### Performance
- `sub`/`gsub`/`match`/`split` now use the compiled-regex cache instead
  of recompiling their pattern on every call

### Internal
- printf conversion engine extracted to `src/fmt.rs` and shared with the
  `OFMT`/`CONVFMT` path
- Removed dead AST variants (`Expr::Concat`, `Stmt::Getline`) and the
  unreachable `call_getline` stub

## [0.1.0] - 2026-01-02

Initial release of awk-rs - a 100% POSIX-compatible AWK implementation in Rust.

### Features

#### POSIX AWK Compatibility
- Complete lexer supporting all AWK token types
- Recursive descent parser with correct operator precedence
- Tree-walking interpreter with proper AWK semantics
- Dynamic typing with automatic string/number coercion

#### Built-in Variables
- Record/field variables: `$0`, `$n`, `NF`, `NR`, `FNR`
- Separators: `FS`, `RS`, `OFS`, `ORS`, `SUBSEP`
- File/program info: `FILENAME`, `ARGC`, `ARGV`, `ENVIRON`
- Formatting: `CONVFMT`, `OFMT`
- Regex state: `RSTART`, `RLENGTH`

#### Built-in Functions
- String: `length`, `substr`, `index`, `split`, `sub`, `gsub`, `match`, `sprintf`, `tolower`, `toupper`
- Math: `sin`, `cos`, `atan2`, `exp`, `log`, `sqrt`, `int`, `rand`, `srand`
- I/O: `print`, `printf`, `getline`, `close`, `fflush`, `system`

#### GNU AWK Extensions
- Time functions: `systime`, `mktime`, `strftime`
- Extended functions: `gensub`, `patsplit`, `asort`, `asorti`
- Special patterns: `BEGINFILE`, `ENDFILE`
- Field modes: `FPAT`, `FIELDWIDTHS`
- Info array: `PROCINFO`
- CLI modes: `--posix`, `--traditional`

#### Language Features
- User-defined functions with local scope and recursion
- Associative arrays with multi-dimensional subscripts
- All control flow: `if`/`else`, `while`, `for`, `for-in`, `do-while`
- Flow control: `break`, `continue`, `next`, `nextfile`, `exit`, `return`
- Pattern types: expression, regex, range, `BEGIN`, `END`, compound
- Output redirection: `>`, `>>`, `|`
- Input: `getline` variants (from file, pipe, variable)
- `delete` for array elements and entire arrays

#### CLI Interface
- `-F fs`: Set field separator
- `-v var=val`: Set variable
- `-f progfile`: Read program from file
- `--posix`: Strict POSIX mode
- `--traditional`: Traditional AWK mode
- `--help`, `--version`

### Quality

#### Testing
- 639 total tests
  - 170 unit tests (lexer, parser, interpreter, value system)
  - 412 end-to-end tests (complete AWK programs)
  - 19 CLI integration tests
  - 34 gawk compatibility tests
  - 4 documentation tests
- 86% library code coverage

#### Performance
- Optimized field splitting (byte-based for single-char FS)
- Compiled regex caching
- Fast integer-to-string conversion
- Release profile with LTO and single codegen unit

#### Infrastructure
- GitHub Actions CI (Linux, macOS, Windows)
- Automated releases with binary builds
- CodeQL security scanning
- Dependabot for dependency updates
- Criterion benchmarks
- Fuzzing targets (lexer, parser, interpreter)

### License

Dual-licensed under MIT and Apache 2.0.
Copyright (c) 2026 Joseph R. Quinn.

---

## Version History

- **0.1.0**: Initial release with full POSIX AWK and GNU AWK extension support
