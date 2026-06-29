//! Integration tests for the pipe-lang typechecker pipeline.
//!
//! These tests verify that the lexer, AST, and typechecker work together
//! correctly. Once the parser is ready, these tests will be extended to
//! cover the full `lex -> parse -> typecheck` pipeline.

use ast::ast::{BinOp, Decl, Expr, MatchArm, Pattern};
use ast::span::Span;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use lexer::Lexer;
use std::rc::Rc;
use typechecker::{MonoType, PolyType, TypeEnv, TypeError, infer_decl, infer_expr};

/// Helper to lex source and return significant tokens (excluding whitespace/newlines/eof).
fn lex_tokens(source: &str) -> Vec<lexer::Token<'_>> {
    Lexer::new(source)
        .filter_map(|r| r.ok())
        .filter(|t| !t.kind.is_trivial() && !matches!(t.kind, lexer::TokenKind::Eof))
        .collect()
}

/// Helper to check if lexing succeeds and produces expected token count.
fn assert_lex_count(source: &str, expected: usize) {
    let tokens = lex_tokens(source);
    assert_eq!(
        tokens.len(),
        expected,
        "expected {expected} significant tokens, got {} for: {source}",
        tokens.len()
    );
}

// ---------------------------------------------------------------------------
// Lexer integration tests
// ---------------------------------------------------------------------------

#[test]
fn integration_lex_simple_binding() {
    assert_lex_count("let x = 42", 4); // let x = 42
}

#[test]
fn integration_lex_closure() {
    assert_lex_count("(x) => x + 1", 7); // ( x ) => x + 1
}

#[test]
fn integration_lex_import() {
    assert_lex_count("use stdlib::io", 4); // use stdlib :: io
}

#[test]
fn integration_lex_keywords() {
    let tokens = lex_tokens("let type if else match use true false");
    let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
    assert!(kinds.contains(&&lexer::TokenKind::Let));
    assert!(kinds.contains(&&lexer::TokenKind::Type));
    assert!(kinds.contains(&&lexer::TokenKind::If));
    assert!(kinds.contains(&&lexer::TokenKind::Else));
    assert!(kinds.contains(&&lexer::TokenKind::Match));
    assert!(kinds.contains(&&lexer::TokenKind::Use));
    assert!(kinds.contains(&&lexer::TokenKind::True));
    assert!(kinds.contains(&&lexer::TokenKind::False));
}

#[test]
fn integration_lex_numeric_types() {
    assert_lex_count("42", 1);
    assert_lex_count("42i32", 1);
    assert_lex_count("255u8", 1);
    assert_lex_count("3.14", 1);
    assert_lex_count("3.14f64", 1);
}

// ---------------------------------------------------------------------------
// AST + Typechecker integration tests
// ---------------------------------------------------------------------------

#[test]
fn integration_let_binding_typechecks() {
    let bump = Bump::new();
    let val = Expr::int("42", Span::new(8, 10), &bump);
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "x",
        ty: None,
        value: val,
        span: Span::new(0, 10),
    };
    let mut env = TypeEnv::new();
    let result = infer_decl(&mut env, &decl);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), PolyType::mono(MonoType::I32));
}

#[test]
fn integration_binary_expression_typechecks() {
    let bump = Bump::new();
    let lhs = Expr::int("1", Span::new(0, 1), &bump);
    let rhs = Expr::int("2", Span::new(4, 5), &bump);
    let expr = Expr::binary(BinOp::Add, lhs, rhs, Span::new(0, 5), &bump);
    let mut env = TypeEnv::new();
    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

#[test]
fn integration_comparison_returns_bool() {
    let bump = Bump::new();
    let lhs = Expr::int("5", Span::new(0, 1), &bump);
    let rhs = Expr::int("10", Span::new(4, 6), &bump);
    let expr = Expr::binary(BinOp::Lt, lhs, rhs, Span::new(0, 6), &bump);
    let mut env = TypeEnv::new();
    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

#[test]
fn integration_import_succeeds() {
    let bump = Bump::new();
    let decl = Decl::Use {
        id: ast::ast::NodeId(0),
        path: BumpVec::from_iter_in(["stdlib", "io"], &bump),
        span: Span::new(0, 13),
    };
    let mut env = TypeEnv::new();
    let result = infer_decl(&mut env, &decl);
    assert!(result.is_ok());
}

#[test]
fn integration_prelude_loads_all_builtins() {
    let mut env = TypeEnv::new();
    env.load_prelude();

    assert!(env.contains("id"));
    assert!(env.contains("const"));
    assert!(env.contains("flip"));
    assert!(env.contains("compose"));
    assert!(env.contains("pipe"));
    assert!(env.contains("apply"));
    assert!(env.contains("Option"));
    assert!(env.contains("Result"));
}

#[test]
fn integration_int_literal_infers_correctly() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let val = Expr::int("42", Span::new(0, 2), &bump);
    let ty = infer_expr(&mut env, val).unwrap();
    assert_eq!(ty, MonoType::I32);
}

#[test]
fn integration_float_literal_infers_correctly() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let val = Expr::float("3.14", Span::new(0, 4), &bump);
    let ty = infer_expr(&mut env, val).unwrap();
    assert_eq!(ty, MonoType::F64);
}

#[test]
fn integration_bool_literal_infers_correctly() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let val = Expr::bool(true, Span::new(0, 4), &bump);
    let ty = infer_expr(&mut env, val).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

#[test]
fn integration_string_literal_infers_correctly() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let val = Expr::str("hello", Span::new(0, 7), &bump);
    let ty = infer_expr(&mut env, val).unwrap();
    assert_eq!(ty, MonoType::Str);
}

#[test]
fn integration_bind_then_reference() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // First bind x = 42
    let val_x = Expr::int("42", Span::new(8, 10), &bump);
    let decl_x = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "x",
        ty: None,
        value: val_x,
        span: Span::new(0, 10),
    };
    infer_decl(&mut env, &decl_x).unwrap();

    // x should now be in the environment
    assert!(env.contains("x"));
}

#[test]
fn integration_unbound_variable_fails() {
    let bump = Bump::new();
    let ident = Expr::ident("undefined", Span::new(0, 9), &bump);
    let mut env = TypeEnv::new();
    let result = infer_expr(&mut env, ident);
    assert!(result.is_err());
}

#[test]
fn integration_chained_binary_ops() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // (1 + 2) + 3
    let a = Expr::int("1", Span::new(0, 1), &bump);
    let b = Expr::int("2", Span::new(4, 5), &bump);
    let inner = Expr::binary(BinOp::Add, a, b, Span::new(0, 5), &bump);
    let c = Expr::int("3", Span::new(8, 9), &bump);
    let outer = Expr::binary(BinOp::Add, inner, c, Span::new(0, 9), &bump);

    let ty = infer_expr(&mut env, outer).unwrap();
    assert_eq!(ty, MonoType::I32);
}

// ---------------------------------------------------------------------------
// Pre-existing tests from earlier (kept for regression)
// ---------------------------------------------------------------------------

#[test]
fn integration_unify_same_concrete() {
    use typechecker::{Substitution, unify};
    let mut sub = Substitution::new();
    assert!(unify(&mut sub, &MonoType::I32, &MonoType::I32).is_ok());
}

#[test]
fn integration_unify_different_fails() {
    use typechecker::{Substitution, unify};
    let mut sub = Substitution::new();
    assert!(unify(&mut sub, &MonoType::I32, &MonoType::Str).is_err());
}

// ---------------------------------------------------------------------------
// HM Inference: Lambda
// ---------------------------------------------------------------------------

/// `(x) => x` should infer as `(?a) -> ?a`  (or a fresh var)
#[test]
fn hm_lambda_identity_infers_func_type() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // body = x
    let body = Expr::ident("x", Span::new(7, 8), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(0, 8),
    });

    let ty = infer_expr(&mut env, lambda).unwrap();
    assert!(matches!(ty, MonoType::Func { .. }));
}

/// `(a, b) => a` — two params, returns first
#[test]
fn hm_lambda_two_params() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let body = Expr::ident("a", Span::new(9, 10), &bump);
    let params = BumpVec::from_iter_in(
        [
            ast::ast::Param {
                name: "a",
                ty: None,
            },
            ast::ast::Param {
                name: "b",
                ty: None,
            },
        ],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(0, 10),
    });

    let ty = infer_expr(&mut env, lambda).unwrap();
    match ty {
        MonoType::Func { params, .. } => assert_eq!(params.len(), 2),
        _ => panic!("expected Func"),
    }
}

// ---------------------------------------------------------------------------
// HM Inference: Application
// ---------------------------------------------------------------------------

/// `id(42)` — apply polymorphic id to i32, should give i32
#[test]
fn hm_app_id_i32() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let func = Expr::ident("id", Span::new(0, 2), &bump);
    let arg = Expr::int("42", Span::new(3, 5), &bump);
    let args = BumpVec::from_iter_in([arg], &bump);
    let expr = Expr::app(func, args, Span::new(0, 6), &bump);

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `id(true)` — apply id to bool, should give bool
#[test]
fn hm_app_id_bool() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let func = Expr::ident("id", Span::new(0, 2), &bump);
    let arg = Expr::bool(true, Span::new(3, 7), &bump);
    let args = BumpVec::from_iter_in([arg], &bump);
    let expr = Expr::app(func, args, Span::new(0, 8), &bump);

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

