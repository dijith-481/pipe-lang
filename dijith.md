# dijith — Compiler Frontend Lead & Architect

**Crate Ownership:** `crates/lexer`, `crates/parser`, `crates/ast`, `crates/typechecker`, `crates/ir`

**Mission:** You own the pipeline from raw source text down to the flattened Intermediate Representation (IR). Your immediate goal is to aggressively strip out legacy bloat and align the codebase with the new minimalist, pure functional specification. Following the cleanup, you will implement a high-performance Hindley-Milner type inference engine and lower the Typed AST into a clean, SSA-lite IR for the Cranelift backend.

---

## Phase 1: The Great Cleanup (Days 1–3)

The current codebase contains remnants of an old, bloated plan (Haskell-style `do`-notation, unused keywords, inefficient memory usage). Your first task is to take a machete to the codebase and enforce the minimalist vision.

### 1. Lexer, Parser, and AST Purge
*   **Kill Dead Keywords:** Remove `with`, `return`, `effect`, `do`, and `yield` from `TokenKind` in `crates/lexer/src/lexer.rs`.
*   **Remove `Do` Blocks:** Delete `Expr::Do` and `DoStmt` from `crates/ast/src/ast.rs`. Remove all parsing and typechecking logic for them. 
*   **Standardize Blocks:** Ensure standard `{ statement; expr }` blocks are the only way to sequence expressions.
*   **Make Types Optional:** Ensure the parser treats type annotations strictly as `Option<&'a TypeExpr>`.
*   **Eliminate Implicit Imports:** Ensure the parser strictly handles `use` statements without any magic prelude injection at the syntax level.

### 2. Typechecker Memory & Performance Overhaul
*   **Stop the Deep Clones:** The current `MonoType` deep-clones itself recursively during unification (`apply()`). Refactor `MonoType` to be allocated in an Arena (`bumpalo`) or use `Rc`/`Arc` so types are passed by reference, not by value.
*   **Union-Find Substitution:** The `unify.rs` substitution uses a slow, cloning `HashMap` that operates in $O(N^2)$. Replace this with a **Union-Find (Disjoint Set)** data structure (e.g., using the `ena` crate or writing a custom one). This makes type variable resolution amortized $O(1)$ via path compression.
*   **Demote `Effect<T>`:** Remove `MonoType::Effect` as a special compiler intrinsic. `Effect<T>` must be treated as a standard generic type, identical in compiler mechanics to `Option<T>`.

### 3. IR Crate Optimization
*   **Box Large Variants:** The `Instruction` enum in `crates/ir/src/lib.rs` is currently ~80 bytes because of variants like `TagConstruct` and `RecordAlloc`. Box the large payloads (e.g., `TagConstruct(Box<TagConstructData>)`) so the enum shrinks to ~32 bytes, maximizing cache locality and speeding up backend traversal.

---

## Phase 2: High-Performance Typechecker (Days 4–8)

Once the foundation is clean, implement the core analysis engine.

### 1. Hindley-Milner Inference (Algorithm W)
*   Implement robust HM inference using your new Union-Find structure in `crates/typechecker/src/infer.rs` and `unify.rs`.
*   Ensure functions without type annotations are fully inferred (e.g., `let add = (a, b) => a + b` infers `(i32, i32) -> i32` based on usage, or generic `<A>(A, A) -> A` if purely polymorphic).

### 2. Let-Polymorphism
*   Implement `generalize` and `instantiate`.
*   Ensure polymorphic functions (`let id = (x) => x`) correctly generalize their type variables and instantiate fresh variables at every call site.

### 3. Strict Rules & Exhaustiveness
*   **No Coercions:** Strictly enforce that `i32` and `f64` do not mix. Enforce explicit numeric suffixes or strict unification.
*   **Pattern Matching:** Implement exhaustiveness checking for Sum Types (`Option`, `Result`) and primitive matching. A missing arm must throw a `TypeError::NonExhaustiveMatch`.

---

## Phase 3: IR Lowering (Days 9–12)

Bridge the gap between your complex Typed AST and the flat structure required by the Cranelift backend.

### 1. Flattening the AST
*   Write `lower_program` in `crates/ir/src/lower.rs`.
*   Convert nested expressions (e.g., `a(b(c()))`) into a flat, SSA-lite list of `Instruction`s bound to distinct `ValueId`s within `BasicBlock`s.
*   Enforce explicit terminators (`Return`, `Jump`, `Branch`, `Switch`) at the end of every `BasicBlock`.

### 2. Closure Hoisting
*   Detect free variables captured by closures during lowering.
*   Extract them and emit explicit `MakeClosure` instructions that package the function pointer with its captured environment.

### 3. Method Desugaring
*   Desugar method chaining (`a.map(f)`) into standard function calls (`map(a, f)`) resolving to the correct standard library function based on the inferred type.

---

## Your API Contracts (What you must expose)

To ensure your team is unblocked, you must guarantee the stability of these API boundaries as early as possible.

### 1. For the Tooling/LSP Member:
They need clean, stateless entry points that return either success or your `CompilerError` types for `miette` rendering.
```rust
// In crates/parser/src/lib.rs
pub fn parse<'a>(bump: &'a Bump, source: &'a str) -> Result<&'a Program<'a>, Vec<CompilerError>>;

// In crates/typechecker/src/lib.rs
pub fn typecheck<'a>(ast: &'a Program<'a>) -> Result<TypedProgram<'a>, Vec<CompilerError>>;
```

### 2. For the Cranelift JIT Member:
They need the frozen, clean IR shape to start mapping to native machine code.
```rust
// In crates/ir/src/lib.rs
pub struct IrModule {
    pub imports: Vec<SmolStr>,
    pub functions: Vec<IrFunction>,
}

pub fn lower(typed_ast: &TypedProgram) -> Result<IrModule, CompilerError>;
```

---

## Success Criteria for dijith

1. **Zero Dead Code:** The codebase contains no logic for `do` blocks, `with`, or `effect` keywords.
2. **Typechecker Speed:** `MonoType` is passed by reference (Arena/Rc) and Unification uses Union-Find.
3. **IR Generates Cleanly:** All 14 example programs parse, typecheck, and lower into a valid `IrModule` without panicking.
4. **Pre-commit Gates Green:** 
   * `cargo fmt --check`
   * `cargo clippy --workspace -- -D warnings`
   * `cargo test --workspace` (All existing and newly added AST/Typechecker/IR tests pass).
