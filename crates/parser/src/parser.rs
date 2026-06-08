use crate::error::ParseError;
use ast::ast::*;
use ast::span::Span;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use lexer::{Lexer, Token, TokenKind};

/// Parses the source string into a complete Program AST.
///
/// # Errors
///
/// Returns [`ParseError`] if the source contains syntax errors.
pub fn parse<'a>(source: &'a str, arena: &'a Bump) -> Result<Program<'a>, ParseError> {
    let mut parser = Parser::new(source, arena)?;
    parser.parse_program()
}

struct Parser<'a> {
    source: &'a str,
    lexer: Lexer<'a>,
    lookahead: std::collections::VecDeque<Token<'a>>,
    arena: &'a Bump,
    last_span: Span,
    lexer_error: Option<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, arena: &'a Bump) -> Result<Self, ParseError> {
        Ok(Self {
            source,
            lexer: Lexer::new(source),
            lookahead: std::collections::VecDeque::new(),
            arena,
            last_span: Span::empty(0),
            lexer_error: None,
        })
    }

    fn fill_lookahead(&mut self, n: usize) {
        if self.lexer_error.is_some() {
            return;
        }
        while self.lookahead.len() <= n {
            match self.lexer.next() {
                Some(Ok(tok)) => {
                    if !tok.kind.is_trivial() {
                        self.lookahead.push_back(tok);
                    }
                }
                Some(Err(e)) => {
                    self.lexer_error = Some(ParseError::UnexpectedToken {
                        expected: vec!["valid token".to_string()],
                        found: format!("lexer error: {e:?}"),
                        span: e.span(),
                    });
                    break;
                }
                None => break,
            }
        }
    }

    fn peek(&mut self) -> Option<Token<'a>> {
        self.fill_lookahead(0);
        self.lookahead.front().cloned()
    }

    fn peek_n(&mut self, n: usize) -> Option<Token<'a>> {
        self.fill_lookahead(n);
        self.lookahead.get(n).cloned()
    }

    fn peek_kind(&mut self) -> Option<TokenKind<'a>> {
        self.peek().map(|t| t.kind)
    }

    // TODO: remove expect
    fn advance(&mut self) -> Token<'a> {
        self.fill_lookahead(0);
        let tok = self
            .lookahead
            .pop_front()
            .expect("advance called when no tokens left");
        self.last_span = tok.span;
        tok
    }

    fn check(&mut self, kind: &TokenKind<'a>) -> bool {
        self.peek_kind().as_ref().is_some_and(|k| k == kind)
    }

    fn match_token(&mut self, kind: &TokenKind<'a>) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind<'a>) -> Result<Token<'a>, ParseError> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            let peeked = self.peek();
            let span = peeked.as_ref().map(|t| t.span).unwrap_or_else(|| {
                if self.last_span.end > 0 {
                    Span::new(self.last_span.end, self.last_span.end)
                } else {
                    Span::new(0, 0)
                }
            });
            let found = peeked
                .map(|t| format!("{:?}", t.kind))
                .unwrap_or_else(|| "EOF".to_string());
            Err(ParseError::UnexpectedToken {
                expected: vec![format!("{kind:?}")],
                found,
                span,
            })
        }
    }

    fn expect_ident(&mut self) -> Result<(&'a str, Span), ParseError> {
        if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let tok = self.advance();
            Ok((name, tok.span))
        } else {
            let peeked = self.peek();
            let span = peeked.as_ref().map(|t| t.span).unwrap_or_else(|| {
                if self.last_span.end > 0 {
                    Span::new(self.last_span.end, self.last_span.end)
                } else {
                    Span::new(0, 0)
                }
            });
            let found = peeked
                .map(|t| format!("{:?}", t.kind))
                .unwrap_or_else(|| "EOF".to_string());
            Err(ParseError::UnexpectedToken {
                expected: vec!["identifier".to_string()],
                found,
                span,
            })
        }
    }

    fn parse_program(&mut self) -> Result<Program<'a>, ParseError> {
        let mut decls = BumpVec::new_in(self.arena);
        while self.peek_kind().is_some() && !self.check(&TokenKind::Eof) {
            self.parse_decl_into(&mut decls)?;
        }
        if let Some(err) = self.lexer_error.take() {
            return Err(err);
        }
        Ok(Program { decls })
    }

    fn parse_decl_into(&mut self, decls: &mut BumpVec<'a, Decl<'a>>) -> Result<(), ParseError> {
        let start_tok = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
            expected: vec!["declaration".to_string()],
            span: Span::empty(self.source.len()),
        })?;

        match &start_tok.kind {
            TokenKind::Use => {
                self.advance();
                let mut path = BumpVec::new_in(self.arena);
                let first_ident = self.expect_ident()?;
                path.push(first_ident.0);
                while self.match_token(&TokenKind::PathSep) {
                    let next_ident = self.expect_ident()?;
                    path.push(next_ident.0);
                }
                let end_span = self.last_span;
                decls.push(Decl::Use {
                    path,
                    span: Span::new(start_tok.span.start, end_span.end),
                });
                Ok(())
            }
            TokenKind::Type => {
                self.advance();
                let (name, _) = self.expect_ident()?;
                let mut params = BumpVec::new_in(self.arena);
                if self.match_token(&TokenKind::Lt) {
                    loop {
                        let param = self.expect_ident()?;
                        params.push(param.0);
                        if self.match_token(&TokenKind::Gt) {
                            break;
                        }
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::Assign)?;
                let rhs = self.parse_type_expr()?;
                let end_span = self.last_span;
                decls.push(Decl::TypeAlias {
                    name,
                    params,
                    rhs,
                    span: Span::new(start_tok.span.start, end_span.end),
                });
                Ok(())
            }
            TokenKind::Let => {
                self.advance();
                let (name, name_span) = self.expect_ident()?;
                let mut ty = None;
                if self.match_token(&TokenKind::Colon) {
                    ty = Some(self.parse_type_expr()?);
                }
                if self.match_token(&TokenKind::Assign) {
                    let value = self.parse_expr()?;
                    let end_span = self.last_span;
                    decls.push(Decl::Bind {
                        name,
                        ty,
                        value,
                        span: Span::new(start_tok.span.start, end_span.end),
                    });
                } else if ty.is_some() {
                    let end_span = self.last_span;
                    decls.push(Decl::Bind {
                        name,
                        ty,
                        value: self.arena.alloc(Expr::Tuple {
                            elems: BumpVec::new_in(self.arena),
                            span: Span::empty(end_span.end),
                        }),
                        span: Span::new(start_tok.span.start, end_span.end),
                    });
                } else {
                    return Err(ParseError::UnexpectedToken {
                        expected: vec![":".to_string(), "=".to_string()],
                        found: format!("{:?}", self.peek_kind()),
                        span: name_span,
                    });
                }
                Ok(())
            }
            _ => {
                let tok = self.advance();
                Err(ParseError::UnexpectedToken {
                    expected: vec!["let".to_string(), "type".to_string(), "use".to_string()],
                    found: format!("{:?}", tok.kind),
                    span: tok.span,
                })
            }
        }
    }

    fn parse_type_expr(&mut self) -> Result<&'a TypeExpr<'a>, ParseError> {
        if self.check(&TokenKind::Bar) {
            return self.parse_type_sum();
        }
        self.parse_type_function()
    }

    fn parse_type_function(&mut self) -> Result<&'a TypeExpr<'a>, ParseError> {
        let start_span = self.peek().map(|t| t.span).unwrap_or(Span::empty(0));
        let left = self.parse_type_apply()?;
        if self.match_token(&TokenKind::FuncArrow) {
            let right = self.parse_type_function()?;
            let end_span = self.last_span;
            Ok(self.arena.alloc(TypeExpr::Function {
                from: left,
                to: right,
                span: Span::new(start_span.start, end_span.end),
            }))
        } else {
            Ok(left)
        }
    }

    fn parse_type_apply(&mut self) -> Result<&'a TypeExpr<'a>, ParseError> {
        let mut current = self.parse_type_atom()?;
        while self.match_token(&TokenKind::Lt) {
            loop {
                let arg = self.parse_type_expr()?;
                let end_span = self.last_span;
                current = self.arena.alloc(TypeExpr::Apply {
                    func: current,
                    arg,
                    span: Span::new(current.span().start, end_span.end),
                });
                if self.match_token(&TokenKind::Gt) {
                    break;
                }
                self.expect(TokenKind::Comma)?;
            }
        }
        Ok(current)
    }

    fn parse_type_atom(&mut self) -> Result<&'a TypeExpr<'a>, ParseError> {
        let peeked = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
            expected: vec!["type expression".to_string()],
            span: Span::empty(self.source.len()),
        })?;

        match &peeked.kind {
            TokenKind::Ident(name) => {
                let tok = self.advance();
                Ok(self.arena.alloc(TypeExpr::Named(name, tok.span)))
            }
            TokenKind::OpenParen => {
                let start_tok = self.advance();
                let mut types = BumpVec::new_in(self.arena);
                let mut has_comma = false;
                while !self.check(&TokenKind::CloseParen) {
                    types.push(self.parse_type_expr()?.clone());
                    if self.match_token(&TokenKind::Comma) {
                        has_comma = true;
                    }
                }
                let end_tok = self.expect(TokenKind::CloseParen)?;
                if types.len() == 1 && !has_comma {
                    Ok(self.arena.alloc(types.pop().unwrap()))
                } else {
                    Ok(self.arena.alloc(TypeExpr::Tuple {
                        types,
                        span: Span::new(start_tok.span.start, end_tok.span.end),
                    }))
                }
            }
            TokenKind::OpenBrace => {
                let start_tok = self.advance();
                let mut fields = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseBrace) {
                    let (name, _) = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type_expr()?;
                    fields.push(TypeField { name, ty });
                    self.match_token(&TokenKind::Comma);
                }
                let end_tok = self.expect(TokenKind::CloseBrace)?;
                Ok(self.arena.alloc(TypeExpr::Record {
                    fields,
                    span: Span::new(start_tok.span.start, end_tok.span.end),
                }))
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec!["identifier".to_string(), "(".to_string(), "{".to_string()],
                found: format!("{:?}", peeked.kind),
                span: peeked.span,
            }),
        }
    }

    fn parse_type_sum(&mut self) -> Result<&'a TypeExpr<'a>, ParseError> {
        let start_span = self.peek().map(|t| t.span).unwrap_or(Span::empty(0));
        let mut variants = BumpVec::new_in(self.arena);

        while self.match_token(&TokenKind::Bar) {
            let (name, name_span) = self.expect_ident()?;
            let mut fields = BumpVec::new_in(self.arena);
            let mut end_span = name_span;
            if self.match_token(&TokenKind::OpenParen) {
                loop {
                    fields.push(self.parse_type_expr()?.clone());
                    if self.match_token(&TokenKind::CloseParen) {
                        break;
                    }
                    self.expect(TokenKind::Comma)?;
                }
                end_span = self.last_span;
            }
            variants.push(TypeVariant {
                name,
                fields,
                span: Span::new(name_span.start, end_span.end),
            });
        }

        let end_span = self.last_span;
        Ok(self.arena.alloc(TypeExpr::Sum {
            variants,
            span: Span::new(start_span.start, end_span.end),
        }))
    }

    fn parse_expr(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        self.parse_expr_with_precedence(0)
    }

    fn parse_expr_with_precedence(&mut self, min_prec: u8) -> Result<&'a Expr<'a>, ParseError> {
        let mut left = self.parse_unary_or_primary()?;

        while let Some(peeked_kind) = self.peek_kind() {
            if let Some(prec) = self.get_binop_precedence(&peeked_kind) {
                if prec < min_prec {
                    break;
                }
                let tok = self.advance();
                let op = self.to_binop(&tok.kind);
                let next_min_prec = prec + 1;
                let right = self.parse_expr_with_precedence(next_min_prec)?;
                let end_span = self.last_span;
                left = self.arena.alloc(Expr::Binary {
                    op,
                    left,
                    right,
                    span: Span::new(left.span().start, end_span.end),
                });
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn get_binop_precedence(&self, kind: &TokenKind<'a>) -> Option<u8> {
        match kind {
            TokenKind::Or => Some(1),
            TokenKind::And => Some(2),
            TokenKind::Eq
            | TokenKind::Ne
            | TokenKind::Lt
            | TokenKind::Le
            | TokenKind::Gt
            | TokenKind::Ge => Some(3),
            TokenKind::Plus | TokenKind::Minus => Some(5),
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some(6),
            _ => None,
        }
    }

    fn to_binop(&self, kind: &TokenKind<'a>) -> BinOp {
        match kind {
            TokenKind::Or => BinOp::Or,
            TokenKind::And => BinOp::And,
            TokenKind::Eq => BinOp::Eq,
            TokenKind::Ne => BinOp::Ne,
            TokenKind::Lt => BinOp::Lt,
            TokenKind::Le => BinOp::Le,
            TokenKind::Gt => BinOp::Gt,
            TokenKind::Ge => BinOp::Ge,
            TokenKind::Plus => BinOp::Add,
            TokenKind::Minus => BinOp::Sub,
            TokenKind::Star => BinOp::Mul,
            TokenKind::Slash => BinOp::Div,
            TokenKind::Percent => BinOp::Mod,
            _ => unreachable!(),
        }
    }

    fn parse_unary_or_primary(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        if let Some(TokenKind::Not | TokenKind::Minus) = self.peek_kind() {
            let tok = self.advance();
            let op = match tok.kind {
                TokenKind::Not => UnaryOp::Not,
                TokenKind::Minus => UnaryOp::Neg,
                _ => unreachable!(),
            };
            let operand = self.parse_unary_or_primary()?;
            let end_span = self.last_span;
            return Ok(self.arena.alloc(Expr::Unary {
                op,
                operand,
                span: Span::new(tok.span.start, end_span.end),
            }));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let peeked = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
            expected: vec!["expression".to_string()],
            span: Span::empty(self.source.len()),
        })?;

        let expr = match &peeked.kind {
            TokenKind::If => {
                let start_tok = self.advance();
                let condition = self.parse_expr()?;
                let then_branch = self.parse_expr()?;
                self.expect(TokenKind::Else)?;
                let else_branch = self.parse_expr()?;
                let end_span = self.last_span;
                self.arena.alloc(Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                    span: Span::new(start_tok.span.start, end_span.end),
                })
            }
            TokenKind::Match => {
                let start_tok = self.advance();
                let subject = self.parse_expr()?;
                self.expect(TokenKind::OpenBrace)?;
                let mut arms = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseBrace) {
                    let pattern = self.parse_pattern()?;
                    self.expect(TokenKind::Arrow)?;
                    let body = self.parse_expr()?;
                    arms.push(MatchArm {
                        pattern: self.arena.alloc(pattern.clone()),
                        body,
                    });
                    self.match_token(&TokenKind::Comma);
                }
                let end_tok = self.expect(TokenKind::CloseBrace)?;
                self.arena.alloc(Expr::Match {
                    subject,
                    arms,
                    span: Span::new(start_tok.span.start, end_tok.span.end),
                })
            }
            TokenKind::Ident(name) => {
                let tok = self.advance();
                self.arena.alloc(Expr::Ident(name, tok.span))
            }
            TokenKind::Int(text) => {
                let tok = self.advance();
                self.arena.alloc(Expr::IntLiteral(text, tok.span))
            }
            TokenKind::Float(text) => {
                let tok = self.advance();
                self.arena.alloc(Expr::FloatLiteral(text, tok.span))
            }
            TokenKind::Str(text) => {
                let tok = self.advance();
                self.arena.alloc(Expr::Str(text, tok.span))
            }
            TokenKind::True => {
                let tok = self.advance();
                self.arena.alloc(Expr::Bool(true, tok.span))
            }
            TokenKind::False => {
                let tok = self.advance();
                self.arena.alloc(Expr::Bool(false, tok.span))
            }
            TokenKind::Backtick => self.parse_template_expr()?,
            TokenKind::OpenBracket => {
                let start_tok = self.advance();
                let mut elems = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseBracket) {
                    elems.push(self.parse_expr()?);
                    if !self.check(&TokenKind::CloseBracket) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                let end_tok = self.expect(TokenKind::CloseBracket)?;
                self.arena.alloc(Expr::Array {
                    elems,
                    span: Span::new(start_tok.span.start, end_tok.span.end),
                })
            }
            TokenKind::OpenBrace => {
                let start_tok = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
                    expected: vec!["expression".to_string()],
                    span: Span::empty(self.source.len()),
                })?;

                // Determine if it is a record or a block.
                let is_record = if let (Some(t1), Some(t2)) = (self.peek_n(1), self.peek_n(2)) {
                    matches!(t1.kind, TokenKind::Ident(_)) && matches!(t2.kind, TokenKind::Colon)
                } else {
                    false
                };

                if is_record {
                    self.advance(); // consume OpenBrace
                    let mut fields = BumpVec::new_in(self.arena);
                    while !self.check(&TokenKind::CloseBrace) {
                        let (name, _) = self.expect_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let value = self.parse_expr()?;
                        fields.push(RecordField { name, value });
                        self.match_token(&TokenKind::Comma);
                    }
                    let end_tok = self.expect(TokenKind::CloseBrace)?;
                    self.arena.alloc(Expr::Record {
                        fields,
                        span: Span::new(start_tok.span.start, end_tok.span.end),
                    })
                } else {
                    self.advance(); // consume OpenBrace
                    let mut stmts = BumpVec::new_in(self.arena);
                    let mut result_expr = None;

                    while !self.check(&TokenKind::CloseBrace) {
                        if self.match_token(&TokenKind::Let) {
                            let pattern = self.parse_pattern()?;
                            self.expect(TokenKind::Assign)?;
                            let value = self.parse_expr()?;
                            stmts.push(Stmt::Let { pattern, value });
                            self.match_token(&TokenKind::Semicolon);
                        } else {
                            let expr = self.parse_expr()?;
                            // If the next non-trivial token is a CloseBrace, this is the final block expression.
                            if self.check(&TokenKind::CloseBrace) {
                                result_expr = Some(expr);
                                break;
                            } else {
                                stmts.push(Stmt::Expr(expr));
                                self.match_token(&TokenKind::Semicolon);
                            }
                        }
                    }

                    let end_tok = self.expect(TokenKind::CloseBrace)?;
                    let result = match result_expr {
                        Some(r) => r,
                        None => self.arena.alloc(Expr::Tuple {
                            elems: BumpVec::new_in(self.arena),
                            span: Span::empty(end_tok.span.start),
                        }),
                    };

                    self.arena.alloc(Expr::Block {
                        stmts,
                        result,
                        span: Span::new(start_tok.span.start, end_tok.span.end),
                    })
                }
            }
            TokenKind::OpenParen => {
                if self.is_lambda_next() {
                    self.parse_lambda()?
                } else {
                    let start_tok = self.advance();
                    let mut elems = BumpVec::new_in(self.arena);
                    let mut has_comma = false;
                    while !self.check(&TokenKind::CloseParen) {
                        elems.push(self.parse_expr()?);
                        if self.match_token(&TokenKind::Comma) {
                            has_comma = true;
                        }
                    }
                    let end_tok = self.expect(TokenKind::CloseParen)?;
                    if elems.len() == 1 && !has_comma {
                        elems.pop().unwrap()
                    } else {
                        self.arena.alloc(Expr::Tuple {
                            elems,
                            span: Span::new(start_tok.span.start, end_tok.span.end),
                        })
                    }
                }
            }
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: vec!["expression".to_string()],
                    found: format!("{:?}", peeked.kind),
                    span: peeked.span,
                });
            }
        };

        let mut left = expr;
        loop {
            if self.check(&TokenKind::OpenParen) {
                if self.is_separated_by_newline() {
                    break;
                }
                self.advance();
                let mut args = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseParen) {
                    args.push(self.parse_expr()?);
                    if !self.check(&TokenKind::CloseParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                let end_tok = self.expect(TokenKind::CloseParen)?;
                left = self.arena.alloc(Expr::Application {
                    func: left,
                    args,
                    span: Span::new(left.span().start, end_tok.span.end),
                });
            } else if self.match_token(&TokenKind::Dot) {
                let (field, field_span) = self.expect_ident()?;
                if self.match_token(&TokenKind::OpenParen) {
                    let mut args = BumpVec::new_in(self.arena);
                    args.push(left);
                    while !self.check(&TokenKind::CloseParen) {
                        args.push(self.parse_expr()?);
                        if !self.check(&TokenKind::CloseParen) {
                            self.expect(TokenKind::Comma)?;
                        }
                    }
                    let end_tok = self.expect(TokenKind::CloseParen)?;
                    let func = self.arena.alloc(Expr::Ident(field, field_span));
                    left = self.arena.alloc(Expr::Application {
                        func,
                        args,
                        span: Span::new(left.span().start, end_tok.span.end),
                    });
                } else {
                    let end_span = field_span;
                    left = self.arena.alloc(Expr::FieldAccess {
                        object: left,
                        field,
                        span: Span::new(left.span().start, end_span.end),
                    });
                }
            } else if self.check(&TokenKind::OpenBracket) {
                if self.is_separated_by_newline() {
                    break;
                }
                self.advance();
                let index = self.parse_expr()?;
                let end_tok = self.expect(TokenKind::CloseBracket)?;
                left = self.arena.alloc(Expr::Index {
                    array: left,
                    index,
                    span: Span::new(left.span().start, end_tok.span.end),
                });
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn is_lambda_next(&mut self) -> bool {
        let mut depth = 0;
        let mut i = 0;
        while let Some(t) = self.peek_n(i) {
            match t.kind {
                TokenKind::OpenParen => depth += 1,
                TokenKind::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(next_tok) = self.peek_n(i + 1) {
                            return matches!(next_tok.kind, TokenKind::Arrow | TokenKind::Colon);
                        }
                        return false;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    }
    fn is_separated_by_newline(&mut self) -> bool {
        let start = self.last_span.end;
        if let Some(peeked) = self.peek() {
            let end = peeked.span.start;
            if start <= end && end <= self.source.len() {
                self.source[start..end].contains('\n')
            } else {
                false
            }
        } else {
            false
        }
    }

    fn parse_lambda(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let start_tok = self.expect(TokenKind::OpenParen)?;
        let mut params = BumpVec::new_in(self.arena);

        while !self.check(&TokenKind::CloseParen) {
            let name = if self.match_token(&TokenKind::Underscore) {
                "_"
            } else {
                let (id, _) = self.expect_ident()?;
                id
            };
            let mut ty = None;
            if self.match_token(&TokenKind::Colon) {
                ty = Some(self.parse_type_expr()?);
            }
            params.push(Param { name, ty });
            if !self.check(&TokenKind::CloseParen) {
                self.expect(TokenKind::Comma)?;
            }
        }
        self.expect(TokenKind::CloseParen)?;

        let mut return_type = None;
        if self.match_token(&TokenKind::Colon) {
            return_type = Some(self.parse_type_expr()?);
        }

        self.expect(TokenKind::Arrow)?;
        let body = self.parse_expr()?;
        let end_span = self.last_span;

        Ok(self.arena.alloc(Expr::Lambda {
            params,
            return_type,
            body,
            span: Span::new(start_tok.span.start, end_span.end),
        }))
    }

    fn parse_template_expr(&mut self) -> Result<&'a Expr<'a>, ParseError> {
        let start_tok = self.expect(TokenKind::Backtick)?;
        let mut parts = BumpVec::new_in(self.arena);

        while !self.check(&TokenKind::TemplateEnd) {
            let peeked = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
                expected: vec!["template content".to_string(), "TemplateEnd".to_string()],
                span: Span::empty(self.source.len()),
            })?;

            match &peeked.kind {
                TokenKind::TemplateStr(text) => {
                    self.advance();
                    parts.push(TemplatePart::Str(text));
                }
                TokenKind::TemplateHoleStart => {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::TemplateHoleEnd)?;
                    parts.push(TemplatePart::Expr(expr));
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: vec!["template string chunk".to_string(), "${".to_string()],
                        found: format!("{:?}", peeked.kind),
                        span: peeked.span,
                    });
                }
            }
        }

        let end_tok = self.expect(TokenKind::TemplateEnd)?;
        Ok(self.arena.alloc(Expr::Template {
            parts,
            span: Span::new(start_tok.span.start, end_tok.span.end),
        }))
    }

    fn parse_pattern(&mut self) -> Result<&'a Pattern<'a>, ParseError> {
        self.parse_pattern_atom()
    }

    fn parse_pattern_atom(&mut self) -> Result<&'a Pattern<'a>, ParseError> {
        let peeked = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
            expected: vec!["pattern".to_string()],
            span: Span::empty(self.source.len()),
        })?;

        match &peeked.kind {
            TokenKind::Underscore => {
                let tok = self.advance();
                Ok(self.arena.alloc(Pattern::Wildcard(tok.span)))
            }
            TokenKind::Int(val) => {
                let tok = self.advance();
                Ok(self
                    .arena
                    .alloc(Pattern::Literal(LiteralPattern::Int(val), tok.span)))
            }
            TokenKind::Float(val) => {
                let tok = self.advance();
                Ok(self
                    .arena
                    .alloc(Pattern::Literal(LiteralPattern::Float(val), tok.span)))
            }
            TokenKind::Str(val) => {
                let tok = self.advance();
                Ok(self
                    .arena
                    .alloc(Pattern::Literal(LiteralPattern::Str(val), tok.span)))
            }
            TokenKind::Backtick => {
                let start_tok = self.advance();
                let next = self.peek().ok_or_else(|| ParseError::UnexpectedEof {
                    expected: vec!["template content".to_string()],
                    span: Span::empty(self.source.len()),
                })?;
                let val = match next.kind {
                    TokenKind::TemplateStr(val) => {
                        self.advance();
                        self.expect(TokenKind::TemplateEnd)?;
                        val
                    }
                    TokenKind::TemplateEnd => {
                        self.advance();
                        ""
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            expected: vec!["constant template string in pattern".to_string()],
                            found: format!("{:?}", next.kind),
                            span: next.span,
                        });
                    }
                };
                Ok(self.arena.alloc(Pattern::Literal(
                    LiteralPattern::Str(val),
                    Span::new(start_tok.span.start, self.last_span.end),
                )))
            }
            TokenKind::True => {
                let tok = self.advance();
                Ok(self
                    .arena
                    .alloc(Pattern::Literal(LiteralPattern::Bool(true), tok.span)))
            }
            TokenKind::False => {
                let tok = self.advance();
                Ok(self
                    .arena
                    .alloc(Pattern::Literal(LiteralPattern::Bool(false), tok.span)))
            }
            TokenKind::Ident(name) => {
                let tok = self.advance();
                if name
                    .chars()
                    .next()
                    .is_some_and(|c: char| c.is_ascii_uppercase())
                {
                    let mut fields = BumpVec::new_in(self.arena);
                    let mut end_span = tok.span;
                    if self.match_token(&TokenKind::OpenParen) {
                        while !self.check(&TokenKind::CloseParen) {
                            fields.push(self.parse_pattern()?.clone());
                            if !self.check(&TokenKind::CloseParen) {
                                self.expect(TokenKind::Comma)?;
                            }
                        }
                        let end_tok = self.expect(TokenKind::CloseParen)?;
                        end_span = end_tok.span;
                    }
                    Ok(self.arena.alloc(Pattern::Constructor {
                        name,
                        fields,
                        span: Span::new(tok.span.start, end_span.end),
                    }))
                } else {
                    Ok(self.arena.alloc(Pattern::Binding(name, tok.span)))
                }
            }
            TokenKind::OpenBrace => {
                let start_tok = self.advance();
                let mut fields = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseBrace) {
                    let (name, _) = self.expect_ident()?;
                    let mut pattern = None;
                    if self.match_token(&TokenKind::Colon) {
                        pattern = Some(self.parse_pattern()?);
                    }
                    fields.push(RecordPatternField { name, pattern });
                    self.match_token(&TokenKind::Comma);
                }
                let end_tok = self.expect(TokenKind::CloseBrace)?;
                Ok(self.arena.alloc(Pattern::Record {
                    fields,
                    span: Span::new(start_tok.span.start, end_tok.span.end),
                }))
            }
            TokenKind::OpenParen => {
                let start_tok = self.advance();
                let mut patterns = BumpVec::new_in(self.arena);
                while !self.check(&TokenKind::CloseParen) {
                    patterns.push(self.parse_pattern()?.clone());
                    if !self.check(&TokenKind::CloseParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                let end_tok = self.expect(TokenKind::CloseParen)?;
                Ok(self.arena.alloc(Pattern::Tuple {
                    patterns,
                    span: Span::new(start_tok.span.start, end_tok.span.end),
                }))
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: vec!["pattern".to_string()],
                found: format!("{:?}", peeked.kind),
                span: peeked.span,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    fn parse_expr_helper<'a>(source: &'a str, arena: &'a Bump) -> &'a Expr<'a> {
        let mut parser = Parser::new(source, arena).unwrap();
        parser.parse_expr().unwrap()
    }

    fn parse_type_helper<'a>(source: &'a str, arena: &'a Bump) -> &'a TypeExpr<'a> {
        let mut parser = Parser::new(source, arena).unwrap();
        parser.parse_type_expr().unwrap()
    }

    #[test]
    fn test_parse_use() {
        let bump = Bump::new();
        let program = parse("use stdlib::io", &bump).unwrap();
        assert_eq!(program.decls.len(), 1);
        match &program.decls[0] {
            Decl::Use { path, .. } => {
                assert_eq!(path.len(), 2);
                assert_eq!(path[0], "stdlib");
                assert_eq!(path[1], "io");
            }
            _ => panic!("expected Use"),
        }
    }

    #[test]
    fn test_parse_type_alias() {
        let bump = Bump::new();
        let program = parse("type UserId = i32", &bump).unwrap();
        assert_eq!(program.decls.len(), 1);
        match &program.decls[0] {
            Decl::TypeAlias {
                name, params, rhs, ..
            } => {
                assert_eq!(*name, "UserId");
                assert!(params.is_empty());
                match rhs {
                    TypeExpr::Named(t, _) => assert_eq!(*t, "i32"),
                    _ => panic!("expected Named type"),
                }
            }
            _ => panic!("expected TypeAlias"),
        }
    }

    #[test]
    fn test_parse_let_bind() {
        let bump = Bump::new();
        let program = parse("let x = 42", &bump).unwrap();
        assert_eq!(program.decls.len(), 1);
        match &program.decls[0] {
            Decl::Bind { name, value, .. } => {
                assert_eq!(*name, "x");
                assert!(matches!(value, Expr::IntLiteral("42", _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn test_parse_type_sig() {
        let bump = Bump::new();
        let program = parse("let factorial : (i32) -> i64", &bump).unwrap();
        assert_eq!(program.decls.len(), 1);
        match &program.decls[0] {
            Decl::Bind { name, ty, .. } => {
                assert_eq!(*name, "factorial");
                assert!(ty.is_some());
                match ty.unwrap() {
                    TypeExpr::Function { from, to, .. } => {
                        match &**from {
                            TypeExpr::Named(f, _) => assert_eq!(*f, "i32"),
                            _ => panic!("expected i32 from type"),
                        }
                        match &**to {
                            TypeExpr::Named(t, _) => assert_eq!(*t, "i64"),
                            _ => panic!("expected i64 to type"),
                        }
                    }
                    _ => panic!("expected Function type"),
                }
            }
            _ => panic!("expected Bind with type"),
        }
    }

    #[test]
    fn test_parse_expr_atom() {
        let bump = Bump::new();
        assert!(matches!(
            parse_expr_helper("42", &bump),
            Expr::IntLiteral("42", _)
        ));
        assert!(matches!(
            parse_expr_helper("3.14", &bump),
            Expr::FloatLiteral("3.14", _)
        ));
        assert!(matches!(
            parse_expr_helper("\"hello\"", &bump),
            Expr::Str("hello", _)
        ));
        assert!(matches!(
            parse_expr_helper("true", &bump),
            Expr::Bool(true, _)
        ));
        assert!(matches!(parse_expr_helper("x", &bump), Expr::Ident("x", _)));
    }

    #[test]
    fn test_parse_lambda() {
        let bump = Bump::new();
        let expr = parse_expr_helper("(x: i32): i64 => x + 1", &bump);
        match expr {
            Expr::Lambda {
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
                assert!(return_type.is_some());
                assert!(matches!(body, Expr::Binary { op: BinOp::Add, .. }));
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn test_parse_application_and_desugaring() {
        let bump = Bump::new();
        // Regular application
        let expr = parse_expr_helper("f(1, 2)", &bump);
        match expr {
            Expr::Application { func, args, .. } => {
                assert!(matches!(&**func, Expr::Ident("f", _)));
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected Application"),
        }

        // Dot call method desugaring: x.map(f) => map(x, f)
        let desugared = parse_expr_helper("x.map(f)", &bump);
        match desugared {
            Expr::Application { func, args, .. } => {
                assert!(matches!(&**func, Expr::Ident("map", _)));
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0], Expr::Ident("x", _)));
                assert!(matches!(args[1], Expr::Ident("f", _)));
            }
            _ => panic!("expected desugared Application"),
        }
    }

    #[test]
    fn test_parse_binary_precedence() {
        let bump = Bump::new();
        // a + b * c => a + (b * c)
        let expr = parse_expr_helper("a + b * c", &bump);
        match expr {
            Expr::Binary {
                op: BinOp::Add,
                left,
                right,
                ..
            } => {
                assert!(matches!(&**left, Expr::Ident("a", _)));
                match &**right {
                    Expr::Binary { op: BinOp::Mul, .. } => {}
                    _ => panic!("expected Mul on right"),
                }
            }
            _ => panic!("expected Add binary"),
        }
    }

    #[test]
    fn test_parse_if_expr() {
        let bump = Bump::new();
        let expr = parse_expr_helper("if true { 1 } else { 2 }", &bump);
        match expr {
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                assert!(matches!(&**condition, Expr::Bool(true, _)));
                assert!(matches!(&**then_branch, Expr::Block { .. }));
                assert!(matches!(&**else_branch, Expr::Block { .. }));
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_parse_match_expr() {
        let bump = Bump::new();
        let expr = parse_expr_helper("match x { Some(y) => y, None => 0 }", &bump);
        match expr {
            Expr::Match { subject, arms, .. } => {
                assert!(matches!(&**subject, Expr::Ident("x", _)));
                assert_eq!(arms.len(), 2);
            }
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn test_parse_record_literal() {
        let bump = Bump::new();
        let expr = parse_expr_helper("{ name: \"Alice\", age: 30 }", &bump);
        match expr {
            Expr::Record { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "name");
                assert_eq!(fields[1].name, "age");
            }
            _ => panic!("expected Record literal"),
        }
    }

    #[test]
    fn test_parse_tuple_literal() {
        let bump = Bump::new();
        let expr = parse_expr_helper("(1, \"two\")", &bump);
        assert!(matches!(expr, Expr::Tuple { .. }));
    }

    #[test]
    fn test_parse_array_literal() {
        let bump = Bump::new();
        let expr = parse_expr_helper("[1, 2, 3]", &bump);
        assert!(matches!(expr, Expr::Array { .. }));
    }

    #[test]
    fn test_parse_field_access_and_index() {
        let bump = Bump::new();
        let expr = parse_expr_helper("user.name", &bump);
        assert!(matches!(expr, Expr::FieldAccess { .. }));

        let idx_expr = parse_expr_helper("grid[0][c]", &bump);
        match idx_expr {
            Expr::Index { array, index, .. } => {
                assert!(matches!(&**array, Expr::Index { .. }));
                assert!(matches!(&**index, Expr::Ident("c", _)));
            }
            _ => panic!("expected nested Index"),
        }
    }

    #[test]
    fn test_parse_template_literal() {
        let bump = Bump::new();
        let expr = parse_expr_helper("`Hi, ${name}!`", &bump);
        match expr {
            Expr::Template { parts, .. } => {
                assert_eq!(parts.len(), 3);
                assert!(matches!(&parts[0], TemplatePart::Str("Hi, ")));
                assert!(matches!(&parts[1], TemplatePart::Expr(_)));
                assert!(matches!(&parts[2], TemplatePart::Str("!")));
            }
            _ => panic!("expected Template literal"),
        }
    }

    #[test]
    fn test_parse_type_expressions() {
        let bump = Bump::new();
        // Named type
        assert!(matches!(
            parse_type_helper("i32", &bump),
            TypeExpr::Named("i32", _)
        ));

        // Nested application: Result<T, E> => Apply(Apply(Result, T), E)
        let result_type = parse_type_helper("Result<i32, str>", &bump);
        match result_type {
            TypeExpr::Apply { func, arg, .. } => {
                match &**func {
                    TypeExpr::Apply {
                        func: res,
                        arg: i32_ty,
                        ..
                    } => {
                        assert!(matches!(&**res, TypeExpr::Named("Result", _)));
                        assert!(matches!(&**i32_ty, TypeExpr::Named("i32", _)));
                    }
                    _ => panic!("expected inner Apply"),
                }
                assert!(matches!(&**arg, TypeExpr::Named("str", _)));
            }
            _ => panic!("expected Apply type"),
        }

        // Sum type
        let sum_type = parse_type_helper("| Some(T) | None", &bump);
        match sum_type {
            TypeExpr::Sum { variants, .. } => {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Some");
                assert_eq!(variants[1].name, "None");
            }
            _ => panic!("expected Sum type"),
        }
    }

    #[test]
    fn parse_all_example_programs() {
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
            let bump = Bump::new();
            let result = parse(&src, &bump);
            assert!(
                result.is_ok(),
                "{} failed to parse: {:?}",
                path.display(),
                result.err()
            );
        }
    }

    #[test]
    fn test_parse_negative_errors() {
        let bump = Bump::new();
        // Mismatched brace
        let result = parse("let x = { name: 1", &bump);
        assert!(result.is_err());
        // Missing else in if
        let result = parse("if true { 1 }", &bump);
        assert!(result.is_err());
    }
}
