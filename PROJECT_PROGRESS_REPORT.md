# PROJECT PROGRESS REPORT

**Project Title:** `pipe-lang` — A Pure Functional Programming Language  
**Team Members:** Dijith Dinesh, Ryan Thomachan, NzM666, Petrj0

---

## 1. Work Completed

- Lexer, parser, and AST — hand-written tokenizer and recursive descent parser producing arena-allocated AST with byte-level span tracking for error reporting
- Typechecker — Hindley-Milner type inference with Union-Find unification, let-polymorphism, and support for algebraic data types (sum types, records, arrays, tuples)
- Intermediate Representation — flat SSA-lite IR with 46 instruction variants across constants, arithmetic, comparisons, logical ops, arrays, records, tags, closures, calls, and strings
- IR lowering — translates typed AST to flat IR with basic blocks, block arguments, closure hoisting, and method desugaring
- JIT backend — Cranelift-based JIT compiler handling 32 of 46 instructions, all terminators (branch, switch, jump, return), and string operations
- Runtime value model — 11-variant `Value` enum with ARC-based memory management, `FuncPtr` closure dispatch (Rust builtin or JIT native), and global `BuiltinRegistry`
- Standard library — 33 builtin functions covering array operations, string manipulation, IO, Option/Result combinators, numeric conversions, and functional combinators
- CLI — `check`/`compile`/`run` subcommands via clap with full pipeline orchestration
- Diagnostics — `CompilerError` enum with miette graphical source-span rendering
- Language server — `tower-lsp` implementation with `didOpen`/`didChange`, hover type information, and completion
- CI pipeline — automated build, test, clippy, and formatting verification
- 14 canonical example programs covering recursion, pattern matching, closures, higher-order functions, records, sum types, IO, sorting, and state machines

## 2. Individual Contributions

**Dijith Dinesh:**
- Built the compiler frontend — lexer, recursive descent parser, and arena-allocated AST
- Implemented the Hindley-Milner typechecker with Union-Find unification and let-polymorphism
- Designed the IR specification and implemented the lowering pass from typed AST to flat SSA form
- Set up the workspace structure, crate dependency graph, and CI pipeline
- Wrote the 14 canonical example programs
- Integrated and reviewed PRs from all other team members

**Ryan Thomachan:**
- Implemented the Cranelift JIT module setup and function compilation pipeline
- Translated 32 of 46 IR instructions to Cranelift IR, including all constants, arithmetic, comparisons, logical ops, and control flow
- Built runtime C-ABI helpers for `println` and string concatenation
- Wrote 52 JIT tests covering constants, arithmetic, branching, switches, strings, and end-to-end control flow

**NzM666:**
- Implemented the `Value` enum with ARC-based heap allocation for strings, arrays, records, tags, closures, and effects
- Built the `BuiltinRegistry` and `BuiltinFunction` trait for JIT-to-Rust function bridging
- Registered and tested all 33 standard library builtins across array, string, IO, Option, Result, and numeric modules
- Implemented safe closure dispatch supporting both Rust builtin and JIT-compiled function pointers

**Petrj0:**
- Built the CLI with clap subcommands and session pipeline orchestrator
- Integrated miette diagnostic rendering with source-span labels
- Developed the tower-lsp language server with incremental document sync, hover type queries, and code completion
- Set up fixture-based end-to-end testing infrastructure

## 3. Current Status

The compiler pipeline from source text through lexing, parsing, typechecking, and IR lowering is functional across all 14 example programs. The JIT backend handles primitives, arithmetic, control flow, and function calls. The standard library provides 33 builtin functions. The language server supports real-time diagnostics and hover information. The codebase contains 466 tests across 9 crates.

Remaining implementation focus areas include heap-type instructions in the JIT (arrays, records, tags, closures), pattern exhaustiveness checking, the effect system, and remaining standard library builtins.

## 4. Next Phase

- Complete all 46 JIT instructions by implementing array, record, tag, and closure operations
- Add optimization passes — inlining, dead code elimination, constant folding, tail call optimization
- Implement compile-time `Effect<T>` type tracking and pattern exhaustiveness checking
- Add first-class tuple types and remaining stdlib builtins (mod, range, Array.drop/take, Option/Result predicates, numeric width conversions)
- Finalize the tree-sitter grammar for syntax highlighting
- Add CLI flags for assembly dump, timing, and named outputs
- Polish diagnostic rendering across all error types
- Deliver an end-to-end working pipeline capable of compiling and running all example programs

## 5. Demo

Not yet available.
