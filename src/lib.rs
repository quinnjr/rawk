//! awk-rs - A 100% POSIX-compatible AWK implementation in Rust
//!
//! This crate provides a complete AWK interpreter that aims for full compatibility
//! with POSIX AWK and GNU AWK extensions.
//!
//! # Example
//!
//! ```
//! use awk_rs::{Lexer, Parser, Interpreter};
//! use std::io::BufReader;
//!
//! // Parse an AWK program
//! let source = r#"BEGIN { print "Hello, World!" }"#;
//! let mut lexer = Lexer::new(source);
//! let tokens = lexer.tokenize().unwrap();
//! let mut parser = Parser::new(tokens);
//! let program = parser.parse().unwrap();
//!
//! // Run the program
//! let mut interpreter = Interpreter::new(&program);
//! let mut output = Vec::new();
//! let inputs: Vec<BufReader<&[u8]>> = vec![];
//! interpreter.run(inputs, &mut output).unwrap();
//!
//! assert_eq!(String::from_utf8(output).unwrap(), "Hello, World!\n");
//! ```
//!
//! # Field Processing Example
//!
//! ```
//! use awk_rs::{Lexer, Parser, Interpreter};
//! use std::io::BufReader;
//!
//! let source = r#"{ print $1, $2 }"#;
//! let mut lexer = Lexer::new(source);
//! let tokens = lexer.tokenize().unwrap();
//! let mut parser = Parser::new(tokens);
//! let program = parser.parse().unwrap();
//!
//! let mut interpreter = Interpreter::new(&program);
//! interpreter.set_fs(",");  // Use comma as field separator
//!
//! let input = b"hello,world\nfoo,bar\n";
//! let mut output = Vec::new();
//! let inputs = vec![BufReader::new(&input[..])];
//! interpreter.run(inputs, &mut output).unwrap();
//!
//! assert_eq!(String::from_utf8(output).unwrap(), "hello world\nfoo bar\n");
//! ```
//!
//! # Pattern Matching Example
//!
//! ```
//! use awk_rs::{Lexer, Parser, Interpreter};
//! use std::io::BufReader;
//!
//! let source = r#"/error/ { print "Found:", $0 }"#;
//! let mut lexer = Lexer::new(source);
//! let tokens = lexer.tokenize().unwrap();
//! let mut parser = Parser::new(tokens);
//! let program = parser.parse().unwrap();
//!
//! let mut interpreter = Interpreter::new(&program);
//! let input = b"info: ok\nerror: failed\ninfo: done\n";
//! let mut output = Vec::new();
//! let inputs = vec![BufReader::new(&input[..])];
//! interpreter.run(inputs, &mut output).unwrap();
//!
//! assert_eq!(String::from_utf8(output).unwrap(), "Found: error: failed\n");
//! ```

pub mod ast;
pub mod error;
pub(crate) mod fmt;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod value;

pub use error::{Error, Result, SourceLocation};
pub use interpreter::Interpreter;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;
pub use value::Value;
