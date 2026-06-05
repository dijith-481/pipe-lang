use ast::span::Span;

/// Errors produced by the lexer.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LexError {
    /// An unexpected character was encountered.
    #[error("unexpected character `{ch}`")]
    UnexpectedChar { ch: char, span: Span },

    /// An unterminated string literal.
    #[error("unterminated string literal")]
    UnterminatedString { span: Span },

    /// An invalid numeric literal.
    #[error("invalid numeric literal")]
    InvalidNumber { span: Span },

    /// End of input was reached unexpectedly.
    #[error("unexpected end of input")]
    UnexpectedEof { span: Span },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_error_display() {
        let err = LexError::UnexpectedChar {
            ch: '@',
            span: Span::new(5, 6),
        };
        assert!(format!("{err}").contains("@"));
    }
}
