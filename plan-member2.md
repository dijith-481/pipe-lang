# Member 2 — Runtime & Standard Library Architect

**Crate Ownership:** `crates/runtime`, `crates/stdlib`

## Current State (Post-Member-3 Typechecker + Lower)

### What's Already Working
- Typechecker prelude (`env.rs:92`) loads types for: `Option`, `Result`, `Some`, `None`, `Ok`, `Err`, `id`, `const`, `flip`, `compose`, `pipe`, `apply`
- Lowerer (`lower.rs`) emits `Instruction::CallNamed { name, args }` for every function call — names are **source-level identifiers** (e.g. `map`, `println`, `filter`)
- JIT skeleton (`jit.rs`) handles `ConstI32 + Return` only — multi-block, calls, heap types are unimplemented (Member 1's work)
- CLI pipeline (`session.rs`) is wired: `lex → parse → typecheck → lower → JIT`
- 333 tests pass across all crates
- `cargo fmt` and `cargo clippy --workspace -- -D warnings` are clean

### What Member 2 Needs to Build

You own the memory model (`Value`), the FFI bridge (`BuiltinFunction`), the builtin registry, and ALL stdlib functions. Your output is consumed by:
1. The **lowerer** (already done — emits `CallNamed` names matching your registry keys)
2. The **JIT** (Member 1) — expects `Value` to be `#[repr(C)]` and functions to be `extern "C" fn(...) -> i32`

You do NOT need to touch lexer, parser, AST, typechecker, IR, CLI, or diagnostics.

---

## Step 1 — Rewrite `crates/runtime/src/value.rs`

**Problem:** Current `Value` has 14 numeric types (I8/I16/I32/I64/U8/U16/U32/U64/Usize/F32/F64 + Bool/Str/...). This is too many for the JIT to handle. The JIT plan expects exactly these runtime types.

**Goal:** Simplified `#[repr(C)]` Value with only the types the type system actually uses.

### What to Keep

| Type | Notes |
|---|---|
| `I32` | Primary integer type — most constants/arithmetic resolve to this |
| `I64` | Used when explicitly sized or for large values |
| `F64` | The only float type the lowerer ever emits |
| `Bool` | Booleans |
| `Unit` | The unit `()` type |
| `Str(Arc<str>)` | String — `Arc<str>`, NOT `SmolStr` (JIT uses fat ptr layout) |
| `Array(Arc<[Value]>)` | Arrays |
| `Record(Arc<RecordData>)` | Records — `RecordData` stores `BTreeMap<SmolStr, Value>` |
| `Closure(Arc<ClosureData>)` | Closures — stores `func_ptr: usize` + `captures: Arc<[Value]>` |
| `Tag { tag: u32, payload: Arc<[Value]> }` | Sum types (Option, Result, user-defined) |

### What to Remove

| Type | Reason |
|---|---|
| `I8`, `I16` | No constant/operation in the language uses these widths |
| `U8`, `U16`, `U32`, `U64`, `Usize` | Same — typechecker only uses I32/I64/F64 |
| `F32` | Lowerer only emits `ConstF64`, never `ConstF32` |
| `Effect(Arc<dyn BuiltinFunction>)` | Remove — IO executes immediately, no deferred effects in v0.1 |

### Must Add
- `#[repr(C)]` — required by JIT for memory layout stability
- Constructor helpers:
  - `pub fn str(s: impl Into<Arc<str>>) -> Self` — currently missing
  - `pub fn array(values: Vec<Value>) -> Self` — already exists
  - `pub fn tag(tag: u32, payload: Vec<Value>) -> Self` — already exists
  - `pub fn record(fields: BTreeMap<SmolStr, Value>) -> Self` — already exists

### ClosureData Changes

Current `ClosureData` has `FuncPtr` enum (`Builtin` or `Jit`). Simplify to just store the raw function pointer:

```rust
#[repr(C)]
pub struct ClosureData {
    pub func_ptr: usize,        // Address of JIT-compiled function
    pub captures: Arc<[Value]>,
    pub arity: usize,
}
```

Remove the `FuncPtr` enum entirely — the JIT always resolves function pointers at compile time.

### Test Changes
- Remove tests for `I8`, `I16`, `U8`, `U16`, `U32`, `U64`, `Usize`, `F32` — they no longer exist
- Add tests for `Value::str()` constructor
- Keep all existing tests for kept types

---

## Step 2 — Rewrite `crates/runtime/src/bridge.rs`

**Problem:** Current trait uses `SmolStr` names, `RuntimeError` error type, no registry for non-prelude builtins.

### New `BuiltinFunction` Trait

```rust
pub trait BuiltinFunction: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
    fn arity(&self) -> usize;
    fn execute(&self, args: &[Value]) -> Result<Value, String>;
}
```

Changes from current:
- `fn name(&self) -> SmolStr` → `fn name(&self) -> &str` (avoid allocation in FFI)
- `Result<Value, RuntimeError>` → `Result<Value, String>` (simpler FFI boundary)
- Keep `Send + Sync + fmt::Debug` bounds

### Add `BuiltinRegistry`

```rust
pub struct BuiltinRegistry {
    functions: HashMap<String, Arc<dyn BuiltinFunction>>,
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        Self { functions: HashMap::new() }
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
```

### Add Helper Functions

```rust
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
```

### Add Global Registry (for JIT integration)

```rust
use std::sync::OnceLock;

static GLOBAL_REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();

/// Initialize the global builtin registry. Call once before JIT compilation.
pub fn init_global_registry(registry: BuiltinRegistry) {
    GLOBAL_REGISTRY.set(registry).expect("global registry already initialized");
}

/// Access the global builtin registry. Panics if not initialized.
pub fn global_registry() -> &'static BuiltinRegistry {
    GLOBAL_REGISTRY.get().expect("global registry not initialized — call init_global_registry() first")
}
```

### Update `lib.rs`

Re-export new items:

```rust
pub mod bridge;
pub mod error;
pub mod jit;
pub mod value;

pub use crate::bridge::{BuiltinFunction, BuiltinRegistry, expect_arity, global_registry, init_global_registry};
pub use crate::error::RuntimeError;
pub use crate::jit::{CompiledModule, JitError, compile_ir};
pub use crate::value::{ClosureData, RecordData, Value};
```

### From NzM666's Patches
- **patch-4's bridge.rs: ACCEPT** — it has the correct `BuiltinRegistry`, `expect_arity`, and simplified `BuiltinFunction` trait
- Remove the `call_closure` function from bridge.rs — closure calls go through JIT

---

## Step 3 — Update Prelude (`crates/stdlib/src/prelude.rs`)

### Problem

Current prelude implements the OLD `BuiltinFunction` trait:
- `fn name(&self) -> SmolStr` → must change to `fn name(&self) -> &str`
- `fn execute(&self, args: &[Value]) -> Result<Value, RuntimeError>` → must change to `Result<Value, String>`
- `ClosureData` constructor must match new struct from Step 1

### What to Update

Every struct in `prelude.rs`:
- `Id`, `Const`, `ConstInner`, `Flip`, `FlipInner`, `Compose`, `ComposeInner`, `Pipe`, `PipeInner`, `Apply`
- Test structs: `AddOne`, `Double`

Changes per struct:
1. `fn name(&self) -> SmolStr { SmolStr::new("...") }` → `fn name(&self) -> &str { "..." }`
2. `fn execute(...) -> Result<Value, RuntimeError>` → `fn execute(...) -> Result<Value, String>`
3. Replace `RuntimeError::TypeMismatch { expected: "...", got: format!("{:?}", ...), }` with `Err(format!("expected Closure, got {:?}", ...))`
4. Update `ClosureData` constructors to use new struct fields (`func_ptr`, `captures`, `arity`)

### Update `prelude_builtins()` Signature

The return type changes from `Vec<(SmolStr, Arc<dyn BuiltinFunction>)>` to something that works with the new trait. Since `BuiltinRegistry::register` takes `Arc<dyn BuiltinFunction>`, simplify:

```rust
pub fn prelude_builtins() -> Vec<Arc<dyn BuiltinFunction>> {
    vec![
        Arc::new(Id),
        Arc::new(Const),
        Arc::new(Flip),
        Arc::new(Compose),
        Arc::new(Pipe),
        Arc::new(Apply),
    ]
}
```

The caller (session.rs) will register them by calling `registry.register(builtin)` for each.

---

## Step 4 — Add Stdlib Modules

### 4.1 Arrays (`crates/stdlib/src/array.rs`)

Builtins to implement:

| Function | Source Name | Arity | Semantics |
|---|---|---|---|
| `ArrayMap` | `map` | 2 | `(Array<a>, (a) -> b) -> Array<b>` |
| `ArrayFilter` | `filter` | 2 | `(Array<a>, (a) -> Bool) -> Array<a>` |
| `ArrayFold` | `fold` | 3 | `(Array<a>, b, (b, a) -> b) -> b` |
| `ArrayConcat` | `concat` | 2 | `(Array<a>, Array<a>) -> Array<a>` |
| `ArrayLen` | `len` | 1 | `(Array<a>) -> I32` |
| `ArrayHead` | `head` | 1 | `(Array<a>) -> Option<a>` |
| `ArrayTail` | `tail` | 1 | `(Array<a>) -> Option<Array<a>>` |

**Key rules:**
- Never mutate input arrays — always return a new `Value::Array(Arc::from(...))`
- `head` returns `Value::tag(0, vec![])` for None, `Value::tag(1, vec![first.clone()])` for Some
- `tail` returns `Value::tag(0, vec![])` for None, `Value::tag(1, vec![Value::array(rest)])` for Some

**`head` implementation:**
```rust
fn execute(&self, args: &[Value]) -> Result<Value, String> {
    expect_arity(self.name(), args, 1)?;
    let array = expect_array(self.name(), &args[0])?;
    match array.first() {
        Some(val) => Ok(Value::tag(1, vec![val.clone()])),  // Some(value)
        None => Ok(Value::tag(0, vec![])),                   // None
    }
}
```

**`tail` implementation:**
```rust
fn execute(&self, args: &[Value]) -> Result<Value, String> {
    expect_arity(self.name(), args, 1)?;
    let array = expect_array(self.name(), &args[0])?;
    if array.len() <= 1 {
        Ok(Value::tag(0, vec![]))  // None
    } else {
        Ok(Value::tag(1, vec![Value::array(array[1..].to_vec())]))  // Some(rest)
    }
}
```

### 4.2 Strings (`crates/stdlib/src/str.rs`)

| Function | Source Name | Arity | Semantics |
|---|---|---|---|
| `StrConcat` | `Str.concat` | 2 | `(Str, Str) -> Str` |
| `StrLen` | `Str.len` | 1 | `(Str) -> I32` — byte length |
| `StrSplit` | `Str.split` | 2 | `(Str, Str) -> Array<Str>` |

**Note:** Names are prefixed `Str.` to avoid collisions with array operations.

### 4.3 IO (`crates/stdlib/src/io.rs`)

| Function | Source Name | Arity | Semantics |
|---|---|---|---|
| `IoPrintln` | `println` | 1 | `(Str) -> Unit` — prints to stdout + newline |
| `IoPrint` | `print` | 1 | `(Str) -> Unit` — prints to stdout, no newline |
| `IoReadLine` | `read_line` | 0 | `() -> Str` — reads a line from stdin |

**Important:** Do NOT use `Value::Effect` wrapping. Just `println!` / `print!` directly. The Effect pattern is nice but the JIT is not ready for it (Phase 6 at earliest). Direct execution is simpler and sufficient for v0.1.

Names are unprefixed (`println`, `print`, `read_line`) because the lowerer emits them directly from `Expr::Call` with the source identifier name.

### 4.4 Module Registration (`crates/stdlib/src/lib.rs`)

```rust
pub mod array;
pub mod io;
pub mod prelude;
pub mod str;

pub fn version() -> &'static str {
    "0.1.0";
}
```

### 4.5 Prelude Registration

Update `prelude_builtins()` in `crates/stdlib/src/prelude.rs` to include all builtins:

```rust
pub fn prelude_builtins() -> Vec<Arc<dyn BuiltinFunction>> {
    let mut builtins: Vec<Arc<dyn BuiltinFunction>> = vec![
        // Core utility functions
        Arc::new(Id),
        Arc::new(Const),
        Arc::new(Flip),
        Arc::new(Compose),
        Arc::new(Pipe),
        Arc::new(Apply),
    ];

    // Array operations
    builtins.push(Arc::new(array::ArrayMap));
    builtins.push(Arc::new(array::ArrayFilter));
    builtins.push(Arc::new(array::ArrayFold));
    builtins.push(Arc::new(array::ArrayConcat));
    builtins.push(Arc::new(array::ArrayLen));
    builtins.push(Arc::new(array::ArrayHead));
    builtins.push(Arc::new(array::ArrayTail));

    // String operations
    builtins.push(Arc::new(str::StrConcat));
    builtins.push(Arc::new(str::StrLen));
    builtins.push(Arc::new(str::StrSplit));

    // IO
    builtins.push(Arc::new(io::IoPrintln));
    builtins.push(Arc::new(io::IoPrint));
    builtins.push(Arc::new(io::IoReadLine));

    builtins
}
```

---

## Step 5 — Wire Registry into CLI Pipeline

### Update `crates/cli/src/session.rs`

Import and call `init_global_registry` before JIT compilation:

```rust
use stdlib::prelude::prelude_builtins;

pub fn run_pipeline(&mut self) -> Result<CompileResult, Box<SourceDiagnostic>> {
    // ... parse, typecheck, lower as before ...

    // Initialize global builtin registry before JIT
    let builtins = prelude_builtins();
    let mut registry = runtime::bridge::BuiltinRegistry::new();
    for builtin in builtins {
        registry.register(builtin);
    }
    runtime::bridge::init_global_registry(registry);

    // Stage 4: JIT compile and run
    let compiled = runtime::compile_ir(&ir_module).map_err(|e| {
        Box::new(SourceDiagnostic::new(
            filename.clone(),
            source_arc.clone(),
            CompilerError::RuntimeError {
                span: None,
                msg: e.to_string(),
            },
        ))
    })?;
    // ...
}
```

The JIT (Member 1) will call `runtime::bridge::global_registry()` to resolve `CallNamed` targets.

---

## Step 6 — Verify & Test

### Test commands
```bash
cargo test -p runtime        # Runtime unit tests
cargo test -p stdlib          # Stdlib unit tests
cargo clippy --workspace -- -D warnings  # Lint check
cargo fmt --check             # Format check
```

### Unit test checklist per builtin

For each stdlib function, write tests for:
1. Normal case returns expected value
2. Edge case (empty array, empty string, etc.)
3. Type errors return error strings (not panics)

### End-to-end smoke tests

After Member 1's JIT supports calls, these should work from the CLI:

```
# test_len.pp
let x = len([1, 2, 3])
println("done")

# test_head.pp
let x = head([1, 2, 3])
println("done")

# test_map.pp
let result = map([1, 2, 3], (x) => x + 1)
println("done")

# test_fold.pp
let sum = fold([1, 2, 3], 0, (acc, x) => acc + x)
println("done")
```

These can be tested from the CLI once JIT supports CallNamed.

---

## NzM666 Patch Assessment

| Patch | Verdict | Action |
|---|---|---|
| **patch-1** (array.rs) | ACCEPT | Use as base, add `len`/`head`/`tail` — core 4 are well-written |
| **patch-2** (str.rs) | ACCEPT | Use as-is |
| **patch-3** (io.rs) | ACCEPT with changes | Remove Effect wrapping, execute IO immediately |
| **patch-4** (bridge.rs) | ACCEPT | Use as base, add `OnceLock` global registry, update `lib.rs` |
| **patch-4** (value.rs) | REJECT | Copy-paste error — replaced all of value.rs with io.rs content |

### How to Apply Patches

```bash
# Apply NzM666 patches to a branch
git checkout -b member2-work
git merge origin/NzM666-patch-4  # bridge.rs (has BuiltinRegistry + expect_arity + simplified trait)
git merge origin/NzM666-patch-1  # array.rs (map, filter, fold, concat)
git merge origin/NzM666-patch-2  # str.rs
git merge origin/NzM666-patch-3  # io.rs

# Then fix up value.rs manually (reject patch-4's value.rs changes)
git checkout origin/main -- crates/runtime/src/value.rs
# Rewrite value.rs per Step 1 above
```

Or cherry-pick only bridge.rs and stdlib files:

```bash
git checkout -b member2-work
git checkout origin/NzM666-patch-4 -- crates/runtime/src/bridge.rs  # get bridge.rs only
git checkout origin/NzM666-patch-1 -- crates/stdlib/src/array.rs
git checkout origin/NzM666-patch-2 -- crates/stdlib/src/str.rs
git checkout origin/NzM666-patch-3 -- crates/stdlib/src/io.rs
```

---

## Critical Timing Dependency

**Member 2 must finish BEFORE Member 1's JIT Phase 3** (function calls), because:
- The JIT's `CallNamed` resolution depends on `BuiltinRegistry` being populated
- The JIT's `Value` layout depends on `#[repr(C)]` matching the IR `IrType` definitions
- The prelude builtins must be registered before any program with function calls can run

**Suggested order:**
1. Member 2: Steps 1-4 (Value, Bridge, Stdlib) — ~2-3 days
2. Member 1: JIT Phase 1-2 (primitives + control flow) — parallel with Member 2
3. Member 2: Step 5 (wiring registry into CLI) — 1 day
4. Member 1: JIT Phase 3 (function calls + BuiltinRegistry integration) — 1 day
5. Joint: Integration test all prelude builtins through the full pipeline

---

## What Member 1 Needs From You

Before starting JIT Phase 3, Member 1 needs:
1. `Value` enum with `#[repr(C)]` — stable memory layout for Cranelift codegen
2. `BuiltinRegistry` accessible via `global_registry()` — to resolve `CallNamed` names
3. All prelude builtins registered — so test programs with `map`/`filter`/`println` actually work

Tell Member 1 when you've completed Steps 1-5 so they can start Phase 3 integration.
