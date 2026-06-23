use std::io;

use runtime::{BuiltinFunction, Value, expect_arity, write_stdout};

/// Prints a string followed by a newline: `println(value)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct IoPrintln;

/// Prints a string without a trailing newline: `print(value)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct IoPrint;

/// Reads one line from standard input: `read_line(_module)`.
/// The module argument is ignored.
#[derive(Clone, Copy, Debug, Default)]
pub struct IoReadLine;

/// Reads an entire file into a string: `read_file(path)`.
/// Returns `Ok(content)` on success, `Err(error)` on failure.
#[derive(Clone, Copy, Debug, Default)]
pub struct IoReadFile;

impl BuiltinFunction for IoPrintln {
    fn name(&self) -> &str {
        "println"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let message = expect_str(self.name(), &args[0])?;
        let output = format!("{message}\n");
        write_stdout(&output);
        Ok(Value::Unit)
    }
}

impl BuiltinFunction for IoPrint {
    fn name(&self) -> &str {
        "print"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let message = expect_str(self.name(), &args[0])?;
        write_stdout(message);
        Ok(Value::Unit)
    }
}

impl BuiltinFunction for IoReadLine {
    fn name(&self) -> &str {
        "read_line"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let mut buffer = String::new();
        io::stdin()
            .read_line(&mut buffer)
            .map_err(|e| e.to_string())?;
        Ok(Value::str(buffer))
    }
}

impl BuiltinFunction for IoReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn arity(&self) -> usize {
        1
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let path = expect_str(self.name(), &args[0])?;
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(Value::tag(1, vec![Value::str(content)])), // Ok(content)
            Err(e) => Ok(Value::tag(0, vec![Value::str(e.to_string())])), // Err(error)
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
    fn println_returns_unit_for_string() {
        let result = IoPrintln
            .execute(&[Value::str("hello")])
            .expect("println should return unit");

        assert_eq!(result, Value::Unit);
    }

    #[test]
    fn println_rejects_non_string() {
        let error = IoPrintln
            .execute(&[Value::Unit])
            .expect_err("println should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn print_returns_unit_for_string() {
        let result = IoPrint
            .execute(&[Value::str("")])
            .expect("print should return unit");

        assert_eq!(result, Value::Unit);
    }

    #[test]
    fn print_rejects_non_string() {
        let error = IoPrint
            .execute(&[Value::Unit])
            .expect_err("print should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn read_line_accepts_module_arg() {
        let result = IoReadLine
            .execute(&[Value::Unit])
            .expect("readLine should accept module arg");
        assert!(result.as_str().is_some());
    }

    #[test]
    fn read_line_rejects_wrong_arity() {
        let error = IoReadLine
            .execute(&[])
            .expect_err("readLine should reject no args");
        assert!(error.contains("expected 1 argument"));
    }

    #[test]
    fn read_file_rejects_non_string() {
        let error = IoReadFile
            .execute(&[Value::Unit])
            .expect_err("readFile should reject non-strings");

        assert!(error.contains("expected Str"));
    }

    #[test]
    fn read_file_returns_err_for_nonexistent() {
        let result = IoReadFile
            .execute(&[Value::str("/nonexistent/path")])
            .expect("readFile should handle missing files");

        match result {
            Value::Tag { tag: 0, .. } => {} // Err variant
            _ => panic!("expected Err tag for missing file"),
        }
    }
}
