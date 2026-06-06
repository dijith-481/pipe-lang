# Phase C — Two Parallel Tracks

This document defines the build phase of pipe-lang v0.1.0, executed as two
**concurrent, independent tracks** that converge at a single integration point.

## Path Split

| Track | Owner | Scope |
|------|------|------|
| **Track A — Frontend** | **dijith** | Lexer → Parser → Typechecker → AST→IR lowering |
| **Track B — Backend** | **assistant** | IR shape definition → Cranelift JIT → test infrastructure |
| **Integration** | both | CLI wiring + 14 example programs run end-to-end |

The two tracks are decoupled by the **IR data type contract** (the contents of
`crates/ir/src/lib.rs`). Track A *consumes* the IR to lower typed ASTs into it.
Track B *consumes* the IR to compile it to native code via Cranelift. The IR
itself is owned by Track B (assistant), but the contract is agreed up front
(Day 1) so both sides can develop in parallel.

## Why this split

- Track A is high-touch (3 crates, ~30-40 tests, hand-written parser). The
  user (dijith) has full context on the spec and the 14 example programs.
- Track B is low-touch (~2 new files, ~15-20 tests) but requires Cranelift
  familiarity. The assistant is better suited to chase down Cranelift API
  details and to design a stable IR that is friendly to lower-to.
- The two tracks **share zero source files** until integration day. No merge
  conflicts possible in the build phase.

## Day-by-Day Plan

### Day 1 — Contracts and baselines (both, 1-2 hours)

**Track A (dijith):**
- Run `cargo test --workspace`; confirm 150 tests green.
- Open `LEXER_PLAN.md`; skim Step 0-4.

**Track B (assistant):**
- Write `IR_DESIGN.md` with the full IR shape:
  - `IrModule { imports, decls, functions }`
  - `IrFunction { name, params, blocks, return_type, captures? }`
  - `IrBlock { id, params, insts, term }`
  - `IrInst` variants: arithmetic, comparison, logical, `AllocArray`,
    `ArrayGet`, `ArraySet`, `AllocRecord`, `RecordGet`, `RecordSet`,
    `ConstructTag { tag, arity }`, `TagGet { index }`, `TagDiscriminant`,
    `MakeClosure { func, captures }`, `CallIndirect`, `EffectCall { builtin }`,
    `Print { parts: Vec<IrPrintPart> }` (for `println`), `StrConcat { parts }`
    (for template strings), `BoxBool`, `BoxInt64`, etc. for unboxing.
  - `Terminator`: `Return`, `Jump`, `Branch`, `Switch` (for match),
    `TailCall` (for tail-recursion).
  - Public `Display` impls for all variants (handy for `--emit-ir`).
- Run `cargo test -p ir`; 4 baseline tests pass.

**Deliverable:** `IR_DESIGN.md` committed to `crates/ir/IR_DESIGN.md`.

---

### Days 1-2 — Lexer (Track A only)

See `LEXER_PLAN.md`. Deliverable: commit
`feat(lexer): add path separator and template literal tokens` with 11+ new
tests. After this commit, all 14 example programs lex cleanly.

**Track B continues in parallel:** Cranelift skeleton (see Days 1-2 below).

---

### Days 2-4 — Parser (Track A only)

**Target:** all 14 example programs parse to ASTs with no errors.

| Day | What | Tests added |
|----|------|-------------|
| 2  | `parse_program`, `parse_decl`, `parse_let_bind`, `parse_type_alias`, `parse_import`, `parse_expr_atom` | 6 |
| 2  | `parse_lambda`, `parse_application`, `parse_binary` (precedence climbing) | 3 |
| 3  | `parse_if`, `parse_match`, `parse_block`, `parse_record` | 4 |
| 3  | `parse_tuple`, `parse_array_literal`, `parse_field_access` | 3 |
| 4  | `parse_do_block`, `parse_template_literal`, `parse_type_expr` (incl. `Array<T>`, functions, tuples, records) | 4 |
| 4  | End-to-end: each of 14 example programs parses successfully (one test per file) | 14 |
| 4  | Negative tests: unexpected EOF, mismatched delimiters, missing `=>`, etc. | 4 |

