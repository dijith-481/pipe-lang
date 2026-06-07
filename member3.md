# Member 3 — Tooling, Diagnostics, & LSP Developer

**Crate Ownership:** `crates/cli`, `crates/diagnostics`, `crates/pipe-lang-lsp`
**Mission:** You own the developer experience. Your job is to wrap the compiler internals into a robust command-line interface, render beautiful and exact error messages, and provide real-time static analysis to code editors via the Language Server Protocol (LSP).

You will not write type inference algorithms or JIT compilers. You will consume the strict public APIs exposed by the frontend and backend.

## 1. Diagnostics & Error Rendering (`crates/diagnostics`)

The compiler uses your `CompilerError` enum. You must implement `miette`'s `Diagnostic` trait to render exact source code snippets highlighting where the error occurred.

### 1.1 The `CompilerError` Type
```rust
use ast::span::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone, Error, Diagnostic)]
pub enum CompilerError {
    #[error("Type mismatch: expected {expected}, got {got}")]
    #[diagnostic(code(pipe::type_error))]
    TypeMismatch { 
        expected: String, 
        got: String, 
        #[label("This evaluates to {got}")] span: Span 
    },
    // ... Implement LexError, ParseError, UnboundVariable, NonExhaustiveMatch
}
```

### 1.2 Formatting
Ensure that your `Span` maps precisely to byte offsets in the source text so that `miette` draws the carets (`^^^^`) under the exact syntax token causing the failure.

## 2. Command Line Interface (`crates/cli`)

Use `clap` to build the `pipe-lang` binary. 

### 2.1 Subcommands
*   `pipe-lang check <file>`: Reads the file, calls `parser::parse()`, then `typechecker::typecheck()`. Prints `miette` diagnostics and exits with `0` on success or `1` on failure.
*   `pipe-lang compile <file> --emit-ir`: Runs the pipeline up to IR Lowering. Iterates over the `IrModule` and prints it to stdout.
*   `pipe-lang run <file>`: The full pipeline. Parses, Typechecks, Lowers, and passes the `IrModule` to the `JitCompiler`. Calls the returned function pointer and gracefully exits.

### 2.2 Orchestration
You are responsible for chaining the `Result`s of the compiler phases. If `parse()` returns a `Vec<CompilerError>`, you iterate and `eprintln!("{:?}", miette::Report::new(err))`, then halt the pipeline.

## 3. Language Server Protocol (`crates/pipe-lang-lsp`)

Build a background language server using `tower-lsp`. This requires converting compiler `Span`s to LSP `Range`s (converting byte offsets to Line/Character coordinates).

### 3.1 Server Skeleton
```rust
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult>;
    async fn did_open(&self, params: DidOpenTextDocumentParams);
    async fn did_change(&self, params: DidChangeTextDocumentParams);
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>>;
}
```

### 3.2 Feature: Real-Time Diagnostics
*   On `did_open` and `did_change`, extract the text from the LSP parameters.
*   Run the compiler frontend pipeline (`parse()` and `typecheck()`).
*   Map any `CompilerError`s to `lsp_types::Diagnostic` and push them to the client via `self.client.publish_diagnostics()`.

### 3.3 Feature: Hover Types
*   When the typechecker succeeds, it returns a `TypedProgram` which contains a `HashMap<Span, String>` (mapping every node's position to its inferred HM type).
*   On `hover`, convert the LSP's cursor position (line/col) to a byte offset.
*   Query the `TypedProgram` map. If a match is found, return an LSP `Hover` payload containing the type signature.

## 4. Testing Requirements
*   `test_cli_exits`: Run the CLI as a subprocess against a known good file (assert exit code 0) and a known bad file (assert exit code 1).
*   `test_lsp_hover`: Construct a mock LSP `HoverParams` request, feed it into the Backend, and assert the correct type string is returned.
