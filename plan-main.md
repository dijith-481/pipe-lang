# Functional Language Project Plan

A working, interesting functional language with basic JIT, type inference, and clean separation of pure/effectful code.

- A Fun learning project
- syntax inspired by Rust, TypeScript.
- pure functional language
- a working prototype in 1 month time

---

## Language Design

### Core Philosophy

```
Pure Functions (Data + Calculations)  |  Effects (Actions/IO)
─────────────────────────────────────────────────────────────
No side effects                       |  Isolated, explicit
Composable                            |  State machine model
JIT optimizable                       |  Controlled execution
```

### Syntax Direction (TS + Rust Feel)

```typescript
// Type definitions (clean, no noise)
type UserId = i32
type Email  = str

// Record types (product types)
type User = {
  id    : UserId
  email : Email
  age   : u8
}

// Sum types (tagged unions)
type Result<T, E> =
  | Ok(T)
  | Err(E)

type Option<T> =
  | Some(T)
  | None

// Functions use `let` binding with arrow syntax
// name(params):ReturnType => body
let add = (a:i32, b:i32):i64 => a + b

// Currying via application
let addFive = add(5)

// Dot operator for method chaining and field access
let processUsers = (users:Array<User>):Array<Email> =>
  users
    .iter()
    .filter((u) => u.age >= 18)
    .map((u) => u.email)
    .distinct()

// Inline closures use `(params) => expr` syntax
let doubled = items.map((x) => x * 2)
let adults = users.filter((u) => u.age >= 18)

// Block closures for multi-step logic
let result = items.filter((item) => {
    let score = item.calculate()
    score > 100
})

// Pattern matching
let describe = (opt:Option<i32>):String => match opt {
  Some(x) => `Got: ${x.toString()}`
  None    => "Nothing"
}

// Type signature on separate line (no let keyword)
let transition : AppState -> Event -> Effect<AppState>
let transition = (state, event) => match (state, event) {
  (Idle,        Login(creds))  => authenticate(creds).map(Ready)
  (Ready(user), Logout)        => Effect.pure(Idle)
  (Loading(id), Timeout)       => Effect.pure(Failed(TimeoutError))
  _                            => Effect.pure(state)
}
```

### Effect System (Pure vs IO Separation)

```typescript
// Pure computation, no marker needed, it's the default
let factorial = (n:i32):i32 => match n {
  0 => 1
  n => n * factorial(n - 1)
}

// IO must be marked, returns an Effect type
// Effect<T> is your IO wrapper
let readUser : Effect<Option<User>>
let readUser = do {
  line <- IO.readLine()
  id   <- Str.parsei32(line).map(Option.fromResult())
  DB.findUser(id)              // also returns Effect<Option<User>>
}

// State machine for IO handling
// States are explicit, transitions are typed
type AppState =
  | Idle
  | Loading(RequestId)
  | Ready(User)
  | Failed(Error)

let transition : AppState -> Event -> Effect<AppState>
let transition = (state, event) => match (state, event) {
  (Idle,        Login(creds))  => authenticate(creds).map(Ready)
  (Ready(user), Logout)        => Effect.pure(Idle)
  (Loading(id), Timeout)       => Effect.pure(Failed(TimeoutError))
  _                            => Effect.pure(state)
}
```

### Type System Design

```typescript
// Hindley-Milner base with explicit annotations optional
// Generic types with constraints

// Typeclass-style traits
trait Functor<F> {
  map : <A, B>(F<A>, A -> B) => F<B>
}

trait Foldable<F> {
  fold : <A, B>(F<A>, B, |B, A| B) => B
}

// Built-in implementations for Array, Option, Result
// Your language ships these

// No implicit coercion, ever
// i32 and f64 are distinct
// Explicit conversions
let x : i32 = 5
let y : f64 = f64::from(x)  // explicit, not x + 0 + 0

// Union types for flexibility (TS-inspired)
type StringOri32 = String | i32

// Intersection for composition
type Named     = { name : str }
type Aged      = { age  : i32 }
type Person    = Named & Aged
```

---

## Architecture

### Compiler Pipeline

