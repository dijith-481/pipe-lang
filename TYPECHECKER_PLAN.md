# Typechecker Plan — Track A (dijith)

**Owner:** dijith (Track A)
**Status:** foundation stub exists (`infer.rs`, `types.rs`); full implementation pending
**Source of truth:** `crates/typechecker/src/`
**Frozen contracts:** `crates/ir/src/lib.rs` (frozen until Day 10)

This is the implementation plan for the Hindley-Milner (HM) type checker
that sits between the parser and the AST→IR lowerer. The goal is for all
14 example programs in `example-programs/` to typecheck, and for the
typechecker to surface any remaining bugs in the parser or AST before
the lowerer is written.

---

## 1. Where we are

**Done** (before this plan):
- `MonoType` enum in `crates/typechecker/src/types.rs` with 12 numeric
  primitives + `Bool` + `Str` + `Unit` + `Record(BTreeMap<...>)` + `Tag`
  + `Func` + `Tuple` + `Var(TypeId)`.
- `TypeEnv` (free-var supply + value bindings + type-alias table).
- `infer_expr` stub that returns `MonoType::Unit` for unimplemented
  arms; handles `IntLiteral` → `I32`, `FloatLiteral` → `F64`,
  numeric binops, comparison, simple `let`.
- 39 typechecker unit tests + 19 integration tests, all passing.

**Not done** (this plan covers it):
- Numeric-literal suffix parsing (`42i32`, `255u8`, `3.14f64`, `100usize`)
- `MonoType::Effect(Box<MonoType>)` (required by `IrType::Effect`)
- Full Hindley-Milner with `unify` + occurs check
- Type aliases / sum types / record types
- Do-blocks, pattern matching, recursive `let`
- The 14 `.pp` examples all typecheck

---

## 2. End state (Definition of Done)

- All 14 `.pp` files in `example-programs/` typecheck successfully.
- Type errors include span + a one-line `miette` message.
- `cargo test --workspace` green; new tests bring the typechecker
  count to ~65 (39 existing + ~26 new).
- `crates/ir/src/lower.rs` exists (or the user picks where the
  lowerer lives) and can lower the typechecked AST to a valid
  `IrModule` for a "hello world" program.

---

## 3. Number literal suffixes (small, do first)

Pipe-lang has typed numeric literals: `42i8`, `42i16`, `42i32`, `42i64`,
`255u8`, `255u16`, `255u32`, `255u64`, `100usize`, `3.14f32`, `3.14f64`.
The lexer returns `IntLiteral("42i32")` and `FloatLiteral("3.14f64")`
as borrowed string slices; the typechecker is responsible for parsing
the suffix and selecting the correct `MonoType`.

**Why this lives in the typechecker, not the parser:** the parser is
syntax-only. The literal `42` is valid in any numeric context; the
typechecker chooses the concrete type using the **expected type** from
unification (so `let x: i8 = 42` works, and so does `let x = 42` →
default `i32`).

**Implementation:**
1. Add `parse_int_literal(s: &str) -> Result<(i128, MonoType), TypeError>`
   that strips the suffix and returns the value + concrete type.
2. Add `parse_float_literal(s: &str) -> Result<(f64, MonoType), TypeError>`
   that returns a `f64` value + the suffix-derived float type.
3. In `infer_expr` for `IntLiteral` / `FloatLiteral`:
   - If the literal has a suffix, use it directly.
   - If it has no suffix, **unify with the expected type** (if any);
     otherwise default to `I32` / `F64`.
4. For `255u8` and friends that overflow the target type, return
   `TypeError::LiteralOutOfRange { span, ty, value }`.
5. 5 new tests:
   - `int_literal_with_i8_suffix`
   - `int_literal_with_usize_suffix`
   - `int_literal_overflow_u8_errors`
   - `float_literal_with_f32_suffix`
   - `int_literal_default_to_i32`

**Effort:** ~2 hours + 1 hour for tests.

---

## 4. `MonoType::Effect(Box<MonoType>)` (1-line change)

The IR has `IrType::Effect(Box<IrType>)` (frozen). The typechecker
must mirror it. This is the canonical type for `do` blocks; the IR
erases it at codegen time.

**Implementation:**
1. Add the variant in `crates/typechecker/src/types.rs`:
   ```rust
   Effect(Box<MonoType>),
   ```
