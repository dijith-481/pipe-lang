# Member 3: Diagnostics, CLI & Tooling (Week 1 Deliverables)

**Crate Ownership:** `crates/cli` and `crates/diagnostics`
**Mission:** Build the developer experience layer — CLI interface, error rendering with `miette`, and the `CompilerSession` pipeline orchestrator.

## Architecture Overview

### What's Already Done

| Component | Location | Status |
|-----------|----------|--------|
| Clap CLI | `crates/cli/src/main.rs` | Complete — subcommands `compile`, `run`, `check` with `--emit-ir`, `--opt-level` flags |
| CompilerError | `crates/diagnostics/src/errors.rs` | Complete — `thiserror` + `miette::Diagnostic` derive, 8 variants |
| Span | `crates/ast/src/span.rs` | Complete — `Span::new(start, end)` with miette label support |
| CompilerSession | `crates/cli/src/session.rs` | Complete — pipeline orchestrator with `load_source()`, `set_source()`, `run_pipeline()` |
| Session tests | `crates/cli/src/session.rs` | 5 tests passing |

### CompilerError Variants (already implemented)

```rust
// crates/diagnostics/src/errors.rs
#[derive(Debug, Clone, thiserror::Error, miette::Diagnostic)]
pub enum CompilerError {
    #[error("lex error: {msg}")]
    #[diagnostic(code(pipe_lang::lex))]
    LexError { src: String, span: Span, msg: String },

    #[error("parse error: {msg}")]
    #[diagnostic(code(pipe_lang::parse))]
    ParseError { src: String, span: Span, msg: String, expected: Vec<String> },

    #[error("type error: {msg}")]
    #[diagnostic(code(pipe_lang::ty))]
    TypeError { src: String, span: Span, msg: String },

    #[error("ir error: {msg}")]
    #[diagnostic(code(pipe_lang::ir))]
    IrError { src: String, span: Span, msg: String },

    #[error("runtime error: {msg}")]
    #[diagnostic(code(pipe_lang::runtime))]
    RuntimeError { src: String, span: Option<Span>, msg: String },

    #[error("effect error: {msg}")]
    #[diagnostic(code(pipe_lang::effect))]
    EffectError { src: String, span: Option<Span>, msg: String },

    #[error("io error: {0}")]
    #[diagnostic(code(pipe_lang::io))]
    IoError(String),

    #[error("encountered {count} error(s)")]
    #[diagnostic(code(pipe_lang::multiple))]
    Multiple { count: usize, src: String, span: Option<Span> },
}
```

### CompilerSession API (already implemented)

```rust
// crates/cli/src/session.rs
impl CompilerSession {
    pub fn new(config: SessionConfig) -> Self;
    pub fn load_source(&mut self) -> Result<(), CompilerError>;
    pub fn set_source(&mut self, source: impl Into<String>);
    pub fn run_pipeline(&mut self) -> Result<CompileResult, CompilerError>;
}

pub struct CompileResult {
    pub diagnostics: Vec<CompilerError>,
    pub success: bool,
}
impl CompileResult {
    pub fn eprint_to_stderr(&self);
}
```

### Tests Already Passing

- `diagnostics/errors.rs`: 8 tests (display, span, multiple errors)
- `cli/session.rs`: 5 tests (config, load, pipeline)
- `ast/span.rs`: 9 tests (span operations)
- **Total across workspace: 144 tests**

## Week 1 Deliverables & Timeline

### Days 1-2: Enhanced CLI (Already Mostly Done)

**Goal:** Polish the CLI and add missing features.

**Task 1: Add `--json` flag for machine-readable output**
```rust
// In main.rs, extend Commands::Compile and Commands::Check:
/// Output diagnostics as JSON
#[arg(long)]
json: bool,
```

**Task 2: Add `--no-color` flag**
```rust
/// Disable colored output
#[arg(long)]
no_color: bool,
```

**Task 3: Add stdin mode**
```rust
// New subcommand:
/// Read from stdin
Stdin {
    /// Emit IR to stdout
    #[arg(long)]
    emit_ir: bool,
},
```

**Task 4: Version info for all crates**
```rust
fn print_version() {
    println!("pipe-lang {}", env!("CARGO_PKG_VERSION"));
    println!("  ast:       {}", ast::VERSION);
    println!("  lexer:     {}", lexer::VERSION);
    println!("  parser:    {}", parser::VERSION);
    println!("  typechecker: {}", typechecker::VERSION);
    println!("  runtime:   {}", runtime::VERSION);
    println!("  stdlib:    {}", stdlib::VERSION);
    println!("  ir:        {}", ir::VERSION);
}
```

**TDD approach:**
- Write test: `["pipe-lang", "compile", "test.pl", "--json"]` parses correctly
- Write test: `["pipe-lang", "compile", "test.pl", "--no-color"]` parses correctly
- Write test: `["pipe-lang", "--version"]` prints all crate versions

### Days 3-4: Rich Error Rendering

**Goal:** Add `miette` dependency to `diagnostics` crate and enable fancy error rendering.

