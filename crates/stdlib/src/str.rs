use std::sync::Arc;

use pipe_lang_runtime::{expect_arity, BuiltinFunction, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct StrConcat;

#[derive(Clone, Copy, Debug, Default)]
pub struct StrLen;

#[derive(Clone, Copy, Debug, Default)]
pub struct StrSplit;

impl BuiltinFunction for StrConcat {
    fn name(&self) -> &str {
        "Str.concat"
    }

    fn arity(&self) -> usize {
        2
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let lhs = expect_str(self.name(), &args[0])?;
        let rhs = expect_str(self.name(), &args[1])?;
        Ok(Value::Str(Arc::<str>::from(format!("{lhs}{rhs}"))))
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
            .map_err(|_| "`Str.len` result does not fit into i32".to_owned())?;
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
        let parts = value
            .split(delimiter)
            .map(Value::str)
            .collect::<Vec<_>>();

        Ok(Value::Array(Arc::<[Value]>::from(parts)))
    }
}

fn expect_str<'a>(name: &str, value: &'a Value) -> Result<&'a str, String> {
    match value {
        Value::Str(text) => Ok(text),
        actual => Err(format!("`{name}` expected str, got {actual:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_concat_len_and_split_are_utf8_safe() {
        let joined = StrConcat
            .execute(&[Value::str("pipe"), Value::str("-lang")])
            .unwrap();
        assert_eq!(joined, Value::str("pipe-lang"));

        let hindi = "\u{0928}\u{092e}\u{0938}\u{094d}\u{0924}\u{0947}";
        let len = StrLen.execute(&[Value::str(hindi)]).unwrap();
        assert_eq!(len, Value::I32(hindi.len() as i32));

        let split = StrSplit
            .execute(&[Value::str("a::b::c"), Value::str("::")])
            .unwrap();
        assert_eq!(
            split,
            Value::array(vec![Value::str("a"), Value::str("b"), Value::str("c")])
        );
    }
}