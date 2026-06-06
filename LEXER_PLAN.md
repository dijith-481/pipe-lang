# Lexer Plan — Phase C, Track A, Step 1

**Owner:** dijith
**Status:** current lexer is ~90% done; the changes below are additive
**Estimated time:** 2-4 hours
**Target file:** `crates/lexer/src/lexer.rs`
**Target test file:** `crates/lexer/src/lexer.rs` (`#[cfg(test)] mod tests`)

## TL;DR

The lexer already handles all 14 example programs **except** for two things:

1. The `::` path separator (used by `use stdlib::io`).
2. Backtick template literals with `${...}` interpolation (used by 12 of 14 examples).

Everything else — keywords, operators, numeric suffixes, line comments, `=>`, `<-`, dots, brackets, `()`, `_` — already works. Verify with `cargo test -p lexer` first; the 15 baseline tests should all pass.

---

## Step 0 — Verify baseline (5 min)

```bash
cargo test -p lexer
```

Expected: 15+ tests pass. If any fail, **stop and report** — the baseline must be green before you change anything.

---

## Step 1 — Add `TokenKind::PathSep` (15 min)

### Why
`use stdlib::io` (in `io-effects.pp`) lexes as `use stdlib . io` today. The parser needs a single `::` token to recognize module paths.

### Change to `TokenKind` enum (around line 18)

Add one variant, alphabetically near the other punctuation:

```rust
    // Punctuation
    Comma,      // ,
    Dot,        // .
    PathSep,    // ::     <-- NEW (used in `use stdlib::io`)
    Colon,      // :
```

### Change to the `:` arm of the lexer's main `match` (around line 321)

```rust
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    TokenKind::PathSep
                } else {
                    TokenKind::Colon
                }
            }
```

### Add a test in the `mod tests` block

```rust
    #[test]
    fn lex_path_separator() {
        let tokens: Vec<_> = Lexer::new("::")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens[0].kind, TokenKind::PathSep);
        assert_eq!(tokens[0].span, Span::new(0, 2));
    }

    #[test]
    fn lex_use_stdlib_io() {
        let tokens: Vec<_> = Lexer::new("use stdlib::io")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial())
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(
            significant,
            vec![
                TokenKind::Use,
                TokenKind::Ident("stdlib"),
                TokenKind::PathSep,
                TokenKind::Ident("io"),
            ]
        );
    }
```

### Update integration test in `crates/typechecker/tests/integration_test.rs` (line 48)

Currently:
```rust
    fn integration_lex_import() {
        assert_lex_count("use stdlib.io", 4); // use stdlib . io
    }
```

Change to:
```rust
    fn integration_lex_import() {
        assert_lex_count("use stdlib::io", 4); // use stdlib :: io
    }
```

Verify: `cargo test -p typechecker integration_lex_import`

---

## Step 2 — Add backtick template literal tokens (2-3 hours)

This is the bulk of the lexer work. The spec is in `pipe-lang.md` §"Template Literals":

- `` `Hello, World!` `` — pure string, no interpolation. Lexes to a single `TemplateStr("Hello, World!")` token.
- `` `Hello, ${name}!` `` — interpolation. Lexes to a sequence:
  `Backtick` → `TemplateStr("Hello, ")` → `TemplateHoleStart` → `<expression tokens>` → `TemplateHoleEnd` → `TemplateStr("!")` → `TemplateEnd`.
- `` `` `` (empty) — single `TemplateStr("")` token.
- Nested template? Out of scope for 0.1. Treat any backtick inside `${...}` as a normal character (parens balance the hole).

### Design choice: emit a token stream, not a parsed tree

The parser is responsible for the full expression tree. The lexer's job is just to chop the source into template parts. A template literal is a single `Expr::Template { parts: Vec<TemplatePart> }` in the AST, but here we emit the linear token stream and let the parser gather it.

### Add 5 new `TokenKind` variants (in the literals section, around line 67)

