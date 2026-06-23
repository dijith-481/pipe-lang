# Member 3 — Phase 2: Tooling, LSP, Diagnostics & Tree-Sitter

**Crate Ownership:** `crates/cli`, `crates/diagnostics`, `crates/pipe-lang-lsp` (new)  
**Timeline:** 3 days (parallel with Member 1, Member 2)  
**Goal:** Diagnostics contract compliance, LSP server for editor integration, tree-sitter grammar for syntax highlighting, CLI improvements (timing, exit codes, flags), and miette graphical rendering. All example programs show proper error messages.

## ⚠️ Task Sequencing

Tasks **MUST** be done in this order:

1. **Task 3.1 first** — Rewrite `CompilerError` with new variants. Everything downstream depends on this.
2. **Tasks 3.2–3.5** — Constructor helpers, `span()`, `From` impls (update the existing `From<TypeError>` once 3.1 is done)
3. **Tasks 3.6–3.9** — session.rs updates, miette rendering, tests
4. **Tasks 3.10–3.15** — LSP core (initialize, didOpen, didChange, hover, completion)
5. **Task 3.16** — **LSP inlay hints** (1–2h, independent of other LSP features)
6. **Task 3.17** — **`crates/formatter/` + `fmt` subcommand** (4–6h, independent of LSP)
7. **Tasks 3.18–3.20** — CLI improvements (lsp subcommand, --time, exit codes)
8. **Tasks 3.21–3.24** — Tree-sitter, CI, integration

Do NOT do Task 3.6 before Task 3.1 — it references `CompilerError` variants that won't exist yet.

---

## Current State

