# Member 1: The Type Checker (Week 1 Deliverables)

**Crate Ownership:** `crates/typechecker`
**Mission:** Build a robust Hindley-Milner (HM) type inference engine. Focus on pure inference, environment management, and unification. Integration with the parser begins on Day 5.

## Architecture Overview

The typechecker crate is already scaffolded with the following modules:

| Module | Purpose | Status |
|--------|---------|--------|
| `types.rs` | `MonoType`, `PolyType`, `TypeId` | Complete |
| `env.rs` | Scoped `TypeEnv` with push/pop | Complete |
| `unify.rs` | `Substitution` + `unify()` algorithm | Stub (basic cases) |
| `infer.rs` | `infer_expr` + `infer_decl` | Stub (literals only) |
| `error.rs` | `TypeError` enum with `thiserror` | Complete |

### Key Types (already implemented)

```rust
// crates/typechecker/src/types.rs

pub struct TypeId(pub u32);         // unique type variable ID
pub struct PolyType {               // ∀a. a -> a
    pub quantified: Vec<TypeId>,
    pub body: MonoType,
}
pub enum MonoType {                 // concrete or variable types
    I8, I16, I32, I64,
    U8, U16, U32, U64, Usize,
    F32, F64,
    Bool, Str, Unit,
    Array(Box<MonoType>),
    Func { params: Vec<MonoType>, ret: Box<MonoType> },
    Record(Vec<(SmolStr, MonoType)>),
    Tag { name: SmolStr, payload: Vec<MonoType> },
    Var(TypeId),                    // unresolved type variable
}
```

```rust
// crates/typechecker/src/env.rs

pub struct TypeEnv {
    scopes: Vec<HashMap<SmolStr, PolyType>>,
    next_type_id: u32,
}
// methods: new(), fresh_var(), push_scope(), pop_scope(), insert(), lookup(), contains()
```

```rust
// crates/typechecker/src/error.rs

pub enum TypeError {
    UnificationFailed { expected, got, span },
    UnboundVariable { name, span },
    ArityMismatch { expected, got, span },
    InfiniteType { var, ty, span },
    AnnotationConflict { annotation, inferred, span },
    NonExhaustiveMatch { span },
    FieldNotFound { field, span },
    NumericOverflow { ty, span },
}
```

### Tests Already Passing (30 tests)

- `types.rs`: `mono_type_is_concrete`, `mono_type_with_var_is_not_concrete`, `is_numeric_true`, `is_numeric_false`, `poly_type_mono_helper`, `func_type_construction`
- `env.rs`: `lookup_in_global_scope`, `lookup_in_inner_scope`, `pop_scope_removes_bindings`, `inner_scope_shadows_outer`, `fresh_var_increments`, `scope_depth_tracks_correctly`
- `unify.rs`: `unify_same_concrete_type`, `unify_var_with_concrete`, `unify_concrete_with_var`, `unify_mismatched_concrete_types`, `unify_arrays`, `unify_functions`, `unify_arity_mismatch`, `substitution_apply_resolves_chain`
- `error.rs`: `unification_failed_display`, `unbound_variable_display`, `error_span_extraction`
- `infer.rs`: `infer_i32_literal`, `infer_bool_literal`, `infer_str_literal`, `infer_f64_literal`, `infer_unbound_variable`, `infer_binary_add_i32`, `infer_comparison_returns_bool`

## Week 1 Deliverables & Timeline

### Days 1-2: Unification Algorithm (Priority: HIGH)

**Goal:** Complete the `unify()` function so it handles all `MonoType` variants correctly.

**Task 1: Full occurs check in `unify()`**
```rust
// In unify.rs, the current stub is missing:
// 1. Occurs check: reject `a = Array(a)` (infinite type)
// 2. Record unification: field-by-field
// 3. Tag unification: name + payload element-wise
// 4. Chained variable resolution: if var maps to another var, follow the chain

fn occurs_check(var: TypeId, ty: &MonoType) -> bool {
    // Walk ty; return true if var appears anywhere inside
}

// Update unify() to call occurs_check before inserting var -> ty mapping
```

