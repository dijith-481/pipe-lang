# dijith's Handoff Timeline (Compiler Internals)

**Owner:** dijith
**Crates:** `lexer`, `parser`, `typechecker`, `ir`, Cranelift wiring in `runtime`
**Mission:** Get 14 example `.pp` programs from source text to running native code via Cranelift JIT, end-to-end.

This doc is the day-by-day contract that dijith must hit so the other 3 team members (Runtime/Stdlib, CLI/Diagnostics, Tooling) can do their work in parallel without blocking on dijith.

## Phase 0 — Foundation (Day 1)

**Goal:** Lock the data structures that cross crate boundaries. Other team members cannot start their TDD cycles without these contracts.

### Deliverable 0.1: AST + Span + Effect types
- **Files:** `crates/ast/src/span.rs`, `crates/ast/src/ast.rs`
- **What:** Span, Expr, Decl, Pattern, TypeExpr, BinOp
- **Blockers removed:** Every other crate imports from `ast`
- **Acceptance:** `cargo build` passes; existing ast tests still pass

### Deliverable 0.2: TokenKind with template + path-sep
- **Files:** `crates/lexer/src/lexer.rs` (TokenKind enum)
- **New variants:** `PathSep` (for `::`), `Backtick` (start of template), `TemplateHoleStart` (the `${`), `TemplateHoleEnd` (the closing `}`), `TemplateEnd` (closing backtick), `TemplateStr(&str)` (literal segments)
- **Blockers removed:** Parser can be written against a known token set
- **Acceptance:** TokenKind compiles, all old tokens still present

### Deliverable 0.3: MonoType + Effect variant
- **Files:** `crates/typechecker/src/types.rs`
- **What:** Add `MonoType::Effect(Box<MonoType>)`; add `Unit` literal `()`; add `TypeExpr::Unit` in ast
- **Blockers removed:** Typechecker can type `Effect<()>`
- **Acceptance:** All existing typechecker tests pass

### Deliverable 0.4: Bind with optional type
- **Files:** `crates/ast/src/ast.rs`
- **What:** Change `Decl::Bind` to `{ name, ty: Option<&TypeExpr>, value }`; remove `Decl::TypeSig` (folded into Bind)
- **Blockers removed:** Parser can produce inline-annotated bindings
- **Acceptance:** Existing AST tests adapted

---

## Phase 1 — Lexer (Days 1–2)

**Goal:** All 14 example programs tokenize without error.

### Deliverable 1.1: Pull-based Lexer
- **File:** `crates/lexer/src/lexer.rs`
- **API:** `Lexer::new(source) -> Self`; implements `Iterator<Item = Result<Token, LexError>>`
- **Spans:** byte-accurate (`Span::new(start, end)`)
- **Acceptance:** All Day-2 tests pass

