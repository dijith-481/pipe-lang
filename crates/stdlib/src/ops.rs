use runtime::{BuiltinFunction, Value, expect_arity};

/// `unwrap(value, default)` — extracts the payload from a tagged value.
///
/// If the tag is 1 (Some/Ok), returns `payload[0]`.
/// If the tag is 0 (None/Err), returns `default`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unwrap;

impl BuiltinFunction for Unwrap {
    fn name(&self) -> &str {
        "unwrap"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::Tag { tag, payload } => match tag {
                0 => Ok(args[1].clone()),
                1 => {
                    if payload.is_empty() {
                        return Err(format!(
                            "`{}` expected tag with payload, got empty",
                            self.name()
                        ));
                    }
                    Ok(payload[0].clone())
                }
                other => Err(format!(
                    "`{}` expected Option or Result tag (0 or 1), got {other}",
                    self.name()
                )),
            },
            actual => Err(format!(
                "`{}` expected Option/Result (Tag), got {actual:?}",
                self.name()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_returns_inner_for_some() {
        let result = Unwrap
            .execute(&[Value::tag(1, vec![Value::I32(42)]), Value::I32(0)])
            .expect("unwrap should return value from Some");

        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn unwrap_returns_default_for_none() {
        let result = Unwrap
            .execute(&[Value::tag(0, vec![]), Value::I32(99)])
            .expect("unwrap should return default for None");

        assert_eq!(result, Value::I32(99));
    }

    #[test]
    fn unwrap_rejects_non_tag() {
        let error = Unwrap
            .execute(&[Value::Unit, Value::I32(0)])
            .expect_err("unwrap should reject non-tag");

        assert!(error.contains("expected Option/Result"));
    }

    #[test]
    fn unwrap_returns_inner_for_ok() {
        let result = Unwrap
            .execute(&[
                Value::tag(1, vec![Value::str("success")]),
                Value::str("default"),
            ])
            .expect("unwrap should return value from Ok");

        assert_eq!(result, Value::str("success"));
    }

    #[test]
    fn unwrap_returns_default_for_err() {
        let result = Unwrap
            .execute(&[
                Value::tag(0, vec![Value::str("error")]),
                Value::str("fallback"),
            ])
            .expect("unwrap should return default for Err");

        assert_eq!(result, Value::str("fallback"));
    }
}
