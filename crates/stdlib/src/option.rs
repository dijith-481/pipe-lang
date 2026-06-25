use std::sync::Arc;

use runtime::{BuiltinFunction, ClosureData, Value, expect_arity};

use crate::closure::call_closure;

/// `Option.map(option, function)` — applies function to the payload if Some, returns None otherwise.
#[derive(Clone, Copy, Debug, Default)]
pub struct OptionMap;

/// `Option.flat_map(option, function)` — applies function returning Option, flattens one level.
#[derive(Clone, Copy, Debug, Default)]
pub struct OptionFlatMap;

/// `Option.unwrap_or(option, default)` — returns the payload if Some, otherwise the default.
#[derive(Clone, Copy, Debug, Default)]
pub struct OptionUnwrapOr;

/// `unwrap_or(option, default)` — bare name alias for Option.unwrapOr.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnwrapOr;

fn expect_option(name: &str, value: &Value) -> Result<(u32, Arc<[Value]>), String> {
    match value {
        Value::Tag { tag, payload } => Ok((*tag, Arc::clone(payload))),
        actual => Err(format!("`{name}` expected Option (Tag), got {actual:?}")),
    }
}

fn expect_closure(name: &str, value: &Value) -> Result<Arc<ClosureData>, String> {
    match value {
        Value::Closure(closure) => Ok(Arc::clone(closure)),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

impl BuiltinFunction for OptionMap {
    fn name(&self) -> &str {
        "Option.map"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_option(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        match tag {
            0 => Ok(Value::tag(0, vec![])), // None
            1 => {
                if payload.is_empty() {
                    return Err(format!("`{}` Option Some has no payload", self.name()));
                }
                let mapped = call_closure(&closure, &payload)?;
                Ok(Value::tag(1, vec![mapped]))
            }
            other => Err(format!("`{}` unexpected tag {other}", self.name())),
        }
    }
}

impl BuiltinFunction for OptionFlatMap {
    fn name(&self) -> &str {
        "Option.flat_map"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_option(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        match tag {
            0 => Ok(Value::tag(0, vec![])), // None
            1 => {
                if payload.is_empty() {
                    return Err(format!("`{}` Option Some has no payload", self.name()));
                }
                let result = call_closure(&closure, &payload)?;
                // Validate result is an Option
                match &result {
                    Value::Tag { tag: 0..=1, .. } => Ok(result),
                    actual => Err(format!(
                        "`{}` expected closure to return Option, got {actual:?}",
                        self.name()
                    )),
                }
            }
            other => Err(format!("`{}` unexpected tag {other}", self.name())),
        }
    }
}

impl BuiltinFunction for OptionUnwrapOr {
    fn name(&self) -> &str {
        "Option.unwrap_or"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_option(self.name(), &args[0])?;
        match tag {
            0 => Ok(args[1].clone()), // None -> return default
            1 => {
                if payload.is_empty() {
                    return Err(format!("`{}` Option Some has no payload", self.name()));
                }
                Ok(payload[0].clone())
            }
            other => Err(format!("`{}` unexpected tag {other}", self.name())),
        }
    }
}

impl BuiltinFunction for UnwrapOr {
    fn name(&self) -> &str {
        "unwrap_or"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let (tag, payload) = expect_option(self.name(), &args[0])?;
        match tag {
            0 => Ok(args[1].clone()),
            1 => {
                if payload.is_empty() {
                    return Err(format!("`{}` called on Some with no payload", self.name()));
                }
                Ok(payload[0].clone())
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
            call_arg_types: Arc::from([]),
        }))
    }

    fn some(value: Value) -> Value {
        Value::tag(1, vec![value])
    }

    fn none() -> Value {
        Value::tag(0, vec![])
    }

    #[test]
    fn option_map_applies_function_to_some() {
        let result = OptionMap
            .execute(&[some(Value::I32(5)), closure(Arc::new(AddOne), 1)])
            .expect("Option.map should apply to Some");

        assert_eq!(result, some(Value::I32(6)));
    }

    #[test]
    fn option_map_returns_none_for_none() {
        let result = OptionMap
            .execute(&[none(), closure(Arc::new(AddOne), 1)])
            .expect("Option.map should pass through None");

        assert_eq!(result, none());
    }

    #[test]
    fn option_map_rejects_non_tag() {
        let error = OptionMap
            .execute(&[Value::Unit, closure(Arc::new(AddOne), 1)])
            .expect_err("Option.map should reject non-tags");

        assert!(error.contains("expected Option"));
    }

    #[test]
    fn option_flat_map_applies_and_flattens() {
        // Returns Some(x * 2) for even, None for odd
        #[derive(Debug)]
        struct TimesTwoIfEven;
        impl BuiltinFunction for TimesTwoIfEven {
            fn name(&self) -> &str {
                "times_two_if_even"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, args: &[Value]) -> Result<Value, String> {
                match &args[0] {
                    Value::I32(n) if *n % 2 == 0 => Ok(some(Value::I32(n * 2))),
                    Value::I32(_) => Ok(none()),
                    actual => Err(format!("expected I32, got {actual:?}")),
                }
            }
        }
        let result = OptionFlatMap
            .execute(&[some(Value::I32(4)), closure(Arc::new(TimesTwoIfEven), 1)])
            .expect("Option.flatMap should apply");

        assert_eq!(result, some(Value::I32(8)));
    }

    #[test]
    fn option_unwrap_or_returns_payload_for_some() {
        let result = OptionUnwrapOr
            .execute(&[some(Value::I32(42)), Value::I32(0)])
            .expect("unwrapOr should return payload");

        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn option_unwrap_or_returns_default_for_none() {
        let result = OptionUnwrapOr
            .execute(&[none(), Value::I32(99)])
            .expect("unwrapOr should return default for None");

        assert_eq!(result, Value::I32(99));
    }
}
