use runtime::{BuiltinFunction, Value, expect_arity};

/// `to_i64(value)` — converts any numeric type to I64.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToI64;

/// `to_i32(value)` — converts any numeric type to I32 (truncating for floats).
#[derive(Clone, Copy, Debug, Default)]
pub struct ToI32;

/// `to_f64(value)` — converts any numeric type to F64.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToF64;

/// `to_str(value)` — formats any primitive as a string.
#[derive(Clone, Copy, Debug, Default)]
pub struct ToStr;

/// `sqrt(value)` — computes the square root of an F64.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sqrt;

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
            Value::I64(n) => Ok(Value::I64(*n)),
            Value::Usize(n) => Ok(Value::I64(*n as i64)),
            Value::F64(n) => Ok(Value::I64(*n as i64)),
            Value::Bool(b) => Ok(Value::I64(*b as i64)),
            actual => Err(format!(
                "`{}` expected numeric, got {actual:?}",
                self.name()
            )),
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
            Value::I32(n) => Ok(Value::I32(*n)),
            Value::I64(n) => Ok(Value::I32(*n as i32)),
            Value::Usize(n) => Ok(Value::I32(*n as i32)),
            Value::F64(n) => Ok(Value::I32(*n as i32)),
            Value::Bool(b) => Ok(Value::I32(*b as i32)),
            actual => Err(format!(
                "`{}` expected numeric, got {actual:?}",
                self.name()
            )),
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
            Value::I64(n) => Ok(Value::F64(*n as f64)),
            Value::Usize(n) => Ok(Value::F64(*n as f64)),
            Value::F64(n) => Ok(Value::F64(*n)),
            Value::Bool(b) => Ok(Value::F64(*b as i32 as f64)),
            actual => Err(format!(
                "`{}` expected numeric, got {actual:?}",
                self.name()
            )),
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
            Value::Usize(n) => Ok(Value::str(n.to_string())),
            Value::F64(n) => Ok(Value::str(n.to_string())),
            Value::Bool(b) => Ok(Value::str(b.to_string())),
            Value::Str(s) => Ok(Value::str(s.to_string())),
            Value::Array(a) => {
                let parts: Vec<String> = a.iter().map(|v| v.to_string()).collect();
                Ok(Value::str(format!("[{}]", parts.join(", "))))
            }
            actual => Err(format!(
                "`{}` expected primitive, got {actual:?}",
                self.name()
            )),
        }
    }
}

impl BuiltinFunction for Sqrt {
    fn name(&self) -> &str {
        "sqrt"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::F64(n) => Ok(Value::F64(n.sqrt())),
            actual => Err(format!("`{}` expected F64, got {actual:?}", self.name())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_i64_widens_i32() {
        assert_eq!(ToI64.execute(&[Value::I32(42)]).unwrap(), Value::I64(42));
    }

    #[test]
    fn to_i64_converts_f64() {
        assert_eq!(ToI64.execute(&[Value::F64(3.9)]).unwrap(), Value::I64(3));
    }

    #[test]
    fn to_i64_converts_bool() {
        assert_eq!(ToI64.execute(&[Value::Bool(true)]).unwrap(), Value::I64(1));
    }

    #[test]
    fn to_i32_truncates_f64() {
        assert_eq!(ToI32.execute(&[Value::F64(3.9)]).unwrap(), Value::I32(3));
    }

    #[test]
    fn to_i32_converts_i64() {
        assert_eq!(ToI32.execute(&[Value::I64(100)]).unwrap(), Value::I32(100));
    }

    #[test]
    fn to_i32_converts_bool() {
        assert_eq!(ToI32.execute(&[Value::Bool(false)]).unwrap(), Value::I32(0));
    }

    #[test]
    fn to_f64_converts_i32() {
        assert_eq!(ToF64.execute(&[Value::I32(5)]).unwrap(), Value::F64(5.0));
    }

    #[test]
    fn to_f64_converts_i64() {
        assert_eq!(
            ToF64.execute(&[Value::I64(100)]).unwrap(),
            Value::F64(100.0)
        );
    }

    #[test]
    fn to_str_formats_i32() {
        assert_eq!(ToStr.execute(&[Value::I32(42)]).unwrap(), Value::str("42"));
    }

    #[test]
    fn to_str_formats_f64() {
        assert_eq!(
            ToStr.execute(&[Value::F64(3.5)]).unwrap(),
            Value::str("3.5")
        );
    }

    #[test]
    fn to_str_formats_bool() {
        assert_eq!(
            ToStr.execute(&[Value::Bool(true)]).unwrap(),
            Value::str("true")
        );
    }

    #[test]
    fn to_str_passes_through_str() {
        assert_eq!(
            ToStr.execute(&[Value::str("hello")]).unwrap(),
            Value::str("hello")
        );
    }

    #[test]
    fn to_str_rejects_unit() {
        assert!(ToStr.execute(&[Value::Unit]).is_err());
    }

    #[test]
    fn sqrt_returns_square_root() {
        let result = Sqrt
            .execute(&[Value::F64(9.0)])
            .expect("sqrt should compute square root");
        assert_eq!(result, Value::F64(3.0));
    }

    #[test]
    fn sqrt_rejects_non_f64() {
        let error = Sqrt
            .execute(&[Value::I32(4)])
            .expect_err("sqrt should reject non-F64");
        assert!(error.contains("expected F64"));
    }
}