// ---------------------------------------------------------------------------
// HM Inference: Block
// ---------------------------------------------------------------------------

/// `{ let y = 1; y }` — block with let stmt, result is i32
#[test]
fn hm_block_let_stmt_then_result() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let val = Expr::int("1", Span::new(10, 11), &bump);
    let pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "y",
        Span::new(6, 7),
    ));
    let stmt = ast::ast::Stmt::Let {
        pattern: pat,
        value: val,
    };
    let stmts = BumpVec::from_iter_in([stmt], &bump);
    let result_expr = Expr::ident("y", Span::new(13, 14), &bump);
    let block = bump.alloc(Expr::Block {
        id: ast::ast::NodeId(0),
        stmts,
        result: result_expr,
        span: Span::new(0, 15),
    });

    let ty = infer_expr(&mut env, block).unwrap();
    assert_eq!(ty, MonoType::I32);
}

// ---------------------------------------------------------------------------
// HM Inference: If
// ---------------------------------------------------------------------------

/// `if true { 1 } else { 2 }` — branches must unify, result is i32
#[test]
fn hm_if_both_branches_same_type() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let cond = Expr::bool(true, Span::new(3, 7), &bump);
    let then_b = Expr::int("1", Span::new(10, 11), &bump);
    let else_b = Expr::int("2", Span::new(20, 21), &bump);
    let expr = bump.alloc(Expr::If {
        id: ast::ast::NodeId(0),
        condition: cond,
        then_branch: then_b,
        else_branch: else_b,
        span: Span::new(0, 22),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `if true { 1 } else { "x" }` — mismatched branches must fail
#[test]
fn hm_if_mismatched_branches_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let cond = Expr::bool(true, Span::new(3, 7), &bump);
    let then_b = Expr::int("1", Span::new(10, 11), &bump);
    let else_b = Expr::str("x", Span::new(20, 23), &bump);
    let expr = bump.alloc(Expr::If {
        id: ast::ast::NodeId(0),
        condition: cond,
        then_branch: then_b,
        else_branch: else_b,
        span: Span::new(0, 24),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// HM Inference: Let-polymorphism
// ---------------------------------------------------------------------------

/// `let id = (x) => x` — id should be polymorphic ∀a. a -> a
#[test]
fn hm_let_poly_id_is_polymorphic() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let body = Expr::ident("x", Span::new(14, 15), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(10, 15),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "my_id",
        ty: None,
        value: lambda,
        span: Span::new(0, 15),
    };

    let poly = infer_decl(&mut env, &decl).unwrap();
    // Must be generalized: ∀a. a -> a
    assert!(!poly.quantified.is_empty(), "id must be polymorphic");
}

/// After `let id = (x) => x`, applying id to i32 and then to bool
/// must both succeed independently (two instantiation sites).
#[test]
fn hm_let_poly_id_two_uses() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // bind id
    let body = Expr::ident("x", Span::new(14, 15), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(10, 15),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "my_id",
        ty: None,
        value: lambda,
        span: Span::new(0, 15),
    };
    infer_decl(&mut env, &decl).unwrap();

    // my_id(42) -> i32
    let f1 = Expr::ident("my_id", Span::new(0, 5), &bump);
    let a1 = Expr::int("42", Span::new(6, 8), &bump);
    let args1 = BumpVec::from_iter_in([a1], &bump);
    let app1 = Expr::app(f1, args1, Span::new(0, 9), &bump);
    let ty1 = infer_expr(&mut env, app1).unwrap();
    assert_eq!(ty1, MonoType::I32);

    // my_id(true) -> bool
    let f2 = Expr::ident("my_id", Span::new(0, 5), &bump);
    let a2 = Expr::bool(true, Span::new(6, 10), &bump);
    let args2 = BumpVec::from_iter_in([a2], &bump);
    let app2 = Expr::app(f2, args2, Span::new(0, 11), &bump);
    let ty2 = infer_expr(&mut env, app2).unwrap();
    assert_eq!(ty2, MonoType::Bool);
}

// ---------------------------------------------------------------------------
// HM Inference: Occurs check
// ---------------------------------------------------------------------------

/// Unifying ?a with Array<?a> should fail with InfiniteType.
#[test]
fn hm_occurs_check_infinite_type() {
    use typechecker::{Substitution, TypeId, unify};
    let mut sub = Substitution::new();
    sub.ensure_key(TypeId(0));
    let a = MonoType::Var(TypeId(0));
    let arr_a = MonoType::Array(std::rc::Rc::new(MonoType::Var(TypeId(0))));
    let result = unify(&mut sub, &a, &arr_a);
    assert!(
        matches!(result, Err(typechecker::TypeError::InfiniteType { .. })),
        "expected InfiniteType, got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// HM Inference: Match
// ---------------------------------------------------------------------------

/// Match on a bool with two arms, both returning i32.
#[test]
fn hm_match_bool_arms_same_type() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::bool(true, Span::new(6, 10), &bump);
    let arm_true = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Bool(true),
            Span::new(13, 17),
        )),
        body: Expr::int("1", Span::new(21, 22), &bump),
    };
    let arm_false = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Wildcard(
            ast::ast::NodeId(0),
            Span::new(25, 26),
        )),
        body: Expr::int("0", Span::new(30, 31), &bump),
    };
    let arms = BumpVec::from_iter_in([arm_true, arm_false], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 32),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with mismatched arm types must fail.
#[test]
fn hm_match_mismatched_arms_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::bool(true, Span::new(6, 10), &bump);
    let arm_true = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Bool(true),
            Span::new(13, 17),
        )),
        body: Expr::int("1", Span::new(21, 22), &bump),
    };
    let arm_false = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Wildcard(
            ast::ast::NodeId(0),
            Span::new(25, 26),
        )),
        body: Expr::str("no", Span::new(30, 34), &bump),
    };
    let arms = BumpVec::from_iter_in([arm_true, arm_false], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 35),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// HM Inference: Array
// ---------------------------------------------------------------------------

/// `[1, 2, 3]` — homogeneous array infers as Array<i32>
#[test]
fn hm_array_homogeneous() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let elems = BumpVec::from_iter_in(
        [
            Expr::int("1", Span::new(1, 2), &bump),
            Expr::int("2", Span::new(4, 5), &bump),
            Expr::int("3", Span::new(7, 8), &bump),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems,
        span: Span::new(0, 9),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Array(std::rc::Rc::new(MonoType::I32)));
}

/// `[1, "x"]` — mixed-type array must fail
#[test]
fn hm_array_mixed_types_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let elems = BumpVec::from_iter_in(
        [
            Expr::int("1", Span::new(1, 2), &bump),
            Expr::str("x", Span::new(4, 7), &bump),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems,
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// HM Inference: Tuple
// ---------------------------------------------------------------------------

/// `(1, true)` — tuple infers element types independently
#[test]
fn hm_tuple_infers_element_types() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let elems = BumpVec::from_iter_in(
        [
            Expr::int("1", Span::new(1, 2), &bump),
            Expr::bool(true, Span::new(4, 8), &bump),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Tuple {
        id: ast::ast::NodeId(0),
        elems,
        span: Span::new(0, 9),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    // A Tuple is represented as a Tag named "Tuple" or a Record; check it's a Tag
    match ty {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Tuple");
            assert_eq!(payload.len(), 2);
            assert_eq!(payload[0], MonoType::I32);
            assert_eq!(payload[1], MonoType::Bool);
        }
        _ => panic!("expected Tag(Tuple, ...), got {ty:?}"),
    }
}

// ---------------------------------------------------------------------------
// HM Inference: Record
// ---------------------------------------------------------------------------

/// `{ name: "Alice", age: 30 }` — record infers field types
#[test]
fn hm_record_infers_field_types() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let fields = BumpVec::from_iter_in(
        [
            ast::ast::RecordField {
                name: "name",
                value: Expr::str("Alice", Span::new(9, 16), &bump),
            },
            ast::ast::RecordField {
                name: "age",
                value: Expr::int("30", Span::new(23, 25), &bump),
            },
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Record {
        id: ast::ast::NodeId(0),
        fields,
        span: Span::new(0, 26),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Record(fields) => {
            assert_eq!(fields.get("name").unwrap(), &MonoType::Str);
            assert_eq!(fields.get("age").unwrap(), &MonoType::I32);
        }
        _ => panic!("expected Record, got {ty:?}"),
    }
}

// ---------------------------------------------------------------------------
// HM Inference: FieldAccess
// ---------------------------------------------------------------------------

/// `rec.name` on `{ name: "Alice" }` — should infer Str
#[test]
fn hm_field_access_infers_field_type() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // build the record { name: "Alice" }
    let fields = BumpVec::from_iter_in(
        [ast::ast::RecordField {
            name: "name",
            value: Expr::str("Alice", Span::new(9, 16), &bump),
        }],
        &bump,
    );
    let rec_expr = Expr::record(fields, Span::new(0, 17), &bump);

    // .name
    let access = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: rec_expr,
        field: "name",
        span: Span::new(0, 22),
    });

    let ty = infer_expr(&mut env, access).unwrap();
    assert_eq!(ty, MonoType::Str);
}

// ---------------------------------------------------------------------------
// HM Inference: Unary
// ---------------------------------------------------------------------------

/// `-42` — neg of i32 is i32
#[test]
fn hm_unary_neg_i32() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let operand = Expr::int("42", Span::new(1, 3), &bump);
    let expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Neg,
        operand,
        span: Span::new(0, 3),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `!true` — not of bool is bool
#[test]
fn hm_unary_not_bool() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let operand = Expr::bool(true, Span::new(1, 5), &bump);
    let expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Not,
        operand,
        span: Span::new(0, 5),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

// ---------------------------------------------------------------------------
// HM Inference: Type annotation checking
// ---------------------------------------------------------------------------

/// `let x: i32 = 42` — annotation matches inferred type, OK
#[test]
fn hm_annotation_matches_inferred() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ann = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(7, 10)));
    let val = Expr::int("42", Span::new(13, 15), &bump);
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "x",
        ty: Some(ann),
        value: val,
        span: Span::new(0, 15),
    };

    let result = infer_decl(&mut env, &decl);
    assert!(result.is_ok());
}

