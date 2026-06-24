//! CLI integration tests for awk-rs
//!
//! These tests run the awk-rs binary and verify command-line behavior.

use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

/// Run awk-rs with the given arguments and input, returning stdout
fn run_awk_rs(args: &[&str], input: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--"]);
    cmd.args(args);

    if input.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    // Only acquire (and thereby close) the child's stdin when we have input to
    // send, preserving the original behavior. Written without a `let`-chain so it
    // compiles on the MSRV (let-chains were stabilized in Rust 1.88).
    if let Some((input_str, mut stdin)) = input.zip(child.stdin.take()) {
        stdin
            .write_all(input_str.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| e.to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[test]
fn test_cli_help() {
    let output = run_awk_rs(&["--help"], None).unwrap();
    assert!(output.contains("Usage:"));
    assert!(output.contains("awk-rs"));
}

#[test]
fn test_cli_version() {
    let output = run_awk_rs(&["--version"], None).unwrap();
    assert!(output.contains("awk-rs"));
}

#[test]
fn test_cli_simple_program() {
    let output = run_awk_rs(&["BEGIN { print \"hello\" }"], None).unwrap();
    assert_eq!(output, "hello\n");
}

#[test]
fn test_cli_with_input() {
    let output = run_awk_rs(&["{ print $1 }"], Some("a b c")).unwrap();
    assert_eq!(output, "a\n");
}

#[test]
fn test_cli_field_separator() {
    let output = run_awk_rs(&["-F:", "{ print $1 }"], Some("a:b:c")).unwrap();
    assert_eq!(output, "a\n");
}

#[test]
fn test_cli_field_separator_attached() {
    let output = run_awk_rs(&["-F,", "{ print $2 }"], Some("a,b,c")).unwrap();
    assert_eq!(output, "b\n");
}

#[test]
fn test_cli_variable() {
    let output = run_awk_rs(&["-v", "x=5", "BEGIN { print x }"], None).unwrap();
    assert_eq!(output, "5\n");
}

#[test]
fn test_cli_program_file() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, r#"BEGIN {{ print "from file" }}"#).unwrap();

    let path = file.path().to_str().unwrap();
    let output = run_awk_rs(&["-f", path], None).unwrap();
    assert_eq!(output, "from file\n");
}

#[test]
fn test_cli_posix_mode() {
    // In posix mode, FPAT should not work
    let output = run_awk_rs(&["--posix", "BEGIN { print \"posix\" }"], None).unwrap();
    assert_eq!(output, "posix\n");
}

#[test]
fn test_cli_traditional_mode() {
    let output = run_awk_rs(&["--traditional", "BEGIN { print \"trad\" }"], None).unwrap();
    assert_eq!(output, "trad\n");
}

#[test]
fn test_cli_separator_end_of_options() {
    // -- marks end of options, program comes before it
    let output = run_awk_rs(&["BEGIN { print \"test\" }", "--"], None).unwrap();
    assert_eq!(output, "test\n");
}

#[test]
fn test_cli_stdin_dash() {
    let output = run_awk_rs(&["{ print }", "-"], Some("hello")).unwrap();
    assert_eq!(output, "hello\n");
}

#[test]
fn test_cli_multiple_inputs() {
    let mut file1 = NamedTempFile::new().unwrap();
    writeln!(file1, "a").unwrap();
    let mut file2 = NamedTempFile::new().unwrap();
    writeln!(file2, "b").unwrap();

    let path1 = file1.path().to_str().unwrap();
    let path2 = file2.path().to_str().unwrap();
    let output = run_awk_rs(&["{ print }", path1, path2], None).unwrap();
    assert!(output.contains("a") && output.contains("b"));
}

#[test]
fn test_cli_error_no_program() {
    let result = run_awk_rs(&[], None);
    assert!(result.is_err());
}

#[test]
fn test_cli_error_unknown_option() {
    let result = run_awk_rs(&["--unknown"], None);
    assert!(result.is_err());
}

#[test]
fn test_cli_error_missing_f_arg() {
    let result = run_awk_rs(&["-f"], None);
    assert!(result.is_err());
}

#[test]
fn test_cli_error_missing_v_arg() {
    let result = run_awk_rs(&["-v"], None);
    assert!(result.is_err());
}

#[test]
fn test_cli_error_invalid_v_arg() {
    let result = run_awk_rs(&["-v", "invalid"], None);
    assert!(result.is_err());
}

#[test]
fn test_cli_error_missing_field_sep_arg() {
    let result = run_awk_rs(&["-F"], None);
    assert!(result.is_err());
}
