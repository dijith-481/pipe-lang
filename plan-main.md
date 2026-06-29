# PIPE-LANG: Master Specification & Architecture Plan

## 1. Language Philosophy & Core Vision

`pipe-lang` is a minimalist, purely functional programming language designed for high performance, deterministic memory management, and uncompromising developer ergonomics. It blends the syntactic simplicity of Go with the expressive power of TypeScript and Rust, grounded strictly in pure functional mathematics.

**The Pillars of `pipe-lang`:**
1. **Absolute Minimalism:** Minimal keyword footprint. No `return`, `with`, `do`, `then`, `yield`, `class`, or `effect` keywords. Everything is an expression.
2. **Purity by Default:** Functions map inputs to outputs. No implicit global state, no side effects, no mutable references, and no hidden control flow.
3. **Explicit Effect Boundary:** Side effects (IO, state, randomness) are modeled strictly as generic data structures (`Effect<T>`) executed by the runtime, preventing impure code from bleeding into pure logic.
4. **No Garbage Collection:** Memory safety is achieved without a GC. Because data is immutable, cyclic references are mathematically impossible. Memory is managed deterministically via Atomic Reference Counting (ARC).
5. **AOT/JIT Execution:** The language compiles to a flat Intermediate Representation (IR) which is Just-In-Time (JIT) compiled to native machine code via Cranelift.

---

## 2. Lexical Structure & Syntax

### 2.1 Keywords
The language reserves only the absolute minimum required keywords:
`let`, `type`, `match`, `if`, `else`, `true`, `false`, `use`.

### 2.2 Comments
```rust
// Single line comments only
```

### 2.3 Identifiers
Identifiers must start with an alphabetic character or underscore, followed by alphanumeric characters or underscores.
```rust
let valid_name = 1
let _private_val = 2
```

### 2.4 Literals
**Numeric Literals:**
There is no implicit type coercion. Numeric literals map to their explicit types. If no suffix is provided, they default to `i32` and `f64` based on the presence of a decimal point.
- **Signed Integers:** `i8`, `i16`, `i32` (default), `i64` (e.g., `42`, `42i64`, `-10i8`)
- **Unsigned Integers:** `u8`, `u16`, `u32`, `u64`, `usize` (e.g., `255u8`, `100usize`)
- **Floats:** `f32`, `f64` (default) (e.g., `3.14`, `2.0f32`)

**Boolean Literals:**
`true`, `false`

**String Literals:**
Strings are UTF-8 encoded and immutable. String literals use double quotes.
```rust
let plain = "Hello, world\n"
```

**Template Literals:**
Template literals use backticks. They are the *only* way to concatenate strings natively (there is no `++` operator).
```rust
let name = "Alice"
let greeting = `Hello, ${name}!`
```

### 2.5 Operators
- **Arithmetic:** `+`, `-`, `*`, `/`, `%`
- **Comparison:** `==`, `!=`, `<`, `<=`, `>`, `>=`
- **Logical:** `&&`, `||`, `!`
- **Data Access:** `.` (Method chaining and record field access), `[]` (Array indexing)
- **No List Operator:** There is no `:` (cons) or `++` operator. Lists are manipulated exclusively via standard library methods (`concat`, `prepend`, `head`, `tail`).

---

## 3. The Type System

The type system is statically verified using the **Hindley-Milner (HM)** inference algorithm with let-polymorphism. Type annotations are entirely optional unless explicitly needed to resolve recursive function boundaries.

### 3.1 Primitive Types
`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `f32`, `f64`, `bool`, `str`, `()` (Unit).

### 3.2 Compound Types
**Arrays:** Homogeneous, immutable lists.
```rust
let arr: Array<i32> = [1, 2, 3]
```

**Tuples:** Fixed-size, heterogeneous collections.
```rust
let pair: (str, i32) = ("Age", 30)
```

**Records:** Anonymous product types with named fields.
```rust
let user: { name: str, age: i32 } = { name: "Alice", age: 30 }
```

**Functions:** First-class types.
```rust
let math_op: (i32, i32) -> i32 = (a, b) => a + b
```

### 3.3 Algebraic Data Types (Sum Types)
Sum types (Tagged Unions) are the only construct that uses the `type` keyword. They support generics.
```rust
type Option<T> =
    | Some(T)
    | None

