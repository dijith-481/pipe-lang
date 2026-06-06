# Member 1 — Runtime (Tree-Walking Interpreter) and Standard Library

**Crate ownership:** `crates/runtime/src/interpreter.rs`, `crates/stdlib/src/**/*.rs`

**Mission:** Make the 14 example `.pp` programs run end-to-end via a tree-walking interpreter (no JIT). Implement all standard-library built-in functions in pure Rust so the interpreter can dispatch to them.

## Why this allocation

- **A tree-walking interpreter is mechanical, not clever.** It's a `match` on IR instructions that dispatches to `Value` operations. No codegen, no register allocation, no FFI. dijith owns the *hard* backend (Cranelift); you own the *easy* executor (the interpreter) and the stdlib.
- **Stdlib is parallelizable and independent of compiler internals.** Once dijith has the IR shape (Day 10), you can write the entire `stdlib` against the public `Value` enum without waiting for typechecker or Cranelift.
- **Both pieces together form a complete "run this program" path.** The interpreter + stdlib = a working execution engine that doesn't depend on the JIT.

## What's already done (don't redo)

- `crates/runtime/src/value.rs` — the `Value` enum with all primitive variants (`I32`, `I64`, `F64`, `Bool`, `Str`, `Array`, `Record`, `Closure`, `Tag`, `Effect`, `Unit`)
- `crates/runtime/src/bridge.rs` — the `BuiltinFunction` trait
- `crates/runtime/src/error.rs` — `RuntimeError` variants
- Tests: 26 value tests, 4 bridge tests, 7 error tests, 9 span tests = ~46 tests already passing
- 14 example programs in `example-programs/` (the integration target)

## Deliverable A: Tree-Walking Interpreter (Days 1–4)

**File:** `crates/runtime/src/interpreter.rs` (new)

**API:**
```rust
pub struct Interpreter {
    globals: HashMap<SmolStr, Value>,
    builtins: HashMap<SmolStr, Arc<dyn BuiltinFunction>>,
}

impl Interpreter {
    pub fn new(builtins: BuiltinRegistry) -> Self;
    pub fn run(&mut self, module: &IrModule) -> Result<i32, RuntimeError>;
    fn execute_function(&mut self, func: &IrFunction, args: &[Value]) -> Result<Value, RuntimeError>;
    fn execute_block(&mut self, block: &IrBlock, env: &mut Env) -> Result<ControlFlow, RuntimeError>;
}
```

**Semantics:**
- Walk the IR. For each `IrInst`, evaluate operands, perform the operation, store the result in a fresh `ValueId`.
- For `Call` instructions, look up the function in `globals` (for user functions) or `builtins` (for stdlib), and invoke it.
- For `Bind` instructions (effect bind), recursively evaluate the effect value, then jump to the continuation block with the result bound.
- For `Return`, propagate the value up to the caller.
- For `Match`, dispatch on tag/structure and branch to the appropriate arm.
- Closures: `MakeClosure` creates a `Value::Closure` capturing the current `Env`; `CallClosure` looks up the captured environment and applies.

**Test suite (10 tests):**
- `interp_runs_hello_world` — `hello.pp` produces `Hello, World!\n` on stdout
- `interp_runs_arithmetic` — `(3 + 4) * 2 == 14`
- `interp_runs_if_else` — `if 1 < 2 then "yes" else "no"` → `"yes"`
- `interp_runs_match_int` — `match 2 { 0 => "zero", 1 => "one", _ => "many" }` → `"many"`
- `interp_runs_match_option` — `match Some(5) { Some(x) => x, None => 0 }` → `5`
- `interp_runs_closure` — `let add = (a, b) => a + b; add(2, 3)` → `5`
- `interp_runs_recursion` — `factorial(10)` → `3628800`
- `interp_runs_array_map` — `[1, 2, 3].map((x) => x * 2)` → `[2, 4, 6]`
- `interp_runs_array_fold` — `[1, 2, 3].fold(0, (a, x) => a + x)` → `6`
- `interp_runs_template_literal` — `` `Hello, ${name}!` `` with `name = "World"` → `"Hello, World!"`

## Deliverable B: Standard Library (Days 2–7)

**File layout:**
```
crates/stdlib/src/
├── lib.rs          # builtin registry
├── array.rs        # Array<T> methods
├── option.rs       # Option<T> methods
├── result.rs       # Result<T, E> methods
├── str.rs          # str methods
├── num.rs          # i32/i64/f64/bool toString
├── io.rs           # io.println, io.readLine, io.readFile, io.writeFile
├── effect.rs       # effect construction (wrap builtin in Value::Effect)
└── combinators.rs  # id, const, flip, compose, pipe, apply
```

### B.1: Array methods (`crates/stdlib/src/array.rs`)

