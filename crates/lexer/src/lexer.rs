use ast::span::Span;

use crate::error::LexError;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The type of a token.
///
/// Numeric and string literals store their raw source text as `String`.
/// The parser is responsible for interpreting the value (e.g. parsing
/// suffixes like `i32`, `u8`, `f64` on numeric literals).
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Type,
    Let,
    In,
    If,
    Then,
    Else,
    Match,
    With,
    Do,
    Effect,
    Return,
    True,
    False,

    // Operators
    Arrow,      // =>
    Bind,       // <-
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Eq,         // ==
    Ne,         // !=
    Lt,         // <
    Le,         // <=
    Gt,         // >
    Ge,         // >=
    And,        // &&
    Or,         // ||
    Not,        // !
    Assign,     // =
    Dot,        // .
    Comma,      // ,
    Colon,      // :
    Semicolon,  // ;
    Underscore, // _

    // Delimiters
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]

    // Literals (raw source text -- parser handles type interpretation)
    Int(String),   // 42, 42i32, 255u8, 100usize
    Float(String), // 3.14, 3.14f64, 2.71f32
    Str(String),   // "hello"

    // Identifier
    Ident(String),

    // Special
    Eof,
}

impl TokenKind {
    /// Returns true if this token kind is a keyword.
    #[must_use]
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Type
                | TokenKind::Let
                | TokenKind::In
                | TokenKind::If
                | TokenKind::Then
                | TokenKind::Else
                | TokenKind::Match
                | TokenKind::With
                | TokenKind::Do
                | TokenKind::Effect
                | TokenKind::Return
                | TokenKind::True
                | TokenKind::False
        )
    }
}

