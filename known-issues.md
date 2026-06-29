# Known Issues

## 1. Lambda parameter type not inferred from array element type

When calling `entries.map((e) => e.key)` on a top-level `entries` array, the typechecker doesn't propagate the array's element type to the lambda parameter `e`. Accessing a second field (`e.value`) fails with `field "value" not found on record`.

```rust
let entries = [{ key: "a", value: 1 }, { key: "b", value: 2 }]
entries.map((e) => println(`${e.key}: ${e.value}`))  // error: field "value" not found on record
```

**Workaround:** Use a top-level recursive function with direct array indexing instead of lambdas.

## 2. Local recursive functions not in scope

A `let` binding referencing itself inside a block fails with `unbound variable`:

```rust
let result = {
    let go = (i) => if i < 10 { go(i + 1) } else { i }  // error: unbound variable
    go(0)
}
```

**Workaround:** Define recursive functions at the top level (module scope).

## 3. Recursive function return type defaults to I32

Unresolved type variables in recursive closures default to `I32` in `mono_to_ir_inner` (`crates/ir/src/lower.rs:149-156`). This causes the return type of recursive calls to be `I32` instead of the correct type.

```rust
let eval : (Expr) -> f64 = (e) => match e { ... }  // add type annotation to work around
```

**Workaround:** Add explicit type annotations on recursive function signatures.

## 4. `use` module imports are parsed but inert

The `use stdlib::io` syntax is parsed, typechecked, and recorded in the IR, but the runtime does not resolve or load modules. All builtins are already in the prelude, and `use` has no observable effect.

## 5. Some example programs lack expected output files

- `higher-order.pp`, `option-result.pp`, `sorting.pp`, `patterns.pp`, `ascii-art.pp`
- `game-of-life.pp`, `records.pp`, `generics.pp`, `state-machine.pp`, `io-effects.pp`

## 6. Type annotation required for unresolved type variables

When the typechecker cannot resolve a type variable (e.g., in recursive functions or functions whose return type is never used), the type defaults to `I32`/`F64`. This can silently produce wrong results rather than a type error.

## 7. Tests use subprocess spawning for isolation

`run_fixture` spawns a `pipe-lang` subprocess per test to avoid global state pollution (JITModule, capture buffer, builtin registry) between tests. This works but adds ~0.5s overhead per test.

## 8. Pre-existing test failures (not caused by this PR)

- `typechecker::integration_test::ctor_ok` — type inference assertion fails
- `typechecker::integration_test::ctor_err` — type inference assertion fails  
- `typechecker::integration_test::pattern_match_result_ok` — unification error
- `testsuite::jit_instructions` — SIGILL on this CPU (Cranelift generates unsupported instructions)
