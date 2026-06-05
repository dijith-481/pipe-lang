# Lead Architect's Handoff Timeline

## Phase 1: The Contracts (Days 1–2)

_Goal: Provide the structs, enums, and traits so the team can write their isolated TDD tests without waiting for your logic to be finished._

### Deliverable 1: The AST & Span Definitions

- **Due:** End of Day 1
- **Recipient:** Member 1 (Type Checker) & Member 3 (Diagnostics)
- **What you must push:**
  - `crates/ast/src/span.rs` (`Span` struct for tracking line/col).
  - `crates/ast/src/ast.rs` (The full `bumpalo` arena-allocated AST Enums: `Expr`, `Decl`, `Pattern`, etc.).
- **Why it unblocks them:** Member 1 cannot write Typechecker tests without an AST to check. By providing this, Member 1 can manually construct `Expr::Int(5)` in their tests. Member 3 needs `Span` to build source code highlighters.

### Deliverable 2: The Runtime Value & Bridge API

- **Due:** Day 2 (Morning)
- **Recipient:** Member 2 (Stdlib & Builtins)
- **What you must push:**
  - `crates/runtime/src/value.rs` (The `Value` enum: `Int`, `Float`, `List`, `Closure`, `Effect`).
  - `crates/runtime/src/bridge.rs` (The `BuiltinFunction` trait).
- **Why it unblocks them:** Member 2 is writing the Rust logic for `List.map` and `IO.println`. They need to know exactly how data is represented (`Value`) and the trait they must implement to hook into your future JIT engine.

### Deliverable 3: The Unified Error Trait & Session Builder

- **Due:** Day 2 (Afternoon)
- **Recipient:** Member 3 (CLI/Diagnostics)
- **What you must push:**
  - `crates/diagnostics/src/lib.rs` (A `thiserror` + `miette` enum called `CompilerError` that wraps `LexerError`, `ParseError`, `TypeError`).
  - `crates/cli/src/session.rs` (The `CompilerSession` builder API).
- **Why it unblocks them:** Member 3 is building the CLI (`lang compile main.ln`). They need a dummy entry point to call, and an error structure to format.

---

## Phase 2: The Frontend Data (Days 5–7)

_Goal: Replace their mock data with real data parsed from strings._

### Deliverable 4: A Working `Lexer` & `Parser` API

- **Due:** End of Day 5
- **Recipient:** Member 1 & Member 3
- **What you must push:**
  - A working `Parser::parse(&str, &Bump) -> Result<Program, ParseError>`.
- **Why it unblocks them:**
  - Member 1 is tired of manually constructing massive AST trees in Rust. They want to write: `let ast = parse("add = (a) => a + 1"); check_types(ast)`.
  - Member 3 needs actual `ParseError`s with real `Span`s to test their `miette` error rendering (e.g., pointing an arrow at a missing `}`).

### Deliverable 5: Parser Error Recovery (The "Don't Panic" update)

- **Due:** Day 7
- **Recipient:** Member 3 (LSP Prep)
- **What you must push:**
  - Update the parser to return a list of errors `Vec<ParseError>` while outputting a partial AST, instead of halting on the first missing semicolon.
- **Why it unblocks them:** The LSP (which Member 3 will start in Week 3) requires the parser to survive broken code as the user types.

---

## Phase 3: The Backend & Execution (Days 10–14)

_Goal: Connect Member 1's Typechecker to your IR, and run Member 2's Standard Library._

### Deliverable 6: IR Lowering Pipeline

- **Due:** Day 10
- **Recipient:** Integration / Full Team
- **What you must push:**
  - Code that takes Member 1's _Typed AST_ and converts it into your flat `IrFunction` (SSA form).
- **Why it unblocks them:** This is the critical integration point. Up until now, the Typechecker just verified code. Now, the compiler pipeline is `Source -> AST -> Typed AST -> IR`.

### Deliverable 7: The Builtin Registry & Basic JIT

- **Due:** Day 12
- **Recipient:** Member 2 (Stdlib)
- **What you must push:**
  - The Cranelift JIT execution engine capable of evaluating simple IR.
  - A `Registry` where Member 2 can inject their `BuiltinFunction` implementations so the JIT can call them.
- **Why it unblocks them:** Member 2 has written 20+ standard library functions in pure Rust. They need to see them actually run when called from the custom language code.

### Deliverable 8: The Effect Boundary Handler

- **Due:** Day 14
- **Recipient:** Member 1 & Member 2
- **What you must push:**
  - The runtime loop that executes `Effect<T>`. When the JIT encounters an IO operation, it halts pure execution, hands the state machine back to Rust, performs the IO, and resumes the JIT.
- **Why it unblocks them:** Member 1 has built the type rules for Effects; Member 2 has built the IO functions. You must provide the engine that honors the pure/impure boundary at runtime.

---

## Phase 4: Tooling & Polish (Days 17–21)

_Goal: Provide the hooks for the Language Server and clean up the API._

### Deliverable 9: AST/IR Introspection APIs (Hover & Goto)

- **Due:** Day 17
- **Recipient:** Member 3 (LSP)
- **What you must push:**
  - Utility functions on your compiler session like `get_type_at_span(Span)` or `get_definition_of(Span)`.
- **Why it unblocks them:** Member 3 is wiring up `tower-lsp`. When a user hovers their mouse over a variable in VSCode, the LSP asks the compiler "What is at byte offset 450?". You must provide the fast lookup function.

### Deliverable 10: The Demo Executable

- **Due:** Day 20
- **Recipient:** The Team (Morale & Final Testing)
- **What you must push:**
  - The fully wired `main.rs` that takes a file, runs Lexer -> Parser -> Typechecker -> IR -> JIT, executes it, and prints the result safely without segfaulting.

---

## Summary of Your Internal Deadlines

- **Day 1:** AST Enums, `Span`, Error traits.
- **Day 2:** `Value` enum, `BuiltinFunction` trait.
- **Day 5:** `parse(String) -> AST`.
- **Day 10:** `lower(Typed_AST) -> IR`.
- **Day 12:** `JIT.run(IR, Stdlib_Registry)`.
- **Day 14:** `Runtime.execute_effect()`.
- **Day 17:** `CompilerSession.lookup_span(Span)`.