**Task 5: Add `miette` as dependency (with `fancy` feature)**
```toml
# crates/diagnostics/Cargo.toml
[dependencies]
miette = { version = "7.5.0", features = ["fancy"] }
```

**Task 6: Create a `DiagnosticReporter` struct**
```rust
// crates/diagnostics/src/reporter.rs

use miette::GraphicalReportHandler;

pub struct DiagnosticReporter {
    use_color: bool,
    use_json: bool,
}

impl DiagnosticReporter {
    pub fn new() -> Self { ... }
    pub fn with_color(mut self, use_color: bool) -> Self { ... }
    pub fn with_json(mut self, use_json: bool) -> Self { ... }

    pub fn report(&self, errors: &[CompilerError]) {
        if self.use_json {
            self.report_json(errors);
        } else {
            self.report_graphical(errors);
        }
    }

    fn report_graphical(&self, errors: &[CompilerError]) {
        let handler = GraphicalReportHandler::new();
        for err in errors {
            let mut buf = String::new();
            handler.render_report(&mut buf, err).unwrap();
            eprintln!("{buf}");
        }
    }

    fn report_json(&self, errors: &[CompilerError]) {
        // Serialize errors to JSON for machine consumption
    }
}
```

**Task 7: Integration with CompilerSession**
```rust
// In session.rs, add a method:
pub fn report_diagnostics(&self, result: &CompileResult) {
    let reporter = DiagnosticReporter::new()
        .with_color(self.config.color)
        .with_json(self.config.json);
    reporter.report(&result.diagnostics);
}
```

**TDD approach:**
- Write `reporter_creates_graphical_output` — errors render with arrows pointing to spans
- Write `reporter_json_output` — errors serialize to valid JSON
- Write `reporter_no_color` — with `use_color(false)`, no ANSI escape codes
- Write `session_reports_errors` — full pipeline with `run_pipeline` + `report_diagnostics`

### Days 5-7: Pipeline Integration & Real Errors

**Goal:** Wire up the parser and typechecker into the session pipeline.

**Task 8: Add parser + typechecker to session pipeline**
```rust
// In session.rs run_pipeline():
pub fn run_pipeline(&mut self) -> Result<CompileResult, CompilerError> {
    let source = self.source.as_deref().expect("source must be loaded");

    // Stage 1: Lex
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.collect();

    // Stage 2: Parse
    let bump = Bump::new();
    let program = parser::parse(&bump, &tokens)
        .map_err(|e| CompilerError::parse_error(
            source.to_string(),
            e.span,
            format!("{e}"),
            e.expected,
        ))?;

    // Stage 3: Typecheck
    let errors = typechecker::check_program(&program);
    if !errors.is_empty() {
        let diags: Vec<_> = errors.into_iter()
            .map(|e| CompilerError::type_error(
                source.to_string(),
                e.span(),
                format!("{e}"),
            ))
            .collect();
        return Ok(CompileResult { diagnostics: diags, success: false });
    }

    Ok(CompileResult { diagnostics: vec![], success: true })
}
```

**Task 9: Add source code to error context**
```rust
// When creating CompilerError variants, always pass the full source string
// so miette can render the code snippet with arrow pointing to the span.
let err = CompilerError::lex_error(
    source.to_string(),   // full source for miette
    error.span(),         // span pointing to the offending code
    format!("{error}"),   // error message
);
```

**Task 10: Error recovery and multiple errors**
```rust
// In run_pipeline, collect ALL errors instead of stopping at first:
let mut all_errors = Vec::new();

// Lex errors
for error in lexer_errors {
    all_errors.push(CompilerError::lex_error(...));
}

// Parse errors (if parser supports recovery)
if let Err(parse_errors) = parse_result {
    for error in parse_errors {
        all_errors.push(CompilerError::parse_error(...));
    }
}

// Type errors
for error in type_errors {
    all_errors.push(CompilerError::type_error(...));
}

Ok(CompileResult {
    success: all_errors.is_empty(),
    diagnostics: all_errors,
})
```

**TDD approach:**
- Write `pipeline_with_parse_error` — missing bracket produces parse error with source snippet
- Write `pipeline_with_type_error` — type mismatch produces error pointing to the expression
- Write `pipeline_with_multiple_errors` — two errors both reported
- Write `pipeline_with_lex_and_type_errors` — mixed error types all collected
- Write `pipeline_exit_code` — `main()` returns exit code 1 on errors

## Common Pitfalls

1. **Always pass source string** — miette needs the full source to render snippets
2. **Span accuracy** — spans must point to the exact token, not the whole expression
3. **Don't panic on bad input** — use `Result` everywhere, return errors gracefully
4. **Exit codes** — `main()` must return non-zero on errors, zero on success

## Dependencies

- `miette` (with `fancy` feature): for rich error rendering
- `clap` (with `derive` feature): already in Cargo.toml
- `serde` + `serde_json`: for `--json` output mode
- `lexer`: for tokenization in pipeline
- `parser`: for AST construction (Day 5+)
- `typechecker`: for type checking (Day 5+)