**Subtotal:** ~38 new tests in `crates/parser/`.

**Hand-off artifact:** `crates/parser/src/lib.rs` with public `parse(source: &str, arena: &Bump) -> Result<Program, ParseError>`.

---

### Days 1-2 — Cranelift skeleton (Track B)

**Files:**
- `crates/runtime/src/jit.rs` (new)
- `crates/runtime/src/jit/func_builder.rs` (new)

**What it does:**
- Sets up a Cranelift module + function builder.
- `pub fn compile_ir(module: &IrModule) -> Result<*const u8, JitError>` that
  walks `IrFunction` blocks and emits Cranelift IR.
- For Day 2, only handle: integer arithmetic, `Return`, function calls
  to registered builtins (`IO.println`, `Int.add`, etc.).
- Exposes a `pub fn call_main(module: &IrModule) -> Result<i32, JitError>`
  that invokes the `main` function pointer.

**Tests (5):**
- `jit_module_compiles_empty` — empty IR module compiles.
- `jit_module_compiles_const_i32` — `(fn main() -> i32 = 42)`.
- `jit_module_compiles_add` — `(fn add(a:i32, b:i32) -> i32 = a + b)`.
- `jit_module_runs_builtin` — calling `IO.println` is wired.
- `jit_module_returns_42` — execute the compiled `main`, get 42.

**Commit:** `feat(runtime): scaffold Cranelift JIT and basic codegen`.

---

### Days 4-8 — Typechecker (Track A)

The typechecker gets a Hindley-Milner engine.

**Files:**
- `crates/typechecker/src/infer.rs` (rewrite)
- `crates/typechecker/src/unify.rs` (extend)
- `crates/typechecker/src/builtins.rs` (new — prelude + method tables)
- `crates/typechecker/src/env.rs` (extend)
- `crates/typechecker/src/types.rs` (add `MonoType::Effect(Box<MonoType>)`)

**Tests (~26):**
- Literal types: i32, f64, str, bool (already exist) → keep, extend
- Identifier lookup, unbound error (already exist) → keep
- HM for non-recursive: `let id = (x) => x` infers `∀a. a -> a` (5)
- Function application unification (3)
- Let polymorphism: `let id = (x) => x; let n = id(42)` (1)
- Recursive fns use explicit annotation: `let fib : (i32) -> i64 = ...` (3)
- Pattern typing: literal patterns, constructor patterns, wildcard, binding (5)
- Match exhaustiveness (basic: error if no wildcard and not all variants) (2)
- `Array<T>` type, `Option<T>`, `Result<T, E>` (2)
- `Effect<T>` type (1)
- Do-block desugaring: `do { x <- m; f(x) }` ≡ `Effect::bind(m, \x -> f(x))` (2)
- Method call `.map(f)` desugars to `Option::map(receiver, f)` (2)

**Subtotal:** ~26 new tests. All 14 example programs must typecheck cleanly.

**Commit:** `feat(typechecker): implement Hindley-Milner inference and Effect tracking`.

---

### Days 8-10 — AST → IR lowering (Track A)

**Files:**
- `crates/ir/src/lower.rs` (new — public API)
- `crates/ir/src/lib.rs` (add `IrModule`, `IrDecl`)

**What it does:** for each `Decl::Bind` at the top level, produce an
`IrFunction`. The HM output (MonoType for every value) drives type info.

**Tests (~7):**
- Lower a single constant int → `IrFunction` with `ConstI32`.
- Lower a lambda `(x) => x + 1` → blocks with `Add`, `Return`.
- Lower an if/else → 3 blocks (`entry`, `then`, `else`) with `Branch`.
- Lower a match on a 2-variant tag → 4 blocks (discriminant, arm1, arm2, exit) with `Switch`.
- Lower an array literal `[1, 2, 3]` → `AllocArray` + 3 `ArraySet` (or `ConstI32` x3 + bulk init).
- Lower a record literal `{ a: 1, b: "x" }` → `AllocRecord` + field sets.
- Lower a closure `let f = (x) => x + 1` → `MakeClosure` referencing the lifted inner function.

