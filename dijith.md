# The Lead Architect's Week 1 Execution Document

## Phase 1: Repo Setup & Core Dependencies (Day 1)

First, set up the repo and pull in the dependencies for _your_ crates. Do not write logic yet — just establish the skeleton and install packages.

```bash
# 1. Initialize Workspace
cargo init --lib --name pipe-lang
cd pipe-lang

# 2. Scaffold Your Crates (Plus shared AST)
mkdir crates
cargo new --lib crates/ast
cargo new --lib crates/lexer
cargo new --lib crates/parser
cargo new --lib crates/ir
cargo new --lib crates/runtime
cargo new --lib crates/stdlib
cargo new --lib crates/diagnostics
cargo new crates/cli

# 3. Setup Workspace Cargo.toml
cat <<EOT > Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"
EOT
```

### Install Dependencies via CLI

Run these from the root of the workspace.

**For `ast` (Memory & Strings)**

```bash
cargo add bumpalo -p ast --features collections  # Arena allocation for AST
cargo add smol_str -p ast                        # String interning for fast cloning
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

Before writing the Lexer, you must define the Types in `crates/ast/`. This is the single source of truth for you and your team.

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
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub fn empty(pos: usize) -> Self { Self { start: pos, end: pos } }
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
    pub fn len(self) -> usize { self.end - self.start }
    pub fn source_text(self, source: &str) -> &str { &source[self.start..self.end] }
}
```

### 2. The Token Architecture

Design the Token as a lightweight struct. Do not put strings inside keywords; only use `&str` for literals/identifiers.

```rust
// crates/lexer/src/lexer.rs
use ast::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Type, Let, In, If, Then, Else, Match, With, Do, Effect, Return, True, False,
    // Operators
    Arrow,          // =>
    Bind,           // <-
    Plus, Minus, Star, Slash, Percent,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or, Not,
    Assign,         // =
    Dot,            // . (field access and method calls)
    Comma, Colon, Semicolon, Underscore,
    // Delimiters
    OpenParen, CloseParen, OpenBrace, CloseBrace,
    OpenBracket, CloseBracket,
    // Literals (raw source text — parser handles type interpretation)
    Int(String),     // 42, 42i32, 255u8
    Float(String),   // 3.14, 3.14f64
    Str(String),
    // Identifier
    Ident(String),
    // Trivial tokens (preserved for tooling, skipped by parser)
    Whitespace(String),
    Comment(String),
    Newline,
    // Special
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
// crates/lexer/src/lexer.rs
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
    done: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self { ... }
    fn advance(&mut self) -> Option<(usize, char)> { ... }
    fn peek(&mut self) -> Option<char> { ... }
}

// Implement Iterator so the Parser can just call `.next()`
// Always yields at least one Eof token at the end
impl<'a> Iterator for Lexer<'a> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        // Skip whitespace/comments, match char, yield Token
        todo!()
    }
}
```

### 2. Lexer Test Driven Development

