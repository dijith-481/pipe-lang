# Member 3 — Phase 2: Tooling, LSP, Diagnostics & Tree-Sitter

**Crate Ownership:** `crates/cli`, `crates/diagnostics`, `crates/pipe-lang-lsp` (new)  
**Timeline:** 3 days (parallel with Member 1, Member 2)  
**Goal:** Diagnostics contract compliance, LSP server for editor integration, tree-sitter grammar for syntax highlighting, CLI improvements (timing, exit codes, flags), and miette graphical rendering. All example programs show proper error messages.

---

## Current State

| Area | Status | Priority |
|---|---|---|
| **Diagnostics crate** | 8 `CompilerError` variants — 3 are non-contract (`TypeError`, `RuntimeError`, `EffectError`), 4 contract variants missing (`TypeMismatch`, `UnboundVariable`, `NonExhaustiveMatch`, `JitError`), `ParseError` fields don't match spec | **1 — Fix first** |
| **Error rendering** | Uses `{diag:?}` debug format, not miette graphical | **2** |
| **CLI** | 3 subcommands (check/compile/run), basic pipeline, no `--time`, `--emit-asm`, `-o`, or `lsp` subcommand | **2** |
| **Exit codes** | Always returns 0 or 1, no distinction between compilation/runtime/IO errors | **2** |
| **LSP** | Does not exist | **2** |
| **Tree-sitter** | Does not exist | **3** |

---

## Diagnostics Changes: Contract Compliance

The `api-contracts.md` defines these `CompilerError` variants:

```rust
pub enum CompilerError {
    LexError          { msg: String, span: Span },
    ParseError        { expected: String, found: String, span: Span },
    TypeMismatch      { expected: String, got: String, span: Span },  // MISSING
    UnboundVariable   { name: String, span: Span },                    // MISSING
    NonExhaustiveMatch { span: Span },                                 // MISSING
    IrError           { msg: String, span: Span },
    JitError          { msg: String },                                 // MISSING
    Multiple          { count: usize, span: Option<Span> },
}
```

Current has: `LexError`, `ParseError` (bad fields), `TypeError` (wrong — should be `TypeMismatch`), `IrError`, `RuntimeError` (wrong — should be `JitError`), `EffectError` (not in contract), `IoError` (not in contract — keep as convenience), `Multiple`.

### Task 3.1: Rewrite `CompilerError` in `crates/diagnostics/src/errors.rs`

**Changes:**

```
REMOVE:
  TypeError      { span, msg }          → split into 3 contract variants
  RuntimeError   { span, msg }          → replaced by JitError
  EffectError    { span, msg }          → removed (not in contract)

ADD:
  TypeMismatch   { expected: String, got: String, span: Span }
  UnboundVariable { name: String, span: Span }
  NonExhaustiveMatch { span: Span }
  JitError       { msg: String }

MODIFY:
  ParseError:
    BEFORE: { span, msg: String, expected: Vec<String> }
    AFTER:  { expected: String, found: String, span: Span }
    Note: Keep old constructor for backwards compat; add new one.

KEEP:
  LexError       { span, msg }
  IrError        { span, msg }
  Multiple       { count, span }
  IoError(String)                   — keep as convenience (not in contract but useful)
```

**Implementation:**

