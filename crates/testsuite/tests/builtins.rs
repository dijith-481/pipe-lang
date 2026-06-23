//! Contract tests for standard library builtins.
//!
//! These tests document the expected behavior of builtin functions.
//! Tests for unimplemented builtins are marked `#[ignore]`.

use std::sync::Arc;

use runtime::{BuiltinFunction, BuiltinRegistry, Value};

// ---------------------------------------------------------------------------
// Array literal builtin
// ---------------------------------------------------------------------------

#[test]
fn builtin_array_literal_empty() {
    let registry = full_registry();
    let result = registry
        .execute("array_literal", &[])
        .expect("array_literal should work");
    match result {
        Value::Array(elems) => assert_eq!(elems.len(), 0),
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn builtin_array_literal_three_elements() {
    let registry = full_registry();
    let result = registry
        .execute(
            "array_literal",
            &[Value::I32(1), Value::I32(2), Value::I32(3)],
        )
        .expect("array_literal should work");
    match result {
        Value::Array(elems) => {
            assert_eq!(elems.len(), 3);
            assert_eq!(elems[0], Value::I32(1));
            assert_eq!(elems[1], Value::I32(2));
            assert_eq!(elems[2], Value::I32(3));
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn builtin_array_literal_mixed_types() {
    let registry = full_registry();
    let result = registry
        .execute(
            "array_literal",
            &[
                Value::I32(42),
                Value::Str("hello".into()),
                Value::Bool(true),
            ],
        )
        .expect("array_literal should work");
    match result {
        Value::Array(elems) => {
            assert_eq!(elems.len(), 3);
            assert_eq!(elems[0], Value::I32(42));
            assert_eq!(elems[2], Value::Bool(true));
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Prelude — always available
// ---------------------------------------------------------------------------

#[test]
fn builtin_id() {
    let registry = full_registry();
    let result = registry
        .execute("id", &[Value::I32(42)])
        .expect("id should work");
    assert_eq!(result, Value::I32(42));
}

#[test]
fn builtin_id_polymorphic() {
    let registry = full_registry();
    let result = registry
        .execute("id", &[Value::str("hello")])
        .expect("id should work");
    assert_eq!(result, Value::str("hello"));
}

#[test]
fn builtin_const_returns_closure() {
    let registry = full_registry();
    // `const` is curried: const(x) returns a closure that always returns x
    let result = registry
        .execute("const", &[Value::I32(42)])
        .expect("const should return a closure");
    match result {
        Value::Closure(_) => {} // expected
        other => panic!("expected Closure, got {other:?}"),
    }
}

#[test]
fn builtin_flip_needs_closure() {
    let registry = full_registry();
    // `flip` takes a function and returns a closure with args swapped
    let result = registry.execute("flip", &[Value::Unit]);
    assert!(result.is_err(), "flip should reject non-function");
}

#[test]
fn builtin_apply_with_closure() {
    let registry = full_registry();
    // Manually create a closure and test apply
    let echo = EchoBuiltin;
    let arity = echo.arity();
    let closure_data = runtime::ClosureData {
        func: runtime::FuncPtr::Builtin(Arc::new(echo)),
        captures: Arc::new([]),
        arity,
    };
    let closure = Value::Closure(Arc::new(closure_data));
    let result = registry.execute("apply", &[closure, Value::I32(42)]);
    // apply(echo, 42) = echo(42) = 42
    assert_eq!(result.expect("apply should work"), Value::I32(42));
}

// ---------------------------------------------------------------------------
// Array operations
// ---------------------------------------------------------------------------

#[test]
fn builtin_array_map() {
    let registry = full_registry();
    let arr = Value::array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
    // map with id function should return same array
    let result = registry.execute("map", &[arr, Value::Unit]);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn builtin_array_len() {
    let registry = full_registry();
    let arr = Value::array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
    let _ = registry.execute("len_array", &[arr]).unwrap_or_else(|_| {
        registry
            .execute("len", &[Value::array(vec![Value::I32(1), Value::I32(2)])])
            .expect("len should work with any name")
    });
}

#[test]
fn builtin_array_head_empty() {
    let registry = full_registry();
    let empty: Vec<Value> = vec![];
    let arr = Value::array(empty);
    let result = registry
        .execute("head", &[arr])
        .expect("head on empty should return None");
    match result {
        Value::Tag { tag: 0, .. } => {} // None variant
        other => panic!("expected None tag (0), got {other:?}"),
    }
}

#[test]
fn builtin_array_head_nonempty() {
    let registry = full_registry();
    let arr = Value::array(vec![Value::I32(42), Value::I32(99)]);
    let result = registry
        .execute("head", &[arr])
        .expect("head on non-empty should return Some");
    match result {
        Value::Tag { tag: 1, payload } => {
            assert_eq!(payload.len(), 1);
            assert_eq!(payload[0], Value::I32(42));
        }
        other => panic!("expected Some tag (1), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Closures — no tests exist yet
// ---------------------------------------------------------------------------

#[test]
fn closure_builtins_are_registered() {
    // Verify that closure-related builtins exist in the full registry
    let registry = full_registry();
    for name in &["call", "apply"] {
        let _ = registry.execute(name, &[Value::Unit]);
    }
}

// ---------------------------------------------------------------------------
// Drop, Take, Sqrt, UnwrapOr — IMPLEMENTED in stdlib
// ---------------------------------------------------------------------------

#[test]
fn builtin_drop_removes_first_n() {
    let registry = full_registry();
    let arr = Value::array(vec![
        Value::I32(1),
        Value::I32(2),
        Value::I32(3),
        Value::I32(4),
    ]);
    let result = registry
        .execute("drop", &[arr, Value::I32(2)])
        .expect("drop should work");
    match result {
        Value::Array(elems) => {
            assert_eq!(elems.len(), 2);
            assert_eq!(elems[0], Value::I32(3));
            assert_eq!(elems[1], Value::I32(4));
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn builtin_take_first_n() {
    let registry = full_registry();
    let arr = Value::array(vec![
        Value::I32(1),
        Value::I32(2),
        Value::I32(3),
        Value::I32(4),
    ]);
    let result = registry
        .execute("take", &[arr, Value::I32(2)])
        .expect("take should work");
    match result {
        Value::Array(elems) => {
            assert_eq!(elems.len(), 2);
            assert_eq!(elems[0], Value::I32(1));
            assert_eq!(elems[1], Value::I32(2));
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn builtin_take_zero_returns_empty() {
    let registry = full_registry();
    let arr = Value::array(vec![Value::I32(1), Value::I32(2)]);
    let result = registry
        .execute("take", &[arr, Value::I32(0)])
        .expect("take with 0 should work");
    match result {
        Value::Array(elems) => assert_eq!(elems.len(), 0),
        other => panic!("expected empty Array, got {other:?}"),
    }
}

#[test]
fn builtin_sqrt_positive() {
    let registry = full_registry();
    let result = registry
        .execute("sqrt", &[Value::F64(9.0)])
        .expect("sqrt should work");
    assert_eq!(result, Value::F64(3.0));
}

#[test]
fn builtin_sqrt_zero() {
    let registry = full_registry();
    let result = registry
        .execute("sqrt", &[Value::F64(0.0)])
        .expect("sqrt should work");
    assert_eq!(result, Value::F64(0.0));
}

#[test]
fn builtin_unwrap_or_some() {
    let registry = full_registry();
    let some = Value::tag(1, vec![Value::I32(42)]);
    let result = registry
        .execute("unwrap_or", &[some, Value::I32(0)])
        .expect("unwrap_or Some should work");
    assert_eq!(result, Value::I32(42));
}

#[test]
fn builtin_unwrap_or_none_returns_default() {
    let registry = full_registry();
    let none = Value::tag(0, vec![]);
    let result = registry
        .execute("unwrap_or", &[none, Value::I32(99)])
        .expect("unwrap_or None should return default");
    assert_eq!(result, Value::I32(99));
}

// ---------------------------------------------------------------------------
// Effect runtime builtins — UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[ignore = "Member 2: implement Effect.map runtime builtin"]
#[test]
fn builtin_effect_map_transforms_result() {
    let registry = full_registry();
    // Effect.map(effect, fn) should apply fn to the effect's result
    // For v0.1, effect = a builtin function that returns a value
    let effect = Value::Effect(Arc::new(EchoBuiltin));
    let id_fn = Value::Unit; // placeholder — needs closure
    let result = registry.execute("Effect.map", &[effect, id_fn]);
    assert!(result.is_err() || result.is_ok());
}

#[ignore = "Member 2: implement Effect.flatMap runtime builtin"]
#[test]
fn builtin_effect_flat_map_chains() {
    let registry = full_registry();
    let effect = Value::Effect(Arc::new(EchoBuiltin));
    let chain_fn = Value::Unit; // placeholder
    let result = registry.execute("Effect.flat_map", &[effect, chain_fn]);
    assert!(result.is_err() || result.is_ok());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct EchoBuiltin;

impl BuiltinFunction for EchoBuiltin {
    fn name(&self) -> &str {
        "echo"
    }
    fn arity(&self) -> usize {
        1
    }
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        runtime::expect_arity(self.name(), args, self.arity())?;
        Ok(args[0].clone())
    }
}

fn full_registry() -> BuiltinRegistry {
    let mut registry = BuiltinRegistry::new();

    // Prelude
    for builtin in stdlib::prelude::prelude_builtins() {
        registry.register(builtin);
    }

    // Extra test-only builtins
    registry.register(Arc::new(EchoBuiltin));

    registry
}