2. Add a `Display` arm: `write!(f, "Effect<{inner}>")`.
3. Add `Effect::is_erased_at_codegen() = true` (codegen docs already
   say this; just a marker).
4. 1 test: `effect_type_display`.

**Effort:** 5 minutes.

---

## 5. Hindley-Milner core (`unify` + occurs check)

The HM algorithm:

```
infer(env, expr) -> MonoType (with free vars replaced by fresh skolems)
generalize(env, mono) -> PolyType  (quantify over vars not in env)
unify(t1, t2) -> Result<(), TypeError>  (with occurs check)
instantiate(poly) -> MonoType  (replace quantified vars with fresh)
```

**Implementation in `crates/typechecker/src/unify.rs`** (new file):

```rust
pub struct Unifier {
    /// Mapping TypeId -> MonoType. Asymmetric (occurs check is one-way).
    bindings: BTreeMap<TypeId, MonoType>,
}

impl Unifier {
    pub fn new() -> Self { ... }
    pub fn fresh(&mut self) -> TypeId { ... }
    /// Resolves a type by following the chain of bindings.
    pub fn resolve(&self, ty: MonoType) -> MonoType { ... }
    pub fn unify(&mut self, t1: MonoType, t2: MonoType) -> Result<(), TypeError> { ... }
}
```

**Rules** (in priority order, each recursive on `resolve`):
1. If both sides are `Var(id)`, bind them to each other (or to the
   other side).
2. If left is `Var(id)` and right is concrete, bind `id` → right,
   after the **occurs check**: walk right to make sure `id` doesn't
   appear inside it (otherwise we have an infinite type).
3. Symmetric: if right is `Var`, bind right → left.
4. `I32` unifies with `I32`, etc. Two numeric types that differ
   (e.g. `I32` and `I64`) **do not** unify (no implicit conversions
   in 0.1).
5. `Array(a)` unifies with `Array(b)` iff `a` unifies with `b`.
6. `Func(p1, r1)` unifies with `Func(p2, r2)` iff `p1.len() == p2.len()`
   and each `p1[i]` unifies with `p2[i]` and `r1` unifies with `r2`.
7. `Record(...)` unifies iff the field name sets are equal and each
   field's type unifies. **Order does not matter** (it's a `BTreeMap`).
8. `Tag{name=l, payload=l_p}` unifies with `Tag{name=r, payload=r_p}`
   iff `l == r` and the payload vectors have the same length and each
   pair unifies. (User-declared sum types must have the same name.)
9. `Effect(t1)` unifies with `Effect(t2)` iff `t1` unifies with `t2`.
10. Any other combination is a type error.

**5 new tests** for `unify`:
- `unify_two_unbound_vars_binds_them`
- `unify_var_with_concrete_binds_var`
- `unify_occurs_check_prevents_infinite_type`
- `unify_i32_with_i64_errors`
- `unify_records_field_order_independent`

**Effort:** ~3 hours.

---

## 6. Type aliases + sum types

The parser emits `Decl::TypeAlias { name, params, rhs }` (verified).
The typechecker turns this into a `MonoType` and stores it in
`TypeEnv.type_aliases: HashMap<SmolStr, PolyType>`.