**Commit:** `feat(ir): add AST-to-IR lowering pass`.

---

### Days 2-4 — Cranelift extensions (Track B)

**Extend `jit.rs` to cover:**
- All arithmetic (`Add`, `Sub`, `Mul`, `Div`, `Rem`) with type-resolved Cranelift opcodes.
- Comparison (`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`) returning `b1` (Cranelift bool).
- `Branch` (Cranelift `brz`/`brnz`).
- Function calls: both by symbol (`IO.println`) and by closure (`CallIndirect`).
- `ArrayGet`, `ArraySet` — `heap_addr` + load/store.
- `AllocArray` with known length (stack alloc for small, heap for large).
- `Print` — special case that calls `IO.println` builtin.

**Tests (~6):**
- `jit_arithmetic_i32` — `a + b` for all int widths.
- `jit_comparison_returns_bool` — `a < b` yields Cranelift b1.
- `jit_branch_conditional` — `if a > 0 then 1 else 2` returns correct value.
- `jit_call_closure` — invoke a `MakeClosure` value.
- `jit_array_get_set_round_trip` — alloc, set, get, check.
- `jit_print_captures_stdout` — `Print("hello")` ends up in test's captured stdout.

**Commit:** `feat(runtime): extend Cranelift codegen for arrays, records, control flow`.

---

### Days 4-8 — Cranelift polish (Track B)

**Add support for:**
- `ConstructTag` + `TagDiscriminant` + `TagGet` — heap-alloc a small struct.
- `RecordGet` / `RecordSet` — same as arrays but typed.
- `MakeClosure` — wrap a function pointer + captures into a fat pointer.
- `Effect::bind` desugaring — sequentialize effects into a Cranelift call chain.
- `StrConcat` — call `Str.concat` builtin (defined in stdlib).
- String interpolation lowering — the typechecker/lower turns `` `Hi, ${name}!` `` into
  `StrConcat([Str("Hi, "), Str(value_of_name), Str("!")])`.

**Tests (~6):**
- `jit_construct_tag` — `Some(42)` and `None` have different discriminants.
- `jit_record_get_set` — set fields, get them back.
- `jit_closure_captures_environment` — `makeAdder(5)` returns a closure that adds 5.
- `jit_effect_bind_chains` — `do { a <- m1; b <- m2; ... }` runs in order.
- `jit_str_concat_interpolation` — `` `Hi, ${name}!` `` produces `"Hi, Alice!"`.
- `jit_match_on_tag` — match expression dispatches to the right arm.

**Commit:** `feat(runtime): support tags, records, closures, and effects in Cranelift`.

---

### Days 8-10 — Test infrastructure (Track B)

**Files:**
- `crates/cli/tests/fixtures.rs` (new — fixture loader)
- `crates/cli/tests/run_fixture.rs` (new — runs one .pp file, captures stdout, diffs)
- `crates/cli/tests/expected/` (new directory — 14 `.expected.txt` files)

**What it does:** for each of the 14 example programs:
1. Run `pipe-lang run <file>` as a subprocess.
2. Capture stdout.
3. Compare against the `.expected.txt` golden file.
4. Fail with a clear diff on mismatch.

**Tests (1 parameterized test = 14 cases):**
- `run_all_example_programs_produces_expected_output` — loops over 14 files, runs each, diffs.

**Commit:** `test(cli): add end-to-end fixture tests for 14 example programs`.

**The expected outputs are written by hand or by running an early interpreter.** As a fallback for the first integration day, the assistant can hand-author `.expected.txt` from the spec and refine as needed.

---

### Days 10-12 — Integration (both)

**Files:**
- `crates/cli/src/session.rs` (extend `run_pipeline` to call parser, typechecker, lower, JIT-compile, and execute)
- `crates/cli/src/main.rs` (wire `pipe-lang run <file>` subcommand)

