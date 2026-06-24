use std::io::Write;

use crate::ast::Expr;
use crate::error::{Error, Result, SourceLocation};
use crate::value::Value;

use super::Interpreter;

impl<'a> Interpreter<'a> {
    /// Call a function with special handling for builtins that need AST access
    pub fn call_function<W: Write>(
        &mut self,
        name: &str,
        args: &[Expr],
        location: SourceLocation,
        output: &mut W,
    ) -> Result<Value> {
        // Check for built-in functions that need special argument handling
        match name {
            "sub" | "gsub" => return self.call_regex_sub(name, args, location),
            "match" => return self.call_match(args, location),
            "split" => return self.call_split(args, location),
            "patsplit" => return self.call_patsplit(args, location),
            "asort" | "asorti" => return self.call_asort(name == "asorti", args, location),
            "getline" => return self.call_getline(args, location),
            "close" => return self.call_close(args, location),
            "fflush" => return self.call_fflush(args, location, output),
            _ => {}
        }

        // Evaluate all arguments for other functions
        let arg_values: Result<Vec<Value>> = args.iter().map(|e| self.eval_expr(e)).collect();
        let arg_values = arg_values?;

        // Check for other built-in functions
        if let Some(result) = self.call_builtin(name, &arg_values)? {
            return Ok(result);
        }

        // Check for user-defined functions
        if let Some(func) = self.functions.get(name).cloned() {
            // Extract array names from arguments for pass-by-reference
            let array_refs: Vec<Option<String>> = args
                .iter()
                .map(|e| match e {
                    Expr::Var(name, _) if self.arrays.contains_key(name) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            return self.call_user_function(func, arg_values, array_refs, output);
        }

        Err(Error::runtime_at(
            format!("undefined function: {}", name),
            location.line,
            location.column,
        ))
    }

    /// Extract regex pattern from an expression (handles both regex literals and strings)
    fn extract_pattern(&mut self, expr: &Expr) -> Result<String> {
        match expr {
            Expr::Regex(pattern, _) => Ok(pattern.clone()),
            other => Ok(self.eval_expr(other)?.to_string_val()),
        }
    }

    /// Call sub or gsub with proper regex and target handling
    fn call_regex_sub(
        &mut self,
        name: &str,
        args: &[Expr],
        location: SourceLocation,
    ) -> Result<Value> {
        let global = name == "gsub";

        let pattern = args
            .first()
            .map(|e| self.extract_pattern(e))
            .transpose()?
            .unwrap_or_default();

        let replacement = args
            .get(1)
            .map(|e| self.eval_expr(e))
            .transpose()?
            .map(|v| v.to_string_val())
            .unwrap_or_default();

        // Get the target (third argument or $0)
        let (target_value, target_expr) = if let Some(target_arg) = args.get(2) {
            (
                self.eval_expr(target_arg)?.to_string_val(),
                Some(target_arg),
            )
        } else {
            (self.record.clone(), None)
        };

        let re = regex::Regex::new(&pattern).map_err(|e| {
            Error::runtime_at(
                format!("invalid regex: {}", e),
                location.line,
                location.column,
            )
        })?;

        let (new_str, count) = regex_sub_helper(&re, &replacement, &target_value, global);

        // Assign the result back to the target
        if let Some(target_arg) = target_expr {
            self.assign_to_lvalue(target_arg, Value::from_string(new_str))?;
        } else {
            self.set_record(&new_str);
        }

        Ok(Value::Number(count as f64))
    }

    /// Call match with proper regex handling
    fn call_match(&mut self, args: &[Expr], location: SourceLocation) -> Result<Value> {
        let s = args
            .first()
            .map(|e| self.eval_expr(e))
            .transpose()?
            .map(|v| v.to_string_val())
            .unwrap_or_default();

        let pattern = args
            .get(1)
            .map(|e| self.extract_pattern(e))
            .transpose()?
            .unwrap_or_default();

        let re = regex::Regex::new(&pattern).map_err(|e| {
            Error::runtime_at(
                format!("invalid regex: {}", e),
                location.line,
                location.column,
            )
        })?;

        if let Some(m) = re.find(&s) {
            self.rstart = m.start() + 1;
            self.rlength = m.len() as i32;
            Ok(Value::Number(self.rstart as f64))
        } else {
            self.rstart = 0;
            self.rlength = -1;
            Ok(Value::Number(0.0))
        }
    }

    /// Call split with proper array name handling
    fn call_split(&mut self, args: &[Expr], location: SourceLocation) -> Result<Value> {
        let s = args
            .first()
            .map(|e| self.eval_expr(e))
            .transpose()?
            .map(|v| v.to_string_val())
            .unwrap_or_default();

        // Get array name from second argument (must be a variable name)
        let array_name = match args.get(1) {
            Some(Expr::Var(name, _)) => name.clone(),
            Some(Expr::ArrayAccess { array, .. }) => array.clone(),
            Some(_) => {
                return Err(Error::runtime_at(
                    "split: second argument must be an array",
                    location.line,
                    location.column,
                ));
            }
            None => {
                return Err(Error::runtime_at(
                    "split: missing array argument",
                    location.line,
                    location.column,
                ));
            }
        };

        // Get separator (third argument or default FS)
        let sep = if let Some(sep_expr) = args.get(2) {
            self.extract_pattern(sep_expr)?
        } else {
            self.fs.clone()
        };

        // Clear the array
        self.arrays.remove(&array_name);

        // Split and populate array
        let parts: Vec<&str> = if sep == " " {
            s.split_whitespace().collect()
        } else if sep.len() == 1 {
            s.split(&sep).collect()
        } else {
            // Use regex split for multi-char separators
            let re = regex::Regex::new(&sep).map_err(|e| {
                Error::runtime_at(
                    format!("invalid regex: {}", e),
                    location.line,
                    location.column,
                )
            })?;
            re.split(&s).collect()
        };

        for (i, part) in parts.iter().enumerate() {
            let key = (i + 1).to_string();
            self.set_array_element(&array_name, &key, Value::from_string(part.to_string()));
        }

        Ok(Value::Number(parts.len() as f64))
    }

    /// asort(source [, dest]) - sort array values
    /// asorti(source [, dest]) - sort array indices
    fn call_asort(
        &mut self,
        sort_indices: bool,
        args: &[Expr],
        location: SourceLocation,
    ) -> Result<Value> {
        // Get source array name
        let source_name = match args.first() {
            Some(Expr::Var(name, _)) => name.clone(),
            _ => {
                return Err(Error::runtime_at(
                    if sort_indices {
                        "asorti: first argument must be an array"
                    } else {
                        "asort: first argument must be an array"
                    },
                    location.line,
                    location.column,
                ));
            }
        };

        // Get optional destination array name
        let dest_name = match args.get(1) {
            Some(Expr::Var(name, _)) => Some(name.clone()),
            None => None,
            _ => {
                return Err(Error::runtime_at(
                    if sort_indices {
                        "asorti: second argument must be an array"
                    } else {
                        "asort: second argument must be an array"
                    },
                    location.line,
                    location.column,
                ));
            }
        };

        // Get values to sort
        let items: Vec<String> = if let Some(arr) = self.arrays.get(&source_name) {
            if sort_indices {
                arr.keys().cloned().collect()
            } else {
                arr.values().map(|v| v.to_string_val()).collect()
            }
        } else {
            Vec::new()
        };

        let mut sorted = items;
        sorted.sort();

        let count = sorted.len();

        // Store in destination (or source if no dest)
        let target = dest_name.unwrap_or_else(|| source_name.clone());
        self.arrays.remove(&target);

        for (i, item) in sorted.iter().enumerate() {
            let key = (i + 1).to_string();
            self.set_array_element(&target, &key, Value::from_string(item.clone()));
        }

        Ok(Value::Number(count as f64))
    }

    /// patsplit(string, array, fieldpat [, seps]) - split by pattern matches
    fn call_patsplit(&mut self, args: &[Expr], location: SourceLocation) -> Result<Value> {
        // Get string to split
        let s = args
            .first()
            .map(|e| self.eval_expr(e))
            .transpose()?
            .map(|v| v.to_string_val())
            .unwrap_or_default();

        // Get array name
        let array_name = match args.get(1) {
            Some(Expr::Var(name, _)) => name.clone(),
            _ => {
                return Err(Error::runtime_at(
                    "patsplit: second argument must be an array",
                    location.line,
                    location.column,
                ));
            }
        };

        // Get field pattern
        let fieldpat = if let Some(pat_expr) = args.get(2) {
            self.extract_pattern(pat_expr)?
        } else {
            return Err(Error::runtime_at(
                "patsplit: missing fieldpat argument",
                location.line,
                location.column,
            ));
        };

        // Optional separator array
        let seps_name = match args.get(3) {
            Some(Expr::Var(name, _)) => Some(name.clone()),
            None => None,
            _ => None,
        };

        // Clear destination arrays
        self.arrays.remove(&array_name);
        if let Some(ref name) = seps_name {
            self.arrays.remove(name);
        }

        // Compile regex and find all matches
        let re = self.get_regex(&fieldpat)?;
        let matches: Vec<regex::Match> = re.find_iter(&s).collect();

        // Store matches in array
        for (i, mat) in matches.iter().enumerate() {
            let key = (i + 1).to_string();
            self.set_array_element(
                &array_name,
                &key,
                Value::from_string(mat.as_str().to_string()),
            );
        }

        // Store separators if requested
        if let Some(ref name) = seps_name {
            let mut last_end = 0;
            for (i, mat) in matches.iter().enumerate() {
                let sep = &s[last_end..mat.start()];
                let key = i.to_string();
                self.set_array_element(name, &key, Value::from_string(sep.to_string()));
                last_end = mat.end();
            }
            // Final separator after last match
            let key = matches.len().to_string();
            self.set_array_element(name, &key, Value::from_string(s[last_end..].to_string()));
        }

        Ok(Value::Number(matches.len() as f64))
    }

    /// Call getline with file/pipe/variable handling
    fn call_getline(&mut self, args: &[Expr], location: SourceLocation) -> Result<Value> {
        // getline returns: 1 (success), 0 (EOF), -1 (error)
        // For now, just return 0 (EOF) for unsupported cases
        // TODO: Implement proper getline with file/pipe support
        let _ = args;
        let _ = location;
        Ok(Value::Number(0.0))
    }

    /// Call close to close a file or pipe
    fn call_close(&mut self, args: &[Expr], location: SourceLocation) -> Result<Value> {
        let filename = args
            .first()
            .map(|e| self.eval_expr(e))
            .transpose()?
            .map(|v| v.to_string_val())
            .unwrap_or_default();

        // Remove from any of our file/pipe collections
        let found = self.output_files.remove(&filename).is_some()
            || self.input_files.remove(&filename).is_some()
            || self.pipes.remove(&filename).is_some();

        let _ = location;
        Ok(Value::Number(if found { 0.0 } else { -1.0 }))
    }

    /// Call fflush to flush output
    fn call_fflush<W: Write>(
        &mut self,
        args: &[Expr],
        _location: SourceLocation,
        output: &mut W,
    ) -> Result<Value> {
        if args.is_empty() {
            // Flush all output
            output.flush().map_err(Error::Io)?;
            for file in self.output_files.values_mut() {
                let _ = file.flush();
            }
            Ok(Value::Number(0.0))
        } else {
            let filename = self.eval_expr(&args[0])?.to_string_val();
            if filename.is_empty() {
                output.flush().map_err(Error::Io)?;
                Ok(Value::Number(0.0))
            } else if let Some(file) = self.output_files.get_mut(&filename) {
                file.flush().map_err(Error::Io)?;
                Ok(Value::Number(0.0))
            } else {
                Ok(Value::Number(-1.0))
            }
        }
    }

    fn call_builtin(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>> {
        match name {
            // String functions
            "length" => {
                let s = args
                    .first()
                    .map(|v| v.to_string_val())
                    .unwrap_or_else(|| self.record.clone());
                // Use character count for UTF-8 support
                Ok(Some(Value::Number(s.chars().count() as f64)))
            }

            "substr" => {
                let s = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let start = args.get(1).map(|v| v.to_number() as usize).unwrap_or(1);
                let len = args.get(2).map(|v| v.to_number() as usize);

                // AWK uses 1-based indexing; ensure start is at least 1
                let start = start.max(1).saturating_sub(1);
                let result = if let Some(len) = len {
                    s.chars().skip(start).take(len).collect()
                } else {
                    s.chars().skip(start).collect()
                };
                Ok(Some(Value::from_string(result)))
            }

            "index" => {
                let s = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let target = args.get(1).map(|v| v.to_string_val()).unwrap_or_default();
                // Find byte position, then convert to character position
                let pos = s
                    .find(&target)
                    .map(|byte_idx| {
                        // Count characters before the byte index
                        s[..byte_idx].chars().count() + 1
                    })
                    .unwrap_or(0);
                Ok(Some(Value::Number(pos as f64)))
            }

            "sprintf" => {
                let format = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let rest = if args.len() > 1 { &args[1..] } else { &[] };
                let result = self.format_printf(&format, rest);
                Ok(Some(Value::from_string(result)))
            }

            "tolower" => {
                let s = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                Ok(Some(Value::from_string(s.to_lowercase())))
            }

            "toupper" => {
                let s = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                Ok(Some(Value::from_string(s.to_uppercase())))
            }

            // Math functions
            "sin" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.sin())))
            }

            "cos" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.cos())))
            }

            "atan2" => {
                let y = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                let x = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(y.atan2(x))))
            }

            "exp" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.exp())))
            }

            "log" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.ln())))
            }

            "sqrt" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.sqrt())))
            }

            "int" => {
                let n = args.first().map(|v| v.to_number()).unwrap_or(0.0);
                Ok(Some(Value::Number(n.trunc())))
            }

            "rand" => {
                // Use the internal RNG state
                let random = self.next_random();
                Ok(Some(Value::Number(random)))
            }

            "srand" => {
                let old_seed = self.rand_seed;
                if let Some(seed) = args.first() {
                    self.rand_seed = seed.to_number() as u64;
                } else {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    self.rand_seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                }
                self.rand_state = self.rand_seed;
                Ok(Some(Value::Number(old_seed as f64)))
            }

            // System functions
            "system" => {
                let cmd = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .status()
                    .map(|s| s.code().unwrap_or(-1))
                    .unwrap_or(-1);
                Ok(Some(Value::Number(status as f64)))
            }

            // === GAWK Extensions ===

            // Time functions
            "systime" => {
                // Return current time as seconds since epoch
                use std::time::{SystemTime, UNIX_EPOCH};
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                Ok(Some(Value::Number(secs as f64)))
            }

            "mktime" => {
                // Parse "YYYY MM DD HH MM SS [DST]" into epoch timestamp
                let datespec = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let parts: Vec<i64> = datespec
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if parts.len() >= 6 {
                    // Simple implementation using chrono-like calculation
                    // This is a simplified version; for full accuracy we'd need chrono crate
                    let year = parts[0];
                    let month = parts[1];
                    let day = parts[2];
                    let hour = parts[3];
                    let min = parts[4];
                    let sec = parts[5];

                    // Simplified epoch calculation (not handling DST or timezones)
                    let epoch = simple_mktime(year, month, day, hour, min, sec);
                    Ok(Some(Value::Number(epoch as f64)))
                } else {
                    Ok(Some(Value::Number(-1.0)))
                }
            }

            "strftime" => {
                // Format timestamp
                let format = args
                    .first()
                    .map(|v| v.to_string_val())
                    .unwrap_or_else(|| "%a %b %e %H:%M:%S %Z %Y".to_string());
                use std::time::{SystemTime, UNIX_EPOCH};
                let timestamp = args
                    .get(1)
                    .map(|v| v.to_number() as u64)
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    });

                let result = format_strftime(&format, timestamp);
                Ok(Some(Value::from_string(result)))
            }

            // gensub - like gsub but returns the result instead of modifying in place
            "gensub" => {
                let pattern = args.first().map(|v| v.to_string_val()).unwrap_or_default();
                let replacement = args.get(1).map(|v| v.to_string_val()).unwrap_or_default();
                let how = args
                    .get(2)
                    .map(|v| v.to_string_val())
                    .unwrap_or_else(|| "g".to_string());
                let target = args
                    .get(3)
                    .map(|v| v.to_string_val())
                    .unwrap_or_else(|| self.record.clone());

                let re = self.get_regex(&pattern)?;

                // "g" or "G" means global, otherwise it's the occurrence number
                let result = if how.eq_ignore_ascii_case("g") {
                    re.replace_all(&target, replacement.replace("&", "$0").as_str())
                        .to_string()
                } else if let Ok(n) = how.parse::<usize>() {
                    // Replace nth occurrence
                    let mut count = 0;
                    let mut last_end = 0;
                    let mut result = String::new();
                    for mat in re.find_iter(&target) {
                        count += 1;
                        if count == n {
                            result.push_str(&target[last_end..mat.start()]);
                            result.push_str(&replacement.replace("&", mat.as_str()));
                            last_end = mat.end();
                            break;
                        }
                    }
                    result.push_str(&target[last_end..]);
                    if count < n { target.clone() } else { result }
                } else {
                    // Default to first occurrence
                    re.replace(&target, replacement.replace("&", "$0").as_str())
                        .to_string()
                };

                Ok(Some(Value::from_string(result)))
            }

            _ => Ok(None), // Not a built-in
        }
    }

    fn call_user_function<W: Write>(
        &mut self,
        func: &crate::ast::FunctionDef,
        args: Vec<Value>,
        array_refs: Vec<Option<String>>,
        output: &mut W,
    ) -> Result<Value> {
        // Save current variables for local scope
        let saved_vars: std::collections::HashMap<String, Value> = func
            .params
            .iter()
            .filter_map(|name| self.variables.get(name).map(|v| (name.clone(), v.clone())))
            .collect();

        // Save any arrays that share names with parameters (for local arrays)
        let saved_arrays: std::collections::HashMap<
            String,
            std::collections::HashMap<String, Value>,
        > = func
            .params
            .iter()
            .filter_map(|name| self.arrays.get(name).map(|a| (name.clone(), a.clone())))
            .collect();

        // Create array aliases for pass-by-reference
        // If an argument is an array reference, the parameter name should point to the same array
        let mut array_aliases: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (i, param) in func.params.iter().enumerate() {
            if let Some(Some(array_name)) = array_refs.get(i) {
                // This parameter is an array reference
                // Create an alias: when we access param, we should access array_name
                if param != array_name {
                    array_aliases.insert(param.clone(), array_name.clone());
                }
            }
        }
        self.array_aliases = array_aliases;

        // Set scalar parameters
        for (i, param) in func.params.iter().enumerate() {
            // Skip if this is an array reference
            if let Some(Some(_)) = array_refs.get(i) {
                continue;
            }
            let value = args.get(i).cloned().unwrap_or(Value::Uninitialized);
            self.set_variable_value(param, value);
        }

        // Execute function body, passing the actual output
        let result = match self.execute_block(&func.body, output)? {
            super::stmt::StmtResult::Return(v) => v,
            _ => Value::Uninitialized,
        };

        // Clear array aliases
        self.array_aliases.clear();

        // Restore saved variables and remove parameters that weren't saved
        for param in &func.params {
            if let Some(value) = saved_vars.get(param) {
                self.set_variable_value(param, value.clone());
            } else {
                self.variables.remove(param);
            }
        }

        // Restore any saved arrays or remove local arrays
        for param in &func.params {
            if let Some(arr) = saved_arrays.get(param) {
                self.arrays.insert(param.clone(), arr.clone());
            } else if !array_refs
                .get(
                    func.params
                        .iter()
                        .position(|p| p == param)
                        .unwrap_or(usize::MAX),
                )
                .map(|r| r.is_some())
                .unwrap_or(false)
            {
                self.arrays.remove(param);
            }
        }

        Ok(result)
    }

    /// Generate a random number between 0 and 1 using xorshift64
    fn next_random(&mut self) -> f64 {
        let mut x = self.rand_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rand_state = x;
        (x as f64) / (u64::MAX as f64)
    }
}

