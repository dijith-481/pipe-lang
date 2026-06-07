# Member 2 — Runtime & Standard Library Architect

**Crate Ownership:** `crates/runtime`, `crates/stdlib`
**Mission:** You own the memory model and the standard library. `pipe-lang` is a purely functional language with no Garbage Collector. You will implement the memory-safe, Atomic Reference Counted (ARC) core and write the built-in functions that give the language its utility.

You do not need to understand ASTs, parsing, or Cranelift JIT compilation. You are strictly building a robust, high-performance Rust library that acts as the execution foundation.

## 1. The Memory Model (`crates/runtime/src/value.rs`)

Because the language is immutable, circular references are impossible. You will define the global `Value` enum. This is the exact memory layout used by the JIT and the interpreter.

### 1.1 The `Value` Enum
Define the enum using `#[repr(C)]` so it has a predictable memory layout for the JIT.

```rust
use std::sync::Arc;
use std::collections::BTreeMap;

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Value {
    I32(i32),
    I64(i64),
    F64(f64),
    Bool(bool),
    Unit,
    
    // Heap Allocated Types (ARC tracked)
    Str(Arc<str>),
    Array(Arc<[Value]>),
    Record(Arc<BTreeMap<String, Value>>),
    Closure(Arc<ClosureData>),
    
    // Sum Types
    Tag { tag: u32, payload: Arc<[Value]> },
    
    // Effects
    Effect(Arc<dyn BuiltinFunction>),
}

pub struct ClosureData {
    pub func_ptr: usize, // Address to JIT compiled function
    pub captures: Arc<[Value]>,
}
```

### 1.2 `Value` Mechanics
Implement traits for `Value`.
*   `PartialEq`: Implement deep equality checks (required for the language's `==` operator).
*   `Drop`: `Arc` handles this natively, but verify that deeply nested arrays are dropped cleanly without overflowing the Rust stack.

## 2. The FFI Bridge (`crates/runtime/src/bridge.rs`)

Define the trait that allows the JIT to call your Rust standard library functions.

```rust
pub trait BuiltinFunction: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn arity(&self) -> usize;
    fn execute(&self, args: &[Value]) -> Result<Value, String>;
}
```

## 3. The Standard Library (`crates/stdlib`)

You will write the native implementations of the language's standard library. Each function is a struct implementing `BuiltinFunction`.

### 3.1 Arrays (`crates/stdlib/src/array.rs`)
Implement immutable array manipulation. **Never mutate the input.** Always return a newly allocated `Value::Array(Arc<...>)`.
*   `ArrayMap`: Takes an array and a `Value::Closure`. Iterates over the array, invoking the closure (via the bridge), and returns a new array.
*   `ArrayFilter`: Returns a new array containing only elements where the closure evaluates to `Value::Bool(true)`.
*   `ArrayFold`: Standard left-fold.
*   `ArrayConcat`: Allocates a new array combining two input arrays.

### 3.2 Strings (`crates/stdlib/src/str.rs`)
Implement UTF-8 safe string functions.
*   `StrConcat`: Joins two `Value::Str`.
*   `StrLen`: Returns a `Value::I32` of the byte length.
*   `StrSplit`: Splits by a delimiter and returns a `Value::Array`.

### 3.3 Effects & IO (`crates/stdlib/src/io.rs`)
The language separates pure logic from IO. Your IO functions do not execute immediately; they return a `Value::Effect`.

*   `IoPrintln`: Takes a string, returns a `Value::Effect` wrapping a closure that actually calls `println!()` when the runtime executes it.
*   `IoReadLine`: Returns an `Effect` wrapping `std::io::stdin().read_line()`.

## 4. Testing Requirements
*   Instantiate `Value`s and test equality deeply.
*   Test that `ArrayMap` correctly applies a mocked Rust closure to every element and returns a distinct `Arc` allocation.
*   Ensure zero memory leaks using `valgrind` or standard Rust drop-check tests on cyclical-avoidance constraints.