| Method | Signature | Implementation |
|---|---|---|
| `Array.map` | `<A, B>(Array<A>, (A) -> B) -> Array<B>` | Apply closure to each element, return new array |
| `Array.filter` | `<T>(Array<T>, (T) -> bool) -> Array<T>` | Keep elements where predicate is true |
| `Array.fold` | `<A, B>(Array<A>, B, (B, A) -> B) -> B` | Left fold with initial accumulator |
| `Array.len` | `<T>(Array<T>) -> usize` | Return `arr.len()` |
| `Array.concat` | `<T>(Array<T>, Array<T>) -> Array<T>` | Concatenate two arrays |
| `Array.drop` | `<T>(Array<T>, usize) -> Array<T>` | Drop first n elements |
| `Array.take` | `<T>(Array<T>, usize) -> Array<T>` | Take first n elements |
| `Array.head` | `<T>(Array<T>) -> Option<T>` | First element or None |
| `Array.tail` | `<T>(Array<T>) -> Option<Array<T>>` | All but first, or None if empty |
| `Array.isEmpty` | `<T>(Array<T>) -> bool` | `arr.len() == 0` |
| `Array.zip` | `<A, B>(Array<A>, Array<B>) -> Array<(A, B)>` | Pairwise zip |
| `Array.find` | `<T>(Array<T>, (T) -> bool) -> Option<T>` | First element matching predicate |
| `Array.flatMap` | `<A, B>(Array<A>, (A) -> Array<B>) -> Array<B>` | Map then flatten |
| `Array.distinct` | `<T: Eq>(Array<T>) -> Array<T>` | Remove duplicates (0.2: real hash) |
| `Array.sortBy` | `<A, B: Ord>(Array<A>, (A) -> B) -> Array<A>` | Stable sort by key |

### B.2: Option methods (`crates/stdlib/src/option.rs`)

| Method | Signature | Implementation |
|---|---|---|
| `Option.map` | `<A, B>(Option<A>, (A) -> B) -> Option<B>` | Apply closure to inner value if Some |
| `Option.flatMap` | `<A, B>(Option<A>, (A) -> Option<B>) -> Option<B>` | Bind |
| `Option.unwrap` | `<A>(Option<A>, A) -> A` | Return inner or default |
| `Option.isSome` | `<A>(Option<A>) -> bool` | Tag check |
| `Option.isNone` | `<A>(Option<A>) -> bool` | Tag check |
| `Option.orElse` | `<A>(Option<A>, Option<A>) -> Option<A>` | First if Some, else second |

### B.3: Result methods (`crates/stdlib/src/result.rs`)

| Method | Signature | Implementation |
|---|---|---|
| `Result.map` | `<T, E, U>(Result<T, E>, (T) -> U) -> Result<U, E>` | Apply closure to Ok value |
| `Result.flatMap` | `<T, E, U>(Result<T, E>, (T) -> Result<U, E>) -> Result<U, E>` | Bind |
| `Result.mapErr` | `<T, E, F>(Result<T, E>, (E) -> F) -> Result<T, F>` | Apply closure to Err value |
| `Result.recover` | `<T, E>(Result<T, E>, (E) -> T) -> T` | Unwrap with handler (panics-style API; 0.1) |
| `Result.unwrap` | `<T, E>(Result<T, E>, E) -> T` | Ok value or default |

### B.4: str methods (`crates/stdlib/src/str.rs`)

| Method | Signature | Implementation |
|---|---|---|
| `str.len` | `() -> i32` | Byte length |
| `str.toString` | `() -> str` | Identity (for the auto-`toString` rule on primitives) |
| `str.concat` | `(str) -> str` | Template-string-style concatenation |
| `str.contains` | `(str) -> bool` | Substring search |
| `str.startsWith` | `(str) -> bool` | Prefix check |
| `str.endsWith` | `(str) -> bool` | Suffix check |
| `str.trim` | `() -> str` | Strip whitespace |
| `str.toUpperCase` | `() -> str` | ASCII uppercase (0.1; 0.2 Unicode) |
| `str.toLowerCase` | `() -> str` | ASCII lowercase |
| `str.split` | `(str) -> Array<str>` | Split by separator |
| `str.replace` | `(str, str) -> str` | Replace all occurrences |
| `str.slice` | `(i32, i32) -> str` | Substring by byte range |

### B.5: Numeric `toString` (`crates/stdlib/src/num.rs`)

| Method | Signature | Implementation |
|---|---|---|
| `i32.toString` | `() -> str` | `itoa`-style formatting |
| `i64.toString` | `() -> str` | `itoa` |
| `f64.toString` | `() -> str` | `dtoa`-style, fixed precision |
| `bool.toString` | `() -> str` | `"true"` / `"false"` |
| `i32.+`, `i32.-`, `i32.*`, `i32./`, `i32.%` | binary ops | Native Rust arithmetic with overflow checks |
| `i32.<`, `i32.<=`, `i32.>`, `i32.>=`, `i32.==`, `i32.!=` | comparison | Native Rust comparison |
| Same for `i64`, `f64` | | |

### B.6: IO (`crates/stdlib/src/io.rs`)

| Function | Signature | Implementation |
|---|---|---|
| `io.println` | `(str) -> Effect<()>` | Wrap `print!` + `println!` in `Value::Effect` |
| `io.print` | `(str) -> Effect<()>` | Wrap `print!` |
| `io.readLine` | `() -> Effect<str>` | Wrap `stdin().read_line()` |
| `io.readFile` | `(str) -> Effect<Result<str, IOError>>` | Wrap `std::fs::read_to_string` |
| `io.writeFile` | `(str, str) -> Effect<Result<(), IOError>>` | Wrap `std::fs::write` |