fn regex_sub_helper(
    re: &regex::Regex,
    replacement: &str,
    target: &str,
    global: bool,
) -> (String, usize) {
    // Handle & in replacement (matched text)
    let mut count = 0;

    if global {
        let result = re.replace_all(target, |caps: &regex::Captures| {
            count += 1;
            replacement.replace("&", caps.get(0).map(|m| m.as_str()).unwrap_or(""))
        });
        (result.to_string(), count)
    } else {
        let result = re.replace(target, |caps: &regex::Captures| {
            count += 1;
            replacement.replace("&", caps.get(0).map(|m| m.as_str()).unwrap_or(""))
        });
        (result.to_string(), count)
    }
}

/// Simplified mktime implementation (UTC-based)
fn simple_mktime(year: i64, month: i64, day: i64, hour: i64, min: i64, sec: i64) -> i64 {
    // Days in each month (non-leap year)
    const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    fn days_in_year(year: i64) -> i64 {
        if is_leap_year(year) { 366 } else { 365 }
    }

    // Calculate days from epoch (1970-01-01)
    let mut days: i64 = 0;

    // Add days for complete years
    for y in 1970..year {
        days += days_in_year(y);
    }
    for y in year..1970 {
        days -= days_in_year(y);
    }

    // Add days for complete months in current year
    for m in 1..month {
        let m_idx = (m - 1) as usize;
        if m_idx < 12 {
            days += DAYS_IN_MONTH[m_idx];
            if m == 2 && is_leap_year(year) {
                days += 1;
            }
        }
    }

    // Add remaining days
    days += day - 1;

    // Convert to seconds
    days * 86400 + hour * 3600 + min * 60 + sec
}

