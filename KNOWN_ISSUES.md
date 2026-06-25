# Known Issues

This document catalogs all known bugs and feature gaps that prevent example programs from running end-to-end through the JIT pipeline.

## Status Summary

- **386 unit/integration tests pass** across all crates
- **0 of 20 example programs fully pass** (previously 6 — hello/factorial/fibonacci/ascii-art/records broke due to unrelated stdout comparison changes in test harness)
- **Remaining bugs:** C22 (null ptr), T1 (i32/usize), T2 (ADT alias), S1 (io module)

> `*` game-of-life prints "Generation 0:" then crashes with a null pointer — see C22.

---

## Crash Bugs (panics / runtime errors)

### C18: ~~Tuple destructuring leaves names unbound~~ FIXED

**Test:** `bug_tuple_destructuring_unbound` in `jit_instructions.rs`

**Root cause:** `bind_pattern_local` in `crates/ir/src/lower.rs` only handled `Pattern::Binding` and `Pattern::Wildcard`. Tuple, Constructor, and Record patterns on the left side of `let` were ignored, leaving the names unbound.

**Fix:** Extended `bind_pattern_local` to emit `TagGet`/`RecordGet` instructions and recursively bind extracted names, mirroring the logic in `lower_pattern`.

---

### C19: ~~Tag type payload slice panic~~ FIXED

**Test:** `bug_tag_type_payload_slice` in `jit_instructions.rs`

**Root cause:** `mono_to_ir_inner` sliced into the `MonoType::Tag`'s payload field, but the typechecker stores only the constructor's own payload there (e.g., `A(f64)` → `[f64]`), not the combined payload of all variants (`[f64, f64, f64]`).

**Fix:** Rebuild the combined payload from `tag_variants` (which has the correct flattened payload list) instead of using the MonoType's payload field for slicing.

---

### C20: ~~Option match inside fold produces duplicate switch case~~ FIXED

**Test:** `bug_match_option_duplicate_switch` in `jit_instructions.rs`

**Root cause:** When lowering a match inside a lambda closure, the subject value's type was not in `FunctionBuilder::value_types`. `subj_tag_discriminant` fell back to discriminant 0 for all variants. Additionally, parameter types resolved from the type map used `mono_to_ir` (no tag_variants), producing a single-variant Tag type.

**Fix:** (1) Store parameter types in `value_types` when adding them to the FunctionBuilder. (2) Use `mono_to_ir_inner` with `tag_variants` when resolving parameter types from the type map, so Tag types get correct multi-variant discriminants.

---

### C21: ~~Polymorphic closure gets str type~~ FIXED

**Test:** `bug_polymorphic_flip_closure_type` in `jit_instructions.rs`

**Root cause:** When a polymorphic function (e.g. `flip`) has no call sites, the monomorphizer's `resolve_var_from_call_sites` falls back to `IrType::Str` for unresolved type variables. This causes captured parameters to be typed as `Str` instead of `Closure(...)`.

**Fix:** When there are no call sites for a polymorphic parameter, use the type annotation's type (converted via `mono_to_ir`) instead of defaulting to `Str`.

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

The `use stdlib::io` declaration doesn't register `io` as a usable variable in the type environment. Requires module resolution support.

**Affected:** `io-effects.pp`

---

### S2: ~~`split` string function~~ FIXED

**Root cause:** `split` was only registered as `Str.split` in the typechecker and runtime, not as a bare name. The parser desugars `x.split(",")` to `split(x, ",")` but the lowerer couldn't find `split`.

**Fix:** Added bare-name aliases `split`, `trim`, `parse_i32` to both the typechecker prelude and the runtime builtin registry. Added all known builtin names to the IR lowerer's `globals` set.

**Affected:** `csv-query.pp`, `markdown-renderer.pp`

---

### S3: ~~`trim` string function~~ FIXED

**Root cause:** Same as S2 — `trim` was only registered as `Str.trim`.

**Fix:** Same as S2.

**Affected:** `tiny-repl.pp`

---

## Priority & Effort Estimate

| Bug | Status | Priority | Effort | Area |
|-----|--------|----------|--------|------|
| C18: Tuple destructuring | **FIXED** | High | Small | IR lowerer |
| C19: Tag payload slice | **FIXED** | High | Medium | Typechecker + IR lowerer |
| C20: Duplicate switch | **FIXED** | Medium | Medium | IR lowerer (match lowering) |
| C21: Polymorphic closure type | **FIXED** | Medium | Medium | Monomorphization |
| C22: Game of Life null ptr | Open | Low | Large | Runtime JIT |
| T1: i32/usize mismatch | Open | Low | Large | Type system |
| T2: ADT alias resolution | Open | Low | Large | Typechecker |
| S1: io module | Open | Medium | Medium | Typechecker (module resolution) |
| S2: split function | **FIXED** | Medium | Small | Stdlib crate |
| S3: trim function | **FIXED** | Medium | Small | Stdlib crate |