```rust
    // Template literals (backtick-delimited, may contain ${expr} holes)
    /// Opening backtick of a template literal.
    Backtick,
    /// A non-interpolated chunk of a template literal.
    TemplateStr(&'a str),
    /// Marks the start of a `${` interpolation hole.
    TemplateHoleStart,
    /// Marks the end of an interpolation hole (the closing `}`).
    TemplateHoleEnd,
    /// Marks the end of a template literal (the closing backtick).
    TemplateEnd,
```

**Do not** add these to `is_trivial()` — they are significant.

### Add a `read_template` method (place it next to `read_string` at line 161)

```rust
    /// Reads a backtick template literal, emitting a stream of tokens:
    ///   `Backtick` (TemplateStr ("...") | TemplateHoleStart <inner-tokens> TemplateHoleEnd)* TemplateEnd
    ///
    /// The inner expression of `${...}` is delimited by balanced braces inside the hole.
    fn read_template(&mut self, start: usize) -> Result<Vec<Token<'a>>, LexError> {
        // The opening backtick is the byte at `start`; we're past it now.
        let mut out = Vec::new();
        out.push(Token {
            kind: TokenKind::Backtick,
            span: Span::new(start, start + 1),
        });

        let mut chunk_start = self.current_pos;
        loop {
            match self.peek() {
                None => {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.current_pos),
                    });
                }
                Some('`') => {
                    // Flush the trailing chunk (possibly empty).
                    let chunk_end = self.current_pos;
                    out.push(Token {
                        kind: TokenKind::TemplateStr(&self.source[chunk_start..chunk_end]),
                        span: Span::new(chunk_start, chunk_end),
                    });
                    self.advance(); // consume `
                    let end = self.current_pos;
                    out.push(Token {
                        kind: TokenKind::TemplateEnd,
                        span: Span::new(end - 1, end),
                    });
                    return Ok(out);
                }
                Some('$') => {
                    // Look ahead for `${`.
                    let mut clone = self.chars.clone();
                    clone.next();
                    if clone.next().is_some_and(|(_, c)| c == '{') {
                        // Flush the chunk before the hole.
                        let chunk_end = self.current_pos;
                        out.push(Token {
                            kind: TokenKind::TemplateStr(&self.source[chunk_start..chunk_end]),
                            span: Span::new(chunk_start, chunk_end),
                        });
                        self.advance(); // consume $
                        self.advance(); // consume {
                        let hole_start = self.current_pos;
                        out.push(Token {
                            kind: TokenKind::TemplateHoleStart,
                            span: Span::new(hole_start - 2, hole_start),
                        });
                        // Recursively lex the hole contents, stopping at the matching `}`.
                        // We embed the result as a *sub-stream* by appending its tokens; the
                        // parser will treat the entire backtick...` sequence as one expression.
                        // For simplicity, we use a separate Lexer over the slice up to the `}`.
                        let hole_bytes = self.find_hole_end(hole_start)?;
                        let hole_src = &self.source[hole_start..hole_start + hole_bytes];
                        let mut sub = Lexer::new(hole_src);
                        for t in sub.by_ref() {
                            match t {
                                Ok(tok) => out.push(tok),
                                Err(e) => return Err(e),
                            }
                        }
                        // sub has consumed `hole_bytes` bytes from the slice; advance by the same
                        // amount in `self`.
                        for _ in 0..hole_bytes {
                            self.advance();
                        }
                        out.push(Token {
                            kind: TokenKind::TemplateHoleEnd,
                            span: Span::new(self.current_pos - 1, self.current_pos),
                        });
                        chunk_start = self.current_pos;
                    } else {
                        self.advance();
                    }
                }
                Some('\\') => {
                    // Skip escape sequence (treat as 2 chars).
                    self.advance();
                    if self.advance().is_none() {
                        return Err(LexError::UnterminatedString {
                            span: Span::new(start, self.current_pos),
                        });
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    /// Returns the byte length of the substring from `start` up to (but not
    /// including) the `}` that closes the current `${...}` hole.
    fn find_hole_end(&mut self, start: usize) -> Result<usize, LexError> {
        let mut depth: u32 = 1;
        let mut pos = start;
        while let Some((i, ch)) = self.chars.clone().next() {
            match ch {
                '{' => {
                    depth += 1;
                    self.advance();
                    pos = self.current_pos;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(pos - start);
                    }
                    self.advance();
                    pos = self.current_pos;
                }
                _ => {
                    self.advance();
                    pos = self.current_pos;
                }
            }
        }
        Err(LexError::UnterminatedString {
            span: Span::new(start, pos),
        })
    }
```

### Add a `Backtick` arm to the lexer's main match (around line 398, near the `'"'` arm)

```rust
            // Template literal (backtick-delimited, may contain ${...} holes)
            '`' => match self.read_template(start) {
                Ok(tokens) => {
                    // Emit the first token now; the rest are queued for the next call.
                    // The simplest correct approach: return the first token and stash
                    // the remainder in self.pending.
                    let mut iter = tokens.into_iter();
                    let first = iter.next().expect("read_template always returns >= 1 token");
                    self.pending = iter.collect();
                    return Some(Ok(first));
                }
                Err(err) => return Some(Err(err)),
            },
```

### Add a `pending` field to the `Lexer` struct (around line 118)

```rust
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
    done: bool,
    pending: std::collections::VecDeque<Token<'a>>,
}
```

### Modify `Iterator::next` to drain `pending` first (line 277)

```rust
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(Ok(tok));
        }
        // ... existing body ...
    }
