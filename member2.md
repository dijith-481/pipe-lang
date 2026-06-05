# Member 2: Standard Library & Effects (Week 1 Deliverables)

**Crate Ownership:** `crates/stdlib`
**Mission:** Build the functional core of the standard library — list operations, Option/Result handling, and the IO effect shell. All functions are pure Rust `BuiltinFunction` implementations exposed to the language via the runtime.

## Architecture Overview

### Your API Contract (already implemented)

```rust
// crates/runtime/src/bridge.rs
pub trait BuiltinFunction: fmt::Debug + Send + Sync {
    fn name(&self) -> SmolStr;
    fn arity(&self) -> usize;
    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError>;
}
```

### Value types you'll work with (already implemented)

```rust
// crates/runtime/src/value.rs
pub enum Value {
    I8(i8), I16(i16), I32(i32), I64(i64),
    U8(u8), U16(u16), U32(u32), U64(u64), Usize(usize),
    F32(f32), F64(f64),
    Bool(bool), Str(SmolStr),
    Array(Arc<[Value]>),
    Record(Arc<RecordData>),
    Closure(Arc<ClosureData>),
    Tag { tag: u32, payload: Arc<[Value]> },
    Effect(Arc<dyn BuiltinFunction>),
    Unit,
}
```

### Tag conventions for Option/Result

```rust
// Option: tag 0 = None, tag 1 = Some
Value::tag(0, vec![])                    // None
Value::tag(1, vec![inner_value])         // Some(inner)

// Result: tag 0 = Err, tag 1 = Ok
Value::tag(0, vec![error_value])         // Err(e)
Value::tag(1, vec![ok_value])            // Ok(v)
```

### Helper constructors (already on Value)

```rust
Value::array(vec![...])                  // Array from Vec
Value::tag(tag_id, vec![...])            // Tag with payload
Value::record(vec![("field", val), ...]) // Record with fields
```

### Current stdlib crate

```rust
// crates/stdlib/src/lib.rs — currently only:
pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }
```

### Tests Already Passing

- `runtime/value.rs`: 26 tests (all Value types, equality, display, helpers)
- `runtime/bridge.rs`: 4 tests (BuiltinFunction trait examples)
- `runtime/error.rs`: 7 tests (RuntimeError variants)

## Week 1 Deliverables & Timeline

### Days 1-2: List Operations (The Functional Core)

**Goal:** Implement all list builtins as structs implementing `BuiltinFunction`.

**File:** `crates/stdlib/src/list.rs`

**Task 1: `ListLen`**
```rust
#[derive(Debug)]
pub struct ListLen;

impl BuiltinFunction for ListLen {
    fn name(&self) -> SmolStr { SmolStr::new("List.len") }
    fn arity(&self) -> usize { 1 }
    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        let arr = args[0].as_array().ok_or_else(|| RuntimeError::TypeMismatch {
            expected: "Array".into(),
            got: format!("{:?}", &args[0]),
        })?;
        Ok(Value::Usize(arr.len()))
    }
}
```

**Task 2: `ListHead`**
```rust
// Returns Option: Tag(1, [first]) or Tag(0, [])
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = args[0].as_array().ok_or(...)?;
    match arr.first() {
        Some(v) => Ok(Value::tag(1, vec![v.clone()])),
        None => Ok(Value::tag(0, vec![])),
    }
}
```

**Task 3: `ListTail`**
```rust
// Returns Option: Tag(1, [rest]) or Tag(0, []) if empty/single
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = args[0].as_array().ok_or(...)?;
    if arr.len() <= 1 {
        Ok(Value::tag(0, vec![]))
    } else {
        Ok(Value::tag(1, vec![Value::array(arr[1..].to_vec())]))
    }
}
```

**Task 4: `ListMap`**
```rust
// Takes (array, closure). Returns new array with closure applied to each element.
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = args[0].as_array().ok_or(...)?;
    let closure = match &args[1] {
        Value::Closure(c) => c,
        _ => return Err(RuntimeError::TypeMismatch { expected: "Closure".into(), got: format!("{:?}", &args[1]) }),
    };
    // For each element, call the closure
    let results: Result<Vec<Value>, _> = arr.iter()
        .map(|elem| call_closure(closure, &[elem.clone()]))
        .collect();
    Ok(Value::array(results?))
}
```

**Task 5: `ListFilter`**
```rust
// Takes (array, predicate_closure). Returns elements where predicate returns true.
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    // Similar to map, but check if closure returns Value::Bool(true)
}
```