### B.7: Combinators (`crates/stdlib/src/combinators.rs`)

| Function | Signature | Implementation |
|---|---|---|
| `id` | `<A>(A) -> A` | Return input |
| `const` | `<A, B>(A) -> (B) -> A` | Ignore second arg, return first |
| `flip` | `<A, B, C>((A, B) -> C) -> (B, A) -> C` | Swap argument order |
| `compose` | `<A, B, C>((B) -> C, (A) -> B) -> (A) -> C` | f ∘ g |
| `pipe` | `<A, B, C>((A) -> B, (B) -> C) -> (A) -> C` | g ∘ f (data flows left to right) |
| `apply` | `<A, B>((A) -> B, A) -> B` | Apply function to value |

### B.8: Builtin registry (`crates/stdlib/src/lib.rs`)

```rust
pub struct BuiltinRegistry {
    by_name: HashMap<SmolStr, Arc<dyn BuiltinFunction>>,
    by_module: HashMap<SmolStr, HashMap<SmolStr, Arc<dyn BuiltinFunction>>>,
}

impl BuiltinRegistry {
    pub fn standard() -> Self {
        // Register all stdlib builtins
        // Group by module: "io", "array", "option", "result", "str", "num"
    }
    pub fn lookup(&self, name: &str) -> Option<Arc<dyn BuiltinFunction>>;
    pub fn module(&self, name: &str) -> Option<&HashMap<SmolStr, Arc<dyn BuiltinFunction>>>;
}
```

**Test suite (10 tests):**
- `registry_has_all_io_builtins` — `io.println`, `io.readLine`, `io.readFile`, `io.writeFile` registered
- `registry_has_all_array_builtins` — All 15 Array methods registered
- `registry_has_all_option_builtins` — All 6 Option methods registered
- `registry_has_all_result_builtins` — All 5 Result methods registered
- `registry_has_all_combinators` — `id`, `const`, `flip`, `compose`, `pipe`, `apply` registered
- `array_map_doubles_each` — `Array.map([1,2,3], (x) => x*2)` → `[2,4,6]`
- `option_flatmap_chains` — `Some(5).flatMap((x) => Some(x + 1))` → `Some(6)`
- `result_maperr_swaps_error` — `Err("oops").mapErr((e) => e ++ "!")` → `Err("oops!")`
- `io_println_writes_to_stdout` — capture stdout, assert contents
- `io_readline_returns_value` — feed stdin, assert return

## Deliverable C: End-to-end test (Day 7)

All 14 example programs in `example-programs/*.pp` must run under the interpreter with their expected output.

**File:** `crates/runtime/tests/example_programs.rs`

```rust
#[test] fn runs_hello() { assert_eq!(run("hello.pp"), "Hello, World!\n"); }
#[test] fn runs_factorial() { assert_eq!(run("factorial.pp"), /* expected output */); }
// ... 14 tests total
```

Expected output for each program is documented in the program itself (a comment block at the top of each `.pp` file, or captured in this test file).

## Test counts

| Suite | Tests |
|---|---|
| Interpreter (Deliverable A) | 10 |
| Stdlib (Deliverable B.1–B.8) | 10 |
| End-to-end (Deliverable C) | 14 |
| **Total new** | **34** |
| Pre-existing | 46 |
| **Grand total** | **80** |

## Common pitfalls

1. **Clone values in closures** — `Arc` makes this cheap, but be explicit; the interpreter is hot code
2. **Tag ID conventions** — `Option::None = 0, Some = 1`; `Result::Err = 0, Ok = 1`. Must match dijith's typechecker and IR.
3. **Stdout capture in tests** — use a `Mutex<Vec<u8>>` global or `gag` crate; never `println!` directly in tests
4. **Effect execution is sequential** — there is no parallel effect; effects chain via `Bind` in the IR
5. **The interpreter is a fallback** — if Cranelift fails, `pipe-lang run` should still work via the interpreter when `PIPE_LANG_INTERP=1` is set
6. **Don't compile the interpreter with `cargo build --release --features=python`** — that's a different crate (we don't have one yet)

## Dependencies

- `runtime` crate: `Value`, `BuiltinFunction`, `RuntimeError` (all exist)
- `ir` crate: `IrModule`, `IrFunction`, `IrBlock`, `IrInst`, `IrTerm` (consumed from dijith's deliverable on Day 10)
- `ast` crate: `SmolStr` (re-exported)
- `thiserror` — add to `crates/stdlib/Cargo.toml`

You can begin Deliverable A (interpreter skeleton) and Deliverable B.1 (Array methods) on Day 1 using only the existing `Value` enum. The full interpreter comes online Day 10 when dijith's IR is ready; until then, write the stdlib and test it with hand-built `Value` instances.

## Handoff milestone: Day 7

`cargo test` for `runtime` and `stdlib` is green. All 14 example programs run via the interpreter and produce expected output. Member 2 (CLI) can now wire `pipe-lang run` to call your interpreter.
