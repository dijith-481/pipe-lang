# pipe-lang Language Specification v0.1

pipe-lang is a minimalist, purely functional programming language designed for high performance, deterministic memory management, and uncompromising developer ergonomics. It blends the syntactic simplicity of Go with the expressive power of TypeScript and Rust, grounded strictly in pure functional mathematics.

---

## 1. Lexical Structure

### 1.1 Keywords

The language reserves exactly 8 keywords:

```
let    type    match    if    else    true    false    use
```

### 1.2 Comments

```
// Single line comments only
```

### 1.3 Identifiers

Identifiers start with an alphabetic character or underscore, followed by alphanumeric characters or underscores.

```
let valid_name = 1
let _privateVal = 2
let foo42 = 3
```

### 1.4 Literals

**Numeric literals:** No implicit type coercion. Default types are `i32` for integers and `f64` for floats.

| Literal | Type | Example |
|---------|------|---------|
| `42` | `i32` | Default integer |
| `42i64` | `i64` | Explicit suffix |
| `255u8` | `u8` | Unsigned |
| `100usize` | `usize` | Pointer-width unsigned |
| `3.14` | `f64` | Default float |
| `2.0f32` | `f32` | Explicit float |

**Boolean literals:** `true`, `false`

**String literals:** UTF-8 encoded, immutable. Supports `\n`, `\t`, `\\`, `\"`.

```
let plain = "Hello, world\n"
```

**Template literals:** Backtick-delimited. The only native way to concatenate values into a string.

```
let name = "Alice"
let greeting = `Hello, ${name}!`
```

### 1.5 Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+` `-` `*` `/` `%` |
| Comparison | `==` `!=` `<` `<=` `>` `>=` |
| Logical | `&&` `\|\|` `!` |
| Data access | `.` (method chaining, field access) |

There is no `:` (cons) or `++` operator. Arrays are manipulated exclusively via standard library methods.

---

## 2. Type System

The type system is statically verified using Hindley-Milner (HM) inference with let-polymorphism. Type annotations are entirely optional.

### 2.1 Primitive Types

| Type | Description | Size |
|------|-------------|------|
| `i8`, `i16`, `i32`, `i64` | Signed integers | 1–8 bytes |
| `u8`, `u16`, `u32`, `u64`, `usize` | Unsigned integers | 1–8 bytes |
| `f32`, `f64` | Floating point | 4–8 bytes |
| `bool` | Boolean | 1 byte |
| `str` | UTF-8 string (heap) | Pointer + length |
| `()` | Unit | 0 bytes |

### 2.2 Compound Types

**Arrays:** Homogeneous, immutable sequences.

```
let arr: Array<i32> = [1, 2, 3]
let empty: Array<bool> = []
```

**Tuples:** Fixed-size, heterogeneous collections.

```
let pair: (str, i32) = ("Age", 30)
```

**Records:** Anonymous product types with named fields.

```
let user: { name: str, age: i32 } = { name: "Alice", age: 30 }

// Field access
user.name

// Functional update (returns new record)
{ user | age = 31 }
```

**Functions:** First-class types.

```
let math_op: (i32, i32) -> i32 = (a, b) => a + b
```

### 2.3 Algebraic Data Types (Sum Types)

Sum types use the `type` keyword. They support generics.

```
type Option<T> =
    | Some(T)
    | None

type Result<T, E> =
    | Ok(T)
    | Err(E)

type Shape =
    | Circle(f64)
    | Rect(f64, f64)
    | Triangle(f64, f64)
```

### 2.4 Type Aliases

```
type UserId = i64
```

Type aliases are transparent to the typechecker (structural, not nominal).

### 2.5 Generics

Functions and types can be polymorphic:

```
let id = <A>(x: A) => x
let compose = <A, B, C>(f: (A) -> B, g: (B) -> C) => (x: A) => g(f(x))
```

---

## 3. Expressions & Control Flow

Everything is an expression. Every block evaluates to its final expression.

### 3.1 Let Bindings

```
let x = 5
let add = (a: i32, b: i32) => a + b
```

### 3.2 Functions & Closures

