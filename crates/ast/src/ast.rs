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
    /// A value binding: `let name [: Type] = expr`
    Bind {
        name: &'a str,
        ty: Option<&'a TypeExpr<'a>>,
        value: &'a Expr<'a>,
        span: Span,
    },
    /// A type alias: `type Name = TypeExpr`
    TypeAlias {
        name: &'a str,
        params: Vec<'a, &'a str>,
        rhs: &'a TypeExpr<'a>,
        span: Span,
    },
    /// A use declaration: `use stdlib::io`
    Use { path: Vec<'a, &'a str>, span: Span },
}

/// Expressions in the language.
#[derive(Debug, Clone)]
pub enum Expr<'a> {
    /// Integer literal: `42`, `42i32`, `255u8`
    IntLiteral(&'a str, Span),
    /// Float literal: `3.14`, `3.14f64`
    FloatLiteral(&'a str, Span),

    // -- Other primitives --
    /// String literal: `"hello"`
    Str(&'a str, Span),
    /// Boolean literal: `true` / `false`
    Bool(bool, Span),

    // -- Composite --
    /// Identifier: `x`, `add`, `user.name`
    Ident(&'a str, Span),

    /// Function application: `f(x, y)` or desugared method call
    Application {
        func: &'a Expr<'a>,
        args: Vec<'a, &'a Expr<'a>>,
        span: Span,
    },

    /// Lambda expression: `(a:i32, b:i32):i64 => a + b` or `(x) => x + 1`
    Lambda {
        params: Vec<'a, Param<'a>>,
        return_type: Option<&'a TypeExpr<'a>>,
        body: &'a Expr<'a>,
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

    /// If expression: `if cond { then } else { else }`
    If {
        condition: &'a Expr<'a>,
        then_branch: &'a Expr<'a>,
        else_branch: &'a Expr<'a>,
        span: Span,
    },

    /// Array literal: `[1, 2, 3]`
    Array {
        elems: Vec<'a, &'a Expr<'a>>,
        span: Span,
    },

    /// A template literal: `` `Hello, ${name}!` ``
    Template {
        parts: Vec<'a, TemplatePart<'a>>,
        span: Span,
    },

    /// Array indexing: `arr[idx]`
    Index {
        array: &'a Expr<'a>,
        index: &'a Expr<'a>,
        span: Span,
    },
}

/// A part of a template literal expression.
#[derive(Debug, Clone)]
pub enum TemplatePart<'a> {
    /// A constant string chunk: `"Hello, "`
    Str(&'a str),
    /// An interpolated expression chunk: `name`
    Expr(&'a Expr<'a>),
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
    /// A let binding: `let pattern = expr`
    Let {
        pattern: &'a Pattern<'a>,
        value: &'a Expr<'a>,
    },
    /// An expression whose value is discarded
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
    Literal(LiteralPattern<'a>, Span),

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
    Int(&'a str),
    Float(&'a str),
    Str(&'a str),
    Bool(bool),
}

/// Type expressions in type annotations.
#[derive(Debug, Clone)]
pub enum TypeExpr<'a> {
    /// Simple type name: `i32`, `f64`, `str`, `bool`
    Named(&'a str, Span),

    /// Type application: `Array<i32>`, `Option<str>`
    Apply {
        func: &'a TypeExpr<'a>,
        arg: &'a TypeExpr<'a>,
        span: Span,
    },

    /// Function type: `i32 -> str -> bool`
    Function {
        from: &'a TypeExpr<'a>,
        to: &'a TypeExpr<'a>,
        span: Span,
    },

    /// Tuple type: `(i32, str)`
    Tuple {
        types: Vec<'a, TypeExpr<'a>>,
        span: Span,
    },

    /// Record type: `{ name: str, age: i32 }`
    Record {
        fields: Vec<'a, TypeField<'a>>,
        span: Span,
    },

    /// A sum type / tagged union: `| Variant1(A) | Variant2`
    Sum {
        variants: Vec<'a, TypeVariant<'a>>,
        span: Span,
    },
}

/// A variant in a sum type definition.
#[derive(Debug, Clone)]
pub struct TypeVariant<'a> {
    pub name: &'a str,
    pub fields: Vec<'a, TypeExpr<'a>>,
    pub span: Span,
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
    pub fn int(text: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::IntLiteral(text, span))
    }

    pub fn float(text: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::FloatLiteral(text, span))
    }

    pub fn str(val: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Str(val, span))
    }

    pub fn bool(val: bool, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Bool(val, span))
    }

    pub fn ident(name: &'a str, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Ident(name, span))
    }

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

    pub fn field_access(
        object: &'a Expr<'a>,
        field: &'a str,
        span: Span,
        arena: &'a Bump,
    ) -> &'a Self {
        arena.alloc(Expr::FieldAccess {
            object,
            field,
            span,
        })
    }

    pub fn record(fields: Vec<'a, RecordField<'a>>, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Record { fields, span })
    }

    pub fn app(
        func: &'a Expr<'a>,
        args: Vec<'a, &'a Expr<'a>>,
        span: Span,
        arena: &'a Bump,
    ) -> &'a Self {
        arena.alloc(Expr::Application { func, args, span })
    }

    /// Returns true if this expression is a numeric literal of any type.
    #[must_use]
    pub fn is_numeric_literal(&self) -> bool {
        matches!(self, Expr::IntLiteral(..) | Expr::FloatLiteral(..))
    }

    /// Returns the span of this expression.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, span)
            | Expr::FloatLiteral(_, span)
            | Expr::Str(_, span)
            | Expr::Bool(_, span)
            | Expr::Ident(_, span)
            | Expr::Application { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Match { span, .. }
            | Expr::Block { span, .. }
            | Expr::Record { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::If { span, .. }
            | Expr::Array { span, .. }
            | Expr::Template { span, .. }
            | Expr::Index { span, .. } => *span,
        }
    }
}

