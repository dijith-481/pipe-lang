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
let _privateVal = 2
```

### 2.4 Literals
**Numeric Literals:**
There is no implicit type coercion. Numeric literals map to their explicit types. If no suffix is provided, they default to `i32` and `f64` based on the presence of a decimal point.
*   **Signed Integers:** `i8`, `i16`, `i32` (default), `i64` (e.g., `42`, `42i64`, `-10i8`)
*   **Unsigned Integers:** `u8`, `u16`, `u32`, `u64`, `usize` (e.g., `255u8`, `100usize`)
*   **Floats:** `f32`, `f64` (default) (e.g., `3.14`, `2.0f32`)

**Boolean Literals:**
`true`, `false`

**String Literals:**
Strings are UTF-8 encoded and immutable.
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
*   **Arithmetic:** `+`, `-`, `*`, `/`, `%`
*   **Comparison:** `==`, `!=`, `<`, `<=`, `>`, `>=`
*   **Logical:** `&&`, `||`, `!`
*   **Data Access:** `.` (Method chaining and record field access)
*   **No List Operator:** There is no `:` (cons) or `++` operator. Lists are manipulated exclusively via standard library methods (`arr.concat()`, `arr.prepend()`).

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
type UserId = i64  //  UserId === i64 : false handled by the Hindley-Milner type checker
```

---

## 4. Expressions & Control Flow

Everything in `pipe-lang` is an expression. Every block evaluates to its final expression.

### 4.1 Let Bindings
```rust
let x = 5
```

### 4.2 Functions & Closures
Functions are defined via arrow syntax. There is no `return` statement.
```rust
// Single expression
let add = (a, b) => a + b

// Block expression
let complex_math = (x) => {
    let doubled = x * 2
    doubled * doubled
}
```

### 4.3 If / Else (Rust-Style)
Control flow uses Rust-style brace syntax. **There is no `then` keyword.** Both branches must evaluate to the same type.
```rust
let absolute = (x) => if x > 0 { x } else { -x }
```

### 4.4 Method Chaining
Methods are syntactic sugar for function application. `a.f(b)` desugars strictly to `f(a, b)`.
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
1.  **Wildcard:** `_` (matches anything, discards value)
2.  **Binding:** `x` (matches anything, binds to name `x`)
3.  **Literal:** `42`, `"hello"`, `true`
4.  **Constructor:** `Some(x)`, `Ok(val)`
5.  **Tuple:** `(a, b)`
6.  **Record:** `{ name: n, age }`

*(Note: There is no list pattern matching like `x:xs`. Arrays are manipulated via `.head()`, `.tail()`, and `.splitAt()` methods).*

---

## 6. The Effect System

`pipe-lang` does not have a `do` block or special syntax for IO. 

Side effects are represented by the generic data type `Effect<T>`. An `Effect` is an immutable description of a computation. The typechecker ensures that pure functions cannot execute effects.

To chain effects, the language relies purely on the `flatMap` and `map` methods. The runtime evaluates the single `Effect<()>` returned by `main`.

```rust
// Reading a file, transforming it, and printing it
let main: () -> Effect<()> = () => 
    io.readLine()
        .flatMap((name) => io.println(`Hello, ${name}!`))
```

---

## 7. Standard Library (Exhaustive Specification)

The standard library is implemented natively in Rust (the host language) and linked via the runtime's JIT bridge. There are no implicit type conversions. 

### 7.1 Numeric Conversions & Methods
All conversions are explicit method calls.

| Method | Type Signature | Description |
| :--- | :--- | :--- |
| `to_i64` | `(i32) -> i64` | Widening conversion |
| `to_i32` | `(f64) -> i32` | Truncating conversion |
| `to_f64` | `(i32) -> f64` | Float conversion |
| `to_str` | `(i32) -> str` | String formatting (available on all primitives) |

### 7.2 Array `<T>`
Arrays are manipulated via methods. No custom operators.

| Method | Type Signature |
| :--- | :--- |
| `map` | `<A, B>(Array<A>, (A) -> B) -> Array<B>` |
| `filter` | `<T>(Array<T>, (T) -> bool) -> Array<T>` |
| `fold` | `<A, B>(Array<A>, B, (B, A) -> B) -> B` |
| `flatMap` | `<A, B>(Array<A>, (A) -> Array<B>) -> Array<B>` |
| `concat` | `<T>(Array<T>, Array<T>) -> Array<T>` |
| `prepend` | `<T>(Array<T>, T) -> Array<T>` |
| `len` | `<T>(Array<T>) -> usize` |
| `head` | `<T>(Array<T>) -> Option<T>` |
| `tail` | `<T>(Array<T>) -> Option<Array<T>>` |

### 7.3 Option `<T>` & Result `<T, E>`
| Method | Type Signature |
| :--- | :--- |
| `Option.map` | `<A, B>(Option<A>, (A) -> B) -> Option<B>` |
| `Option.flatMap` | `<A, B>(Option<A>, (A) -> Option<B>) -> Option<B>` |
| `Option.unwrapOr` | `<A>(Option<A>, A) -> A` (Requires default fallback) |
| `Result.map` | `<T, E, U>(Result<T, E>, (T) -> U) -> Result<U, E>` |
| `Result.flatMap` | `<T, E, U>(Result<T, E>, (T) -> Result<U, E>) -> Result<U, E>` |

