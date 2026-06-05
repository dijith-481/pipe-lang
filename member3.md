# Member 3: Diagnostics, CLI & Tooling (Week 1 Deliverables)

**Crate Ownership:** `crates/cli` and `crates/diagnostics`
**Mission:** A language is only as good as its error messages. You are the owner of the developer experience (DX). You will build the compiler CLI and integrate `miette` to render beautiful, Rust-style error messages.

## The Workflow & TDD Strategy

You will start immediately by building the CLI flags using `clap`. By Day 2, the Lead Architect will give you the unified `CompilerError` traits. You will mock fake errors and write tests to ensure they render beautifully in the terminal.

### Your API Contract

You will depend on `thiserror` and `miette`. The Lead will define the `Span` struct, which you will use to point arrows directly at the offending source code.

## Week 1 Deliverables & Timeline

### Days 1-2: The CLI Builder

- **Deliverable 1: Clap Integration.** Build the `cli` crate using `clap` (derive API).
- **Deliverable 2: Subcommands.** Support `lang compile <file>`, `lang run <file>`, and flags like `--emit-ir`, `--opt-level`.
- **TDD Focus:** Write tests passing mock argument arrays (e.g., `["lang", "compile", "test.ln"]`) and assert the parsed `struct` has the correct enums and boolean flags set.

### Days 3-4: The Diagnostic Engine

- **Deliverable 3: Miette Setup.** Implement the `miette::Diagnostic` traits for our `CompilerError` enums.
- **Deliverable 4: Source Code Snippets.** Wire up the error reporting so it takes the source file, a `Span` (start/end bytes), and an error message, and prints a visual snippet to the console.
- **TDD Focus:** Create a fake source string: `let x = 5 + "hello"`. Manually create a `TypeError` with a `Span` pointing to `"hello"`. Assert that your formatting logic generates the correct visual text output.

### Days 5-7: Session Management & Parser Hooks

- **Deliverable 5: CompilerSession.** Build the pipeline orchestrator. `CompilerSession::new(config).read_file().run()`.
- **Deliverable 6: Real Error Hookup.** (On Day 5, the Lead gives you the working Parser). Hook up the real `ParseError`s to your diagnostic engine.
- **TDD Focus:** Feed the compiler a file with missing brackets. Ensure your pipeline catches the `Vec<ParseError>`, feeds them to `miette`, and gracefully exits with a non-zero status code without panicking.