**Task 6: `ListFold`**
```rust
// Takes (array, initial, reducer_closure). Folds left.
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = args[0].as_array().ok_or(...)?;
    let mut acc = args[1].clone();
    let closure = match &args[2] { Value::Closure(c) => c, ... };
    for elem in arr {
        acc = call_closure(closure, &[acc, elem.clone()])?;
    }
    Ok(acc)
}
```

**TDD approach:** Write all 6 tests before implementation:
- `list_len_empty` — `List.len([])` returns `0usize`
- `list_len_three` — `List.len([1,2,3])` returns `3usize`
- `list_head_some` — `List.head([1,2])` returns `Some(1)`
- `list_head_none` — `List.head([])` returns `None`
- `list_tail_some` — `List.tail([1,2,3])` returns `Some([2,3])`
- `list_tail_single` — `List.tail([1])` returns `None`
- `list_map_double` — `List.map([1,2,3], (x) => x * 2)` returns `[2,4,6]`
- `list_filter_even` — `List.filter([1,2,3,4], (x) => x % 2 == 0)` returns `[2,4]`
- `list_fold_sum` — `List.fold([1,2,3], 0, (acc, x) => acc + x)` returns `6`

### Days 3-4: Option & Result Handling

**File:** `crates/stdlib/src/option.rs`, `crates/stdlib/src/result.rs`

**Task 7: `OptionMap`**
```rust
// Takes (option, closure). If Some(v), applies closure. If None, returns None.
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let (tag, payload) = args[0].as_tag().ok_or(...)?;
    match tag {
        0 => Ok(Value::tag(0, vec![])),  // None -> None
        1 => {
            let closure = match &args[1] { Value::Closure(c) => c, ... };
            let result = call_closure(closure, &[payload[0].clone()])?;
            Ok(Value::tag(1, vec![result]))
        }
        _ => Err(RuntimeError::TypeMismatch { expected: "Option".into(), ... }),
    }
}
```

**Task 8: `OptionUnwrap`**
```rust
// Takes (option, default). Returns inner value or default.
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let (tag, payload) = args[0].as_tag().ok_or(...)?;
    match tag {
        0 => Ok(args[1].clone()),  // None -> default
        1 => Ok(payload[0].clone()),  // Some(v) -> v
        _ => Err(...),
    }
}
```

**Task 9: `OptionIsSome` / `OptionIsNone`**
```rust
// Simple tag checks
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let (tag, _) = args[0].as_tag().ok_or(...)?;
    Ok(Value::Bool(tag == 1))  // is_some
}
```

**Task 10: `ResultMap` / `ResultFlatMap`**
```rust
// Result.map: if Ok(v), applies closure. If Err(e), returns Err(e).
// Result.flatMap: like map but closure returns Result directly (no wrapping).
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let (tag, payload) = args[0].as_tag().ok_or(...)?;
    match tag {
        0 => Ok(Value::tag(0, vec![payload[0].clone()])),  // Err -> Err
        1 => {
            let closure = match &args[1] { Value::Closure(c) => c, ... };
            let result = call_closure(closure, &[payload[0].clone()])?;
            Ok(Value::tag(1, vec![result]))  // Ok -> Ok(f(v))
        }
        _ => Err(...),
    }
}
```

**TDD approach:**
- `option_map_some` — `Option.map(Some(5), (x) => x * 2)` returns `Some(10)`
- `option_map_none` — `Option.map(None, (x) => x * 2)` returns `None`
- `option_unwrap_some` — `Option.unwrap(Some(5), 0)` returns `5`
- `option_unwrap_none` — `Option.unwrap(None, 0)` returns `0`
- `option_is_some_true` — `Option.isSome(Some(5))` returns `true`
- `option_is_some_false` — `Option.isSome(None)` returns `false`
- `result_map_ok` — `Result.map(Ok(5), (x) => x + 1)` returns `Ok(6)`
- `result_map_err` — `Result.map(Err("e"), (x) => x + 1)` returns `Err("e")`
- `result_flatmap_chain` — chaining `Result.flatMap` on `Ok` values

### Days 5-7: The IO API Shell

**Goal:** Implement IO builtins that return `Value::Effect` wrappers (deferred execution).

**File:** `crates/stdlib/src/io.rs`