```
// Single expression
let add = (a, b) => a + b

// Block expression
let complex_math = (x) => {
    let doubled = x * 2
    doubled * doubled
}

// With type annotation
let factorial : (i32) -> i32 = (n) => match n {
    0 => 1
    n => n * factorial(n - 1)
}
```

Closures capture their environment automatically:

```
let make_adder = (x) => (y) => x + y
let add5 = make_adder(5)
add5(3)   // → 8
```

### 3.3 If / Else

Rust-style brace syntax. Both branches must have the same type.

```
let absolute = (x) => if x > 0 { x } else { -x }
let result = if condition { expr1 } else { expr2 }
```

### 3.4 Method Chaining

Methods are syntactic sugar for function application: `a.f(b)` desugars to `f(a, b)`.

```
let adults = users
    .filter((u) => u.age >= 18)
    .map((u) => u.name)
```

---

## 4. Pattern Matching

Pattern matching is exhaustive — every possible case must be handled.

```
let describe = (opt) => match opt {
    Some(val) => `Found value: ${val}`
    None => "Nothing found"
}
```

### Supported Patterns

| Pattern | Example | Matches |
|---------|---------|---------|
| Wildcard | `_` | Anything (discards value) |
| Binding | `x` | Anything (binds to name) |
| Literal | `42`, `"hello"`, `true` | Exact value |
| Constructor | `Some(x)`, `Ok(val)` | Tagged union variant |
| Tuple | `(a, b)` | Tuple |
| Record | `{ name: n, age }` | Record with field destructuring |

### Match on Integers

```
let classify = (n) => match n {
    0 => "zero"
    1 => "one"
    _ => "many"
}
```

---

## 5. Modules & Imports

```
use stdlib::io
use stdlib::array
```

The `use` keyword imports a module's functions into scope. Only `stdlib::io` requires an explicit import in v0.1; all other prelude functions are available by default.

### v0.1 Module Structure

| Module | Path | Requires `use`? |
|--------|------|-----------------|
| Prelude | (built-in) | No |
| IO | `stdlib::io` | Yes |

---

## 6. Effect System

Side effects are modeled as the generic type `Effect<T>`. An `Effect` is an immutable description of a computation. The typechecker ensures pure functions cannot execute effects.

```
// Pure function — no side effects
let greet : (str) -> str = (name) => `Hello, ${name}!`

// Effectful computation
let main : () -> Effect<()> = () =>
    read_line()
        .flat_map((name) => println(greet(name)))
```

### Effect Operators

| Method | Signature | Description |
|--------|-----------|-------------|
| `map` | `Effect<A>, (A) -> B -> Effect<B>` | Transform the result |
| `flat_map` | `Effect<A>, (A) -> Effect<B> -> Effect<B>` | Chain effects sequentially |

The runtime evaluates the single `Effect<()>` returned by `main`. Pure functions cannot call effectful functions (the typechecker rejects it).

---

## 7. Standard Library

### 7.1 Prelude (always available)

**Core utilities:**

| Function | Signature | Description |
|----------|-----------|-------------|
| `id` | `<A>(A) -> A` | Identity function |
| `const` | `<A, B>(A, B) -> A` | Constant function |
| `flip` | `<A, B, C>((A, B) -> C) -> (B, A) -> C` | Flip argument order |
| `compose` | `<A, B, C>((B) -> C, (A) -> B) -> (A) -> C` | Function composition |
| `pipe` | `<A, B, C>((A) -> B, (B) -> C) -> (A) -> C` | Forward pipe |
| `apply` | `<A, B>((A) -> B, A) -> B` | Apply function to argument |

**IO (always available):**

| Function | Signature | Description |
|----------|-----------|-------------|
| `println` | `(str) -> Effect<()>` | Print with newline |
| `print` | `(str) -> Effect<()>` | Print without newline |
| `read_line` | `() -> Effect<str>` | Read line from stdin |
| `read_file` | `(str) -> Effect<Result<str, str>>` | Read file, returns Ok/Err |

### 7.2 Array Methods

All methods are available as `array.method(args)`:

| Method | Signature | Description |
|--------|-----------|-------------|
| `map` | `<A, B>(Array<A>, (A) -> B) -> Array<B>` | Transform elements |
| `filter` | `<T>(Array<T>, (T) -> bool) -> Array<T>` | Keep matching elements |
| `fold` | `<A, B>(Array<A>, B, (B, A) -> B) -> B` | Left fold / reduce |
| `flat_map` | `<A, B>(Array<A>, (A) -> Array<B>) -> Array<B>` | Map + flatten |
| `concat` | `<T>(Array<T>, Array<T>) -> Array<T>` | Concatenate arrays |
| `prepend` | `<T>(Array<T>, T) -> Array<T>` | Prepend element |
| `len` | `<T>(Array<T>) -> usize` | Array length |
| `head` | `<T>(Array<T>) -> Option<T>` | First element |
| `tail` | `<T>(Array<T>) -> Option<Array<T>>` | All but first |

### 7.3 Option Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `map` | `<A, B>(Option<A>, (A) -> B) -> Option<B>` | Transform inner value |
| `flat_map` | `<A, B>(Option<A>, (A) -> Option<B>) -> Option<B>` | Chain optional operations |
| `unwrap_or` | `<A>(Option<A>, A) -> A` | Unwrap with default |

### 7.4 Result Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `map` | `<T, E, U>(Result<T, E>, (T) -> U) -> Result<U, E>` | Transform success |
| `flat_map` | `<T, E, U>(Result<T, E>, (T) -> Result<U, E>) -> Result<U, E>` | Chain fallible operations |

### 7.5 String Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `len` | `(str) -> usize` | Byte length |
| `concat` | `(str, str) -> str` | Concatenate strings |
| `split` | `(str, str) -> Array<str>` | Split by delimiter |
| `trim` | `(str) -> str` | Trim whitespace |
| `parse_i32` | `(str) -> Result<i32, str>` | Parse as integer |

### 7.6 Numeric Conversions

| Method | Signature | Description |
|--------|-----------|-------------|
| `to_i32` | `(f64) -> i32` | Convert f64 to i32 |
| `to_i64` | `(i32) -> i64` | Widen i32 to i64 |
| `to_f64` | `(i32) -> f64` | Convert i32 to f64 |
| `to_str` | various | Format as string |

---

## 8. Compiler Pipeline & CLI

### 8.1 CLI Commands

```
pipe-lang check <file>      # Lex → Parse → Typecheck (no codegen)
pipe-lang run   <file>      # Full pipeline: lex → parse → typecheck → lower → JIT → execute
pipe-lang compile <file>    # Full pipeline + emit binary
pipe-lang lsp               # Start language server
```

### 8.2 Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Compilation error (parse, type, IR) |
| 2 | Runtime error (panic, bounds, division by zero) |
| 3 | IO error (file not found, permission) |

### 8.3 Compilation Pipeline

```
Source (.pp)
  ↓ Lexer (zero-copy tokens with byte spans)
Vec<Token>
  ↓ Parser (recursive descent, bumpalo arena)
Program<'a> (AST)
  ↓ Typechecker (Hindley-Milner, Union-Find)
TypedProgram<'a> (AST + type map)
  ↓ IR Lowerer (flat SSA-lite IR)
IrModule (functions + basic blocks)
  ↓ Cranelift JIT
Native function pointer → execution
```

---

## 9. Examples

### Hello World

```
let main = () => println(`Hello, World!`)
```

### Factorial

```
let factorial : (i32) -> i32 = (n) => match n {
    0 => 1
    1 => 1
    n => n * factorial(n - 1)
}

let main = () => factorial(5)
```

### Map, Filter, Fold

```
let numbers = [1, 2, 3, 4, 5]

let doubled = numbers.map((x) => x * 2)
let evens = numbers.filter((x) => x % 2 == 0)
let sum = numbers.fold(0, (acc, x) => acc + x)

let main = () => println(`sum: ${sum}`)
```

### Pattern Matching with ADTs

```
type Shape =
    | Circle(f64)
    | Rect(f64, f64)

let area = (s) => match s {
    Circle(r) => 3.14159 * r * r
    Rect(w, h) => w * h
}

let main = () => println(`area: ${area(Circle(5.0))}`)
```