/// `let x: str = 42` — annotation conflicts with inferred type, error
#[test]
fn hm_annotation_conflicts_with_inferred() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ann = bump.alloc(ast::ast::TypeExpr::Named("str", Span::new(7, 10)));
    let val = Expr::int("42", Span::new(13, 15), &bump);
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "x",
        ty: Some(ann),
        value: val,
        span: Span::new(0, 15),
    };

    let result = infer_decl(&mut env, &decl);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Currying & Higher-Order Functions
// ===========================================================================

/// Curried add: `(a:i32) => (b:i32) => a + b` — infers `(i32) -> (i32) -> i32`
#[test]
fn hm_curried_add() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ty_i32_ann = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(0, 3)));

    // inner lambda: (b:i32) => a + b
    let b_param = Expr::ident("b", Span::new(16, 17), &bump);
    let a_ref = Expr::ident("a", Span::new(20, 21), &bump);
    let add_expr = Expr::binary(BinOp::Add, a_ref, b_param, Span::new(20, 23), &bump);
    let inner_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "b",
            ty: Some(ty_i32_ann),
        }],
        &bump,
    );
    let inner_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: inner_params,
        return_type: None,
        body: add_expr,
        span: Span::new(10, 23),
    });

    // outer lambda: (a:i32) => (b:i32) => a + b
    let ty_i32_ann2 = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(0, 3)));
    let outer_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "a",
            ty: Some(ty_i32_ann2),
        }],
        &bump,
    );
    let outer_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: outer_params,
        return_type: None,
        body: inner_lambda,
        span: Span::new(0, 23),
    });

    let ty = infer_expr(&mut env, outer_lambda).unwrap();
    match ty {
        MonoType::Func { params, ret } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0], MonoType::I32);
            match ret.as_ref() {
                MonoType::Func {
                    params: inner_params,
                    ret: inner_ret,
                } => {
                    assert_eq!(inner_params.len(), 1);
                    assert_eq!(inner_params[0], MonoType::I32);
                    assert_eq!(inner_ret.as_ref(), &MonoType::I32);
                }
                _ => panic!("expected nested Func"),
            }
        }
        _ => panic!("expected Func"),
    }
}

/// Applying a curried function: `add(1)(2)` should infer as i32
#[test]
fn hm_curried_application() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // Build add: (a) => (b) => a + b
    let b_param = Expr::ident("b", Span::new(12, 13), &bump);
    let a_ref = Expr::ident("a", Span::new(16, 17), &bump);
    let add_expr = Expr::binary(BinOp::Add, a_ref, b_param, Span::new(16, 19), &bump);
    let inner_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "b",
            ty: None,
        }],
        &bump,
    );
    let inner_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: inner_params,
        return_type: None,
        body: add_expr,
        span: Span::new(10, 19),
    });
    let outer_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "a",
            ty: None,
        }],
        &bump,
    );
    let outer_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: outer_params,
        return_type: None,
        body: inner_lambda,
        span: Span::new(0, 19),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "add",
        ty: None,
        value: outer_lambda,
        span: Span::new(0, 19),
    };
    infer_decl(&mut env, &decl).unwrap();

    // add(1)(2)
    let one = Expr::int("1", Span::new(23, 24), &bump);
    let add_ref = Expr::ident("add", Span::new(20, 23), &bump);
    let app1 = Expr::app(
        add_ref,
        BumpVec::from_iter_in([one], &bump),
        Span::new(20, 25),
        &bump,
    );
    let two = Expr::int("2", Span::new(26, 27), &bump);
    let app2 = Expr::app(
        app1,
        BumpVec::from_iter_in([two], &bump),
        Span::new(20, 28),
        &bump,
    );

    let ty = infer_expr(&mut env, app2).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Passing a function as argument: `apply(f, x) => f(x)`
#[test]
fn hm_higher_order_apply() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // apply = (f, x) => f(x)
    let f_ref = Expr::ident("f", Span::new(12, 13), &bump);
    let x_ref = Expr::ident("x", Span::new(14, 15), &bump);
    let app_expr = Expr::app(
        f_ref,
        BumpVec::from_iter_in([x_ref], &bump),
        Span::new(12, 16),
        &bump,
    );
    let params = BumpVec::from_iter_in(
        [
            ast::ast::Param {
                name: "f",
                ty: None,
            },
            ast::ast::Param {
                name: "x",
                ty: None,
            },
        ],
        &bump,
    );
    let apply_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body: app_expr,
        span: Span::new(0, 16),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "apply",
        ty: None,
        value: apply_lambda,
        span: Span::new(0, 16),
    };
    infer_decl(&mut env, &decl).unwrap();

    // apply(id, 42) — id is from prelude, polymorphic
    env.load_prelude();
    let apply_ref = Expr::ident("apply", Span::new(0, 5), &bump);
    let id_ref = Expr::ident("id", Span::new(6, 8), &bump);
    let num = Expr::int("42", Span::new(9, 11), &bump);
    let expr = Expr::app(
        apply_ref,
        BumpVec::from_iter_in([id_ref, num], &bump),
        Span::new(0, 12),
        &bump,
    );

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// map as higher-order: `(xs, f) => [f(xs[0])]` — inferred type
#[test]
fn hm_higher_order_map_single() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // map = (xs, f) => f(xs[0])
    let xs_ref = Expr::ident("xs", Span::new(12, 14), &bump);
    let idx_zero = Expr::int("0", Span::new(15, 16), &bump);
    let index_expr = bump.alloc(Expr::Index {
        id: ast::ast::NodeId(0),
        array: xs_ref,
        index: idx_zero,
        span: Span::new(12, 17),
    });
    let index_ref: &Expr = index_expr;
    let f_ref = Expr::ident("f", Span::new(19, 20), &bump);
    let body = Expr::app(
        f_ref,
        BumpVec::from_iter_in([index_ref], &bump),
        Span::new(19, 21),
        &bump,
    );
    let params = BumpVec::from_iter_in(
        [
            ast::ast::Param {
                name: "xs",
                ty: None,
            },
            ast::ast::Param {
                name: "f",
                ty: None,
            },
        ],
        &bump,
    );
    let map_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(0, 21),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "map_single",
        ty: None,
        value: map_lambda,
        span: Span::new(0, 21),
    };

    let poly = infer_decl(&mut env, &decl).unwrap();
    // Should be polymorphic
    assert!(
        !poly.quantified.is_empty(),
        "map_single should be polymorphic"
    );
}

// ===========================================================================
// HM Edge Case Tests — Let-Polymorphism (Advanced)
// ===========================================================================

/// `let f = id; f(42)` — f gets the same polymorphic type as id
#[test]
fn hm_let_poly_alias_polymorphic() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // f = id
    let id_ref = Expr::ident("id", Span::new(8, 10), &bump);
    let decl_f = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: None,
        value: id_ref,
        span: Span::new(0, 10),
    };
    infer_decl(&mut env, &decl_f).unwrap();

    // f(42)
    let f_ref = Expr::ident("f", Span::new(0, 1), &bump);
    let num = Expr::int("42", Span::new(2, 4), &bump);
    let app = Expr::app(
        f_ref,
        BumpVec::from_iter_in([num], &bump),
        Span::new(0, 5),
        &bump,
    );
    let ty1 = infer_expr(&mut env, app).unwrap();
    assert_eq!(ty1, MonoType::I32);

    // f(true) — same binding, different instantiation
    let f_ref2 = Expr::ident("f", Span::new(0, 1), &bump);
    let bool_val = Expr::bool(true, Span::new(2, 6), &bump);
    let app2 = Expr::app(
        f_ref2,
        BumpVec::from_iter_in([bool_val], &bump),
        Span::new(0, 7),
        &bump,
    );
    let ty2 = infer_expr(&mut env, app2).unwrap();
    assert_eq!(ty2, MonoType::Bool);
}

