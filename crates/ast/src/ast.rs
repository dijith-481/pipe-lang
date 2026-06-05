use bumpalo::Bump;
use bumpalo::collections::Vec;

use crate::span::Span;

/// A complete program consisting of top-level declarations.
#[derive(Debug, Clone)]
pub struct Program<'a> {
    pub decls: Vec<'a, Decl<'a>>,
}

/// Top-level declarations.
#[derive(Debug, Clone)]
pub enum Decl<'a> {
    /// A function definition: `name = (params) => body`
    Function {
        name: &'a str,
        params: Vec<'a, &'a str>,
        body: &'a Expr<'a>,
        span: Span,
    },
    /// A type alias: `type Name = TypeExpr`
    TypeAlias {
        name: &'a str,
        params: Vec<'a, &'a str>,
        rhs: &'a TypeExpr<'a>,
        span: Span,
    },
}

/// Expressions in the language.
#[derive(Debug, Clone)]
pub enum Expr<'a> {
    /// Integer literal: `42`
    Int(i64, Span),

    /// Float literal: `3.14`
    Float(f64, Span),

    /// String literal: `"hello"`
    Str(&'a str, Span),

    /// Boolean literal: `true` / `false`
    Bool(bool, Span),

    /// Identifier: `x`, `add`, `User.name`
    Ident(&'a str, Span),

    /// Function application: `f(x, y)`
    Application {
        func: &'a Expr<'a>,
        args: Vec<'a, &'a Expr<'a>>,
        span: Span,
    },

    /// Lambda expression: `(a, b) => a + b`
    Lambda {
        params: Vec<'a, Param<'a>>,
        body: &'a Expr<'a>,
        span: Span,
    },

    /// Pipeline operator: `x |> f |> g`
    Pipeline {
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
    },

    /// Binary operation: `a + b`, `a == b`
    Binary {
        op: BinOp,
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
    },

    /// Unary operation: `!x`, `-x`
    Unary {
        op: UnaryOp,
        operand: &'a Expr<'a>,
        span: Span,
    },

    /// Let expression: `let x = e1 in e2`
    Let {
        name: &'a str,
        value: &'a Expr<'a>,
        body: &'a Expr<'a>,
        span: Span,
    },

    /// Match expression: `match subject { pattern => arm, ... }`
    Match {
        subject: &'a Expr<'a>,
        arms: Vec<'a, MatchArm<'a>>,
        span: Span,
    },

    /// Block expression: `{ stmts; expr }`
    Block {
        stmts: Vec<'a, Stmt<'a>>,
        result: &'a Expr<'a>,
        span: Span,
    },

    /// Record literal: `{ name: "Alice", age: 30 }`
    Record {
        fields: Vec<'a, RecordField<'a>>,
        span: Span,
    },

    /// Record field access: `user.name`
    FieldAccess {
        object: &'a Expr<'a>,
        field: &'a str,
        span: Span,
    },

    /// Tuple literal: `(a, b, c)`
    Tuple {
        elems: Vec<'a, &'a Expr<'a>>,
        span: Span,
    },

    /// If expression: `if cond then a else b`
    If {
        condition: &'a Expr<'a>,
        then_branch: &'a Expr<'a>,
        else_branch: Option<&'a Expr<'a>>,
        span: Span,
    },

    /// Do block for effectful computation: `do { x <- effect; ... }`
    Do {
        stmts: Vec<'a, DoStmt<'a>>,
        span: Span,
    },
}

/// A function parameter with an optional type annotation.
#[derive(Debug, Clone)]
pub struct Param<'a> {
    pub name: &'a str,
    pub ty: Option<&'a TypeExpr<'a>>,
}

/// A statement within a block (not the last expression).
#[derive(Debug, Clone)]
pub enum Stmt<'a> {
    /// A let binding: `let x = expr`
    Let { name: &'a str, value: &'a Expr<'a> },
    /// An expression whose value is discarded
    Expr(&'a Expr<'a>),
}

/// A statement inside a do block.
#[derive(Debug, Clone)]
pub enum DoStmt<'a> {
    /// A bind: `x <- effect_expr`
    Bind { name: &'a str, value: &'a Expr<'a> },
    /// An expression (side-effectful)
    Expr(&'a Expr<'a>),
}

