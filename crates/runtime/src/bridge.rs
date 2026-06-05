use std::fmt;

use ast::SmolStr;

use crate::error::RuntimeError;
use crate::value::Value;

/// Trait for built-in functions that the language runtime can execute.
///
/// Implement this trait to expose Rust functions to the language.
/// Each builtin has a name (used for error messages and the registry),
/// a fixed arity, and an execute method that takes values and returns
/// a result.
pub trait BuiltinFunction: fmt::Debug + Send + Sync {
    /// The name of this builtin (e.g., `"List.map"`, `"IO.println"`).
    fn name(&self) -> SmolStr;

    /// The number of arguments this function expects.
    fn arity(&self) -> usize;

    /// Execute the builtin with the given arguments.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] if the execution fails (wrong types,
    /// out of bounds, etc.).
    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct AddBuiltin;

    impl BuiltinFunction for AddBuiltin {
        fn name(&self) -> SmolStr {
            SmolStr::new("Int.add")
        }

        fn arity(&self) -> usize {
            2
        }

        fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
            match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                _ => Err(RuntimeError::TypeMismatch {
                    expected: "Int".into(),
                    got: format!("{:?}", &args[0]),
                }),
            }
        }
    }

    #[derive(Debug)]
    struct PrintlnBuiltin;

    impl BuiltinFunction for PrintlnBuiltin {
        fn name(&self) -> SmolStr {
            SmolStr::new("IO.println")
        }

        fn arity(&self) -> usize {
            1
        }

        fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
            let msg = args[0].as_str().ok_or_else(|| RuntimeError::TypeMismatch {
                expected: "Str".into(),
                got: format!("{:?}", &args[0]),
            })?;
            Ok(Value::Str(SmolStr::new(msg)))
        }
    }

    #[test]
    fn builtin_execute_returns_value() {
        let builtin = AddBuiltin;
        let args = vec![Value::Int(3), Value::Int(4)];
        let result = builtin.execute(&args).expect("should succeed");
        assert_eq!(result.as_int(), Some(7));
    }

    #[test]
    fn builtin_wrong_type_errors() {
        let builtin = AddBuiltin;
        let args = vec![Value::Int(1), Value::Str(SmolStr::new("two"))];
        let err = builtin.execute(&args).unwrap_err();
        assert!(matches!(err, RuntimeError::TypeMismatch { .. }));
    }

    #[test]
    fn builtin_name_and_arity() {
        let builtin = PrintlnBuiltin;
        assert_eq!(builtin.name().as_str(), "IO.println");
        assert_eq!(builtin.arity(), 1);
    }

    #[test]
    fn builtin_returns_unit_for_io() {
        let builtin = PrintlnBuiltin;
        let args = vec![Value::Str(SmolStr::new("hello"))];
        let result = builtin.execute(&args).expect("should succeed");
        assert_eq!(result.as_str(), Some("hello"));
    }
}
