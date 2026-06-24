use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;

/// AWK value type with dynamic typing and automatic coercion
///
/// AWK has a unique type system where values can be strings, numbers, or
/// "numeric strings" (strings that look like numbers). This enum captures
/// all three cases.
///
/// # Examples
///
/// ```
/// use awk_rs::Value;
///
/// // Numbers
/// let num = Value::Number(42.0);
/// assert_eq!(num.to_number(), 42.0);
/// assert_eq!(num.to_string_val(), "42");
///
/// // Strings
/// let s = Value::from_string("hello".to_string());
/// assert_eq!(s.to_string_val(), "hello");
/// assert_eq!(s.to_number(), 0.0);  // Non-numeric string coerces to 0
///
/// // Numeric strings
/// let ns = Value::from_string("123".to_string());
/// assert_eq!(ns.to_number(), 123.0);
/// assert_eq!(ns.to_string_val(), "123");
///
/// // Truthiness
/// assert!(Value::Number(1.0).is_truthy());
/// assert!(!Value::Number(0.0).is_truthy());
/// assert!(Value::from_string("hello".to_string()).is_truthy());
/// assert!(!Value::from_string("".to_string()).is_truthy());
/// ```
#[derive(Debug, Clone, Default)]
pub enum Value {
    /// Uninitialized value - coerces to "" or 0 depending on context
    #[default]
    Uninitialized,
    /// Numeric value
    Number(f64),
    /// String value
    String(String),
    /// Numeric string - a string that looks like a number
    /// (used for comparison semantics)
    NumericString(String, f64),
}

impl Value {
    /// Create a new string value, detecting if it's a numeric string
    #[inline]
    pub fn from_string(s: String) -> Self {
        if let Some(num) = parse_numeric_string(&s) {
            Value::NumericString(s, num)
        } else {
            Value::String(s)
        }
    }

    /// Create a numeric value
    #[inline]
    pub fn from_number(n: f64) -> Self {
        Value::Number(n)
    }

    /// Check if this value is "true" in boolean context
    /// - Uninitialized is false
    /// - Number 0 is false
    /// - Empty string is false
    /// - Everything else is true
    #[inline]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Uninitialized => false,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::NumericString(s, _) => !s.is_empty(),
        }
    }

    /// Coerce to numeric value
    #[inline]
    pub fn to_number(&self) -> f64 {
        match self {
            Value::Uninitialized => 0.0,
            Value::Number(n) => *n,
            Value::String(s) => parse_leading_number(s),
            Value::NumericString(_, n) => *n,
        }
    }

    /// Coerce to string value
    #[inline]
    pub fn to_string_val(&self) -> String {
        self.to_string_with_format("%.6g")
    }

    /// Get string as Cow to avoid allocation when possible
    #[inline]
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Value::Uninitialized => Cow::Borrowed(""),
            Value::Number(n) => Cow::Owned(format_number(*n, "%.6g")),
            Value::String(s) => Cow::Borrowed(s),
            Value::NumericString(s, _) => Cow::Borrowed(s),
        }
    }

    /// Coerce to string with specific format (for OFMT/CONVFMT)
    pub fn to_string_with_format(&self, format: &str) -> String {
        match self {
            Value::Uninitialized => String::new(),
            Value::Number(n) => format_number(*n, format),
            Value::String(s) => s.clone(),
            Value::NumericString(s, _) => s.clone(),
        }
    }

    /// Check if this value is definitely numeric
    #[inline]
    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    /// Check if this value is a numeric string
    #[inline]
    pub fn is_numeric_string(&self) -> bool {
        matches!(self, Value::NumericString(_, _))
    }

    /// Check if this value should compare as a number
    #[inline]
    pub fn compares_as_number(&self) -> bool {
        matches!(
            self,
            Value::Number(_) | Value::NumericString(_, _) | Value::Uninitialized
        )
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Uninitialized => Ok(()),
            Value::Number(n) => write!(f, "{}", format_number(*n, "%.6g")),
            Value::String(s) => write!(f, "{}", s),
            Value::NumericString(s, _) => write!(f, "{}", s),
        }
    }
}

/// Compare two AWK values according to AWK comparison rules
#[inline]
pub fn compare_values(left: &Value, right: &Value) -> Ordering {
    // If both are numeric or numeric strings, compare numerically
    if left.compares_as_number() && right.compares_as_number() {
        let l = left.to_number();
        let r = right.to_number();
        l.partial_cmp(&r).unwrap_or(Ordering::Equal)
    } else {
        // Otherwise compare as strings - use as_str to avoid allocation
        left.as_str().cmp(&right.as_str())
    }
}

