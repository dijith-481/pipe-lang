use runtime::{BuiltinFunction, Value, expect_arity};

/// String concatenation builtin: `Str.concat(left, right)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrConcat;

/// String byte-length builtin: `Str.len(value)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrLen;

/// String split builtin: `Str.split(value, delimiter)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrSplit;

/// String trim builtin: `Str.trim(value)`.
/// Removes leading and trailing whitespace.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrTrim;

/// String parse_i32 builtin: `Str.parse_i32(value)`.
/// Parses a string as an I32, returning `Ok(i32)` or `Err(error)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrParseI32;

impl BuiltinFunction for StrConcat {
    fn name(&self) -> &str {
        "Str.concat"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let left = expect_str(self.name(), &args[0])?;
        let right = expect_str(self.name(), &args[1])?;
        let capacity = left
            .len()
            .checked_add(right.len())
            .ok_or_else(|| format!("`{}` string length overflow", self.name()))?;
        let mut combined = String::with_capacity(capacity);
        combined.push_str(left);
        combined.push_str(right);
        Ok(Value::str(combined))
    }
}

impl BuiltinFunction for StrLen {
    fn name(&self) -> &str {
        "Str.len"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let value = expect_str(self.name(), &args[0])?;
        let len = i32::try_from(value.len())
            .map_err(|_| format!("`{}` length does not fit in I32", self.name()))?;
        Ok(Value::I32(len))
    }
}

impl BuiltinFunction for StrSplit {
    fn name(&self) -> &str {
        "Str.split"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let value = expect_str(self.name(), &args[0])?;
        let delimiter = expect_str(self.name(), &args[1])?;
        Ok(Value::array(
            value.split(delimiter).map(Value::str).collect::<Vec<_>>(),
        ))
    }
}

impl BuiltinFunction for StrTrim {
    fn name(&self) -> &str {
        "Str.trim"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let value = expect_str(self.name(), &args[0])?;
        Ok(Value::str(value.trim()))
    }
}

impl BuiltinFunction for StrParseI32 {
    fn name(&self) -> &str {
        "Str.parse_i32"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let value = expect_str(self.name(), &args[0])?;
        match value.trim().parse::<i32>() {
            Ok(n) => Ok(Value::tag(1, vec![Value::I32(n)])), // Ok(n)
            Err(e) => Ok(Value::tag(0, vec![Value::str(e.to_string())])), // Err(msg)
        }
    }
}

fn expect_str<'a>(name: &str, value: &'a Value) -> Result<&'a str, String> {
    match value {
        Value::Str(text) => Ok(text),
        actual => Err(format!("`{name}` expected Str, got {actual:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_combines_strings() {
        let result = StrConcat
            .execute(&[Value::str("hello"), Value::str(" world")])
            .expect("concat should combine strings");

        assert_eq!(result, Value::str("hello world"));
    }

    #[test]
    fn concat_handles_empty_string() {
        let result = StrConcat
            .execute(&[Value::str(""), Value::str("tail")])
            .expect("concat should handle empty strings");

        assert_eq!(result, Value::str("tail"));
    }

    #[test]
    fn concat_rejects_non_string() {
        let error = StrConcat
            .execute(&[Value::str("hello"), Value::Unit])
            .expect_err("concat should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn len_returns_byte_length() {
        let result = StrLen
            .execute(&[Value::str("hello")])
            .expect("len should return byte length");

        assert_eq!(result, Value::I32(5));
    }

    #[test]
    fn len_returns_zero_for_empty_string() {
        let result = StrLen
            .execute(&[Value::str("")])
            .expect("len should handle empty strings");

        assert_eq!(result, Value::I32(0));
    }

    #[test]
    fn len_rejects_non_string() {
        let error = StrLen
            .execute(&[Value::Unit])
            .expect_err("len should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn split_returns_string_array() {
        let result = StrSplit
            .execute(&[Value::str("a,b,c"), Value::str(",")])
            .expect("split should return parts");

        assert_eq!(
            result,
            Value::array(vec![Value::str("a"), Value::str("b"), Value::str("c")])
        );
    }

    #[test]
    fn split_returns_original_when_delimiter_is_absent() {
        let result = StrSplit
            .execute(&[Value::str("abc"), Value::str(",")])
            .expect("split should return original string when delimiter is absent");

        assert_eq!(result, Value::array(vec![Value::str("abc")]));
    }

    #[test]
    fn split_rejects_non_string() {
        let error = StrSplit
            .execute(&[Value::str("abc"), Value::Unit])
            .expect_err("split should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn trim_removes_leading_and_trailing_whitespace() {
        let result = StrTrim
            .execute(&[Value::str("  hello  ")])
            .expect("trim should remove whitespace");

        assert_eq!(result, Value::str("hello"));
    }

    #[test]
    fn trim_returns_empty_for_whitespace_only() {
        let result = StrTrim
            .execute(&[Value::str("   ")])
            .expect("trim should handle whitespace-only strings");

        assert_eq!(result, Value::str(""));
    }

    #[test]
    fn trim_returns_same_for_already_trimmed() {
        let result = StrTrim
            .execute(&[Value::str("hello")])
            .expect("trim should pass through clean strings");

        assert_eq!(result, Value::str("hello"));
    }

    #[test]
    fn trim_rejects_non_string() {
        let error = StrTrim
            .execute(&[Value::Unit])
            .expect_err("trim should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn parse_i32_returns_ok_for_valid_input() {
        let result = StrParseI32
            .execute(&[Value::str("42")])
            .expect("parse_i32 should succeed");

        assert_eq!(result, Value::tag(1, vec![Value::I32(42)]));
    }

    #[test]
    fn parse_i32_returns_ok_for_negative() {
        let result = StrParseI32
            .execute(&[Value::str("-10")])
            .expect("parse_i32 should handle negative");

        assert_eq!(result, Value::tag(1, vec![Value::I32(-10)]));
    }

    #[test]
    fn parse_i32_returns_err_for_invalid() {
        let result = StrParseI32
            .execute(&[Value::str("not_a_num")])
            .expect("parse_i32 should handle invalid input");

        match result {
            Value::Tag { tag: 0, .. } => {} // Err variant
            other => panic!("expected Err tag, got {other:?}"),
        }
    }

    #[test]
    fn parse_i32_rejects_non_string() {
        let error = StrParseI32
            .execute(&[Value::Unit])
            .expect_err("parse_i32 should reject non-strings");

        assert!(error.contains("expected Str"));
    }
}
