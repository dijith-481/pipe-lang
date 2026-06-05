use ast::span::Span;

/// Errors produced by the parser.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    /// Unexpected token encountered.
    #[error("unexpected token `{found}`, expected one of: {expected:?}")]
    UnexpectedToken {
        expected: Vec<String>,
        found: String,
        span: Span,
    },

    /// Unexpected end of input.
    #[error("unexpected end of input, expected: {expected:?}")]
    UnexpectedEof { expected: Vec<String>, span: Span },

    /// Missing expression.
    #[error("expected expression")]
    ExpectedExpression { span: Span },
}

impl ParseError {
    /// Returns the span of this error.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            ParseError::UnexpectedToken { span, .. }
            | ParseError::UnexpectedEof { span, .. }
            | ParseError::ExpectedExpression { span } => *span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_display() {
        let err = ParseError::UnexpectedToken {
            expected: vec!["`(`".into(), "identifier".into()],
            found: "`}`".into(),
            span: Span::new(10, 11),
        };
        let msg = format!("{err}");
        assert!(msg.contains("unexpected token"));
        assert!(msg.contains("`}`"));
    }

    #[test]
    fn parse_error_span() {
        let err = ParseError::ExpectedExpression {
            span: Span::new(5, 5),
        };
        assert_eq!(err.span(), Span::new(5, 5));
    }
}