/// `let f = (x) => x; f(1); f("hello")` — both uses of f succeed
#[test]
fn hm_let_poly_identity_multiple_instantiations() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // f = (x) => x
    let body = Expr::ident("x", Span::new(11, 12), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(7, 12),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: None,
        value: lambda,
        span: Span::new(0, 12),
    };
    infer_decl(&mut env, &decl).unwrap();

    // f(42)
    let f1 = Expr::ident("f", Span::new(0, 1), &bump);
    let n1 = Expr::int("42", Span::new(2, 4), &bump);
    let ty1 = infer_expr(
        &mut env,
        Expr::app(
            f1,
            BumpVec::from_iter_in([n1], &bump),
            Span::new(0, 5),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty1, MonoType::I32);

    // f("hello")
    let f2 = Expr::ident("f", Span::new(0, 1), &bump);
    let s = Expr::str("hello", Span::new(2, 9), &bump);
    let ty2 = infer_expr(
        &mut env,
        Expr::app(
            f2,
            BumpVec::from_iter_in([s], &bump),
            Span::new(0, 10),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty2, MonoType::Str);
}

/// Two different polymorphic bindings don't interfere
#[test]
fn hm_let_poly_independent_bindings() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // id = (x) => x
    let id_body = Expr::ident("x", Span::new(12, 13), &bump);
    let id_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let id_lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: id_params,
        return_type: None,
        body: id_body,
        span: Span::new(8, 13),
    });
    let id_decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "id",
        ty: None,
        value: id_lambda,
        span: Span::new(0, 13),
    };
    infer_decl(&mut env, &id_decl).unwrap();

    // const_fn = (a) => (b) => a
    let const_b = Expr::ident("a", Span::new(26, 27), &bump);
    let inner_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "b",
            ty: None,
        }],
        &bump,
    );
    let inner = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: inner_params,
        return_type: None,
        body: const_b,
        span: Span::new(21, 27),
    });
    let outer_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "a",
            ty: None,
        }],
        &bump,
    );
    let outer = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: outer_params,
        return_type: None,
        body: inner,
        span: Span::new(17, 27),
    });
    let const_decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "const_fn",
        ty: None,
        value: outer,
        span: Span::new(0, 27),
    };
    infer_decl(&mut env, &const_decl).unwrap();

    // id(42) -> i32
    let ty1 = infer_expr(
        &mut env,
        Expr::app(
            Expr::ident("id", Span::new(0, 2), &bump),
            BumpVec::from_iter_in([Expr::int("42", Span::new(3, 5), &bump)], &bump),
            Span::new(0, 6),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty1, MonoType::I32);

    // const_fn(1)(true) -> i32
    let inner_app = Expr::app(
        Expr::ident("const_fn", Span::new(0, 8), &bump),
        BumpVec::from_iter_in([Expr::int("1", Span::new(9, 10), &bump)], &bump),
        Span::new(0, 11),
        &bump,
    );
    let ty2 = infer_expr(
        &mut env,
        Expr::app(
            inner_app,
            BumpVec::from_iter_in([Expr::bool(true, Span::new(12, 16), &bump)], &bump),
            Span::new(0, 17),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty2, MonoType::I32);
}

// ===========================================================================
// HM Edge Case Tests — Nested & Complex Expressions
// ===========================================================================

/// Nested if: `if true { if false { 1 } else { 2 } } else { 3 }`
#[test]
fn hm_nested_if() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let inner_then = Expr::int("1", Span::new(22, 23), &bump);
    let inner_else = Expr::int("2", Span::new(32, 33), &bump);
    let inner_if = bump.alloc(Expr::If {
        id: ast::ast::NodeId(0),
        condition: Expr::bool(false, Span::new(12, 17), &bump),
        then_branch: inner_then,
        else_branch: inner_else,
        span: Span::new(9, 34),
    });
    let outer_else = Expr::int("3", Span::new(45, 46), &bump);
    let outer_if = bump.alloc(Expr::If {
        id: ast::ast::NodeId(0),
        condition: Expr::bool(true, Span::new(3, 7), &bump),
        then_branch: inner_if,
        else_branch: outer_else,
        span: Span::new(0, 47),
    });

    let ty = infer_expr(&mut env, outer_if).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Nested block: `{ let x = { let y = 1; y }; x }`
#[test]
fn hm_nested_block() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // inner block: { let y = 1; y }
    let inner_val = Expr::int("1", Span::new(16, 17), &bump);
    let inner_pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "y",
        Span::new(12, 13),
    ));
    let inner_stmt = ast::ast::Stmt::Let {
        pattern: inner_pat,
        value: inner_val,
    };
    let inner_result = Expr::ident("y", Span::new(19, 20), &bump);
    let inner_block = bump.alloc(Expr::Block {
        id: ast::ast::NodeId(0),
        stmts: BumpVec::from_iter_in([inner_stmt], &bump),
        result: inner_result,
        span: Span::new(6, 21),
    });

    // outer block: { let x = inner; x }
    let outer_pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "x",
        Span::new(3, 4),
    ));
    let outer_stmt = ast::ast::Stmt::Let {
        pattern: outer_pat,
        value: inner_block,
    };
    let outer_result = Expr::ident("x", Span::new(24, 25), &bump);
    let outer_block = bump.alloc(Expr::Block {
        id: ast::ast::NodeId(0),
        stmts: BumpVec::from_iter_in([outer_stmt], &bump),
        result: outer_result,
        span: Span::new(0, 26),
    });

    let ty = infer_expr(&mut env, outer_block).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Nested record field access: `{ a: { b: 42 } }.a.b`
#[test]
fn hm_nested_record_field_access() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // { b: 42 }
    let inner_fields = BumpVec::from_iter_in(
        [ast::ast::RecordField {
            name: "b",
            value: Expr::int("42", Span::new(9, 11), &bump),
        }],
        &bump,
    );
    let inner_rec = Expr::record(inner_fields, Span::new(4, 12), &bump);

    // { a: inner }
    let outer_fields = BumpVec::from_iter_in(
        [ast::ast::RecordField {
            name: "a",
            value: inner_rec,
        }],
        &bump,
    );
    let outer_rec = bump.alloc(Expr::Record {
        id: ast::ast::NodeId(0),
        fields: outer_fields,
        span: Span::new(0, 16),
    });

    // outer_rec.a
    let access_a = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: outer_rec,
        field: "a",
        span: Span::new(0, 19),
    });

    // outer_rec.a.b
    let access_b = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: access_a,
        field: "b",
        span: Span::new(0, 22),
    });

    let ty = infer_expr(&mut env, access_b).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Deeply nested application: `((f) => f)(id)(42)`
#[test]
fn hm_deeply_nested_application() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // (f) => f
    let body = Expr::ident("f", Span::new(6, 7), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "f",
            ty: None,
        }],
        &bump,
    );
    let identity_fn = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(0, 7),
    });

    // (f) => f applied to nothing — wait, this is (f) => f, then apply to id, then apply to 42
    // ((f) => f)(id) — first application
    let id_ref = Expr::ident("id", Span::new(10, 12), &bump);
    let app1 = Expr::app(
        identity_fn,
        BumpVec::from_iter_in([id_ref], &bump),
        Span::new(0, 13),
        &bump,
    );

    // ((f) => f)(id)(42) — second application
    let num = Expr::int("42", Span::new(14, 16), &bump);
    let app2 = Expr::app(
        app1,
        BumpVec::from_iter_in([num], &bump),
        Span::new(0, 17),
        &bump,
    );

    let ty = infer_expr(&mut env, app2).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Block with multiple let statements and a complex result
#[test]
fn hm_block_multiple_lets() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // { let a = 1; let b = 2; let c = 3; a + b + c }
    let a_val = Expr::int("1", Span::new(10, 11), &bump);
    let a_pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "a",
        Span::new(6, 7),
    ));
    let stmt_a = ast::ast::Stmt::Let {
        pattern: a_pat,
        value: a_val,
    };

    let b_val = Expr::int("2", Span::new(21, 22), &bump);
    let b_pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "b",
        Span::new(17, 18),
    ));
    let stmt_b = ast::ast::Stmt::Let {
        pattern: b_pat,
        value: b_val,
    };

    let c_val = Expr::int("3", Span::new(32, 33), &bump);
    let c_pat = bump.alloc(ast::ast::Pattern::Binding(
        ast::ast::NodeId(0),
        "c",
        Span::new(28, 29),
    ));
    let stmt_c = ast::ast::Stmt::Let {
        pattern: c_pat,
        value: c_val,
    };

    let a_ref = Expr::ident("a", Span::new(35, 36), &bump);
    let b_ref = Expr::ident("b", Span::new(39, 40), &bump);
    let sum1 = Expr::binary(BinOp::Add, a_ref, b_ref, Span::new(35, 40), &bump);
    let c_ref = Expr::ident("c", Span::new(43, 44), &bump);
    let sum2 = Expr::binary(BinOp::Add, sum1, c_ref, Span::new(35, 44), &bump);

    let block = bump.alloc(Expr::Block {
        id: ast::ast::NodeId(0),
        stmts: BumpVec::from_iter_in([stmt_a, stmt_b, stmt_c], &bump),
        result: sum2,
        span: Span::new(0, 45),
    });

    let ty = infer_expr(&mut env, block).unwrap();
    assert_eq!(ty, MonoType::I32);
}

// ===========================================================================
// HM Edge Case Tests — Template Strings
// ===========================================================================

