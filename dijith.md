# The Lead Architect's Week 1 Execution Document

## Phase 1: Repo Setup & Core Dependencies (Day 1)

First, set up the repo and pull in the dependencies for _your_ crates. Do not write logic yet—just establish the skeleton and install packages.

```bash
# 1. Initialize Workspace
cargo new --name lang_core lang
cd lang
rm src/main.rs

# 2. Scaffold Your Crates (Plus shared AST)
mkdir crates
cd crates
cargo new --lib ast
cargo new --lib lexer
cargo new --lib parser
cargo new --lib ir
cargo new --lib runtime
cd ..

# 3. Setup Workspace Cargo.toml
cat <<EOT > Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"
EOT
```

### Install Dependencies via CLI

Run these from the root of the workspace.

**For `ast` & `lexer` (Memory & Strings)**

```bash
cargo add bumpalo -p ast      # Arena allocation for AST/IR
cargo add smol_str -p ast     # String interning for fast cloning
cargo add smol_str -p lexer
```

_Intuition:_ ASTs involve thousands of small nodes and identifiers. Using `Box` and `String` fragments memory and slows down the compiler. `bumpalo` allows you to allocate the entire AST in one contiguous memory arena and drop it instantly. `smol_str` prevents heap allocations for identifiers under 22 bytes.

**For `parser` (Errors & Diagnostics)**

```bash
cargo add thiserror -p parser
cargo add miette -p parser
```

**For `runtime` & `ir` (JIT)**

```bash
cargo add cranelift-codegen -p runtime
cargo add cranelift-frontend -p runtime
cargo add cranelift-jit -p runtime
cargo add cranelift-module -p runtime
```

---

## Phase 2: Defining the Common Domain (Day 1)

Before writing the Lexer, you must define the Types in `crates/ast/src/lib.rs`. This is the single source of truth for you and your team.

### 1. Spans (Source Mapping)

Every token and AST node must know where it came from for JIT debugging and LSP support.

```rust
// crates/ast/src/span.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
```

### 2. The Token Architecture

Design the Token as a lightweight struct. Do not put strings inside keywords; only use `SmolStr` for literals/identifiers.

```rust
// crates/lexer/src/token.rs
use ast::span::Span;
use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Type, Match, Do,
    // Operators
    Arrow,          // =>
    Pipe,           // |>
    Bind,           // <-
    // Identifiers & Literals
    Ident(SmolStr),
    Int(i64),
    String(SmolStr),
    // Punctuation
    OpenParen, CloseParen, OpenBrace, CloseBrace,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
```

---

## Phase 3: The Lexer TDD Plan (Days 1 - 2)

Your lexer must be a hand-written, pull-based iterator over a `&str` or `&[u8]`. No regex.

### 1. The API Boundary

```rust
// crates/lexer/src/lib.rs
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self { ... }
    fn advance(&mut self) -> Option<(usize, char)> { ... }
    fn peek(&mut self) -> Option<char> { ... }
}

// Implement Iterator so the Parser can just call `.next()`
impl<'a> Iterator for Lexer<'a> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        // Skip whitespace/comments, match char, yield Token
        todo!()
    }
}
```

### 2. Lexer Test Driven Development

Write these tests _first_ in `crates/lexer/src/lib.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_basic_identifiers() {
        let mut lexer = Lexer::new("add = (a, b) => a + b");
        assert_eq!(lexer.next().unwrap().kind, TokenKind::Ident("add".into()));
        assert_eq!(lexer.next().unwrap().kind, TokenKind::Assign);
        // ... test rest of tokens
    }

    #[test]
    fn lex_pipeline_and_bind() {
        let mut lexer = Lexer::new("x |> map \n y <- effect");
        // Assert Pipe (|>) and Bind (<-) are recognized
    }

    #[test]
    fn lex_unicode_and_spans() {
        let source = "let α = 5"; // Unicode handling is critical
        let mut lexer = Lexer::new(source);
        // Test that spans are byte-accurate, not char-accurate
    }
}
```

_Goal for Day 2:_ Make all Lexer tests pass.

---

## Phase 4: The Parser TDD Plan (Days 3 - 4)

Your parser takes the Lexer and an Arena (`bumpalo::Bump`) and outputs AST references.

### 1. The AST Architecture

Notice the use of `&'a` lifetimes. The AST lives in the arena.

