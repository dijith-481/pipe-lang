# INTERNAL API SPECIFICATION — `pipe-lang`

**Status:** LOCKED  
**Purpose:** This document is the absolute source of truth for all internal boundaries, crate interactions, and data structures. It serves as the formal contract between team members. 
**Rule:** You may not alter these public signatures or cross-boundary data structures without team consensus. If your implementation cannot fulfill this contract, the implementation must change, not the contract.

---

## 1. Crate Dependency Graph & Data Flow

The compiler is a strictly unidirectional pipeline. Cyclic dependencies between crates are forbidden.

```text
source (str)
  │
  ▼ [lexer]
Vec<Token>
  │
  ▼ [parser]  <-- Allocates into bumpalo::Bump
&'a Program<'a>
  │
  ▼ [typechecker] <-- Resolves variables, infers types (Union-Find)
TypedProgram<'a>
  │
  ▼ [ir]      <-- Flattens AST into SSA, hoists closures
IrModule
  │
  ▼ [runtime/jit] <-- Cranelift compiles IR to machine code
Native Function Pointer
```

---

## 2. Global Diagnostics Contract (`crates/diagnostics`)

**Owner:** LSP / CLI Team
All stages of the compiler must fail gracefully and return a standardized error type. Stages like parsing and typechecking should attempt to recover and return multiple errors.

```rust
// crates/diagnostics/src/lib.rs

use ast::span::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
pub enum CompilerError {
    #[error("Lexer error: {msg}")]
    LexError { msg: String, #[label] span: Span },

    #[error("Parse error: expected {expected}, found {found}")]
    ParseError { expected: String, found: String, #[label] span: Span },

    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String, #[label] span: Span },

    #[error("Unbound variable: {name}")]
    UnboundVariable { name: String, #[label] span: Span },

    #[error("Non-exhaustive pattern match")]
    NonExhaustiveMatch { #[label] span: Span },

    #[error("IR Lowering error: {msg}")]
    IrError { msg: String, #[label] span: Span },

    #[error("JIT Compilation error: {msg}")]
    JitError { msg: String },
}
```

---

## 3. Frontend AST & Lexer (`crates/ast`, `crates/lexer`, `crates/parser`)

**Owner:** dijith
The AST is heavily lifetime-bound to an Arena (`bumpalo::Bump`) to ensure zero-cost allocations and eliminate deep-clone overhead. 

### Lexer API
```rust
// crates/lexer/src/lib.rs
pub fn tokenize<'a>(source: &'a str) -> Result<Vec<Token<'a>>, Vec<CompilerError>>;

pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

// Minimalist TokenKind. Note: NO `do`, `yield`, `return`, `with`.
pub enum TokenKind<'a> {
    Let, Type, Match, If, Else, True, False, Use,
    Ident(&'a str),
    Int(&'a str), Float(&'a str), Str(&'a str),
    Backtick, TemplateStr(&'a str), TemplateHoleStart, TemplateHoleEnd, TemplateEnd,
    Arrow, /* => */
    // ... basic operators and delimiters (+, -, (, {, etc.)
}
```

### AST API
```rust
// crates/ast/src/ast.rs
use bumpalo::Bump;

pub struct Program<'a> {
    pub decls: bumpalo::collections::Vec<'a, Decl<'a>>,
}

pub enum Decl<'a> {
    // let name [: Type] = expr
    Bind { name: &'a str, ty: Option<&'a TypeExpr<'a>>, value: &'a Expr<'a>, span: Span },
    // type Name<T> = ...
    TypeAlias { name: &'a str, params: bumpalo::collections::Vec<'a, &'a str>, rhs: &'a TypeExpr<'a>, span: Span },
    // use path::to::module
    Use { path: bumpalo::collections::Vec<'a, &'a str>, span: Span },
}

pub enum Expr<'a> {
    IntLiteral(&'a str, Span),
    FloatLiteral(&'a str, Span),
    Str(&'a str, Span),
    Bool(bool, Span),
    Ident(&'a str, Span),
    
    // String interpolation: `Hello ${name}`
    Template { parts: bumpalo::collections::Vec<'a, TemplatePart<'a>>, span: Span },
    
    // (a) => a + 1
    Lambda { params: bumpalo::collections::Vec<'a, Param<'a>>, body: &'a Expr<'a>, span: Span },
    
    // f(x) or a.f()
    App { func: &'a Expr<'a>, args: bumpalo::collections::Vec<'a, &'a Expr<'a>>, span: Span },
    
    // { stmt; stmt; expr }
    Block { stmts: bumpalo::collections::Vec<'a, Stmt<'a>>, result: &'a Expr<'a>, span: Span },
    
    If { cond: &'a Expr<'a>, then_branch: &'a Expr<'a>, else_branch: &'a Expr<'a>, span: Span },
    Match { subject: &'a Expr<'a>, arms: bumpalo::collections::Vec<'a, MatchArm<'a>>, span: Span },
    
    Record { fields: bumpalo::collections::Vec<'a, (&'a str, &'a Expr<'a>)>, span: Span },
    FieldAccess { obj: &'a Expr<'a>, field: &'a str, span: Span },
    
    Binary { op: BinOp, left: &'a Expr<'a>, right: &'a Expr<'a>, span: Span },
}

pub enum Stmt<'a> {
    Let { pattern: &'a Pattern<'a>, value: &'a Expr<'a>, span: Span },
    Expr(&'a Expr<'a>),
}
```

### Parser API
```rust
// crates/parser/src/lib.rs
pub fn parse<'a>(bump: &'a Bump, tokens: &[Token<'a>]) -> Result<&'a Program<'a>, Vec<CompilerError>>;
```

---

