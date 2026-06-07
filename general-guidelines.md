# General Development Guidelines

This document outlines the core coding standards, architectural constraints, and Rust idioms that all team members must follow when working on `pipe-lang`. 

## 1. Single-Threaded Compiler Pipeline
The `pipe-lang` compiler (Lexer → Parser → Typechecker → IR Lowering → JIT) operates in a **strictly single-threaded** pipeline. 
*   **No Synchronization:** Do not use `Mutex`, `RwLock`, `mpsc` channels, or spawn threads anywhere in the compiler frontend or backend.
*   **Runtime Exception:** The only place thread-safe primitives are permitted is the `Value` enum in the runtime, which strictly uses `Arc` (Atomic Reference Counting) to safely manage heap-allocated data without a GC.

## 2. Strings & `SmolStr`
Never use standard `String` for identifiers, map keys, or small text payloads.
*   **Use `SmolStr`:** Always use the `smol_str` crate for identifiers, field names, and module paths. It uses Small String Optimization (SSO) to store strings up to 22 bytes inline without allocating on the heap.
*   **Zero-Copy Lexing:** The Lexer should strictly emit `&'a str` slices referencing the original source code. `SmolStr` should only be instantiated when owned data is strictly required (e.g., in the IR or Runtime).

## 3. Performance & Memory 
*   **No Deep Cloning:** Never recursively `.clone()` AST nodes, Types, or IR structures. 
*   **Use Arenas:** The AST and Typechecker must use `bumpalo::Bump` arenas. Types and syntax nodes should be passed as references (`&'a Expr`) rather than owned, boxed values.
*   **Box Large Enums:** If an `enum` has one massive variant and several small ones, wrap the massive variant's payload in a `Box` to keep the size of the overall enum small (target <= 32 bytes). This maximizes cache locality.

## 4. Functional Rust Idioms
Write Rust in a functional, declarative style. 
*   **Immutability:** Avoid `mut` bindings wherever possible. Shadow variables if transformation is needed.
*   **Iterators over Loops:** Prefer `.map()`, `.filter()`, and `.fold()` over explicit `for` and `while` loops. 
*   **Exhaustive Matching:** Always use `match` rather than `if let` chains when evaluating enums to ensure the compiler catches unhandled variants.
*   **No Panics:** **NEVER** use `.unwrap()` or `.expect()` in production code. Always propagate errors up the call stack using the `?` operator.

## 5. Test-Driven Development (TDD)
All code must be written using Test-Driven Development. 
*   **Write Tests First:** Before writing a parser rule, type inference rule, or standard library function, write the test case defining its expected behavior.
*   **Inline Tests:** Place unit tests in a `#[cfg(test)] mod tests { ... }` block at the bottom of the file you are working on.
*   **AAA Pattern:** Structure your tests strictly into three phases: *Arrange* (setup data), *Act* (call the function), and *Assert* (verify the result).

## 6. The Builder Pattern
Use the Builder Pattern for initializing complex structures with multiple optional configurations (e.g., JIT Compilation settings, LSP Server configurations, or Diagnostics formatting).
*   **Fluency:** Methods should take `mut self` and return `Self` to allow for clean method chaining.

```rust
// Example:
let compiler = CompilerSession::builder()
    .with_opt_level(2)
    .emit_ir(true)
    .build()?;
```
