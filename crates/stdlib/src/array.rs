use runtime::{BuiltinFunction, ClosureData, Value, expect_arity};

use crate::closure::call_closure;

/// Array mapping builtin: `map(array, function)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayMap;

/// Array filtering builtin: `filter(array, predicate)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayFilter;

/// Array folding builtin: `fold(array, initial, function)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayFold;

/// Array concatenation builtin: `concat(left, right)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayConcat;

/// Array length builtin: `len(array)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayLen;

/// Array head builtin: `head(array)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayHead;

/// Array tail builtin: `tail(array)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayTail;

impl BuiltinFunction for ArrayMap {
    fn name(&self) -> &str {
        "map"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        let mut mapped = Vec::with_capacity(array.len());
        for value in array {
            mapped.push(call_closure(closure, std::slice::from_ref(value))?);
        }
        Ok(Value::array(mapped))
    }
}

impl BuiltinFunction for ArrayFilter {
    fn name(&self) -> &str {
        "filter"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        let mut filtered = Vec::with_capacity(array.len());
        for value in array {
            match call_closure(closure, std::slice::from_ref(value))? {
                Value::Bool(true) => filtered.push(value.clone()),
                Value::Bool(false) => {}
                actual => {
                    return Err(format!(
                        "`{}` expected predicate to return Bool, got {actual:?}",
                        self.name()
                    ));
                }
            }
        }
        Ok(Value::array(filtered))
    }
}

impl BuiltinFunction for ArrayFold {
    fn name(&self) -> &str {
        "fold"
    }

    fn arity(&self) -> usize {
        3
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[2])?;
        let mut accumulator = args[1].clone();
        for value in array {
            let call_args = [accumulator, value.clone()];
            accumulator = call_closure(closure, &call_args)?;
        }
        Ok(accumulator)
    }
}

impl BuiltinFunction for ArrayConcat {
    fn name(&self) -> &str {
        "concat"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let left = expect_array(self.name(), &args[0])?;
        let right = expect_array(self.name(), &args[1])?;
        let capacity = left
            .len()
            .checked_add(right.len())
            .ok_or_else(|| format!("`{}` array length overflow", self.name()))?;
        let mut values = Vec::with_capacity(capacity);
        values.extend_from_slice(left);
        values.extend_from_slice(right);
        Ok(Value::array(values))
    }
}

impl BuiltinFunction for ArrayLen {
    fn name(&self) -> &str {
        "len"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let len = i32::try_from(array.len())
            .map_err(|_| format!("`{}` length does not fit in I32", self.name()))?;
        Ok(Value::I32(len))
    }
}

impl BuiltinFunction for ArrayHead {
    fn name(&self) -> &str {
        "head"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        match array.first() {
            Some(value) => Ok(Value::tag(1, vec![value.clone()])),
            None => Ok(Value::tag(0, vec![])),
        }
    }
}

impl BuiltinFunction for ArrayTail {
    fn name(&self) -> &str {
        "tail"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        if array.len() <= 1 {
            Ok(Value::tag(0, vec![]))
        } else {
            Ok(Value::tag(1, vec![Value::array(array[1..].to_vec())]))
        }
    }
}

fn expect_array<'a>(name: &str, value: &'a Value) -> Result<&'a [Value], String> {
    match value {
        Value::Array(values) => Ok(values),
        actual => Err(format!("`{name}` expected Array, got {actual:?}")),
    }
}

