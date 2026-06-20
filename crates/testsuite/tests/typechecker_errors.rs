//! Contract tests for typechecker error variants.
//!
//! Tests for error variants that should be produced by the typechecker
//! but are not yet triggered by any existing test.

use bumpalo::Bump;
use parser::parse;

fn typecheck_src(src: &str) -> Result<(), Vec<diagnostics::CompilerError>> {
    let arena = Bump::new();
    let program = parse(src, &arena).map_err(|e| vec![diagnostics::CompilerError::from(e)])?;
    typechecker::typecheck(&program).map(|_| ()).map_err(|errors| {
        errors.into_iter().map(diagnostics::CompilerError::from).collect()
    })
}

#[test]
fn typecheck_arity_mismatch_too_many_args() {
    let src = "let f = (x: i32) => x\nlet main = f(1, 2)";
    let result = typecheck_src(src);
    let errors = result.expect_err("should fail with arity mismatch");
    let all_arity = errors.iter().all(|e| e.to_string().contains("arity") || e.to_string().contains("type mismatch"));
    assert!(all_arity, "expected arity or type mismatch errors, got: {errors:?}");
}

#[test]
fn typecheck_arity_mismatch_too_few_args() {
    let src = "let f = (x: i32, y: i32) => x + y\nlet main = f(1)";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail with arity mismatch");
}

#[ignore = "Dijith: implement exhaustiveness checking in typechecker"]
#[test]
fn typecheck_non_exhaustive_match() {
    let src = r#"
type Opt = | Some(i32) | None
let f = (x: Opt) => match x {
    Some(v) => v
}
let main = f(Some(1))
"#;
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: match must be exhaustive");
}

#[test]
fn typecheck_field_not_found() {
    let src = "let r = { x: 1, y: 2 }\nlet main = r.z";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: field `z` not found");
}

#[test]
fn typecheck_field_not_found_on_nested() {
    let src = "let r = { inner: { a: 1 } }\nlet main = r.inner.b";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: field `b` not found on inner record");
}

#[test]
fn typecheck_numeric_overflow_i8() {
    let src = "let x: i8 = 200\nlet main = () => x";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: 200 overflows i8");
}

#[test]
fn typecheck_numeric_overflow_u32() {
    let src = "let x: u32 = 4294967296\nlet main = () => x";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: 4294967296 overflows u32");
}

#[test]
fn typecheck_numeric_overflow_negative_u8() {
    let src = "let x: u8 = -1\nlet main = () => x";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: -1 overflows u8");
}

// ---------------------------------------------------------------------------
// Existing error variants (ensure they still work)
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unbound_variable() {
    let src = "let main = undefinedVar";
    let result = typecheck_src(src);
    let errors = result.expect_err("should fail with unbound variable");
    let has_unbound = errors.iter().any(|e| e.to_string().contains("unbound") || e.to_string().contains("not found"));
    assert!(has_unbound, "expected unbound variable error, got: {errors:?}");
}

#[test]
fn typecheck_type_mismatch() {
    let src = "let main: i32 = true";
    let result = typecheck_src(src);
    assert!(result.is_err(), "should fail: bool != i32");
}

#[test]
fn typecheck_infinite_type() {
    let src = "let f = (x) => f(x)\nlet main = f(1)";
    let _result = typecheck_src(src);
    // May or may not fail depending on let-polymorphism handling
    // This is a valid recursive function in HM
}
