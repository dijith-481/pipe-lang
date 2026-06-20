use runtime::{BuiltinFunction, Value, expect_arity};

/// `I32.to_i64(value)` — widens I32 to I64.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToI64;

/// `F64.to_i32(value)` — truncates F64 to I32.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToI32;

/// `I32.to_f64(value)` — converts I32 to F64.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToF64;

/// `I32.to_str(value)` — formats I32 as a string.
/// Available on all primitives.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToStr;

impl BuiltinFunction for ToI64 {
    fn name(&self) -> &str {
        "to_i64"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::I32(n) => Ok(Value::I64(*n as i64)),
            actual => Err(format!("`{}` expected I32, got {actual:?}", self.name())),
        }
    }
}

impl BuiltinFunction for ToI32 {
    fn name(&self) -> &str {
        "to_i32"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::F64(n) => Ok(Value::I32(*n as i32)),
            actual => Err(format!("`{}` expected F64, got {actual:?}", self.name())),
        }
    }
}

impl BuiltinFunction for ToF64 {
    fn name(&self) -> &str {
        "to_f64"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::I32(n) => Ok(Value::F64(*n as f64)),
            actual => Err(format!("`{}` expected I32, got {actual:?}", self.name())),
        }
    }
}

impl BuiltinFunction for ToStr {
    fn name(&self) -> &str {
        "to_str"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::I32(n) => Ok(Value::str(n.to_string())),
            Value::I64(n) => Ok(Value::str(n.to_string())),
            Value::F64(n) => Ok(Value::str(n.to_string())),
            Value::Bool(b) => Ok(Value::str(b.to_string())),
            Value::Str(s) => Ok(Value::str(s.to_string())),
            actual => Err(format!(
                "`{}` expected primitive, got {actual:?}",
                self.name()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_i64_widens_i32() {
        let result = ToI64
            .execute(&[Value::I32(42)])
            .expect("to_i64 should work");
        assert_eq!(result, Value::I64(42));
    }

    #[test]
    fn to_i64_rejects_non_i32() {
        let error = ToI64
            .execute(&[Value::Unit])
            .expect_err("to_i64 should reject non-I32");
        assert!(error.contains("expected I32"));
    }

    #[test]
    fn to_i32_truncates_f64() {
        let result = ToI32
            .execute(&[Value::F64(3.9)])
            .expect("to_i32 should truncate");
        assert_eq!(result, Value::I32(3));
    }

    #[test]
    fn to_i32_rejects_non_f64() {
        let error = ToI32
            .execute(&[Value::Unit])
            .expect_err("to_i32 should reject non-F64");
        assert!(error.contains("expected F64"));
    }

    #[test]
    fn to_f64_converts_i32() {
        let result = ToF64.execute(&[Value::I32(5)]).expect("to_f64 should work");
        assert_eq!(result, Value::F64(5.0));
    }

    #[test]
    fn to_f64_rejects_non_i32() {
        let error = ToF64
            .execute(&[Value::Unit])
            .expect_err("to_f64 should reject non-I32");
        assert!(error.contains("expected I32"));
    }

    #[test]
    fn to_str_formats_i32() {
        let result = ToStr
            .execute(&[Value::I32(42)])
            .expect("to_str should format I32");
        assert_eq!(result, Value::str("42"));
    }

    #[test]
    fn to_str_formats_f64() {
        let result = ToStr
            .execute(&[Value::F64(3.5)])
            .expect("to_str should format F64");
        assert_eq!(result, Value::str("3.5"));
    }

    #[test]
    fn to_str_formats_bool() {
        let result = ToStr
            .execute(&[Value::Bool(true)])
            .expect("to_str should format bool");
        assert_eq!(result, Value::str("true"));
    }

    #[test]
    fn to_str_passes_through_str() {
        let result = ToStr
            .execute(&[Value::str("hello")])
            .expect("to_str should pass through Str");
        assert_eq!(result, Value::str("hello"));
    }

    #[test]
    fn to_str_rejects_unit() {
        let error = ToStr
            .execute(&[Value::Unit])
            .expect_err("to_str should reject Unit");
        assert!(error.contains("expected primitive"));
    }
}