### 7.4 Strings (`str`)
| Method | Type Signature |
| :--- | :--- |
| `len` | `(str) -> usize` (Byte length) |
| `concat` | `(str, str) -> str` |
| `split` | `(str, str) -> Array<str>` |
| `trim` | `(str) -> str` |
| `parse_i32` | `(str) -> Result<i32, str>` |

### 7.5 IO Module (Requires `use stdlib::io`)
| Function | Type Signature |
| :--- | :--- |
| `io.println` | `(str) -> Effect<()>` |
| `io.readLine` | `() -> Effect<str>` |
| `io.readFile` | `(str) -> Effect<Result<str, str>>` |

---

## 8. Memory Model & Runtime Architecture

`pipe-lang` achieves complete memory safety and determinism without a tracing Garbage Collector.

### 8.1 Region-Based RC
- **Primitives:** (`i32`, `f64`, `bool`) are unboxed and passed by value on the stack.
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
    F64(f64),
    Bool(bool),
    Str(Arc<str>),
    Array(Arc<[Value]>),
    Record(Arc<BTreeMap<SmolStr, Value>>),
    Closure(Arc<ClosureData>),
    Effect(Arc<dyn BuiltinFunction>),
    Tag { tag: u32, payload: Arc<[Value]> },
    Unit,
}

pub struct ClosureData {
    pub func: FuncPtr,
    pub captures: Arc<[Value]>, 
    pub arity: usize,
}
```

---

## 9. Compiler Pipeline & Intermediate Representation (IR)

The compiler is a strict, single-pass pipeline.

### Phase 1: Lexer & Parser (Frontend)
- **Lexer:** Hand-written, zero-copy. Emits tokens, tracking accurate byte-spans for error reporting. 
- **Parser:** Hand-written recursive descent. Constructs the AST into a memory Arena (`bumpalo`) for extreme performance and cache locality, preventing deep clone overhead.

### Phase 2: Typechecker (Hindley-Milner)
- Implements Algorithm W with Let-Polymorphism.
- Resolves all type variables. Rejects programs with type mismatches or unhandled effect boundaries.
- **Data Structure:** Unification uses an efficient Union-Find (Disjoint Set) structure rather than cloning HashMaps.

### Phase 3: IR Lowering
Translates the hierarchical typed AST into a flat, SSA-lite (Static Single Assignment) representation.
- Functions are hoisted.
- Closures are lowered by identifying free variables and constructing explicit capture arrays (`MakeClosure` instruction).
- **Basic Blocks:** Code is arranged into basic blocks ending in explicit terminators (`Return`, `Jump`, `Branch`, `Switch`).
- **Effect Flattening:** `Effect.flatMap` chains are flattened into sequential instructions for the runtime to interpret or JIT.

### Phase 4: Cranelift JIT Backend
- Consumes the IR. Maps IR instructions directly to Cranelift IR.
- Passes: Inlining small functions, Dead Code Elimination, Constant Folding, and Tail Call Optimization (crucial for recursive functional logic).
- Output: A callable native function pointer that takes a packed buffer of arguments and returns the result.

---

## 10. Tooling & Ecosystem

### 10.1 Command Line Interface (CLI)
Built with `clap`.
*   `pipe-lang check <file.pp>` - Runs Lex, Parse, and Typecheck. Prints diagnostics.
*   `pipe-lang compile <file.pp> --emit-ir` - Compiles and dumps the IR.
*   `pipe-lang run <file.pp>` - Full pipeline execution.

### 10.2 Diagnostics
Powered by `miette`. Errors must contain:
1. The exact byte span of the error.
2. A graphical snippet of the source code highlighting the exact tokens.
3. A clear message (e.g., "type mismatch: expected i32, got str").

### 10.3 Language Server Protocol (LSP)
Implemented as an out-of-process server using `tower-lsp`.
- **Supported features:**
  - `textDocument/didOpen` & `didChange`: Triggers background compilation and streams `PublishDiagnostics`.
  - `textDocument/hover`: Queries the typechecker's span-map to return the inferred HM type of any expression.

### 10.4 Tree-sitter
A declarative `grammar.js` repository defining the exact lexical and syntactic rules of the language. Used by modern editors (Neovim, Zed) for syntax highlighting and AST-based text manipulation, completely decoupled from the Rust compiler binary.

---

## 11. Project Workspace & Crate Structure

The repository must adhere exactly to this workspace configuration. Separation of concerns is paramount.

```text
pipe-lang/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── ast/                    # Pure data structures: Expr, Decl, Pattern (Arena allocated)
│   ├── lexer/                  # Tokenizer and Token definitions
│   ├── parser/                 # Recursive descent parser -> AST
│   ├── typechecker/            # HM unification, Types, Env, Inference rules
│   ├── ir/                     # SSA Data structures and Lowering logic
│   ├── runtime/                # ARC memory model, Value enum, Cranelift JIT logic
│   ├── stdlib/                 # Rust implementations of Arrays, Options, Strings, IO
│   ├── diagnostics/            # miette error formatting
│   └── cli/                    # Clap CLI entry point
│   └── lsp/                    # tower-lsp implementation
├── tree-sitter-pipe-lang/      # Separate grammar package
└── example-programs/           # Canonical test files
```
