# pipe-lang Specification

**Version:** 0.1.0 (prototype)
**File extension:** `.pp`
**Compiler:** `pipe-lang` (Rust CLI with Cranelift JIT)

---

## 1. Design Philosophy

pipe-lang is a **pure functional language** with Rust/TypeScript-inspired syntax. It compiles to native code via JIT (Cranelift) and enforces a strict separation between pure computations and effectful (IO) operations.

### Core Principles

| Principle | Description |
|-----------|-------------|
| **Purity by default** | All functions are pure — no side effects, no mutation |
| **Explicit effects** | Side effects are wrapped in `Effect<T>` and tracked by the type system. Effects only execute inside a `do { ... }` block. |
| **No implicit coercion** | `i32` and `f64` are distinct types; conversions are explicit |
| **No string concatenation operator** | Strings are built with template literals (`` `...${expr}...` ``); array concatenation uses `.concat()` |
| **Let bindings only** | All bindings require `let` — no hidden assignments |
| **Immutable data** | All values are immutable; sharing is safe via reference counting |

### Type System

- Hindley-Milner type inference with optional explicit annotations
- 11 distinct numeric types (no single generic `Int`)
- Generic types via type application: `Option<T>`, `Result<T, E>`, `Array<T>`
- Effect types: `Effect<T>` wraps effectful computations

---

## 2. Lexical Structure

### 2.1 Comments

```
// This is a line comment
-- This is also a line comment (fallback syntax)
```

Comments are whitespace-equivalent and ignored by the parser.

### 2.2 Identifiers

```
identifier ::= [a-zA-Z_][a-zA-Z0-9_']*
```

Examples: `x`, `user_name`, `add'`, `MAX_SIZE`

### 2.3 Keywords

```
let      type     match    if       then     else
do       true     false
```

### 2.4 Literals

#### Integer Literals

```
42         -- i32 (default)
42i8       -- i8
42i16      -- i16
42i32      -- i32
42i64      -- i64
255u8      -- u8
255u16     -- u16
255u32     -- u32
255u64     -- u64
100usize   -- usize (for indexing/sizes)
0xFF       -- hex (parsed as i32 by default)
0o77       -- octal
0b1010     -- binary
```

#### Float Literals

```
3.14       -- f64 (default)
3.14f32    -- f32
3.14f64    -- f64
1.0e10     -- scientific notation
```

**Note:** `1.name` is parsed as integer `1` followed by field access `.name`, NOT as a float.

#### String Literals

```
"hello"
"line\nnewline"
"tab\there"
"backslash\\"
"quote\""
```

Plain `"..."` strings are always allowed and are the natural
choice when the string contains no interpolation. Backtick template
literals (below) are only required when an embedded `${expr}` is
needed; they have the same escape semantics.

```pipe
let greeting = "Hello, World!"          // plain — fine
let greeting = `Hello, World!`          // equivalent — also fine
let greeting = `Hello, ${name}!`        // template — required
```

#### Template Literals (String Interpolation)

Template literals are delimited by backticks and embed expressions inside
`${ ... }`. They replace string concatenation (`++`) as the only way to
build strings from parts.

```pipe
`Hello, World!`
`Hello, ${name}!`
`${n}! = ${result}`
`${a} + ${b} = ${a + b}`
```

**Semantics:**
- The literal text outside `${}` is preserved verbatim
- Inside `${}` is any expression; its value is converted to `str`
- Built-in numeric and boolean primitives are auto-converted
  (`i32`, `i64`, `f64`, `bool`, `char` — they have `toString()`)
- `str` values are inserted as-is
- Other types (records, arrays, options, results) require an explicit
  `.toString()` call inside the hole
- Template holes can span multiple lines and contain any expression,
  including nested template literals