/// Parse the leading numeric portion of a string using optimized byte-based parsing
/// "42abc" -> 42.0
/// "  3.14  " -> 3.14
/// "abc" -> 0.0
#[inline]
pub fn parse_leading_number(s: &str) -> f64 {
    let bytes = s.as_bytes();
    let mut i = 0;

    // Skip leading whitespace
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i >= bytes.len() {
        return 0.0;
    }

    let start = i;

    // Optional sign
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }

    let mut has_digits = false;

    // Digits before decimal
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        has_digits = true;
    }

    // Decimal point and digits after
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            has_digits = true;
        }
    }

    // Exponent
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let exp_start = i;
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if i < bytes.len() && bytes[i].is_ascii_digit() {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            // Invalid exponent, back up
            i = exp_start;
        }
    }

    if !has_digits {
        return 0.0;
    }

    // Fast path for common integer case
    let num_str = &s[start..i];
    let is_integer_like =
        !num_str.contains('.') && !num_str.contains('e') && !num_str.contains('E');
    match num_str.parse::<i64>() {
        Ok(n) if is_integer_like => return n as f64,
        _ => {}
    }

    num_str.parse().unwrap_or(0.0)
}

/// Check if a string is a numeric string (looks entirely like a number)
#[inline]
fn parse_numeric_string(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Fast path: check if it's a simple integer
    if trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return trimmed.parse().ok();
    }

    // Check for leading sign
    let check = if trimmed.starts_with('-') || trimmed.starts_with('+') {
        &trimmed[1..]
    } else {
        trimmed
    };

    // Simple float pattern check
    let mut has_dot = false;
    let mut has_e = false;
    for (i, b) in check.bytes().enumerate() {
        match b {
            b'0'..=b'9' => continue,
            b'.' if !has_dot && !has_e => has_dot = true,
            b'e' | b'E' if !has_e && i > 0 => {
                has_e = true;
                // Check for sign after e
                if i + 1 < check.len() {
                    let next = check.as_bytes()[i + 1];
                    if next == b'+' || next == b'-' {
                        continue;
                    }
                }
            }
            b'+' | b'-' if has_e => continue,
            _ => return None,
        }
    }

    trimmed.parse().ok()
}

/// Format a number according to printf-style format
pub fn format_number(n: f64, format: &str) -> String {
    if n.is_nan() {
        return "nan".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "inf" } else { "-inf" }.to_string();
    }

    // Handle %.6g (default OFMT) - optimized path
    if format == "%.6g" {
        // If it's an integer, print without decimal
        if n.fract() == 0.0 && n.abs() < 1e15 {
            return itoa_fast(n as i64);
        }
        // Otherwise use default formatting with reasonable precision
        let s = format!("{:.6}", n);
        // Trim trailing zeros after decimal point
        if s.contains('.') {
            let trimmed = s.trim_end_matches('0');
            if let Some(stripped) = trimmed.strip_suffix('.') {
                return stripped.to_string();
            }
            return trimmed.to_string();
        }
        return s;
    }

    // Fallback
    format!("{}", n)
}

