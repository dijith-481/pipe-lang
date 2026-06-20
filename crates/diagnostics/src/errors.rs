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
    #[error("lex error: {msg}")]
    #[diagnostic(code(pipe_lang::lex))]
    LexError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced by the parser.
    #[error("parse error: {msg}")]
    #[diagnostic(code(pipe_lang::parse))]
    ParseError {
        #[label]
        span: Span,
        msg: String,
        expected: Vec<String>,
    },

    /// Error produced by the type checker.
    #[error("type error: {msg}")]
    #[diagnostic(code(pipe_lang::ty))]
    TypeError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during IR lowering.
    #[error("ir error: {msg}")]
    #[diagnostic(code(pipe_lang::ir))]
    IrError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during runtime execution.
    #[error("runtime error: {msg}")]
    #[diagnostic(code(pipe_lang::runtime))]
    RuntimeError {
        #[label]
        span: Option<Span>,
        msg: String,
    },

    /// Error during effect execution.
    #[error("effect error: {msg}")]
    #[diagnostic(code(pipe_lang::effect))]
    EffectError {
        #[label]
        span: Option<Span>,
        msg: String,
    },

    /// I/O error (file not found, permission denied, etc.).
    #[error("io error: {0}")]
    #[diagnostic(code(pipe_lang::io))]
    IoError(String),

    /// Multiple errors collected together (for error recovery).
    #[error("encountered {count} error(s)")]
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
#[derive(Debug, Clone, thiserror::Error, Diagnostic)]
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
            } => CompilerError::parse_error(span, format!("unexpected token `{found}`"), expected),
            parser::error::ParseError::UnexpectedEof { expected, span } => {
                CompilerError::parse_error(span, "unexpected end of input", expected)
            }
            parser::error::ParseError::ExpectedExpression { span } => {
                CompilerError::parse_error(span, "expected expression", vec![])
            }
            parser::error::ParseError::Unimplemented { span } => {
                CompilerError::parse_error(span, "parser stub in use", vec![])
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
    pub fn parse_error(span: Span, msg: impl Into<String>, expected: Vec<String>) -> Self {
        CompilerError::ParseError {
            span,
            msg: msg.into(),
            expected,
        }
    }

    /// Creates a new type error.
    pub fn type_error(span: Span, msg: impl Into<String>) -> Self {
        CompilerError::TypeError {
            span,
            msg: msg.into(),
        }
    }

    /// Returns the source span for this error, if available.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            CompilerError::LexError { span, .. }
            | CompilerError::ParseError { span, .. }
            | CompilerError::TypeError { span, .. }
            | CompilerError::IrError { span, .. } => Some(*span),
            CompilerError::RuntimeError { span, .. } | CompilerError::EffectError { span, .. } => {
                *span
            }
            CompilerError::IoError(_) => None,
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
        assert!(msg.contains("lex error"));
        assert!(msg.contains("unexpected character"));
    }

    #[test]
    fn parse_error_includes_expected_tokens() {
        let err = CompilerError::parse_error(
            Span::new(15, 16),
            "expected `)` ",
            vec!["`(`".into(), "identifier".into()],
        );
        match &err {
            CompilerError::ParseError { expected, .. } => {
                assert_eq!(expected.len(), 2);
            }
            _ => panic!("expected ParseError variant"),
        }
    }

    #[test]
    fn type_error_display() {
        let err = CompilerError::type_error(Span::new(14, 20), "cannot add `Int` and `Str` ");
        let msg = format!("{err}");
        assert!(msg.contains("type error"));
        assert!(msg.contains("cannot add"));
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
        assert!(msg.contains("io error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn runtime_error_with_optional_span() {
        let err = CompilerError::RuntimeError {
            span: None,
            msg: "division by zero".into(),
        };
        assert_eq!(err.span(), None);
    }

    #[test]
    fn runtime_error_with_span() {
        let err = CompilerError::RuntimeError {
            span: Some(Span::new(10, 12)),
            msg: "out of bounds".into(),
        };
        assert_eq!(err.span(), Some(Span::new(10, 12)));
    }

    #[test]
    fn source_diagnostic_wrapping() {
        let source = Arc::from("let x = 42");
        let err = CompilerError::lex_error(Span::new(0, 3), "test error");
        let diag = SourceDiagnostic::new("test.pp", source, err);
        assert!(format!("{diag}").contains("test error"));
    }
}