type Result<T, E> =
    | Ok(T)
    | Err(E)
```

### 3.4 Type Aliases
```rust
type UserId = i64  // UserId === i64: transparent alias in the HM type checker
```

---

## 4. Expressions & Control Flow

Everything in `pipe-lang` is an expression. Every block evaluates to its final expression.

### 4.1 Let Bindings
```rust
let x = 5
```
Bindings can optionally include type annotations:
```rust
let x: i64 = 5
```

### 4.2 Functions & Closures
Functions are defined via arrow syntax. There is no `return` statement. Functions are closures that can capture variables from their enclosing scope.
```rust
// Single expression
let add = (a, b) => a + b

// Block expression
let complex_math = (x) => {
    let doubled = x * 2
    doubled * doubled
}

// With type annotation
let greet: (str) -> str = (name) => `Hello, ${name}!`
```

### 4.3 If / Else (Rust-Style)
Control flow uses Rust-style brace syntax. **There is no `then` keyword.** Both branches must evaluate to the same type.
```rust
let absolute = (x) => if x > 0 { x } else { -x }
```

### 4.4 Method Chaining
Method calls are syntactic sugar for function application. `a.f(b)` desugars strictly to `f(a, b)`.
```rust
let adults = users
    .filter((u) => if u.age >= 18 { true } else { false })
    .map((u) => u.name)
