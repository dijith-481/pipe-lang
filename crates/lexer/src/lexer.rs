use ast::span::Span;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The type of a token.
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
    Arrow,    // =>
    FatArrow, // =>
    Pipe,     // |>
    Bind,     // <-
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Assign, // =
    Dot,
    Comma,
    Colon,
    Semicolon,
    Underscore,

    // Delimiters
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]

    // Literals
    Int(i64),
    Float(f64),
    Str(String),

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
    #[allow(dead_code)]
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
    done: bool,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source string.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
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
                Some('-') => {
                    // Check for -- line comment
                    let saved = self.current_pos;
                    self.advance();
                    if self.peek() == Some('-') {
                        // Skip until newline
                        while let Some(ch) = self.peek() {
                            if ch == '\n' {
                                self.advance();
                                break;
                            }
                            self.advance();
                        }
                    } else {
                        // Not a comment, restore position
                        self.current_pos = saved;
                        break;
                    }
                }
                _ => break,
            }
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
            '(' => TokenKind::OpenParen,
            ')' => TokenKind::CloseParen,
            '{' => TokenKind::OpenBrace,
            '}' => TokenKind::CloseBrace,
            '[' => TokenKind::OpenBracket,
            ']' => TokenKind::CloseBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '_' => TokenKind::Underscore,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
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
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Ne
                } else {
                    TokenKind::Not
                }
            }
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
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '|' => {
                if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Pipe
                } else {
                    TokenKind::Pipe
                }
            }
            '-' => {
                if self.peek() == Some('-') {
                    // Line comment, skip
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
            '"' => {
                // String literal
                let mut s = String::new();
                loop {
                    match self.peek() {
                        Some('"') => {
                            self.advance();
                            break;
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
                                _ => {}
                            }
                        }
                        Some(ch) => {
                            s.push(ch);
                            self.advance();
                        }
                        None => {
                            return Some(self.make_token(TokenKind::Str(s), start));
                        }
                    }
                }
                TokenKind::Str(s)
            }
            ch if ch.is_ascii_digit() => {
                let mut num = String::new();
                num.push(ch);
                let mut is_float = false;
                while let Some(next) = self.peek() {
                    if next.is_ascii_digit() {
                        num.push(next);
                        self.advance();
                    } else if next == '.' && !is_float {
                        is_float = true;
                        num.push(next);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if is_float {
                    TokenKind::Float(num.parse().expect("valid float"))
                } else {
                    TokenKind::Int(num.parse().expect("valid int"))
                }
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                let mut ident = String::new();
                ident.push(ch);
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
            _ => return Some(self.make_token(TokenKind::Ident(String::new()), start)),
        };

        Some(self.make_token(kind, start))
    }
}

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
        assert_eq!(tokens[0].kind, TokenKind::Int(42));
    }

    #[test]
    fn lex_float_literal() {
        let tokens: Vec<Token> = Lexer::new("3.14").collect();
        assert_eq!(tokens[0].kind, TokenKind::Float(3.14));
    }

    #[test]
    fn lex_string_literal() {
        let tokens: Vec<Token> = Lexer::new("\"hello\"").collect();
        assert_eq!(tokens[0].kind, TokenKind::Str("hello".into()));
    }

    #[test]
    fn lex_operators() {
        let tokens: Vec<Token> = Lexer::new("|> => <-").collect();
        assert_eq!(tokens[0].kind, TokenKind::Pipe);
        assert_eq!(tokens[1].kind, TokenKind::Arrow);
        assert_eq!(tokens[2].kind, TokenKind::Bind);
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
    fn lex_skips_line_comments() {
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
}