/// A match arm: `pattern => expression`
#[derive(Debug, Clone)]
pub struct MatchArm<'a> {
    pub pattern: &'a Pattern<'a>,
    pub body: &'a Expr<'a>,
}

/// A record field in a record literal.
#[derive(Debug, Clone)]
pub struct RecordField<'a> {
    pub name: &'a str,
    pub value: &'a Expr<'a>,
}

/// Patterns for match expressions.
#[derive(Debug, Clone)]
pub enum Pattern<'a> {
    /// Wildcard: `_`
    Wildcard(Span),

    /// Literal pattern: `42`, `"hello"`, `true`
    Literal(LiteralPattern<'a>),

    /// Constructor pattern: `Some(x)`, `None`, `Ok(val)`
    Constructor {
        name: &'a str,
        fields: Vec<'a, Pattern<'a>>,
        span: Span,
    },

    /// Binding pattern: `x` (binds the matched value)
    Binding(&'a str, Span),

    /// Tuple pattern: `(a, b)`
    Tuple {
        patterns: Vec<'a, Pattern<'a>>,
        span: Span,
    },

    /// Record pattern: `{ name, age }`
    Record {
        fields: Vec<'a, RecordPatternField<'a>>,
        span: Span,
    },
}

/// A field in a record pattern.
#[derive(Debug, Clone)]
pub struct RecordPatternField<'a> {
    pub name: &'a str,
    pub pattern: Option<&'a Pattern<'a>>,
}

/// Literal values used in patterns.
#[derive(Debug, Clone)]
pub enum LiteralPattern<'a> {
    Int(i64),
    Float(f64),
    Str(&'a str),
    Bool(bool),
}

/// Type expressions in type annotations.
#[derive(Debug, Clone)]
pub enum TypeExpr<'a> {
    /// Simple type name: `Int`, `Str`, `Bool`
    Named(&'a str, Span),

    /// Type application: `Array<Int>`, `Option<Str>`
    Apply {
        func: &'a TypeExpr<'a>,
        arg: &'a TypeExpr<'a>,
        span: Span,
    },

    /// Function type: `Int -> Str -> Bool`
    Function {
        from: &'a TypeExpr<'a>,
        to: &'a TypeExpr<'a>,
        span: Span,
    },

    /// Tuple type: `(Int, Str)`
    Tuple {
        types: Vec<'a, TypeExpr<'a>>,
        span: Span,
    },

    /// Record type: `{ name: Str, age: Int }`
    Record {
        fields: Vec<'a, TypeField<'a>>,
        span: Span,
    },
}

