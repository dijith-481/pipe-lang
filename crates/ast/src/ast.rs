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
    /// A value binding: `let name = expr`
    Bind {
        name: &'a str,
        value: &'a Expr<'a>,
        span: Span,
    },
    /// A type signature: `let name : Type`
    TypeSig {
        name: &'a str,
        ty: &'a TypeExpr<'a>,
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
///
/// Each numeric type has its own variant to preserve type information
/// from parsing through JIT compilation. No implicit widening or
/// coercion — the type checker enforces explicit conversions.
#[derive(Debug, Clone)]
pub enum Expr<'a> {
    // -- Signed integers --
    /// Signed 8-bit integer: `42i8`
    I8(i8, Span),
    /// Signed 16-bit integer: `42i16`
    I16(i16, Span),
    /// Signed 32-bit integer: `42i32` or `42` (default)
    I32(i32, Span),
    /// Signed 64-bit integer: `42i64`
    I64(i64, Span),

    // -- Unsigned integers --
    /// Unsigned 8-bit integer: `42u8`
    U8(u8, Span),
    /// Unsigned 16-bit integer: `42u16`
    U16(u16, Span),
    /// Unsigned 32-bit integer: `42u32`
    U32(u32, Span),
    /// Unsigned 64-bit integer: `42u64`
    U64(u64, Span),
    /// Platform-dependent unsigned integer: `42usize` (used for indexing/sizes)
    Usize(usize, Span),

    // -- Floats --
    /// 32-bit float: `3.14f32`
    F32(f32, Span),
    /// 64-bit float: `3.14` (default) or `3.14f64`
    F64(f64, Span),

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
///
/// Each numeric type has its own variant so the type checker can
/// validate that pattern literals match the scrutinee type.
#[derive(Debug, Clone)]
pub enum LiteralPattern<'a> {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Usize(usize),
    F32(f32),
    F64(f64),
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
    pub fn i8(val: i8, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::I8(val, span))
    }

    pub fn i16(val: i16, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::I16(val, span))
    }

    pub fn i32(val: i32, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::I32(val, span))
    }

    pub fn i64(val: i64, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::I64(val, span))
    }

    pub fn u8(val: u8, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::U8(val, span))
    }

    pub fn u16(val: u16, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::U16(val, span))
    }

    pub fn u32(val: u32, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::U32(val, span))
    }

    pub fn u64(val: u64, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::U64(val, span))
    }

    pub fn usize(val: usize, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::Usize(val, span))
    }

    pub fn f32(val: f32, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::F32(val, span))
    }

    pub fn f64(val: f64, span: Span, arena: &'a Bump) -> &'a Self {
        arena.alloc(Expr::F64(val, span))
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
        matches!(
            self,
            Expr::I8(..)
                | Expr::I16(..)
                | Expr::I32(..)
                | Expr::I64(..)
                | Expr::U8(..)
                | Expr::U16(..)
                | Expr::U32(..)
                | Expr::U64(..)
                | Expr::Usize(..)
                | Expr::F32(..)
                | Expr::F64(..)
        )
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
            ]
            .into_iter(),
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
            value: &lambda,
            span: sp(0, 33),
        };

        match decl {
            Decl::Bind { name, value, span } => {
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
    fn construct_let_binding_with_i32_value() {
        // let x : i32 = 42
        let bump = Bump::new();
        let val = Expr::i32(42, sp(13, 15), &bump);
        let decl = Decl::Bind {
            name: "x",
            value: val,
            span: sp(0, 15),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(value, Expr::I32(42, _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_i64_value() {
        // let x : i64 = 42i64
        let bump = Bump::new();
        let val = Expr::i64(42, sp(13, 17), &bump);
        let decl = Decl::Bind {
            name: "x",
            value: val,
            span: sp(0, 17),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(value, Expr::I64(42, _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_unsigned_value() {
        // let count : u32 = 100u32
        let bump = Bump::new();
        let val = Expr::u32(100, sp(16, 22), &bump);
        let decl = Decl::Bind {
            name: "count",
            value: val,
            span: sp(0, 22),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "count");
                assert!(matches!(value, Expr::U32(100, _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_float_value() {
        // let pi : f64 = 3.14
        let bump = Bump::new();
        let val = Expr::f64(3.14, sp(13, 17), &bump);
        let decl = Decl::Bind {
            name: "pi",
            value: val,
            span: sp(0, 17),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "pi");
                assert!(matches!(value, Expr::F64(3.14, _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_let_binding_with_f32_value() {
        // let pi : f32 = 3.14f32
        let bump = Bump::new();
        let val = Expr::f32(3.14, sp(13, 19), &bump);
        let decl = Decl::Bind {
            name: "pi",
            value: val,
            span: sp(0, 19),
        };
        match decl {
            Decl::Bind { name, value, .. } => {
                assert_eq!(name, "pi");
                assert!(matches!(value, Expr::F32(3.14, _)));
            }
            _ => panic!("expected Bind"),
        }
    }

    #[test]
    fn construct_type_signature() {
        // let transition : i32 -> i32 -> str
        let bump = Bump::new();
        let from = bump.alloc(TypeExpr::Named("i32", sp(18, 21)));
        let mid = bump.alloc(TypeExpr::Named("i32", sp(25, 28)));
        let to = bump.alloc(TypeExpr::Named("str", sp(32, 35)));
        let func1 = bump.alloc(TypeExpr::Function {
            from: mid,
            to,
            span: sp(25, 35),
        });
        let func2 = TypeExpr::Function {
            from,
            to: func1,
            span: sp(18, 35),
        };
        let decl = Decl::TypeSig {
            name: "transition",
            ty: &func2,
            span: sp(0, 35),
        };
        match decl {
            Decl::TypeSig { name, ty, .. } => {
                assert_eq!(name, "transition");
                assert!(matches!(ty, TypeExpr::Function { .. }));
            }
            _ => panic!("expected TypeSig"),
        }
    }

    #[test]
    fn construct_closure_single_expression() {
        // (x) => x.age >= 30i32
        let bump = Bump::new();
        let obj = Expr::ident("x", sp(4, 5), &bump);
        let field = Expr::field_access(obj, "age", sp(4, 8), &bump);
        let lit = Expr::i32(30, sp(12, 14), &bump);
        let body = Expr::binary(BinOp::Ge, field, lit, sp(4, 14), &bump);

        let params = Vec::from_iter_in(
            [Param {
                name: "x",
                ty: None,
            }]
            .into_iter(),
            &bump,
        );

        let lambda = Expr::Lambda {
            params,
            return_type: None,
            body,
            span: sp(0, 14),
        };

        match lambda {
            Expr::Lambda {
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(params.len(), 1);
                assert!(return_type.is_none());
                assert!(matches!(body, Expr::Binary { op: BinOp::Ge, .. }));
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn construct_closure_with_block() {
        // (x) => { let y = x.age; y >= 30i32 }
        let bump = Bump::new();
        let obj = Expr::ident("x", sp(6, 7), &bump);
        let age = Expr::field_access(obj, "age", sp(6, 10), &bump);
        let let_stmt = Stmt::Let {
            name: "y",
            value: age,
        };
        let stmts = Vec::from_iter_in([let_stmt].into_iter(), &bump);
        let y = Expr::ident("y", sp(23, 24), &bump);
        let lit = Expr::i32(30, sp(28, 30), &bump);
        let result = Expr::binary(BinOp::Ge, y, lit, sp(23, 30), &bump);

        let body = Expr::Block {
            stmts,
            result,
            span: sp(4, 32),
        };

        let params = Vec::from_iter_in(
            [Param {
                name: "x",
                ty: None,
            }]
            .into_iter(),
            &bump,
        );

        let lambda = Expr::Lambda {
            params,
            return_type: None,
            body: &body,
            span: sp(0, 32),
        };

        match lambda {
            Expr::Lambda { params, body, .. } => {
                assert_eq!(params.len(), 1);
                match body {
                    Expr::Block { stmts, result, .. } => {
                        assert_eq!(stmts.len(), 1);
                        assert!(matches!(result, Expr::Binary { op: BinOp::Ge, .. }));
                    }
                    _ => panic!("expected Block"),
                }
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn construct_method_call_as_application() {
        // users.filter((x) => x.age >= 18i32) desugars to:
        // filter(users, (x) => x.age >= 18i32)
        let bump = Bump::new();
        let users = Expr::ident("users", sp(0, 5), &bump);
        let filter = Expr::ident("filter", sp(6, 12), &bump);

        let x = Expr::ident("x", sp(18, 19), &bump);
        let age = Expr::field_access(x, "age", sp(18, 22), &bump);
        let lit = Expr::i32(18, sp(26, 28), &bump);
        let cmp = Expr::binary(BinOp::Ge, age, lit, sp(18, 28), &bump);

        let closure_params = Vec::from_iter_in(
            [Param {
                name: "x",
                ty: None,
            }]
            .into_iter(),
            &bump,
        );
        let closure = Expr::Lambda {
            params: closure_params,
            return_type: None,
            body: cmp,
            span: sp(13, 30),
        };

        let args = Vec::from_iter_in([users, &closure].into_iter(), &bump);
        let app = Expr::app(filter, args, sp(0, 31), &bump);

        match app {
            Expr::Application { func, args, .. } => {
                assert!(matches!(func, Expr::Ident("filter", _)));
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0], Expr::Ident("users", _)));
                assert!(matches!(args[1], Expr::Lambda { .. }));
            }
            _ => panic!("expected Application"),
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
    fn construct_let_expression() {
        let bump = Bump::new();
        let value = Expr::i32(5, sp(8, 9), &bump);
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
        let rhs = bump.alloc(TypeExpr::Named("i32", sp(18, 21)));
        let decl = Decl::TypeAlias {
            name: "UserId",
            params: Vec::new_in(&bump),
            rhs,
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
        let bump = Bump::new();
        let from = bump.alloc(TypeExpr::Named("i32", sp(0, 3)));
        let to = bump.alloc(TypeExpr::Named("str", sp(7, 10)));
        let func_type = TypeExpr::Function {
            from,
            to,
            span: sp(0, 10),
        };
        match func_type {
            TypeExpr::Function { .. } => {}
            _ => panic!("expected Function type"),
        }
    }

    #[test]
    fn type_expr_generic_apply() {
        let bump = Bump::new();
        let base = bump.alloc(TypeExpr::Named("Option", sp(0, 6)));
        let arg = bump.alloc(TypeExpr::Named("i32", sp(7, 10)));
        let applied = TypeExpr::Apply {
            func: base,
            arg,
            span: sp(0, 11),
        };
        match applied {
            TypeExpr::Apply { func, arg, .. } => {
                assert!(matches!(func, TypeExpr::Named("Option", _)));
                assert!(matches!(arg, TypeExpr::Named("i32", _)));
            }
            _ => panic!("expected Apply"),
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

    #[test]
    fn pattern_literal_i32() {
        let p = Pattern::Literal(LiteralPattern::I32(42));
        match p {
            Pattern::Literal(LiteralPattern::I32(v)) => assert_eq!(v, 42),
            _ => panic!("expected I32 literal pattern"),
        }
    }

    #[test]
    fn pattern_literal_u8() {
        let p = Pattern::Literal(LiteralPattern::U8(255));
        match p {
            Pattern::Literal(LiteralPattern::U8(v)) => assert_eq!(v, 255),
            _ => panic!("expected U8 literal pattern"),
        }
    }

    #[test]
    fn is_numeric_literal_true() {
        let bump = Bump::new();
        assert!(Expr::i32(1, sp(0, 1), &bump).is_numeric_literal());
        assert!(Expr::f64(1.0, sp(0, 1), &bump).is_numeric_literal());
        assert!(Expr::u8(1, sp(0, 1), &bump).is_numeric_literal());
    }

    #[test]
    fn is_numeric_literal_false() {
        let bump = Bump::new();
        assert!(!Expr::str("hi", sp(0, 2), &bump).is_numeric_literal());
        assert!(!Expr::bool(true, sp(0, 4), &bump).is_numeric_literal());
        assert!(!Expr::ident("x", sp(0, 1), &bump).is_numeric_literal());
    }

    fn arena_alloc_pattern<'a>(bump: &'a Bump, p: Pattern<'a>) -> &'a Pattern<'a> {
        bump.alloc(p)
    }
}
