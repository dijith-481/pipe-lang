# pipe-lang

A minimalist, purely functional programming language with Hindley-Milner type inference, pattern matching, and JIT compilation via Cranelift.

## Philosophy

- **Minimal keyword footprint** — no `return`, `class`, `do`, or `yield`
- **Everything is an expression** — if/else, match, blocks all produce values
- **Purely functional** — immutable data, no global state, explicit effects
- **No GC** — deterministic memory management via atomic reference counting
- **JIT compiled** — flat SSA IR compiled to native code via Cranelift

## Quick Start

```bash
# Run a program
pipe-lang run example-programs/hello.pp

# Typecheck only
pipe-lang check example-programs/hello.pp

# Dump intermediate representation
pipe-lang compile example-programs/hello.pp --emit-ir
```

## Example

```rust
type Option<T> =
    | Some(T)
    | None

type Result<T, E> =
    | Ok(T)
    | Err(E)

let describe = (opt) => match opt {
    Some(val) => `Found value: ${val}`,
    None => "Nothing found"
}

let main = () => {
    println(describe(Some(42)))
}
```

## Language Features

- **Algebraic Data Types** — sum types with pattern matching
- **Records** — anonymous product types with named fields
- **Arrays** — homogeneous, immutable, with map/filter/fold
- **Closures** — first-class functions with capture semantics
- **Templates** — backtick-delimited string interpolation `${}`
- **Effect System** — `Effect<T>` for side-effecting computations
- **HM Type Inference** — full let-polymorphism, optional annotations
- **Method Chaining** — `arr.map(f).filter(g)` syntactic sugar
- **Module Imports** — `use stdlib::io` (module resolution WIP)

## Built-in Types

| Type | Description |
| :--- | :--- |
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64`, `usize` | Unsigned integers |
| `f32`, `f64` | Floating point |
| `bool`, `str` | Boolean, UTF-8 string |
| `()` | Unit type |
| `Array<T>` | Homogeneous immutable list |
| `{ name: T, ... }` | Record with named fields |
| `(A, B) -> C` | Function type |
| `Option<T>`, `Result<T, E>` | Built-in generic ADTs |
| `Effect<T>` | Deferred side effect |

## Project Structure

```
crates/
  ast/              # AST data structures (arena-allocated)
  lexer/            # Hand-written zero-copy lexer
  parser/           # Recursive descent parser
  typechecker/      # Hindley-Milner type inference
  ir/               # SSA intermediate representation
  runtime/          # Value enum, ARC memory, Cranelift JIT
  stdlib/           # Builtin function implementations
  diagnostics/      # Error formatting
  cli/              # CLI entry point
  pipe-lang-lsp/    # Language server (tower-lsp)
example-programs/   # 22 example programs
```

## Status

pipe-lang is a work-in-progress. Most core features are implemented but the runtime is still maturing. See [known-issues.md](known-issues.md) for the current bug tracker and [plan-main.md](plan-main.md) for the full specification.