```

---

## 5. Pattern Matching

Pattern matching replaces complex branching logic. It must be exhaustive.

```rust
let describe = (opt) => match opt {
    Some(val) => `Found value: ${val}`,
    None => "Nothing found"
}
```

**Supported Patterns:**
1. **Wildcard:** `_` (matches anything, discards value)
2. **Binding:** `x` (matches anything, binds to name `x`)
3. **Literal:** `42`, `"hello"`, `true`
4. **Constructor:** `Some(x)`, `Ok(val)`
5. **Tuple:** `(a, b)`
6. **Record:** `{ name: n, age }`

*(Note: There is no list pattern matching like `x:xs`. Arrays are manipulated exclusively via standard library methods).*

---

## 6. The Effect System

`pipe-lang` represents side effects via the generic type `Effect<T>`. Functions that perform IO return `Effect<T>` instead of `T`. The runtime stores effects as `Value::Effect(Arc<dyn BuiltinFunction>)` — a deferred computation thunk.

**Current implementation:**
- `println`, `print`, `read_line`, `read_file` return `Effect<()>`, `Effect<str>`, and `Effect<Result<str, str>>` respectively.
- `Effect.map` and `Effect.flatMap` allow chaining effectful computations.
- The runtime evaluates effects eagerly when the effect closure is invoked by `main`.
- The typechecker does NOT enforce that pure functions cannot execute effects — this is a planned improvement.

```rust
let main: () -> Effect<()> = () => {
    println(`Hello, world!`)
}
```

---

## 7. Standard Library (Exhaustive Specification)

The standard library is implemented natively in Rust (the host language) and linked via the runtime's JIT bridge. All 44 builtins are registered in the prelude and available without imports.

### 7.1 Core Combinators (6)

| Function | Signature | Description |
| :--- | :--- | :--- |
| `id` | `(a) -> a` | Identity function |
| `const` | `(a) -> (b) -> a` | Constant combinator (K combinator) |
| `flip` | `((a, b) -> c) -> (b, a) -> c` | Swaps arguments of a binary function |
| `compose` | `((b -> c), (a -> b)) -> (a -> c)` | Left-to-right function composition |
| `pipe` | `((a -> b), (b -> c)) -> (a -> c)` | Right-to-left function piping |
| `apply` | `((a -> b), a) -> b` | Applies a function to an argument |

### 7.2 Array Operations (12)

| Method | Signature | Description |
| :--- | :--- | :--- |
| `map` | `(Array<A>, (A) -> B) -> Array<B>` | Transform each element |
| `filter` | `(Array<T>, (T) -> Bool) -> Array<T>` | Keep elements matching predicate |
| `fold` | `(Array<A>, B, (B, A) -> B) -> B` | Left fold |
| `flat_map` | `(Array<A>, (A) -> Array<B>) -> Array<B>` | Map then flatten |
| `concat` | `(Array<T>, Array<T>) -> Array<T>` | Concatenate two arrays |
| `prepend` | `(Array<T>, T) -> Array<T>` | Prepend element to array |
| `len` | `(Array<T>) -> Usize` | Number of elements |
| `head` | `(Array<T>) -> Option<T>` | First element (None if empty) |
| `tail` | `(Array<T>) -> Option<Array<T>>` | All elements except first |
| `drop` | `(Array<T>, I32) -> Array<T>` | Remove first N elements |
| `take` | `(Array<T>, I32) -> Array<T>` | Keep first N elements |

### 7.3 String Operations

| Method | Signature | Description |
| :--- | :--- | :--- |
| `Str.concat` | `(Str, Str) -> Str` | Append second string |
| `Str.len` | `(Str) -> Usize` | Byte length |
| `Str.split` | `(Str, Str) -> Array<Str>` | Split on delimiter |
| `split` | `(Str, Str) -> Array<Str>` | Bare alias for `Str.split` |
| `Str.trim` | `(Str) -> Str` | Strip leading/trailing whitespace |
| `trim` | `(Str) -> Str` | Bare alias for `Str.trim` |
| `Str.parse_i32` | `(Str) -> Result<I32, Str>` | Parse as decimal i32 |
| `parse_i32` | `(Str) -> Result<I32, Str>` | Bare alias for `Str.parse_i32` |

### 7.4 IO Operations

| Function | Signature | Description |
| :--- | :--- | :--- |
| `println` | `(Str) -> Unit` | Print with trailing newline |
| `print` | `(Str) -> Unit` | Print without newline |
| `read_line` | `(Unit) -> Str` | Read one line from stdin |
| `read_file` | `(Str) -> Result<Str, Str>` | Read entire file |

### 7.5 Option Operations

| Method | Signature | Description |
| :--- | :--- | :--- |
| `Option.map` | `(Option<A>, (A) -> B) -> Option<B>` | Transform inner value |
| `Option.flat_map` | `(Option<A>, (A) -> Option<B>) -> Option<B>` | Chain optional operations |
| `Option.unwrap_or` | `(Option<A>, A) -> A` | Default fallback |
| `unwrap_or` | `(Option<A>, A) -> A` | Bare alias |
| `Option.unwrap_or_panic` | `(Option<A>) -> A` | Panic on None |
| `unwrap_or_panic` | `(Option<A>) -> A` | Bare alias |

### 7.6 Result Operations

| Method | Signature | Description |
| :--- | :--- | :--- |
| `Result.map` | `(Result<T, E>, (T) -> U) -> Result<U, E>` | Transform Ok value |
| `Result.flat_map` | `(Result<T, E>, (T) -> Result<U, E>) -> Result<U, E>` | Chain fallible operations |
| `unwrap_or_panic` | `(Result<T, E>) -> T` | Panic on Err |

### 7.7 Numeric Conversions

| Method | Signature | Description |
| :--- | :--- | :--- |
| `to_i64` | `(numeric) -> I64` | Widen to 64-bit signed |
| `to_i32` | `(numeric) -> I32` | Narrow to 32-bit signed |
| `to_f64` | `(numeric) -> F64` | Convert to 64-bit float |
| `to_str` | `(primitive) -> Str` | Format as string |

### 7.8 Other

| Function | Signature | Description |
| :--- | :--- | :--- |
| `sqrt` | `(F64) -> F64` | Square root |
| `unwrap` | `(Option<A>) -> A` | Panic on None |

### 7.9 Effect Combinators

| Method | Signature | Description |
| :--- | :--- | :--- |
| `Effect.map` | `(Effect<A>, (A) -> B) -> Effect<B>` | Transform effect result |
| `Effect.flatMap` | `(Effect<A>, (A) -> Effect<B>) -> Effect<B>` | Chain effects |

---

## 8. Memory Model & Runtime Architecture

`pipe-lang` achieves complete memory safety and determinism without a tracing Garbage Collector.

### 8.1 Region-Based RC
- **Primitives:** (`i32`, `f64`, `bool`, etc.) are unboxed and passed by value on the stack.
- **Complex Types:** (`str`, `Array<T>`, `Records`, `Closures`, `Tags` with payloads) are heap-allocated and wrapped in an Atomic Reference Count (`Arc`).
- **No Cycles:** Because there is no mutation (no mutable bindings, no cell/ref types), it is structurally impossible to construct reference cycles.
- **Deterministic Drop:** As soon as an `Arc` count reaches zero, the memory is instantly reclaimed.

### 8.2 Value Representation (Rust Native)
The runtime uses a flat `Value` enum mapped directly to Rust types:
```rust
#[repr(C)]
pub enum Value {
    I32(i32),
    I64(i64),
    Usize(usize),
    F64(f64),
    Bool(bool),
    Unit,
    Str(Arc<str>),
    Array(Arc<[Value]>),
    Record(Arc<RecordData>),   // fields: BTreeMap<SmolStr, Value>
    Closure(Arc<ClosureData>), // func ptr + captures + arity + descriptors
    Tag { tag: u32, payload: Arc<[Value]> },
    Effect(Arc<dyn BuiltinFunction>),
}

