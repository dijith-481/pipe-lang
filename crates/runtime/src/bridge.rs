use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use ast::SmolStr;

use crate::error::RuntimeError;
use crate::value::Value;

static GLOBAL_REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();

/// Trait for built-in functions that the language runtime can execute.
///
/// Implement this trait to expose Rust functions to the language.
/// Each builtin has a name (used for error messages and the registry),
/// a fixed arity, and and an execute method that takes values and returns
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

/// Registry of native builtins addressable by source-level name.
#[derive(Clone, Default, Debug)]
pub struct BuiltinRegistry {
    functions: HashMap<SmolStr, Arc<dyn BuiltinFunction>>,
}

impl BuiltinRegistry {
    /// Creates an empty builtin registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a builtin, replacing any existing builtin with the same name.
    ///
    /// # Arguments
    ///
    /// * `function` - The builtin implementation to register.
    pub fn register(&mut self, function: Arc<dyn BuiltinFunction>) {
        self.functions.insert(function.name(), function);
    }

    /// Looks up a builtin by source-level name.
    ///
    /// # Arguments
    ///
    /// * `name` - The builtin name emitted by lowering.
    ///
    /// # Returns
    ///
    /// A cloned [`Arc`] to the builtin implementation, if present.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn BuiltinFunction>> {
        self.functions.get(name).cloned()
    }

    /// Executes a builtin by source-level name.
    ///
    /// # Arguments
    ///
    /// * `name` - The builtin name emitted by lowering.
    /// * `args` - Runtime arguments supplied by the caller.
    ///
    /// # Returns
    ///
    /// The builtin's runtime result.
    ///
    /// # Errors
    ///
    /// Returns an error when the builtin is unknown or execution fails.
    pub fn execute(&self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let function = self.get(name).ok_or_else(|| RuntimeError::EffectError {
            msg: format!("unknown builtin function `{name}`"),
        })?;
        function.execute(args)
    }
}

/// Initializes the process-wide builtin registry.
///
/// # Arguments
///
/// * `registry` - The complete builtin registry for JIT name resolution.
///
/// # Panics
///
/// Panics if the global registry was already initialized.
pub fn init_global_registry(registry: BuiltinRegistry) {
    GLOBAL_REGISTRY
        .set(registry)
        .expect("global registry already initialized");
}

/// Returns the process-wide builtin registry.
///
/// # Panics
///
/// Panics when [`init_global_registry`] has not been called yet.
#[must_use]
pub fn global_registry() -> &'static BuiltinRegistry {
    GLOBAL_REGISTRY
        .get()
        .expect("global registry not initialized - call init_global_registry() first")
}

/// Checks that a builtin received the expected number of arguments.
///
/// # Arguments
///
/// * `name` - The builtin's source-level name.
/// * `args` - Runtime arguments supplied by the caller.
/// * `expected` - Required argument count.
///
/// # Errors
///
/// Returns an error when `args.len()` does not match `expected`.
pub fn expect_arity(_name: &str, args: &[Value], expected: usize) -> Result<(), RuntimeError> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(RuntimeError::ArityMismatch {
            expected,
            got: args.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Echo;

    impl BuiltinFunction for Echo {
        fn name(&self) -> SmolStr {
            SmolStr::new("echo")
        }

        fn arity(&self) -> usize {
            1
        }

        fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
            Ok(args[0].clone())
        }
    }

    #[test]
    fn registry_executes_registered_builtin() {
        let mut registry = BuiltinRegistry::new();
        registry.register(Arc::new(Echo));

        let result = registry
            .execute("echo", &[Value::I32(42)])
            .expect("registered builtin should execute");

        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn registry_errors_for_unknown_builtin() {
        let registry = BuiltinRegistry::new();

        let error = registry
            .execute("missing", &[])
            .expect_err("unknown builtin should error");

        assert!(matches!(error, RuntimeError::EffectError { .. }));
        assert!(
            format!("{error:?}").contains("unknown builtin function `missing`"),
            "error should mention 'missing': {error:?}"
        );
    }

    #[test]
    fn builtin_execute_returns_value() {
        let builtin = AddBuiltin;
        let args = vec![Value::I32(3), Value::I32(4)];
        let result = builtin.execute(&args).expect("should succeed");
        assert_eq!(result.as_i32(), Some(7));
    }

    #[test]
    fn builtin_wrong_type_errors() {
        let builtin = AddBuiltin;
        let args = vec![Value::I32(1), Value::Str(SmolStr::new("two"))];
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

    #[test]
    fn expect_arity_reports_mismatch() {
        let error =
            expect_arity("echo", &[Value::Unit, Value::Unit], 1).expect_err("arity must fail");

        assert!(matches!(error, RuntimeError::ArityMismatch { .. }));
    }

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
                (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a + b)),
                _ => Err(RuntimeError::TypeMismatch {
                    expected: "I32".into(),
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
}