- To embed a literal `${`, escape as `\${`; to embed a backtick, escape
  as `` \` ``
- Template literals are typed `str`

```pipe
// Multi-line template literal
let banner = `
=========================================
 Welcome, ${user.name}!
 You have ${count} new messages.
=========================================
`
```

**Note:** pipe-lang has no `++` operator and no separate string
concatenation operator. Template literals are the canonical way to
build strings. For accumulation in a `fold`, template literals are
used directly (O(n²) cost; a `StringBuilder`-style helper is a 0.2
optimization).

#### Boolean Literals

```
true
false
```

### 2.5 Operators

#### Arithmetic

| Op | Description |
|----|-------------|
| `+` | Addition |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division |
| `%` | Modulo |

#### Comparison

| Op | Description |
|----|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

#### Logical

| Op | Description |
|----|-------------|
| `&&` | Logical AND |
| `\|\|` | Logical OR |
| `!` | Logical NOT |

#### Unary

| Op | Description |
|----|-------------|
| `-` | Numeric negation |
| `!` | Boolean negation |

### 2.6 Delimiters

```
( )  { }  [ ]  ,  ;  :  .  =>
```

---

## 3. Type System

### 3.1 Primitive Types

| Type | Description | Size |
|------|-------------|------|
| `i8` | Signed 8-bit integer | 1 byte |
| `i16` | Signed 16-bit integer | 2 bytes |
| `i32` | Signed 32-bit integer | 4 bytes |
| `i64` | Signed 64-bit integer | 8 bytes |
| `u8` | Unsigned 8-bit integer | 1 byte |
| `u16` | Unsigned 16-bit integer | 2 bytes |
| `u32` | Unsigned 32-bit integer | 4 bytes |
| `u64` | Unsigned 64-bit integer | 8 bytes |
| `usize` | Platform-dependent unsigned | 4/8 bytes |
| `f32` | 32-bit IEEE 754 float | 4 bytes |
| `f64` | 64-bit IEEE 754 float | 8 bytes |
| `bool` | Boolean | 1 byte |
| `str` | Immutable string (reference-counted) | variable |

### 3.2 Compound Types

#### Array (homogeneous, immutable)

```
[1, 2, 3]           -- Array<i32>
["a", "b", "c"]     -- Array<str>
```

Type: `Array<T>`

#### Record (product type)

```
{ name: "Alice", age: 30 }
```

Type: `{ name: str, age: i32 }`

#### Tuple

```
(42, "hello", true)  -- (i32, str, bool)
```

#### Function

```
(a:i32, b:i32) => a + b   -- type: (i32, i32) -> i32
```

### 3.3 Sum Types (Tagged Unions)

```
type Option<T> =
  | Some(T)
  | None

type Result<T, E> =
  | Ok(T)
  | Err(E)
```

Tag variants are identified by `u32` tag IDs at runtime:
- `None` = tag 0, `Some(v)` = tag 1 with payload `[v]`
- `Err(e)` = tag 0 with payload `[e]`, `Ok(v)` = tag 1 with payload `[v]`

### 3.4 Type Aliases

```
type UserId = i32
type Email = str
type Message = str
```

### 3.5 Generic Types

```
type Option<T> = | Some(T) | None
type Result<T, E> = | Ok(T) | Err(E)
type Array<T> = ...  // built-in
```

Type application: `Option<i32>`, `Result<str, Error>`, `Array<User>`

### 3.6 Type Annotations

Type annotations are **inline** on the binding:

```pipe
let add : (i32, i32) -> i32 = (a, b) => a + b
let main : () -> Effect<()> = do { ... }
let transition : (AppState, Event) -> AppState = (state, event) => ...
```

Annotations are **optional** for non-recursive bindings; Hindley-Milner
infers the type from the body. Recursive functions **require** an
explicit annotation because HM cannot resolve the self-reference
without it.

```pipe
// inferred: (i32) -> i32
let double = (x) => x * 2

// inferred: ((b) -> c, (a) -> b) -> (a) -> c
let compose = (f, g) => (x) => f(g(x))

// recursive — annotation required
let factorial : (i32) -> i64 = (n) => match n {
    0 => 1i64
    1 => 1i64
    n => n * factorial(n - 1)
}
```

The unit type is written `()`. `Effect<()>` is the canonical return
type for `main`.

### 3.7 Effect Types

```
Effect<T>    -- an effectful computation producing T
```

`Effect<T>` wraps a computation that may perform IO. Effect values
are produced by the IO module and executed by the runtime. Pure
functions cannot perform IO; only expressions inside a `do { ... }`
block can run effects.

The `io` module is accessed via `use stdlib::io`; the functions
`println`, `print`, `eprint`, `eprintln` are in the prelude and do
not require any import.

```
use stdlib::io