pub struct ClosureData {
    pub func: FuncPtr,          // Builtin or Jit { address, arity }
    pub captures: Arc<[Value]>,
    pub arity: usize,
    pub param_descs: Arc<[JitArgType]>,
    pub ret_desc: Vec<JitArgType>,
}
```

---

## 9. Compiler Pipeline & Intermediate Representation (IR)

The compiler is a strict, single-pass pipeline.

### Phase 1: Lexer & Parser (Frontend)
- **Lexer:** Hand-written, zero-copy. Emits tokens, tracking accurate byte-spans for error reporting.
- **Parser:** Hand-written recursive descent. Constructs the AST into a memory Arena (`bumpalo`) for extreme performance and cache locality.

### Phase 2: Typechecker (Hindley-Milner)
- Implements Algorithm W with Let-Polymorphism.
- Resolves all type variables. Rejects programs with type mismatches or unhandled effect boundaries.
- **Data Structure:** Unification uses an efficient Union-Find (Disjoint Set) structure via the `ena` crate.
- **Output:** `TypedProgram` with `type_map: HashMap<NodeId, MonoType>` and `tag_variants: TagVariants`.

### Phase 3: IR Lowering
Translates the hierarchical typed AST into a flat, SSA-lite (Static Single Assignment) representation.
- Functions are hoisted.
- Closures are lowered by identifying free variables (`Vec`-based capture analysis for deterministic ordering) and constructing explicit capture arrays (`MakeClosure` instruction).
- **Basic Blocks:** Code is arranged into basic blocks ending in explicit terminators (`Return`, `Jump`, `Branch`, `Switch`, `TailCall`, `Unreachable`).
- **42 Instruction Variants:** Constants (I8-I64, U8-U64, Usize, F32, F64, Bool, Str, Unit), Arithmetic (Add, Sub, Mul, Div, Rem, Neg), Comparisons (Eq, Ne, Lt, Le, Gt, Ge), Logical (And, Or, Not), Array (Alloc, Get, Set, Len, Concat), Record (Alloc, Get, Set), Tag (Construct, Discriminant, Get), Closure (MakeClosure, CallIndirect, CallNamed), String (StrConcat, Println), and runtime (Panic, Retain, Release, ClosureGet).

### Phase 4: Cranelift JIT Backend
- Consumes the IR. Maps IR instructions directly to Cranelift IR.
- All functions use a uniform calling convention: `extern "C" fn(args: *const u8, ret: *mut u8) -> i32`.
- Passes: Inlining small functions, Dead Code Elimination, Constant Folding.
- Output: A callable native function pointer. `CompiledModule::call_main()` executes the program.

---

## 10. Module System (`use`)

The `use` keyword imports modules by path. The parser, typechecker, and IR lowerer all support `use` declarations.

```rust
use stdlib::io
```

- **Parsed:** Path segments separated by `::` (e.g., `use stdlib::io`).
- **Typechecked:** Returns `Unit`. No name resolution is performed (all builtins are in prelude).
- **IR:** Recorded as `IrModule::imports` for future module resolution.
- **Runtime:** Currently inert — module loading is not yet implemented.

---

## 11. Tooling & Ecosystem

### 11.1 Command Line Interface (CLI)
Built with `clap`.
- `pipe-lang check <file.pp>` — Runs Lex, Parse, and Typecheck. Prints diagnostics.
- `pipe-lang compile <file.pp> --emit-ir` — Compiles and dumps the IR to stdout.
- `pipe-lang run <file.pp>` — Full pipeline: parse, typecheck, lower to IR, JIT compile, and execute.
- `pipe-lang lsp` — Launch the Language Server Protocol server (stdio-based).

### 11.2 Diagnostics
Errors use custom formatted diagnostics with:
1. The exact byte span of the error.
2. A graphical snippet of the source code highlighting the exact tokens.
3. A clear message (e.g., "type mismatch: expected i32, got str").

### 11.3 Language Server Protocol (LSP)
Implemented as an out-of-process server using `tower-lsp`.
- **Supported features:**
  - `textDocument/didOpen` & `didChange` (full sync) — Triggers background compilation and streams diagnostics.
  - `textDocument/hover` — Queries the typechecker's span-map to return the inferred HM type of any expression.
- **Planned:** Go-to-definition, completions, document symbols.

### 11.4 Tree-sitter
A declarative `grammar.js` repository defining the exact lexical and syntactic rules of the language. Used by modern editors (Neovim, Zed) for syntax highlighting and AST-based text manipulation, completely decoupled from the Rust compiler binary. Available in a separate repository.

---

## 12. Project Workspace & Crate Structure

```text
pipe-lang/
├── Cargo.toml                       # Workspace root
├── crates/
│   ├── ast/                         # Pure data structures: Expr, Decl, Pattern (Arena allocated)
│   ├── lexer/                       # Tokenizer and Token definitions
│   ├── parser/                      # Recursive descent parser -> AST
│   ├── typechecker/                 # HM unification, MonoType, Env, Inference rules
│   ├── ir/                          # SSA data structures and Lowering logic
│   ├── runtime/                     # ARC memory model, Value enum, Cranelift JIT
│   ├── stdlib/                      # Rust implementations of all builtins (Arrays, Strings, IO, etc.)
│   ├── diagnostics/                 # Error formatting and diagnostic output
│   ├── cli/                         # Clap CLI entry point and session orchestration
│   └── pipe-lang-lsp/              # tower-lsp language server implementation
├── example-programs/                # Canonical test files (22 programs)
└── plan-main.md                     # This specification document
```

---

## 13. Example Programs

The `example-programs/` directory contains 22 canonical `.pp` programs demonstrating the language:

| Program | Lines | Demonstrates |
| :--- | :--- | :--- |
| `hello.pp` | 3 | Simple println |
| `map-strings.pp` | 3 | Array.map with strings |
| `capturing-closures.pp` | 17 | Closures capturing local variables |
| `io-effects.pp` | 21 | Pure/impure separation, Effect types |
| `json-parser.pp` | 28 | Records, recursive functions |
| `ascii-art.pp` | 33 | String building, templates, recursion |
| `higher-order.pp` | 36 | map/filter/fold pipelines, closures |
| `closures.pp` | 39 | Closure creation, composition, higher-order |
| `expression-evaluator.pp` | 39 | ADTs, recursive eval, Result |
| `patterns.pp` | 47 | Exhaustive pattern matching, nested patterns |
| `option-result.pp` | 50 | Option/Result, .map(), .unwrap_or() |
| `state-machine.pp` | 51 | Typed state machine, fold over events |
| `markdown-renderer.pp` | 53 | ADTs, pattern matching, HTML rendering |
| `generics.pp` | 59 | Polymorphic functions, generic ADTs |
| `sorting.pp` | 55 | Quicksort/Mergesort, recursion |
| `tiny-repl.pp` | 79 | Simulated REPL, parse/eval loop |
| `csv-query.pp` | 114 | CSV processing, map/filter/fold pipeline |
| `pathfinding-bfs.pp` | 122 | BFS on grid, recursive frontier expansion |
| `game-of-life.pp` | 125 | 2D arrays, neighbor counting, recursion |
| `factorial.pp` | 12 | Recursive factorial |
| `fibonacci.pp` | 11 | Naive recursive Fibonacci |
| `records.pp` | 36 | Typed records, field access, functional update |

12 of these have corresponding `.expected.txt` output files used by integration tests.