/// Simplified strftime implementation
fn format_strftime(format: &str, timestamp: u64) -> String {
    // Break down timestamp into components
    let secs = timestamp as i64;

    // Calculate year, month, day, etc.
    let (year, month, day, hour, min, sec, wday, yday) = breakdown_time(secs);

    let weekday_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekday_full = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let month_full = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    let mut result = String::new();
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.next() {
                Some('Y') => result.push_str(&format!("{:04}", year)),
                Some('y') => result.push_str(&format!("{:02}", year % 100)),
                Some('m') => result.push_str(&format!("{:02}", month)),
                Some('d') => result.push_str(&format!("{:02}", day)),
                Some('e') => result.push_str(&format!("{:2}", day)),
                Some('H') => result.push_str(&format!("{:02}", hour)),
                Some('M') => result.push_str(&format!("{:02}", min)),
                Some('S') => result.push_str(&format!("{:02}", sec)),
                Some('a') => result.push_str(weekday_names.get(wday as usize).unwrap_or(&"???")),
                Some('A') => result.push_str(weekday_full.get(wday as usize).unwrap_or(&"???")),
                Some('b') | Some('h') => {
                    result.push_str(month_names.get((month - 1) as usize).unwrap_or(&"???"))
                }
                Some('B') => {
                    result.push_str(month_full.get((month - 1) as usize).unwrap_or(&"???"))
                }
                Some('j') => result.push_str(&format!("{:03}", yday)),
                Some('u') => result.push_str(&format!("{}", if wday == 0 { 7 } else { wday })),
                Some('w') => result.push_str(&format!("{}", wday)),
                Some('Z') => result.push_str("UTC"),
                Some('z') => result.push_str("+0000"),
                Some('%') => result.push('%'),
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some(c) => {
                    result.push('%');
                    result.push(c);
                }
                None => result.push('%'),
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Break down epoch seconds into date/time components
fn breakdown_time(secs: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let mut days = secs / 86400;

    // wday: 0 = Sunday, 1970-01-01 was Thursday (4)
    let wday = ((days + 4) % 7 + 7) % 7;

    // Calculate year
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days >= days_in_year {
            days -= days_in_year;
            year += 1;
        } else if days < 0 {
            year -= 1;
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            days += days_in_year;
        } else {
            break;
        }
    }

    let yday = days + 1; // 1-based day of year

    // Calculate month and day
    let mut month = 1i64;
    for (m, &dim) in DAYS_IN_MONTH.iter().enumerate() {
        let mut days_in_month = dim;
        if m == 1 && is_leap_year(year) {
            days_in_month += 1;
        }
        if days < days_in_month {
            month = m as i64 + 1;
            break;
        }
        days -= days_in_month;
    }
    let day = days + 1;

    (year, month, day, hour, min, sec, wday, yday)
}
