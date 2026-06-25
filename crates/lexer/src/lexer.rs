use ast::span::Span;

use crate::error::LexError;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

/// The type of a token.
///
/// Numeric and string literals borrow their raw source text from the input string.
/// The parser is responsible for interpreting the value (e.g. parsing
/// suffixes like `i32`, `u8`, `f64` on numeric literals, or handling escapes in strings).
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind<'a> {
    // Keywords
    Type,
    Let,
    If,
    Else,
    Match,
    Use,
    True,
    False,

    // Operators
    Arrow,      // =>
    FuncArrow,  // ->
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
    PathSep,    // ::
    Bar,        // |
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

    // Literals (borrowed from source)
    Int(&'a str),   // 42, 42i32, 255u8, 100usize
    Float(&'a str), // 3.14, 3.14f64, 2.71f32
    Str(&'a str),   // "hello"

    // Template literals (backtick-delimited, may contain ${expr} holes)
    /// Opening backtick of a template literal.
    Backtick,
    /// A non-interpolated chunk of a template literal.
    TemplateStr(&'a str),
    /// Marks the start of a `${` interpolation hole.
    TemplateHoleStart,
    /// Marks the end of an interpolation hole (the closing `}`).
    TemplateHoleEnd,
    /// Marks the end of a template literal (the closing backtick).
    TemplateEnd,

    // Identifier
    Ident(&'a str),

    // Trivial tokens (preserved for tooling, skipped by parser)
    Whitespace(&'a str), // spaces and tabs
    Comment(&'a str),    // // line comment
    Newline,             // \n or \r\n

    // Special
    Eof,
}

impl<'a> TokenKind<'a> {
    /// Returns true if this token kind is a keyword.
    #[must_use]
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Type
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::Match
                | TokenKind::Use
                | TokenKind::True
                | TokenKind::False
        )
    }

    /// Returns true if this token is trivial (whitespace, comment, newline).
    /// The parser should skip these during parsing.
    #[must_use]
    pub fn is_trivial(&self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace(_) | TokenKind::Comment(_) | TokenKind::Newline
        )
    }
}

/// The lexer converts source code into a stream of tokens.
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
    done: bool,
    pending: std::collections::VecDeque<Token<'a>>,
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
            pending: std::collections::VecDeque::new(),
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

    fn make_token(&self, kind: TokenKind<'a>, start: usize) -> Token<'a> {
        Token {
            kind,
            span: Span::new(start, self.current_pos),
        }
    }

    fn read_string(&mut self, start: usize) -> Result<TokenKind<'a>, LexError> {
        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.advance();
                    let raw = &self.source[start + 1..self.current_pos - 1];
                    return Ok(TokenKind::Str(raw));
                }
                '\\' => {
                    self.advance();
                    if self.advance().is_none() {
                        return Err(LexError::UnterminatedString {
                            span: Span::new(start, self.current_pos),
                        });
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        Err(LexError::UnterminatedString {
            span: Span::new(start, self.current_pos),
        })
    }

    /// Reads a backtick template literal, emitting a stream of tokens:
    ///   `Backtick` (TemplateStr ("...") | TemplateHoleStart <inner-tokens> TemplateHoleEnd)* TemplateEnd
    fn read_template(&mut self, start: usize) -> Result<Token<'a>, LexError> {
        self.pending.clear();
        let first = Token {
            kind: TokenKind::Backtick,
            span: Span::new(start, start + 1),
        };

        let mut chunk_start = self.current_pos;
        loop {
            match self.peek() {
                None => {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.current_pos),
                    });
                }
                Some('`') => {
                    let chunk_end = self.current_pos;
                    let raw = &self.source[chunk_start..chunk_end];
                    self.pending.push_back(Token {
                        kind: TokenKind::TemplateStr(raw),
                        span: Span::new(chunk_start, chunk_end),
                    });
                    self.advance(); // consume `
                    let end = self.current_pos;
                    self.pending.push_back(Token {
                        kind: TokenKind::TemplateEnd,
                        span: Span::new(end - 1, end),
                    });
                    return Ok(first);
                }
                Some('$') => {
                    let mut clone = self.chars.clone();
                    clone.next();
                    if clone.next().is_some_and(|(_, c)| c == '{') {
                        let chunk_end = self.current_pos;
                        let raw = &self.source[chunk_start..chunk_end];
                        self.pending.push_back(Token {
                            kind: TokenKind::TemplateStr(raw),
                            span: Span::new(chunk_start, chunk_end),
                        });
                        self.advance(); // consume $
                        self.advance(); // consume {
                        let hole_start = self.current_pos;
                        self.pending.push_back(Token {
                            kind: TokenKind::TemplateHoleStart,
                            span: Span::new(hole_start - 2, hole_start),
                        });
                        let hole_bytes = self.find_hole_end(hole_start)?;
                        let hole_src = &self.source[hole_start..hole_start + hole_bytes];
                        let mut sub = Lexer::new(hole_src);
                        for t in sub.by_ref() {
                            match t {
                                Ok(mut tok) => {
                                    if matches!(tok.kind, TokenKind::Eof) {
                                        continue;
                                    }
                                    tok.span.start += hole_start;
                                    tok.span.end += hole_start;
                                    self.pending.push_back(tok);
                                }
                                Err(mut e) => {
                                    match &mut e {
                                        LexError::UnexpectedChar { span, .. }
                                        | LexError::UnterminatedString { span }
                                        | LexError::InvalidNumber { span }
                                        | LexError::UnexpectedEof { span } => {
                                            span.start += hole_start;
                                            span.end += hole_start;
                                        }
                                    }
                                    return Err(e);
                                }
                            }
                        }
                        for _ in 0..hole_bytes {
                            self.advance();
                        }
                        self.advance(); // consume }
                        let end = self.current_pos;
                        self.pending.push_back(Token {
                            kind: TokenKind::TemplateHoleEnd,
                            span: Span::new(end - 1, end),
                        });
                        chunk_start = self.current_pos;
                    } else {
                        self.advance();
                    }
                }
                Some('\\') => {
                    self.advance();
                    if self.advance().is_none() {
                        return Err(LexError::UnterminatedString {
                            span: Span::new(start, self.current_pos),
                        });
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    fn find_hole_end(&self, start: usize) -> Result<usize, LexError> {
        let mut depth: u32 = 1;
        let chars = self.chars.clone();
        let mut last_pos = start;
        for (idx, ch) in chars {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(idx - start);
                    }
                }
                _ => {}
            }
            last_pos = idx + ch.len_utf8();
        }
        Err(LexError::UnterminatedString {
            span: Span::new(start, last_pos),
        })
    }

    fn read_number(&mut self, start: usize) -> TokenKind<'a> {
        let mut is_float = false;
        while let Some(next) = self.peek() {
            if next.is_ascii_digit() {
                self.advance();
            } else if next == '.' && !is_float {
                // Check if next char after '.' is a digit (not field access like `1.abc`)
                let mut clone = self.chars.clone();
                clone.next(); // skip '.'
                if clone.next().is_some_and(|(_, c)| c.is_ascii_digit()) {
                    is_float = true;
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
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[start..self.current_pos];
        if is_float {
            TokenKind::Float(text)
        } else {
            TokenKind::Int(text)
        }
    }

    fn read_identifier(&mut self, start: usize) -> TokenKind<'a> {
        while let Some(next) = self.peek() {
            if next.is_alphanumeric() || next == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[start..self.current_pos];
        match text {
            "type" => TokenKind::Type,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "match" => TokenKind::Match,
            "use" => TokenKind::Use,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(text),
        }
    }

    fn read_comment(&mut self, start: usize) -> TokenKind<'a> {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.advance();
        }
        TokenKind::Comment(&self.source[start..self.current_pos])
    }

    fn read_whitespace(&mut self, start: usize) -> TokenKind<'a> {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
        TokenKind::Whitespace(&self.source[start..self.current_pos])
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(Ok(tok));
        }
        let (start, ch) = self.advance().or_else(|| {
            if self.done {
                return None;
            }
            self.done = true;
            Some((self.current_pos, '\0'))
        })?;

        if ch == '\0' {
            return Some(Ok(Token {
                kind: TokenKind::Eof,
                span: Span::empty(self.current_pos),
            }));
        }

        let kind = match ch {
            // Newline
            '\n' => TokenKind::Newline,

            // \r\n or \r
            '\r' => {
                if self.peek() == Some('\n') {
                    self.advance();
                }
                TokenKind::Newline
            }

            // Whitespace (spaces and tabs)
            ' ' | '\t' => self.read_whitespace(start),

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
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    TokenKind::PathSep
                } else {
                    TokenKind::Colon
                }
            }

            // Arithmetic
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '%' => TokenKind::Percent,

            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::And
                } else {
                    return Some(Err(LexError::UnexpectedChar {
                        ch: '&',
                        span: Span::new(start, self.current_pos),
                    }));
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::Or
                } else {
                    TokenKind::Bar
                }
            }

            // Division or // line comment
            '/' => {
                if self.peek() == Some('/') {
                    self.advance(); // consume second /
                    self.read_comment(start)
                } else {
                    TokenKind::Slash
                }
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

            // < or <=
            '<' => {
                if self.peek() == Some('=') {
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

            // - or -> function arrow
            '-' => {
                if self.peek() == Some('>') {
                    self.advance(); // consume >
                    TokenKind::FuncArrow
                } else {
                    TokenKind::Minus
                }
            }

            // String literal
            '"' => match self.read_string(start) {
                Ok(kind) => kind,
                Err(err) => return Some(Err(err)),
            },

            // Template literal (backtick-delimited, may contain ${...} holes)
            '`' => match self.read_template(start) {
                Ok(first) => return Some(Ok(first)),
                Err(err) => return Some(Err(err)),
            },

            // Numeric literal
            ch if ch.is_ascii_digit() => self.read_number(start),

            // Identifier or keyword
            ch if ch.is_alphabetic() || ch == '_' => self.read_identifier(start),

            // Unexpected character
            _ => {
                return Some(Err(LexError::UnexpectedChar {
                    ch,
                    span: Span::new(start, self.current_pos),
                }));
            }
        };

        Some(Ok(self.make_token(kind, start)))
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
        let tokens: Vec<_> = Lexer::new("").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn lex_single_token() {
        let tokens: Vec<_> = Lexer::new("+").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Plus);
    }

    #[test]
    fn lex_keywords_and_identifiers() {
        let tokens: Vec<_> = Lexer::new("type let if")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Type);
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" "));
        assert_eq!(tokens[2].kind, TokenKind::Let);
        assert_eq!(tokens[3].kind, TokenKind::Whitespace(" "));
        assert_eq!(tokens[4].kind, TokenKind::If);
    }

    #[test]
    fn lex_integer_literal() {
        let tokens: Vec<_> = Lexer::new("42").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Int("42"));
    }

    #[test]
    fn lex_integer_with_suffix() {
        let tokens: Vec<_> = Lexer::new("42i32").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Int("42i32"));
    }

    #[test]
    fn lex_float_literal() {
        let tokens: Vec<_> = Lexer::new("3.14").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Float("3.14"));
    }

    #[test]
    fn lex_string_literal() {
        let tokens: Vec<_> = Lexer::new(r#""hello""#)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Str("hello"));
    }

    #[test]
    fn lex_unterminated_string() {
        let result = Lexer::new(r#""hello"#).collect::<Result<Vec<_>, _>>();
        assert!(result.is_err());
    }

    #[test]
    fn lex_unexpected_char() {
        let result = Lexer::new("@").collect::<Result<Vec<_>, _>>();
        assert!(result.is_err());
    }

    #[test]
    fn lex_operators() {
        let tokens: Vec<_> = Lexer::new("=> <=").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Arrow);
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" "));
        assert_eq!(tokens[2].kind, TokenKind::Le);
    }

    #[test]
    fn lex_comparison_operators() {
        let tokens: Vec<_> = Lexer::new("== != < <= > >=")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial())
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(significant[0], TokenKind::Eq);
        assert_eq!(significant[1], TokenKind::Ne);
        assert_eq!(significant[2], TokenKind::Lt);
        assert_eq!(significant[3], TokenKind::Le);
        assert_eq!(significant[4], TokenKind::Gt);
        assert_eq!(significant[5], TokenKind::Ge);
    }

    #[test]
    fn lex_comments() {
        let tokens: Vec<_> = Lexer::new("a // comment\nb")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a"));
        assert_eq!(tokens[1].kind, TokenKind::Whitespace(" "));
        assert_eq!(tokens[2].kind, TokenKind::Comment("// comment"));
        assert_eq!(tokens[3].kind, TokenKind::Newline);
        assert_eq!(tokens[4].kind, TokenKind::Ident("b"));
    }

    #[test]
    fn lex_span_is_accurate() {
        let tokens: Vec<_> = Lexer::new("abc + 123")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 3);
        assert_eq!(tokens[4].span.start, 6);
        assert_eq!(tokens[4].span.end, 9);
    }

    #[test]
    fn keyword_recognition() {
        assert!(TokenKind::Type.is_keyword());
        assert!(TokenKind::Let.is_keyword());
        assert!(!TokenKind::Ident("foo").is_keyword());
    }

    #[test]
    fn lex_path_separator() {
        let tokens: Vec<_> = Lexer::new("::").collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::PathSep);
        assert_eq!(tokens[0].span, Span::new(0, 2));
    }

    #[test]
    fn lex_use_stdlib_io() {
        let tokens: Vec<_> = Lexer::new("use stdlib::io")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial() && !matches!(t.kind, TokenKind::Eof))
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(
            significant,
            vec![
                TokenKind::Use,
                TokenKind::Ident("stdlib"),
                TokenKind::PathSep,
                TokenKind::Ident("io"),
            ]
        );
    }

    #[test]
    fn lex_empty_template() {
        let tokens: Vec<_> = Lexer::new("``").collect::<Result<Vec<_>, _>>().unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial() && !matches!(t.kind, TokenKind::Eof))
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(
            significant,
            vec![
                TokenKind::Backtick,
                TokenKind::TemplateStr(""),
                TokenKind::TemplateEnd
            ]
        );
    }

    #[test]
    fn lex_pure_template() {
        let tokens: Vec<_> = Lexer::new("`hello`")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial() && !matches!(t.kind, TokenKind::Eof))
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(
            significant,
            vec![
                TokenKind::Backtick,
                TokenKind::TemplateStr("hello"),
                TokenKind::TemplateEnd
            ]
        );
    }

    #[test]
    fn lex_template_with_hole() {
        let tokens: Vec<_> = Lexer::new("`Hi, ${name}!`")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial() && !matches!(t.kind, TokenKind::Eof))
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(significant[0], TokenKind::Backtick);
        assert_eq!(significant[1], TokenKind::TemplateStr("Hi, "));
        assert_eq!(significant[2], TokenKind::TemplateHoleStart);
        assert_eq!(significant[3], TokenKind::Ident("name"));
        assert_eq!(significant[4], TokenKind::TemplateHoleEnd);
        assert_eq!(significant[5], TokenKind::TemplateStr("!"));
        assert_eq!(significant[6], TokenKind::TemplateEnd);
    }

    #[test]
    fn lex_template_unterminated() {
        let result = Lexer::new("`hello").collect::<Result<Vec<_>, _>>();
        assert!(result.is_err());
    }

    #[test]
    fn lex_template_in_hello_pp() {
        let src = include_str!("../../../example-programs/hello.pp");
        let result: Result<Vec<_>, _> = Lexer::new(src).collect();
        assert!(result.is_ok(), "hello.pp failed to lex: {:?}", result.err());
    }

    #[test]
    fn lex_all_example_programs() {
        let paths = ["../../example-programs", "example-programs"];
        let mut dir = None;
        for p in &paths {
            if let Ok(d) = std::fs::read_dir(p) {
                dir = Some(d);
                break;
            }
        }
        let dir = dir.expect("Could not find example-programs directory");
        for entry in dir {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("pp") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let result: Result<Vec<_>, _> = Lexer::new(&src).collect();
            assert!(
                result.is_ok(),
                "{} failed to lex: {:?}",
                path.display(),
                result.err()
            );
        }
    }
}