/// A field in a record type.
#[derive(Debug, Clone)]
pub struct TypeField<'a> {
    pub name: &'a str,
    pub ty: &'a TypeExpr<'a>,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    PipeRight,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl<'a> Expr<'a> {
    /// Create an integer literal expression.
    pub fn int(val: i64, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Int(val, span))
    }

    /// Create a string literal expression.
    pub fn str(val: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Str(val, span))
    }

    /// Create a boolean literal expression.
    pub fn bool(val: bool, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Bool(val, span))
    }

    /// Create an identifier expression.
    pub fn ident(name: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Ident(name, span))
    }

    /// Create a binary operation expression.
    pub fn binary(
        op: BinOp,
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
        arena: &'a Bump,
    ) -> &'a Self {
        arena.alloc(Expr::Binary {
            op,
            left,
            right,
            span,
        })
    }

    /// Create a pipeline expression.
    pub fn pipeline(
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
        arena: &'a Bump,
    ) -> &'a Self {
        arena.alloc(Expr::Pipeline { left, right, span })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(s: usize, e: usize) -> Span {
        Span::new(s, e)
    }

    #[test]
    fn construct_function_declaration() {
        let bump = Bump::new();
        let params = Vec::from_iter_in(["a", "b"].into_iter(), &bump);
        let lhs = Expr::ident("a", sp(15, 16), &bump);
        let rhs = Expr::ident("b", sp(19, 20), &bump);
        let body = Expr::binary(BinOp::Add, lhs, rhs, sp(15, 20), &bump);
        let decl = Decl::Function {
            name: "add",
            params,
            body,
            span: sp(0, 20),
        };
        match decl {
            Decl::Function {
                name,
                params,
                body,
                span,
            } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert!(matches!(body, Expr::Binary { op: BinOp::Add, .. }));
                assert_eq!(span, sp(0, 20));
            }
            _ => panic!("expected Function declaration"),
        }
    }

    #[test]
    fn construct_pipeline_expression() {
        let bump = Bump::new();
        let x = Expr::ident("users", sp(0, 5), &bump);
        let filter = Expr::ident("filter", sp(8, 14), &bump);
        let step1 = Expr::pipeline(x, filter, sp(0, 14), &bump);
        let map = Expr::ident("map", sp(17, 20), &bump);
        let step2 = Expr::pipeline(step1, map, sp(0, 20), &bump);
        match step2 {
            Expr::Pipeline { left, right, .. } => {
                assert!(matches!(left, Expr::Pipeline { .. }));
                assert!(matches!(right, Expr::Ident("map", _)));
            }
            _ => panic!("expected Pipeline"),
        }
    }

    #[test]
    fn construct_match_expression() {
        let bump = Bump::new();
        let subject = Expr::ident("opt", sp(6, 9), &bump);
        let arm_pattern = arena_alloc_pattern(
            &bump,
            Pattern::Constructor {
                name: "Some",
                fields: Vec::from_iter_in([Pattern::Binding("x", sp(18, 19))].into_iter(), &bump),
                span: sp(14, 20),
            },
        );
        let arm_body = Expr::ident("x", sp(24, 25), &bump);
        let arm = MatchArm {
            pattern: arm_pattern,
            body: arm_body,
        };
        let arms = Vec::from_iter_in([arm].into_iter(), &bump);
        let expr = Expr::Match {
            subject,
            arms,
            span: sp(0, 28),
        };
        match expr {
            Expr::Match { arms, .. } => {
                assert_eq!(arms.len(), 1);
            }
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn construct_lambda_expression() {
        let bump = Bump::new();
        let params = Vec::from_iter_in(
            [Param {
                name: "x",
                ty: None,
            }]
            .into_iter(),
            &bump,
        );
        let body = Expr::ident("x", sp(11, 12), &bump);
        let lambda = Expr::Lambda {
            params,
            body,
            span: sp(0, 12),
        };
        match lambda {
            Expr::Lambda { params, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn construct_let_expression() {
        let bump = Bump::new();
        let value = Expr::int(5, sp(8, 9), &bump);
        let body = Expr::ident("x", sp(16, 17), &bump);
        let expr = Expr::Let {
            name: "x",
            value,
            body,
            span: sp(0, 17),
        };
        match expr {
            Expr::Let { name, .. } => {
                assert_eq!(name, "x");
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn construct_type_alias() {
        let bump = Bump::new();
        let rhs = TypeExpr::Named("Int", sp(18, 21));
        let decl = Decl::TypeAlias {
            name: "UserId",
            params: Vec::new_in(&bump),
            rhs: &rhs,
            span: sp(0, 21),
        };
        match decl {
            Decl::TypeAlias { name, .. } => {
                assert_eq!(name, "UserId");
            }
            _ => panic!("expected TypeAlias"),
        }
    }

    #[test]
    fn type_expr_function_arrow() {
        let from = TypeExpr::Named("Int", sp(0, 3));
        let to = TypeExpr::Named("Str", sp(7, 10));
        let func_type = TypeExpr::Function {
            from: &from,
            to: &to,
            span: sp(0, 10),
        };
        match func_type {
            TypeExpr::Function { .. } => {}
            _ => panic!("expected Function type"),
        }
    }

    #[test]
    fn pattern_wildcard() {
        let p = Pattern::Wildcard(sp(0, 1));
        assert!(matches!(p, Pattern::Wildcard(_)));
    }

    #[test]
    fn pattern_binding() {
        let p = Pattern::Binding("x", sp(0, 1));
        match p {
            Pattern::Binding(name, span) => {
                assert_eq!(name, "x");
                assert_eq!(span, sp(0, 1));
            }
            _ => panic!("expected Binding"),
        }
    }

    fn arena_alloc_pattern<'a>(bump: &'a Bump, p: Pattern<'a>) -> &'a Pattern<'a> {
        bump.alloc(p)
    }
}
