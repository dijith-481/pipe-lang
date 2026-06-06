# Member 2 — CLI, Diagnostics, Module Resolver, and Prelude

**Crate ownership:** `crates/cli/src/**/*.rs`, `crates/diagnostics/src/**/*.rs`, `crates/typechecker/src/resolve.rs` (new), `crates/stdlib/src/prelude.rs` (new)

**Mission:** Build the developer-experience layer: the `pipe-lang` binary, rich error rendering, the `use` statement resolver, and the implicit-import prelude. None of this touches compiler internals — it's plumbing and lookup tables.

## Why this allocation

- **CLI is `clap` plumbing.** Subcommand definitions, argument parsing, exit codes. No clever algorithms.
- **Diagnostics is `miette` rendering.** Take error structs, format with source snippets and arrows. Mechanical.
- **Module resolver is a small lookup table.** Parse `use path::{a, b}` syntax, look up the module by name, return the bound names with their types. No unification, no inference.
- **Prelude is a fixed list of names.** A `Vec<(SmolStr, PolyType)>` returned at typechecker init. No cleverness.

All four are independent of the hard compiler internals (lexer internals, typechecker internals, IR lowering, Cranelift). You can start and finish most of this work in parallel with dijith, using mocked APIs that get replaced when dijith's crates land.

## What's already done (don't redo)