impl<'a> TypeExpr<'a> {
    /// Returns the span of this type expression.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(_, span)
            | TypeExpr::Apply { span, .. }
            | TypeExpr::Function { span, .. }
            | TypeExpr::Tuple { span, .. }
            | TypeExpr::Record { span, .. }
            | TypeExpr::Sum { span, .. } => *span,
        }
    }
}

impl<'a> Pattern<'a> {
    /// Returns the span of this pattern.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(span)
            | Pattern::Literal(_, span)
            | Pattern::Constructor { span, .. }
            | Pattern::Binding(_, span)
            | Pattern::Tuple { span, .. }
            | Pattern::Record { span, .. } => *span,
        }
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
    fn construct_let_binding_with_lambda() {
        // let add = (a:i32, b:i32):i64 => a + b
        let bump = Bump::new();
        let lhs = Expr::ident("a", sp(18, 19), &bump);
        let rhs = Expr::ident("b", sp(22, 23), &bump);
        let body = Expr::binary(BinOp::Add, lhs, rhs, sp(18, 23), &bump);

        let ty_i32_1 = bump.alloc(TypeExpr::Named("i32", sp(12, 15)));
        let ty_i32_2 = bump.alloc(TypeExpr::Named("i32", sp(17, 20)));
        let ty_i64 = bump.alloc(TypeExpr::Named("i64", sp(25, 28)));

        let params = Vec::from_iter_in(
            [
                Param {
                    name: "a",
                    ty: Some(ty_i32_1),
                },
                Param {
                    name: "b",
                    ty: Some(ty_i32_2),
                },
            ],
            &bump,
        );

        let lambda = Expr::Lambda {
            params,
            return_type: Some(ty_i64),
            body,
            span: sp(9, 33),
        };

        let decl = Decl::Bind {
            name: "add",
            ty: None,
            value: &lambda,
            span: sp(0, 33),
        };

        match decl {
            Decl::Bind {
                name, value, span, ..
            } => {
                assert_eq!(name, "add");
                assert_eq!(span, sp(0, 33));
                match value {
                    Expr::Lambda {
                        params,
                        return_type,
                        ..
                    } => {
                        assert_eq!(params.len(), 2);
                        assert!(return_type.is_some());
                        match return_type.unwrap() {
                            TypeExpr::Named(n, _) => assert_eq!(*n, "i64"),
                            _ => panic!("expected Named type"),
                        }
                    }
                    _ => panic!("expected Lambda"),
                }
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_int_value() {
        // let x = 42
        let bump = Bump::new();
        let val = Expr::int("42", sp(13, 15), &bump);
        let decl = Decl::Bind {
            name: "x",
            ty: None,
            value: val,
            span: sp(0, 15),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(value, Expr::IntLiteral("42", _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_float_value() {
        // let pi = 3.14
        let bump = Bump::new();
        let val = Expr::float("3.14", sp(13, 17), &bump);
        let decl = Decl::Bind {
            name: "pi",
            ty: None,
            value: val,
            span: sp(0, 17),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "pi");
                assert!(matches!(value, Expr::FloatLiteral("3.14", _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn pattern_literal_int() {
        let p = Pattern::Literal(LiteralPattern::Int("42"), sp(0, 2));
        match p {
            Pattern::Literal(LiteralPattern::Int(v), _) => assert_eq!(v, "42"),
            _ => panic!("expected Int literal pattern"),
        }
    }

    #[test]
    fn is_numeric_literal_true() {
        let bump = Bump::new();
        assert!(Expr::int("1", sp(0, 1), &bump).is_numeric_literal());
        assert!(Expr::float("1.0", sp(0, 1), &bump).is_numeric_literal());
    }
}