fn expect_closure<'a>(name: &str, value: &'a Value) -> Result<&'a ClosureData, String> {
    match value {
        Value::Closure(closure) => Ok(closure),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::closure::ClosureThunk;

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

    unsafe extern "C" fn is_even(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 1) };
        match &values[0] {
            Value::I32(value) => unsafe {
                std::ptr::write(ret.cast::<Value>(), Value::Bool(*value % 2 == 0));
                0
            },
            _ => 1,
        }
    }

    unsafe extern "C" fn return_i32(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 1) };
        match &values[0] {
            Value::I32(_) => unsafe {
                std::ptr::write(ret.cast::<Value>(), values[0].clone());
                0
            },
            _ => 1,
        }
    }

    unsafe extern "C" fn sum(args: *const u8, ret: *mut u8) -> i32 {
        let values = unsafe { std::slice::from_raw_parts(args.cast::<Value>(), 2) };
        match (&values[0], &values[1]) {
            (Value::I32(left), Value::I32(right)) => unsafe {
                std::ptr::write(ret.cast::<Value>(), Value::I32(*left + *right));
                0
            },
            _ => 1,
        }
    }

    fn closure(function: ClosureThunk, arity: usize) -> Value {
        Value::Closure(Arc::new(ClosureData {
            func_ptr: function as usize,
            captures: Arc::from([]),
            arity,
        }))
    }

    fn int_array(values: &[i32]) -> Value {
        Value::array(values.iter().copied().map(Value::I32).collect())
    }

    #[test]
    fn map_transforms_each_value() {
        let result = ArrayMap
            .execute(&[int_array(&[1, 2, 3]), closure(add_one, 1)])
            .expect("map should transform values");

        assert_eq!(result, int_array(&[2, 3, 4]));
    }

    #[test]
    fn map_rejects_non_array() {
        let error = ArrayMap
            .execute(&[Value::Unit, closure(add_one, 1)])
            .expect_err("map should reject non-arrays");

        assert!(error.contains("expected Array"));
    }

    #[test]
    fn filter_keeps_matching_values() {
        let result = ArrayFilter
            .execute(&[int_array(&[1, 2, 3, 4]), closure(is_even, 1)])
            .expect("filter should keep even values");

        assert_eq!(result, int_array(&[2, 4]));
    }

    #[test]
    fn filter_rejects_non_bool_predicate_result() {
        let error = ArrayFilter
            .execute(&[int_array(&[1]), closure(return_i32, 1)])
            .expect_err("filter should require bool predicate results");

        assert!(error.contains("expected predicate to return Bool"));
    }

    #[test]
    fn fold_accumulates_values() {
        let result = ArrayFold
            .execute(&[int_array(&[1, 2, 3]), Value::I32(0), closure(sum, 2)])
            .expect("fold should accumulate values");

        assert_eq!(result, Value::I32(6));
    }

    #[test]
    fn fold_returns_initial_for_empty_array() {
        let result = ArrayFold
            .execute(&[int_array(&[]), Value::I32(9), closure(sum, 2)])
            .expect("fold should return initial value for empty arrays");

        assert_eq!(result, Value::I32(9));
    }

    #[test]
    fn concat_combines_arrays() {
        let result = ArrayConcat
            .execute(&[int_array(&[1, 2]), int_array(&[3])])
            .expect("concat should combine arrays");

        assert_eq!(result, int_array(&[1, 2, 3]));
    }

    #[test]
    fn concat_rejects_non_array() {
        let error = ArrayConcat
            .execute(&[int_array(&[]), Value::Unit])
            .expect_err("concat should reject non-arrays");

        assert!(error.contains("expected Array"));
    }

    #[test]
    fn len_returns_i32_length() {
        let result = ArrayLen
            .execute(&[int_array(&[1, 2, 3])])
            .expect("len should return length");

        assert_eq!(result, Value::I32(3));
    }

    #[test]
    fn len_rejects_non_array() {
        let error = ArrayLen
            .execute(&[Value::Unit])
            .expect_err("len should reject non-arrays");

        assert!(error.contains("expected Array"));
    }

    #[test]
    fn head_returns_some_first_value() {
        let result = ArrayHead
            .execute(&[int_array(&[7, 8])])
            .expect("head should return first value");

        assert_eq!(result, Value::tag(1, vec![Value::I32(7)]));
    }

    #[test]
    fn head_returns_none_for_empty_array() {
        let result = ArrayHead
            .execute(&[int_array(&[])])
            .expect("head should return none for empty arrays");

        assert_eq!(result, Value::tag(0, vec![]));
    }

    #[test]
    fn tail_returns_some_rest_for_multi_element_array() {
        let result = ArrayTail
            .execute(&[int_array(&[7, 8, 9])])
            .expect("tail should return rest");

        assert_eq!(result, Value::tag(1, vec![int_array(&[8, 9])]));
    }

    #[test]
    fn tail_returns_none_for_single_element_array() {
        let result = ArrayTail
            .execute(&[int_array(&[7])])
            .expect("tail should return none for single-element arrays");

        assert_eq!(result, Value::tag(0, vec![]));
    }
}
