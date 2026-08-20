//! printf-style formatting engine shared by `printf`/`sprintf` and the
//! OFMT/CONVFMT number-to-string conversion path.

use crate::value::Value;

/// Format `args` according to a C printf-style `format` string.
pub(crate) fn sprintf(format: &str, args: &[Value]) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut arg_idx = 0;

    let take_arg = |args: &[Value], idx: &mut usize| -> Value {
        let v = args.get(*idx).cloned().unwrap_or(Value::Uninitialized);
        *idx += 1;
        v
    };

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        // Check for %%
        if chars.peek() == Some(&'%') {
            chars.next();
            result.push('%');
            continue;
        }

        // Flags
        let mut flags = String::new();
        while let Some(&c) = chars.peek() {
            if c == '-' || c == '+' || c == ' ' || c == '#' || c == '0' {
                flags.push(c);
                chars.next();
            } else {
                break;
            }
        }
        let mut left_align = flags.contains('-');
        let plus = flags.contains('+');
        let space = flags.contains(' ');
        let alt = flags.contains('#');

        // Width (digits or '*')
        let mut width_num: Option<usize> = None;
        if chars.peek() == Some(&'*') {
            chars.next();
            let v = take_arg(args, &mut arg_idx);
            let w = v.to_number() as i64;
            if w < 0 {
                left_align = true;
                width_num = Some((-w) as usize);
            } else {
                width_num = Some(w as usize);
            }
        } else {
            let mut width = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    width.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if !width.is_empty() {
                width_num = width.parse().ok();
            }
        }

        // Precision (digits or '*')
        let mut precision_num: Option<usize> = None;
        let mut has_precision = false;
        if chars.peek() == Some(&'.') {
            chars.next();
            has_precision = true;
            if chars.peek() == Some(&'*') {
                chars.next();
                let v = take_arg(args, &mut arg_idx);
                let p = v.to_number() as i64;
                if p < 0 {
                    has_precision = false;
                } else {
                    precision_num = Some(p as usize);
                }
            } else {
                let mut precision = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() {
                        precision.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                precision_num = Some(precision.parse().unwrap_or(0));
            }
        }

        // Conversion specifier
        let spec = chars.next().unwrap_or('s');
        let arg = take_arg(args, &mut arg_idx);
        let zero_pad = flags.contains('0') && !left_align;

        let formatted = match spec {
            's' => {
                let s = arg.to_string_val();
                let s = if let Some(p) = precision_num {
                    s.chars().take(p).collect()
                } else {
                    s
                };
                pad_plain(&s, width_num, left_align)
            }
            'c' => {
                let s = match &arg {
                    Value::Number(_) | Value::NumericString(_, _) | Value::Uninitialized => {
                        let n = arg.to_number() as u32;
                        char::from_u32(n).map(|c| c.to_string()).unwrap_or_default()
                    }
                    Value::String(s) => s.chars().next().map(|c| c.to_string()).unwrap_or_default(),
                };
                pad_plain(&s, width_num, left_align)
            }
            'd' | 'i' => {
                let n = arg.to_number().trunc() as i64;
                let neg = n < 0;
                let mut digits = n.unsigned_abs().to_string();
                let zero_pad = zero_pad && !has_precision;
                if has_precision {
                    let p = precision_num.unwrap_or(0);
                    if digits == "0" && p == 0 {
                        digits.clear();
                    } else if digits.len() < p {
                        digits = format!("{}{}", "0".repeat(p - digits.len()), digits);
                    }
                }
                let sign = if neg {
                    "-"
                } else if plus {
                    "+"
                } else if space {
                    " "
                } else {
                    ""
                };
                pad_numeric(sign, "", &digits, width_num, left_align, zero_pad)
            }
            'u' => {
                let n = arg.to_number().trunc() as i64 as u64;
                let mut digits = n.to_string();
                let zero_pad = zero_pad && !has_precision;
                if has_precision {
                    let p = precision_num.unwrap_or(0);
                    if digits == "0" && p == 0 {
                        digits.clear();
                    } else if digits.len() < p {
                        digits = format!("{}{}", "0".repeat(p - digits.len()), digits);
                    }
                }
                pad_numeric("", "", &digits, width_num, left_align, zero_pad)
            }
            'o' => {
                let n = arg.to_number().trunc() as i64 as u64;
                let mut digits = format!("{:o}", n);
                let zero_pad = zero_pad && !has_precision;
                if has_precision {
                    let p = precision_num.unwrap_or(0);
                    if digits == "0" && p == 0 {
                        digits.clear();
                    } else if digits.len() < p {
                        digits = format!("{}{}", "0".repeat(p - digits.len()), digits);
                    }
                }
                if alt && !digits.starts_with('0') {
                    digits = format!("0{}", digits);
                }
                pad_numeric("", "", &digits, width_num, left_align, zero_pad)
            }
            'x' | 'X' => {
                let n = arg.to_number().trunc() as i64 as u64;
                let mut digits = if spec == 'x' {
                    format!("{:x}", n)
                } else {
                    format!("{:X}", n)
                };
                let zero_pad = zero_pad && !has_precision;
                if has_precision {
                    let p = precision_num.unwrap_or(0);
                    if digits == "0" && p == 0 {
                        digits.clear();
                    } else if digits.len() < p {
                        digits = format!("{}{}", "0".repeat(p - digits.len()), digits);
                    }
                }
                let prefix = if alt && n != 0 {
                    if spec == 'x' { "0x" } else { "0X" }
                } else {
                    ""
                };
                pad_numeric("", prefix, &digits, width_num, left_align, zero_pad)
            }
            'f' | 'F' => {
                let n = arg.to_number();
                let p = precision_num.unwrap_or(6);
                let sign = float_sign(n, plus, space);
                let body = format_float_fixed(n.abs(), p, alt);
                pad_numeric(sign, "", &body, width_num, left_align, zero_pad)
            }
            'e' | 'E' => {
                let n = arg.to_number();
                let p = precision_num.unwrap_or(6);
                let sign = float_sign(n, plus, space);
                let body = format_float_exp(n.abs(), p, spec == 'E', alt);
                pad_numeric(sign, "", &body, width_num, left_align, zero_pad)
            }
            'g' | 'G' => {
                let n = arg.to_number();
                let p = precision_num.unwrap_or(6);
                let sign = float_sign(n, plus, space);
                let body = format_float_g(n.abs(), p, spec == 'G', alt);
                pad_numeric(sign, "", &body, width_num, left_align, zero_pad)
            }
            _ => format!("%{}", spec),
        };

        result.push_str(&formatted);
    }

    result
}