Write these tests _first_ in `crates/lexer/src/lexer.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_basic_identifiers() {
        let mut lexer = Lexer::new("let add = (a, b) => a + b");
        assert_eq!(lexer.next().unwrap().kind, TokenKind::Let);
        assert_eq!(lexer.next().unwrap().kind, TokenKind::Ident("add".into()));
        // ... test rest of tokens
    }

    #[test]
    fn lex_dot_and_bind() {
        let mut lexer = Lexer::new("x.filter \n y <- effect");
        // Assert Dot (.) and Bind (<-) are recognized
    }

    #[test]
    fn lex_unicode_and_spans() {
        let source = "let α = 5"; // Unicode handling is critical
        let mut lexer = Lexer::new(source);
        // Test that spans are byte-accurate, not char-accurate
    }

    #[test]
    fn lex_closure_syntax() {
        let mut lexer = Lexer::new("(x) => x + 1");
        // Assert OpenParen, Ident, CloseParen, Arrow, Ident, Plus, Int
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
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Program<'a> {
    pub decls: bumpalo::collections::Vec<'a, Decl<'a>>,
}

#[derive(Debug, Clone)]
pub enum Decl<'a> {
    /// let name = expr
    Bind {
        name: &'a str,
        value: &'a Expr<'a>,
        span: Span,
    },
    /// let name : Type
    TypeSig {
        name: &'a str,
        ty: &'a TypeExpr<'a>,
        span: Span,
    },
    /// type Name = TypeExpr
    TypeAlias {
        name: &'a str,
        params: bumpalo::collections::Vec<'a, &'a str>,
        rhs: &'a TypeExpr<'a>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    Int(i64, Span),
    Float(f64, Span),
    Str(&'a str, Span),
    Bool(bool, Span),
    Ident(&'a str, Span),
    Application {
        func: &'a Expr<'a>,
        args: bumpalo::collections::Vec<'a, &'a Expr<'a>>,
        span: Span,
    },
    Lambda {
        params: bumpalo::collections::Vec<'a, Param<'a>>,
        return_type: Option<&'a TypeExpr<'a>>,
        body: &'a Expr<'a>,
        span: Span,
    },
    Binary {
        op: BinOp,
        left: &'a Expr<'a>,
        right: &'a Expr<'a>,
        span: Span,
    },
    FieldAccess {
        object: &'a Expr<'a>,
        field: &'a str,
        span: Span,
    },
    Match {
        subject: &'a Expr<'a>,
        arms: bumpalo::collections::Vec<'a, MatchArm<'a>>,
        span: Span,
    },
    // ... other variants
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
    fn parse_method_chaining() {
        let arena = Bump::new();
        let mut parser = Parser::new(
            Lexer::new("users.filter(|x| x.age >= 18).map(|x| x.name)"),
            &arena,
        );
        let ast = parser.parse_program().unwrap();
        // Assert that method calls are desugared to function applications:
        // map(filter(users, |x| x.age >= 18), |x| x.name)
    }

    #[test]
    fn parse_let_binding() {
        let arena = Bump::new();
        let mut parser = Parser::new(
            Lexer::new("let add = (a:i32, b:i32):i64 => a + b"),
            &arena,
        );
        let ast = parser.parse_program().unwrap();
        // Assert: Decl::Bind { name: "add", value: Lambda { ... } }
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
use ast::SmolStr;

// Typed, flat instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

pub enum Instruction {
    // Signed integer constants
    ConstI8(i8), ConstI16(i16), ConstI32(i32), ConstI64(i64),
    // Unsigned integer constants
    ConstU8(u8), ConstU16(u16), ConstU32(u32), ConstU64(u64), ConstUsize(usize),
    // Float constants
    ConstF32(f32), ConstF64(f64),
    // Other constants
    ConstBool(bool), ConstStr(SmolStr),
    // Arithmetic
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    Rem(ValueId, ValueId),
    // Comparison
    Eq(ValueId, ValueId), Ne(ValueId, ValueId),
    Lt(ValueId, ValueId), Le(ValueId, ValueId),
    Gt(ValueId, ValueId), Ge(ValueId, ValueId),
    // Logical
    And(ValueId, ValueId), Or(ValueId, ValueId), Not(ValueId),
    // Control flow
    Call(SmolStr, Vec<ValueId>),
    Return(ValueId),
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

    // Test case: Compile a simple `let return_5 = () => 5` to test the pipeline
}
```

---

## Your Deliverables by the End of Week 1

1. **Workspace & Build Pipeline:** Everything compiles with `cargo build`.
2. **`ast` crate:** Spans, Token enums, and Arena-based AST node definitions are solid.
3. **`lexer` crate:** All TDD tests pass. Converts strings to `Token` streams perfectly.
4. **`parser` crate:** All TDD tests pass. Correctly handles dot operator and builds a `bumpalo` AST.
5. **`ir` crate:** The SSA structures are defined.
6. **`runtime` crate:** The Cranelift module can be instantiated without panicking.

By setting up `crates/ast` correctly on Day 1, you unblock Member 1 (Typechecker). They don't need your Parser to work; they can manually construct `bumpalo` AST nodes in their tests and start writing the HM unification algorithm immediately!