**Rules:**
- The `rhs` of a type alias is a `TypeExpr` (parser's representation);
  the typechecker converts it to a `MonoType` (with the alias's
  quantified params bound to fresh `TypeId`s).
- Sum types are written as `type Option<T> = | None | Some(T)`. The
  `Pipe` token starts the variant list. Variants can have zero or
  more payload fields.
- **Discriminant assignment** (critical for IR codegen, frozen in
  `IR_DESIGN.md`):
  - `Option::None` → 0
  - `Option::Some` → 1
  - `Result::Err` → 0
  - `Result::Ok` → 1
  - User-defined sum types: in declaration order 0, 1, 2, ...

**Implementation:**
1. `TypeEnv::define_alias(name, poly)` — stores in a side table.
2. `convert_type_expr(env, expr) -> Result<PolyType, TypeError>` —
   walks the `TypeExpr` tree and produces a `PolyType`. The
   `params: Vec<&str>` of the alias become quantified `TypeId`s.
3. Recursive types are **not** allowed in 0.1 (the parser doesn't
   support them; e.g. no `type Tree = | Leaf | Node(Tree, Tree)`).
4. 4 new tests:
   - `type_alias_introduces_polytype`
   - `type_alias_used_in_let_binding`
   - `sum_type_two_variants_distinct_discriminants`
   - `sum_type_variants_with_payload`

**Effort:** ~2 hours.

---

## 7. Records

Already represented in AST as `Expr::Record { fields, span }` and
`TypeExpr::Record { fields, span }`. The typechecker:

1. For a `TypeExpr::Record`, convert to `MonoType::Record(BTreeMap)`.
2. For an `Expr::Record { a: 1, b: "x" }`, infer the type of each
   field, then build the `MonoType::Record`.
3. For `Expr::FieldAccess { object, field }`, infer `object`'s type,
   look up `field` in the resulting `Record`; missing field is a
   `TypeError::NoSuchField`.
4. 3 new tests:
   - `record_literal_infers_field_types`
   - `field_access_on_record`
   - `field_access_missing_field_errors`

**Effort:** ~1.5 hours.

---

## 8. Patterns + pattern matching

The AST has:
- `Pattern::Wildcard`
- `Pattern::Binding(&str)` (lowercase ident)
- `Pattern::Constructor { name, fields }` (uppercase ident — sum variant)
- `Pattern::Literal(LiteralPattern)`
- `Pattern::Tuple(Vec<Pattern>)`
- `Pattern::Cons { head, tail }` (list cons)
- `Pattern::Array(Vec<Pattern>)`

`match` produces `Expr::Match { scrutinee, arms }`. Each arm is
`(Pattern, Expr)`.

**Implementation:**

```rust
fn infer_pattern(env, pattern) -> Result<MonoType, TypeError>;
fn pattern_binds(pattern) -> Vec<&str>;  // which names the pattern introduces
```

**Rules:**
1. `Wildcard` → `MonoType::fresh_var()` (any type).
2. `Binding(name)` → fresh var; add `name → var` to the env.
3. `Literal(Int(n))` → unifies with the literal's parsed type (see §3).
4. `Constructor(name, [])` → look up `name` in the env's sum-type
   table, return its 0-arity variant's type.
5. `Constructor(name, [p1, ..., pN])` → same, then unify each `pi`
   with the corresponding payload type.
6. `Tuple(ps)` → `(infer(p1), ..., infer(pN))`.
7. `Cons { head, tail }` → head gives the element type, tail must
   have the same type. Result is `Array(elem_ty)`.
8. `Array(ps)` → all elements must have the same type; result is
   `Array(elem_ty)`.

**Match exhaustiveness:** out of scope for 0.1. A non-exhaustive
match is a runtime `Panic` (codegen lowers it to a trap).

**5 new tests:**
- `match_literal_arm`
- `match_constructor_arm_binds_payload`
- `match_tuple_arm`
- `match_cons_arm_unifies_list_type`
- `match_wildcard_arm_accepts_anything`

**Effort:** ~3 hours.

---

## 9. Do-blocks

`Expr::Do { stmts, span }` where each `Stmt` is one of:
- `Let { pattern, value }` — bind the pattern to the value's type
- `Bind { name, value, cont }` — `name <- value; cont`
- `Expr(expr)` — discard the result

**Rules:**
1. `Bind` requires `value` to have type `Effect<T>` for some `T`,
   and `cont` must be a lambda/closure of type `(T) -> Effect<U>`.
   The whole `Bind` has type `Effect<U>`.
2. The final `Expr` of a do-block's `result` (which is part of the
   surrounding `Expr::Do`'s enclosing expression, not a stmt) is
   usually an `Effect<()>` for a `main` function.
3. **Desugaring (optional):** a `Bind { name, value, cont }` can be
   lowered to a call to `Effect::bind` (a runtime builtin). For
   typechecking purposes, just check the types.

**3 new tests:**
- `do_block_with_single_bind`
- `do_block_with_two_binds_chains_types`
- `do_block_with_let_then_bind`

**Effort:** ~1.5 hours.

---

## 10. Recursive `let` + `letrec`

By default, `let x = expr; body` does **not** put `x` in the env
when inferring `expr`. To allow recursive functions, we need
`letrec`:

- `Expr::Let { name, value, body, span }` is a **non-recursive** let.
  The parser uses this for `let x = 1 in x + 1` (an expression).
- `Decl::Bind { name, value, span }` is a top-level binding. For
  recursive functions, the value is checked with `name` in scope
  (i.e. the value's type can refer to `name`).

For 0.1, only `Decl::Bind` (top-level) can be recursive. The
function body typechecks with the function's own name in scope.

**1 new test:**
- `recursive_factorial_function_typechecks` (uses `factorial.pp`)

**Effort:** 30 minutes (just env management).

---

## 11. Numeric coercions and literal width

In 0.1 there are **no implicit numeric coercions**. `1 + 2i64` is a
type error. The user can write `1 as i64` (NOT YET IN SYNTAX) or
`Int::from_i64(2)` (NOT YET IN SYNTAX). For 0.1, the safe bet is
to require the suffix on every literal whose target type is not
the default. Examples:

```pipe
let n: i8 = 42i8     // ok
let n: i8 = 42       // ERROR: cannot infer, default is i32
let n: i32 = 42      // ok (suffix is redundant but valid)
```

This is the strictest policy. We can relax in 0.2 if needed.

**2 new tests:**
- `no_implicit_widening_for_inferred_literal`
- `explicit_suffix_unifies_with_expected_type`

**Effort:** 30 minutes (covered by §3 mostly).

---

## 12. End-to-end smoke tests

After §3-§10 are done, add an integration test that parses + typechecks
each of the 14 `.pp` files and asserts no errors:

```rust
// crates/typechecker/tests/all_examples.rs
#[test] fn all_examples_typecheck() { ... }
```

This test will fail for some examples initially (e.g. ones that use
features not yet implemented); track which fail in the typechecker's
`README.md` and resolve them one by one.

**Effort:** 1 hour (setup + 14 line items).

---

## 13. Schedule (target: end of Day 8)

| Day | Hours | Task |
|-----|-------|------|
| Day 1 | 2 | §3 numeric-literal suffixes (5 tests) |
| Day 1 | 0.1 | §4 `MonoType::Effect` (1 test) |
| Day 2-3 | 4 | §5 HM core + unify (5 tests) |
| Day 3-4 | 2 | §6 type aliases + sum types (4 tests) |
| Day 4 | 1.5 | §7 records (3 tests) |
| Day 5-6 | 3 | §8 patterns + match (5 tests) |
| Day 6 | 1.5 | §9 do-blocks (3 tests) |
| Day 7 | 0.5 | §10 recursive let (1 test) |
| Day 7 | 0.5 | §11 no implicit widening (2 tests) |
| Day 8 | 1 | §12 end-to-end smoke test |
| Day 8 | 0.5 | bug fixes, clippy, fmt |

**Total:** ~16.5 hours of focused work over 8 days.

**Test budget:** 39 existing + 29 new = 68 typechecker tests.

---

## 14. Open questions (defer to after Day 10)

1. **Type classes / traits.** `Print` for `println`, `Eq` for `==`,
   `Ord` for `<`. The IR has no `Display` impls; println works
   on `Str` only in 0.1.
2. **Row polymorphism.** Records with `..rest` (the
   `{ x: 1, ..other }` pattern). Out of scope for 0.1.
3. **GADTs / dependent types.** Out of scope.
4. **Bidirectional typechecking.** Optional speed-up; not needed
   for 0.1.

---

## 15. Files to be added/modified

- `crates/typechecker/src/types.rs` — add `Effect`, helper methods
- `crates/typechecker/src/unify.rs` (new) — HM `Unifier`
- `crates/typechecker/src/infer.rs` — fill in all `infer_expr` arms
- `crates/typechecker/src/convert.rs` (new) — `TypeExpr` → `MonoType`
- `crates/typechecker/src/error.rs` — new `TypeError` variants
  (`LiteralOutOfRange`, `NoSuchField`, `NotASumType`, etc.)
- `crates/typechecker/tests/integration_test.rs` — add
  `all_examples_typecheck` smoke test

**No changes to other crates** (the IR contract is frozen).

---

## 16. Hand-off to lowerer (Day 9+)

Once the typechecker is done, the lowerer (in `crates/ir/src/lower.rs`)
takes a typed `Program` and produces an `IrModule`. Each `Decl::Bind`
becomes an `IrDecl::Function`; each `Decl::TypeAlias` becomes an
`IrDecl::TypeAlias`. The `MonoType` is converted to the corresponding
`IrType` (note: `MonoType::Effect` is preserved as `IrType::Effect`
in the IR for now; the codegen ignores it). Patterns become SSA
constructs (`TagConstruct` for constructors, `ArrayAlloc` for array
patterns, etc.).

The lowerer is **not part of this plan** — that's a separate document
(likely `LOWER_PLAN.md` once the typechecker is done).