| Area | Status | Priority |
|---|---|---|---|
| **Diagnostics crate** | 8 `CompilerError` variants — 3 are non-contract (`TypeError`, `RuntimeError`, `EffectError`), 4 contract variants missing (`TypeMismatch`, `UnboundVariable`, `NonExhaustiveMatch`, `JitCompileError`), `ParseError` fields don't match spec | **1 — Fix first** |
| **Error rendering** | ✅ **Done** — `eprint_to_stderr()` uses `miette::Report::new(diag)` | **2** |
| **CLI** | ✅ `lsp` subcommand added. Missing `--time`, `--emit-asm`, `-o` | **2** |
| **Exit codes** | Still 0/1 only, no distinction for compile/runtime/IO | **2** |
| **LSP** | ✅ **Done** — `crates/pipe-lang-lsp/` with hover, diagnostics publication, binary entry point. Missing `completion`, `inlayHint` capabilities. | **2** |
| **Inlay hints** | Not implemented — type_map infrastructure exists | **2** |
| **Formatter** | Not implemented — basic AST pretty-printer needed | **3** |
| **Tree-sitter** | Does not exist | **3** |
| **Span::contains()** | ✅ **Done by Dijith** (`crates/ast/src/span.rs`) | **—** |
| **From<TypeError>** | ✅ **Done** (in `crates/typechecker/src/error.rs` — will need update after Task 3.1) | **—** |

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
    JitCompileError   { msg: String },                                 // MISSING (NOT `JitError` — clashes with existing runtime::jit::JitError)
    Multiple          { count: usize, span: Option<Span> },
}
```

Current has: `LexError`, `ParseError` (bad fields), `TypeError` (wrong — should be `TypeMismatch`), `IrError`, `RuntimeError` (wrong — should be `JitError`), `EffectError` (not in contract), `IoError` (not in contract — keep as convenience), `Multiple`.

### Task 3.1: Rewrite `CompilerError` in `crates/diagnostics/src/errors.rs`

**Changes:**

```
REMOVE:
  TypeError      { span, msg }          → split into 3 contract variants
  RuntimeError   { span, msg }          → replaced by JitCompileError
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

    #[error("JIT compile error: {msg}")]
    #[diagnostic(code(pipe_lang::jit))]
    JitCompileError {
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

    pub fn jit_compile_error(msg: impl Into<String>) -> Self {
        Self::JitCompileError { msg: msg.into() }
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
        CompilerError::JitCompileError { .. } | CompilerError::IoError(_) => None,
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

**IMPORTANT: This impl MUST go in `crates/typechecker/src/error.rs`, NOT in `crates/diagnostics/src/errors.rs`.** Adding it to diagnostics creates a circular dependency (`diagnostics` → `typechecker` → `diagnostics`). The `typechecker` crate already depends on `diagnostics`, so it can legally implement `From<TypeError> for diagnostics::CompilerError`.

The typechecker's `TypeError` (from `crates/typechecker/src/error.rs`) has structured variants. Add a `From` impl in `crates/typechecker/src/error.rs` that maps each variant to the correct `CompilerError`:

```rust
impl From<TypeError> for diagnostics::CompilerError {
    fn from(err: TypeError) -> Self {
        match err {
            TypeError::UnificationFailed { span, expected, got } => {
                diagnostics::CompilerError::type_mismatch(expected.to_string(), got.to_string(), span)
            }
            TypeError::UnboundVariable { name, span } => {
                diagnostics::CompilerError::unbound_variable(name, span)
            }
            TypeError::NonExhaustiveMatch { span } => {
                diagnostics::CompilerError::non_exhaustive_match(span)
            }
            TypeError::ArityMismatch { expected, got, span } => {
                diagnostics::CompilerError::type_mismatch(
                    format!("{expected} arguments"),
                    format!("{got} arguments"),
                    span,
                )
            }
            TypeError::AnnotationConflict { annotation, inferred, span } => {
                diagnostics::CompilerError::type_mismatch(annotation.to_string(), inferred.to_string(), span)
            }
            TypeError::FieldNotFound { field, span } => {
                diagnostics::CompilerError::type_mismatch("record with field", field, span)
            }
            TypeError::InfiniteType { var, ty, span } => {
                diagnostics::CompilerError::TypeMismatch {
                    expected: format!("finite type"),
                    got: format!("Type var {var} occurs in {ty}"),
                    span,
                }
            }
            TypeError::NumericOverflow { ty, span } => {
                diagnostics::CompilerError::TypeMismatch {
                    expected: format!("value within range of {ty}"),
                    got: "overflow".to_string(),
                    span,
                }
            }
        }
    }
}
```

⚠️ **This impl has already been implemented** (see `crates/typechecker/src/error.rs`). It currently maps to `CompilerError::type_error()` (the old variant) because Task 3.1 hasn't been done yet. **After Task 3.1 is complete, update this impl to use the new contract variants** (`TypeMismatch`, `UnboundVariable`, `NonExhaustiveMatch`) as shown above.

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
CompilerError::jit_compile_error(e.to_string())
```

Same for line 238.

### Task 3.7: Update all tests

The diagnostics test file has tests for the old variants. Update them:
- `type_error_display` → `type_mismatch_display`
- `runtime_error_with_optional_span` → `jit_compile_error_display`
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
fn jit_compile_error_display() {
    let err = CompilerError::jit_compile_error("segfault at 0x0");
    let msg = format!("{err}");
    assert!(msg.contains("JIT compile error"));
    assert!(msg.contains("segfault"));
}

#[test]
fn jit_compile_error_has_no_span() {
    let err = CompilerError::jit_compile_error("oops");
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

### Task 3.13: Add `contains` method to `Span` ✅ ALREADY DONE

**File:** `crates/ast/src/span.rs`

**Already implemented by Dijith.** `Span::contains(byte_offset)` is available at `crates/ast/src/span.rs:49`:

```rust
pub fn contains(self, byte_offset: usize) -> bool {
    self.start <= byte_offset && byte_offset < self.end
}
```

The LSP `hover_for_position` should use `span.contains(offset)` instead of inlining the check manually.

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

## Day 2 — Mid-Morning (Hours 6–8): LSP Inlay Hints

### Task 3.16: Add LSP inlay hint support

Inlay hints display inferred types inline in the editor next to each expression.
The infrastructure already exists — only the LSP handler is missing.

**What exists:**
- `type_map: HashMap<Span, MonoType>` in `TypedProgram` — maps every expression span to its inferred type
- Span→Position conversion — already implemented for hover
- `InlayHint` types — available from `lsp-types` (transitive dep of `tower-lsp 0.20`)

**File:** `crates/pipe-lang-lsp/src/lib.rs`

**Changes:**

1. **Update `ServerCapabilities`** to advertise inlay hint support:

```rust
use tower_lsp::lsp_types::{
    // ... existing imports ...
    InlayHint, InlayHintLabel, InlayHintKind, InlayHintOptions,
    InlayHintParams, InlayHintProviderCapability,
};

// In initialize():
inlay_hint_provider: Some(InlayHintProviderCapability::Options(InlayHintOptions {
    resolve_provider: Some(false),
})),
```

2. **Add `inlay_hint` method to `Backend`:**

```rust
#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    // ... existing methods ...

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let documents = self.documents.read().await;
        let doc = documents.get(&uri)?;

        let hints: Vec<InlayHint> = doc.type_map
            .iter()
            .filter(|(span, _)| {
                let span_range = span_to_range(&doc.source, **span);
                ranges_overlap(&range, &span_range) && !span.is_empty()
            })
            // Skip the outermost decl span (the whole-line span)
            .filter(|(span, _)| {
                let text = span.source_text(&doc.source);
                !text.trim().starts_with("let ") &&
                !text.trim().starts_with("type ") &&
                !text.trim().starts_with("use ")
            })
            .map(|(span, ty)| {
                let pos = span_end_position(&doc.source, *span);
                InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!(": {ty}")),
                    kind: Some(InlayHintKind::Type),
                    padding_left: Some(true),
                    padding_right: None,
                    tooltip: None,
                    text_edits: None,
                    data: None,
                }
            })
            .collect();

        Ok(Some(hints))
    }
}

