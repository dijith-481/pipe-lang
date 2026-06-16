use std::io;
use std::sync::Arc;

use pipe_lang_runtime::{expect_arity, BuiltinFunction, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct IoPrintln;

#[derive(Clone, Copy, Debug, Default)]
pub struct IoReadLine;

#[derive(Clone, Debug)]
struct PrintlnEffect {
    message: Arc<str>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ReadLineEffect;

impl BuiltinFunction for IoPrintln {
    fn name(&self) -> &str {
        "io.println"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::Str(message) => Ok(Value::Effect(Arc::new(PrintlnEffect {
                message: Arc::clone(message),
            }))),
            actual => Err(format!("`{}` expected str, got {actual:?}", self.name())),
        }
    }
}

impl BuiltinFunction for IoReadLine {
    fn name(&self) -> &str {
        "io.readLine"
    }

    fn arity(&self) -> usize {
        0
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        Ok(Value::Effect(Arc::new(ReadLineEffect)))
    }
}

impl BuiltinFunction for PrintlnEffect {
    fn name(&self) -> &str {
        "effect.io.println"
    }

    fn arity(&self) -> usize {
        0
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        println!("{}", self.message);
        Ok(Value::Unit)
    }
}

impl BuiltinFunction for ReadLineEffect {
    fn name(&self) -> &str {
        "effect.io.readLine"
    }

    fn arity(&self) -> usize {
        0
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let mut buffer = String::new();
        io::stdin()
            .read_line(&mut buffer)
            .map_err(|err| err.to_string())?;
        Ok(Value::str(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_println_returns_deferred_effect() {
        let effect = IoPrintln.execute(&[Value::str("hello")]).unwrap();
        match effect {
            Value::Effect(effect) => {
                assert_eq!(effect.name(), "effect.io.println");
                assert_eq!(effect.arity(), 0);
            }
            actual => panic!("expected effect, got {actual:?}"),
        }
    }

    #[test]
    fn io_read_line_returns_deferred_effect() {
        let effect = IoReadLine.execute(&[]).unwrap();
        match effect {
            Value::Effect(effect) => {
                assert_eq!(effect.name(), "effect.io.readLine");
                assert_eq!(effect.arity(), 0);
            }
            actual => panic!("expected effect, got {actual:?}"),
        }
    }
}