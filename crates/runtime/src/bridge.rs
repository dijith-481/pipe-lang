use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::value::Value;

static GLOBAL_REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();

/// A native function exposed to pipe-lang programs.
///
/// Builtins are registered by source-level name and called by the JIT when it
/// lowers an [`ir::Instruction::CallNamed`] instruction.
pub trait BuiltinFunction: Send + Sync + fmt::Debug {
    /// Returns the source-level name used to register this builtin.
    fn name(&self) -> &str;

    /// Returns the number of arguments expected by this builtin.
    fn arity(&self) -> usize;

    /// Executes the builtin with runtime values.
    ///
    /// # Arguments
    ///
    /// * `args` - Runtime arguments supplied by the caller.
    ///
    /// # Returns
    ///
    /// The builtin's runtime result.
    ///
    /// # Errors
    ///
    /// Returns a descriptive string for arity, type, IO, or user-code failures.
    fn execute(&self, args: &[Value]) -> Result<Value, String>;
}

/// Registry of native builtins addressable by source-level name.
#[derive(Clone, Default, Debug)]
pub struct BuiltinRegistry {
    functions: HashMap<String, Arc<dyn BuiltinFunction>>,
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
        self.functions.insert(function.name().to_owned(), function);
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
    pub fn execute(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let function = self
            .get(name)
            .ok_or_else(|| format!("unknown builtin function `{name}`"))?;
        function.execute(args)
    }
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
pub fn expect_arity(name: &str, args: &[Value], expected: usize) -> Result<(), String> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "`{name}` expected {expected} argument(s), got {}",
            args.len()
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Echo;

    impl BuiltinFunction for Echo {
        fn name(&self) -> &str {
            "echo"
        }

        fn arity(&self) -> usize {
            1
        }

        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            expect_arity(self.name(), args, self.arity())?;
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

        assert!(error.contains("unknown builtin function `missing`"));
    }

    #[test]
    fn expect_arity_reports_mismatch() {
        let error =
            expect_arity("echo", &[Value::Unit, Value::Unit], 1).expect_err("arity must fail");

        assert_eq!(error, "`echo` expected 1 argument(s), got 2");
    }
}
