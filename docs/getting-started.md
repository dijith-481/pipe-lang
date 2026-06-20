# Getting Started with pipe-lang

## Installation

Build from source:

```bash
git clone https://github.com/your-org/pipe-lang
cd pipe-lang
cargo build --release
```

The compiled binary is at `target/release/pipe-lang`. Add it to your `PATH` or use `cargo run -- <args>`.

## CLI Usage

```
pipe-lang check <file.pp>      # Type-check only
pipe-lang run   <file.pp>      # Type-check + JIT compile + execute
pipe-lang compile <file.pp>    # Type-check + compile to native binary
pipe-lang lsp                  # Start the LSP server
```

### Options

| Flag | Description |
|------|-------------|
| `--time` | Print compilation and execution timings |
| `--emit-asm` | Print generated machine code (when available) |
| `-o <file>` | Write compiled output to a file |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Compilation error (parse, type, IR) |
| 2 | Runtime error (panic, bounds, division by zero) |
| 3 | IO error (file not found, permission denied) |

## Hello World

```rust
// hello.pp
let main = () => println(`Hello, World!`)
```

Run it:

```bash
pipe-lang run hello.pp
# → Hello, World!
```

## Editor Support

pipe-lang ships with a built-in LSP server. Supported editors:

**VS Code:** Install the `pipe-lang` extension (see `editors/vscode/`).

**Neovim** (via `lspconfig`):
```lua
require('lspconfig').pipe_lang.setup {
  cmd = { 'pipe-lang', 'lsp' },
}
```

**Helix:** Add to your `languages.toml`:
```toml
[language-server.pipe-lang]
command = "pipe-lang"
args = ["lsp"]

[[language]]
name = "pipe-lang"
language-servers = ["pipe-lang"]
```

## Your Second Program

```rust
// factorial.pp
let factorial : (i32) -> i32 = (n) => match n {
    0 => 1
    1 => 1
    n => n * factorial(n - 1)
}

let main = () => factorial(5)
```