**Task 2: `Substitution::walk` helper**
```rust
// Add a method that resolves a type through the full substitution chain,
// not just one level. Currently `apply()` does this, but consider renaming
// to `walk()` for clarity and adding a `walk_var` helper.
```

**TDD approach:**
- Write `unify_records_same_fields` — two records with same field names/types unify
- Write `unify_records_different_types` — records with same names but different types fail
- Write `unify_tags_same_name` — tags with same name and matching payloads unify
- Write `unify_tags_different_name` — different tag names fail
- Write `occurs_check_infinite_array` — `a ~ [a]` fails
- Write `occurs_check_infinite_func` — `a ~ (a) -> a` fails
- Write `chained_var_resolution` — `a ~ b, b ~ i32` resolves `a` to `i32`

### Days 3-4: Expression Inference

**Goal:** Implement `infer_expr` for all expression variants.

**Task 3: Variables and let-bindings**
```rust
// In infer.rs, extend infer_expr:

Expr::Ident(name, span) => {
    match env.lookup(name) {
        Some(poly) => Ok(instantiate(poly, env)),  // instantiate with fresh vars
        None => Err(TypeError::UnboundVariable { name: name.to_string(), span: *span })
    }
}

Expr::Let { name, value, body, .. } => {
    let val_ty = infer_expr(env, value)?;
    let poly = generalize(env, val_ty);
    env.push_scope();
    env.insert(name, poly);
    let body_ty = infer_expr(env, body)?;
    env.pop_scope();
    Ok(body_ty)
}
```

**Task 4: Function inference**
```rust
Expr::Lambda { params, body, .. } => {
    env.push_scope();
    let param_tys: Vec<MonoType> = params.iter().map(|_| {
        MonoType::Var(env.fresh_var())
    }).collect();
    for (param, ty) in params.iter().zip(&param_tys) {
        env.insert(param.name.clone(), PolyType::mono(ty.clone()));
    }
    let ret_ty = infer_expr(env, body)?;
    env.pop_scope();
    Ok(MonoType::Func { params: param_tys, ret: Box::new(ret_ty) })
}

Expr::Apply { func, args, .. } => {
    let func_ty = infer_expr(env, func)?;
    let arg_tys: Vec<MonoType> = args.iter()
        .map(|a| infer_expr(env, a))
        .collect::<Result<_, _>>()?;
    let ret = MonoType::Var(env.fresh_var());
    let expected = MonoType::Func { params: arg_tys, ret: Box::new(ret.clone()) };
    let sub = unify(&func_ty, &expected)?;
    Ok(sub.apply(&ret))
}
```

**Task 5: Pattern matching**
```rust
Expr::Match { scrutinee, cases, .. } => {
    let scrutinee_ty = infer_expr(env, scrutinee)?;
    let result_ty = MonoType::Var(env.fresh_var());
    for (pattern, body) in cases {
        let pattern_ty = infer_pattern(env, pattern)?;
        let sub = unify(&scrutinee_ty, &pattern_ty)?;
        let body_ty = infer_expr(env, body)?; // after applying sub
        unify(&result_ty, &body_ty)?;
    }
    Ok(result_ty)
}
```

**TDD approach:**
- Write `infer_ident_bound` — lookup variable returns its type
- Write `infer_let_binding` — `let x = 5 in x` infers `i32`
- Write `infer_let_shadowing` — inner scope shadows outer
- Write `infer_lambda` — `(x) => x` infers `(?0) -> ?0`
- Write `infer_lambda_typed` — `(x: i32) => x` infers `(i32) -> i32`
- Write `infer_apply` — `add(1, 2)` where `add: (i32, i32) -> i32`
- Write `infer_apply_arity_mismatch` — wrong number of args fails
- Write `infer_if_else` — `if c then 1 else 2` requires bool condition, matching branches
- Write `infer_binary_mismatch` — `1 + "hello"` fails

