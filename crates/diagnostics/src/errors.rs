use std::fmt::Display;

use ast::span::Span;
use miette::{Diagnostic, GraphicalReportHandler, LabeledSpan, NamedSource, Severity,
             SourceCode};
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
    #[diagnostic(code(pipe_lang::lex), help("Check the syntax of the highlighted character"))]
    LexError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced by the parser.
    #[error("parse error: {msg}")]
    #[diagnostic(code(pipe_lang::parse), help("Check the syntax near the highlighted token"))]
    ParseError {
        #[label]
        span: Span,
        msg: String,
        expected: Vec<String>,
    },

    /// Error produced by the type checker.
    #[error("type error: {msg}")]
    #[diagnostic(code(pipe_lang::ty), help("Make sure the types in this expression are consistent"))]
    TypeError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during IR lowering.
    #[error("ir error: {msg}")]
    #[diagnostic(code(pipe_lang::ir), help("Internal compiler error — this is a bug in pipe-lang"))]
    IrError {
        #[label]
        span: Span,
        msg: String,
    },

    /// Error produced during runtime execution.
    #[error("runtime error: {msg}")]
    #[diagnostic(code(pipe_lang::runtime), help("Runtime execution failed"))]
    RuntimeError {
        #[label]
        span: Option<Span>,
        msg: String,
    },

    /// Error during effect execution.
    #[error("effect error: {msg}")]
    #[diagnostic(code(pipe_lang::effect), help("Effect execution failed — check IO operations"))]
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
    #[diagnostic(code(pipe_lang::multiple), help("Fix each error individually and recompile"))]
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
///
/// `Diagnostic` is implemented manually rather than via derive so that
/// `source_code()` returns `Self::src` while `labels()`, `help()`, etc.
/// are delegated to the inner `CompilerError`.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct SourceDiagnostic {
    pub src: NamedSource<Arc<str>>,

    pub error: CompilerError,
}

impl Diagnostic for SourceDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.error.code()
    }

    fn severity(&self) -> Option<Severity> {
        self.error.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.error.help()
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.error.labels()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        self.error.related()
    }
}

impl SourceDiagnostic {
    /// Creates a new diagnostic wrapper.
    pub fn new(filename: impl Into<String>, source: Arc<str>, error: CompilerError) -> Self {
        Self {
            src: NamedSource::new(filename.into(), source),
            error,
        }
    }

    /// Renders this diagnostic as a pretty-printed string with source-code
    /// annotations, underlines, and help text (requires miette's `fancy`
    /// feature).
    #[must_use]
    pub fn render(&self) -> String {
        let mut output = String::new();
        let handler = GraphicalReportHandler::new()
            .with_width(term_width().unwrap_or(100));
        let _ = handler.render_report(&mut output, self);
        output
    }
}

/// Attempts to detect the terminal width for diagnostic rendering.
fn term_width() -> Option<usize> {
    use std::env;
    if let Ok(Ok(w)) = env::var("COLUMNS").map(|v| v.parse::<usize>()) {
        return Some(w);
    }
    #[cfg(unix)]
    {
        use std::process::{Command, Stdio};
        let child = Command::new("stty")
            .arg("size")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .output()
            .ok()?;
        if child.status.success() {
            let utf8 = String::from_utf8(child.stdout).ok()?;
            let parts: Vec<&str> = utf8.split_whitespace().collect();
            if parts.len() == 2 {
                return parts[1].parse::<usize>().ok();
            }
        }
    }
    None
}

impl From<lexer::error::LexError> for CompilerError {
    fn from(err: lexer::error::LexError) -> Self {
        match err {
            lexer::error::LexError::UnexpectedChar { ch, span } => {
                CompilerError::lex_error(
                    span,
                    format!(
                        "unexpected character `{ch}` — pipe-lang identifiers \
                         start with a letter or underscore"
                    ),
                )
            }
            lexer::error::LexError::UnterminatedString { span } => {
                CompilerError::lex_error(
                    span,
                    "unterminated string literal — close the string with a double quote (\")",
                )
            }
            lexer::error::LexError::InvalidNumber { span } => {
                CompilerError::lex_error(
                    span,
                    "invalid numeric literal — use a format like `42`, `3.14`, or `42i64`",
                )
            }
            lexer::error::LexError::UnexpectedEof { span } => {
                CompilerError::lex_error(
                    span,
                    "unexpected end of input — check for unclosed braces, parentheses, or quotes",
                )
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
                let expected_str = if expected.is_empty() {
                    String::new()
                } else {
                    format!(" (expected {})", expected.join(", "))
                };
                CompilerError::parse_error(
                    span,
                    format!("unexpected token `{found}`{expected_str}"),
                    expected,
                )
            }
            parser::error::ParseError::UnexpectedEof { expected, span } => {
                let expected_str = if expected.is_empty() {
                    String::new()
                } else {
                    format!(" (expected {})", expected.join(", "))
                };
                CompilerError::parse_error(
                    span,
                    format!("unexpected end of input{expected_str}"),
                    expected,
                )
            }
            parser::error::ParseError::ExpectedExpression { span } => {
                CompilerError::parse_error(
                    span,
                    "expected an expression here — try a value, variable, or function call",
                    vec![],
                )
            }
            parser::error::ParseError::Unimplemented { span } => {
                CompilerError::parse_error(
                    span,
                    "this language feature is not yet implemented",
                    vec![],
                )
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