fn ranges_overlap(a: &Range, b: &Range) -> bool {
    !(a.end.line < b.start.line || (a.end.line == b.start.line && a.end.character <= b.start.character)
        || b.end.line < a.start.line || (b.end.line == a.start.line && b.end.character <= a.start.character))
}

fn span_end_position(source: &str, span: Span) -> Position {
    byte_offset_to_position(source, span.end)
}
```

**Guidelines:**
- ✅ Show types for all sub-expressions within the visible range
- ❌ Skip `Decl::Bind` whole-line spans (those are "let x = ..." — too noisy)
- ✅ Use `InlayHintKind::Type` and `padding_left: true` for visual separation

**Testing:**

```rust
#[test]
fn inlay_hint_shows_type_after_expression() {
    let mut type_map = HashMap::new();
    type_map.insert(Span::new(8, 10), "i32".to_string());
    let document = DocumentState {
        source: "let x = 42".to_string(),
        type_map,
    };
    let range = Range::new(Position::new(0, 0), Position::new(0, 10));
    // Verify hint appears at position (0, 10) with label ": i32"
}
```

**Estimated effort:** 1–2 hours.

---

## Day 2 — Late Morning (Hours 8–10): Code Formatter

### Task 3.17: Implement basic code formatter

A `pipe-lang fmt` subcommand that pretty-prints source code.
Comments are NOT preserved (they are discarded by the lexer). A comment-preserving
formatter is deferred to v0.2.

**Approach:** AST pretty-printer — walk the typed AST and emit formatted code
with consistent indentation, line breaks, and spacing.

**New crate:** `crates/formatter/`

**`crates/formatter/Cargo.toml`:**
```toml
[package]
name = "formatter"
version = "0.1.0"
edition = "2024"

[dependencies]
ast = { path = "../ast" }
bumpalo = "3"

[dev-dependencies]
parser = { path = "../parser" }
```

**`crates/formatter/src/lib.rs`:**

```rust
use ast::ast::{Decl, Expr, MatchArm, Pattern, Program, Stmt, TypeExpr};

/// Formats a parsed program into a pretty-printed string.
/// Does NOT preserve comments (they are discarded during lexing).
pub fn format(program: &Program) -> String {
    let mut out = String::new();
    for decl in &program.decls {
        format_decl(decl, &mut out, 0);
        out.push('\n');
    }
    out
}