### Day 5: Parser Integration

**Goal:** The lead architect hands you the working parser. Wire it into the typechecker.

**Task 6: `check_program` entry point**
```rust
// Add to lib.rs or a new check.rs module:

pub fn check_program(program: &Program) -> Result<(), Vec<TypeError>> {
    let mut env = TypeEnv::new();
    // Pre-populate stdlib builtins (add, sub, etc.) if available
    let mut errors = Vec::new();
    for decl in &program.declarations {
        match infer_decl(&mut env, decl) {
            Ok(poly) => { env.insert(decl.name(), poly); }
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Task 7: Replace manual AST construction in tests**
```rust
// Before (manual construction):
let bump = Bump::new();
let expr = Expr::i32(42, Span::new(0, 2), &bump);
let ty = infer_expr(&mut env, expr).unwrap();

// After (from parsed source):
let bump = Bump::new();
let tokens: Vec<_> = Lexer::new("let x = 42").collect();
let program = parse(&bump, &tokens).unwrap();
let result = check_program(&program);
assert!(result.is_ok());
```

**TDD approach:**
- Write `check_simple_binding` — `let x = 5` passes
- Write `check_type_annotation` — `let x : i32 = 5` passes
- Write `check_type_annotation_mismatch` — `let x : str = 5` fails
- Write `check_function` — `let add = (a, b) => a + b` infers `(i32, i32) -> i32`
- Write `check_recursive_function` — `let fact = (n) => if n == 0 then 1 else n * fact(n - 1)` passes
- Write `check_multiple_errors` — two errors reported, not just first

### Days 6-7: Advanced Features & Polish

**Task 8: Generic functions (let-polymorphism)**
```rust
// The identity function should be polymorphic:
// let id = (x) => x
// id(5) : i32   -- works
// id("hi") : str -- works

// This requires generalize() to create PolyType with quantified vars
fn generalize(env: &TypeEnv, ty: MonoType) -> PolyType {
    let free = ty.free_vars().difference(env.free_vars());
    PolyType::poly(free.collect(), ty)
}

fn instantiate(poly: &PolyType, env: &mut TypeEnv) -> MonoType {
    let mapping: HashMap<_, _> = poly.quantified.iter()
        .map(|v| (*v, MonoType::Var(env.fresh_var())))
        .collect();
    Substitution::from_mapping(mapping).apply(&poly.body)
}
```

**Task 9: Type annotations on declarations**
```rust
// Handle Decl::TypeSig:
// let transition : AppState -> Event -> Effect<AppState>
// transition = (state, event) => ...

// When a TypeSig is encountered:
// 1. Parse the type annotation into a MonoType
// 2. Insert it into the environment as a PolyType
// 3. When the corresponding Bind is processed, unify the inferred type
//    with the annotated type
```

**TDD approach:**
- Write `infer_identity_polymorphic` — `id(5)` and `id("hi")` both work
- Write `infer_const_polymorphic` — `const(a, b) = a` works for any types
- Write `check_type_sig_applied` — annotation is checked against inferred
- Write `check_type_sig_conflict` — mismatch produces `AnnotationConflict`
- Write `infer_recursive_function` — factorial-like recursion works
- Write `infer_mutual_recursion` — `let f = (x) => g(x); let g = (x) => x + 1`

## Common Pitfalls

1. **Forgetting occurs check** — will cause infinite loops in unification
2. **Not instantiating polymorphic types** — `id` would only work for one type
3. **Scope leaking** — always `pop_scope` in a `finally`-like pattern
4. **Variable capture** — when instantiating, ensure fresh vars don't collide

## Dependencies

- `ast` crate: `Expr`, `Decl`, `BinOp`, `Pattern`, `LiteralPattern`, `SmolStr`
- `ast::span::Span`: for error locations
- `bumpalo` (dev-dependency): for manual AST construction in tests
- `thiserror`: already in Cargo.toml
