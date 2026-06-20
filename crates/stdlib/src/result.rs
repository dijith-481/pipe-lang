use std::sync::Arc;

use runtime::{BuiltinFunction, ClosureData, Value, expect_arity};

use crate::closure::call_closure;

/// `Result.map(result, function)` — applies function to Ok payload, passes through Err.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResultMap;

/// `Result.flatMap(result, function)` — applies function returning Result, flattens one level.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResultFlatMap;

fn expect_result(name: &str, value: &Value) -> Result<(u32, Arc<[Value]>), String> {
    match value {
        Value::Tag { tag, payload } => Ok((*tag, Arc::clone(payload))),
        actual => Err(format!("`{name}` expected Result (Tag), got {actual:?}")),
    }
}

fn expect_closure(name: &str, value: &Value) -> Result<Arc<ClosureData>, String> {
    match value {
        Value::Closure(closure) => Ok(Arc::clone(closure)),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

impl BuiltinFunction for ResultMap {
    fn name(&self) -> &str {
        "Result.map"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_result(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        match tag {
            0 => {
                // Err — pass through as-is (single payload element: error string)
                Ok(Value::tag(0, payload.to_vec()))
            }
            1 => {
                // Ok — apply function
                if payload.is_empty() {
                    return Err(format!("`{}` Result Ok has no payload", self.name()));
                }
                let mapped = call_closure(&closure, &payload)?;
                Ok(Value::tag(1, vec![mapped]))
            }
            other => Err(format!("`{}` unexpected tag {other}", self.name())),
        }
    }
}

impl BuiltinFunction for ResultFlatMap {
    fn name(&self) -> &str {
        "Result.flatMap"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_result(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        match tag {
            0 => {
                // Err — pass through
                Ok(Value::tag(0, payload.to_vec()))
            }
            1 => {
                // Ok — apply function
                if payload.is_empty() {
                    return Err(format!("`{}` Result Ok has no payload", self.name()));
                }
                let result = call_closure(&closure, &payload)?;
                // Validate result is a Result
                match &result {
                    Value::Tag { tag: 0..=1, .. } => Ok(result),
                    actual => Err(format!(
                        "`{}` expected closure to return Result, got {actual:?}",
                        self.name()
                    )),
                }
            }
            other => Err(format!("`{}` unexpected tag {other}", self.name())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime::FuncPtr;

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
                actual => Err(format!("expected I32, got {actual:?}")),
            }
        }
    }

    fn closure(builtin: Arc<dyn BuiltinFunction>, arity: usize) -> Value {
        Value::Closure(Arc::new(ClosureData {
            func: FuncPtr::Builtin(builtin),
            captures: Arc::from([]),
            arity,
        }))
    }

    fn ok_val(value: Value) -> Value {
        Value::tag(1, vec![value])
    }

    fn err_val(msg: &str) -> Value {
        Value::tag(0, vec![Value::str(msg)])
    }

    #[test]
    fn result_map_applies_function_to_ok() {
        let result = ResultMap
            .execute(&[ok_val(Value::I32(5)), closure(Arc::new(AddOne), 1)])
            .expect("Result.map should apply to Ok");

        assert_eq!(result, ok_val(Value::I32(6)));
    }

    #[test]
    fn result_map_passes_through_err() {
        let input = err_val("oops");
        let result = ResultMap
            .execute(&[input.clone(), closure(Arc::new(AddOne), 1)])
            .expect("Result.map should pass through Err");

        assert_eq!(result, input);
    }

    #[test]
    fn result_map_rejects_non_tag() {
        let error = ResultMap
            .execute(&[Value::Unit, closure(Arc::new(AddOne), 1)])
            .expect_err("Result.map should reject non-tags");

        assert!(error.contains("expected Result"));
    }

    #[test]
    fn result_flat_map_applies_to_ok() {
        #[derive(Debug)]
        struct TryAddOne;
        impl BuiltinFunction for TryAddOne {
            fn name(&self) -> &str {
                "try_add_one"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, args: &[Value]) -> Result<Value, String> {
                match &args[0] {
                    Value::I32(n) => Ok(ok_val(Value::I32(n + 1))),
                    actual => Err(format!("expected I32, got {actual:?}")),
                }
            }
        }
        let result = ResultFlatMap
            .execute(&[ok_val(Value::I32(5)), closure(Arc::new(TryAddOne), 1)])
            .expect("Result.flatMap should apply");

        assert_eq!(result, ok_val(Value::I32(6)));
    }

    #[test]
    fn result_flat_map_passes_through_err() {
        let input = err_val("fail");
        #[derive(Debug)]
        struct NeverCalled;
        impl BuiltinFunction for NeverCalled {
            fn name(&self) -> &str {
                "never"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, _: &[Value]) -> Result<Value, String> {
                panic!("should not be called on Err");
            }
        }
        let result = ResultFlatMap
            .execute(&[input.clone(), closure(Arc::new(NeverCalled), 1)])
            .expect("Result.flatMap should pass through Err");

        assert_eq!(result, input);
    }
}