fn format_decl(decl: &Decl, out: &mut String, indent: usize) {
    match decl {
        Decl::Bind { name, ty, value, .. } => {
            out.push_str(&indent_str(indent));
            out.push_str("let ");
            out.push_str(name);
            if let Some(ann) = ty {
                out.push_str(" : ");
                format_type_expr(ann, out);
            }
            out.push_str(" = ");
            format_expr(value, out, indent);
        }
        Decl::TypeAlias { name, params, rhs, .. } => {
            out.push_str(&indent_str(indent));
            out.push_str("type ");
            out.push_str(name);
            if !params.is_empty() {
                out.push('<');
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    out.push_str(p);
                }
                out.push('>');
            }
            out.push_str(" =\n");
            format_type_expr(rhs, out);
        }
        Decl::Use { path, .. } => {
            out.push_str(&indent_str(indent));
            out.push_str("use ");
            out.push_str(&path.join("::"));
        }
    }
}

fn format_expr(expr: &Expr, out: &mut String, indent: usize) {
    match expr {
        Expr::IntLiteral(s, _) => out.push_str(s),
        Expr::FloatLiteral(s, _) => out.push_str(s),
        Expr::Str(s, _) => { out.push('"'); out.push_str(s); out.push('"'); }
        Expr::Bool(true, _) => out.push_str("true"),
        Expr::Bool(false, _) => out.push_str("false"),
        Expr::Ident(s, _) => out.push_str(s),
        Expr::Lambda { params, body, .. } => {
            out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(p.name);
                if let Some(ann) = &p.ty {
                    out.push_str(": ");
                    format_type_expr(ann, out);
                }
            }
            out.push_str(") => ");
            format_expr(body, out, indent);
        }
        Expr::Application { func, args, .. } => {
            format_expr(func, out, indent);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr(a, out, indent);
            }
            out.push(')');
        }
        Expr::Binary { op, left, right, .. } => {
            format_expr(left, out, indent);
            out.push(' ');
            out.push_str(match op {
                BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
                BinOp::Div => "/", BinOp::Rem => "%",
                BinOp::Eq => "==", BinOp::Ne => "!=",
                BinOp::Lt => "<", BinOp::Le => "<=",
                BinOp::Gt => ">", BinOp::Ge => ">=",
                BinOp::And => "&&", BinOp::Or => "||",
            });
            out.push(' ');
            format_expr(right, out, indent);
        }
        Expr::Block { stmts, result, .. } => {
            out.push_str("{\n");
            for s in stmts {
                format_stmt(s, out, indent + 1);
                out.push('\n');
            }
            out.push_str(&indent_str(indent + 1));
            format_expr(result, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::If { condition, then_branch, else_branch, .. } => {
            out.push_str("if ");
            format_expr(condition, out, indent);
            out.push_str(" {\n");
            out.push_str(&indent_str(indent + 1));
            format_expr(then_branch, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push_str("} else {\n");
            out.push_str(&indent_str(indent + 1));
            format_expr(else_branch, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::Match { subject, arms, .. } => {
            out.push_str("match ");
            format_expr(subject, out, indent);
            out.push_str(" {\n");
            for arm in arms {
                out.push_str(&indent_str(indent + 1));
                format_pattern(&arm.pattern, out);
                out.push_str(" => ");
                format_expr(&arm.body, out, indent + 1);
                out.push('\n');
            }
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::Array { elems, .. } => {
            out.push('[');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr(e, out, indent);
            }
            out.push(']');
        }
        Expr::Tuple { elems, .. } => {
            out.push('(');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_expr(e, out, indent);
            }
            out.push(')');
        }
        Expr::Record { fields, .. } => {
            out.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(f.name);
                out.push_str(": ");
                format_expr(f.value, out, indent);
            }
            out.push_str(" }");
        }
        Expr::FieldAccess { object, field, .. } => {
            format_expr(object, out, indent);
            out.push('.');
            out.push_str(field);
        }
        Expr::Index { array, index, .. } => {
            format_expr(array, out, indent);
            out.push('[');
            format_expr(index, out, indent);
            out.push(']');
        }
        Expr::Template { parts, .. } => {
            out.push('`');
            for part in parts {
                match part {
                    ast::ast::TemplatePart::Str(s) => out.push_str(s),
                    ast::ast::TemplatePart::Expr(e) => {
                        out.push_str("${");
                        format_expr(e, out, indent);
                        out.push('}');
                    }
                }
            }
            out.push('`');
        }
        Expr::Unary { op, operand, .. } => {
            out.push_str(match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            });
            format_expr(operand, out, indent);
        }
    }
}