```
Source Code
    │
    ▼
┌─────────┐
│  Lexer  │  Hand-written in Rust, fast, no regex
└─────────┘
    │ Token Stream
    ▼
┌─────────┐
│ Parser  │  Recursive descent, produces CST first
└─────────┘
    │ CST
    ▼
┌─────────┐
│   AST   │  Lower CST to typed AST nodes
└─────────┘
    │
    ▼
┌──────────────┐
│  Name Res    │  Resolve symbols, scopes, modules
└──────────────┘
    │
    ▼
┌──────────────┐
│  Type Infer  │  HM inference + constraint solving
└──────────────┘
    │ Typed AST
    ▼
┌──────────────┐
│  Effect Chk  │  Verify pure/effect boundaries
└──────────────┘
    │
    ▼
┌──────────────┐
│     IR       │  Your own simple IR (SSA-lite)
└──────────────┘
    │
    ▼
┌──────────────┐
│  JIT / AOT   │  Cranelift backend (realistic choice)
└──────────────┘
```

### Module Structure (What You Initialize)

```
lang/
├── Cargo.toml          (workspace)
├── crates/
│   ├── ast/            // Shared AST types (Day 1)
│   │   ├── src/
│   │   │   ├── span.rs
│   │   │   └── ast.rs
│   │
│   ├── lexer/          // Person 1
│   │   ├── src/
│   │   │   ├── error.rs
│   │   │   └── lexer.rs
│   │
│   ├── parser/         // Person 2
│   │   ├── src/
│   │   │   ├── error.rs
│   │   │   └── parser.rs
│   │
│   ├── typechecker/    // Person 3
│   │   ├── src/
│   │   │   ├── types.rs
│   │   │   ├── infer.rs
│   │   │   ├── unify.rs
│   │   │   └── env.rs
│   │
│   ├── ir/             // Person 4
│   │   ├── src/
│   │   │   ├── ir.rs
│   │   │   ├── lower.rs
│   │   │   └── opt.rs
│   │
│   ├── runtime/        // You wire this
│   │   ├── src/
│   │   │   ├── jit.rs      (Cranelift)
│   │   │   ├── builtins.rs
│   │   │   └── effect.rs
│   │
│   ├── stdlib/         // Shared effort
│   │   ├── src/
│   │   │   ├── list.rs
│   │   │   ├── option.rs
│   │   │   ├── result.rs
│   │   │   └── io.rs
│   │
│   ├── diagnostics/    // Error aggregation
│   │   └── src/
│   │       └── errors.rs
│   │
│   └── cli/            // You
│       └── src/
│           └── main.rs
│
├── lsp/                // Separate crate
│   └── src/
│       └── main.rs     (tower-lsp)
│
└── tree-sitter-lang/   // Separate repo
    ├── grammar.js
    └── queries/
```

---

## Memory Model (No GC, No Unsafe Chaos)

### Strategy: Region-Based + Reference Counting for Effects

```
Pure functions:
  - Stack allocated where possible
  - Values are copied or moved (Rust semantics)
  - No heap unless necessary (Vec, large structs)
  - Compiler tracks ownership through the IR

Effect boundary:
  - Arc<T> for shared state crossing effect boundaries
  - Arenas for short-lived allocations within a computation
  - Drop-based cleanup, deterministic

Collections (Array, etc):
  - Persistent-ish via structural sharing using Rc in pure context
  - Clone-on-write for mutations at effect boundary
```

```rust
// In your runtime, this is the Value representation
// No GC tag, no header, flat where possible

#[repr(C)]
pub enum Value {
    // Signed integers
    I8(i8), I16(i16), I32(i32), I64(i64),
    // Unsigned integers
    U8(u8), U16(u16), U32(u32), U64(u64), Usize(usize),
    // Floats
    F32(f32), F64(f64),
    // Other primitives
    Bool(bool),
    Str(Arc<str>),          // immutable, shared
    Array(Arc<[Value]>),     // immutable list, ref counted
    Record(Arc<RecordData>),
    Closure(Arc<Closure>),
    Effect(Arc<dyn EffectFn>),
    Tag(u32, Arc<[Value]>), // sum type variant
}

// Closure captures are explicit
pub struct Closure {
    pub func    : FuncPtr,
    pub captures: Arc<[Value]>,  // captured environment
    pub arity   : u32,
}
```

### Why This Works Without GC