### Deliverable 1.2: Test suite (11 tests)
- `lex_basic_identifiers` — keywords, idents, numbers
- `lex_dot_and_bind` — `.` and `<-`
- `lex_closure_syntax` — `(x) => x + 1`
- `lex_unicode_and_spans` — `let α = 5`, byte offsets
- `lex_string_with_escapes` — `"line\nnewline"`
- `lex_template_literal` — `` `Hello, ${name}!` `` produces `TemplateStr` + `TemplateHoleStart` + `Ident` + `TemplateHoleEnd` + `TemplateEnd`
- `lex_template_with_method_in_hole` — `` `${n.toString()}` ``
- `lex_path_separator` — `use stdlib::io` → `Ident("use") Ident("stdlib") PathSep Ident("io")`
- `lex_unterminated_string` — `"hello` → `LexError::UnterminatedString`
- `lex_unterminated_template` — `` `hello ${x `` → `LexError::UnterminatedTemplate`
- `lex_template_with_literal_dollar` — `` `\${not interpolated}` `` → single `TemplateStr`

### Handoff milestone: Day 2 EOD
- `crates/lexer` is feature-complete
- Member 2 (CLI) can integrate `Lexer::new()` into the pipeline shell
- Member 1 (Runtime) does not need lexer yet

---

## Phase 2 — Parser (Days 2–4)

**Goal:** All 14 example programs parse to a `Program` AST.

### Deliverable 2.1: Recursive-descent Parser
- **File:** `crates/parser/src/parser.rs`, `crates/parser/src/lib.rs`
- **API:**
  ```rust
  pub fn parse<'a>(bump: &'a Bump, tokens: &[Token]) -> Result<&'a Program<'a>, Vec<ParseError>>;
  ```
- **Arena:** AST nodes are `&'a Expr<'a>` etc., allocated in the bump
- **Error recovery:** Returns `Vec<ParseError>` and a partial AST, never panics

### Deliverable 2.2: Grammar coverage
- **Declarations:** `let name [: T] = expr`, `type Name [params] = type-expr`, `use path[::{a,b}|::name|::*]`
- **Expressions:** literals (int/float/str/template/bool), ident, application, lambda, binary, unary, `if`/`match`/`do`/`block`/record/tuple/array/index, field access, method call, template literal
- **Patterns:** wildcard, binding, literal (int/str), constructor, tuple, record, list (`[]`/`[x]`/`x:xs`)
- **Type expressions:** `()`, primitives, `Array<T>`, `Option<T>`, `Result<T,E>`, `Effect<T>`, function `(A, B) -> C`, record, tuple, generic application

### Deliverable 2.3: Test suite (14 tests)
- `parse_let_binding` — `let add = (a, b) => a + b`
- `parse_let_with_type_annotation` — `let add : (i32, i32) -> i32 = ...`
- `parse_function` — lambda forms
- `parse_method_chaining` — `xs.filter(f).map(g)`
- `parse_match_expression` — `match opt { Some(x) => x, None => 0 }`
- `parse_do_block_with_bind` — `do { x <- m; e }`
- `parse_record_literal` — `{ name: "Alice", age: 30 }`
- `parse_template_literal` — `` `Hello, ${name}!` `` → `Template { parts: [Literal("Hello, "), Expr(Ident("name")), Literal("!")] }`
- `parse_template_with_method_in_hole` — `` `${n.toString()}` ``
- `parse_sum_type` — `type Shape = | Circle(f64) | Rectangle(f64, f64) | Triangle(f64, f64, f64)`
- `parse_use_simple` — `use stdlib::io`
- `parse_brace_import` — `use stdlib::io::{println, readLine}`
- `parse_glob_import` — `use stdlib::io::*`
- `parse_nested_pattern` — `match (a, b) { ([], bs) => bs, (a:as_, b:bs) => ... }`
- `parse_list_pattern` — `match arr { [] => ..., [x] => ..., x:xs => ... }`
- `parse_partial_program_with_errors` — missing `}` produces `Vec<ParseError>` with span, still returns partial AST

### Handoff milestone: Day 4 EOD
- All 14 example `.pp` files parse cleanly
- `parse()` returns `Vec<ParseError>` and partial AST on bad input
- Member 2 can wire parser into CLI for early error display

---

## Phase 3 — Typechecker (Days 4–8) — THE HARD ONE

**Goal:** All 14 example programs type-check under HM with let-polymorphism.

### Deliverable 3.1: HM solver
- **Files:** `crates/typechecker/src/{types,unify,infer,env,error}.rs`
- **What:**
  - Full `unify()` with occurs check, record unification, tuple unification
  - `infer_expr` for all AST expression variants
  - `infer_pattern` for all AST pattern variants (including integer/str literal patterns, constructor patterns, list patterns)
  - `generalize` / `instantiate` for let-polymorphism
  - `MonoType::Effect`, `MonoType::Unit` wired through

### Deliverable 3.2: Built-in type environment
- **File:** `crates/typechecker/src/builtins.rs` (new)
- **What:** Pre-populated `TypeEnv` with:
  - Primitives: `i32`, `i64`, `f64`, `bool`, `str`, `()`
  - Generic types: `Option<T>`, `Result<T,E>`, `Array<T>` (constructors + method signatures)
  - Method tables: `Array<T>.map/filter/fold/len/concat/drop/take/head/tail/isEmpty/zip/sortBy/distinct`, `Option<T>.map/flatMap/unwrap/isSome/isNone`, `Result<T,E>.map/flatMap/mapErr`, primitives `toString()`
  - Combinators: `id`, `const`, `flip`, `compose`, `pipe`, `apply` (with explicit polymorphic schemes)

### Deliverable 3.3: Template-literal typing
- `Template { parts }` is type `str`
- Each `Expr` hole is type-checked
- If the type is `i32`/`i64`/`f64`/`bool`/`str` (auto-`toString`), type-checker inserts `.toString()` at IR-lowering time
- If the type is anything else (record, array, Option, Result), the user must call `.toString()` explicitly inside the hole
- Error: `TemplateHoleNotStringifiable { ty, span }` when the hole is a non-`str` type with no `toString` method

### Deliverable 3.4: Effect checking (light)
- `Effect<T>` is a normal type
- A function body that calls an effect-producing function (in the `io` module) must have its return type be `Effect<_>` — checked at `infer_decl` for the body of non-`do` functions
- For 0.1, the check is structural: a function declared `: T` is rejected if it contains an effect call and `T` is not `Effect<_>`. Deep effect tracking is 0.2.

### Deliverable 3.5: Test suite (26 tests)
- `infer_i32_literal`, `infer_str_literal`, `infer_bool_literal`
- `infer_lambda_identity` — `(x) => x` → `(?a) -> ?a`
- `infer_lambda_polymorphic` — `let id = (x) => x; id(5); id("hi")`
- `infer_let_polymorphism` — `let f = (x) => x; f(0); f("")`
- `infer_function_application` — `add(1, 2)` with `add: (i32, i32) -> i32`
- `infer_binary_arithmetic` — `a + b` with `a, b: i32` → `i32`
- `infer_comparison` — `a > b` → `bool`
- `infer_if_else_branches_unify` — `if c then 1 else "hi"` errors
- `infer_match_exhaustive` — `match opt { Some(x) => x, None => 0 }` exhausts `Option<i32>`
- `infer_match_int_literal_pattern` — `match n { 0 => ..., 1 => ..., n => ... }`
- `infer_match_str_literal_pattern` — `match s { "a" => ..., "b" => ..., _ => ... }`
- `infer_match_constructor_pattern` — `match opt { Some(x) => ..., None => ... }`
- `infer_match_list_pattern` — `match arr { [] => ..., [x] => ..., x:xs => ... }`
- `infer_match_wildcard` — `match x { _ => 0 }`
- `infer_recursive_function_with_annotation` — `let fact : (i32) -> i64 = (n) => match n { 0 => 1i64, n => n * fact(n - 1) }`
- `infer_array_literal` — `[1, 2, 3]` → `Array<i32>`
- `infer_array_map` — `xs.map(f)` with `xs : Array<T>`, `f : (T) -> U` → `Array<U>`
- `infer_array_fold` — `xs.fold(init, f)` with `xs : Array<T>`, `init : U`, `f : (U, T) -> U` → `U`
- `infer_array_concat` — `arr.concat(other)` → `Array<T>`
- `infer_option_map` — `Some(x).map(f)` → `Option<U>`
- `infer_result_flat_map` — `Ok(x).flatMap(f)` → `Result<U, E>`
- `infer_template_literal_with_hole` — `` `Hello, ${name}!` `` with `name: str` → `str`
- `infer_template_literal_with_int_hole` — `` `${n}` `` with `n: i32` → `str` (auto-`toString`)
- `infer_template_literal_with_method` — `` `${n.toString()}` ``
- `infer_method_call_on_record` — `user.name` → field access
- `infer_method_call_on_array` — `xs.filter(...)` method resolution

### Handoff milestone: Day 8 EOD
- All 14 example programs type-check without error
- `infer_program(&Program) -> Result<TypedProgram, Vec<TypeError>>` is the public entry point
- `TypedProgram` carries the AST + the type environment + any annotations, ready for IR lowering
- Member 2 can now integrate typechecker into the CLI's `check` subcommand

---

## Phase 4 — IR (Days 8–10)

**Goal:** All 14 example programs lower to a flat `IrModule` ready for Cranelift.

### Deliverable 4.1: IR data structures
- **File:** `crates/ir/src/lib.rs`
- **What:**
  - `IrModule { functions: Vec<IrFunction> }`
  - `IrFunction { name, params: Vec<IrParam>, body: IrBlock, return_type: IrType }`
  - `IrBlock { id: BlockId, instructions: Vec<(ValueId, IrInst)>, terminator: IrTerm }`
  - `IrInst` enum: `ConstI32(i32)`, `ConstI64(i64)`, `ConstF64(f64)`, `ConstBool(bool)`, `ConstStr(SmolStr)`, `BinOp(BinKind, ValueId, ValueId)`, `Cmp(CmpKind, ValueId, ValueId)`, `Call(SmolStr, Vec<ValueId>)`, `MakeArray(Vec<ValueId>)`, `IndexArray(ValueId, ValueId)`, `MakeRecord(Vec<(SmolStr, ValueId)>)`, `Field(ValueId, SmolStr)`, `Tag(u32, Vec<ValueId>)`, `Match { scrutinee: ValueId, arms: Vec<(Vec<IrPat>, BlockId)>, fallback: Option<BlockId> }`, `MakeClosure { name: SmolStr, captures: Vec<ValueId> }`, `CallClosure(ValueId, Vec<ValueId>)`, `Print(ValueId)`, `Println(ValueId)`, `ReadLine`, `Bind { value: ValueId, cont: BlockId, span }` (effect bind)
  - `IrTerm`: `Return(ValueId)`, `Branch(BlockId)`, `CondBranch { cond: ValueId, then: BlockId, else_: BlockId }`, `Unreachable`

### Deliverable 4.2: Lowering
- **File:** `crates/ir/src/lower.rs`
- **API:** `IrBuilder::lower_program(&TypedProgram, &TypeEnv) -> Result<IrModule, IrError>`
- Each top-level `let name = expr` becomes a function (or a constant for non-function bindings)
- `main` is identified and made the entry point
- Template literals with `i32`/`i64`/`f64`/`bool` holes have `.toString()` auto-inserted in the IR
- `do { x <- m; e }` lowers to a chain of `Bind` instructions linking the effect to the continuation block

### Deliverable 4.3: Test suite (7 tests)
- `lower_let_binding` — `let x = 5` → `IrFunction { name: "x", body: [ConstI32(5), Return] }`
- `lower_function_call` — `let f = (x) => x * 2; f(5)` → two functions, `main` calls `f`
- `lower_match` — `match n { 0 => ..., _ => ... }` → `Match` with two arms
- `lower_template_literal` — `` `Hello, ${name}!` `` → `ConstStr("Hello, ")` + `Call("toString", [name])` + concatenation
- `lower_do_block` — `do { x <- m; e }` → `Bind` chain
- `lower_use_resolved` — `use stdlib::io; io.readLine()` → `Call("io.readLine", [])`
- `lower_recursive_function` — `let fact = (n) => if n == 0 then 1 else n * fact(n - 1)` → tail-call to self

### Handoff milestone: Day 10 EOD
- `IrBuilder::lower_program` is the public entry point
- All 14 example programs lower to `IrModule` without error
- Member 1 (Runtime) can write a tree-walking interpreter against the IR
- Member 1 does NOT need the typechecker or Cranelift

---

## Phase 5 — Cranelift JIT (Days 10–12)

**Goal:** The simplest example program (`hello.pp`) prints `Hello, World!` via native code.

### Deliverable 5.1: Cranelift wiring
- **File:** `crates/runtime/src/jit.rs`
- **What:**
  - `JitCompiler::new() -> Self` — initializes `JITModule`, `Context`, `FunctionBuilderContext`
  - `JitCompiler::compile(&mut self, func: &IrFunction) -> Result<*const u8, JitError>` — lowers IR to Cranelift IR, defines the function, returns a function pointer
  - Built-in function calls (the `Call` instructions) lower to direct calls to `Value`-taking extern `"C"` trampolines
  - Allocations (Array, Record, Closure) use a simple bump allocator or `Arc`-shared heap

### Deliverable 5.2: Effect execution
- **File:** `crates/runtime/src/effect.rs`
- **What:** `Effect::execute(value: Value) -> Result<Value, RuntimeError>` — dispatches the `Effect` value to its `BuiltinFunction::execute` impl, which performs the actual IO (or returns the next `Effect` for monadic chaining — but for 0.1 we only have terminal effects)

### Deliverable 5.3: Runtime entry point
- **File:** `crates/runtime/src/runtime.rs`
- **What:** `Runtime::run(module: &IrModule) -> Result<i32, RuntimeError>` — finds `main`, JIT-compiles it, calls it via FFI, returns exit code
- For non-Cranelift dev mode, the same `Runtime::run` can dispatch to a tree-walking interpreter (Member 1's deliverable) if `target-cpu=interp` env var is set

### Deliverable 5.4: Test suite (8 tests)
- `jit_compiles_simple_function` — `() => 42` compiles and returns 42
- `jit_compiles_arithmetic` — `(a, b) => a + b` with i32 args
- `jit_compiles_string_return` — `() => "hello"` returns a `Value::Str`
- `jit_executes_builtin_print` — `println("hi")` writes to stdout
- `jit_runs_hello_program` — full pipeline on `hello.pp`
- `jit_handles_recursion` — `factorial(10)` returns `3628800`
- `jit_handles_closure_capture` — `makeAdder(5)(10)` returns `15`
- `jit_runs_all_14_example_programs` — fixture test, expected output for each

### Handoff milestone: Day 12 EOD
- `pipe-lang run hello.pp` works end-to-end on the developer's machine
- All 14 example programs run under Cranelift with expected output
- The hard compiler internals are done

---

## Phase 6 — Integration & Polish (Days 12–14)

**Goal:** `cargo test` is green; `cargo clippy -- -D warnings` is green; pre-commit gates pass.

### Deliverable 6.1: Wire into the session
- `crates/cli/src/session.rs::run_pipeline` calls into dijith's crates in order
- The `CompileResult.diagnostics` collects errors from all stages (lex, parse, typecheck, lower)

### Deliverable 6.2: Test fixtures
- Move the 14 example programs from `example-programs/` into a `tests/fixtures/` directory of each compiler crate
- Add `#[test]` wrappers that lex → parse → typecheck → lower → (interpret) and assert success
- Each example becomes the integration test for that compiler stage

### Deliverable 6.3: Pre-commit gates
- `cargo fmt --check` clean
- `cargo clippy -- -D warnings` clean
- `cargo test` all green
- `cargo doc --no-deps` generates clean docs

---

## Summary: dijith's Deadlines

| Day | Deliverable | Recipient unblocked |
|---|---|---|
| 1 | AST contracts, TokenKind, MonoType::Effect, Bind.ty: Option | All |
| 2 | Lexer complete (11 tests) | Member 2 (CLI) |
| 4 | Parser complete (14 tests, 14 example files parse) | Member 2 (CLI), early typechecker work |
| 8 | Typechecker complete (26 tests, 14 example files typecheck) | Member 2 (`check` subcommand), Member 1 (full type info in IR) |
| 10 | IR complete (7 tests, 14 example files lower) | Member 1 (interpreter) |
| 12 | Cranelift JIT complete (8 tests, 14 example programs run) | Member 2 (`run` subcommand) |
| 14 | Integration polish, all 14 programs pass through full pipeline | Release |

**Total tests dijith owns: 66** (11 lexer + 14 parser + 26 typechecker + 7 IR + 8 JIT). The end-to-end test is: `pipe-lang run example-programs/hello.pp` prints `Hello, World!` and exits 0.