fn format_stmt(stmt: &Stmt, out: &mut String, indent: usize) {
    match stmt {
        Stmt::Let { pattern, value } => {
            out.push_str(&indent_str(indent));
            out.push_str("let ");
            format_pattern(pattern, out);
            out.push_str(" = ");
            format_expr(value, out, indent);
        }
        Stmt::Expr(expr) => {
            out.push_str(&indent_str(indent));
            format_expr(expr, out, indent);
        }
    }
}

fn format_pattern(pattern: &Pattern, out: &mut String) {
    match pattern {
        Pattern::Wildcard(_) => out.push('_'),
        Pattern::Binding(name, _) => out.push_str(name),
        Pattern::Literal(expr, _) => format_expr(expr, out, 0),
        Pattern::Constructor { name, args, .. } => {
            out.push_str(name);
            if !args.is_empty() {
                out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    format_pattern(a, out);
                }
                out.push(')');
            }
        }
        Pattern::Tuple { elems, .. } => {
            out.push('(');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                format_pattern(e, out);
            }
            out.push(')');
        }
        Pattern::Record { fields, .. } => {
            out.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(f.name);
                if let Some(p) = &f.pattern {
                    out.push_str(": ");
                    format_pattern(p, out);
                }
            }
            out.push_str(" }");
        }
    }
}

fn indent_str(level: usize) -> String {
    "    ".repeat(level)
}
```

**Add `fmt` subcommand to CLI:**

```rust
// crates/cli/src/main.rs
Subcommand::Fmt { path } => {
    let src = std::fs::read_to_string(&path)
        .map_err(|e| CompilerError::IoError(e.to_string()))?;
    let arena = bumpalo::Bump::new();
    let program = parser::parse(&src, &arena)
        .map_err(|e| CompilerError::from(e))?;
    let formatted = formatter::format(&program);
    println!("{formatted}");
}
```

**Note:** The formatter loses comments (`//` comments) because the parser discards
them. A token-aware formatter that preserves comments requires a different approach
(re-lex the source, attach comments to nearby tokens, format the token stream).
That is out of scope for v0.1.

**Testing:**

```rust
#[test]
fn formats_simple_let_binding() {
    let src = "let   x   =   42";
    let bump = Bump::new();
    let program = parse(src, &bump).unwrap();
    let out = format(&program);
    assert_eq!(out, "let x = 42\n");
}

#[test]
fn formats_block_with_indentation() {
    let src = "let f = (x) => {\nlet y = x + 1\ny * 2\n}";
    let bump = Bump::new();
    let program = parse(src, &bump).unwrap();
    let out = format(&program);
    assert!(out.contains("    let y = x + 1"));
    assert!(out.contains("    y * 2"));
}

#[test]
fn formats_if_else_with_braces() {
    let src = "let abs = (x) => if x > 0 { x } else { -x }";
    let bump = Bump::new();
    let program = parse(src, &bump).unwrap();
    let out = format(&program);
    assert!(out.contains("if ") && out.contains("} else {"));
}
```

**Estimated effort:** 4–6 hours for basic formatter + CLI integration.

---

## Day 2 — Mid (Hours 10–12): CLI Improvements

### Task 3.18: Add `lsp` subcommand to CLI (already done — verify present)

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

### Task 3.19: `--time` flag

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

### Task 3.20: Proper exit codes

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

### Task 3.21: Tree-sitter grammar

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

### Task 3.22: Verify tree-sitter tests

```bash
cd tree-sitter-pipe-lang
npm install
npx tree-sitter generate
npx tree-sitter test
```

---

## Day 2 — Late (Hours 10–12): Integration + Verification