```rust
#[derive(Debug, Clone, thiserror::Error, Diagnostic)]
pub enum CompilerError {
    #[error("Lex error: {msg}")]
    #[diagnostic(code(pipe_lang::lex))]
    LexError {
        #[label]
        span: Span,
        msg: String,
    },

    #[error("Parse error: expected {expected}, found {found}")]
    #[diagnostic(code(pipe_lang::parse))]
    ParseError {
        expected: String,
        found: String,
        #[label("expected {expected}, found {found}")]
        span: Span,
    },

    #[error("Type mismatch: expected {expected}, got {got}")]
    #[diagnostic(code(pipe_lang::type_mismatch))]
    TypeMismatch {
        expected: String,
        got: String,
        #[label("expected {expected}, got {got}")]
        span: Span,
    },

    #[error("Unbound variable: {name}")]
    #[diagnostic(code(pipe_lang::unbound))]
    UnboundVariable {
        name: String,
        #[label("`{name}` is not defined in this scope")]
        span: Span,
    },

    #[error("Non-exhaustive pattern match")]
    #[diagnostic(code(pipe_lang::non_exhaustive_match))]
    NonExhaustiveMatch {
        #[label("This match does not cover all possible values")]
        span: Span,
    },

    #[error("IR error: {msg}")]
    #[diagnostic(code(pipe_lang::ir))]
    IrError {
        #[label]
        span: Span,
        msg: String,
    },

    #[error("JIT error: {msg}")]
    #[diagnostic(code(pipe_lang::jit))]
    JitError {
        msg: String,
    },

    #[error("I/O error: {0}")]
    #[diagnostic(code(pipe_lang::io))]
    IoError(String),

    #[error("Encountered {count} error(s)")]
    #[diagnostic(code(pipe_lang::multiple))]
    Multiple {
        count: usize,
        #[label]
        span: Option<Span>,
    },
}
```

### Task 3.2: Add constructor helpers

```rust
impl CompilerError {
    // Keep existing helpers
    pub fn lex_error(span: Span, msg: impl Into<String>) -> Self { ... }
    pub fn ir_error(span: Span, msg: impl Into<String>) -> Self { ... }

    // New helpers
    pub fn parse_error(expected: impl Into<String>, found: impl Into<String>, span: Span) -> Self {
        Self::ParseError {
            expected: expected.into(),
            found: found.into(),
            span,
        }
    }

    pub fn type_mismatch(expected: impl Into<String>, got: impl Into<String>, span: Span) -> Self {
        Self::TypeMismatch {
            expected: expected.into(),
            got: got.into(),
            span,
        }
    }

    pub fn unbound_variable(name: impl Into<String>, span: Span) -> Self {
        Self::UnboundVariable {
            name: name.into(),
            span,
        }
    }

    pub fn non_exhaustive_match(span: Span) -> Self {
        Self::NonExhaustiveMatch { span }
    }

    pub fn jit_error(msg: impl Into<String>) -> Self {
        Self::JitError { msg: msg.into() }
    }
}
```

### Task 3.3: Update `span()` method

```rust
pub fn span(&self) -> Option<Span> {
    match self {
        CompilerError::LexError { span, .. }
        | CompilerError::ParseError { span, .. }
        | CompilerError::TypeMismatch { span, .. }
        | CompilerError::UnboundVariable { span, .. }
        | CompilerError::NonExhaustiveMatch { span, .. }
        | CompilerError::IrError { span, .. } => Some(*span),
        CompilerError::JitError { .. } | CompilerError::IoError(_) => None,
        CompilerError::Multiple { span, .. } => *span,
    }
}
```

### Task 3.4: Update `From` impls

**`From<lexer::error::LexError>`** — keep same (just calls `lex_error`)

**`From<parser::error::ParseError>`** — update to use new `parse_error` constructor:

```rust
impl From<parser::error::ParseError> for CompilerError {
    fn from(err: parser::error::ParseError) -> Self {
        match err {
            parser::error::ParseError::UnexpectedToken { expected, found, span } => {
                // expected is Vec<String> — join with " or "
                let expected_str = expected.join(" or ");
                CompilerError::parse_error(expected_str, found, span)
            }
            parser::error::ParseError::UnexpectedEof { expected, span } => {
                let expected_str = expected.join(" or ");
                CompilerError::parse_error(expected_str, "end of file".to_string(), span)
            }
            parser::error::ParseError::ExpectedExpression { span } => {
                CompilerError::parse_error("expression", "something else".to_string(), span)
            }
            parser::error::ParseError::Unimplemented { span } => {
                CompilerError::parse_error("unimplemented", "".to_string(), span)
            }
        }
    }
}
```

