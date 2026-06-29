# Implementation Plan: Error Diagnostics & Tooling

## Overview

Five phases to transform the compiler's diagnostic output from bare debug-format
text into beautiful, graphical, source-code-annotated error messages with
actionable help, plus a tree-sitter grammar for editor tooling.

---

## Phase 1 — Miette Graphical Rendering (Priority: High)

**Goal:** Replace `eprintln!("{diag:?}")` with miette's graphical report handler
so errors display with source snippets and underlines.

### Files touched
- `crates/diagnostics/src/errors.rs` — Add a `report()` method to `SourceDiagnostic`
- `crates/cli/src/session.rs` — Use `miette::Report` / `GraphicalReportHandler` for rendering
- `crates/cli/src/main.rs` — Pass `--color` awareness through to handler

### Expected output format
```
  ╭─[test.pp:3:1]
  │
3 │ let x = "hello" + 42
  │         ───┬───  ─┬─
  │            │       ╰── expected i32, got str
  │            ╰── type mismatch
  │
  = note: Both operands of `+` must have the same type
```

---

## Phase 2 — Richer Error Messages & Help Hints (Priority: High)

**Goal:** Add miette `#[help()]` annotations to every `CompilerError` variant,
improve contextual information in error messages.

### Files touched
- `crates/diagnostics/src/errors.rs` — Add `#[help]` attributes
- `crates/lexer/src/error.rs` — More descriptive messages with suggestions
- `crates/parser/src/error.rs` — Include expected token context in help
- `crates/typechecker/src/error.rs` — Detailed type mismatch info

### What we add

| Error Variant | Help Text |
|---|---|
| `LexError::UnexpectedChar` | `"Check that all characters are valid in pipe-lang"` |
| `LexError::UnterminatedString` | `"Close the string literal with a double quote \""` |
| `ParseError::UnexpectedToken` | `"Expected one of: {expected}"` |
| `TypeError::UnificationFailed` | `"Both sides of this expression must have the same type"` |
| `TypeError::UnboundVariable` | `"Did you mean one of: {suggestions}?"` |
| `TypeError::ArityMismatch` | `"This function expects {expected} arguments"` |
| `TypeError::FieldNotFound` | `"Available fields: {fields}"` |

---

## Phase 3 — Tree-sitter Grammar (Priority: High)

**Goal:** Create a declarative `grammar.js` for pipe-lang syntax highlighting
and AST-based text manipulation in editors (Neovim, Zed).

### Location
`tree-sitter-pipe-lang/` at workspace root (not a separate repo).

### Structure
```
tree-sitter-pipe-lang/
├── grammar.js       # Main grammar definition
├── package.json     # npm package for tree-sitter CLI
├── binding.gyp      # Node.js native binding (for testing)
├── Cargo.toml       # Rust parser binding (if needed)
└── queries/
    ├── highlights.scm   # Syntax highlighting rules
    └── injections.scm   # Language injection rules
```

### Grammar coverage
- Comments (`//`)
- Literals: integers (with type suffix), floats, strings, booleans
- Templates (backtick strings with `${}` interpolation)
- Identifiers, operators, keywords (`let`, `type`, `match`, `if`, `else`, `true`, `false`, `use`)
- Expressions: binary, unary, application, lambda, if/else, match, block
- Patterns: wildcard, binding, literal, constructor, tuple, record
- Type expressions: named, apply, function arrow, tuple, record, sum

---

## Phase 4 — Runtime Error → Source Spans (Priority: Medium)

**Goal:** Thread source span information through the JIT so runtime panics
(division by zero, bounds checks, pattern match failures) show source locations.

### Approach
1. Add optional `Span` to `runtime::error::RuntimeError` variants
2. Thread source positions through IR lowering into `Panic` instructions
3. When a panic fires in JIT code, capture the span and wrap in `RuntimeError`
4. Map `ValueId`s used in panics back to their AST source spans

### Files touched
- `crates/runtime/src/error.rs` — Add `span: Option<Span>` to error variants
- `crates/ir/src/lower.rs` — Emit spans in panic instructions for index-out-of-bounds etc.
- `crates/runtime/src/jit.rs` — Capture and propagate span info
- `crates/diagnostics/src/errors.rs` — Accept runtime errors with spans

---

## Phase 5 — CLI UX Enhancements (Priority: Medium)

**Goal:** Polish the CLI for developer ergonomics.

### Features
1. `--color` / `--no-color` / `--color=always` flags
2. Warning support (currently errors-only)
3. `--explain <error-code>` — print detailed explanation of an error code
4. Summary line: `"Compilation failed with 3 errors"` vs `"Compilation succeeded"`

### Files touched
- `crates/cli/src/main.rs` — Add flags, summary output
- `crates/cli/src/session.rs` — Render warnings alongside errors
- `crates/diagnostics/src/errors.rs` — Error code registry for `--explain`

---

## Dependencies

| Phase | Depends On |
|---|---|
| Phase 1 | Nothing |
| Phase 2 | Phase 1 (rendering must work first) |
| Phase 3 | Nothing (independent) |
| Phase 4 | Phase 1 (needs rendering path) |
| Phase 5 | Phases 1 + 2 (needs good errors to explain) |

Phases 1 and 3 can be implemented in parallel.