- Immutable values + reference counting = safe sharing
- `Arc` drop is deterministic
- Pure functions never escape their stack frame without explicit return
- Effects explicitly model the boundary where state lives
- No cycles in pure data (enforce this in type checker - no recursive values without explicit `Rec` wrapper)

---

## JIT Strategy

### Use Cranelift (Not LLVM)

Cranelift is the right call here.

```
LLVM:
  - Complex API
  - Long compile times for the compiler itself
  - Overkill for 21 days
  - Hard to embed cleanly

Cranelift:
  - Pure Rust
  - Designed for JIT use cases (used in Wasmtime)
  - Simpler IR
  - Fast compile times
  - Good enough output quality
```

```toml
[dependencies]
cranelift-codegen  = "0.132"
cranelift-frontend = "0.132"
cranelift-jit      = "0.132"
cranelift-module   = "0.132"
```

### JIT Compilation Flow

```rust
// Simplified sketch of your JIT module

pub struct Jit {
    builder_ctx : FunctionBuilderContext,
    ctx         : codegen::Context,
    module      : JITModule,
}

impl Jit {
    pub fn compile(&mut self, func: &IrFunction) -> *const u8 {
        // Lower your IR to Cranelift IR
        // This is the main work of the ir/ crate
        let mut builder = FunctionBuilder::new(
            &mut self.ctx.func,
            &mut self.builder_ctx
        );

        self.lower_function(&mut builder, func);

        builder.finalize();
        self.module.define_function(func.id, &mut self.ctx)?;
        self.module.finalize_definitions()?;
        self.module.get_finalized_function(func.id)
    }
}
```

### Optimization Passes (Realistic for Timeline)

```
Pass 1: Inline small functions         (huge win for FP style)
Pass 2: Specialize on known types      (remove dynamic dispatch)
Pass 3: Dead code elimination
Pass 4: Constant folding
Pass 5: Tail call optimization         (critical for recursion)

Skip for now:
  - Loop optimization (no loops anyway)
  - Escape analysis  (future work)
  - SIMD             (future work)
```

---

## 21-Day Sprint Plan

### Week 1: Foundation (Days 1-7) - Parallel Work

```
Day 1 (You):
  - Create repo, workspace Cargo.toml
  - Define shared AST types in crates/ast
  - Define IR types in crates/ir
  - Define error types in crates/diagnostics
  - Write trait contracts for each module
  - Document the token set, operator precedence table
  - Everyone pulls and starts

Person 1 (Lexer) Days 1-5:
  - All tokens defined
  - Span tracking (file, line, col, byte offset)
  - String interning from day 1
  - Unicode identifiers
  - Good error messages on invalid chars
  - 100% test coverage on token types

Person 2 (Parser) Days 1-5:
  - Recursive descent
  - Full expression grammar
  - Pattern matching syntax
  - Type annotation syntax
  - Error recovery (don't stop on first error)
  - Produces clean AST

Person 3 (Type Checker) Days 2-7:
  - Types enum (Mono, Poly, Effect, etc)
  - Unification algorithm
  - Type environment
  - HM inference for core expressions
  - Effect type checking (pure vs IO)

Person 4 (IR) Days 2-7:
  - IR node definitions
  - Lowering from typed AST to IR
  - Basic optimization passes
  - IR printer for debugging

Day 5 (You):
  - Integrate lexer + parser
  - Make sure they communicate correctly
  - Set up CI (GitHub Actions, cargo test + cargo clippy)

Day 7 (Everyone):
  - Integration checkpoint
  - Can we lex, parse, typecheck hello world?
  - Fix integration issues
```

### Week 2: Core Features (Days 8-14)

```
Days 8-10:
  - JIT backend wiring (you + person 4)
  - Cranelift integration
  - Can compile and run a pure function
  - Runtime Value type finalized

Days 8-10 (Person 1 + 2):
  - Switch to stdlib
  - Implement Array, Option, Result in terms of your IR
  - map, filter, fold, flatMap as built-in or library functions

Days 11-12:
  - Effect system runtime
  - IO monad execution model
  - do-notation desugaring in parser/AST lowering

Days 13-14:
  - Pattern matching compilation
  - Sum type dispatch in JIT
  - Tail call optimization in JIT
```