```

### Add template tests

```rust
    #[test]
    fn lex_empty_template() {
        let tokens: Vec<_> = Lexer::new("``")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial())
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(significant, vec![TokenKind::Backtick, TokenKind::TemplateStr(""), TokenKind::TemplateEnd]);
    }

    #[test]
    fn lex_pure_template() {
        let tokens: Vec<_> = Lexer::new("`hello`")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial())
            .map(|t| t.kind.clone())
            .collect();
        assert_eq!(significant, vec![TokenKind::Backtick, TokenKind::TemplateStr("hello"), TokenKind::TemplateEnd]);
    }

    #[test]
    fn lex_template_with_hole() {
        let tokens: Vec<_> = Lexer::new("`Hi, ${name}!`")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let significant: Vec<_> = tokens
            .iter()
            .filter(|t| !t.kind.is_trivial())
            .map(|t| t.kind.clone())
            .collect();
        // Outer: Backtick TemplateStr("Hi, ") TemplateHoleStart <name ident> TemplateHoleEnd TemplateStr("!") TemplateEnd
        assert_eq!(significant[0], TokenKind::Backtick);
        assert_eq!(significant[1], TokenKind::TemplateStr("Hi, "));
        assert_eq!(significant[2], TokenKind::TemplateHoleStart);
        assert_eq!(significant[3], TokenKind::Ident("name"));
        assert_eq!(significant[4], TokenKind::TemplateHoleEnd);
        assert_eq!(significant[5], TokenKind::TemplateStr("!"));
        assert_eq!(significant[6], TokenKind::TemplateEnd);
    }

    #[test]
    fn lex_template_unterminated() {
        let result = Lexer::new("`hello").collect::<Result<Vec<_>, _>>();
        assert!(result.is_err());
    }

    #[test]
    fn lex_template_in_hello_pp() {
        let src = std::fs::read_to_string("example-programs/hello.pp").unwrap();
        let tokens: Vec<_> = Lexer::new(&src)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        // No errors; we should see Backtick and TemplateEnd somewhere.
        let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
        assert!(kinds.iter().any(|k| matches!(k, TokenKind::Backtick)));
        assert!(kinds.iter().any(|k| matches!(k, TokenKind::TemplateEnd)));
    }
```

The last test will require a `#[path]` or `include_str!` because `cargo test -p lexer` runs from `crates/lexer/`. Use `include_str!("../../../example-programs/hello.pp")` (this is `crates/lexer/src/lexer.rs`, so `../../../` goes to repo root):

```rust
    #[test]
    fn lex_template_in_hello_pp() {
        let src = include_str!("../../../example-programs/hello.pp");
        let result: Result<Vec<_>, _> = Lexer::new(src).collect();
        assert!(result.is_ok(), "hello.pp failed to lex: {:?}", result.err());
    }
```

