mod builtins;
mod expr;
pub mod stmt;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};

use crate::ast::*;
use crate::error::{Error, Result};
use crate::value::Value;

use regex::Regex;

/// Input source for getline from pipe
pub struct PipeInput {
    #[allow(dead_code)]
    child: Child,
    reader: BufReader<ChildStdout>,
}

/// Output destination for print/printf redirection
pub enum OutputFile {
    File(File),
    Pipe(ChildStdin),
}

impl Write for OutputFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            OutputFile::File(f) => f.write(buf),
            OutputFile::Pipe(p) => p.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            OutputFile::File(f) => f.flush(),
            OutputFile::Pipe(p) => p.flush(),
        }
    }
}

/// The AWK interpreter runtime
pub struct Interpreter<'a> {
    /// The parsed program
    program: &'a Program,

    /// Global variables
    pub(crate) variables: HashMap<String, Value>,

    /// Associative arrays
    pub(crate) arrays: HashMap<String, HashMap<String, Value>>,

    /// User-defined functions
    pub(crate) functions: HashMap<String, &'a FunctionDef>,

    /// Built-in variables
    /// Field separator (FS)
    pub(crate) fs: String,
    /// Output field separator (OFS)
    pub(crate) ofs: String,
    /// Record separator (RS)
    pub(crate) rs: String,
    /// Output record separator (ORS)
    pub(crate) ors: String,
    /// Number format for output (OFMT)
    pub(crate) ofmt: String,
    /// Conversion format (CONVFMT)
    pub(crate) convfmt: String,
    /// Subscript separator (SUBSEP)
    pub(crate) subsep: String,
    /// Field pattern (FPAT) - gawk extension
    pub(crate) fpat: String,
    /// Fixed field widths (FIELDWIDTHS) - gawk extension
    pub(crate) fieldwidths: String,

    /// Mode flags
    pub(crate) posix_mode: bool,
    pub(crate) traditional_mode: bool,

    /// Current record ($0)
    pub(crate) record: String,
    /// Current fields ($1, $2, ...)
    pub(crate) fields: Vec<String>,
    /// Number of fields (NF)
    pub(crate) nf: usize,
    /// Record number (NR)
    pub(crate) nr: usize,
    /// File record number (FNR)
    pub(crate) fnr: usize,
    /// Current filename (FILENAME)
    pub(crate) filename: String,

    /// RSTART and RLENGTH from match()
    pub(crate) rstart: usize,
    pub(crate) rlength: i32,

    /// Control flow flags
    should_exit: bool,
    exit_code: i32,
    should_next: bool,
    should_nextfile: bool,

    /// Open files for output redirection
    pub(crate) output_files: HashMap<String, OutputFile>,

    /// Open files for input (getline)
    pub(crate) input_files: HashMap<String, BufReader<File>>,

    /// Open pipes for input (getline from command)
    pub(crate) pipes: HashMap<String, PipeInput>,

    /// Compiled regex cache
    pub(crate) regex_cache: HashMap<String, Regex>,

    /// Range pattern state (for /start/,/end/ patterns)
    range_states: HashMap<usize, bool>,

    /// Random number generator state
    pub(crate) rand_seed: u64,
    pub(crate) rand_state: u64,

    /// Command line arguments (ARGC, ARGV)
    pub(crate) argc: usize,
    pub(crate) argv: Vec<String>,

    /// Environment variables (ENVIRON)
    pub(crate) environ: HashMap<String, String>,

    /// Array aliases for pass-by-reference in functions
    /// Maps parameter name -> actual array name
    pub(crate) array_aliases: HashMap<String, String>,
}

impl<'a> Interpreter<'a> {
    pub fn new(program: &'a Program) -> Self {
        let mut functions = HashMap::new();
        for func in &program.functions {
            functions.insert(func.name.clone(), func);
        }

        // Initialize environment variables
        let environ: HashMap<String, String> = std::env::vars().collect();

        // Initialize random seed from current time
        use std::time::{SystemTime, UNIX_EPOCH};
        let rand_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);

