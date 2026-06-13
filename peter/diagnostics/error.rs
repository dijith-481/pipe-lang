use ast::span::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
pub enum CompilerError {
    #[error("Type mismatch: expected {expected}, got {got}")]
    #[diagnostic(code(pipe::type_error))]
    TypeMismatch {
        expected: String,
        got: String,

        #[label("This evaluates to {got}")]
        span: Span,
    },

    #[error("Lexing error: {message}")]
    #[diagnostic(code(pipe::lex_error))]
    LexError {
        message: String,

        #[label("Invalid token")]
        span: Span,
    },

    #[error("Parse error: {message}")]
    #[diagnostic(code(pipe::parse_error))]
    ParseError {
        message: String,

        #[label("Unexpected syntax")]
        span: Span,
    },

    #[error("Unbound variable `{name}`")]
    #[diagnostic(code(pipe::unbound_variable))]
    UnboundVariable {
        name: String,

        #[label("Variable used here")]
        span: Span,
    },

    #[error("Non exhaustive match")]
    #[diagnostic(code(pipe::non_exhaustive_match))]
    NonExhaustiveMatch {
        #[label("Missing patterns")]
        span: Span,
    },
}