**Task 11: `IOPrint`**
```rust
#[derive(Debug)]
pub struct IOPrint;

impl BuiltinFunction for IOPrint {
    fn name(&self) -> SmolStr { SmolStr::new("IO.print") }
    fn arity(&self) -> usize { 1 }
    fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
        // Validate input is a string
        let msg = args[0].as_str().ok_or_else(|| RuntimeError::TypeMismatch {
            expected: "Str".into(),
            got: format!("{:?}", &args[0]),
        })?;
        // Return an Effect wrapping the print intent
        // The runtime will execute this effect later
        Ok(Value::Effect(Arc::new(PrintEffect {
            msg: SmolStr::new(msg),
            newline: false,
        })))
    }
}

#[derive(Debug)]
struct PrintEffect {
    msg: SmolStr,
    newline: bool,
}

impl BuiltinFunction for PrintEffect {
    fn name(&self) -> SmolStr { SmolStr::new("IO.print.effect") }
    fn arity(&self) -> usize { 0 }
    fn execute(&self, _args: &[Value]) -> Result<Value, RuntimeError> {
        if self.newline {
            println!("{}", self.msg);
        } else {
            print!("{}", self.msg);
        }
        Ok(Value::Unit)
    }
}
```

**Task 12: `IOPrintln`**
```rust
// Same as IOPrint but with newline: true
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    let msg = args[0].as_str().ok_or(...)?;
    Ok(Value::Effect(Arc::new(PrintEffect {
        msg: SmolStr::new(msg),
        newline: true,
    })))
}
```

**Task 13: `IOReadLine`**
```rust
// Returns an Effect that, when executed, reads a line from stdin
fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Effect(Arc::new(ReadLineEffect)))
}

#[derive(Debug)]
struct ReadLineEffect;

impl BuiltinFunction for ReadLineEffect {
    fn name(&self) -> SmolStr { SmolStr::new("IO.readLine.effect") }
    fn arity(&self) -> usize { 0 }
    fn execute(&self, _args: &[Value]) -> Result<Value, RuntimeError> {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map_err(|e| RuntimeError::EffectError {
            msg: format!("failed to read line: {e}"),
        })?;
        Ok(Value::Str(SmolStr::new(input.trim_end())))
    }
}
```

**Task 14: Registry module**
```rust
// crates/stdlib/src/lib.rs
pub mod io;
pub mod list;
pub mod option;
pub mod result;

use std::sync::Arc;
use runtime::bridge::BuiltinFunction;

/// Returns all standard library builtins as (name, Arc<dyn BuiltinFunction>) pairs.
pub fn builtins() -> Vec<(SmolStr, Arc<dyn BuiltinFunction>)> {
    vec![
        ("List.len".into(), Arc::new(list::ListLen)),
        ("List.head".into(), Arc::new(list::ListHead)),
        ("List.tail".into(), Arc::new(list::ListTail)),
        ("List.map".into(), Arc::new(list::ListMap)),
        ("List.filter".into(), Arc::new(list::ListFilter)),
        ("List.fold".into(), Arc::new(list::ListFold)),
        ("Option.map".into(), Arc::new(option::OptionMap)),
        ("Option.unwrap".into(), Arc::new(option::OptionUnwrap)),
        ("Option.isSome".into(), Arc::new(option::OptionIsSome)),
        ("Option.isNone".into(), Arc::new(option::OptionIsNone)),
        ("Result.map".into(), Arc::new(result::ResultMap)),
        ("Result.flatMap".into(), Arc::new(result::ResultFlatMap)),
        ("IO.print".into(), Arc::new(io::IOPrint)),
        ("IO.println".into(), Arc::new(io::IOPrintln)),
        ("IO.readLine".into(), Arc::new(io::IOReadLine)),
    ]
}
```

**TDD approach:**
- `io_print_returns_effect` — `IO.print("hello")` returns `Value::Effect`, not `Unit`
- `io_println_returns_effect` — same for `IO.println`
- `io_print_effect_executes` — when the effect is executed, it produces `Unit`
- `io_readline_returns_effect` — `IO.readLine()` returns an effect
- `io_print_wrong_type` — `IO.print(42)` returns `RuntimeError::TypeMismatch`
- `registry_has_all_builtins` — `builtins()` returns 15 entries

## Common Pitfalls

1. **Don't execute IO directly** — always return `Value::Effect` wrapper
2. **Clone values in closures** — `Arc` makes this cheap, but be explicit
3. **Tag IDs must be consistent** — Option::None is always tag 0, Some is always tag 1
4. **Return `RuntimeError::TypeMismatch`** for wrong argument types, not panics

## Dependencies

- `runtime` crate: `Value`, `BuiltinFunction`, `RuntimeError`
- `ast` crate: `SmolStr` (re-exported)
- `thiserror`: already in Cargo.toml (add to stdlib's Cargo.toml)
