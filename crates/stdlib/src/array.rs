use std::sync::Arc;

use pipe_lang_runtime::{call_closure, expect_arity, BuiltinFunction, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayFilter;

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayFold;

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayConcat;

impl BuiltinFunction for ArrayMap {
    fn name(&self) -> &str {
        "Array.map"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;

        let mapped = array
            .iter()
            .map(|item| call_closure(closure, std::slice::from_ref(item)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Value::Array(Arc::<[Value]>::from(mapped)))
    }
}

impl BuiltinFunction for ArrayFilter {
    fn name(&self) -> &str {
        "Array.filter"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        let mut filtered = Vec::new();

        for item in array.iter() {
            match call_closure(closure, std::slice::from_ref(item))? {
                Value::Bool(true) => filtered.push(item.clone()),
                Value::Bool(false) => {}
                actual => {
                    return Err(format!(
                        "`{}` predicate must return bool, got {actual:?}",
                        self.name()
                    ));
                }
            }
        }

        Ok(Value::Array(Arc::<[Value]>::from(filtered)))
    }
}

impl BuiltinFunction for ArrayFold {
    fn name(&self) -> &str {
        "Array.fold"
    }

    fn arity(&self) -> usize {
        3
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[2])?;
        let mut accumulator = args[1].clone();

        for item in array.iter() {
            accumulator = call_closure(closure, &[accumulator, item.clone()])?;
        }

        Ok(accumulator)
    }
}

impl BuiltinFunction for ArrayConcat {
    fn name(&self) -> &str {
        "Array.concat"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let lhs = expect_array(self.name(), &args[0])?;
        let rhs = expect_array(self.name(), &args[1])?;

        let mut combined = Vec::with_capacity(lhs.len() + rhs.len());
        combined.extend(lhs.iter().cloned());
        combined.extend(rhs.iter().cloned());

        Ok(Value::Array(Arc::<[Value]>::from(combined)))
    }
}

fn expect_array<'a>(name: &str, value: &'a Value) -> Result<&'a Arc<[Value]>, String> {
    match value {
        Value::Array(array) => Ok(array),
        actual => Err(format!("`{name}` expected array, got {actual:?}")),
    }
}

fn expect_closure<'a>(
    name: &str,
    value: &'a Value,
) -> Result<&'a pipe_lang_runtime::ClosureData, String> {
    match value {
        Value::Closure(closure) => Ok(closure),
        actual => Err(format!("`{name}` expected closure, got {actual:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipe_lang_runtime::ClosureData;

    extern "C" fn add_one(args: *const *const Value, len: usize) -> *mut Value {
        assert_eq!(len, 1);
        let args = unsafe { std::slice::from_raw_parts(args, len) };
        let value = unsafe { &*args[0] };
        Box::into_raw(Box::new(Value::I32(value.as_i32() + 1)))
    }

    extern "C" fn is_even(args: *const *const Value, len: usize) -> *mut Value {
        assert_eq!(len, 1);
        let args = unsafe { std::slice::from_raw_parts(args, len) };
        let value = unsafe { &*args[0] };
        Box::into_raw(Box::new(Value::Bool(value.as_i32() % 2 == 0)))
    }

    #[test]
    fn array_map_applies_closure_and_returns_distinct_allocation() {
        let input: Arc<[Value]> = Arc::from(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
        let closure = Value::Closure(Arc::new(ClosureData::new(
            add_one as usize,
            Arc::<[Value]>::from(Vec::<Value>::new()),
        )));

        let result = ArrayMap
            .execute(&[Value::Array(Arc::clone(&input)), closure])
            .unwrap();

        match result {
            Value::Array(output) => {
                assert!(!Arc::ptr_eq(&input, &output));
                assert_eq!(
                    output.as_ref(),
                    &[Value::I32(2), Value::I32(3), Value::I32(4)]
                );
            }
            actual => panic!("expected array result, got {actual:?}"),
        }
    }

    #[test]
    fn array_filter_keeps_matching_values_without_mutating_input() {
        let input: Arc<[Value]> = Arc::from(vec![Value::I32(1), Value::I32(2), Value::I32(4)]);
        let closure = Value::Closure(Arc::new(ClosureData::new(
            is_even as usize,
            Arc::<[Value]>::from(Vec::<Value>::new()),
        )));

        let result = ArrayFilter
            .execute(&[Value::Array(Arc::clone(&input)), closure])
            .unwrap();

        assert_eq!(input.as_ref(), &[Value::I32(1), Value::I32(2), Value::I32(4)]);
        assert_eq!(result, Value::array(vec![Value::I32(2), Value::I32(4)]));
    }
}