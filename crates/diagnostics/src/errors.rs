use ast::span::Span;

/// Unified compiler error type aggregating all phases of compilation.
///
/// Each variant carries a [`Span`] for precise error location reporting
/// via `miette` diagnostics.
#[derive(Debug, Clone, thiserror::Error, miette::Diagnostic)]
pub enum CompilerError {
    /// Error produced by the lexer.
    #[error("lex error: {msg}")]
    #[diagnostic(code(pipe_lang::lex))]
    LexError {
        #[source_code]
        src: String,
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced by the parser.
    #[error("parse error: {msg}")]
    #[diagnostic(code(pipe_lang::parse))]
    ParseError {
        #[source_code]
        src: String,
        #[label]
        span: Span,
        msg: String,
        expected: Vec<String>,
    },

    /// Error produced by the type checker.
    #[error("type error: {msg}")]
    #[diagnostic(code(pipe_lang::ty))]
    TypeError {
        #[source_code]
        src: String,
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during IR lowering.
    #[error("ir error: {msg}")]
    #[diagnostic(code(pipe_lang::ir))]
    IrError {
        #[source_code]
        src: String,
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during runtime execution.
    #[error("runtime error: {msg}")]
    #[diagnostic(code(pipe_lang::runtime))]
    RuntimeError {
        #[source_code]
        src: String,
        #[label]
        span: Option<Span>,
        msg: String,
    },

    /// Error during effect execution.
    #[error("effect error: {msg}")]
    #[diagnostic(code(pipe_lang::effect))]
    EffectError {
        #[source_code]
        src: String,
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
        #[source_code]
        src: String,
        #[label]
        span: Option<Span>,
    },
}

impl CompilerError {
    /// Creates a new lex error.
    pub fn lex_error(source: String, span: Span, msg: impl Into<String>) -> Self {
        CompilerError::LexError {
            src: source,
            span,
            msg: msg.into(),
        }
    }

    /// Creates a new parse error.
    pub fn parse_error(
        source: String,
        span: Span,
        msg: impl Into<String>,
        expected: Vec<String>,
    ) -> Self {
        CompilerError::ParseError {
            src: source,
            span,
            msg: msg.into(),
            expected,
        }
    }

    /// Creates a new type error.
    pub fn type_error(source: String, span: Span, msg: impl Into<String>) -> Self {
        CompilerError::TypeError {
            src: source,
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
            CompilerError::Multiple { .. } => None,
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

    fn sample_source() -> String {
        "let x = 5 + \"hello\"".to_string()
    }

    #[test]
    fn lex_error_is_display() {
        let err =
            CompilerError::lex_error(sample_source(), Span::new(0, 1), "unexpected character");
        let msg = format!("{err}");
        assert!(msg.contains("lex error"));
        assert!(msg.contains("unexpected character"));
    }

    #[test]
    fn parse_error_includes_expected_tokens() {
        let err = CompilerError::parse_error(
            sample_source(),
            Span::new(15, 16),
            "expected `)`",
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
        let err = CompilerError::type_error(
            sample_source(),
            Span::new(14, 20),
            "cannot add `Int` and `Str`",
        );
        let msg = format!("{err}");
        assert!(msg.contains("type error"));
        assert!(msg.contains("cannot add"));
    }

    #[test]
    fn multiple_errors_collection() {
        let err = CompilerError::Multiple {
            count: 2,
            src: sample_source(),
            span: None,
        };
        assert!(err.is_multiple());
        assert!(err.span().is_none());
        let msg = format!("{err}");
        assert!(msg.contains("2"));
    }

    #[test]
    fn span_returns_correct_location() {
        let err = CompilerError::lex_error(sample_source(), Span::new(10, 15), "unexpected");
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
            src: sample_source(),
            span: None,
            msg: "division by zero".into(),
        };
        assert_eq!(err.span(), None);
    }

    #[test]
    fn runtime_error_with_span() {
        let err = CompilerError::RuntimeError {
            src: sample_source(),
            span: Some(Span::new(10, 12)),
            msg: "out of bounds".into(),
        };
        assert_eq!(err.span(), Some(Span::new(10, 12)));
    }
}