### Task 3.23: Update CI configuration

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

### Task 3.24: End-to-end verification

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

1. **`crates/diagnostics/src/errors.rs`** — Contract-compliant `CompilerError` with `TypeMismatch`, `UnboundVariable`, `NonExhaustiveMatch`, `JitCompileError` variants (NOT `JitError` — name clash with `runtime::jit::JitError`)
2. **`crates/typechecker/src/error.rs`** — Update existing `From<TypeError>` impl to use new contract variants after Task 3.1 (NOT in diagnostics crate — circular dependency)
3. **`crates/cli/src/session.rs`** — Update `CompilerError::RuntimeError` → `CompilerError::jit_compile_error()` (already has miette rendering)
4. **`crates/pipe-lang-lsp/`** — Full LSP crate with hover, completion, **inlay hints**, diagnostics publication (already done for hover/diagnostics — add `completion` + `inlay_hint`)
5. **`crates/ast/src/span.rs`** — `Span::contains()` method ✅ done
6. **`crates/formatter/src/lib.rs`** — **NEW**: AST-based pretty-printer with `format()` and `Config`
7. **`crates/cli/src/main.rs`** — `lsp` subcommand ✅ done. Add `--time`, `fmt`, and proper exit codes (0/1/2/3)
8. **`tree-sitter-pipe-lang/`** — Tree-sitter grammar with highlights and test corpus
9. **`.github/workflows/ci.yml`** — CI configuration
10. **`crates/runtime/src/error.rs`** — `From<RuntimeError> for CompilerError` (NOT in diagnostics — circular dep)
11. **`crates/ir/src/lower.rs`** — `From<LowerError> for CompilerError` (NOT in diagnostics — circular dep)
12. **Test updates** — Diagnostics tests, LSP tests, formatter tests

---

### Gap G5: Missing `From<LowerError> for CompilerError`

**⚠️ Same circular dependency issue as Task 3.5.** `diagnostics` cannot depend on `ir`. This impl belongs in `crates/ir/src/lower.rs`:

```rust
// In crates/ir/src/lower.rs
impl From<LowerError> for diagnostics::CompilerError {
    fn from(err: LowerError) -> Self {
        diagnostics::CompilerError::ir_error(Span::empty(0), err.to_string())
    }
}
```

### Gap G6: Missing `From<RuntimeError> for CompilerError`

**⚠️ Same circular dependency.** This impl belongs in `crates/runtime/src/error.rs`:

```rust
// In crates/runtime/src/error.rs
impl From<RuntimeError> for diagnostics::CompilerError {
    fn from(err: RuntimeError) -> Self {
        diagnostics::CompilerError::jit_compile_error(err.to_string())
    }
}
```

## Coordination Notes

- **Dijith** already added `Span::contains()` to `crates/ast/src/span.rs` (Gap G2) — the LSP `hover_for_position` should use it
- **Dijith** already implemented `From<TypeError> for diagnostics::CompilerError` in `crates/typechecker/src/error.rs` — it currently maps to old `CompilerError::type_error()`. After Task 3.1 is complete, **update this impl** to use the new `TypeMismatch`/`UnboundVariable`/`NonExhaustiveMatch` variants.
- **Dijith / Member 2** added `NonExhaustiveMatch` to `TypeError` — Member 3 maps it to `CompilerError::NonExhaustiveMatch`
- **Member 1** uses `JitError` — `From<RuntimeError> for CompilerError` lives in `crates/runtime/src/error.rs` (NOT diagnostics — circular dep)
- `From<LowerError> for CompilerError` lives in `crates/ir/src/lower.rs` (NOT diagnostics — circular dep)
- The `lsp` subcommand runs the LSP server which blocks on stdin/stdout — must not conflict with other CLI commands
- **Inlay hints** (Task 3.16) requires NO new infrastructure — `type_map` already exists, `InlayHint` types are available from `lsp-types`
- **Formatter** (Task 3.17) is a new `crates/formatter/` crate with no dependencies on other phase-2 changes — can be worked on in parallel with everything
- Tree-sitter is independent of the Rust pipeline — can be worked on in parallel with everything