```rust
// crates/ast/src/ast.rs
use bumpalo::Bump;
use smol_str::SmolStr;
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Program<'a> {
    pub declarations: bumpalo::collections::Vec<'a, Decl<'a>>,
}

#[derive(Debug, Clone)]
pub enum Decl<'a> {
    Function {
        name: SmolStr,
        params: bumpalo::collections::Vec<'a, SmolStr>,
        body: &'a Expr<'a>,
        span: Span,
    },
    // Type aliases, etc.
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    Int(i64, Span),
    Ident(SmolStr, Span),
    Application {
        func: &'a Expr<'a>,
        args: bumpalo::collections::Vec<'a, &'a Expr<'a>>,
        span: Span,
    },
    Pipeline {
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
    },
    Match {
        subject: &'a Expr<'a>,
        arms: bumpalo::collections::Vec<'a, MatchArm<'a>>,
        span: Span,
    }
}
```

### 2. The Parser API

```rust
// crates/parser/src/lib.rs
use ast::ast::{Program, Expr};
use lexer::{Lexer, Token};
use bumpalo::Bump;

pub struct Parser<'a, 'source> {
    lexer: Lexer<'source>,
    arena: &'a Bump,
    current: Token,
    previous: Token,
}

impl<'a, 'source> Parser<'a, 'source> {
    pub fn new(lexer: Lexer<'source>, arena: &'a Bump) -> Self { ... }

    // Pratt parsing for expressions
    fn parse_expression(&mut self, precedence: Precedence) -> Result<&'a Expr<'a>, ParseError> {
        todo!()
    }
}
```

### 3. Parser Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn parse_pipeline_operator() {
        let arena = Bump::new();
        let mut parser = Parser::new(Lexer::new("users |> filter |> map"), &arena);
        let ast = parser.parse_expression(Precedence::Lowest).unwrap();

        // Assert that the AST formed is properly left-associative:
        // Pipeline(Pipeline(users, filter), map)
    }

    #[test]
    fn parse_match_expression() {
        let arena = Bump::new();
        let source = "match opt { Some(x) => x, None => 0 }";
        let mut parser = Parser::new(Lexer::new(source), &arena);
        // Assert arms are parsed correctly
    }
}
```

---

## Phase 5: IR Design & Skeleton (Days 5 - 6)

The AST is a tree. IR must be flat, essentially a typed SSA (Static Single Assignment) form, making it trivial to map to Cranelift.

### 1. IR Architecture

```rust
// crates/ir/src/lib.rs
use smol_str::SmolStr;

// Typed, flat instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

pub enum Instruction {
    ConstInt(i64),
    Add(ValueId, ValueId),
    Call(SmolStr, Vec<ValueId>),
    Phi(ValueId, ValueId), // Crucial for pattern matching/branching
    MakeEffect(ValueId),   // Wraps a pure value in an effect marker
}

pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<(ValueId, Instruction)>,
    pub terminator: Terminator,
}

pub enum Terminator {
    Return(ValueId),
    Branch(BlockId),
    CondBranch { condition: ValueId, true_block: BlockId, false_block: BlockId },
}
```

### 2. The Lowering API (AST -> IR)

```rust
pub struct IrBuilder {
    // current block, variable mappings, etc.
}

impl IrBuilder {
    /// Takes a type-checked AST and flattens it into IR
    pub fn lower_function(&mut self, ast: &ast::Decl) -> IrFunction {
        todo!()
    }
}
```

---

## Phase 6: JIT/Cranelift Initialization (Day 7)

On Day 7, you initialize the Cranelift engine. You won't compile complex code yet, just verify you can translate your `IrFunction` into Cranelift IR and generate a pointer.

```rust
// crates/runtime/src/jit.rs
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::Module;

pub struct JitCompiler {
    module: JITModule,
    ctx: cranelift_codegen::Context,
    builder_ctx: cranelift_frontend::FunctionBuilderContext,
}

impl JitCompiler {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let builder = JITBuilder::with_isa(
            isa_builder.finish(settings::Flags::new(flag_builder)).unwrap(),
            cranelift_module::default_libcall_names(),
        );

        Self {
            module: JITModule::new(builder),
            ctx: cranelift_codegen::Context::new(),
            builder_ctx: cranelift_frontend::FunctionBuilderContext::new(),
        }
    }

    // Test case: Compile a simple `fn return_5() -> Int` to test the pipeline
}
```

---

## Your Deliverables by the End of Week 1

1. **Workspace & Build Pipeline:** Everything compiles with `cargo build`.
2. **`ast` crate:** Spans, Token enums, and Arena-based AST node definitions are solid.
3. **`lexer` crate:** All TDD tests pass. Converts strings to `Token` streams perfectly.
4. **`parser` crate:** All TDD tests pass. Correctly handles associativity of `|>` and builds a `bumpalo` AST.
5. **`ir` crate:** The SSA structures are defined.
6. **`runtime` crate:** The Cranelift module can be instantiated without panicking.

By setting up `crates/ast` correctly on Day 1, you unblock Member 1 (Typechecker). They don't need your Parser to work; they can manually construct `bumpalo` AST nodes in their tests and start writing the HM unification algorithm immediately!
