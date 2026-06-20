use runtime::{ClosureData, FuncPtr, Value};

/// Calls a closure with the given arguments.
///
/// Dispatches through [`FuncPtr`] — builtins are called via their
/// [`BuiltinFunction`](runtime::BuiltinFunction) trait, JIT closures
/// use the native calling convention.
pub(crate) fn call_closure(closure: &ClosureData, args: &[Value]) -> Result<Value, String> {
    if args.len() != closure.arity {
        return Err(format!(
            "closure expected {} argument(s), got {}",
            closure.arity,
            args.len()
        ));
    }
    match &closure.func {
        FuncPtr::Builtin(function) => function.execute(args),
        FuncPtr::Jit { .. } => Err("JIT closures not yet supported".to_string()),
    }
}