## 4. Typechecker (`crates/typechecker`)

**Owner:** dijith
The Typechecker receives the AST and applies Hindley-Milner inference. It returns a `TypedProgram` that contains the original AST and an environment mapping every AST Span to a fully resolved type (critical for the LSP).

### Types & Inference Core
```rust
// crates/typechecker/src/types.rs

// Types are allocated in an Arena or via Rc to prevent O(N^2) deep cloning during unification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonoType<'arena> {
    I32, I64, F64, Bool, Str, Unit,
    Var(TypeId), // References a node in the Union-Find structure
    Func { params: Vec<&'arena MonoType<'arena>>, ret: &'arena MonoType<'arena> },
    Array(&'arena MonoType<'arena>),
    Record(BTreeMap<String, &'arena MonoType<'arena>>),
    // Sum types, Generic applications (including Effect<T>)
    App { name: String, args: Vec<&'arena MonoType<'arena>> }, 
}

// Union-Find structure for $O(1)$ amortized type resolution
pub struct TypeEnv<'arena> {
    // ... Union-Find disjoint set arrays for TypeId resolution
    // ... Scope mapping for lexically bound variables
}
```

### Typechecker API
```rust
// crates/typechecker/src/lib.rs
pub struct TypedProgram<'a, 'arena> {
    pub ast: &'a Program<'a>,
    pub env: TypeEnv<'arena>,
    // Maps the Span of any expression in the AST to its inferred type. (Used by LSP Hover).
    pub type_map: HashMap<Span, &'arena MonoType<'arena>>,
}

pub fn typecheck<'a, 'arena>(
    ast: &'a Program<'a>, 
    type_arena: &'arena bumpalo::Bump
) -> Result<TypedProgram<'a, 'arena>, Vec<CompilerError>>;
```

---

## 5. Intermediate Representation (`crates/ir`)

**Owner:** dijith (Lowering) & Backend Team (Consuming)
The IR is flat, SSA-lite, and optimized for cache locality. Large enum variants are strictly boxed. 

```rust
// crates/ir/src/lib.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

pub struct IrModule {
    pub functions: Vec<IrFunction>,
}

pub struct IrFunction {
    pub name: String,
    pub params: Vec<ValueId>,
    pub blocks: Vec<BasicBlock>,
    pub entry_block: BlockId,
}

pub struct BasicBlock {
    pub id: BlockId,
    pub insts: Vec<(ValueId, Instruction)>,
    pub terminator: Terminator,
}

// Size optimized: Large payloads MUST be Boxed to keep Instruction size <= 32 bytes.
pub enum Instruction {
    ConstI32(i32),
    ConstF64(f64),
    ConstBool(bool),
    ConstStr(String),
    
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Eq(ValueId, ValueId),
    
    // Boxed to save space on the enum tag
    Call(Box<CallData>),            
    MakeClosure(Box<ClosureData>),  
    RecordAlloc(Box<RecordData>),   
    TagAlloc(Box<TagData>),         
}

pub struct CallData { pub target: String, pub args: Vec<ValueId> }
pub struct ClosureData { pub func_name: String, pub captures: Vec<ValueId> }
pub struct RecordData { pub fields: Vec<(String, ValueId)> }
pub struct TagData { pub tag_id: u32, pub payload: Vec<ValueId> }

pub enum Terminator {
    Return(ValueId),
    Jump(BlockId),
    Branch { cond: ValueId, then_blk: BlockId, else_blk: BlockId },
    Switch { discriminant: ValueId, arms: Vec<(u32, BlockId)>, fallback: BlockId },
}
```

### IR Lowering API
```rust
// crates/ir/src/lib.rs
pub fn lower(typed_prog: &TypedProgram) -> Result<IrModule, CompilerError>;
```

---

## 6. Runtime, Memory Model, & Stdlib (`crates/runtime`, `crates/stdlib`)

**Owner:** Runtime/Backend Team
The runtime manages memory deterministically using `Arc` (Atomic Reference Counting). There is NO garbage collector. Native functions implemented in Rust bridge to the JIT via `BuiltinFunction`.

### Value Enum (Memory Model)
```rust
// crates/runtime/src/value.rs
use std::sync::Arc;
use std::collections::BTreeMap;

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Value {
    // Unboxed primitives (passed by value on the stack)
    I32(i32),
    I64(i64),
    F64(f64),
    Bool(bool),
    Unit,
    
    // Boxed complex types (Heap allocated, ARC tracked)
    Str(Arc<str>),
    Array(Arc<[Value]>),
    Record(Arc<BTreeMap<String, Value>>),
    Closure(Arc<ClosureRuntimeData>),
    
    // Sum Types
    Tag { tag: u32, payload: Arc<[Value]> },
    
    // Effects are just generic data descriptions
    Effect(Arc<dyn BuiltinFunction>),
}

pub struct ClosureRuntimeData {
    pub func_ptr: usize, // JIT Address
    pub captures: Arc<[Value]>,
}
```

### Standard Library FFI Bridge
```rust
// crates/runtime/src/bridge.rs
pub trait BuiltinFunction: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn arity(&self) -> usize;
    fn execute(&self, args: &[Value]) -> Result<Value, String>;
}
```

### JIT Compilation API
```rust
// crates/runtime/src/jit.rs
pub struct JitCompiler { ... }

impl JitCompiler {
    pub fn new() -> Self;
    // Compiles the flat IR down to Cranelift machine code.
    pub fn compile(&mut self, module: &IrModule) -> Result<(), CompilerError>;
    
    // Returns a pointer to the compiled `main` function.
    pub fn get_main(&self) -> Result<extern "C" fn() -> Value, CompilerError>;
}
```