### Week 3: Polish + LSP (Days 15-21)

```
Days 15-16:
  - Tree-sitter grammar
  - Highlighting queries
  - This is mostly grammar.js work, doable fast

Days 17-18:
  - LSP server (tower-lsp)
  - Hover types
  - Go to definition
  - Diagnostics from type checker
  - Completion for record fields

Days 19-20:
  - Error message quality
  - Standard library completeness
  - Edge case fixing

Day 21:
  - Demo programs
  - README
  - Basic docs
```

---

## Standard Library Design

### What to Ship Day 1

```typescript
// Array operations
Array.map      : <A, B>(Array<A>, A -> B) => Array<B>
Array.filter   : <A>(Array<A>, A -> bool) => Array<A>
Array.flatMap  : <A, B>(Array<A>, A -> Array<B>) => Array<B>
Array.fold     : <A, B>(Array<A>, B, (B, A) -> B) => B
Array.reduce   : <A>(Array<A>, (A, A) -> A) => Option<A>
Array.find     : <A>(Array<A>, A -> bool) => Option<A>
Array.zip      : <A, B>(Array<A>, Array<B>) => Array<(A, B)>
Array.take     : <A>(Array<A>, i32) => Array<A>
Array.drop     : <A>(Array<A>, i32) => Array<A>
Array.head     : <A>(Array<A>) => Option<A>
Array.tail     : <A>(Array<A>) => Array<A>
Array.len      : <A>(Array<A>) => i32
Array.isEmpty  : <A>(Array<A>) => bool
Array.distinct : <A: Eq>(Array<A>) => Array<A>
Array.sort     : <A: Ord>(Array<A>) => Array<A>
Array.sortBy   : <A, B: Ord>(Array<A>, A -> B) => Array<A>
Array.groupBy  : <A, B: Eq>(Array<A>, A -> B) => Map<B, Array<A>>

// Option
Option.map     : <A, B>(Option<A>, A -> B) => Option<B>
Option.flatMap : <A, B>(Option<A>, A -> Option<B>) => Option<B>
Option.orElse  : <A>(Option<A>, Option<A>) => Option<A>
Option.unwrap  : <A>(Option<A>, A) => A  // with default
Option.isSome  : <A>(Option<A>) => bool

// Result
Result.map      : <T, E, U>(Result<T,E>, T -> U) => Result<U,E>
Result.flatMap  : <T, E, U>(Result<T,E>, T -> Result<U,E>) => Result<U,E>
Result.mapErr   : <T, E, F>(Result<T,E>, E -> F) => Result<T,F>
Result.recover  : <T, E>(Result<T,E>, E -> T) => T

// IO / Effect
IO.print    : (str) => Effect<Unit>
IO.println  : (str) => Effect<Unit>
IO.readLine : () => Effect<str>
IO.readFile : (Path) => Effect<Result<str, IOError>>
IO.writeFile: (Path, str) => Effect<Result<Unit, IOError>>

// Function combinators
compose : <A, B, C>(B -> C, A -> B) => A -> C
pipe    : <A, B, C>(A -> B, B -> C) => A -> C
id      : <A>(A) => A
const   : <A, B>(A) => B -> A
flip    : <A, B, C>(A -> B -> C) => B -> A -> C
```

---

## Tree-Sitter Grammar Sketch

```javascript
// grammar.js - the key rules

module.exports = grammar({
  name: "pipe_lang",

  rules: {
    source_file: ($) => repeat($._top_level),

    _top_level: ($) => choice($.type_def, $.let_def, $.trait_def),

    // type User = { id: i32, name: str }
    type_def: ($) =>
      seq("type", $.identifier, optional($.type_params), "=", $._type_expr),

    // let name = (params):ReturnType => body
    let_def: ($) =>
      seq("let", $.identifier, optional(seq(":", $._type_expr)), "=", $._expr),

    // Lambda: (params) => body
    lambda: ($) => seq(
      "(", optional($.typed_param_list), ")",
      optional(seq(":", $._type_expr)),
      "=>", $._expr,
    ),

    // match x { Pat => expr, ... }
    match_expr: ($) => seq("match", $._expr, "{", repeat($.match_arm), "}"),

    match_arm: ($) => seq($._pattern, "=>", $._expr),

    // Method call: expr.method(args)
    method_call: ($) => seq($._expr, ".", $.identifier, "(", optional($.arg_list), ")"),

    // Field access: expr.field
    field_access: ($) => prec.left(1, seq($._expr, ".", $.identifier)),

    // do { x <- effect; ... }
    do_block: ($) => seq("do", "{", repeat($.do_stmt), "}"),

    do_stmt: ($) => choice(seq($.identifier, "<-", $._expr), $._expr),
  },
});
```