/// Template with interpolation: `` `count: ${42}` ``
#[test]
fn hm_template_with_interpolation() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let parts = BumpVec::from_iter_in(
        [
            ast::ast::TemplatePart::Str("count: "),
            ast::ast::TemplatePart::Expr(Expr::int("42", Span::new(12, 14), &bump)),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Template {
        id: ast::ast::NodeId(0),
        parts,
        span: Span::new(0, 15),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

/// Template with multiple interpolations
#[test]
fn hm_template_multi_interpolation() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let parts = BumpVec::from_iter_in(
        [
            ast::ast::TemplatePart::Str(""),
            ast::ast::TemplatePart::Expr(Expr::int("1", Span::new(5, 6), &bump)),
            ast::ast::TemplatePart::Str("+"),
            ast::ast::TemplatePart::Expr(Expr::int("2", Span::new(8, 9), &bump)),
            ast::ast::TemplatePart::Str("="),
            ast::ast::TemplatePart::Expr(Expr::int("3", Span::new(11, 12), &bump)),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Template {
        id: ast::ast::NodeId(0),
        parts,
        span: Span::new(0, 13),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

/// Template with bool interpolation is fine (template always returns str)
#[test]
fn hm_template_bool_interpolation() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let parts = BumpVec::from_iter_in(
        [
            ast::ast::TemplatePart::Str("flag: "),
            ast::ast::TemplatePart::Expr(Expr::bool(true, Span::new(10, 14), &bump)),
        ],
        &bump,
    );
    let expr = bump.alloc(Expr::Template {
        id: ast::ast::NodeId(0),
        parts,
        span: Span::new(0, 15),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

// ===========================================================================
// HM Edge Case Tests — Index Expressions
// ===========================================================================

/// `[1, 2, 3][0]` — index into i32 array, returns i32
#[test]
fn hm_index_i32_array() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let arr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::int("1", Span::new(1, 2), &bump),
                Expr::int("2", Span::new(4, 5), &bump),
                Expr::int("3", Span::new(7, 8), &bump),
            ],
            &bump,
        ),
        span: Span::new(0, 9),
    });
    let idx = Expr::int("0", Span::new(10, 11), &bump);
    let expr = bump.alloc(Expr::Index {
        id: ast::ast::NodeId(0),
        array: arr,
        index: idx,
        span: Span::new(0, 12),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `["a", "b"][0]` — index into str array, returns str
#[test]
fn hm_index_str_array() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let arr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::str("a", Span::new(1, 4), &bump),
                Expr::str("b", Span::new(6, 9), &bump),
            ],
            &bump,
        ),
        span: Span::new(0, 10),
    });
    let idx = Expr::int("0", Span::new(11, 12), &bump);
    let expr = bump.alloc(Expr::Index {
        id: ast::ast::NodeId(0),
        array: arr,
        index: idx,
        span: Span::new(0, 13),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

// ===========================================================================
// HM Edge Case Tests — Match with Patterns
// ===========================================================================

/// Match with binding pattern: `match 1 { x => x + 1 }`
#[test]
fn hm_match_binding_pattern() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::int("1", Span::new(6, 7), &bump);
    let x_ref = Expr::ident("x", Span::new(12, 13), &bump);
    let one = Expr::int("1", Span::new(16, 17), &bump);
    let body = Expr::binary(BinOp::Add, x_ref, one, Span::new(12, 17), &bump);
    let arm = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Binding(
            ast::ast::NodeId(0),
            "x",
            Span::new(10, 11),
        )),
        body,
    };
    let arms = BumpVec::from_iter_in([arm], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 18),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with tuple pattern: `match (1, true) { (a, b) => a }`
#[test]
fn hm_match_tuple_pattern() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = bump.alloc(Expr::Tuple {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::int("1", Span::new(7, 8), &bump),
                Expr::bool(true, Span::new(10, 14), &bump),
            ],
            &bump,
        ),
        span: Span::new(6, 15),
    });

    let a_ref = Expr::ident("a", Span::new(22, 23), &bump);
    let pat_a = ast::ast::Pattern::Binding(ast::ast::NodeId(0), "a", Span::new(19, 20));
    let pat_b = ast::ast::Pattern::Wildcard(ast::ast::NodeId(0), Span::new(22, 23));
    let tuple_pat = bump.alloc(ast::ast::Pattern::Tuple {
        id: ast::ast::NodeId(0),
        patterns: BumpVec::from_iter_in([pat_a, pat_b], &bump),
        span: Span::new(18, 24),
    });

    let arm = ast::ast::MatchArm {
        pattern: tuple_pat,
        body: a_ref,
    };
    let arms = BumpVec::from_iter_in([arm], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 25),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with record pattern: `match { x: 1, y: 2 } { { x, y } => x + y }`
#[test]
fn hm_match_record_pattern() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = bump.alloc(Expr::Record {
        id: ast::ast::NodeId(0),
        fields: BumpVec::from_iter_in(
            [
                ast::ast::RecordField {
                    name: "x",
                    value: Expr::int("1", Span::new(9, 10), &bump),
                },
                ast::ast::RecordField {
                    name: "y",
                    value: Expr::int("2", Span::new(16, 17), &bump),
                },
            ],
            &bump,
        ),
        span: Span::new(6, 18),
    });

    let x_ref = Expr::ident("x", Span::new(25, 26), &bump);
    let y_ref = Expr::ident("y", Span::new(29, 30), &bump);
    let body = Expr::binary(BinOp::Add, x_ref, y_ref, Span::new(25, 30), &bump);

    let x_field = ast::ast::RecordPatternField {
        name: "x",
        pattern: None,
    };
    let y_field = ast::ast::RecordPatternField {
        name: "y",
        pattern: None,
    };
    let rec_pat = bump.alloc(ast::ast::Pattern::Record {
        id: ast::ast::NodeId(0),
        fields: BumpVec::from_iter_in([x_field, y_field], &bump),
        span: Span::new(20, 31),
    });

    let arm = ast::ast::MatchArm {
        pattern: rec_pat,
        body,
    };
    let arms = BumpVec::from_iter_in([arm], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 32),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with binding pattern on a complex expression: `match (1 + 2) { x => x }`
#[test]
fn hm_match_constructor_pattern() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // subject: 1 + 2
    let one = Expr::int("1", Span::new(6, 7), &bump);
    let two = Expr::int("2", Span::new(10, 11), &bump);
    let subject = bump.alloc(Expr::binary(BinOp::Add, one, two, Span::new(6, 11), &bump));

    // x => x
    let x_ref = Expr::ident("x", Span::new(16, 17), &bump);
    let arm = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Binding(
            ast::ast::NodeId(0),
            "x",
            Span::new(14, 15),
        )),
        body: x_ref,
    };
    let arms = BumpVec::from_iter_in([arm], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 18),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with wildcard default: `match true { true => 1, _ => 0 }`
#[test]
fn hm_match_wildcard_default() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::bool(true, Span::new(6, 10), &bump);
    let arm1 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Bool(true),
            Span::new(13, 17),
        )),
        body: Expr::int("1", Span::new(21, 22), &bump),
    };
    let arm2 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Wildcard(
            ast::ast::NodeId(0),
            Span::new(25, 26),
        )),
        body: Expr::int("0", Span::new(30, 31), &bump),
    };
    let arms = BumpVec::from_iter_in([arm1, arm2], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 32),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Match with three arms: `match 1 { 0 => "a", 1 => "b", _ => "c" }`
#[test]
fn hm_match_three_arms() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::int("1", Span::new(6, 7), &bump);
    let arm1 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Int("0"),
            Span::new(10, 11),
        )),
        body: Expr::str("a", Span::new(15, 18), &bump),
    };
    let arm2 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Int("1"),
            Span::new(21, 22),
        )),
        body: Expr::str("b", Span::new(26, 29), &bump),
    };
    let arm3 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Wildcard(
            ast::ast::NodeId(0),
            Span::new(32, 33),
        )),
        body: Expr::str("c", Span::new(37, 40), &bump),
    };
    let arms = BumpVec::from_iter_in([arm1, arm2, arm3], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 41),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

/// Match arms with mismatched types must fail
#[test]
fn hm_match_three_arms_mismatch() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::int("1", Span::new(6, 7), &bump);
    let arm1 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Int("0"),
            Span::new(10, 11),
        )),
        body: Expr::int("0", Span::new(15, 16), &bump),
    };
    let arm2 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Literal(
            ast::ast::NodeId(0),
            ast::ast::LiteralPattern::Int("1"),
            Span::new(19, 20),
        )),
        body: Expr::str("one", Span::new(24, 29), &bump),
    };
    let arm3 = ast::ast::MatchArm {
        pattern: bump.alloc(ast::ast::Pattern::Wildcard(
            ast::ast::NodeId(0),
            Span::new(32, 33),
        )),
        body: Expr::int("2", Span::new(37, 38), &bump),
    };
    let arms = BumpVec::from_iter_in([arm1, arm2, arm3], &bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 39),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Complex Binary Expressions
// ===========================================================================

/// Mixed arithmetic: `1 + 2 * 3` — nested binary, result is i32
#[test]
fn hm_mixed_binary_ops() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let two = Expr::int("2", Span::new(4, 5), &bump);
    let three = Expr::int("3", Span::new(8, 9), &bump);
    let mul = Expr::binary(BinOp::Mul, two, three, Span::new(4, 9), &bump);
    let one = Expr::int("1", Span::new(0, 1), &bump);
    let add = Expr::binary(BinOp::Add, one, mul, Span::new(0, 9), &bump);

    let ty = infer_expr(&mut env, add).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// Float arithmetic: `3.14 * 2.0`
