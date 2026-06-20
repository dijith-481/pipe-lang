use std::sync::Arc;

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

/// Array flatMap builtin: `flatMap(array, function)`.
///
/// Applies the closure to each element and flattens the resulting arrays.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayFlatMap;

/// Array prepend builtin: `prepend(array, value)`.
///
/// Returns a new array with the value prepended at the front.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayPrepend;

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
            mapped.push(call_closure(&closure, std::slice::from_ref(value))?);
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
            match call_closure(&closure, std::slice::from_ref(value))? {
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
            accumulator = call_closure(&closure, &call_args)?;
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

impl BuiltinFunction for ArrayFlatMap {
    fn name(&self) -> &str {
        "flatMap"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let closure = expect_closure(self.name(), &args[1])?;
        let mut results = Vec::new();
        for value in array {
            let mapped = call_closure(&closure, std::slice::from_ref(value))?;
            match mapped {
                Value::Array(elements) => results.extend(elements.iter().cloned()),
                actual => {
                    return Err(format!(
                        "`{}` expected closure to return Array, got {actual:?}",
                        self.name()
                    ));
                }
            }
        }
        Ok(Value::array(results))
    }
}

impl BuiltinFunction for ArrayPrepend {
    fn name(&self) -> &str {
        "prepend"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let array = expect_array(self.name(), &args[0])?;
        let capacity = array
            .len()
            .checked_add(1)
            .ok_or_else(|| format!("`{}` array length overflow", self.name()))?;
        let mut values = Vec::with_capacity(capacity);
        values.push(args[1].clone());
        values.extend_from_slice(array);
        Ok(Value::array(values))
    }
}

fn expect_array<'a>(name: &str, value: &'a Value) -> Result<&'a [Value], String> {
    match value {
        Value::Array(values) => Ok(values),
        actual => Err(format!("`{name}` expected Array, got {actual:?}")),
    }
}

