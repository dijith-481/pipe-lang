use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::value::{ClosureData, Value};

pub trait BuiltinFunction: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
    fn arity(&self) -> usize;
    fn execute(&self, args: &[Value]) -> Result<Value, String>;
}

#[derive(Clone, Default, Debug)]
pub struct BuiltinRegistry {
    functions: BTreeMap<String, Arc<dyn BuiltinFunction>>,
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, function: Arc<dyn BuiltinFunction>) {
        self.functions.insert(function.name().to_owned(), function);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn BuiltinFunction>> {
        self.functions.get(name).cloned()
    }

    pub fn execute(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let function = self
            .get(name)
            .ok_or_else(|| format!("unknown builtin function `{name}`"))?;
        function.execute(args)
    }
}

pub fn expect_arity(name: &str, args: &[Value], expected: usize) -> Result<(), String> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "`{name}` expected {expected} argument(s), got {}",
            args.len()
        ))
    }
}

pub fn call_closure(closure: &ClosureData, args: &[Value]) -> Result<Value, String> {
    closure.call(args)
}