#[test]
fn hm_float_binary_ops() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let lhs = Expr::float("3.14", Span::new(0, 4), &bump);
    let rhs = Expr::float("2.0", Span::new(7, 10), &bump);
    let expr = Expr::binary(BinOp::Mul, lhs, rhs, Span::new(0, 10), &bump);

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::F64);
}

/// Chain of comparisons: `1 < 2 && 3 > 0`
#[test]
fn hm_chain_comparisons() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let a = Expr::int("1", Span::new(0, 1), &bump);
    let b = Expr::int("2", Span::new(4, 5), &bump);
    let lt = Expr::binary(BinOp::Lt, a, b, Span::new(0, 5), &bump);

    let c = Expr::int("3", Span::new(9, 10), &bump);
    let d = Expr::int("0", Span::new(13, 14), &bump);
    let gt = Expr::binary(BinOp::Gt, c, d, Span::new(9, 14), &bump);

    let expr = Expr::binary(BinOp::And, lt, gt, Span::new(0, 14), &bump);

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

/// Nested logical: `!true || false && true`
#[test]
fn hm_nested_logical() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let not_expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Not,
        operand: Expr::bool(true, Span::new(1, 5), &bump),
        span: Span::new(0, 5),
    });
    let false_val = Expr::bool(false, Span::new(9, 14), &bump);
    let true_val = Expr::bool(true, Span::new(18, 22), &bump);
    let and_expr = Expr::binary(BinOp::And, false_val, true_val, Span::new(9, 22), &bump);
    let or_expr = Expr::binary(BinOp::Or, not_expr, and_expr, Span::new(0, 22), &bump);

    let ty = infer_expr(&mut env, or_expr).unwrap();
    assert_eq!(ty, MonoType::Bool);
}

// ===========================================================================
// HM Edge Case Tests — Negation Edge Cases
// ===========================================================================

/// `-3.14` — neg of f64 is f64
#[test]
fn hm_unary_neg_f64() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let operand = Expr::float("3.14", Span::new(1, 5), &bump);
    let expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Neg,
        operand,
        span: Span::new(0, 5),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    assert_eq!(ty, MonoType::F64);
}

/// `!"hello"` — not on non-bool must fail
#[test]
fn hm_unary_not_on_string_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let operand = Expr::str("hello", Span::new(1, 8), &bump);
    let expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Not,
        operand,
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

/// `-"hello"` — neg on non-numeric must fail
#[test]
fn hm_unary_neg_on_string_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let operand = Expr::str("hello", Span::new(1, 8), &bump);
    let expr = bump.alloc(Expr::Unary {
        id: ast::ast::NodeId(0),
        op: ast::ast::UnaryOp::Neg,
        operand,
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Boolean Operations Error Cases
// ===========================================================================

/// `1 && true` — left operand of && must be bool
#[test]
fn hm_and_with_non_bool_left_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let lhs = Expr::int("1", Span::new(0, 1), &bump);
    let rhs = Expr::bool(true, Span::new(5, 9), &bump);
    let expr = Expr::binary(BinOp::And, lhs, rhs, Span::new(0, 9), &bump);

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

/// `true || 2` — right operand of || must be bool
#[test]
fn hm_or_with_non_bool_right_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let lhs = Expr::bool(true, Span::new(0, 4), &bump);
    let rhs = Expr::int("2", Span::new(8, 9), &bump);
    let expr = Expr::binary(BinOp::Or, lhs, rhs, Span::new(0, 9), &bump);

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

/// `"a" + 1` — string + int must fail (no implicit coercion)
#[test]
fn hm_string_plus_int_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let lhs = Expr::str("a", Span::new(0, 3), &bump);
    let rhs = Expr::int("1", Span::new(6, 7), &bump);
    let expr = Expr::binary(BinOp::Add, lhs, rhs, Span::new(0, 7), &bump);

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Application Errors
// ===========================================================================

/// `42(1)` — applying a non-function must fail
#[test]
fn hm_apply_non_function_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let num = Expr::int("42", Span::new(0, 2), &bump);
    let arg = Expr::int("1", Span::new(3, 4), &bump);
    let expr = Expr::app(
        num,
        BumpVec::from_iter_in([arg], &bump),
        Span::new(0, 5),
        &bump,
    );

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

/// `id(1, 2)` — arity mismatch (id takes 1 arg)
#[test]
fn hm_arity_mismatch_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let id_ref = Expr::ident("id", Span::new(0, 2), &bump);
    let a1 = Expr::int("1", Span::new(3, 4), &bump);
    let a2 = Expr::int("2", Span::new(6, 7), &bump);
    let expr = Expr::app(
        id_ref,
        BumpVec::from_iter_in([a1, a2], &bump),
        Span::new(0, 8),
        &bump,
    );

    let result = infer_expr(&mut env, expr);
    assert!(matches!(result, Err(TypeError::ArityMismatch { .. })));
}

/// `id()` — zero args to id (needs 1)
#[test]
fn hm_zero_args_to_function_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let id_ref = Expr::ident("id", Span::new(0, 2), &bump);
    let expr = Expr::app(id_ref, BumpVec::new_in(&bump), Span::new(0, 4), &bump);

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Field Access Errors
// ===========================================================================

/// `(1, 2).x` — field access on tuple (Tag) must fail
#[test]
fn hm_field_access_on_tuple_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let tup = bump.alloc(Expr::Tuple {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::int("1", Span::new(1, 2), &bump),
                Expr::int("2", Span::new(4, 5), &bump),
            ],
            &bump,
        ),
        span: Span::new(0, 6),
    });
    let access = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: tup,
        field: "x",
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, access);
    assert!(result.is_err());
}

/// `[1, 2].x` — field access on array must fail
#[test]
fn hm_field_access_on_array_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let arr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::int("1", Span::new(1, 2), &bump),
                Expr::int("2", Span::new(4, 5), &bump),
            ],
            &bump,
        ),
        span: Span::new(0, 6),
    });
    let access = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: arr,
        field: "x",
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, access);
    assert!(result.is_err());
}

/// `{ x: 1 }.y` — missing field must fail
#[test]
fn hm_field_access_missing_field_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let rec = bump.alloc(Expr::Record {
        id: ast::ast::NodeId(0),
        fields: BumpVec::from_iter_in(
            [ast::ast::RecordField {
                name: "x",
                value: Expr::int("1", Span::new(5, 6), &bump),
            }],
            &bump,
        ),
        span: Span::new(0, 7),
    });
    let access = bump.alloc(Expr::FieldAccess {
        id: ast::ast::NodeId(0),
        object: rec,
        field: "y",
        span: Span::new(0, 9),
    });

    let result = infer_expr(&mut env, access);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Index Errors
// ===========================================================================

/// `42[0]` — indexing a non-array must fail
#[test]
fn hm_index_non_array_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let num = Expr::int("42", Span::new(0, 2), &bump);
    let idx = Expr::int("0", Span::new(3, 4), &bump);
    let expr = bump.alloc(Expr::Index {
        id: ast::ast::NodeId(0),
        array: num,
        index: idx,
        span: Span::new(0, 5),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

/// `[1, 2]["x"]` — non-integer index must fail
#[test]
fn hm_index_with_string_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let arr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in(
            [
                Expr::int("1", Span::new(1, 2), &bump),
                Expr::int("2", Span::new(4, 5), &bump),
            ],
            &bump,
        ),
        span: Span::new(0, 6),
    });
    let idx = Expr::str("x", Span::new(7, 10), &bump);
    let expr = bump.alloc(Expr::Index {
        id: ast::ast::NodeId(0),
        array: arr,
        index: idx,
        span: Span::new(0, 11),
    });

    let result = infer_expr(&mut env, expr);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Empty Collection Edge Cases
// ===========================================================================

/// Empty array `[]` — infers as Array<fresh_var>
#[test]
fn hm_empty_array() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let expr = bump.alloc(Expr::Array {
        id: ast::ast::NodeId(0),
        elems: BumpVec::new_in(&bump),
        span: Span::new(0, 2),
    });

    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Array(_) => {} // OK — any element type is fine
        _ => panic!("expected Array, got {ty:?}"),
    }
}

/// Empty match must fail with NonExhaustiveMatch
#[test]
fn hm_empty_match_fails() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let subject = Expr::int("1", Span::new(6, 7), &bump);
    let arms = BumpVec::new_in(&bump);
    let expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject,
        arms,
        span: Span::new(0, 8),
    });

    let result = infer_expr(&mut env, expr);
    assert!(matches!(result, Err(TypeError::NonExhaustiveMatch { .. })));
}

// ===========================================================================
// HM Edge Case Tests — Annotation with Complex Types
// ===========================================================================

/// Annotation with function type: `let f: i32 -> i32 = (x) => x + 1`
#[test]
fn hm_annotation_function_type_matches() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ann_from = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(7, 10)));
    let ann_to = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(14, 17)));
    let ann = bump.alloc(ast::ast::TypeExpr::Function {
        from: ann_from,
        to: ann_to,
        span: Span::new(7, 17),
    });

    let x_param = Expr::ident("x", Span::new(23, 24), &bump);
    let one = Expr::int("1", Span::new(27, 28), &bump);
    let body = Expr::binary(BinOp::Add, x_param, one, Span::new(23, 28), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(20, 28),
    });

    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: Some(ann),
        value: lambda,
        span: Span::new(0, 28),
    };
    let result = infer_decl(&mut env, &decl);
    assert!(result.is_ok());
}