fn expect_closure(name: &str, value: &Value) -> Result<Arc<ClosureData>, String> {
    match value {
        Value::Closure(closure) => Ok(Arc::clone(closure)),
        actual => Err(format!("`{name}` expected Closure, got {actual:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
                actual => Err(format!("AddOne expected I32, got {actual:?}")),
            }
        }
    }

    #[derive(Debug)]
    struct IsEven;

    impl BuiltinFunction for IsEven {
        fn name(&self) -> &str {
            "is_even"
        }
        fn arity(&self) -> usize {
            1
        }
        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match &args[0] {
                Value::I32(n) => Ok(Value::Bool(*n % 2 == 0)),
                actual => Err(format!("IsEven expected I32, got {actual:?}")),
            }
        }
    }

    #[derive(Debug)]
    struct ReturnI32;

    impl BuiltinFunction for ReturnI32 {
        fn name(&self) -> &str {
            "return_i32"
        }
        fn arity(&self) -> usize {
            1
        }
        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match &args[0] {
                Value::I32(_) => Ok(args[0].clone()),
                actual => Err(format!("ReturnI32 expected I32, got {actual:?}")),
            }
        }
    }

    #[derive(Debug)]
    struct Sum;

    impl BuiltinFunction for Sum {
        fn name(&self) -> &str {
            "sum"
        }
        fn arity(&self) -> usize {
            2
        }
        fn execute(&self, args: &[Value]) -> Result<Value, String> {
            match (&args[0], &args[1]) {
                (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a + b)),
                actual => Err(format!("Sum expected I32, I32, got {actual:?}")),
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

    fn int_array(values: &[i32]) -> Value {
        Value::array(values.iter().copied().map(Value::I32).collect())
    }

    #[test]
    fn map_transforms_each_value() {
        let result = ArrayMap
            .execute(&[int_array(&[1, 2, 3]), closure(Arc::new(AddOne), 1)])
            .expect("map should transform values");

        assert_eq!(result, int_array(&[2, 3, 4]));
    }

    #[test]
    fn map_rejects_non_array() {
        let error = ArrayMap
            .execute(&[Value::Unit, closure(Arc::new(AddOne), 1)])
            .expect_err("map should reject non-arrays");

        assert!(error.contains("expected Array"));
    }

    #[test]
    fn filter_keeps_matching_values() {
        let result = ArrayFilter
            .execute(&[int_array(&[1, 2, 3, 4]), closure(Arc::new(IsEven), 1)])
            .expect("filter should keep even values");

        assert_eq!(result, int_array(&[2, 4]));
    }

    #[test]
    fn filter_rejects_non_bool_predicate_result() {
        let error = ArrayFilter
            .execute(&[int_array(&[1]), closure(Arc::new(ReturnI32), 1)])
            .expect_err("filter should require bool predicate results");

        assert!(error.contains("expected predicate to return Bool"));
    }

    #[test]
    fn fold_accumulates_values() {
        let result = ArrayFold
            .execute(&[
                int_array(&[1, 2, 3]),
                Value::I32(0),
                closure(Arc::new(Sum), 2),
            ])
            .expect("fold should accumulate values");

        assert_eq!(result, Value::I32(6));
    }

    #[test]
    fn fold_returns_initial_for_empty_array() {
        let result = ArrayFold
            .execute(&[int_array(&[]), Value::I32(9), closure(Arc::new(Sum), 2)])
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

    #[test]
    fn flat_map_flattens_mapped_arrays() {
        // Duplicate each element: flatMap([1, 2, 3], (x) => [x, x])
        #[derive(Debug)]
        struct Duplicate;
        impl BuiltinFunction for Duplicate {
            fn name(&self) -> &str {
                "duplicate"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, args: &[Value]) -> Result<Value, String> {
                match &args[0] {
                    Value::I32(n) => Ok(Value::array(vec![Value::I32(*n), Value::I32(*n)])),
                    actual => Err(format!("expected I32, got {actual:?}")),
                }
            }
        }
        let result = ArrayFlatMap
            .execute(&[int_array(&[1, 2, 3]), closure(Arc::new(Duplicate), 1)])
            .expect("flatMap should flatten mapped arrays");

        assert_eq!(result, int_array(&[1, 1, 2, 2, 3, 3]));
    }

    #[test]
    fn flat_map_returns_empty_for_empty_array() {
        #[derive(Debug)]
        struct Identity;
        impl BuiltinFunction for Identity {
            fn name(&self) -> &str {
                "id"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, args: &[Value]) -> Result<Value, String> {
                Ok(Value::array(vec![args[0].clone()]))
            }
        }
        let result = ArrayFlatMap
            .execute(&[int_array(&[]), closure(Arc::new(Identity), 1)])
            .expect("flatMap on empty returns empty");

        assert_eq!(result, Value::array(vec![]));
    }

    #[test]
    fn flat_map_rejects_non_array_result() {
        #[derive(Debug)]
        struct ReturnI32;
        impl BuiltinFunction for ReturnI32 {
            fn name(&self) -> &str {
                "return_i32"
            }
            fn arity(&self) -> usize {
                1
            }
            fn execute(&self, args: &[Value]) -> Result<Value, String> {
                Ok(args[0].clone())
            }
        }
        let error = ArrayFlatMap
            .execute(&[int_array(&[1]), closure(Arc::new(ReturnI32), 1)])
            .expect_err("flatMap should reject non-array results");

        assert!(error.contains("expected closure to return Array"));
    }

    #[test]
    fn prepend_adds_element_to_front() {
        let result = ArrayPrepend
            .execute(&[int_array(&[2, 3]), Value::I32(1)])
            .expect("prepend should add element to front");

        assert_eq!(result, int_array(&[1, 2, 3]));
    }

    #[test]
    fn prepend_works_on_empty_array() {
        let result = ArrayPrepend
            .execute(&[int_array(&[]), Value::I32(42)])
            .expect("prepend on empty should work");

        assert_eq!(result, int_array(&[42]));
    }

    #[test]
    fn prepend_rejects_non_array() {
        let error = ArrayPrepend
            .execute(&[Value::Unit, Value::I32(1)])
            .expect_err("prepend should reject non-arrays");

        assert!(error.contains("expected Array"));
    }
}
