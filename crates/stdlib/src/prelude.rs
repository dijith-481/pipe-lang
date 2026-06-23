use std::sync::Arc;

use runtime::{BuiltinFunction, ClosureData, FuncPtr, Value, expect_arity};

use crate::closure::call_closure;
use crate::{array, io, numeric, ops, option, result as rslt, str as string};

// ---------------------------------------------------------------------------
// Prelude type definitions (for the typechecker)
// ---------------------------------------------------------------------------

/// Returns the prelude type definitions as a list of names.
///
/// These are the types automatically available in every pipe-lang program:
/// `Option<T>` and `Result<T, E>`.
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
        // Core utility functions
        Arc::new(Id),
        Arc::new(Const),
        Arc::new(Flip),
        Arc::new(Compose),
        Arc::new(Pipe),
        Arc::new(Apply),
        // Array operations
        Arc::new(array::ArrayLiteral),
        Arc::new(array::ArrayMap),
        Arc::new(array::ArrayFilter),
        Arc::new(array::ArrayFold),
        Arc::new(array::ArrayFlatMap),
        Arc::new(array::ArrayConcat),
        Arc::new(array::ArrayPrepend),
        Arc::new(array::ArrayLen),
        Arc::new(array::ArrayHead),
        Arc::new(array::ArrayTail),
        // String operations
        Arc::new(string::StrConcat),
        Arc::new(string::StrLen),
        Arc::new(string::StrSplit),
        Arc::new(string::StrTrim),
        Arc::new(string::StrParseI32),
        // IO
        Arc::new(io::IoPrintln),
        Arc::new(io::IoPrint),
        Arc::new(io::IoReadLine),
        Arc::new(io::IoReadFile),
        // Option combinators
        Arc::new(option::OptionMap),
        Arc::new(option::OptionFlatMap),
        Arc::new(option::OptionUnwrapOr),
        // Result combinators
        Arc::new(rslt::ResultMap),
        Arc::new(rslt::ResultFlatMap),
        // Numeric conversions
        Arc::new(numeric::ToI64),
        Arc::new(numeric::ToI32),
        Arc::new(numeric::ToF64),
        Arc::new(numeric::ToStr),
        // Numeric functions
        Arc::new(numeric::Sqrt),
        // Array utilities
        Arc::new(array::ArrayDrop),
        Arc::new(array::ArrayTake),
        // Tag utilities
        Arc::new(ops::Unwrap),
        // IO standalones
        Arc::new(io::ReadLine),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn closure_value(builtin: Arc<dyn BuiltinFunction>, arity: usize) -> Value {
    Value::Closure(Arc::new(ClosureData {
        func: FuncPtr::Builtin(builtin),
        captures: Arc::from([]),
        arity,
    }))
}

