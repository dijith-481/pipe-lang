//! Integration tests for the pipe-lang typechecker pipeline.
//!
//! These tests verify that the lexer, AST, and typechecker work together
//! correctly. Once the parser is ready, these tests will be extended to
//! cover the full `lex -> parse -> typecheck` pipeline.

use ast::ast::{BinOp, Decl, Expr};
use ast::span::Span;
use bumpalo::Bump;
use lexer::Lexer;
use typechecker::{MonoType, PolyType, TypeEnv, infer_decl, infer_expr};

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
    let tokens = lex_tokens("let type if then else match do use true false");
    let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
    assert!(kinds.contains(&&lexer::TokenKind::Let));
    assert!(kinds.contains(&&lexer::TokenKind::Type));
    assert!(kinds.contains(&&lexer::TokenKind::If));
    assert!(kinds.contains(&&lexer::TokenKind::Then));
    assert!(kinds.contains(&&lexer::TokenKind::Else));
    assert!(kinds.contains(&&lexer::TokenKind::Match));
    assert!(kinds.contains(&&lexer::TokenKind::Do));
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
        name: "x",
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
    let decl = Decl::Import {
        path: "stdlib.io",
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
        name: "x",
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
    use typechecker::unify;
    let result = unify(&MonoType::I32, &MonoType::I32);
    assert!(result.is_ok());
}

#[test]
fn integration_unify_different_fails() {
    use typechecker::unify;
    let result = unify(&MonoType::I32, &MonoType::Str);
    assert!(result.is_err());
}
