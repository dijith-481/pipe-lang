use std::sync::Arc;

use ast::SmolStr;
use runtime::{BuiltinFunction, RuntimeError, Value};

// ---------------------------------------------------------------------------
// Prelude type definitions (for the typechecker)
// ---------------------------------------------------------------------------

/// Returns the prelude type definitions as a list of (name, definition) pairs.
///
/// These are the types automatically available in every pipe-lang program:
/// - `Option<T>`: `| Some(T) | None`
/// - `Result<T, E>`: `| Ok(T) | Err(E)`
#[must_use]
pub fn prelude_type_names() -> Vec<&'static str> {
    vec!["Option", "Result"]
}

// ---------------------------------------------------------------------------
// Prelude builtins (for the runtime)
// ---------------------------------------------------------------------------

/// Returns all builtins that are available without an import.
///
/// # Returns
///
/// A list of builtin implementations ready to register in a
/// [`runtime::BuiltinRegistry`].
#[must_use]
pub fn prelude_builtins() -> Vec<Arc<dyn BuiltinFunction>> {
    vec![
        Arc::new(Id),
        Arc::new(Const),
        Arc::new(Flip),
        Arc::new(Compose),
        Arc::new(Pipe),
        Arc::new(Apply),
    ]
}

// ---------------------------------------------------------------------------
// Builtin implementations
// ---------------------------------------------------------------------------

/// Identity function: `id(x) = x`
#[derive(Debug)]
struct Id;

impl BuiltinFunction for Id {
    fn name(&self) -> SmolStr {
        SmolStr::new("id")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        Ok(args[0].clone())
    }
}

/// Constant function: `const(a)(b) = a`
///
/// Takes one argument and returns a closure that ignores its argument
/// and returns the first argument.
#[derive(Debug)]
struct Const;

impl BuiltinFunction for Const {
    fn name(&self) -> SmolStr {
        SmolStr::new("const")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let value = args[0].clone();
        Ok(Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(ConstInner(value))),
            captures: Arc::from([]),
            arity: 1,
        })))
    }
}

/// Inner closure returned by `const`: ignores its argument, returns captured value.
#[derive(Debug)]
struct ConstInner(Value);

impl BuiltinFunction for ConstInner {
    fn name(&self) -> SmolStr {
        SmolStr::new("const.closure")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, _args: &[Value]) -> Result<Value, RuntimeError> {
        Ok(self.0.clone())
    }
}

/// Flip argument order: `flip(f)(a, b) = f(b, a)`
#[derive(Debug)]
struct Flip;

impl BuiltinFunction for Flip {
    fn name(&self) -> SmolStr {
        SmolStr::new("flip")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let func = match &args[0] {
            Value::Closure(c) => c.clone(),
            _ => {
                return Err(RuntimeError::TypeMismatch {
                    expected: "Closure".into(),
                    got: format!("{:?}", &args[0]),
                });
            }
        };
        Ok(Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(FlipInner(func))),
            captures: Arc::from([]),
            arity: 2,
        })))
    }
}

/// Inner closure returned by `flip`: swaps arguments and calls original.
#[derive(Debug)]
struct FlipInner(Arc<runtime::value::ClosureData>);

impl BuiltinFunction for FlipInner {
    fn name(&self) -> SmolStr {
        SmolStr::new("flip.closure")
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        // Swap args: flip(f)(a, b) = f(b, a)
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }
        let swapped = vec![args[1].clone(), args[0].clone()];
        call_closure(&self.0, &swapped)
    }
}

/// Function composition: `compose(f, g)(x) = f(g(x))`
#[derive(Debug)]
struct Compose;

impl BuiltinFunction for Compose {
    fn name(&self) -> SmolStr {
        SmolStr::new("compose")
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let f = extract_closure(&args[0])?;
        let g = extract_closure(&args[1])?;
        Ok(Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(ComposeInner(f, g))),
            captures: Arc::from([]),
            arity: 1,
        })))
    }
}

/// Inner closure for compose: applies g then f.
#[derive(Debug)]
struct ComposeInner(
    Arc<runtime::value::ClosureData>,
    Arc<runtime::value::ClosureData>,
);

impl BuiltinFunction for ComposeInner {
    fn name(&self) -> SmolStr {
        SmolStr::new("compose.closure")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let g_result = call_closure(&self.1, args)?;
        call_closure(&self.0, &[g_result])
    }
}

/// Pipe (reverse compose): `pipe(f, g)(x) = g(f(x))`
#[derive(Debug)]
struct Pipe;

impl BuiltinFunction for Pipe {
    fn name(&self) -> SmolStr {
        SmolStr::new("pipe")
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let f = extract_closure(&args[0])?;
        let g = extract_closure(&args[1])?;
        Ok(Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(PipeInner(f, g))),
            captures: Arc::from([]),
            arity: 1,
        })))
    }
}