let main : () -> Effect<()> = do {
    name <- io.readLine()
    println(`Hello, ${name}!`)
}
```

**Pure-by-default enforcement:** mutations, IO, and other side
effects are rejected at compile time unless the call site is inside
a `do` block. The type checker verifies that the body of every
non-`do` function is referentially transparent.

---

## 4. Expressions

### 4.1 Literals

```
42          -- i32
42i64       -- i64
3.14        -- f64
3.14f32     -- f32
"hello"     -- str
true        -- bool
false       -- bool
```

### 4.2 Variables

```
x
user_name
add
```

### 4.3 Binary Operations

```
a + b
a - b
a * b
a / b
a % b
a == b
a != b
a < b
a <= b
a > b
a >= b
a && b
a || b
```

### 4.4 Unary Operations

```
-x      -- numeric negation
!x      -- boolean negation
```

### 4.5 Function Calls (Application)

```
add(1, 2)
factorial(5)
List.map([1,2,3], (x) => x * 2)
```

### 4.6 Closures

#### Single-expression closures

```
(x) => x + 1
(a, b) => a + b
(x:i32) => x * 2
(x:i32, y:i32):i64 => x + y
```

#### Block closures (multi-step)

```
(x) => {
    let y = x * 2
    let z = y + 1
    z
}
```

**Note:** Only `(x) => expr` syntax is supported. The `|x| expr` syntax is NOT used.

### 4.7 Method Chaining (Dot Operator)

Method calls are syntactic sugar for function application. The `.` operator accesses fields and calls methods:

```
users
    .filter((u) => u.age >= 18)
    .map((u) => u.email)
    .distinct()
```

Desugars to:
```
distinct(map(filter(users, (u) => u.age >= 18), (u) => u.email))
```

Field access:
```
user.name
user.age
config.database.host
```

### 4.8 Let Bindings

```
let x = 5
let name = "Alice"
let result = add(1, 2)
```

#### Let Expressions (scoped)

```
let x = 5 in x + 1
```

### 4.9 If Expressions

```
if condition then value else alternative
```

```
if x > 0 then x else -x
if age >= 18 then "adult" else "minor"
```

`else` is optional (defaults to `()` unit).

### 4.10 Match Expressions

```
match subject {
    pattern1 => result1
    pattern2 => result2
    _        => default
}
```

### 4.11 Block Expressions

```
{
    let x = 1
    let y = 2
    x + y    -- this is the result
}
```

### 4.12 Record Literals

```
{ name: "Alice", age: 30 }
```

### 4.13 Tuple Literals

```
(42, "hello")
(1, 2, 3)
```

### 4.14 Do Blocks (Effect Sequencing)

```pipe
use stdlib::io

let main : () -> Effect<()> = do {
    name <- io.readLine()
    println(`Hello, ${name}!`)
}
```

`do` blocks sequence effectful operations. Each `<-` binding
extracts the value from an `Effect<T>`. The final expression of the
block determines the block's return type, which becomes the `T` in
`Effect<T>`. Pure statements (non-effect expressions) interleaved
with `<-` bindings are allowed and evaluated in order.

```pipe
let main : () -> Effect<()> = do {
    println(`What is your name?`)
    name <- io.readLine()
    let greeting = `Hello, ${name}!`
    println(greeting)
}
```

A `do` block with no `<-` bindings desugars to a single effectful
expression; it is required whenever any statement of `main`
performs IO. The simplest program — `let main : () -> Effect<()> =
println(`Hello`)` — is a single-expression `do` body.

---

## 5. Patterns

### 5.1 Wildcard Pattern

```
_       -- matches anything, discards value
```

### 5.2 Binding Pattern

```
x       -- matches anything, binds to name `x`
```

### 5.3 Literal Patterns

```
42
"hello"
true
false
```

### 5.4 Constructor Patterns

```
Some(x)
None
Ok(val)
Err(msg)
```

### 5.5 Tuple Patterns

```
(a, b)
(1, 2)
(x, _, z)
```

### 5.6 Record Patterns

```
{ name, age }
{ name: n, age }
{ name: "Alice", age }
```

---

## 6. Declarations

### 6.1 Value Bindings

```
let x = 5
let add = (a, b) => a + b
let name = "Alice"
```

### 6.2 Type Signatures

```
let add : (i32, i32) -> i32
let factorial : (i32) -> i32
let transition : AppState -> Event -> Effect<AppState>
```

### 6.3 Type Aliases

```
type UserId = i32
type Email = str
```

### 6.4 Sum Type Definitions

```
type Option<T> =
  | Some(T)
  | None

type Result<T, E> =
  | Ok(T)
  | Err(E)

type AppState =
  | Idle
  | Loading(RequestId)
  | Ready(User)
  | Failed(Error)