---

## LSP Implementation

```rust
// lsp/src/main.rs - skeleton with tower-lsp

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client  : Client,
    // Hold a handle to your compiler state
    db      : Arc<Mutex<CompilerDb>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams)
        -> Result<InitializeResult>
    {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider         : Some(HoverProviderCapability::Simple(true)),
                definition_provider    : Some(OneOf::Left(true)),
                completion_provider    : Some(CompletionOptions::default()),
                diagnostic_provider    : Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions::default()
                )),
                text_document_sync     : Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn hover(&self, params: HoverParams)
        -> Result<Option<Hover>>
    {
        // Query your type checker for the type at position
        let db  = self.db.lock().await;
        let pos = params.text_document_position_params.position;
        let typ = db.type_at(pos);

        Ok(typ.map(|t| Hover {
            contents: HoverContents::Scalar(
                MarkedString::String(t.display())
            ),
            range: None,
        }))
    }

    // ... hover, goto_def, completion, diagnostics
}
```

---

## Key Technical Decisions Summary

| Concern     | Decision                       | Reason                                     |
| ----------- | ------------------------------ | ------------------------------------------ |
| JIT Backend | Cranelift                      | Pure Rust, simpler API, fast iteration     |
| Memory      | RC + Arenas, no GC             | Deterministic, simpler runtime             |
| Type System | HM + row types                 | Enough power, implementable in time        |
| Effects     | Effect<T> wrapper              | Clear boundary, no monad transformer stack |
| Parsing     | Hand-written recursive descent | Full error control, no grammar conflicts   |
| Strings     | Arc<str> interned              | Shared, immutable, cheap clone             |
| Collections | Persistent via Arc<[]>         | Immutable FP style without GC              |
| Syntax      | Dot operator, no pipeline      | Rust/TS style, easier to type              |
| Closures    | `\|params\|` for inline        | Distinguishes from function definitions    |
| Functions   | `let name = (params) => body`  | `let` required for all bindings            |

---

## Biggest Risks + Mitigations

```
Risk 1: Type checker takes too long
  Mitigation: Start with simple HM, no rank-N types
              Add features after it works

Risk 2: JIT integration is hard to debug
  Mitigation: Have an interpreter fallback first
              Get programs running, then JIT them

Risk 3: Team coordination on shared AST types
  Mitigation: You define ast.rs on day 1, it's read-only
              Changes go through you

Risk 4: Effect system is complicated
  Mitigation: Start as a tag/wrapper, not a full effect system
              Effect<T> is just a description, runtime runs it

Risk 5: 21 days isn't enough
  Mitigation: Prioritize: working language > LSP > tree-sitter
              LSP and tree-sitter are optional for a working language
```

---

## First Day Checklist (For You)

```bash
# 1. Create workspace
cargo init --lib --name pipe-lang
cd pipe-lang

# 2. Create all crates
cargo new --lib crates/ast
cargo new --lib crates/lexer
cargo new --lib crates/parser
cargo new --lib crates/ir
cargo new --lib crates/runtime
cargo new --lib crates/stdlib
cargo new --lib crates/diagnostics
cargo new crates/cli

# 3. Workspace Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

# 4. Write these files before anyone else touches code:
#    - crates/ast/src/span.rs      (Span struct)
#    - crates/ast/src/ast.rs       (all AST nodes)
#    - crates/ir/src/ir.rs         (all IR nodes)
#    - crates/diagnostics/src/errors.rs (CompilerError enum)

# 5. Write a test for each module that currently fails
#    This defines done for each person
```

The most important thing you can do on day 1 is define the data structures that cross module boundaries. Everything else can be parallelized once those are locked.

What part do you want to go deeper on first - the type system design, the JIT setup, or the effect system?