fn expect_closure(name: &str, value: &Value) -> Result<Arc<ClosureData>, String> {
    match value {
        Value::Closure(closure) => Ok(Arc::clone(closure)),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

// ---------------------------------------------------------------------------
// Builtin implementations
// ---------------------------------------------------------------------------

// -- Id --

#[derive(Clone, Copy, Debug, Default)]
struct Id;

impl BuiltinFunction for Id {
    fn name(&self) -> &str {
        "id"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        Ok(args[0].clone())
    }
}

// -- Const --

#[derive(Clone, Copy, Debug, Default)]
struct Const;

impl BuiltinFunction for Const {
    fn name(&self) -> &str {
        "const"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        Ok(closure_value(Arc::new(ConstInner(args[0].clone())), 1))
    }
}

/// Inner closure returned by `const`: ignores its argument, returns captured value.
#[derive(Debug, Clone)]
struct ConstInner(Value);

impl BuiltinFunction for ConstInner {
    fn name(&self) -> &str {
        "const.closure"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, _args: &[Value]) -> Result<Value, String> {
        Ok(self.0.clone())
    }
}

// -- Flip --

#[derive(Clone, Copy, Debug, Default)]
struct Flip;

impl BuiltinFunction for Flip {
    fn name(&self) -> &str {
        "flip"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let func = expect_closure(self.name(), &args[0])?.clone();
        Ok(closure_value(Arc::new(FlipInner(func)), 2))
    }
}

/// Inner closure returned by `flip`: swaps arguments and calls original.
#[derive(Debug, Clone)]
struct FlipInner(Arc<ClosureData>);

impl BuiltinFunction for FlipInner {
    fn name(&self) -> &str {
        "flip.closure"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        if args.len() < 2 {
            return Err(format!(
                "flip.closure expected 2 arguments, got {}",
                args.len()
            ));
        }
        let swapped = vec![args[1].clone(), args[0].clone()];
        call_closure(&self.0, &swapped)
    }
}

// -- Compose --

#[derive(Clone, Copy, Debug, Default)]
struct Compose;

impl BuiltinFunction for Compose {
    fn name(&self) -> &str {
        "compose"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let f = expect_closure(self.name(), &args[0])?.clone();
        let g = expect_closure(self.name(), &args[1])?.clone();
        Ok(closure_value(Arc::new(ComposeInner(f, g)), 1))
    }
}

/// Inner closure for compose: applies g then f.
#[derive(Debug, Clone)]
struct ComposeInner(Arc<ClosureData>, Arc<ClosureData>);

impl BuiltinFunction for ComposeInner {
    fn name(&self) -> &str {
        "compose.closure"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        let g_result = call_closure(&self.1, args)?;
        call_closure(&self.0, &[g_result])
    }
}

// -- Pipe --

#[derive(Clone, Copy, Debug, Default)]
struct Pipe;

impl BuiltinFunction for Pipe {
    fn name(&self) -> &str {
        "pipe"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let f = expect_closure(self.name(), &args[0])?.clone();
        let g = expect_closure(self.name(), &args[1])?.clone();
        Ok(closure_value(Arc::new(PipeInner(f, g)), 1))
    }
}

/// Inner closure for pipe: applies f then g.
#[derive(Debug, Clone)]
struct PipeInner(Arc<ClosureData>, Arc<ClosureData>);

impl BuiltinFunction for PipeInner {
    fn name(&self) -> &str {
        "pipe.closure"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        let f_result = call_closure(&self.0, args)?;
        call_closure(&self.1, &[f_result])
    }
}

// -- Apply --

#[derive(Clone, Copy, Debug, Default)]
struct Apply;

impl BuiltinFunction for Apply {
    fn name(&self) -> &str {
        "apply"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let function = expect_closure(self.name(), &args[0])?;
        call_closure(&function, std::slice::from_ref(&args[1]))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct AddOne;

    impl BuiltinFunction for AddOne {
        fn name(&self) -> &str {
            "add_one"
        }

        fn arity(&self) -> usize {
            1
        }

        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match &args[0] {
                Value::I32(n) => Ok(Value::I32(n + 1)),
                actual => Err(format!("AddOne expected I32, got {actual:?}")),
            }
        }
    }

    #[derive(Debug)]
    struct Double;

    impl BuiltinFunction for Double {
        fn name(&self) -> &str {
            "double"
        }

        fn arity(&self) -> usize {
            1
        }

        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match &args[0] {
                Value::I32(n) => Ok(Value::I32(n * 2)),
                actual => Err(format!("Double expected I32, got {actual:?}")),
            }
        }
    }

    #[derive(Debug)]
    struct Subtract;

    impl BuiltinFunction for Subtract {
        fn name(&self) -> &str {
            "subtract"
        }

        fn arity(&self) -> usize {
            2
        }

        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match (&args[0], &args[1]) {
                (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a - b)),
                actual => Err(format!("Subtract expected I32, I32, got {actual:?}")),
            }
        }
    }

    fn test_closure(builtin: Arc<dyn BuiltinFunction>, arity: usize) -> Value {
        closure_value(builtin, arity)
    }

    #[test]
    fn prelude_has_type_names() {
        let names = prelude_type_names();
        assert!(names.contains(&"Option"));
        assert!(names.contains(&"Result"));
    }

    #[test]
    fn prelude_has_all_builtin_names() {
        let builtins = prelude_builtins();
        let names: Vec<_> = builtins.iter().map(|builtin| builtin.name()).collect();

        assert!(names.contains(&"id"));
        assert!(names.contains(&"const"));
        assert!(names.contains(&"flip"));
        assert!(names.contains(&"compose"));
        assert!(names.contains(&"pipe"));
        assert!(names.contains(&"apply"));
        assert!(names.contains(&"map"));
        assert!(names.contains(&"filter"));
        assert!(names.contains(&"fold"));
        assert!(names.contains(&"concat"));
        assert!(names.contains(&"len"));
        assert!(names.contains(&"head"));
        assert!(names.contains(&"tail"));
        assert!(names.contains(&"Str.concat"));
        assert!(names.contains(&"Str.len"));
        assert!(names.contains(&"Str.split"));
        assert!(names.contains(&"Str.trim"));
        assert!(names.contains(&"Str.parse_i32"));
        assert!(names.contains(&"println"));
        assert!(names.contains(&"print"));
        assert!(names.contains(&"read_line"));
        assert!(names.contains(&"readFile"));
        assert!(names.contains(&"flatMap"));
        assert!(names.contains(&"prepend"));
        assert!(names.contains(&"Option.map"));
        assert!(names.contains(&"Option.flatMap"));
        assert!(names.contains(&"Option.unwrapOr"));
        assert!(names.contains(&"Result.map"));
        assert!(names.contains(&"Result.flatMap"));
        assert!(names.contains(&"to_i64"));
        assert!(names.contains(&"to_i32"));
        assert!(names.contains(&"to_f64"));
        assert!(names.contains(&"to_str"));
        assert!(names.contains(&"sqrt"));
        assert!(names.contains(&"drop"));
        assert!(names.contains(&"take"));
        assert!(names.contains(&"unwrap"));
        assert!(names.contains(&"readLine"));
    }

    #[test]
    fn id_returns_same_value() {
        let value = Value::I32(42);
        let result = Id
            .execute(std::slice::from_ref(&value))
            .expect("id should return its argument");

        assert_eq!(result, value);
    }

    #[test]
    fn const_returns_closure_that_returns_captured_value() {
        let constant = Const
            .execute(&[Value::I32(42)])
            .expect("const should return a closure");
        let result = Apply
            .execute(&[constant, Value::Unit])
            .expect("const closure should return captured value");

        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn apply_calls_function() {
        let result = Apply
            .execute(&[test_closure(Arc::new(AddOne), 1), Value::I32(5)])
            .expect("apply should call closures");

        assert_eq!(result, Value::I32(6));
    }

    #[test]
    fn apply_rejects_non_closure() {
        let error = Apply
            .execute(&[Value::Unit, Value::I32(5)])
            .expect_err("apply should reject non-closures");

        assert!(error.contains("expected Closure"));
    }

    #[test]
    fn flip_swaps_arguments() {
        let flipped = Flip
            .execute(&[test_closure(Arc::new(Subtract), 2)])
            .expect("flip should return a closure");
        let closure = expect_closure("flip", &flipped).expect("flip should return a closure");
        let result = call_closure(&closure, &[Value::I32(3), Value::I32(10)])
            .expect("flipped closure should execute");

        assert_eq!(result, Value::I32(7));
    }

    #[test]
    fn compose_applies_second_then_first() {
        let composed = Compose
            .execute(&[
                test_closure(Arc::new(AddOne), 1),
                test_closure(Arc::new(Double), 1),
            ])
            .expect("compose should return a closure");
        let result = Apply
            .execute(&[composed, Value::I32(5)])
            .expect("composed closure should execute");

        assert_eq!(result, Value::I32(11));
    }

    #[test]
    fn pipe_applies_first_then_second() {
        let piped = Pipe
            .execute(&[
                test_closure(Arc::new(AddOne), 1),
                test_closure(Arc::new(Double), 1),
            ])
            .expect("pipe should return a closure");
        let result = Apply
            .execute(&[piped, Value::I32(5)])
            .expect("piped closure should execute");

        assert_eq!(result, Value::I32(12));
    }

    #[test]
    fn flip_rejects_non_closure() {
        let error = Flip
            .execute(&[Value::Unit])
            .expect_err("flip should reject non-closures");

        assert!(error.contains("expected Closure"));
    }

    #[test]
    fn compose_rejects_non_closure_first() {
        let error = Compose
            .execute(&[Value::Unit, test_closure(Arc::new(AddOne), 1)])
            .expect_err("compose should reject non-closure first arg");

        assert!(error.contains("expected Closure"));
    }

    #[test]
    fn compose_rejects_non_closure_second() {
        let error = Compose
            .execute(&[test_closure(Arc::new(AddOne), 1), Value::Unit])
            .expect_err("compose should reject non-closure second arg");

        assert!(error.contains("expected Closure"));
    }

    #[test]
    fn pipe_rejects_non_closure() {
        let error = Pipe
            .execute(&[Value::Unit, test_closure(Arc::new(AddOne), 1)])
            .expect_err("pipe should reject non-closures");

        assert!(error.contains("expected Closure"));
    }
}
