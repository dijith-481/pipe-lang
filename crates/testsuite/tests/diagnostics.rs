//! Contract tests for CompilerError diagnostics and rendering.

use std::sync::Arc;

use ast::span::Span;
use diagnostics::CompilerError;
use diagnostics::errors::SourceDiagnostic;

// ---------------------------------------------------------------------------
// Error construction
// ---------------------------------------------------------------------------

#[test]
fn diag_lex_error_has_span() {
    let err = CompilerError::lex_error(Span::new(5, 10), "bad char");
    assert_eq!(err.span(), Some(Span::new(5, 10)));
    assert!(err.to_string().contains("Lex error"));
}

#[test]
fn diag_parse_error_has_expected() {
    let err = CompilerError::parse_error("let", "unexpected '!'", Span::new(0, 1));
    assert_eq!(err.span(), Some(Span::new(0, 1)));
    match &err {
        CompilerError::ParseError {
            expected, found, ..
        } => {
            assert_eq!(expected, "let");
            assert_eq!(found, "unexpected '!'");
        }
        _ => panic!("expected ParseError"),
    }
}

#[test]
fn diag_type_mismatch_display() {
    let err = CompilerError::type_mismatch("i32", "str", Span::new(10, 20));
    let msg = format!("{err}");
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("i32"));
}

#[test]
fn diag_ir_error_display() {
    let err = CompilerError::IrError {
        span: Span::new(5, 15),
        msg: "closure capture not found".into(),
    };
    assert_eq!(err.span(), Some(Span::new(5, 15)));
    assert!(err.to_string().contains("IR error"));
}

#[test]
fn diag_jit_compile_error_no_span() {
    let err = CompilerError::jit_compile_error("division by zero");
    assert!(err.span().is_none());
    assert!(err.to_string().contains("JIT compile error"));
}

#[test]
fn diag_unbound_variable_display() {
    let err = CompilerError::unbound_variable("x", Span::new(0, 1));
    assert_eq!(err.span(), Some(Span::new(0, 1)));
    assert!(err.to_string().contains("Unbound variable"));
    assert!(err.to_string().contains("x"));
}

#[test]
fn diag_non_exhaustive_match_display() {
    let err = CompilerError::non_exhaustive_match(Span::new(0, 5));
    assert_eq!(err.span(), Some(Span::new(0, 5)));
    assert!(err.to_string().contains("Non-exhaustive pattern match"));
}

#[test]
fn diag_io_error_no_span() {
    let err = CompilerError::IoError("file not found".into());
    assert!(err.span().is_none());
    assert!(err.to_string().contains("I/O error"));
}

#[test]
fn diag_multiple_errors() {
    let err = CompilerError::Multiple {
        count: 3,
        span: Some(Span::new(0, 10)),
    };
    assert!(err.is_multiple());
    assert_eq!(err.span(), Some(Span::new(0, 10)));
    assert!(err.to_string().contains("3"));
}

// ---------------------------------------------------------------------------
// SourceDiagnostic + miette rendering
// ---------------------------------------------------------------------------

#[test]
fn source_diagnostic_wraps_error() {
    let source = Arc::from("let x = 42\nlet y = x + true");
    let err = CompilerError::type_mismatch("i32", "bool", Span::new(20, 24));
    let diag = SourceDiagnostic::new("test.pp", source, err);
    let rendered = format!("{diag:?}");
    assert!(rendered.contains("Type mismatch") || rendered.contains("i32"));
}

#[test]
fn source_diagnostic_display_contains_message() {
    let source = Arc::from("fn main() { 1 + \"two\" }");
    let err = CompilerError::type_mismatch("i32", "str", Span::new(15, 20));
    let diag = SourceDiagnostic::new("test.pp", source, err);
    let msg = format!("{}", diag);
    assert!(msg.contains("Type mismatch") || msg.contains("i32"));
}

#[test]
fn source_diagnostic_with_lex_error() {
    let source = Arc::from("let @ = 1");
    let err = CompilerError::lex_error(Span::new(4, 5), "unexpected character `@`");
    let diag = SourceDiagnostic::new("test.pp", source, err);
    let msg = format!("{diag}");
    assert!(msg.contains("unexpected character"));
}

// ---------------------------------------------------------------------------
// Error conversion from typechecker
// ---------------------------------------------------------------------------

#[test]
fn type_error_converts_to_compiler_error() {
    use typechecker::{MonoType, TypeError};

    let ty_err = TypeError::UnificationFailed {
        expected: MonoType::I32,
        got: MonoType::Bool,
        span: Span::new(5, 10),
    };
    let comp_err: CompilerError = ty_err.into();
    assert!(comp_err.span().is_some());
    assert!(comp_err.to_string().contains("Type mismatch"));
}