        Self {
            program,
            variables: HashMap::new(),
            arrays: HashMap::new(),
            functions,
            fs: " ".to_string(),
            ofs: " ".to_string(),
            rs: "\n".to_string(),
            ors: "\n".to_string(),
            ofmt: "%.6g".to_string(),
            convfmt: "%.6g".to_string(),
            subsep: "\x1c".to_string(),
            fpat: String::new(),
            fieldwidths: String::new(),
            posix_mode: false,
            traditional_mode: false,
            record: String::new(),
            fields: Vec::new(),
            nf: 0,
            nr: 0,
            fnr: 0,
            filename: String::new(),
            rstart: 0,
            rlength: -1,
            should_exit: false,
            exit_code: 0,
            should_next: false,
            should_nextfile: false,
            output_files: HashMap::new(),
            input_files: HashMap::new(),
            pipes: HashMap::new(),
            regex_cache: HashMap::new(),
            range_states: HashMap::new(),
            rand_seed,
            rand_state: rand_seed,
            argc: 0,
            argv: Vec::new(),
            environ,
            array_aliases: HashMap::new(),
        }
    }

    /// Set command line arguments (ARGC and ARGV)
    pub fn set_args(&mut self, args: Vec<String>) {
        self.argc = args.len();
        self.argv = args;
    }

    /// Set the field separator
    pub fn set_fs(&mut self, fs: &str) {
        self.fs = fs.to_string();
        // Clear FPAT and FIELDWIDTHS when FS is set
        self.fpat.clear();
        self.fieldwidths.clear();
    }

    /// Set POSIX strict mode
    pub fn set_posix_mode(&mut self, enabled: bool) {
        self.posix_mode = enabled;
        if enabled {
            self.traditional_mode = false;
        }
    }

    /// Set traditional AWK mode (no gawk extensions)
    pub fn set_traditional_mode(&mut self, enabled: bool) {
        self.traditional_mode = enabled;
        if enabled {
            self.posix_mode = false;
        }
    }

    /// Set a variable before execution
    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.variables
            .insert(name.to_string(), Value::from_string(value.to_string()));
    }

    /// Set the current filename (FILENAME)
    pub fn set_filename(&mut self, filename: &str) {
        self.filename = filename.to_string();
    }

    /// Run the AWK program with given input
    pub fn run<R: BufRead, W: Write>(&mut self, inputs: Vec<R>, output: &mut W) -> Result<i32> {
        // Execute BEGIN rules
        for rule in &self.program.rules {
            if matches!(&rule.pattern, Some(Pattern::Begin)) {
                if let Some(action) = &rule.action {
                    self.execute_block(action, output)?;
                }
                if self.should_exit {
                    return Ok(self.exit_code);
                }
            }
        }

        // Process input files
        for input in inputs {
            self.fnr = 0;

            // Execute BEGINFILE rules (gawk extension)
            for rule in &self.program.rules {
                if matches!(&rule.pattern, Some(Pattern::BeginFile)) {
                    if let Some(action) = &rule.action {
                        self.execute_block(action, output)?;
                    }
                    if self.should_exit {
                        return Ok(self.exit_code);
                    }
                }
            }

            self.process_input(input, output)?;

            // Execute ENDFILE rules (gawk extension)
            for rule in &self.program.rules {
                if matches!(&rule.pattern, Some(Pattern::EndFile)) {
                    if let Some(action) = &rule.action {
                        self.execute_block(action, output)?;
                    }
                    if self.should_exit {
                        return Ok(self.exit_code);
                    }
                }
            }

            if self.should_exit {
                return Ok(self.exit_code);
            }
        }

        // Execute END rules
        for rule in &self.program.rules {
            if let (Some(Pattern::End), Some(action)) = (&rule.pattern, &rule.action) {
                self.execute_block(action, output)?;
            }
        }

        Ok(self.exit_code)
    }

    fn process_input<R: BufRead, W: Write>(&mut self, mut input: R, output: &mut W) -> Result<()> {
        // Check for paragraph mode (RS = "")
        if self.rs.is_empty() {
            return self.process_input_paragraph_mode(input, output);
        }

        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = input.read_line(&mut line).map_err(Error::Io)?;
            if bytes_read == 0 {
                break; // EOF
            }

            // Remove record separator
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            self.nr += 1;
            self.fnr += 1;
            self.set_record(&line);

            self.process_current_record(output)?;

            if self.should_nextfile {
                self.should_nextfile = false;
                break;
            }

            if self.should_exit {
                break;
            }
        }

        Ok(())
    }

    /// Process input in paragraph mode (RS = "")
    /// Blank lines separate records; multiple blank lines count as one separator
    fn process_input_paragraph_mode<R: BufRead, W: Write>(
        &mut self,
        mut input: R,
        output: &mut W,
    ) -> Result<()> {
        let mut line = String::new();
        let mut record = String::new();
        let mut in_record = false;

        loop {
            line.clear();
            let bytes_read = input.read_line(&mut line).map_err(Error::Io)?;

            // Check if line is blank (empty or only whitespace)
            let is_blank = line.trim().is_empty();

            if bytes_read == 0 {
                // EOF - process any remaining record
                if !record.is_empty() {
                    // Remove trailing newline
                    while record.ends_with('\n') || record.ends_with('\r') {
                        record.pop();
                    }
                    self.nr += 1;
                    self.fnr += 1;
                    self.set_record(&record);
                    self.process_current_record(output)?;
                }
                break;
            }

            if is_blank {
                // Blank line - end of record if we're in one
                if in_record && !record.is_empty() {
                    // Remove trailing newline
                    while record.ends_with('\n') || record.ends_with('\r') {
                        record.pop();
                    }
                    self.nr += 1;
                    self.fnr += 1;
                    self.set_record(&record);
                    self.process_current_record(output)?;

                    record.clear();
                    in_record = false;

                    if self.should_nextfile || self.should_exit {
                        break;
                    }
                }
            } else {
                // Non-blank line - add to record
                if in_record {
                    record.push('\n');
                }
                // Remove trailing newline from line before adding
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                record.push_str(&line);
                in_record = true;
            }
        }

        if self.should_nextfile {
            self.should_nextfile = false;
        }

        Ok(())
    }

    /// Process the current record through all matching rules
    fn process_current_record<W: Write>(&mut self, output: &mut W) -> Result<()> {
        for (idx, rule) in self.program.rules.iter().enumerate() {
            // Skip special patterns that are handled separately
            if matches!(
                &rule.pattern,
                Some(Pattern::Begin)
                    | Some(Pattern::End)
                    | Some(Pattern::BeginFile)
                    | Some(Pattern::EndFile)
            ) {
                continue;
            }

            let matches = self.pattern_matches(&rule.pattern, idx)?;
            if matches {
                if let Some(action) = &rule.action {
                    self.execute_block(action, output)?;
                } else {
                    // Default action is to print $0
                    writeln!(output, "{}", self.record).map_err(Error::Io)?;
                }
            }

            if self.should_next {
                self.should_next = false;
                break;
            }

            if self.should_nextfile || self.should_exit {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn set_record(&mut self, record: &str) {
        self.record = record.to_string();
        self.split_fields();
    }

    fn split_fields(&mut self) {
        self.fields.clear();

        if self.record.is_empty() {
            self.nf = 0;
            return;
        }

        // Pre-estimate capacity to reduce reallocations
        let estimated_fields = self.record.len() / 8 + 1;
        self.fields.reserve(estimated_fields.min(64));

        // Check for FPAT (field pattern) - gawk extension
        if !self.fpat.is_empty() && !self.posix_mode && !self.traditional_mode {
            self.split_fields_fpat();
            return;
        }

        // Check for FIELDWIDTHS - gawk extension
        if !self.fieldwidths.is_empty() && !self.posix_mode && !self.traditional_mode {
            self.split_fields_widths();
            return;
        }

        // Standard FS-based splitting
        if self.fs == " " {
            // Special case: split on runs of whitespace, trimming leading/trailing
            // Use byte-based iteration for ASCII optimization
            self.fields
                .extend(self.record.split_whitespace().map(String::from));
        } else if self.fs.len() == 1 {
            // Single character separator - most common case, optimize for it
            let sep = self.fs.as_bytes()[0];
            let bytes = self.record.as_bytes();
            let mut start = 0;

            for (i, &b) in bytes.iter().enumerate() {
                if b == sep {
                    self.fields.push(self.record[start..i].to_string());
                    start = i + 1;
                }
            }
            // Don't forget the last field
            self.fields.push(self.record[start..].to_string());
        } else {
            // Regex separator - cache the compiled regex
            let fs = self.fs.clone();
            let record = self.record.clone();
            if let Some(regex) = self.regex_cache.get(&fs) {
                self.fields.extend(regex.split(&record).map(String::from));
            } else if let Ok(regex) = Regex::new(&fs) {
                self.fields.extend(regex.split(&record).map(String::from));
                self.regex_cache.insert(fs, regex);
            } else {
                // If regex fails, treat as literal string
                self.fields.extend(record.split(&fs).map(String::from));
            }
        }

        self.nf = self.fields.len();
    }

    /// Split fields using FPAT (field pattern matching)
    fn split_fields_fpat(&mut self) {
        let fpat = self.fpat.clone();
        let record = self.record.clone();

        if let Some(regex) = self.regex_cache.get(&fpat) {
            for mat in regex.find_iter(&record) {
                self.fields.push(mat.as_str().to_string());
            }
        } else if let Ok(regex) = Regex::new(&fpat) {
            for mat in regex.find_iter(&record) {
                self.fields.push(mat.as_str().to_string());
            }
            self.regex_cache.insert(fpat, regex);
        }

        self.nf = self.fields.len();
    }

    /// Split fields using FIELDWIDTHS (fixed-width fields)
    fn split_fields_widths(&mut self) {
        let widths: Vec<usize> = self
            .fieldwidths
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        let mut pos = 0;
        let chars: Vec<char> = self.record.chars().collect();

        for width in widths {
            if pos >= chars.len() {
                break;
            }
            let end = (pos + width).min(chars.len());
            let field: String = chars[pos..end].iter().collect();
            self.fields.push(field);
            pos = end;
        }

        self.nf = self.fields.len();
    }

    #[inline]
    pub(crate) fn get_field(&self, index: usize) -> String {
        if index == 0 {
            self.record.clone()
        } else if index <= self.fields.len() {
            self.fields[index - 1].clone()
        } else {
            String::new()
        }
    }

    /// Get field reference without cloning (for read-only access)
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn get_field_ref(&self, index: usize) -> &str {
        if index == 0 {
            &self.record
        } else if index <= self.fields.len() {
            &self.fields[index - 1]
        } else {
            ""
        }
    }

    pub(crate) fn set_field(&mut self, index: usize, value: String) {
        if index == 0 {
            self.record = value;
            self.split_fields();
        } else {
            // Extend fields if necessary
            while self.fields.len() < index {
                self.fields.push(String::new());
            }
            self.fields[index - 1] = value;
            self.nf = self.fields.len();
            // Rebuild $0
            self.record = self.fields.join(&self.ofs);
        }
    }

    fn pattern_matches(&mut self, pattern: &Option<Pattern>, rule_idx: usize) -> Result<bool> {
        match pattern {
            None => Ok(true), // No pattern means always match
            Some(Pattern::Begin)
            | Some(Pattern::End)
            | Some(Pattern::BeginFile)
            | Some(Pattern::EndFile) => Ok(false),
            Some(Pattern::Expr(expr)) => {
                let val = self.eval_expr(expr)?;
                Ok(val.is_truthy())
            }
            Some(Pattern::Regex(regex)) => {
                let record = self.record.clone();
                let re = self.get_regex(regex)?;
                Ok(re.is_match(&record))
            }
            Some(Pattern::Range { start, end }) => {
                let active = self.range_states.get(&rule_idx).copied().unwrap_or(false);
                if !active {
                    // Check if start pattern matches
                    if self.pattern_matches(&Some(start.as_ref().clone()), rule_idx)? {
                        self.range_states.insert(rule_idx, true);
                        return Ok(true);
                    }
                    Ok(false)
                } else {
                    // Range is active, check if end pattern matches
                    if self.pattern_matches(&Some(end.as_ref().clone()), rule_idx)? {
                        self.range_states.insert(rule_idx, false);
                    }
                    Ok(true)
                }
            }
            Some(Pattern::And(left, right)) => Ok(self
                .pattern_matches(&Some(left.as_ref().clone()), rule_idx)?
                && self.pattern_matches(&Some(right.as_ref().clone()), rule_idx)?),
            Some(Pattern::Or(left, right)) => Ok(self
                .pattern_matches(&Some(left.as_ref().clone()), rule_idx)?
                || self.pattern_matches(&Some(right.as_ref().clone()), rule_idx)?),
            Some(Pattern::Not(inner)) => {
                Ok(!self.pattern_matches(&Some(inner.as_ref().clone()), rule_idx)?)
            }
        }
    }

    pub(crate) fn get_regex(&mut self, pattern: &str) -> Result<&Regex> {
        if !self.regex_cache.contains_key(pattern) {
            let regex = Regex::new(pattern).map_err(Error::Regex)?;
            self.regex_cache.insert(pattern.to_string(), regex);
        }
        Ok(self.regex_cache.get(pattern).unwrap())
    }

    pub(crate) fn get_variable(&self, name: &str) -> Value {
        // Check special variables first
        match name {
            "NF" => Value::Number(self.nf as f64),
            "NR" => Value::Number(self.nr as f64),
            "FNR" => Value::Number(self.fnr as f64),
            "FS" => Value::from_string(self.fs.clone()),
            "OFS" => Value::from_string(self.ofs.clone()),
            "RS" => Value::from_string(self.rs.clone()),
            "ORS" => Value::from_string(self.ors.clone()),
            "OFMT" => Value::from_string(self.ofmt.clone()),
            "CONVFMT" => Value::from_string(self.convfmt.clone()),
            "SUBSEP" => Value::from_string(self.subsep.clone()),
            "FILENAME" => Value::from_string(self.filename.clone()),
            "RSTART" => Value::Number(self.rstart as f64),
            "RLENGTH" => Value::Number(self.rlength as f64),
            "ARGC" => Value::Number(self.argc as f64),
            // gawk extensions
            "FPAT" => Value::from_string(self.fpat.clone()),
            "FIELDWIDTHS" => Value::from_string(self.fieldwidths.clone()),
            _ => self
                .variables
                .get(name)
                .cloned()
                .unwrap_or(Value::Uninitialized),
        }
    }

    /// Get an element from ARGV, ENVIRON, or PROCINFO arrays
    pub(crate) fn get_special_array(&self, array: &str, key: &str) -> Option<Value> {
        match array {
            "ARGV" => key
                .parse::<usize>()
                .ok()
                .and_then(|i| self.argv.get(i))
                .map(|s| Value::from_string(s.clone())),
            "ENVIRON" => self.environ.get(key).map(|s| Value::from_string(s.clone())),
            "PROCINFO" => {
                // gawk PROCINFO array - system information
                match key {
                    "version" => Some(Value::from_string(env!("CARGO_PKG_VERSION").to_string())),
                    "strftime" => Some(Value::from_string("%a %b %e %H:%M:%S %Z %Y".to_string())),
                    "FS" => {
                        if !self.fpat.is_empty() {
                            Some(Value::from_string("FPAT".to_string()))
                        } else if !self.fieldwidths.is_empty() {
                            Some(Value::from_string("FIELDWIDTHS".to_string()))
                        } else {
                            Some(Value::from_string("FS".to_string()))
                        }
                    }
                    "identifiers" => Some(Value::Number(0.0)), // Not implemented
                    "pid" => Some(Value::Number(std::process::id() as f64)),
                    "ppid" => Some(Value::Number(0.0)), // Not easily available in Rust
                    "uid" => Some(Value::Number(0.0)),  // Platform specific
                    "gid" => Some(Value::Number(0.0)),  // Platform specific
                    "euid" => Some(Value::Number(0.0)), // Platform specific
                    "egid" => Some(Value::Number(0.0)), // Platform specific
                    "pgrpid" => Some(Value::Number(0.0)), // Platform specific
                    _ => Some(Value::Uninitialized),
                }
            }
            _ => None,
        }
    }

    pub(crate) fn set_variable_value(&mut self, name: &str, value: Value) {
        // Handle special variables
        match name {
            "NF" => {
                let new_nf = value.to_number() as usize;
                if new_nf < self.nf {
                    self.fields.truncate(new_nf);
                } else {
                    while self.fields.len() < new_nf {
                        self.fields.push(String::new());
                    }
                }
                self.nf = new_nf;
                self.record = self.fields.join(&self.ofs);
            }
            "FS" => {
                self.fs = value.to_string_val();
                // Clear FPAT and FIELDWIDTHS when FS is set
                self.fpat.clear();
                self.fieldwidths.clear();
            }
            "OFS" => self.ofs = value.to_string_val(),
            "RS" => self.rs = value.to_string_val(),
            "ORS" => self.ors = value.to_string_val(),
            "OFMT" => self.ofmt = value.to_string_val(),
            "CONVFMT" => self.convfmt = value.to_string_val(),
            "SUBSEP" => self.subsep = value.to_string_val(),
            // gawk extensions
            "FPAT" => {
                self.fpat = value.to_string_val();
                // FPAT takes precedence over FS and FIELDWIDTHS
                self.fieldwidths.clear();
            }
            "FIELDWIDTHS" => {
                self.fieldwidths = value.to_string_val();
                // FIELDWIDTHS takes precedence over FS
                self.fpat.clear();
            }
            _ => {
                self.variables.insert(name.to_string(), value);
            }
        }
    }

    /// Resolve array name through aliases (for pass-by-reference in functions)
    fn resolve_array_name<'b>(&'b self, array: &'b str) -> &'b str {
        self.array_aliases
            .get(array)
            .map(|s| s.as_str())
            .unwrap_or(array)
    }

    pub(crate) fn get_array_element(&self, array: &str, key: &str) -> Value {
        let array = self.resolve_array_name(array);

        // Check for special arrays first
        if let Some(val) = self.get_special_array(array, key) {
            return val;
        }

        self.arrays
            .get(array)
            .and_then(|arr| arr.get(key))
            .cloned()
            .unwrap_or(Value::Uninitialized)
    }

    pub(crate) fn set_array_element(&mut self, array: &str, key: &str, value: Value) {
        let array = self.resolve_array_name(array).to_string();
        self.arrays
            .entry(array)
            .or_default()
            .insert(key.to_string(), value);
    }

    pub(crate) fn array_key_exists(&self, array: &str, key: &str) -> bool {
        let array = self.resolve_array_name(array);

        // Check special arrays
        match array {
            "ARGV" => key
                .parse::<usize>()
                .ok()
                .map(|i| i < self.argv.len())
                .unwrap_or(false),
            "ENVIRON" => self.environ.contains_key(key),
            _ => self
                .arrays
                .get(array)
                .map(|arr| arr.contains_key(key))
                .unwrap_or(false),
        }
    }

    pub(crate) fn delete_array_element(&mut self, array: &str, key: &str) {
        let array = self.resolve_array_name(array).to_string();
        if let Some(arr) = self.arrays.get_mut(&array) {
            arr.remove(key);
        }
    }

    pub(crate) fn make_array_key(&self, indices: &[Value]) -> String {
        indices
            .iter()
            .map(|v| v.to_string_val())
            .collect::<Vec<_>>()
            .join(&self.subsep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use std::io::Cursor;

    fn run_awk(program: &str, input: &str) -> String {
        let mut lexer = Lexer::new(program);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let mut interpreter = Interpreter::new(&ast);
        let mut output = Vec::new();
        let inputs: Vec<std::io::BufReader<Cursor<&str>>> = if input.is_empty() {
            vec![]
        } else {
            vec![std::io::BufReader::new(Cursor::new(input))]
        };

        interpreter.run(inputs, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn test_begin_print() {
        let output = run_awk(r#"BEGIN { print "hello" }"#, "");
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn test_print_field() {
        let output = run_awk("{ print $1 }", "one two three");
        assert_eq!(output, "one\n");
    }

    #[test]
    fn test_print_multiple_fields() {
        let output = run_awk("{ print $1, $3 }", "one two three");
        assert_eq!(output, "one three\n");
    }

    #[test]
    fn test_arithmetic() {
        let output = run_awk("BEGIN { print 2 + 3 * 4 }", "");
        assert_eq!(output, "14\n");
    }

    #[test]
    fn test_variable() {
        let output = run_awk("BEGIN { x = 5; print x }", "");
        assert_eq!(output, "5\n");
    }

    #[test]
    fn test_if_statement() {
        let output = run_awk("BEGIN { x = 10; if (x > 5) print \"big\" }", "");
        assert_eq!(output, "big\n");
    }

    #[test]
    fn test_while_loop() {
        let output = run_awk("BEGIN { i = 1; while (i <= 3) { print i; i++ } }", "");
        assert_eq!(output, "1\n2\n3\n");
    }

    #[test]
    fn test_pattern_match() {
        let output = run_awk("/two/ { print $0 }", "one\ntwo\nthree");
        assert_eq!(output, "two\n");
    }

    #[test]
    fn test_for_loop() {
        let output = run_awk("BEGIN { for (i = 1; i <= 3; i++) print i }", "");
        assert_eq!(output, "1\n2\n3\n");
    }

    #[test]
    fn test_for_in_loop() {
        let output = run_awk(
            "BEGIN { a[1]=1; a[2]=2; for (k in a) count++; print count }",
            "",
        );
        assert_eq!(output, "2\n");
    }

    #[test]
    fn test_do_while() {
        let output = run_awk("BEGIN { i = 0; do { i++ } while (i < 3); print i }", "");
        assert_eq!(output, "3\n");
    }

    #[test]
    fn test_break() {
        let output = run_awk(
            "BEGIN { for (i=1; i<=10; i++) { if (i==3) break; print i } }",
            "",
        );
        assert_eq!(output, "1\n2\n");
    }

    #[test]
    fn test_continue() {
        let output = run_awk(
            "BEGIN { for (i=1; i<=3; i++) { if (i==2) continue; print i } }",
            "",
        );
        assert_eq!(output, "1\n3\n");
    }

    #[test]
    fn test_next() {
        let output = run_awk("/skip/ { next } { print }", "one\nskip\ntwo");
        assert_eq!(output, "one\ntwo\n");
    }

    #[test]
    fn test_exit() {
        // exit is called before print on NR == 2
        let output = run_awk("NR == 2 { exit } { print }", "one\ntwo\nthree");
        assert_eq!(output, "one\n");
    }

    #[test]
    fn test_exit_in_end() {
        let output = run_awk("{ print } END { print \"done\" }", "one\ntwo");
        assert!(output.contains("done"));
    }

    #[test]
    fn test_array_access() {
        let output = run_awk("BEGIN { a[\"x\"] = 1; print a[\"x\"] }", "");
        assert_eq!(output, "1\n");
    }

    #[test]
    fn test_array_in() {
        let output = run_awk("BEGIN { a[1]=1; print (1 in a), (2 in a) }", "");
        assert_eq!(output, "1 0\n");
    }

    #[test]
    fn test_delete() {
        let output = run_awk(
            "BEGIN { a[1]=1; a[2]=2; delete a[1]; for(k in a) print k }",
            "",
        );
        assert_eq!(output, "2\n");
    }

    #[test]
    fn test_special_variables() {
        let output = run_awk("{ print NR, NF, $0 }", "a b c");
        assert_eq!(output, "1 3 a b c\n");
    }

    #[test]
    fn test_fs_change() {
        let output = run_awk("BEGIN { FS = \":\" } { print $1 }", "a:b:c");
        assert_eq!(output, "a\n");
    }

    #[test]
    fn test_ofs() {
        let output = run_awk("BEGIN { OFS = \"-\" } { print $1, $2 }", "a b c");
        assert_eq!(output, "a-b\n");
    }

    #[test]
    fn test_nf_access() {
        let output = run_awk("{ print $NF }", "a b c");
        assert_eq!(output, "c\n");
    }

    #[test]
    fn test_field_modify() {
        let output = run_awk("{ $2 = \"X\"; print $0 }", "a b c");
        assert_eq!(output, "a X c\n");
    }

    #[test]
    fn test_user_function() {
        let output = run_awk(
            "function double(x) { return x*2 } BEGIN { print double(5) }",
            "",
        );
        assert_eq!(output, "10\n");
    }

    #[test]
    fn test_recursion() {
        let output = run_awk(
            "function fact(n) { return n<=1 ? 1 : n*fact(n-1) } BEGIN { print fact(5) }",
            "",
        );
        assert_eq!(output, "120\n");
    }

    #[test]
    fn test_printf() {
        let output = run_awk("BEGIN { printf \"%d %s\\n\", 42, \"hello\" }", "");
        assert_eq!(output, "42 hello\n");
    }

    #[test]
    fn test_range_pattern() {
        let output = run_awk(
            "/start/,/end/ { print }",
            "before\nstart\nmiddle\nend\nafter",
        );
        assert_eq!(output, "start\nmiddle\nend\n");
    }

    #[test]
    fn test_compound_pattern_and() {
        let output = run_awk("NR > 1 && NR < 4 { print }", "one\ntwo\nthree\nfour");
        assert_eq!(output, "two\nthree\n");
    }

    #[test]
    fn test_logical_or_in_expr() {
        let output = run_awk("{ if (/a/ || /c/) print }", "a\nb\nc");
        assert_eq!(output, "a\nc\n");
    }

    #[test]
    fn test_negated_pattern() {
        let output = run_awk("!/skip/ { print }", "keep\nskip\nkeep");
        assert_eq!(output, "keep\nkeep\n");
    }

    #[test]
    fn test_builtin_length() {
        let output = run_awk("BEGIN { print length(\"hello\") }", "");
        assert_eq!(output, "5\n");
    }

    #[test]
    fn test_builtin_substr() {
        let output = run_awk("BEGIN { print substr(\"hello\", 2, 3) }", "");
        assert_eq!(output, "ell\n");
    }

    #[test]
    fn test_builtin_index() {
        let output = run_awk("BEGIN { print index(\"hello\", \"ll\") }", "");
        assert_eq!(output, "3\n");
    }

    #[test]
    fn test_builtin_split() {
        let output = run_awk(
            "BEGIN { n = split(\"a:b:c\", arr, \":\"); print n, arr[1], arr[2] }",
            "",
        );
        assert_eq!(output, "3 a b\n");
    }

    #[test]
    fn test_builtin_sub() {
        let output = run_awk("BEGIN { x = \"hello\"; sub(\"l\", \"L\", x); print x }", "");
        assert_eq!(output, "heLlo\n");
    }

    #[test]
    fn test_builtin_gsub() {
        let output = run_awk(
            "BEGIN { x = \"hello\"; gsub(\"l\", \"L\", x); print x }",
            "",
        );
        assert_eq!(output, "heLLo\n");
    }

    #[test]
    fn test_builtin_match() {
        let output = run_awk("BEGIN { print match(\"hello\", \"ll\") }", "");
        assert_eq!(output, "3\n");
    }

    #[test]
    fn test_builtin_sprintf() {
        let output = run_awk("BEGIN { print sprintf(\"%05d\", 42) }", "");
        assert_eq!(output, "00042\n");
    }

    #[test]
    fn test_builtin_tolower() {
        let output = run_awk("BEGIN { print tolower(\"HELLO\") }", "");
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn test_builtin_toupper() {
        let output = run_awk("BEGIN { print toupper(\"hello\") }", "");
        assert_eq!(output, "HELLO\n");
    }

    #[test]
    fn test_builtin_math() {
        let output = run_awk("BEGIN { print int(3.7), sqrt(4), sin(0) }", "");
        assert_eq!(output, "3 2 0\n");
    }

    #[test]
    fn test_ternary() {
        let output = run_awk("BEGIN { print 1 ? \"yes\" : \"no\" }", "");
        assert_eq!(output, "yes\n");
    }

    #[test]
    fn test_concatenation() {
        let output = run_awk("BEGIN { print \"a\" \"b\" \"c\" }", "");
        assert_eq!(output, "abc\n");
    }

    #[test]
    fn test_unary_ops() {
        let output = run_awk("BEGIN { x = 5; print -x, +x, !0 }", "");
        assert_eq!(output, "-5 5 1\n");
    }

    #[test]
    fn test_post_increment() {
        let output = run_awk("BEGIN { x = 5; print x++ \" \" x }", "");
        assert_eq!(output, "5 6\n");
    }

    #[test]
    fn test_pre_increment() {
        let output = run_awk("BEGIN { x = 5; print ++x }", "");
        assert_eq!(output, "6\n");
    }

    #[test]
    fn test_compound_assign() {
        let output = run_awk("BEGIN { x = 10; x += 5; x -= 3; x *= 2; print x }", "");
        assert_eq!(output, "24\n");
    }

    #[test]
    fn test_getline_var() {
        let output = run_awk("{ getline next_line; print $0, next_line }", "a\nb");
        // When we read "a", getline reads "b" into next_line
        assert!(output.contains("a") && output.contains("b"));
    }

    #[test]
    fn test_fpat() {
        let output = run_awk("BEGIN { FPAT = \"[^,]+\" } { print $1, $2 }", "a,b,c");
        assert_eq!(output, "a b\n");
    }

    #[test]
    fn test_fieldwidths() {
        let output = run_awk(
            "BEGIN { FIELDWIDTHS = \"2 3 2\" } { print $1, $2 }",
            "abcdefg",
        );
        assert_eq!(output, "ab cde\n");
    }

    #[test]
    fn test_paragraph_mode() {
        let output = run_awk("BEGIN { RS = \"\" } { print NR, NF }", "a b\nc d\n\ne f");
        // First paragraph has 4 words across 2 lines, second has 2 words
        assert!(output.contains("1"));
    }
}