/// Pad a plain string (no sign/prefix concept) to width, left or right aligned.
fn pad_plain(s: &str, width: Option<usize>, left_align: bool) -> String {
    match width {
        Some(w) => {
            if left_align {
                format!("{:<width$}", s, width = w)
            } else {
                format!("{:>width$}", s, width = w)
            }
        }
        None => s.to_string(),
    }
}

/// Pad a numeric value made of `sign` + `prefix` + `digits`, applying zero-fill
/// between the prefix and the digits when `zero_pad` is set (and not left-aligned).
fn pad_numeric(
    sign: &str,
    prefix: &str,
    digits: &str,
    width: Option<usize>,
    left_align: bool,
    zero_pad: bool,
) -> String {
    let body_len = prefix.len() + digits.len();
    let total_len = sign.len() + body_len;
    match width {
        Some(w) if w > total_len => {
            let pad = w - total_len;
            if left_align {
                format!("{}{}{}{}", sign, prefix, digits, " ".repeat(pad))
            } else if zero_pad {
                format!("{}{}{}{}", sign, prefix, "0".repeat(pad), digits)
            } else {
                format!("{}{}{}{}", " ".repeat(pad), sign, prefix, digits)
            }
        }
        _ => format!("{}{}{}", sign, prefix, digits),
    }
}

fn float_sign(n: f64, plus: bool, space: bool) -> &'static str {
    if n.is_sign_negative() {
        "-"
    } else if plus {
        "+"
    } else if space {
        " "
    } else {
        ""
    }
}

/// Format `n` (already non-negative) as fixed-point with `precision` digits after
/// the decimal point, honoring the alternate-form flag (force a decimal point).
fn format_float_fixed(n: f64, precision: usize, alt: bool) -> String {
    let mut s = format!("{:.*}", precision, n);
    if alt && precision == 0 {
        s.push('.');
    }
    s
}

/// Format `n` (already non-negative) in `d.ddde±dd` form.
fn format_float_exp(n: f64, precision: usize, upper: bool, alt: bool) -> String {
    let raw = format!("{:.*e}", precision, n);
    let (mantissa, exp_str) = raw.split_once('e').unwrap_or((raw.as_str(), "0"));
    let exp: i32 = exp_str.parse().unwrap_or(0);
    let mut mantissa = mantissa.to_string();
    if alt && precision == 0 && !mantissa.contains('.') {
        mantissa.push('.');
    }
    let e_char = if upper { 'E' } else { 'e' };
    format!(
        "{}{}{}{:02}",
        mantissa,
        e_char,
        if exp < 0 { "-" } else { "+" },
        exp.abs()
    )
}

/// Format `n` (already non-negative) using the shorter of %e/%f style, per C's %g rules.
fn format_float_g(n: f64, precision: usize, upper: bool, alt: bool) -> String {
    let p = if precision == 0 { 1 } else { precision };

    // Determine the decimal exponent that %e would use after rounding to `p`
    // significant digits.
    let exp = if n == 0.0 {
        0
    } else {
        let probe = format!("{:.*e}", p - 1, n);
        let exp_str = probe.split_once('e').map(|(_, e)| e).unwrap_or("0");
        exp_str.parse::<i32>().unwrap_or(0)
    };

    let mut s = if exp < -4 || exp >= p as i32 {
        format_float_exp(n, p - 1, upper, alt)
    } else {
        let frac_digits = (p as i32 - 1 - exp).max(0) as usize;
        format_float_fixed(n, frac_digits, alt)
    };

    if !alt {
        s = strip_trailing_zeros(&s, upper);
    }

    s
}

/// Strip insignificant trailing zeros (and a bare trailing decimal point) from a
/// formatted float, without touching an exponent suffix.
fn strip_trailing_zeros(s: &str, upper: bool) -> String {
    let e_char = if upper { 'E' } else { 'e' };
    let (mantissa, suffix) = match s.split_once(e_char) {
        Some((m, e)) => (m, format!("{}{}", e_char, e)),
        None => (s, String::new()),
    };
    if !mantissa.contains('.') {
        return format!("{}{}", mantissa, suffix);
    }
    let trimmed = mantissa.trim_end_matches('0');
    let trimmed = trimmed.trim_end_matches('.');
    format!("{}{}", trimmed, suffix)
}