/// Fast integer to string conversion
#[inline]
fn itoa_fast(n: i64) -> String {
    if n == 0 {
        return "0".to_string();
    }

    let mut result = String::with_capacity(20);
    let mut num = n;
    let negative = num < 0;
    if negative {
        num = -num;
    }

    while num > 0 {
        result.push((b'0' + (num % 10) as u8) as char);
        num /= 10;
    }

    if negative {
        result.push('-');
    }

    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninitialized() {
        let v = Value::Uninitialized;
        assert_eq!(v.to_number(), 0.0);
        assert_eq!(v.to_string_val(), "");
        assert!(!v.is_truthy());
    }

    #[test]
    fn test_number() {
        let v = Value::Number(42.0);
        assert_eq!(v.to_number(), 42.0);
        assert_eq!(v.to_string_val(), "42");
        assert!(v.is_truthy());

        let zero = Value::Number(0.0);
        assert!(!zero.is_truthy());
    }

    #[test]
    fn test_string() {
        let v = Value::from_string("hello".to_string());
        assert_eq!(v.to_number(), 0.0);
        assert_eq!(v.to_string_val(), "hello");
        assert!(v.is_truthy());

        let empty = Value::from_string("".to_string());
        assert!(!empty.is_truthy());
    }

    #[test]
    fn test_numeric_string() {
        let v = Value::from_string("42".to_string());
        assert!(v.is_numeric_string());
        assert_eq!(v.to_number(), 42.0);
        assert_eq!(v.to_string_val(), "42");
    }

    #[test]
    fn test_leading_number() {
        assert_eq!(parse_leading_number("42abc"), 42.0);
        assert_eq!(parse_leading_number("  2.75  "), 2.75);
        assert_eq!(parse_leading_number("abc"), 0.0);
        assert_eq!(parse_leading_number("-5.5"), -5.5);
        assert_eq!(parse_leading_number("1e10"), 1e10);
    }

    #[test]
    fn test_comparison() {
        let n1 = Value::Number(10.0);
        let n2 = Value::Number(2.0);
        assert_eq!(compare_values(&n1, &n2), Ordering::Greater);

        let s1 = Value::from_string("10".to_string());
        let s2 = Value::from_string("2".to_string());
        // Both numeric strings -> compare numerically
        assert_eq!(compare_values(&s1, &s2), Ordering::Greater);

        let s3 = Value::from_string("abc".to_string());
        let s4 = Value::from_string("def".to_string());
        // Both pure strings -> compare lexically
        assert_eq!(compare_values(&s3, &s4), Ordering::Less);
    }

    #[test]
    fn test_itoa_fast() {
        assert_eq!(itoa_fast(0), "0");
        assert_eq!(itoa_fast(42), "42");
        assert_eq!(itoa_fast(-123), "-123");
        assert_eq!(itoa_fast(1000000), "1000000");
    }

    #[test]
    fn test_format_number_nan() {
        assert_eq!(format_number(f64::NAN, "%.6g"), "nan");
    }

    #[test]
    fn test_format_number_inf() {
        assert_eq!(format_number(f64::INFINITY, "%.6g"), "inf");
        assert_eq!(format_number(f64::NEG_INFINITY, "%.6g"), "-inf");
    }

    #[test]
    fn test_format_number_integer() {
        assert_eq!(format_number(42.0, "%.6g"), "42");
        assert_eq!(format_number(-100.0, "%.6g"), "-100");
    }

    #[test]
    fn test_format_number_float() {
        assert_eq!(format_number(2.75, "%.6g"), "2.75");
    }

    #[test]
    fn test_from_number() {
        let v = Value::from_number(2.75);
        assert_eq!(v.to_number(), 2.75);
    }

    #[test]
    fn test_is_truthy_numeric_string() {
        let v = Value::NumericString("42".to_string(), 42.0);
        assert!(v.is_truthy());

        let empty = Value::NumericString("".to_string(), 0.0);
        assert!(!empty.is_truthy());
    }

    #[test]
    fn test_comparison_number_vs_string() {
        let n = Value::Number(10.0);
        let s = Value::from_string("hello".to_string());
        // Number vs non-numeric string
        assert!(compare_values(&n, &s) != Ordering::Equal);
    }

    #[test]
    fn test_comparison_uninitialized() {
        let u = Value::Uninitialized;
        let n = Value::Number(1.0);
        // Uninitialized (0) vs 1 should be Less
        assert_eq!(compare_values(&u, &n), Ordering::Less);
    }

    #[test]
    fn test_parse_leading_with_sign() {
        assert_eq!(parse_leading_number("+42"), 42.0);
        assert_eq!(parse_leading_number("  +2.75"), 2.75);
    }

    #[test]
    fn test_parse_leading_exponent() {
        assert_eq!(parse_leading_number("1e-5"), 1e-5);
        assert_eq!(parse_leading_number("2E+3"), 2000.0);
    }

    #[test]
    fn test_numeric_string_with_exponent() {
        let v = Value::from_string("1e5".to_string());
        assert!(v.is_numeric_string());
        assert_eq!(v.to_number(), 1e5);
    }

    #[test]
    fn test_numeric_string_with_sign() {
        let v = Value::from_string("-42.5".to_string());
        assert!(v.is_numeric_string());
        assert_eq!(v.to_number(), -42.5);
    }

    #[test]
    fn test_numeric_string_whitespace() {
        let v = Value::from_string("  123  ".to_string());
        assert!(v.is_numeric_string());
        assert_eq!(v.to_number(), 123.0);
    }

    #[test]
    fn test_to_string_val_uninitialized() {
        let v = Value::Uninitialized;
        assert_eq!(v.to_string_val(), "");
    }

    #[test]
    fn test_to_number_uninitialized() {
        let v = Value::Uninitialized;
        assert_eq!(v.to_number(), 0.0);
    }
}