/// Inner closure for pipe: applies f then g.
#[derive(Debug)]
struct PipeInner(
    Arc<runtime::value::ClosureData>,
    Arc<runtime::value::ClosureData>,
);

impl BuiltinFunction for PipeInner {
    fn name(&self) -> SmolStr {
        SmolStr::new("pipe.closure")
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let f_result = call_closure(&self.0, args)?;
        call_closure(&self.1, &[f_result])
    }
}

/// Apply a function to a value: `apply(f, x) = f(x)`
#[derive(Debug)]
struct Apply;

impl BuiltinFunction for Apply {
    fn name(&self) -> SmolStr {
        SmolStr::new("apply")
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let func = extract_closure(&args[0])?;
        call_closure(&func, &[args[1].clone()])
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_closure(val: &Value) -> Result<Arc<runtime::value::ClosureData>, RuntimeError> {
    match val {
        Value::Closure(c) => Ok(c.clone()),
        _ => Err(RuntimeError::TypeMismatch {
            expected: "Closure".into(),
            got: format!("{val:?}"),
        }),
    }
}

fn call_closure(
    closure: &runtime::value::ClosureData,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match &closure.func {
        runtime::value::FuncPtr::Builtin(f) => f.execute(args),
        runtime::value::FuncPtr::Jit { .. } => Err(RuntimeError::EffectError {
            msg: "JIT closures not yet supported in prelude".into(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_has_type_names() {
        let names = prelude_type_names();
        assert!(names.contains(&"Option"));
        assert!(names.contains(&"Result"));
    }

    #[test]
    fn prelude_has_builtins() {
        let builtins = prelude_builtins();
        let names: Vec<String> = builtins.iter().map(|b| b.name().to_string()).collect();
        assert!(names.iter().any(|n| n == "id"));
        assert!(names.iter().any(|n| n == "const"));
        assert!(names.iter().any(|n| n == "flip"));
        assert!(names.iter().any(|n| n == "compose"));
        assert!(names.iter().any(|n| n == "pipe"));
        assert!(names.iter().any(|n| n == "apply"));
    }

    #[test]
    fn id_returns_same_value() {
        let id = Id;
        let val = Value::I32(42);
        let result = id.execute(std::slice::from_ref(&val)).unwrap();
        assert_eq!(result, val);
    }

    #[test]
    fn const_returns_closure() {
        let c = Const;
        let val = Value::I32(42);
        let result = c.execute(&[val]).unwrap();
        assert!(matches!(result, Value::Closure(_)));
    }

    #[test]
    fn apply_calls_function() {
        let apply = Apply;
        let add_one = Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(AddOne)),
            captures: Arc::from([]),
            arity: 1,
        }));
        let result = apply.execute(&[add_one, Value::I32(5)]).unwrap();
        assert_eq!(result.as_i32(), Some(6));
    }

    #[test]
    fn compose_composes_functions() {
        let compose = Compose;
        let double = Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(Double)),
            captures: Arc::from([]),
            arity: 1,
        }));
        let add_one = Value::Closure(Arc::new(runtime::value::ClosureData {
            func: runtime::value::FuncPtr::Builtin(Arc::new(AddOne)),
            captures: Arc::from([]),
            arity: 1,
        }));

        // compose(addOne, double)(5) = addOne(double(5)) = addOne(10) = 11
        let composed = compose.execute(&[add_one, double]).unwrap();
        let apply = Apply;
        let result = apply.execute(&[composed, Value::I32(5)]).unwrap();
        assert_eq!(result.as_i32(), Some(11));
    }

    // Test helper closures
    #[derive(Debug)]
    struct AddOne;
    impl BuiltinFunction for AddOne {
        fn name(&self) -> SmolStr {
            SmolStr::new("addOne")
        }
        fn arity(&self) -> usize {
            1
        }
        fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
            match &args[0] {
                Value::I32(n) => Ok(Value::I32(n + 1)),
                _ => Err(RuntimeError::TypeMismatch {
                    expected: "I32".into(),
                    got: format!("{:?}", &args[0]),
                }),
            }
        }
    }

    #[derive(Debug)]
    struct Double;
    impl BuiltinFunction for Double {
        fn name(&self) -> SmolStr {
            SmolStr::new("double")
        }
        fn arity(&self) -> usize {
            1
        }
        fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
            match &args[0] {
                Value::I32(n) => Ok(Value::I32(n * 2)),
                _ => Err(RuntimeError::TypeMismatch {
                    expected: "I32".into(),
                    got: format!("{:?}", &args[0]),
                }),
            }
        }
    }
}