### Task 3.5: Add `From<TypeError>` impl

The typechecker's `TypeError` (from `crates/typechecker/src/error.rs`) has structured variants. Add a `From` impl that maps each variant to the correct `CompilerError`:

```rust
impl From<typechecker::TypeError> for CompilerError {
    fn from(err: typechecker::TypeError) -> Self {
        match err {
            typechecker::TypeError::UnificationFailed { expected, got, span } => {
                CompilerError::type_mismatch(expected.to_string(), got.to_string(), span)
            }
            typechecker::TypeError::UnboundVariable { name, span } => {
                CompilerError::unbound_variable(name, span)
            }
            typechecker::TypeError::NonExhaustiveMatch { span } => {
                CompilerError::non_exhaustive_match(span)
            }
            typechecker::TypeError::ArityMismatch { expected, got, span } => {
                CompilerError::type_mismatch(
                    format!("{expected} arguments"),
                    format!("{got} arguments"),
                    span,
                )
            }
            typechecker::TypeError::AnnotationConflict { annotation, inferred, span } => {
                CompilerError::type_mismatch(annotation.to_string(), inferred.to_string(), span)
            }
            typechecker::TypeError::FieldNotFound { field, span } => {
                CompilerError::type_mismatch("record with field", field, span)
            }
            typechecker::TypeError::InfiniteType { var, ty, span } => {
                CompilerError::TypeMismatch {
                    expected: format!("finite type"),
                    got: format!("Type var {var} occurs in {ty}"),
                    span,
                }
            }
            typechecker::TypeError::NumericOverflow { ty, span } => {
                CompilerError::TypeMismatch {
                    expected: format!("value within range of {ty}"),
                    got: "overflow".to_string(),
                    span,
                }
            }
        }
    }
}
```

### Task 3.6: Update session.rs to use new variants

**File:** `crates/cli/src/session.rs`

Before (line 168):
```rust
let err = CompilerError::TypeError {
    span: first.span(),
    msg: first.to_string(),
};
```

After (using `From<TypeError>`):
```rust
let err: CompilerError = first.into();
```

Before (line 222):
```rust
CompilerError::RuntimeError {
    span: None,
    msg: e.to_string(),
}
```

After:
```rust
CompilerError::jit_error(e.to_string())
```

Same for line 238.

### Task 3.7: Update all tests

The diagnostics test file has tests for the old variants. Update them:
- `type_error_display` → `type_mismatch_display`
- `runtime_error_with_optional_span` → `jit_error_display`
- Add tests for `unbound_variable_display`, `nonexhaustive_match_display`, `type_mismatch_source_code_label`

### Tests for Task 3.1–3.7

