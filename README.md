# pipe-lang

A minimalist, purely functional programming language with Hindley-Milner type inference, pattern matching, and JIT compilation via Cranelift.

## Philosophy

- **Minimal keyword footprint** — no `return`, `class`, `do`, or `yield`
- **Everything is an expression** — if/else, match, blocks all produce values
- **Purely functional** — immutable data, no global state, explicit effects
- **No GC** — deterministic memory management via atomic reference counting
- **JIT compiled** — flat SSA IR compiled to native code via Cranelift

## Quick Start

```bash
# Install
cargo install --path crates/cli

# Run a program
pipe-lang run example-programs/hello.pp

# Typecheck only
pipe-lang check example-programs/hello.pp

# Dump intermediate representation
pipe-lang compile example-programs/hello.pp --emit-ir

# Format source code in-place
pipe-lang fmt example-programs/hello.pp

# Check formatting without modifying
pipe-lang fmt --check example-programs/hello.pp

# Explain an error code
pipe-lang explain pipe_lang::ty

# Start language server (for editor integration)
pipe-lang lsp
```

## Example

```rust
type Option<T> =
    | Some(T)
    | None

type Result<T, E> =
    | Ok(T)
    | Err(E)

let describe = (opt) => match opt {
    Some(val) => `Found value: ${val}`,
    None => "Nothing found"
}

let main = () => {
    println(describe(Some(42)))
}
```

## Language Features

- **Algebraic Data Types** — sum types with pattern matching
- **Records** — anonymous product types with named fields
- **Arrays** — homogeneous, immutable, with map/filter/fold
- **Closures** — first-class functions with capture semantics
- **Templates** — backtick-delimited string interpolation `${}`
- **Effect System** — `Effect<T>` for side-effecting computations
- **HM Type Inference** — full let-polymorphism, optional annotations
- **Method Chaining** — `arr.map(f).filter(g)` syntactic sugar
- **Module Imports** — `use stdlib::io` (module resolution WIP)

## Built-in Types

| Type | Description |
| :--- | :--- |
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64`, `usize` | Unsigned integers |
| `f32`, `f64` | Floating point |
| `bool`, `str` | Boolean, UTF-8 string |
| `()` | Unit type |
| `Array<T>` | Homogeneous immutable list |
| `{ name: T, ... }` | Record with named fields |
| `(A, B) -> C` | Function type |
| `Option<T>`, `Result<T, E>` | Built-in generic ADTs |
| `Effect<T>` | Deferred side effect |

## Tooling

### Pretty Error Messages

Errors are rendered with source-code annotations, underlines, and help text:

```
  x type error: type mismatch: expected `str`, got `i32`
   ,-[example-programs/hello.pp:1:9]
 1 | let x = "hello" + 42
   :         ^^^^^^^^^^^^
   `----
  help: Make sure the types in this expression are consistent
```

Use `--color never` to disable ANSI output, or `pipe-lang explain <code>` for detailed explanations of error types.

### Formatter

```bash
# Format a file in-place
pipe-lang fmt file.pp

# Check formatting without modifying (exit code 1 if not formatted)
pipe-lang fmt --check file.pp
```

### Language Server Protocol (LSP)

The LSP server implements the Language Server Protocol over stdio:

```bash
pipe-lang lsp
```

Supported capabilities:
- **Diagnostics** on file open and change (full-sync)
- **Hover** — shows the inferred Hindley-Milner type of any expression

Neovim integration (0.12+): create `~/.config/nvim/lsp/pipe_lang.lua`:
```lua
return {
  cmd = { "pipe-lang", "lsp" },
}
```

Then add `"pipe_lang"` to `vim.lsp.enable({...})` in your config.

### Tree-sitter Grammar

A tree-sitter grammar is available at `tree-sitter-pipe-lang/` for syntax highlighting, code folding, and indentation in editors (21/22 example programs parse correctly).

To install in Neovim (requires `cc` compiler):
```bash
cd tree-sitter-pipe-lang
tree-sitter generate
cc -shared -fPIC -o parser.so src/parser.c -Isrc/tree_sitter
mkdir -p ~/.local/share/nvim/treesitter/pipe_lang
cp parser.so ~/.local/share/nvim/treesitter/pipe_lang/
cp queries/*.scm ~/.local/share/nvim/site/after/queries/pipe_lang/
```

Register the parser in your Neovim config:
```lua
vim.filetype.add({ extension = { pp = "pipe_lang" } })
local parser_path = vim.fn.stdpath("data") .. "/treesitter/pipe_lang/parser.so"
if vim.uv.fs_stat(parser_path) then
  vim.treesitter.language.add("pipe_lang", { path = parser_path })
end
```

### Conform (Formatter) Integration

Add to your `conform.lua`:
```lua
formatters_by_ft = {
  pipe_lang = { "pipe-lang" },
  -- ...
},
formatters = {
  ["pipe-lang"] = {
    command = "pipe-lang",
    args = { "fmt", "$FILENAME" },
    stdin = false,
  },
},
```

## Project Structure

```
crates/
  ast/              # AST data structures (arena-allocated)
  lexer/            # Hand-written zero-copy lexer
  parser/           # Recursive descent parser
  typechecker/      # Hindley-Milner type inference
  ir/               # SSA intermediate representation
  runtime/          # Value enum, ARC memory, Cranelift JIT
  stdlib/           # Builtin function implementations
  diagnostics/      # Error formatting with miette
  formatter/        # AST-based pretty printer
  cli/              # CLI entry point (clap)
  pipe-lang-lsp/    # Language server (tower-lsp)
tree-sitter-pipe-lang/  # Tree-sitter grammar
example-programs/       # 22 example programs
```

## Status

pipe-lang is a work-in-progress. Most core features are implemented but the runtime is still maturing. See [known-issues.md](known-issues.md) for the current bug tracker and [plan-main.md](plan-main.md) for the full specification.
