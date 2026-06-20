# Project Abstract

### **Project Title**

**pipe-lang:** An Interpreted Pure Functional Language Exploring Hindley-Milner Type Inference and JIT Compilation

### **Team Members**

1. Dijith
1. Nazim
1. Ryan
1. Peter Joe Menachery

### **Problem Statement**

This project is a research and learning experiment aimed at understanding the deep internals of programming language theory and interpreter design. Implementing a modern language requires bridging theoretical computer science such as lambda calculus and Hindley-Milner type inference with practical system engineering. The core challenge of this project is to build a fully functioning interpreter from scratch that achieves strict type safety via union types, guarantees memory safety without a Garbage Collector (GC), and accelerates runtime execution using Just-In-Time (JIT) compilation, all while remaining strictly pure and immutable.

### **Proposed Solution**

We are building **pipe-lang**, an experimental, pure functional interpreted language. Rather than relying on existing language runtimes, our solution is a custom-built execution pipeline designed to explore several specific concepts:

- **Hindley-Milner Type System & Union Types:** We are implementing an HM type inference engine using a Union-Find algorithm. This mathematically proves type safety at compile-time and natively supports algebraic data types (union/sum types) for exhaustive pattern matching, without requiring explicit user annotations.
- **Lambda Calculus & State Machines:** Rooted in lambda calculus, the language treats closures and functions as first-class citizens. This allows complex program flows, such as state machines, to be modeled elegantly using pure functions and immutable data.
- **Interpreter & JIT Execution:** The frontend parses source code into an Arena-allocated AST and lowers it to an Intermediate Representation (IR). The runtime executes this IR using a custom tree-walking interpreter, which is backed by a Cranelift JIT compiler to dynamically accelerate hot code paths.
- **GC-Less Memory Safety:** By enforcing strict immutability, cyclic references are impossible. This allows the interpreter to safely manage memory using deterministic Atomic Reference Counting (ARC) rather than a complex garbage collector or borrow checker.

### **Tech Stack**

- **Host Language:** Rust
- **Frontend:** Custom Lexer & Recursive Descent Parser, `bumpalo` (Arena memory allocation)
- **Type Analysis:** Custom Hindley-Milner Inference Engine (Union-Find)
- **JIT Compiler:** Cranelift (Native machine code generation for the IR)
- **Memory Model:** `std::sync::Arc` (Immutable reference counting)
- **Tooling:** `clap` (CLI), `miette` (Error diagnostics)

### **Expected Outcome**

The outcome is a working, memory-safe interpreter capable of executing `pipe-lang` source files. As a research project, it will successfully demonstrate how theoretical language concepts like HM type inference, pure lambda evaluation, and union types can be practically implemented and accelerated via JIT compilation, serving as a comprehensive educational artifact for the team.
