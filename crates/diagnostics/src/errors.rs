use ast::span::Span;
use miette::{Diagnostic, NamedSource};
use std::sync::Arc;

/// Unified compiler error type aggregating all phases of compilation.
///
/// Each variant carries a [`Span`] for precise error location reporting
/// via `miette` diagnostics. This type is lightweight and does NOT carry
/// the source code itself.
#[derive(Debug, Clone, thiserror::Error, Diagnostic)]
pub enum CompilerError {
    /// Error produced by the lexer.
    #[error("Lex error: {msg}")]
    #[diagnostic(code(pipe_lang::lex))]
    LexError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced by the parser.
    #[error("Parse error: expected {expected}, found {found}")]
    #[diagnostic(code(pipe_lang::parse))]
    ParseError {
        expected: String,
        found: String,
        #[label("expected {expected}, found {found}")]
        span: Span,
    },

    /// Error produced when a type mismatch is detected.
    #[error("Type mismatch: expected {expected}, got {got}")]
    #[diagnostic(code(pipe_lang::type_mismatch))]
    TypeMismatch {
        expected: String,
        got: String,
        #[label("expected {expected}, got {got}")]
        span: Span,
    },

    /// Error produced when a variable is used but not bound.
    #[error("Unbound variable: {name}")]
    #[diagnostic(code(pipe_lang::unbound))]
    UnboundVariable {
        name: String,
        #[label("`{name}` is not defined in this scope")]
        span: Span,
    },

    /// Error produced when a pattern match is not exhaustive.
    #[error("Non-exhaustive pattern match")]
    #[diagnostic(code(pipe_lang::non_exhaustive_match))]
    NonExhaustiveMatch {
        #[label("This match does not cover all possible values")]
        span: Span,
    },

    /// Error produced during IR lowering.
    #[error("IR error: {msg}")]
    #[diagnostic(code(pipe_lang::ir))]
    IrError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during JIT compilation.
    #[error("JIT compile error: {msg}")]
    #[diagnostic(code(pipe_lang::jit))]
    JitCompileError { msg: String },

    /// I/O error (file not found, permission denied, etc.).
    #[error("I/O error: {0}")]
    #[diagnostic(code(pipe_lang::io))]
    IoError(String),

    /// Multiple errors collected together (for error recovery).
    #[error("Encountered {count} error(s)")]
    #[diagnostic(code(pipe_lang::multiple))]
    Multiple {
        count: usize,
        #[label]
        span: Option<Span>,
    },
}

/// The top-level diagnostic wrapper that pairs an error with the source code.
///
/// This is what is actually rendered to the user. It uses `Arc<str>` to
/// ensure the source code is not duplicated in memory.
#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("{error}")]
pub struct SourceDiagnostic {
    #[source_code]
    pub src: NamedSource<Arc<str>>,

    #[diagnostic(transparent)]
    pub error: CompilerError,
}

impl SourceDiagnostic {
    /// Creates a new diagnostic wrapper.
    pub fn new(filename: impl Into<String>, source: Arc<str>, error: CompilerError) -> Self {
        Self {
            src: NamedSource::new(filename.into(), source),
            error,
        }
    }
}

impl From<lexer::error::LexError> for CompilerError {
    fn from(err: lexer::error::LexError) -> Self {
        match err {
            lexer::error::LexError::UnexpectedChar { ch, span } => {
                CompilerError::lex_error(span, format!("unexpected character `{ch}`"))
            }
            lexer::error::LexError::UnterminatedString { span } => {
                CompilerError::lex_error(span, "unterminated string literal")
            }
            lexer::error::LexError::InvalidNumber { span } => {
                CompilerError::lex_error(span, "invalid numeric literal")
            }
            lexer::error::LexError::UnexpectedEof { span } => {
                CompilerError::lex_error(span, "unexpected end of input")
            }
        }
    }
}

impl From<parser::error::ParseError> for CompilerError {
    fn from(err: parser::error::ParseError) -> Self {
        match err {
            parser::error::ParseError::UnexpectedToken {
                expected,
                found,
                span,
            } => {
                let expected_str = expected.join(" or ");
                CompilerError::parse_error(expected_str, found, span)
            }
            parser::error::ParseError::UnexpectedEof { expected, span } => {
                let expected_str = expected.join(" or ");
                CompilerError::parse_error(expected_str, "end of file".to_string(), span)
            }
            parser::error::ParseError::ExpectedExpression { span } => CompilerError::parse_error(
                "expression".to_string(),
                "something else".to_string(),
                span,
            ),
            parser::error::ParseError::Unimplemented { span } => {
                CompilerError::parse_error("unimplemented".to_string(), String::new(), span)
            }
        }
    }
}

impl CompilerError {
    /// Creates a new lex error.
    pub fn lex_error(span: Span, msg: impl Into<String>) -> Self {
        CompilerError::LexError {
            span,
            msg: msg.into(),
        }
    }

    /// Creates a new parse error.
    pub fn parse_error(expected: impl Into<String>, found: impl Into<String>, span: Span) -> Self {
        CompilerError::ParseError {
            expected: expected.into(),
            found: found.into(),
            span,
        }
    }

