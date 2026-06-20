use std::sync::Arc;

use runtime::{BuiltinFunction, ClosureData, Value, expect_arity};

use crate::closure::{ClosureThunk, call_closure};
use crate::{array, io, str as string};

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
        Arc::new(Id),
        Arc::new(Const),
        Arc::new(Flip),
        Arc::new(Compose),
        Arc::new(Pipe),
        Arc::new(Apply),
        Arc::new(array::ArrayMap),
        Arc::new(array::ArrayFilter),
        Arc::new(array::ArrayFold),
        Arc::new(array::ArrayConcat),
        Arc::new(array::ArrayLen),
        Arc::new(array::ArrayHead),
        Arc::new(array::ArrayTail),
        Arc::new(string::StrConcat),
        Arc::new(string::StrLen),
        Arc::new(string::StrSplit),
        Arc::new(io::IoPrintln),
        Arc::new(io::IoPrint),
        Arc::new(io::IoReadLine),
    ]
}

// ---------------------------------------------------------------------------
// Builtin implementations
// ---------------------------------------------------------------------------

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
        Ok(closure_value(const_inner, vec![args[0].clone()], 1))
    }
}

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
        expect_closure(self.name(), &args[0])?;
        Ok(closure_value(flip_inner, vec![args[0].clone()], 2))
    }
}

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
        expect_closure(self.name(), &args[0])?;
        expect_closure(self.name(), &args[1])?;
        Ok(closure_value(
            compose_inner,
            vec![args[0].clone(), args[1].clone()],
            1,
        ))
    }
}

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
        expect_closure(self.name(), &args[0])?;
        expect_closure(self.name(), &args[1])?;
        Ok(closure_value(
            pipe_inner,
            vec![args[0].clone(), args[1].clone()],
            1,
        ))
    }
}

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
        call_closure(function, std::slice::from_ref(&args[1]))
    }
}

fn expect_closure<'a>(name: &str, value: &'a Value) -> Result<&'a ClosureData, String> {
    match value {
        Value::Closure(closure) => Ok(closure),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

fn closure_value(function: ClosureThunk, captures: Vec<Value>, arity: usize) -> Value {
    Value::Closure(Arc::new(ClosureData {
        func_ptr: function as usize,
        captures: Arc::from(captures.into_boxed_slice()),
        arity,
    }))
}

unsafe extern "C" fn const_inner(args: *const u8, ret: *mut u8) -> i32 {
    let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 2) };
    unsafe {
        std::ptr::write(ret.cast::<Value>(), values[0].clone());
    }
    0
}

unsafe extern "C" fn flip_inner(args: *const u8, ret: *mut u8) -> i32 {
    let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 3) };
    let Value::Closure(function) = &values[0] else {
        return 1;
    };
    let swapped = [values[2].clone(), values[1].clone()];
    write_call_result(function, &swapped, ret)
}

unsafe extern "C" fn compose_inner(args: *const u8, ret: *mut u8) -> i32 {
    let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 3) };
    let (Value::Closure(first), Value::Closure(second)) = (&values[0], &values[1]) else {
        return 1;
    };
    let second_result = match call_closure(second, std::slice::from_ref(&values[2])) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    write_call_result(first, &[second_result], ret)
}

unsafe extern "C" fn pipe_inner(args: *const u8, ret: *mut u8) -> i32 {
    let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 3) };
    let (Value::Closure(first), Value::Closure(second)) = (&values[0], &values[1]) else {
        return 1;
    };
    let first_result = match call_closure(first, std::slice::from_ref(&values[2])) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    write_call_result(second, &[first_result], ret)
}

fn write_call_result(closure: &ClosureData, args: &[Value], ret: *mut u8) -> i32 {
    match call_closure(closure, args) {
        Ok(value) => {
            unsafe {
                std::ptr::write(ret.cast::<Value>(), value);
            }
            0
        }
        Err(_) => 1,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn add_one(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 1) };
        match &values[0] {
            Value::I32(value) => unsafe {
                std::ptr::write(ret.cast::<Value>(), Value::I32(*value + 1));
                0
            },
            _ => 1,
        }
    }

    unsafe extern "C" fn double(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 1) };
        match &values[0] {
            Value::I32(value) => unsafe {
                std::ptr::write(ret.cast::<Value>(), Value::I32(*value * 2));
                0
            },
            _ => 1,
        }
    }

    unsafe extern "C" fn subtract(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 2) };
        match (&values[0], &values[1]) {
            (Value::I32(left), Value::I32(right)) => unsafe {
                std::ptr::write(ret.cast::<Value>(), Value::I32(*left - *right));
                0
            },
            _ => 1,
        }
    }

    fn test_closure(function: ClosureThunk, arity: usize) -> Value {
        closure_value(function, Vec::new(), arity)
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
        assert!(names.contains(&"println"));
        assert!(names.contains(&"print"));
        assert!(names.contains(&"read_line"));
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
            .execute(&[test_closure(add_one, 1), Value::I32(5)])
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
            .execute(&[test_closure(subtract, 2)])
            .expect("flip should return a closure");
        let result = call_closure(
            expect_closure("test", &flipped).expect("flip should produce a closure"),
            &[Value::I32(3), Value::I32(10)],
        )
        .expect("flipped closure should execute");

        assert_eq!(result, Value::I32(7));
    }

    #[test]
    fn compose_applies_second_then_first() {
        let composed = Compose
            .execute(&[test_closure(add_one, 1), test_closure(double, 1)])
            .expect("compose should return a closure");
        let result = Apply
            .execute(&[composed, Value::I32(5)])
            .expect("composed closure should execute");

        assert_eq!(result, Value::I32(11));
    }

    #[test]
    fn pipe_applies_first_then_second() {
        let piped = Pipe
            .execute(&[test_closure(add_one, 1), test_closure(double, 1)])
            .expect("pipe should return a closure");
        let result = Apply
            .execute(&[piped, Value::I32(5)])
            .expect("piped closure should execute");

        assert_eq!(result, Value::I32(12));
    }
}
