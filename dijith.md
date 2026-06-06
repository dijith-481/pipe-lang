# dijith — The Compiler Internals Owner

**Crate ownership:** `crates/lexer`, `crates/parser`, `crates/typechecker`, `crates/ir`, Cranelift wiring in `crates/runtime`

**Mission:** Get 14 example `.pp` programs from source text to running native code via Cranelift JIT, end-to-end. Owns the full compiler pipeline from `&str` to function pointer.

## Why this allocation

The 4 modules above form a single, sequential pipeline:

```
.pp → [lexer] → [parser] → [typechecker] → [ir] → [cranelift] → running program
```

The output of each stage is the input to the next. Putting all 4 on one person means:
- One person owns the AST shape and the IR shape
- One person controls the contract with the Cranelift backend
- One person is responsible for the type-driven optimizations (HM inference informs IR)
- No cross-team coordination on compiler internals — the other 3 members consume the public API

The other 3 team members do **easy, parallel** work that doesn't touch compiler internals: a tree-walking interpreter, CLI plumbing, and tooling. See `member1.md`, `member2.md`, `member3.md`.

## What dijith delivers

The day-by-day plan is in `deliverables.md`. Summary:

| Phase | Days | Deliverable | Tests |
|---|---|---|---|
| 0 | 1 | AST, TokenKind, MonoType::Effect, Bind.ty contracts | — |
| 1 | 1–2 | Lexer (hand-written, pull-based, template literals, `::` path sep) | 11 |
| 2 | 2–4 | Parser (recursive descent, arena AST, error recovery) | 14 |
| 3 | 4–8 | Typechecker (HM with let-polymorphism, Effect<T>, pattern typing) | 26 |
| 4 | 8–10 | IR (flat SSA-lite, lowered from typed AST) | 7 |
| 5 | 10–12 | Cranelift JIT (IR → Cranelift IR → native code, FFI for builtins) | 8 |
| 6 | 12–14 | Integration polish, pre-commit gates green | — |
| **Total** | **14 days** | **`pipe-lang run example-programs/hello.pp` prints `Hello, World!`** | **66 tests** |

## Public APIs dijith exposes to other team members

These are the contracts other members will consume. They must be stable from the day listed.

### Day 2: `crates/lexer`
```rust
pub struct Lexer<'a> { /* ... */ }
impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self;
    // Implements Iterator<Item = Result<Token<'a>, LexError>>
}
pub struct Token<'a> { pub kind: TokenKind<'a>, pub span: Span }
pub enum TokenKind<'a> { /* see deliverables.md Day 1 */ }
```

### Day 4: `crates/parser`
```rust
pub fn parse<'a>(bump: &'a Bump, tokens: &[Token]) -> Result<&'a Program<'a>, Vec<ParseError>>;
```

### Day 8: `crates/typechecker`
```rust
pub fn infer_program<'a>(program: &'a Program<'a>) -> Result<TypedProgram<'a>, Vec<TypeError>>;
pub struct TypedProgram<'a> {
    pub ast: &'a Program<'a>,
    pub env: TypeEnv,
    pub type_for_span: HashMap<Span, MonoType>,
}
```

### Day 10: `crates/ir`
```rust
pub fn lower_program(typed: &TypedProgram) -> Result<IrModule, IrError>;
pub struct IrModule { pub functions: Vec<IrFunction> }
```

### Day 12: `crates/runtime::jit` (Cranelift)
```rust
pub struct JitCompiler { /* Cranelift module + context */ }
impl JitCompiler {
    pub fn new() -> Self;
    pub fn compile(&mut self, func: &IrFunction) -> Result<*const u8, JitError>;
}
pub fn run(module: &IrModule) -> Result<i32, RuntimeError>;
```

## The 14 example programs (integration target)

All 14 programs in `example-programs/*.pp` must lex, parse, type-check, lower, JIT, and run with expected output by Day 14. They are the spec.

```
hello.pp               hello world, prelude println
factorial.pp           recursive + tail-recursive
fibonacci.pp           naive + tail-recursive
io-effects.pp          do-block with stdlib::io
closures.pp            higher-order, fold-based counter
option-result.pp       Option/Result methods
patterns.pp            sum types, exhaustive match
records.pp             record types, field access, record update
state-machine.pp       AppState transition fold
sorting.pp             quicksort, mergesort (recursive fns)
higher-order.pp        map, filter, fold chains
generics.pp            polymorphic combinators
ascii-art.pp           recursive string repeat
game-of-life.pp        2D grid, cell predicates
```

Every program uses only features dijith's stack must implement. No program is "out of scope" for 0.1.

## Pre-commit gates (run before every commit)

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

These three commands are dijith's responsibility to keep green. When the other 3 members merge into main, the gates still hold.

## Working with the other team members

- **Member 1 (Runtime + Stdlib)** — does NOT need dijith's code; works off the IR shape and `Value` enum. Once dijith lands the IR on Day 10, Member 1 can write the tree-walking interpreter against it. Member 1's interpreter is the dev-mode fallback when Cranelift is unavailable.
- **Member 2 (CLI + Diagnostics + Resolver + Prelude)** — wires the CLI. Can integrate the lexer on Day 2, parser on Day 4, typechecker on Day 8, full pipeline on Day 12. Their module resolver consumes the public typechecker API.
- **Member 3 (Tooling + Docs + Examples)** — completely independent. Tree-sitter grammar is a separate repo; LSP server consumes the public APIs; docs are docs.

## What dijith does NOT do

- Tree-walking interpreter (Member 1)
- Stdlib built-in function implementations (Member 1)
- Module resolver implementation (Member 2)
- CLI subcommand wiring (Member 2)
- Error rendering with miette (Member 2)
- Prelude definition (Member 2)
- Tree-sitter grammar (Member 3)
- LSP server (Member 3)
- README, getting-started, language tour docs (Member 3)
- More example programs beyond the 14 (Member 3)

The boundary is sharp: anything outside `lexer`, `parser`, `typechecker`, `ir`, or the Cranelift wiring in `runtime` belongs to someone else.

## Reference: the language spec

The full syntax and type system is in `pipe-lang.md`. Key points dijith must implement:

- **Template strings** `` `Hello, ${name}!` `` for string interpolation; plain `"hello"` allowed when no `${}` is needed
- **No `++` operator**; array concatenation is `arr.concat(other)`
- **Inline type annotations** `let name : T = expr`; HM inference for non-recursive, explicit for recursive
- **Effect types** `Effect<T>`; do-blocks are pure desugaring to monadic bind (Haskell IO model)
- **Pure-by-default**; mutations are rejected by the typechecker
- **Module resolver** is `Member 2`'s deliverable; dijith's typechecker calls into it during `infer_decl` for `Use`

The spec is the contract. If the spec and the implementation disagree, the spec wins.