/// The lexer converts source code into a stream of tokens.
pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
    done: bool,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source string.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            chars: source.char_indices().peekable(),
            current_pos: 0,
            done: false,
        }
    }

    /// Returns the current byte position.
    #[must_use]
    pub fn position(&self) -> usize {
        self.current_pos
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next()?;
        self.current_pos = next.0 + next.1.len_utf8();
        Some(next)
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, ch)| ch)
    }

    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            span: Span::new(start, self.current_pos),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(ch) if ch.is_whitespace() => {
                    self.advance();
                }
                Some('/') => {
                    // Check for // line comment
                    let saved_pos = self.current_pos;
                    let saved_chars = self.chars.clone();
                    self.advance();
                    if self.peek() == Some('/') {
                        // Skip until newline or EOF
                        while let Some(ch) = self.peek() {
                            if ch == '\n' {
                                self.advance();
                                break;
                            }
                            self.advance();
                        }
                    } else {
                        // Not a comment -- restore iterator and position
                        self.chars = saved_chars;
                        self.current_pos = saved_pos;
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn read_string(&mut self, start: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        loop {
            match self.peek() {
                Some('"') => {
                    self.advance();
                    return Ok(TokenKind::Str(s));
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('n') => {
                            s.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            s.push('\t');
                            self.advance();
                        }
                        Some('\\') => {
                            s.push('\\');
                            self.advance();
                        }
                        Some('"') => {
                            s.push('"');
                            self.advance();
                        }
                        Some(ch) => {
                            return Err(LexError::UnexpectedChar {
                                ch,
                                span: Span::new(self.current_pos, self.current_pos + ch.len_utf8()),
                            });
                        }
                        None => {
                            return Err(LexError::UnterminatedString {
                                span: Span::new(start, self.current_pos),
                            });
                        }
                    }
                }
                Some(ch) => {
                    s.push(ch);
                    self.advance();
                }
                None => {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.current_pos),
                    });
                }
            }
        }
    }

    fn read_number(&mut self, first: char) -> TokenKind {
        let mut num = String::new();
        num.push(first);
        let mut is_float = false;
        while let Some(next) = self.peek() {
            if next.is_ascii_digit() {
                num.push(next);
                self.advance();
            } else if next == '.' && !is_float {
                // Check if next char after '.' is a digit (not field access like `1.abc`)
                let mut clone = self.chars.clone();
                clone.next(); // skip '.'
                if clone.next().is_some_and(|(_, c)| c.is_ascii_digit()) {
                    is_float = true;
                    num.push(next);
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        // Collect optional type suffix (alphanumeric + underscore)
        while let Some(next) = self.peek() {
            if next.is_alphanumeric() || next == '_' {
                num.push(next);
                self.advance();
            } else {
                break;
            }
        }
        if is_float {
            TokenKind::Float(num)
        } else {
            TokenKind::Int(num)
        }
    }

    fn read_identifier(&mut self, first: char) -> TokenKind {
        let mut ident = String::new();
        ident.push(first);
        while let Some(next) = self.peek() {
            if next.is_alphanumeric() || next == '_' {
                ident.push(next);
                self.advance();
            } else {
                break;
            }
        }
        match ident.as_str() {
            "type" => TokenKind::Type,
            "let" => TokenKind::Let,
            "in" => TokenKind::In,
            "if" => TokenKind::If,
            "then" => TokenKind::Then,
            "else" => TokenKind::Else,
            "match" => TokenKind::Match,
            "with" => TokenKind::With,
            "do" => TokenKind::Do,
            "effect" => TokenKind::Effect,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(ident),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace_and_comments();

        let (start, ch) = self.advance().or_else(|| {
            if self.done {
                return None;
            }
            self.done = true;
            Some((self.current_pos, '\0'))
        })?;

        if ch == '\0' {
            return Some(Token {
                kind: TokenKind::Eof,
                span: Span::empty(self.current_pos),
            });
        }

        let kind = match ch {
            // Delimiters
            '(' => TokenKind::OpenParen,
            ')' => TokenKind::CloseParen,
            '{' => TokenKind::OpenBrace,
            '}' => TokenKind::CloseBrace,
            '[' => TokenKind::OpenBracket,
            ']' => TokenKind::CloseBracket,

            // Punctuation
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '_' => TokenKind::Underscore,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,

            // Arithmetic
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '%' => TokenKind::Percent,

            // Division or line comment start
            '/' => {
                if self.peek() == Some('/') {
                    // Line comment -- skip until newline or EOF
                    while let Some(ch) = self.peek() {
                        if ch == '\n' {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                    return self.next();
                }
                TokenKind::Slash
            }

            // = or == or =>
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Eq
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Assign
                }
            }

            // ! or !=
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Ne
                } else {
                    TokenKind::Not
                }
            }

            // < or <= or <-
            '<' => {
                if self.peek() == Some('-') {
                    self.advance();
                    TokenKind::Bind
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }

            // > or >=
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }

            // - or -- comment
            '-' => {
                if self.peek() == Some('-') {
                    // Line comment -- skip until newline or EOF
                    while let Some(ch) = self.peek() {
                        if ch == '\n' {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                    return self.next();
                }
                TokenKind::Minus
            }

            // String literal
            '"' => match self.read_string(start) {
                Ok(kind) => kind,
                Err(err) => {
                    return Some(Token {
                        kind: TokenKind::Eof,
                        span: err.span(),
                    });
                }
            },

            // Numeric literal
            ch if ch.is_ascii_digit() => self.read_number(ch),

            // Identifier or keyword
            ch if ch.is_alphabetic() || ch == '_' => self.read_identifier(ch),

            // Unexpected character
            _ => {
                return Some(self.make_token(TokenKind::Ident(String::new()), start));
            }
        };

        Some(self.make_token(kind, start))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_empty_input() {
        let tokens: Vec<Token> = Lexer::new("").collect();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn lex_single_token() {
        let tokens: Vec<Token> = Lexer::new("+").collect();
        assert_eq!(tokens[0].kind, TokenKind::Plus);
    }

    #[test]
    fn lex_keywords_and_identifiers() {
        let tokens: Vec<Token> = Lexer::new("type let in").collect();
        assert_eq!(tokens[0].kind, TokenKind::Type);
        assert_eq!(tokens[1].kind, TokenKind::Let);
        assert_eq!(tokens[2].kind, TokenKind::In);
    }

    #[test]
    fn lex_integer_literal() {
        let tokens: Vec<Token> = Lexer::new("42").collect();
        assert_eq!(tokens[0].kind, TokenKind::Int("42".into()));
    }

    #[test]
    fn lex_integer_with_suffix() {
        let tokens: Vec<Token> = Lexer::new("42i32").collect();
        assert_eq!(tokens[0].kind, TokenKind::Int("42i32".into()));

        let tokens: Vec<Token> = Lexer::new("255u8").collect();
        assert_eq!(tokens[0].kind, TokenKind::Int("255u8".into()));

        let tokens: Vec<Token> = Lexer::new("100usize").collect();
        assert_eq!(tokens[0].kind, TokenKind::Int("100usize".into()));
    }

    #[test]
    fn lex_float_literal() {
        let tokens: Vec<Token> = Lexer::new("3.14").collect();
        assert_eq!(tokens[0].kind, TokenKind::Float("3.14".into()));
    }

    #[test]
    fn lex_float_with_suffix() {
        let tokens: Vec<Token> = Lexer::new("3.14f64").collect();
        assert_eq!(tokens[0].kind, TokenKind::Float("3.14f64".into()));

        let tokens: Vec<Token> = Lexer::new("2.71f32").collect();
        assert_eq!(tokens[0].kind, TokenKind::Float("2.71f32".into()));
    }

    #[test]
    fn lex_string_literal() {
        let tokens: Vec<Token> = Lexer::new(r#""hello""#).collect();
        assert_eq!(tokens[0].kind, TokenKind::Str("hello".into()));
    }

    #[test]
    fn lex_string_with_escapes() {
        let tokens: Vec<Token> = Lexer::new(r#""line\n\t\\""#).collect();
        assert_eq!(tokens[0].kind, TokenKind::Str("line\n\t\\".into()));
    }

    #[test]
    fn lex_operators() {
        let tokens: Vec<Token> = Lexer::new("=> <-").collect();
        assert_eq!(tokens[0].kind, TokenKind::Arrow);
        assert_eq!(tokens[1].kind, TokenKind::Bind);
    }

    #[test]
    fn lex_comparison_operators() {
        let tokens: Vec<Token> = Lexer::new("== != < <= > >=").collect();
        assert_eq!(tokens[0].kind, TokenKind::Eq);
        assert_eq!(tokens[1].kind, TokenKind::Ne);
        assert_eq!(tokens[2].kind, TokenKind::Lt);
        assert_eq!(tokens[3].kind, TokenKind::Le);
        assert_eq!(tokens[4].kind, TokenKind::Gt);
        assert_eq!(tokens[5].kind, TokenKind::Ge);
    }

    #[test]
    fn lex_delimiters() {
        let tokens: Vec<Token> = Lexer::new("(){}[]").collect();
        assert_eq!(tokens[0].kind, TokenKind::OpenParen);
        assert_eq!(tokens[1].kind, TokenKind::CloseParen);
        assert_eq!(tokens[2].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[3].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[4].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[5].kind, TokenKind::CloseBracket);
    }

    #[test]
    fn lex_slash_comments() {
        let tokens: Vec<Token> = Lexer::new("a // comment\nb").collect();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".into()));
        assert_eq!(tokens[1].kind, TokenKind::Ident("b".into()));
    }

    #[test]
    fn lex_dash_comments() {
        let tokens: Vec<Token> = Lexer::new("a -- comment\nb").collect();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".into()));
        assert_eq!(tokens[1].kind, TokenKind::Ident("b".into()));
    }

    #[test]
    fn lex_skips_whitespace() {
        let tokens: Vec<Token> = Lexer::new("  x  +  y  ").collect();
        assert_eq!(tokens[0].kind, TokenKind::Ident("x".into()));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Ident("y".into()));
    }

    #[test]
    fn lex_span_is_accurate() {
        let tokens: Vec<Token> = Lexer::new("abc + 123").collect();
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 3);
        assert_eq!(tokens[2].span.start, 6);
        assert_eq!(tokens[2].span.end, 9);
    }

    #[test]
    fn lex_function_syntax() {
        let tokens: Vec<Token> = Lexer::new("add = (a, b) => a + b").collect();
        assert_eq!(tokens[0].kind, TokenKind::Ident("add".into()));
        assert_eq!(tokens[1].kind, TokenKind::Assign);
        assert_eq!(tokens[2].kind, TokenKind::OpenParen);
        assert_eq!(tokens[3].kind, TokenKind::Ident("a".into()));
        assert_eq!(tokens[4].kind, TokenKind::Comma);
        assert_eq!(tokens[5].kind, TokenKind::Ident("b".into()));
        assert_eq!(tokens[6].kind, TokenKind::CloseParen);
        assert_eq!(tokens[7].kind, TokenKind::Arrow);
        assert_eq!(tokens[8].kind, TokenKind::Ident("a".into()));
        assert_eq!(tokens[9].kind, TokenKind::Plus);
        assert_eq!(tokens[10].kind, TokenKind::Ident("b".into()));
    }

    #[test]
    fn lex_true_false_keywords() {
        let tokens: Vec<Token> = Lexer::new("true false").collect();
        assert_eq!(tokens[0].kind, TokenKind::True);
        assert_eq!(tokens[1].kind, TokenKind::False);
    }

    #[test]
    fn keyword_recognition() {
        assert!(TokenKind::Type.is_keyword());
        assert!(TokenKind::Let.is_keyword());
        assert!(TokenKind::Match.is_keyword());
        assert!(!TokenKind::Ident("foo".into()).is_keyword());
        assert!(!TokenKind::Plus.is_keyword());
    }

    #[test]
    fn lex_dot_field_access() {
        let tokens: Vec<Token> = Lexer::new("user.name").collect();
        assert_eq!(tokens[0].kind, TokenKind::Ident("user".into()));
        assert_eq!(tokens[1].kind, TokenKind::Dot);
        assert_eq!(tokens[2].kind, TokenKind::Ident("name".into()));
    }

    #[test]
    fn lex_float_not_field_access() {
        // 1.0 should be a float, not Int(1) + Dot + Int(0)
        let tokens: Vec<Token> = Lexer::new("1.0").collect();
        assert_eq!(tokens[0].kind, TokenKind::Float("1.0".into()));
    }

    #[test]
    fn lex_integer_then_dot() {
        // 1.name should be Int(1) + Dot + Ident(name)
        let tokens: Vec<Token> = Lexer::new("1.name").collect();
        assert_eq!(tokens[0].kind, TokenKind::Int("1".into()));
        assert_eq!(tokens[1].kind, TokenKind::Dot);
        assert_eq!(tokens[2].kind, TokenKind::Ident("name".into()));
    }

    #[test]
    fn lex_negative_number() {
        // -42 is Minus + Int(42), not a negative literal
        let tokens: Vec<Token> = Lexer::new("-42").collect();
        assert_eq!(tokens[0].kind, TokenKind::Minus);
        assert_eq!(tokens[1].kind, TokenKind::Int("42".into()));
    }
}