    /// Creates a new type mismatch error.
    pub fn type_mismatch(expected: impl Into<String>, got: impl Into<String>, span: Span) -> Self {
        CompilerError::TypeMismatch {
            expected: expected.into(),
            got: got.into(),
            span,
        }
    }

    /// Creates a new unbound variable error.
    pub fn unbound_variable(name: impl Into<String>, span: Span) -> Self {
        CompilerError::UnboundVariable {
            name: name.into(),
            span,
        }
    }

    /// Creates a new non-exhaustive match error.
    pub fn non_exhaustive_match(span: Span) -> Self {
        CompilerError::NonExhaustiveMatch { span }
    }

    /// Creates a new IR error.
    pub fn ir_error(span: Span, msg: impl Into<String>) -> Self {
        CompilerError::IrError {
            span,
            msg: msg.into(),
        }
    }

    /// Creates a new JIT compile error.
    pub fn jit_compile_error(msg: impl Into<String>) -> Self {
        CompilerError::JitCompileError { msg: msg.into() }
    }

    /// Returns the source span for this error, if available.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            CompilerError::LexError { span, .. }
            | CompilerError::ParseError { span, .. }
            | CompilerError::TypeMismatch { span, .. }
            | CompilerError::UnboundVariable { span, .. }
            | CompilerError::NonExhaustiveMatch { span, .. }
            | CompilerError::IrError { span, .. } => Some(*span),
            CompilerError::JitCompileError { .. } | CompilerError::IoError(_) => None,
            CompilerError::Multiple { span, .. } => *span,
        }
    }

    /// Returns true if this is a collection of multiple errors.
    #[must_use]
    pub fn is_multiple(&self) -> bool {
        matches!(self, CompilerError::Multiple { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_error_is_display() {
        let err = CompilerError::lex_error(Span::new(0, 1), "unexpected character");
        let msg = format!("{err}");
        assert!(msg.contains("Lex error"));
        assert!(msg.contains("unexpected character"));
    }

    #[test]
    fn parse_error_uses_expected_found() {
        let err = CompilerError::parse_error("`(`", "identifier", Span::new(1, 2));
        let msg = format!("{err}");
        assert!(msg.contains("expected"));
        assert!(msg.contains("`(`"));
        assert!(msg.contains("identifier"));
    }

    #[test]
    fn type_mismatch_display() {
        let err = CompilerError::type_mismatch("i32", "str", Span::new(5, 10));
        let msg = format!("{err}");
        assert!(msg.contains("Type mismatch"));
        assert!(msg.contains("i32"));
        assert!(msg.contains("str"));
    }

    #[test]
    fn type_mismatch_has_span() {
        let err = CompilerError::type_mismatch("i32", "str", Span::new(5, 10));
        assert_eq!(err.span(), Some(Span::new(5, 10)));
    }

    #[test]
    fn unbound_variable_display() {
        let err = CompilerError::unbound_variable("x", Span::new(0, 1));
        let msg = format!("{err}");
        assert!(msg.contains("Unbound variable"));
        assert!(msg.contains("x"));
    }

    #[test]
    fn nonexhaustive_match_display() {
        let err = CompilerError::non_exhaustive_match(Span::new(10, 20));
        let msg = format!("{err}");
        assert!(msg.contains("Non-exhaustive pattern match"));
    }

    #[test]
    fn jit_compile_error_display() {
        let err = CompilerError::jit_compile_error("segfault at 0x0");
        let msg = format!("{err}");
        assert!(msg.contains("JIT compile error"));
        assert!(msg.contains("segfault"));
    }

    #[test]
    fn jit_compile_error_has_no_span() {
        let err = CompilerError::jit_compile_error("oops");
        assert!(err.span().is_none());
    }

    #[test]
    fn multiple_errors_collection() {
        let err = CompilerError::Multiple {
            count: 2,
            span: None,
        };
        assert!(err.is_multiple());
        assert!(err.span().is_none());
        let msg = format!("{err}");
        assert!(msg.contains("2"));
    }

    #[test]
    fn span_returns_correct_location() {
        let err = CompilerError::lex_error(Span::new(10, 15), "unexpected");
        assert_eq!(err.span(), Some(Span::new(10, 15)));
    }

    #[test]
    fn io_error_display() {
        let err = CompilerError::IoError("file not found: test.ln".into());
        let msg = format!("{err}");
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn source_diagnostic_wrapping() {
        let source = Arc::from("let x = 42");
        let err = CompilerError::lex_error(Span::new(0, 3), "test error");
        let diag = SourceDiagnostic::new("test.pp", source, err);
        assert!(format!("{diag}").contains("test error"));
    }

    #[test]
    fn source_diagnostic_graphical_rendering() {
        let source = Arc::from("let x = 42");
        let err = CompilerError::type_mismatch("i32", "str", Span::new(0, 3));
        let diag = SourceDiagnostic::new("test.pp", source, err);
        let report = miette::Report::new(diag);
        let rendered = format!("{report:?}");
        assert!(rendered.contains("Type mismatch"), "rendered: {rendered}");
    }
}