```

---

## 7. Standard Library

### 7.1 Array Operations

```
Array.len       : <T>(Array<T>) -> usize
Array.head      : <T>(Array<T>) -> Option<T>
Array.tail      : <T>(Array<T>) -> Option<Array<T>>
Array.map       : <A, B>(Array<A>, (A) -> B) -> Array<B>
Array.filter    : <T>(Array<T>, (T) -> bool) -> Array<T>
Array.fold      : <A, B>(Array<A>, B, (B, A) -> B) -> B
Array.flatMap   : <A, B>(Array<A>, (A) -> Array<B>) -> Array<B>
Array.find      : <T>(Array<T>, (T) -> bool) -> Option<T>
Array.zip       : <A, B>(Array<A>, Array<B>) -> Array<(A, B)>
Array.take      : <T>(Array<T>, usize) -> Array<T>
Array.drop      : <T>(Array<T>, usize) -> Array<T>
Array.isEmpty   : <T>(Array<T>) -> bool
Array.concat    : <T>(Array<T>, Array<T>) -> Array<T>
Array.distinct  : <T: Eq>(Array<T>) -> Array<T>
Array.sort      : <T: Ord>(Array<T>) -> Array<T>
Array.sortBy    : <A, B: Ord>(Array<A>, (A) -> B) -> Array<A>
```

### 7.2 Option Operations

```
Option.map      : <A, B>(Option<A>, (A) -> B) -> Option<B>
Option.flatMap  : <A, B>(Option<A>, (A) -> Option<B>) -> Option<B>
Option.unwrap   : <A>(Option<A>, A) -> A
Option.isSome   : <A>(Option<A>) -> bool
Option.isNone   : <A>(Option<A>) -> bool
```

### 7.3 Result Operations

```
Result.map      : <T, E, U>(Result<T,E>, (T) -> U) -> Result<U,E>
Result.flatMap  : <T, E, U>(Result<T,E>, (T) -> Result<U,E>) -> Result<U,E>
Result.mapErr   : <T, E, F>(Result<T,E>, (E) -> F) -> Result<T,F>
```

### 7.4 IO / Effect

```
// In the prelude — no import required
print         : (str) -> Effect<()>
println       : (str) -> Effect<()>
eprint        : (str) -> Effect<()>
eprintln      : (str) -> Effect<()>

// In stdlib::io — import via `use stdlib::io`
io.print      : (str) -> Effect<()>
io.println    : (str) -> Effect<()>
io.readLine   : () -> Effect<str>
io.readFile   : (str) -> Effect<Result<str, IOError>>
io.writeFile  : (str, str) -> Effect<Result<(), IOError>>
```

`io` is the only module that needs an explicit import for 0.1;
`println` and friends are in the prelude because every program
prints. Methods like `io.readLine()` are accessed as field-style
calls after `use stdlib::io`.

### 7.5 Function Combinators

```
compose  : <A, B, C>((B) -> C, (A) -> B) -> (A) -> C
pipe     : <A, B>((A) -> B, ...) -> ...  (chained composition)
id       : <A>(A) -> A
const    : <A, B>(A) -> (B) -> A
flip     : <A, B, C>((A, B) -> C) -> (B, A) -> C
```

---

## 8. Memory Model

- **No garbage collector** — reference counting (`Arc`) for shared data
- **Immutable values** — safe sharing without synchronization
- **Arena allocation** for AST nodes during compilation
- **Deterministic cleanup** — `Arc::drop` runs immediately when refcount hits zero
- **No cycles** in pure data (enforced by type checker)

---

## 9. Compiler Pipeline

```
Source (.pp)
    |
    v
  Lexer         Hand-written, emits tokens with spans
    |
    v
  Parser        Recursive descent, produces AST
    |
    v
  Typechecker   Hindley-Milner inference, effect checking
    |
    v
  IR Lowering   Typed AST -> SSA-like IR
    |
    v
  Optimization  Constant folding, dead code elimination, tail call opt
    |
    v
  JIT (Cranelift)  Native code generation and execution
```

---

## 10. CLI Usage

```bash
# Type check a file
pipe-lang check program.pp

# Compile and run
pipe-lang run program.pp

# Compile to native (emit IR)
pipe-lang compile program.pp --emit-ir

# Optimization level
pipe-lang compile program.pp --opt-level 2

# Machine-readable output
pipe-lang check program.pp --json
```

---

## 11. Error Messages

pipe-lang uses `miette` for rich, Rust-style error messages with source snippets:

```
  error[pipe_lang::ty]: type mismatch: expected i32, got str
    --> program.pp:5:15
     |
   5 | let x: i32 = "hello"
     |               ^^^^^^^ expected i32, got str
```

---

## 12. File Structure

```
pipe-lang/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── ast/                    # Shared AST types
│   ├── lexer/                  # Tokenizer
│   ├── parser/                 # Recursive descent parser
│   ├── typechecker/            # HM type inference
│   ├── ir/                     # Intermediate representation
│   ├── runtime/                # JIT execution, Value types
│   ├── stdlib/                 # Standard library builtins
│   ├── diagnostics/            # Error types and rendering
│   └── cli/                    # Command-line interface
├── example-programs/           # Demo .pp files
│   ├── hello.pp
│   ├── fibonacci.pp
│   ├── factorial.pp
│   └── ...
└── pipe-lang.md                # This specification
```
