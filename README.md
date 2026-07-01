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
# Run a program
pipe-lang run example-programs/hello.pp

# Typecheck only
pipe-lang check example-programs/hello.pp

# Dump intermediate representation
pipe-lang compile example-programs/hello.pp --emit-ir
```

## Editor Setup

### Prerequisites
- Node.js + npm (for the tree-sitter grammar)
- `pipe-lang` binary on `$PATH` (for LSP and formatting)

### 1. Build the grammar

```bash
cd tree-sitter-pipe-lang
npm install
npx tree-sitter build        # produces tree-sitter-pipe-lang/parser.so
```

### 2. Syntax highlighting, LSP, and formatting

#### Neovim

Add to your Neovim config (e.g. `~/.config/nvim/init.lua`):

```lua
-- Register grammar
vim.filetype.add({ extension = { pp = "pipe" } })
local parser_config = require("nvim-treesitter.parsers").get_parser_configs()
parser_config.pipe = {
  install_info = {
    url = "/full/path/to/tree-sitter-pipe-lang",
    files = { "src/parser.c" },
    generate_requires_npm = true,
  },
  filetype = "pipe",
}
-- Register queries directory
vim.treesitter.query.set_dir("pipe", "/full/path/to/tree-sitter-pipe-lang/queries")
```

Then install the parser via `:TSInstall pipe`.

**LSP**
```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "pipe",
  callback = function()
    vim.lsp.start({ name = "pipe-lang", cmd = { "pipe-lang", "lsp" } })
  end,
})
```

**Formatter**
```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "pipe",
  callback = function()
    vim.bo.formatprg = "pipe-lang fmt"
  end,
})
```

#### Zed

Add to `~/.config/zed/settings.json`:

```json
{
  "languages": {
    "PipeLang": {
      "file_types": ["pp"],
      "grammar": "pipe_lang",
      "language_servers": ["pipe-lang"],
      "formatter": {
        "external": {
          "command": "pipe-lang",
          "arguments": ["fmt"]
        }
      }
    }
  },
  "local_grammars": {
    "pipe_lang": "/full/path/to/tree-sitter-pipe-lang"
  },
  "language_server": {
    "pipe-lang": {
      "command": "pipe-lang",
      "args": ["lsp"]
    }
  }
}
```

#### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[grammar]]
name = "pipe_lang"
source = { path = "/full/path/to/tree-sitter-pipe-lang" }

[[language]]
name = "pipe_lang"
scope = "source.pipe"
file-types = ["pp"]
grammar = "pipe_lang"
language-server = { command = "pipe-lang", args = ["lsp"] }
formatter = { command = "pipe-lang", args = ["fmt"] }
```

Then symlink the queries:
```bash
mkdir -p ~/.config/helix/runtime/queries/pipe_lang
ln -s /full/path/to/tree-sitter-pipe-lang/queries/*.scm ~/.config/helix/runtime/queries/pipe_lang/
```

### 3. Rebuilding after grammar changes

```bash
cd tree-sitter-pipe-lang
npx tree-sitter build
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
  diagnostics/      # Error formatting
  cli/              # CLI entry point
  pipe-lang-lsp/    # Language server (tower-lsp)
example-programs/   # 22 example programs
tree-sitter-pipe-lang/  # Editor grammar (syntax highlighting, folding)
```

## Status

pipe-lang is a work-in-progress. Most core features are implemented but the runtime is still maturing. See [known-issues.md](known-issues.md) for the current bug tracker and [plan-main.md](plan-main.md) for the full specification.