/// Annotation `i32 -> i32` but value is `(x) => x + 1.0` — conflict
#[test]
fn hm_annotation_function_type_conflict() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ann_from = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(7, 10)));
    let ann_to = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(14, 17)));
    let ann = bump.alloc(ast::ast::TypeExpr::Function {
        from: ann_from,
        to: ann_to,
        span: Span::new(7, 17),
    });

    let x_param = Expr::ident("x", Span::new(23, 24), &bump);
    let one = Expr::float("1.0", Span::new(27, 30), &bump);
    let body = Expr::binary(BinOp::Add, x_param, one, Span::new(23, 30), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(20, 30),
    });

    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: Some(ann),
        value: lambda,
        span: Span::new(0, 30),
    };
    let result = infer_decl(&mut env, &decl);
    assert!(result.is_err());
}

/// Annotation `i32 -> str` on `(x) => x + 1` — return type mismatch
#[test]
fn hm_annotation_return_type_mismatch() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let ann_from = bump.alloc(ast::ast::TypeExpr::Named("i32", Span::new(7, 10)));
    let ann_to = bump.alloc(ast::ast::TypeExpr::Named("str", Span::new(14, 17)));
    let ann = bump.alloc(ast::ast::TypeExpr::Function {
        from: ann_from,
        to: ann_to,
        span: Span::new(7, 17),
    });

    let x_param = Expr::ident("x", Span::new(23, 24), &bump);
    let one = Expr::int("1", Span::new(27, 28), &bump);
    let body = Expr::binary(BinOp::Add, x_param, one, Span::new(23, 28), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(20, 28),
    });

    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: Some(ann),
        value: lambda,
        span: Span::new(0, 28),
    };
    let result = infer_decl(&mut env, &decl);
    assert!(result.is_err());
}

// ===========================================================================
// HM Edge Case Tests — Prelude Functions
// ===========================================================================

/// `const(1, true)` — const is `(a) -> (b) -> a`, should return i32
#[test]
fn hm_prelude_const() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // const(1)(true)
    let const_ref = Expr::ident("const", Span::new(0, 5), &bump);
    let one = Expr::int("1", Span::new(6, 7), &bump);
    let app1 = Expr::app(
        const_ref,
        BumpVec::from_iter_in([one], &bump),
        Span::new(0, 8),
        &bump,
    );
    let t = Expr::bool(true, Span::new(9, 13), &bump);
    let app2 = Expr::app(
        app1,
        BumpVec::from_iter_in([t], &bump),
        Span::new(0, 14),
        &bump,
    );

    let ty = infer_expr(&mut env, app2).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `flip` should be polymorphic and have function type
#[test]
fn hm_prelude_flip() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // Just verify flip is a polymorphic function in the env
    let flip_ref = Expr::ident("flip", Span::new(0, 4), &bump);
    let ty = infer_expr(&mut env, flip_ref).unwrap();
    assert!(
        matches!(ty, MonoType::Func { .. }),
        "flip should be a function, got {ty:?}"
    );
}

// ===========================================================================
// HM Edge Case Tests — Multiple Let Bindings with Dependencies
// ===========================================================================

/// `let a = 1; let b = a + 2; let c = b * 3; c`
#[test]
fn hm_chained_bindings() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let decl_a = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "a",
        ty: None,
        value: Expr::int("1", Span::new(8, 9), &bump),
        span: Span::new(0, 9),
    };
    infer_decl(&mut env, &decl_a).unwrap();

    let a_ref = Expr::ident("a", Span::new(18, 19), &bump);
    let two = Expr::int("2", Span::new(22, 23), &bump);
    let add = Expr::binary(BinOp::Add, a_ref, two, Span::new(18, 23), &bump);
    let decl_b = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "b",
        ty: None,
        value: add,
        span: Span::new(10, 23),
    };
    infer_decl(&mut env, &decl_b).unwrap();

    let b_ref = Expr::ident("b", Span::new(32, 33), &bump);
    let three = Expr::int("3", Span::new(36, 37), &bump);
    let mul = Expr::binary(BinOp::Mul, b_ref, three, Span::new(32, 37), &bump);
    let decl_c = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "c",
        ty: None,
        value: mul,
        span: Span::new(24, 37),
    };
    let poly = infer_decl(&mut env, &decl_c).unwrap();

    assert_eq!(poly.body, MonoType::I32);
}

/// `let a = 1; let b = "hello"; a + 1` — reusing a in a new context
#[test]
fn hm_binding_reuse() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let decl_a = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "a",
        ty: None,
        value: Expr::int("1", Span::new(8, 9), &bump),
        span: Span::new(0, 9),
    };
    infer_decl(&mut env, &decl_a).unwrap();

    let decl_b = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "b",
        ty: None,
        value: Expr::str("hello", Span::new(18, 25), &bump),
        span: Span::new(10, 25),
    };
    infer_decl(&mut env, &decl_b).unwrap();

    let a_ref = Expr::ident("a", Span::new(27, 28), &bump);
    let one = Expr::int("1", Span::new(31, 32), &bump);
    let add = Expr::binary(BinOp::Add, a_ref, one, Span::new(27, 32), &bump);

    let ty = infer_expr(&mut env, add).unwrap();
    assert_eq!(ty, MonoType::I32);
}

// ===========================================================================
// HM Edge Case Tests — Complex Polymorphism Scenarios
// ===========================================================================

/// `let f = (x) => x; f(1); f("a"); f(true)` — three different instantiations
#[test]
fn hm_let_poly_three_instantiations() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    let body = Expr::ident("x", Span::new(11, 12), &bump);
    let params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "x",
            ty: None,
        }],
        &bump,
    );
    let lambda = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params,
        return_type: None,
        body,
        span: Span::new(7, 12),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "f",
        ty: None,
        value: lambda,
        span: Span::new(0, 12),
    };
    infer_decl(&mut env, &decl).unwrap();

    // f(1) -> i32
    let ty1 = infer_expr(
        &mut env,
        Expr::app(
            Expr::ident("f", Span::new(0, 1), &bump),
            BumpVec::from_iter_in([Expr::int("1", Span::new(2, 3), &bump)], &bump),
            Span::new(0, 4),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty1, MonoType::I32);

    // f("a") -> str
    let ty2 = infer_expr(
        &mut env,
        Expr::app(
            Expr::ident("f", Span::new(0, 1), &bump),
            BumpVec::from_iter_in([Expr::str("a", Span::new(2, 5), &bump)], &bump),
            Span::new(0, 6),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty2, MonoType::Str);

    // f(true) -> bool
    let ty3 = infer_expr(
        &mut env,
        Expr::app(
            Expr::ident("f", Span::new(0, 1), &bump),
            BumpVec::from_iter_in([Expr::bool(true, Span::new(2, 6), &bump)], &bump),
            Span::new(0, 7),
            &bump,
        ),
    )
    .unwrap();
    assert_eq!(ty3, MonoType::Bool);
}

/// `let swap = (a) => (b) => (b, a); swap(1, "x")` — returns (str, i32)
#[test]
fn hm_let_poly_swap_tuple() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();

    // swap = (a) => (b) => (b, a)
    let b_param = Expr::ident("b", Span::new(13, 14), &bump);
    let a_ref = Expr::ident("a", Span::new(17, 18), &bump);
    let tuple_expr = bump.alloc(Expr::Tuple {
        id: ast::ast::NodeId(0),
        elems: BumpVec::from_iter_in([b_param, a_ref], &bump),
        span: Span::new(12, 19),
    });
    let inner_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "b",
            ty: None,
        }],
        &bump,
    );
    let inner = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: inner_params,
        return_type: None,
        body: tuple_expr,
        span: Span::new(10, 19),
    });
    let outer_params = BumpVec::from_iter_in(
        [ast::ast::Param {
            name: "a",
            ty: None,
        }],
        &bump,
    );
    let outer = bump.alloc(Expr::Lambda {
        id: ast::ast::NodeId(0),
        params: outer_params,
        return_type: None,
        body: inner,
        span: Span::new(0, 19),
    });
    let decl = Decl::Bind {
        id: ast::ast::NodeId(0),
        name: "swap",
        ty: None,
        value: outer,
        span: Span::new(0, 19),
    };
    infer_decl(&mut env, &decl).unwrap();

    // swap(1) -> (b) -> (b, i32)
    let swap_ref = Expr::ident("swap", Span::new(0, 4), &bump);
    let one = Expr::int("1", Span::new(5, 6), &bump);
    let app1 = Expr::app(
        swap_ref,
        BumpVec::from_iter_in([one], &bump),
        Span::new(0, 7),
        &bump,
    );
    let ty_inner = infer_expr(&mut env, app1).unwrap();
    assert!(
        matches!(ty_inner, MonoType::Func { .. }),
        "swap(1) should be a function, got {ty_inner:?}"
    );

    // swap(1)("x") -> (str, i32)
    let hello = Expr::str("x", Span::new(9, 12), &bump);
    let app2 = Expr::app(
        app1,
        BumpVec::from_iter_in([hello], &bump),
        Span::new(0, 13),
        &bump,
    );
    let ty_tuple = infer_expr(&mut env, app2).unwrap();
    match ty_tuple {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Tuple");
            assert_eq!(payload[0], MonoType::Str);
            assert_eq!(payload[1], MonoType::I32);
        }
        _ => panic!("expected Tuple, got {ty_tuple:?}"),
    }
}