Add the same for any other template-heavy example to be safe:
- `io-effects.pp`, `state-machine.pp`, `factorial.pp`, `fibonacci.pp`, `records.pp`, `higher-order.pp`, `closures.pp`, `option-result.pp`, `ascii-art.pp`, `game-of-life.pp`, `patterns.pp`, `sorting.pp`, `generics.pp`.

You can collapse all 14 into one parameterized helper or just write one test per file in a small loop. A loop is cleanest:

```rust
    #[test]
    fn lex_all_example_programs() {
        for entry in std::fs::read_dir("../../../example-programs").unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("pp") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let result: Result<Vec<_>, _> = Lexer::new(&src).collect();
            assert!(result.is_ok(), "{} failed to lex: {:?}", path.display(), result.err());
        }
    }
```

### Critical edge cases to think through

1. **Empty template** `` `` `` → 3 tokens: `Backtick`, `TemplateStr("")`, `TemplateEnd`. The `TemplateStr("")` is required so the parser always sees at least one chunk.
2. **Empty hole** `` `${} ` `` — currently `find_hole_end` returns 0 and the sub-lexer produces no tokens. The parser will need to reject this. For now, lex accepts it.
3. **Hole with parens/braces** `` `${ {a: 1}.a }` `` — the `find_hole_end` brace counter handles braces; parens are transparent to `find_hole_end` but the sub-lexer handles them as normal.
4. **Backtick inside a hole** `` `${ `x` }` `` — `find_hole_end` does **not** track backticks. This means `` `${ `x` }` `` would treat the first `` ` `` as ending the outer template. **For 0.1, this is acceptable** (none of the 14 examples need it). Document the limitation; the parser can add a "no backticks in holes" check if needed.
5. **Escape `\\`` in template** — already handled by the `\\` arm of `read_template`. Lexes as two backslashes followed by a backtick, but the backtick is still an outer terminator. The escape only matters for `\\$` to prevent `${` from being mistaken for a hole. If you want to be strict, add a `\\$` check; for 0.1, treat `\\` as "skip next char" and don't worry.
6. **Multiple holes** `` `${a}${b}` `` — works naturally; you get `TemplateStr("")` between the two `TemplateHoleEnd` and `TemplateHoleStart` tokens.

---

## Step 3 — Run the full test suite (5 min)

```bash
cargo test --workspace
```

Expected: all 150 baseline tests + your new tests pass, zero clippy warnings (`cargo clippy --workspace -- -D warnings`).

If the parser-related tests are still passing, that means they don't exercise template literals yet — that's fine, the parser is the next phase.

---

## Step 4 — Commit (5 min)

Use a Conventional Commits message that scopes to the lexer:

```bash
git add crates/lexer/
git commit -m "feat(lexer): add path separator and template literal tokens

- Add TokenKind::PathSep (::) for use stdlib::io style imports
- Add TokenKind::{Backtick, TemplateStr, TemplateHoleStart, TemplateHoleEnd, TemplateEnd}
- Implement read_template() that emits a linear stream of template parts
- Add find_hole_end() helper to balance braces in interpolation holes
- Add 11 new tests covering path separator, empty/pure/holey templates,
  and lexing of all 14 example programs end-to-end"
```

---

## What is intentionally NOT in scope for the lexer

These are parser/ast concerns; do **not** add lexer support for them:

- Type annotation `: T` parsing — parser's job.
- Type application `Array<T>` parsing — parser's job.
- Lambda parameter lists `(x, y) => body` vs. parenthesized type `(T) -> U` — parser's job (the lexer already produces the right token stream).
- Record literal `{ name: "Alice" }` vs. block `{ stmts; expr }` — parser's job (lexer already emits `OpenBrace`).
- Pattern matching constructor `Some(x)` vs. function call `Some(x)` — parser's job.
- Tuple `(a, b)` vs. grouping `(a)` — parser's job.

The lexer is dumb on purpose: it produces a flat stream of tokens. The parser disambiguates.

---

## Hand-off

When the commit lands, ping me. I'll then:
1. Update `crates/cli/src/session.rs` to use the new `PathSep` token.
2. Update the integration tests in `crates/typechecker/tests/integration_test.rs`.
3. Start the parser plan based on the finalized token stream.
