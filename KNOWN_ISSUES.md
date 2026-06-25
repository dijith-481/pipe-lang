# Known Issues

This document catalogs all known bugs and feature gaps that prevent example programs from running end-to-end through the JIT pipeline.

## Status Summary

- **386 unit/integration tests pass** across all crates
- **6 of 20 example programs work** (hello, factorial, fibonacci, ascii-art, records, game-of-life*)
- **14 of 20 example programs fail** due to bugs listed below

> `*` game-of-life prints "Generation 0:" then crashes with a null pointer — see C22.

---

## Crash Bugs (panics / runtime errors)

### C18: Tuple destructuring leaves names unbound

**Test:** `bug_tuple_destructuring_unbound` in `jit_instructions.rs`

**Symptom:** `let (a, b) = (1, 2)` compiles but `a` and `b` are not bound in the IR lowerer's local scope, producing:
```
ir error: unbound name in IR lowering: a
```

**Root cause:** `crates/ir/src/lower.rs` — the tuple pattern on the left side of `let` is not handled. The lowerer only binds simple identifiers.

**Affected programs:**
- `closures.pp` — `let (state, snaps) = acc`
- `sorting.pp` — `let (left, right) = split(arr)`

---

### C19: Tag type payload slice panic

**Test:** `bug_tag_type_payload_slice` in `jit_instructions.rs`

**Symptom:** A type alias with variants that have multiple payload fields (e.g. `| A(f64) | B(f64, f64)`) causes a panic at `crates/ir/src/lower.rs:74`:
```
range end index 3 out of range for slice of length 1
```

**Root cause:** `mono_to_ir_inner` at `lower.rs:74` slices `payload[offset..offset + count]` but `payload` has fewer elements than expected. The typechecker likely constructs an incorrect flattened payload list for type aliases.

**Affected programs:**
- `patterns.pp` — `Shape` type with `Circle(f64)`, `Rectangle(f64, f64)`, `Triangle(f64, f64, f64)`
- `state-machine.pp` — `AppState` type with `Idle`, `Loading`, `Ready(str)`, `Failed(str)`

---

### C20: Option match inside fold produces duplicate switch case

**Test:** `bug_match_option_duplicate_switch` in `jit_instructions.rs`

**Symptom:** `match x { None => ..., Some(m) => ... }` inside a closure passed to `fold` fails with:
```
runtime error: unimplemented IR instruction: duplicate switch case 0 (in main_lambda_5)
```

**Root cause:** The IR lowerer generates a `Switch` instruction with duplicate case 0 when lowering `match` on an `Option` inside a closure. The JIT's `validate_switch_arms` rejects duplicate cases.

**Affected programs:**
- `higher-order.pp` — `max` function: `xs.fold(None, (acc, x) => match acc { None => Some(x), Some(m) => ... })`

---

### C21: Polymorphic closure gets str type

**Test:** `bug_polymorphic_flip_closure_type` in `jit_instructions.rs`

**Symptom:** A higher-order function with type annotation like `let flip : ((a, b) -> c) -> (b, a) -> c = (f) => (b, a) => f(a, b)` — when never called (no monomorphization site) — causes `f` inside the inner lambda to resolve to `str` instead of `Closure(...)`:
```
runtime error: unimplemented IR instruction: CallIndirect: callee is not a closure, got str (in flip_lambda_1)
```

**Root cause:** The monomorphizer or type resolver defaults unresolved polymorphic parameters to `str` when no concrete call site exists. The IR emits `CallIndirect` with the parameter's value, but the JIT checks the type is `IrType::Closure(...)`.

**Affected programs:**
- `generics.pp` — `flip`, `compose`, `pipe`, `apply` all have type annotations with polymorphic parameters

---

### C22: Game of Life null pointer abort

**Symptom:** Running `game-of-life.pp` prints "Generation 0:" then aborts:
```
thread 'main' panicked at .../core/src/ptr/unique.rs:89:36:
unsafe precondition(s) violated: NonNull::new_unchecked requires that the pointer is non-null
```

This is a non-unwinding abort — `#[should_panic]` cannot catch it.

**Root cause:** Unknown. Likely a null pointer returned from the runtime (JIT bridge, string/array allocation, or effect dispatch). Occurs during rendering of the first generation.

**Affected programs:**
- `game-of-life.pp`

---

## Type System Bugs

### T1: i32 / usize mismatch

Several programs use `usize` for array indices/lengths, but the language only has `i32`. This causes type errors like:
```
type mismatch: expected i32, got usize
type mismatch: expected usize, got i32
```

**Affected programs:** `csv-query.pp`, `pathfinding-bfs.pp`, `markdown-renderer.pp`, `tiny-repl.pp`, `sorting.pp`, `state-machine.pp`

**Fix needed:** Either add `usize` to the type system or rewrite programs to use `i32`.

---

### T2: ADT type alias resolution

Complex ADTs with type aliases fail to resolve constructors:
```
type mismatch: expected [?92], got Option({email: str, id: i32, name: str})
type mismatch: expected Expr(f64), got Add(Expr(f64), Expr(f64))
```

**Affected programs:** `expression-evaluator.pp`, `option-result.pp`, `json-parser.pp`, `tiny-repl.pp`

**Fix needed:** Type alias resolution for recursive/nested ADTs.

---

### T3: Unbound ADT constructors

ADT constructors defined via `type` aliases aren't resolvable at usage sites:
```
unbound variable: Expr
unbound variable: Json
unbound variable: Add
```

**Affected programs:** `expression-evaluator.pp`, `json-parser.pp`, `tiny-repl.pp`

**Fix needed:** Constructors from `type` aliases must be added to the global scope.

---

## Missing Standard Library Features

### S1: `io` module

```
unbound variable: io
```

**Affected:** `io-effects.pp`

---

### S2: `split` string function

```
unbound variable: split
```

**Affected:** `csv-query.pp`, `markdown-renderer.pp`

---

### S3: `trim` string function

```
unbound variable: trim
```

**Affected:** `tiny-repl.pp` (also needed in other example programs that use string processing)

---

## Priority & Effort Estimate

| Bug | Priority | Effort | Area |
|-----|----------|--------|------|
| C18: Tuple destructuring | **High** | Small | IR lowerer |
| C19: Tag payload slice | **High** | Medium | Typechecker + IR lowerer |
| C20: Duplicate switch | **Medium** | Medium | IR lowerer (match lowering) |
| C21: Polymorphic closure type | **Medium** | Medium | Monomorphization |
| C22: Game of Life null ptr | **Low** | Large | Runtime JIT |
| T1: i32/usize mismatch | **Low** | Large | Type system |
| T2: ADT alias resolution | **Low** | Large | Typechecker |
| S1-S3: Missing stdlib | **Medium** | Small | Stdlib crate |