// ===========================================================================
// Constructor Expression Tests
// ===========================================================================

/// `Some(42)` should produce `Option<i32>`
#[test]
fn ctor_some_with_value() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let some_ref = Expr::ident("Some", Span::new(0, 4), &bump);
    let num = Expr::int("42", Span::new(5, 7), &bump);
    let expr = Expr::app(
        some_ref,
        BumpVec::from_iter_in([num], &bump),
        Span::new(0, 8),
        &bump,
    );

    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Option");
            assert_eq!(payload[0], MonoType::I32);
        }
        _ => panic!("expected Option<i32>, got {ty:?}"),
    }
}

/// `None` should produce `Option<?fresh>`
#[test]
fn ctor_none() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let expr = Expr::ident("None", Span::new(0, 4), &bump);
    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Option");
            assert_eq!(payload.len(), 1);
        }
        _ => panic!("expected Option<_>, got {ty:?}"),
    }
}

/// `Ok("hello")` should produce `Result<str, ?fresh>`
#[test]
fn ctor_ok() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let ok_ref = Expr::ident("Ok", Span::new(0, 2), &bump);
    let s = Expr::str("hello", Span::new(3, 10), &bump);
    let expr = Expr::app(
        ok_ref,
        BumpVec::from_iter_in([s], &bump),
        Span::new(0, 11),
        &bump,
    );

    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Result");
            // payload order: [e, t] = [Err_type, Ok_type]
            assert_eq!(payload[1], MonoType::Str); // Ok("hello") → Ok_type = Str at index 1
            assert_eq!(payload.len(), 2);
        }
        _ => panic!("expected Result<str, _>, got {ty:?}"),
    }
}

/// `Err(42)` should produce `Result<?fresh, i32>`
#[test]
fn ctor_err() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let err_ref = Expr::ident("Err", Span::new(0, 3), &bump);
    let num = Expr::int("42", Span::new(4, 6), &bump);
    let expr = Expr::app(
        err_ref,
        BumpVec::from_iter_in([num], &bump),
        Span::new(0, 7),
        &bump,
    );

    let ty = infer_expr(&mut env, expr).unwrap();
    match ty {
        MonoType::Tag { name, payload } => {
            assert_eq!(name.as_str(), "Result");
            // payload order: [e, t] = [Err_type, Ok_type]
            assert_eq!(payload[0], MonoType::I32); // Err(42) → Err_type = I32 at index 0
            assert_eq!(payload.len(), 2);
        }
        _ => panic!("expected Result<_, i32>, got {ty:?}"),
    }
}

/// `Some(42)` and `Some("hello")` have different payload types (polymorphism)
#[test]
fn ctor_some_polymorphic() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // Some(42) -> Option<i32>
    let some1 = Expr::ident("Some", Span::new(0, 4), &bump);
    let num = Expr::int("42", Span::new(5, 7), &bump);
    let expr1 = Expr::app(
        some1,
        BumpVec::from_iter_in([num], &bump),
        Span::new(0, 8),
        &bump,
    );
    let ty1 = infer_expr(&mut env, expr1).unwrap();

    // Some("hello") -> Option<str>
    let some2 = Expr::ident("Some", Span::new(0, 4), &bump);
    let s = Expr::str("hello", Span::new(5, 12), &bump);
    let expr2 = Expr::app(
        some2,
        BumpVec::from_iter_in([s], &bump),
        Span::new(0, 13),
        &bump,
    );
    let ty2 = infer_expr(&mut env, expr2).unwrap();

    // Both should be Option but with different payloads
    match (&ty1, &ty2) {
        (
            MonoType::Tag {
                name: n1,
                payload: p1,
            },
            MonoType::Tag {
                name: n2,
                payload: p2,
            },
        ) => {
            assert_eq!(n1.as_str(), "Option");
            assert_eq!(n2.as_str(), "Option");
            assert_eq!(p1[0], MonoType::I32);
            assert_eq!(p2[0], MonoType::Str);
        }
        _ => panic!("expected two Option tags, got {ty1:?} and {ty2:?}"),
    }
}

// ===========================================================================
// Pattern Matching with Constructors
// ===========================================================================

/// `match x { Some(v) => v None => 0 }` on `Option<i32>` — v should be i32
#[test]
fn pattern_match_option_some() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    // Build: match x { Some(v) => v, None => 0 }
    // where x: Option<i32>
    let x_ref = Expr::ident("x", Span::new(6, 7), &bump);

    // Some(v) => v
    let v_ref = Expr::ident("v", Span::new(16, 17), &bump);
    let some_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "Some",
        fields: BumpVec::from_iter_in(
            [Pattern::Binding(
                ast::ast::NodeId(0),
                "v",
                Span::new(15, 16),
            )],
            &bump,
        ),
        span: Span::new(11, 18),
    };
    let arm1 = MatchArm {
        pattern: bump.alloc(some_pat),
        body: v_ref,
    };

    // None => 0
    let zero = Expr::int("0", Span::new(27, 28), &bump);
    let none_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "None",
        fields: BumpVec::new_in(&bump),
        span: Span::new(21, 25),
    };
    let arm2 = MatchArm {
        pattern: bump.alloc(none_pat),
        body: zero,
    };

    let arms = BumpVec::from_iter_in([arm1, arm2], &bump);
    let match_expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject: x_ref,
        arms,
        span: Span::new(0, 29),
    });

    // Bind x: Option<i32>
    let opt_i32 = MonoType::Tag {
        name: "Option".into(),
        payload: Rc::from([MonoType::I32]),
    };
    env.insert("x", PolyType::mono(opt_i32));

    let ty = infer_expr(&mut env, match_expr).unwrap();
    assert_eq!(ty, MonoType::I32);
}

/// `match x { Ok(v) => v, Err(_) => 0 }` on `Result<str, i32>` — v should be str
#[test]
fn pattern_match_result_ok() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let x_ref = Expr::ident("x", Span::new(6, 7), &bump);

    // Ok(v) => v
    let v_ref = Expr::ident("v", Span::new(15, 16), &bump);
    let ok_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "Ok",
        fields: BumpVec::from_iter_in(
            [Pattern::Binding(
                ast::ast::NodeId(0),
                "v",
                Span::new(14, 15),
            )],
            &bump,
        ),
        span: Span::new(10, 17),
    };
    let arm1 = MatchArm {
        pattern: bump.alloc(ok_pat),
        body: v_ref,
    };

    // Err(_) => "" (str) — same type as Ok arm
    let empty = Expr::str("", Span::new(28, 30), &bump);
    let err_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "Err",
        fields: BumpVec::from_iter_in(
            [Pattern::Wildcard(ast::ast::NodeId(0), Span::new(25, 26))],
            &bump,
        ),
        span: Span::new(21, 27),
    };
    let arm2 = MatchArm {
        pattern: bump.alloc(err_pat),
        body: empty,
    };

    let arms = BumpVec::from_iter_in([arm1, arm2], &bump);
    let match_expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject: x_ref,
        arms,
        span: Span::new(0, 31),
    });

    // Bind x: Result<str, i32> — payload order [e, t] = [Err, Ok]
    let res_str_i32 = MonoType::Tag {
        name: "Result".into(),
        payload: Rc::from([MonoType::I32, MonoType::Str]),
    };
    env.insert("x", PolyType::mono(res_str_i32));

    let ty = infer_expr(&mut env, match_expr).unwrap();
    assert_eq!(ty, MonoType::Str);
}

/// Mismatched arms: `match x { Some(v) => v, None => "hello" }` should fail
#[test]
fn pattern_match_option_mismatched_arms() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    env.load_prelude();

    let x_ref = Expr::ident("x", Span::new(6, 7), &bump);

    // Some(v) => v (i32)
    let v_ref = Expr::ident("v", Span::new(16, 17), &bump);
    let some_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "Some",
        fields: BumpVec::from_iter_in(
            [Pattern::Binding(
                ast::ast::NodeId(0),
                "v",
                Span::new(15, 16),
            )],
            &bump,
        ),
        span: Span::new(11, 18),
    };
    let arm1 = MatchArm {
        pattern: bump.alloc(some_pat),
        body: v_ref,
    };

    // None => "hello" (str) — mismatch!
    let hello = Expr::str("hello", Span::new(27, 34), &bump);
    let none_pat = Pattern::Constructor {
        id: ast::ast::NodeId(0),
        name: "None",
        fields: BumpVec::new_in(&bump),
        span: Span::new(21, 25),
    };
    let arm2 = MatchArm {
        pattern: bump.alloc(none_pat),
        body: hello,
    };

    let arms = BumpVec::from_iter_in([arm1, arm2], &bump);
    let match_expr = bump.alloc(Expr::Match {
        id: ast::ast::NodeId(0),
        subject: x_ref,
        arms,
        span: Span::new(0, 35),
    });

    let opt_i32 = MonoType::Tag {
        name: "Option".into(),
        payload: Rc::from([MonoType::I32]),
    };
    env.insert("x", PolyType::mono(opt_i32));

    let result = infer_expr(&mut env, match_expr);
    assert!(result.is_err());
}