- `crates/cli/src/main.rs` — Clap CLI with `compile`, `run`, `check` subcommands and `--emit-ir`, `--opt-level` flags
- `crates/cli/src/session.rs` — `CompilerSession` with `load_source`, `set_source`, `run_pipeline` (currently a stub; you'll wire it)
- `crates/diagnostics/src/errors.rs` — `CompilerError` enum (8 variants, `thiserror` + `miette::Diagnostic`)
- `crates/diagnostics/src/lib.rs` — re-exports
- 13 tests in `diagnostics/errors.rs`, 5 in `cli/session.rs`, 9 in `ast/span.rs` = 27 tests already passing

## Deliverable A: Module Resolver (Days 1–4)

**File:** `crates/typechecker/src/resolve.rs` (new)

**Purpose:** Handle `use` statements. Given a parsed `Decl::Use { path, kind }`, populate the `TypeEnv` with the resolved bindings.

**API:**
```rust
pub struct Resolver<'a> {
    modules: HashMap<ModulePath, ModuleDef>,
}

pub struct ModulePath(pub Vec<SmolStr>);  // e.g. ModulePath(vec!["stdlib", "io"])

pub struct ModuleDef {
    pub name: SmolStr,
    pub exports: HashMap<SmolStr, PolyType>,
    pub methods: HashMap<SmolStr, PolyType>,
}

pub enum UseKind {
    Module,                          // use stdlib::io
    Single(SmolStr),                 // use stdlib::io::println
    Brace(Vec<SmolStr>),             // use stdlib::io::{println, readLine}
    Glob,                            // use stdlib::io::*
}

impl<'a> Resolver<'a> {
    pub fn new() -> Self;
    pub fn register_module(&mut self, path: ModulePath, def: ModuleDef);
    pub fn resolve_use(
        &mut self,
        env: &mut TypeEnv,
        path: &ModulePath,
        kind: &UseKind,
        span: Span,
    ) -> Result<(), ResolveError>;
}
```

**Resolution rules:**

| `use` form | Result |
|---|---|
| `use stdlib::io` | Bind `io` as a record value of type `{ println: (str) -> Effect<()>, readLine: () -> Effect<str>, ... }` |
| `use stdlib::io::println` | Bind `println` directly: `println : (str) -> Effect<()>` |
| `use stdlib::io::{println, readLine}` | Bind both unqualified |
| `use stdlib::io::*` | Glob-import all of `io`'s exports into the current scope |

Method calls on a module record desugar to field access + apply:
`io.readLine()` → `(io.readLine)()` → `Call(lookup(io, "readLine"), [])`

**Error type:**
```rust
pub enum ResolveError {
    UnknownModule { path: ModulePath, span: Span },
    UnknownSymbol { module: ModulePath, name: SmolStr, span: Span },
    DuplicateImport { name: SmolStr, span: Span },
}
```

**Test suite (6 tests):**
- `resolve_module_use` — `use stdlib::io` binds `io` as a record
- `resolve_single_use` — `use stdlib::io::println` binds `println`
- `resolve_brace_use` — `use stdlib::io::{println, readLine}` binds both
- `resolve_glob_use` — `use stdlib::io::*` imports everything
- `resolve_unknown_module` — `use stdlib::nope` → `ResolveError::UnknownModule`
- `resolve_unknown_symbol` — `use stdlib::io::nope` → `ResolveError::UnknownSymbol`

## Deliverable B: Stdlib Prelude (Days 1–3)

**File:** `crates/stdlib/src/prelude.rs` (new)

**Purpose:** Define the implicit imports every program gets. The typechecker calls this on startup to seed the global `TypeEnv`.

**API:**
```rust
pub fn prelude() -> Vec<(SmolStr, PolyType)>;
pub fn io_module() -> ModuleDef;
pub fn array_methods() -> Vec<(SmolStr, PolyType)>;
pub fn option_methods() -> Vec<(SmolStr, PolyType)>;
pub fn result_methods() -> Vec<(SmolStr, PolyType)>;
pub fn str_methods() -> Vec<(SmolStr, PolyType)>;
```

**The prelude (no `use` needed):**

| Name | Type |
|---|---|
| `println` | `(str) -> Effect<()>` |
| `print` | `(str) -> Effect<()>` |
| `eprint` | `(str) -> Effect<()>` |
| `eprintln` | `(str) -> Effect<()>` |
| `Some` | `<A>(A) -> Option<A>` |
| `None` | `Option<Nothing>` (or `forall a. Option<a>` via polymorphism) |
| `Ok` | `<T, E>(T) -> Result<T, E>` |
| `Err` | `<T, E>(E) -> Result<T, E>` |
| `id` | `<A>(A) -> A` |
| `const` | `<A, B>(A) -> (B) -> A` |
| `flip` | `<A, B, C>((A, B) -> C) -> (B, A) -> C` |
| `compose` | `<A, B, C>((B) -> C, (A) -> B) -> (A) -> C` |
| `pipe` | `<A, B, C>((A) -> B, (B) -> C) -> (A) -> C` |
| `apply` | `<A, B>((A) -> B, A) -> B` |

**`io` module exports (require `use stdlib::io`):**
- `io.println : (str) -> Effect<()>`
- `io.print : (str) -> Effect<()>`
- `io.readLine : () -> Effect<str>`
- `io.readFile : (str) -> Effect<Result<str, IOError>>`
- `io.writeFile : (str, str) -> Effect<Result<(), IOError>>`

**Method tables (auto-resolved by type at use site):**
- `Array<T>.map, .filter, .fold, .len, .concat, .drop, .take, .head, .tail, .isEmpty, .zip, .find, .flatMap, .distinct, .sortBy`
- `Option<T>.map, .flatMap, .unwrap, .isSome, .isNone, .orElse`
- `Result<T, E>.map, .flatMap, .mapErr, .recover, .unwrap`
- `str.len, .toString, .concat, .contains, .startsWith, .endsWith, .trim, .toUpperCase, .toLowerCase, .split, .replace, .slice`
- `i32.toString, .+, .-, .*, ./, .%, .<, .<=, .>, .>=, .==, .!=`
- `i64.toString, ...` (same set)
- `f64.toString, ...` (same set)
- `bool.toString, .==, .!=, .&&, .||, .!`

**Test suite (4 tests):**
- `prelude_has_println` — `prelude()` contains `(println, (str) -> Effect<()>)`
- `prelude_has_constructors` — `Some`, `None`, `Ok`, `Err` present
- `io_module_has_all_exports` — `io.println`, `io.readLine`, `io.readFile`, `io.writeFile` present
- `array_methods_have_correct_signatures` — `Array.map` has the right polymorphic type

## Deliverable C: CLI Subcommands (Days 2–6)

**File:** `crates/cli/src/main.rs`, `crates/cli/src/session.rs`

**Subcommands:**

```
pipe-lang check <file>           # lex + parse + typecheck; print errors; exit 0/1
pipe-lang run <file>             # lex + parse + typecheck + lower + execute
pipe-lang compile <file> [--emit-ir]  # full pipeline, optionally emit IR to stdout
pipe-lang --version              # print all crate versions
pipe-lang run -                  # read source from stdin
```

**Flags (extend existing CLI):**
- `--json` — machine-readable error output
- `--no-color` — disable ANSI colors
- `--opt-level <0|1|2>` — Cranelift optimization (default 1)
- `--interp` — force tree-walking interpreter instead of Cranelift (for Member 1's fallback)

**Test suite (4 tests):**
- `cli_check_exits_zero_on_success`
- `cli_check_exits_one_on_type_error`
- `cli_run_executes_hello_world`
- `cli_json_output_is_valid_json`

## Deliverable D: Diagnostics Rendering (Days 3–5)

**File:** `crates/diagnostics/src/reporter.rs` (new)

**API:**
```rust
pub struct DiagnosticReporter {
    use_color: bool,
    use_json: bool,
}

impl DiagnosticReporter {
    pub fn new() -> Self;
    pub fn with_color(mut self, b: bool) -> Self;
    pub fn with_json(mut self, b: bool) -> Self;
    pub fn report(&self, errors: &[CompilerError]) -> String;
}
```

**Behavior:**
- `report()` returns a single string with all errors formatted
- Each error has a source snippet with `^^^^` pointing to the `Span`
- JSON output: `[{ code, message, span: { start, end }, source_line }]`
- The reporter consumes `CompilerError` (existing in `diagnostics::errors`) and renders it via `miette::GraphicalReportHandler` for the pretty form, or via `serde_json` for the JSON form

**Wire-up in `session.rs`:**
```rust
pub fn run_pipeline(&mut self) -> Result<CompileResult, Box<SourceDiagnostic>> {
    // Stage 1: Lex (calls dijith's lexer)
    let tokens: Vec<Token> = Lexer::new(&self.source).collect::<Result<_, _>>()
        .map_err(|e| Box::new(SourceDiagnostic::new(...)))?;

    // Stage 2: Parse (calls dijith's parser)
    let bump = Bump::new();
    let program = parser::parse(&bump, &tokens)
        .map_err(|errs| /* convert to SourceDiagnostic */)?;

    // Stage 3: Resolve (this doc's Deliverable A)
    let mut resolver = Resolver::new();
    resolver.resolve_program(program)?;

    // Stage 4: Typecheck (calls dijith's typechecker)
    let typed = typechecker::infer_program(program)
        .map_err(|errs| /* convert */)?;

    // Stage 5: Lower (calls dijith's IR)
    let ir_module = ir::lower_program(&typed)
        .map_err(|e| /* convert */)?;

    Ok(CompileResult { diagnostics: vec![], success: true, ir: Some(ir_module) })
}

pub fn report_diagnostics(&self, result: &CompileResult) -> String {
    let reporter = DiagnosticReporter::new()
        .with_color(self.config.color)
        .with_json(self.config.json);
    reporter.report(&result.diagnostics)
}
```

**Test suite (4 tests):**
- `reporter_renders_lex_error` — `Lexer` error becomes a string with source snippet
- `reporter_renders_parse_error` — missing `}` becomes a string with `^^^^` under the right line
- `reporter_renders_type_error` — type mismatch shows both expected and got
- `reporter_json_is_valid` — JSON output parses with `serde_json::from_str`

## Deliverable E: End-to-end CLI (Day 7)

`cargo run --bin pipe-lang -- run example-programs/hello.pp` prints `Hello, World!` and exits 0.

`cargo run --bin pipe-lang -- check example-programs/factorial.pp` exits 0 with no output.

`cargo run --bin pipe-lang -- check broken.pp` exits 1 with a `miette`-rendered error.

## Test counts

| Suite | Tests |
|---|---|
| Module Resolver (Deliverable A) | 6 |
| Prelude (Deliverable B) | 4 |
| CLI (Deliverable C) | 4 |
| Diagnostics (Deliverable D) | 4 |
| **Total new** | **18** |
| Pre-existing | 27 |
| **Grand total** | **45** |

## Common pitfalls

1. **Span accuracy is everything.** The arrow must point to the right character, not the whole expression. Verify by hand on a few examples.
2. **JSON output must be valid.** `serde_json::from_str` it in tests; don't trust `format!()`.
3. **Exit codes matter.** 0 on success, 1 on any error. CI scripts depend on this.
4. **Stdin mode (`pipe-lang run -`)** must read the entire stream before lexing.
5. **The resolver never panics.** Bad `use` produces a `ResolveError`, not a crash.
6. **Don't recompile `cargo build --features=python`** — that feature doesn't exist for the CLI; only used by Member 1's stdlib if at all.

## Dependencies

- `miette` (with `fancy` feature) — add to `crates/diagnostics/Cargo.toml`
- `serde` + `serde_json` — add to `crates/diagnostics/Cargo.toml`
- `clap` (with `derive` feature) — already present
- `lexer`, `parser`, `typechecker`, `ir` — consumed as APIs (the implementation is dijith's; you write the glue)

You can write Deliverables A and B in parallel with dijith's work, using the public APIs documented in `dijith.md`. Deliverables C, D, E depend on dijith's stages landing at the documented days.

## Handoff milestone: Day 7

`pipe-lang` is a working binary. `check` and `run` subcommands work. Errors are rendered with `miette`. The 14 example programs can be type-checked and run via the CLI.