**Steps:**
1. Track A's `parser::parse` is called.
2. Track A's `typechecker::check` produces a typed AST + env.
3. Track A's `ir::lower::lower` produces an `IrModule`.
4. Track B's `runtime::jit::compile_ir` produces native code.
5. Track B's `runtime::jit::call_main` invokes `main`.
6. Stdout is captured and forwarded to the user's terminal (or compared to `.expected.txt` in test mode).

**Tests (~6):**
- `cli_run_hello` — `pipe-lang run hello.pp` prints `Hello, World!`.
- `cli_run_factorial` — `pipe-lang run factorial.pp` prints 0! through 10!.
- `cli_run_state_machine` — `pipe-lang run state-machine.pp` prints the state trace.
- `cli_run_io_effects` — `pipe-lang run io-effects.pp` reads from stdin (use a piped input).
- `cli_error_on_unbound_variable` — exits with non-zero and prints a clear diagnostic.
- `cli_error_on_unmatched_brace` — parser error displayed with source span.

**Commit:** `feat(cli): wire end-to-end pipeline and run subcommand`.

---

### Days 12-14 — Polish and stabilization (both)

- Add `use stdlib::io` to `io-effects.pp` test (verifies module resolver).
- Add a `pipe-lang --emit-ir` flag (prints the IR module).
- Add a `pipe-lang --check` flag (typecheck only, no codegen).
- Verify all 150 baseline + ~80 new tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt --all -- --check`.

**Commit:** `chore: 0.1.0 release prep — polish, clippy, fmt, docs`.

---

## What to do if a track slips

| Slip | Mitigation |
|------|-----------|
| Track A: Parser takes longer than 4 days | Ship a minimal-parser that supports the simplest 4 example programs first; expand later. |
| Track B: Cranelift is harder than expected | Use the **tree-walking interpreter** as a parallel backend (member 1's territory, but the assistant can scaffold a stub in `crates/runtime/src/interpreter.rs` for fallback). |
| IR contract changes mid-flight | Both sides agree on a 1-day freeze on the IR shape after Day 4. |
| Integration breaks on Day 10 | Revert to a "compile-and-print-IR" demo for Day 11; debug on Day 12. |

---

## Communication protocol

- Each track uses **conventional commits** scoped to its own files. No track
  edits the other's files until Day 10.
- A `PHASE_C_STATUS.md` file is updated daily with a 1-line status from each
  track. Format:
  ```
  ## Day N
  - Track A: ...
  - Track B: ...
  ```
- On Day 10, both tracks open a single shared PR titled "Phase C integration".
  The user (dijith) is the reviewer.

---

## File ownership matrix

| File / Dir | Owner | Other can read? |
|-----------|-------|----------------|
| `crates/lexer/**` | Track A | yes |
| `crates/parser/**` | Track A | yes |
| `crates/typechecker/**` | Track A | yes |
| `crates/ast/**` | Track A (jointly) | yes |
| `crates/ir/**` | **Track B** (assistant) | yes |
| `crates/runtime/**` | Track B (assistant) | yes |
| `crates/cli/**` | joint on Day 10 | yes |
| `example-programs/**` | shared | yes |
| `pipe-lang.md` | shared | yes |
| `dijith.md` / `member*.md` / `deliverables.md` | shared | yes |

---

## What "done" means for v0.1.0

All 14 example programs in `example-programs/`:
1. `cargo test --workspace` is green (≥ 230 tests).
2. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
3. `cargo fmt --all -- --check` is clean.
4. `pipe-lang run example-programs/hello.pp` prints `Hello, World!`.
5. `pipe-lang run example-programs/factorial.pp` prints factorials 0! through 10!.
6. The 14 fixture tests in `crates/cli/tests/` all pass.

When all 6 are true, we tag `v0.1.0` and hand off to the other 3 team members
(their work — tree-walking interpreter, stdlib, CLI polish, LSP, tree-sitter,
docs — runs in parallel on top of this base).