```rust
#[test]
fn type_mismatch_display() {
    let err = CompilerError::type_mismatch("i32", "str", Span::new(5, 10));
    let msg = format!("{err}");
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("i32"));
    assert!(msg.contains("str"));
}

#[test]
fn unbound_variable_display() {
    let err = CompilerError::unbound_variable("x", Span::new(0, 1));
    let msg = format!("{err}");
    assert!(msg.contains("Unbound variable"));
    assert!(msg.contains("x"));
}

#[test]
fn nonexhaustive_match_display() {
    let err = CompilerError::non_exhaustive_match(Span::new(10, 20));
    let msg = format!("{err}");
    assert!(msg.contains("Non-exhaustive pattern match"));
}

#[test]
fn jit_error_display() {
    let err = CompilerError::jit_error("segfault at 0x0");
    let msg = format!("{err}");
    assert!(msg.contains("JIT error"));
    assert!(msg.contains("segfault"));
}

#[test]
fn jit_error_has_no_span() {
    let err = CompilerError::jit_error("oops");
    assert!(err.span().is_none());
}

#[test]
fn parse_error_uses_expected_found() {
    let err = CompilerError::parse_error("`(`", "identifier", Span::new(1, 2));
    let msg = format!("{err}");
    assert!(msg.contains("expected"));
    assert!(msg.contains("`(`"));
    assert!(msg.contains("identifier"));
}

#[test]
fn from_typechecker_typeerror_converts_correctly() {
    use typechecker::TypeError;
    let tc_err = TypeError::UnboundVariable { name: "foo".into(), span: Span::new(3, 6) };
    let ce: CompilerError = tc_err.into();
    assert!(matches!(ce, CompilerError::UnboundVariable { .. }));
    assert!(format!("{ce}").contains("foo"));
}
```

---

## Day 1 — Mid (Hours 4–8): Miette Graphical Rendering

### Task 3.8: Add miette graphical rendering to session.rs

The current pipeline prints errors using Debug format. Replace with miette's graphical output:

**In `crates/cli/src/session.rs`, after the pipeline runs:**

```rust
fn report_diagnostics(diagnostics: &[SourceDiagnostic]) {
    for diag in diagnostics {
        eprintln!("{:?}", miette::Report::new(diag.clone()));
    }
}

fn report_error(error: &SourceDiagnostic) {
    eprintln!("{:?}", miette::Report::new(error.clone()));
}
```

Update the pipeline to call these instead of `println!("{diag:?}")` or implicit Debug.

**Make `CompilerResult` use miette for rendering:**

```rust
impl CompileResult {
    pub fn eprint_to_stderr(&self) {
        for diag in &self.diagnostics {
            eprintln!("{:?}", miette::Report::new(diag.clone()));
        }
    }
}
```

### Task 3.9: Ensure `SourceDiagnostic` renders correctly

The `SourceDiagnostic` struct already has:
- `#[source_code]` on `src: NamedSource<Arc<str>>`
- `#[diagnostic(transparent)]` on `error: CompilerError`

When the miette crate renders `Report::new(diag)`, it should show:
1. The error message (from `Display` impl of `CompilerError`)
2. The source code (from `NamedSource`)
3. The span label (from `#[label]` attributes)

**Test:**

```rust
#[test]
fn source_diagnostic_graphical_rendering() {
    let source = Arc::from("let x = 42");
    let err = CompilerError::type_mismatch("i32", "str", Span::new(0, 3));
    let diag = SourceDiagnostic::new("test.pp", source, err);
    let report = miette::Report::new(diag);
    let rendered = format!("{report:?}");
    assert!(rendered.contains("test.pp"));
    assert!(rendered.contains("Type mismatch"));
}
```

---

## Day 2 — Morning (Hours 0–6): LSP Server

### Task 3.10: Create `crates/pipe-lang-lsp/` crate

**Cargo.toml:**
```toml
[package]
name = "pipe-lang-lsp"
version = "0.1.0"
edition = "2021"

[dependencies]
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ast = { path = "../ast" }
lexer = { path = "../lexer" }
parser = { path = "../parser" }
typechecker = { path = "../typechecker" }
diagnostics = { path = "../diagnostics" }

[lib]
name = "pipe_lang_lsp"
```

### Task 3.11: LSP backend implementation

**`crates/pipe-lang-lsp/src/lib.rs`:**

```rust
use std::collections::HashMap;
use std::sync::Arc;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

pub struct Backend {
    client: Client,
    documents: HashMap<Url, DocumentState>,
}

struct DocumentState {
    source: Arc<str>,
    typed: Option<typechecker::TypedProgram<'static>>,
    errors: Vec<String>,
}
```

**Implement:**
- `initialize` — declare capabilities (text sync, hover, completion)
- `did_open` — store source, run pipeline, publish diagnostics
- `did_change` — incremental sync, re-run pipeline
- `hover` — query `TypedProgram.type_map` at position, return type as Markdown
- `completion` — offer keywords (`let`, `type`, `match`, `if`, `else`, `use`) and prelude names

**Note:** The LSP uses `tower-lsp` which requires `Arc` + `Send + Sync`. The `TypedProgram` contains references to bump-allocated AST data (`'a` lifetime). For the LSP, we need to either:
1. Create a `TypedProgram<'static>` by leaking the arena (acceptable for LSP — long-lived)
2. Own the bump arena alongside the typed program

**Approach:** Store the bump arena alongside the typed program:

```rust
struct DocumentState {
    source: Arc<str>,
    arena: Bump,
    typed: typechecker::TypedProgram<'static>,
    errors: Vec<String>,
}
```

Where the typed program's lifetime is extended by leaking the arena's reference. This is safe because the arena lives as long as the DocumentState.

### Task 3.12: LSP hover implementation

```rust
async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let state = self.documents.get(&uri).ok_or_else(|| {
        jsonrpc::Error::method_not_found("document not found")
    })?;

    let typed = state.typed.as_ref().ok_or_else(|| {
        jsonrpc::Error::method_not_found("program has type errors")
    })?;

    // Convert LSP position (line, character) to byte offset
    let offset = lsp_position_to_offset(&state.source, pos.line as usize, pos.character as usize);

    // Find the expression span that contains this offset
    let type_str = typed.type_map.iter()
        .find(|(span, _)| span.contains(offset))
        .map(|(_, ty)| ty.to_string());

    match type_str {
        Some(s) => Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                format!("```pipe-lang\n{s}\n```"),
            )),
            range: None,
        })),
        None => Ok(None),
    }
}

fn lsp_position_to_offset(source: &str, line: usize, character: usize) -> usize {
    source.lines()
        .take(line)
        .map(|l| l.len() + 1) // +1 for newline
        .sum::<usize>() + character
}
```

### Task 3.13: Add `contains` method to `Span`

**File:** `crates/ast/src/span.rs`

```rust
impl Span {
    /// Returns true if `byte_offset` falls within this span.
    pub fn contains(&self, byte_offset: usize) -> bool {
        self.start <= byte_offset && byte_offset < self.end
    }
}
```

### Task 3.14: LSP binary entry point

**`crates/pipe-lang-lsp/src/main.rs`:**

```rust
use pipe_lang_lsp::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

### Task 3.15: Update workspace `Cargo.toml`

```toml
members = [
    "crates/ast",
    "crates/lexer",
    "crates/parser",
    "crates/typechecker",
    "crates/ir",
    "crates/runtime",
    "crates/stdlib",
    "crates/diagnostics",
    "crates/cli",
    "crates/pipe-lang-lsp",
]
```

### Tests for LSP

```rust
#[tokio::test]
async fn lsp_initialize_returns_capabilities() {
    let (client, _) = tower_lsp::create_test_client();
    let backend = Backend::new(client);
    let result = backend.initialize(InitializeParams::default()).await.unwrap();
    assert!(result.capabilities.hover_provider.is_some());
    assert!(result.capabilities.completion_provider.is_some());
}

#[tokio::test]
async fn lsp_hover_returns_type() {
    // Open a valid file, hover, verify type returned
}

#[tokio::test]
async fn lsp_hover_returns_none_for_invalid_position() {
    // Hover on whitespace, returns None
}

#[tokio::test]
async fn lsp_did_open_publishes_diagnostics() {
    // Open an invalid file, diagnostics published
}
```

---

## Day 2 — Mid (Hours 6–10): CLI Improvements + Tree-Sitter

### Task 3.16: Add `lsp` subcommand to CLI

**File:** `crates/cli/src/main.rs`

```rust
#[derive(Subcommand)]
pub enum Commands {
    Check { file: String },
    Compile {
        file: String,
        #[arg(long)]
        emit_ir: bool,
        #[arg(long)]
        time: bool,
        #[arg(long, short)]
        output: Option<String>,
    },
    Run {
        file: String,
        #[arg(long)]
        time: bool,
    },
    /// Start the language server protocol server on stdin/stdout.
    Lsp,
}
```

### Task 3.17: `--time` flag

Add timing to the pipeline in `session.rs`:

```rust
pub fn run_pipeline(&mut self) -> Result<CompileResult, Box<SourceDiagnostic>> {
    let mut timings: HashMap<String, Duration> = HashMap::new();

    let start = Instant::now();
    // ... stage 1: parse ...
    timings.insert("parse".into(), start.elapsed());

    let start = Instant::now();
    // ... stage 2: typecheck ...
    timings.insert("typecheck".into(), start.elapsed());

    // ... etc ...

    Ok(CompileResult { timings, success: true, exit_code: 0, diagnostics: vec![] })
}
```

Add `timings` field to `CompileResult`:

```rust
pub struct CompileResult {
    pub diagnostics: Vec<SourceDiagnostic>,
    pub success: bool,
    pub exit_code: i32,
    pub timings: HashMap<String, Duration>,
}
```

Print timing table when `--time` is set:

```rust
fn print_timings(timings: &HashMap<String, Duration>) {
    eprintln!("\nTiming:");
    eprintln!("  {:<15} {:>12}", "Stage", "Duration");
    eprintln!("  {}", "-".repeat(29));
    let total: Duration = timings.values().copied().sum();
    for (stage, duration) in timings {
        eprintln!("  {stage:<15} {duration:>8?}");
    }
    eprintln!("  {:<15} {total:>8?}", "Total");
}
```

### Task 3.18: Proper exit codes

```rust
fn main() -> ExitCode {
    match run() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(1) => ExitCode::from(1),   // Program returned 1
        Err(diag) => match &diag.error {
            CompilerError::JitError { .. } => ExitCode::from(2),   // Runtime error
            CompilerError::IoError(_) => ExitCode::from(3),        // I/O error
            _ => ExitCode::from(1),                                // Compilation error
        },
    }
}
```

### Task 3.19: Tree-sitter grammar

**Create `tree-sitter-pipe-lang/` directory** with:

- `grammar.js` — full grammar covering pipe-lang syntax (let, type, use, match, if, lambda, records, arrays, templates, patterns, type expressions)
- `highlights.scm` — syntax highlighting queries
- `package.json` — NPM package metadata
- `bindings/rust/` — Rust crate for embedding tree-sitter
- `test/corpus/` — test cases for the grammar

**Key grammar rules:**

```javascript
module.exports = grammar({
    name: 'pipe_lang',

    extras: $ => [/\s/, $.comment],

    rules: {
        source_file: $ => repeat($.declaration),

        declaration: $ => choice(
            $.let_declaration,
            $.type_declaration,
            $.use_declaration,
        ),

        let_declaration: $ => seq(
            'let',
            field('name', $.identifier),
            optional(seq(':', field('type', $.type_expression))),
            '=',
            field('value', $.expression),
        ),

        type_declaration: $ => seq(
            'type',
            field('name', $.identifier),
            optional(seq('<', commaSep1($.type_parameter), '>')),
            '=',
            field('rhs', $.type_expression),
        ),

        use_declaration: $ => seq(
            'use',
            commaSep1($.identifier),
        ),

        expression: $ => choice(
            $.binary_expression,
            $.unary_expression,
            $.application_expression,
            $.if_expression,
            $.match_expression,
            $.block_expression,
            $.lambda_expression,
            $.record_expression,
            $.array_expression,
            $.tuple_expression,
            $.field_access_expression,
            $.index_expression,
            $.template_expression,
            $.literal,
            $.identifier,
        ),

        // ... (see original member3-phase2.md for full grammar — it's correct)
    },
});

function commaSep1(rule) {
    return seq(rule, repeat(seq(',', rule)));
}
```

**Full grammar** — see the Day 1 Mid section of the original `member3-phase2.md` (the grammar.js content in lines 308–626 of the original file is correct and should be kept as-is).

### Task 3.20: Verify tree-sitter tests

```bash
cd tree-sitter-pipe-lang
npm install
npx tree-sitter generate
npx tree-sitter test
```

---

## Day 2 — Late (Hours 10–12): Integration + Verification

### Task 3.21: Update CI configuration

**File:** `.github/workflows/ci.yml`

```yaml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo build --all
      - run: cargo test --workspace
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
```

### Task 3.22: End-to-end verification

```bash
# Full build
cargo build --all

# All tests pass
cargo test --workspace

# No clippy warnings
cargo clippy -- -D warnings

# Proper formatting
cargo fmt --check

# Example programs (requires Dijith + Member 1 work)
cargo run -- check example-programs/hello.pp
cargo run -- run example-programs/hello.pp

# LSP binary compiles
cargo build -p pipe-lang-lsp

# tree-sitter tests
cd tree-sitter-pipe-lang && npx tree-sitter test
```

---

## Deliverables

1. **`crates/diagnostics/src/errors.rs`** — Contract-compliant `CompilerError` with TypeMismatch, UnboundVariable, NonExhaustiveMatch, JitError variants
2. **`crates/diagnostics/src/errors.rs`** — `From<TypeError>` impl mapping all typechecker errors to correct CompilerError variants
3. **`crates/cli/src/session.rs`** — Updated to use new CompilerError variants + miette graphical rendering
4. **`crates/pipe-lang-lsp/`** — Full LSP crate with hover, completion, diagnostics publication
5. **`crates/ast/src/span.rs`** — `Span::contains()` method
6. **`crates/cli/src/main.rs`** — `lsp` subcommand, `--time` flag, proper exit codes (0/1/2/3)
7. **`tree-sitter-pipe-lang/`** — Tree-sitter grammar with highlights and test corpus
8. **`.github/workflows/ci.yml`** — CI configuration
9. **Workspace `Cargo.toml`** — Updated member list
10. **Test updates** — Diagnostics tests, LSP tests

---

### Gap G5: Missing `From<LowerError> for CompilerError`

**File:** `crates/diagnostics/src/errors.rs`

The `ir::lower` module produces `Result<IrModule, LowerError>` (variants: `Unbound(SmolStr)`, `Unimplemented`). There is no `From` impl for it.

**Add before `From<typechecker::TypeError>` (Task 3.5 area):**
```rust
impl From<ir::LowerError> for CompilerError {
    fn from(err: ir::LowerError) -> Self {
        CompilerError::ir_error(err.to_string())
    }
}
```
Note: `ir::LowerError` doesn't carry a span. Use `Span::empty(0)` or just the message.

### Gap G6: Missing `From<RuntimeError> for CompilerError`

**File:** `crates/diagnostics/src/errors.rs`

The runtime crate's `RuntimeError` enum has 9 variants (DivisionByZero, IndexOutOfBounds, etc.). These need a conversion path to `CompilerError`.

```rust
impl From<runtime::error::RuntimeError> for CompilerError {
    fn from(err: runtime::error::RuntimeError) -> Self {
        CompilerError::IoError(err.to_string())
    }
}
```

## Coordination Notes

- **Dijith** already added `Span::contains()` to `crates/ast/src/span.rs` for the LSP hover feature (Gap G2)
- **Dijith** must coordinate the `From<TypeError>` impl — the typechecker's error types must be stable before Member 3 writes the conversion
- **Dijith / Member 2** add `NonExhaustiveMatch` to `TypeError` — Member 3 maps it to `CompilerError::NonExhaustiveMatch`
- **Member 1** uses `JitError` — its error type should eventually convert to `CompilerError::JitError`
- The `lsp` subcommand runs the LSP server which blocks on stdin/stdout — must not conflict with other CLI commands
- Tree-sitter is independent of the Rust pipeline — can be worked on in parallel with everything
