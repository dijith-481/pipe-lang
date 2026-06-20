# Member 2 — Phase 2: Stdlib Completeness + Pattern Exhaustiveness

**Crate Ownership:** `crates/stdlib`  
**Scope note:** Dijith already registered prelude types (including `readLine`, `drop`, `take`, `sqrt`, `unwrap`) and owns IR lowering. `crates/ir` and `crates/typechecker` are Dijith's domain; Member 2's exhaustiveness module is a new file that calls into existing infer.rs APIs.  
**Prerequisite:** dijith-phase2 (typechecker prelude registration) must be complete  
**Timeline:** 3 days (parallel with Member 1, Member 3 after Dijith handoff)  
**Goal:** All builtins used by example programs are implemented and registered. Pattern exhaustiveness is checked at compile time. Clippy warnings resolved.

---

## Scope

The critical pipe-lang v0.1 feature set is **narrower** than the original Phase 2 plan. After the codebase audit, we know:

| Feature | Original Phase 2 | Revised Phase 2 | Reason |
|---|---|---|---|
| Stdlib completion | ✔ | **✔ HIGHEST PRIORITY** | Example programs depend on `drop`, `take`, `sqrt`, `unwrap`, `readLine` |
| Typechecker prelude alignment | ✔ | → Dijith phase 2 | Dijith handles all 27 missing type signatures |
| Pattern exhaustiveness | ✔ | **✔ HIGH PRIORITY** | Needed for `match` safety in patterns.pp, state-machine.pp |
| First-class tuples | ✔ | ❌ **Deferred to Phase 3** | Tag hack works for v0.1 |
| Effect<T> type | ✔ | ❌ **Deferred to Phase 3** | IO builtins execute immediately for v0.1 |
| Clippy warnings | — | **✔ MUST FIX** | `cargo clippy -- -D warnings` is a hard requirement |

---

## Current State

**Stdlib** (`crates/stdlib/src/prelude.rs`):
- 33 builtins registered
- **Missing** (needed by example programs): `drop`, `take`, `sqrt`, `unwrap`
- **Name mismatch**: `readLine` is used by `io-effects.pp` but registry only has `read_line`
- **Type dispatch**: `map`/`flatMap` on Option/Result values resolves to array versions (desugaring strips type info). Programs calling `.map()` on Option (like `option-result.pp`) will fail at runtime.

**Pattern exhaustiveness**: Not implemented. `match` expressions are never checked for coverage.

**Clippy**: 3 warnings in `crates/stdlib/src/array.rs` — non-snake-case test function names.

---

## Day 1 — Morning (Hours 0–4): Missing Stdlib Builtins

### Task 2.1: Add `drop` builtin

**File:** `crates/stdlib/src/array.rs`

```rust
#[derive(Debug)]
pub struct ArrayDrop;

impl BuiltinFunction for ArrayDrop {
    fn name(&self) -> &str { "drop" }
    fn arity(&self) -> usize { 2 }
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let arr = args[0].as_array_ref().ok_or_else(||
            format!("`drop` expected Array as first argument, got {:?}", args[0])
        )?;
        let n = match &args[1] {
            Value::I32(v) => *v as usize,
            Value::Usize(v) => *v,
            other => return Err(format!("`drop` expected integer, got {other:?}")),
        };
        if n >= arr.len() {
            return Ok(Value::array(Vec::new()));
        }
        let values: Vec<Value> = arr[n..].to_vec();
        Ok(Value::array(values))
    }
}
```

**Tests:**
```rust
#[test]
fn array_drop_zero_elements() {
    let arr = Value::array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
    let result = ArrayDrop.execute(&[arr, Value::I32(0)]).unwrap();
    assert_eq!(result.as_array_ref().unwrap().len(), 3);
}

#[test]
fn array_drop_some_elements() {
    let arr = Value::array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
    let result = ArrayDrop.execute(&[arr, Value::I32(2)]).unwrap();
    assert_eq!(result.as_array_ref().unwrap().len(), 1);
}

#[test]
fn array_drop_all_elements() {
    let arr = Value::array(vec![Value::I32(1), Value::I32(2)]);
    let result = ArrayDrop.execute(&[arr, Value::I32(5)]).unwrap();
    assert!(result.as_array_ref().unwrap().is_empty());
}
```

### Task 2.2: Add `take` builtin

**File:** `crates/stdlib/src/array.rs`

```rust
#[derive(Debug)]
pub struct ArrayTake;

impl BuiltinFunction for ArrayTake {
    fn name(&self) -> &str { "take" }
    fn arity(&self) -> usize { 2 }
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        let arr = args[0].as_array_ref().ok_or_else(||
            format!("`take` expected Array as first argument, got {:?}", args[0])
        )?;
        let n = match &args[1] {
            Value::I32(v) => *v as usize,
            Value::Usize(v) => *v,
            other => return Err(format!("`take` expected integer, got {other:?}")),
        };
        let end = n.min(arr.len());
        let values: Vec<Value> = arr[..end].to_vec();
        Ok(Value::array(values))
    }
}
```

**Tests:**
```rust
#[test] fn array_take_zero()
#[test] fn array_take_some()
#[test] fn array_take_more_than_len()
```

### Task 2.3: Add `sqrt` builtin

**File:** `crates/stdlib/src/numeric.rs`

```rust
#[derive(Debug)]
pub struct Sqrt;

impl BuiltinFunction for Sqrt {
    fn name(&self) -> &str { "sqrt" }
    fn arity(&self) -> usize { 1 }
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        match &args[0] {
            Value::F32(v) => Ok(Value::F32(v.sqrt())),
            Value::F64(v) => Ok(Value::F64(v.sqrt())),
            other => Err(format!("`sqrt` expected float, got {other:?}")),
        }
    }
}
```

**Tests:**
```rust
#[test] fn sqrt_f64_positive()
#[test] fn sqrt_f64_zero()
#[test] fn sqrt_f32()
#[test] fn sqrt_rejects_non_float()
```

### Task 2.4: Add `unwrap` builtin

**File:** `crates/stdlib/src/option.rs` or new `crates/stdlib/src/ops.rs`

```rust
#[derive(Debug)]
pub struct Unwrap;

impl BuiltinFunction for Unwrap {
    fn name(&self) -> &str { "unwrap" }
    fn arity(&self) -> usize { 2 }
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        expect_arity(self.name(), args, self.arity())?;
        // unwrap(value, default_or_error_msg)
        match &args[0] {
            Value::Tag { disc, payload, .. } if *disc == 0 => {
                // Some(T) or Ok(T) — assuming disc 0 is the success variant
                Ok(payload[0].clone())
            }
            Value::Tag { .. } => {
                // None or Err — return the default value (second arg)
                Ok(args[1].clone())
            }
            other => Err(format!("`unwrap` expected Option or Result, got {other:?}")),
        }
    }
}
```

**Important**: This assumes the runtime representation of tags follows a consistent discriminant scheme (0 for success/Some/Ok, 1 for None/Err). Verify the JIT's `TagConstruct` uses this scheme.

Alernative: Use structural matching on the tag name (check if it's `Some`, `None`, `Ok`, `Err`) instead of discriminant numbers. This is more robust:

```rust
fn unwrap_impl(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::Tag { name, payload, .. } if matches!(name.as_str(), "Some" | "Ok") => {
            Ok(payload[0].clone())
        }
        Value::Tag { name, .. } if matches!(name.as_str(), "None" | "Err") => {
            Ok(args[1].clone())
        }
        other => Err(format!("`unwrap` expected Option or Result, got {other:?}")),
    }
}
```

**Tests:**
```rust
#[test] fn unwrap_some_returns_value()
#[test] fn unwrap_none_returns_default()
#[test] fn unwrap_ok_returns_value()
#[test] fn unwrap_err_returns_default()
#[test] fn unwrap_rejects_non_tag()
```

### Task 2.5: Add `readLine` alias for `read_line`

**File:** `crates/stdlib/src/io.rs`

The `io-effects.pp` program calls `io.readLine()` which desugars to `readLine(io)`. But actually, `io.readLine()` with the `io.` prefix... Let's check the parser desugaring.

`io.readLine()` parses as: `App { func: FieldAccess { object: ident("io"), field: "readLine" }, args: [] }`.

The method call desugaring rewrites `obj.method(args)` → `method(obj, args)`. So `io.readLine()` → `readLine(io)`.

This means `readLine` would be called with `io` (the module binding) as an argument. The `Decl::Use` handler currently stores `Unit` for `io`, so `io` would be of type `()`.

**Fix:** The simplest approach is to register `readLine` as a no-argument builtin that doesn't look at the module argument:

```rust
#[derive(Debug)]
pub struct ReadLine;

impl BuiltinFunction for ReadLine {
    fn name(&self) -> &str { "readLine" }
    fn arity(&self) -> usize { 1 } // Takes the `io` module as arg (ignored)
    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        // Ignore args[0] (the `io` module value)
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map_err(|e| format!("IO error: {e}"))?;
        Ok(Value::Str(input.trim_end().to_string().into()))
    }
}
```

Actually, wait. The `Decl::Use` handler returns `Unit` for now. And `io.readLine()` desugars to `readLine(io)`. If `io` has type `()`, then the typechecker expects `readLine : (()) -> str`. But we registered `readLine : () -> str` (zero args). This will fail unification.

There are two approaches:
1. **Change `Decl::Use` to not consume the module argument.** Instead of `use stdlib::io` creating a binding `io : ()`, it could inject `readLine` directly into scope. Then `io.readLine()` → `readLine(io)` would be `readLine(())` if `io` is still `()`.
2. **Change the typechecker to handle module-qualified names.** Make `io.readLine()` resolve to the `readLine` builtin through module resolution, stripping the qualifier.

For Phase 2, approach 1 is simpler but changes how `use` works. Approach 2 is more architecturally correct.

**Pragmatic fix for Phase 2:** Change `Decl::Use` in the typechecker to not create a binding for the module name. Instead, just make it a no-op. Then change `io-effects.pp` to call `readLine()` directly (without `io.` prefix), since `println()` is also called directly without module prefix.

OR: Register BOTH `readLine` and `read_line` in the runtime. The `use stdlib::io` statement can just inject both `readLine` and `read_line` into scope.

**Simplest fix:** Change `io-effects.pp` to use `readLine()` without `io.` prefix. The `use stdlib::io` line can stay (it's a no-op). This matches how `println` is used without `io.` prefix.

**Add to `prelude_builtins()`:**
```rust
// readLine is just a preregistered builtin, no module prefix needed
Arc::new(io::ReadLine),
```

**Also register in typechecker prelude** (Dijith's scope from dijith-phase2.md — coordinate):
```rust
// readLine : () -> str
let read_line_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([]),
    ret: Rc::new(MonoType::Str),
});
self.insert("readLine", read_line_ty);
```

### Task 2.6: Register all new builtins in prelude

**File:** `crates/stdlib/src/prelude.rs`, in `prelude_builtins()`:

```rust
pub fn prelude_builtins() -> Vec<Arc<dyn BuiltinFunction>> {
    vec![
        // ... existing 33 ...
        Arc::new(array::ArrayDrop),
        Arc::new(array::ArrayTake),
        Arc::new(numeric::Sqrt),
        Arc::new(ops::Unwrap),
        Arc::new(io::ReadLine),
    ]
}
```

### Tests for Tasks 2.1–2.6

```rust
#[test] fn stdlib_drop()
#[test] fn stdlib_take()
#[test] fn stdlib_sqrt()
#[test] fn stdlib_unwrap_some()
#[test] fn stdlib_unwrap_none()
#[test] fn stdlib_readline_exists()
#[test] fn prelude_has_all_builtin_names_drop()  // verify new names registered
```

---

## Day 1 — Mid (Hours 4–8): Stdlib `map`/`flatMap` Type Dispatch

### Task 2.7: The `map`/`flatMap` overloading problem

**Context:** `option-result.pp` calls `.map()` and `.flatMap()` on Option/Result values. The parser desugars `opt.map(f)` → `map(opt, f)` — the same name as the array `map` builtin.

**Problem:** The runtime has:
- `map` (registered as array `map`) — expects `Array<a>` first arg
- `Option.map` — expects `Option<a>` first arg  
- `Result.map` — expects `Result<a>` first arg

But `opt.map(f)` calls `map(opt, f)`, which reaches the array `map` and fails.

**Solution:** Make the `map` builtin (and `flatMap`) perform **type dispatch at runtime**:

```rust
// In array.rs, modify Map::execute:
fn execute(&self, args: &[Value]) -> Result<Value, String> {
    expect_arity(self.name(), args, self.arity())?;
    match &args[0] {
        Value::Tag { name, payload, .. } if name == "Option" || name == "Result" => {
            // Delegate to Option.map or Result.map
            if name == "Option" {
                OptionMap.execute(args)
            } else {
                ResultMap.execute(args)
            }
        }
        _ => {
            // Original array map logic
            let arr = args[0].as_array_ref().ok_or_else(|| ...)?;
            // ... existing array map impl ...
        }
    }
}
```

Similarly for `flatMap`.

This is a pragmatic solution. In Phase 3, a proper type-based method resolution pass in the typechecker/lowerer would handle this more elegantly.

**Better approach:** Instead of modifying `map`/`flatMap` themselves, create a **single dispatch builtin** that all three call through:

```rust
// In crate/stdlib/src/dispatch.rs (new module)
pub fn dispatch_map(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::Array(_) => array::map_impl(args),
        Value::Tag { name, .. } if name == "Option" => option::OptionMap.execute(args),
        Value::Tag { name, .. } if name == "Result" => result::ResultMap.execute(args),
        other => Err(format!("`map` not defined for {other:?}")),
    }
}

pub fn dispatch_flatMap(args: &[Value]) -> Result<Value, String> {
    // Similar
}
```

**Actual implementation:** Modify `crates/stdlib/src/array.rs` Map and FlatMap to do type-based dispatch. Extract the core logic into inner functions (`array_map_impl`, `option_map_impl`, `result_map_impl`) that the dispatch wrapper delegates to.

**Important:** The typechecker signatures for `map` and `flatMap` in the prelude (Dijith's scope) need to be compatible. Currently:
- `map : <a,b>(Array<a>, (a) -> b) -> Array<b>`
- `Option.map : <a,b>(Option<a>, (a) -> b) -> Option<b>`

These have different return types. The typechecker will unify the return type with `Array<b>` if `map` is used with an Option arg, causing a type error. So this dispatch approach only works **at runtime** — typechecking will still fail.

**Better approach for Phase 2:** After Dijith's typechecker fixes, verify whether `opt.map(f)` actually typechecks. It may fail at typecheck even before reaching the JIT.

If it does fail at typecheck, the options are:
1. **Change the example programs** to use `Option.map(opt, f)` syntax (qualified name)
2. **Add type-based method resolution** in the typechecker (significant work — Phase 3)
3. **Register `map` with a flexible signature** that unifies with both — not possible in HM

**Recommendation for Phase 2:** 
1. Implement runtime dispatch for `map`/`flatMap` (in case programs reach JIT)
2. If typechecker rejects `.map()` on Option, modify `option-result.pp` and `io-effects.pp` to use `Option.map(opt, f)` syntax
3. Document the method resolution gap for Phase 3

### Task 2.8: Update `option-result.pp` example program

If type-based method resolution isn't implemented, update the example to use qualified syntax:

```diff
- greetUser(1).unwrap(`User not found`)
+ Option.unwrapOr(greetUser(1), `User not found`)
```

And for `.map()`:
```diff
- findUser(userId).map((user) => `Hello, ${user.name}!`)
+ Option.map(findUser(userId), (user) => `Hello, ${user.name}!`)
```

Similarly for `io-effects.pp`:
```diff
- io.readLine().flatMap(...)
+ flatMap(readLine(), ...)
```

---

## Day 1 — Late (Hours 8–12): Fix Clippy Warnings

### Task 2.9: Rename non-snake-case test functions

**File:** `crates/stdlib/src/array.rs`, lines 488, 510, 526

```diff
- fn flatMap_flattens_mapped_arrays()
+ fn flat_map_flattens_mapped_arrays()
- fn flatMap_returns_empty_for_empty_array()
+ fn flat_map_returns_empty_for_empty_array()
- fn flatMap_rejects_non_array_result()
+ fn flat_map_rejects_non_array_result()
```

Also check for any other clippy warnings in `crates/stdlib/` and `crates/typechecker/`:

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | grep "warning:"
```

---

## Day 2 — Morning (Hours 0–6): Pattern Exhaustiveness Checking

### Task 2.10: Create `crates/typechecker/src/exhaustiveness.rs`

**Algorithm:** Use the SLYM ("Synthetic Lattice of Y") approach — track "uncovered" patterns as a set and subtract each arm's coverage.

**Core types:**

```rust
/// Represents the set of values covered by a pattern (or not yet covered).
#[derive(Debug, Clone)]
enum PatternSet {
    /// Everything (top).
    Wild,
    /// Nothing (bottom).
    Empty,
    /// For sum types: per-variant coverage.
    /// Missing variants are uncovered.
    Variants(HashMap<SmolStr, Box<PatternSet>>),
    /// For integer types: ranges covered.
    IntRanges(Vec<(i64, i64)>),
    /// For string types: set of exact strings.
    StrValues(HashSet<String>),
    /// For floating: never exhaustive without wildcard.
    Float,
}
```

**Exhaustiveness check entry point:**

```rust
/// Check that the match arms cover all possible values of `subject_type`.
///
/// Returns `Ok(())` if coverage is complete.
/// Returns `Err(NonExhaustiveMatch { span, missing })` if not.
pub fn check_exhaustive(
    subject_type: &MonoType,
    arms: &[MatchArm],
    match_span: Span,
) -> Result<(), Vec<TypeError>> {
    let uncovered = compute_uncovered(subject_type, arms)?;
    if uncovered.is_empty() {
        Ok(())
    } else {
        // Try to generate a human-readable "missing" description
        Err(vec![TypeError::NonExhaustiveMatch { span: match_span }])
    }
}

fn compute_uncovered(
    subject_type: &MonoType,
    arms: &[MatchArm],
) -> Result<PatternSet, Vec<TypeError>> {
    let mut uncovered = initial_pattern_set(subject_type);
    for arm in arms {
        let covered = pattern_to_set(arm.pattern, subject_type)?;
        uncovered = subtract(&uncovered, &covered);
        if uncovered.is_empty() {
            return Ok(PatternSet::Empty);
        }
    }
    Ok(uncovered)
}
```

**Key operations:**

```rust
fn initial_pattern_set(ty: &MonoType) -> PatternSet {
    match ty {
        MonoType::I32 | MonoType::I64 | MonoType::U32 | MonoType::U64
        | MonoType::Usize | MonoType::I8 | MonoType::I16
        | MonoType::U8 | MonoType::U16 => {
            PatternSet::IntRanges(vec![(i64::MIN, i64::MAX)])
        }
        MonoType::Bool => PatternSet::IntRanges(vec![(0, 0), (1, 1)]),
        MonoType::Str => PatternSet::Wild,  // Can't exhaust strings without wildcard
        MonoType::F32 | MonoType::F64 => PatternSet::Float,
        MonoType::Unit => PatternSet::Wild,  // Only one value
        MonoType::Tag { name, .. } => {
            // For sum types: each variant starts as uncovered
            PatternSet::Variants(HashMap::new())  // Filled from tag_variants
        }
        _ => PatternSet::Wild,
    }
}

fn pattern_to_set(pat: &Pattern, subject_ty: &MonoType) -> Result<PatternSet, Vec<TypeError>> {
    match pat {
        Pattern::Wildcard(_) | Pattern::Binding(_, _) => Ok(PatternSet::Wild),
        Pattern::Literal(lit, _) => match lit {
            LiteralPattern::Int(v) => Ok(PatternSet::IntRanges(vec![(*v as i64, *v as i64)])),
            LiteralPattern::Bool(b) => Ok(PatternSet::IntRanges(vec![(*b as i64, *b as i64)])),
            LiteralPattern::Str(s) => Ok(PatternSet::StrValues(HashSet::from([s.to_string()]))),
        },
        Pattern::Constructor { name, args, .. } => {
            // Covers a specific variant
            let inner_sets: Result<Vec<PatternSet>, _> = args.iter()
                .map(|a| pattern_to_set(a, &MonoType::Unit)) // simplified
                .collect();
            let inner = inner_sets?;
            Ok(PatternSet::Variants(HashMap::from([
                (SmolStr::from(*name), Box::new(PatternSet::Wild))  // simplified
            ])))
        }
        _ => Ok(PatternSet::Wild), // Conservative fallback for complex patterns
    }
}

fn subtract(a: &PatternSet, b: &PatternSet) -> PatternSet {
    match (a, b) {
        (_, PatternSet::Wild) => PatternSet::Empty,
        (PatternSet::Empty, _) => PatternSet::Empty,
        (PatternSet::Wild, _) => PatternSet::Wild,  // Can't subtract from Wild without concrete info
        (PatternSet::Variants(a_vars), PatternSet::Variants(b_vars)) => {
            // Remove covered variants from uncovered set
            let mut result = a_vars.clone();
            for (name, b_inner) in b_vars {
                match result.get(name) {
                    Some(a_inner) => {
                        let remaining = subtract(a_inner, b_inner);
                        if remaining.is_empty() {
                            result.remove(name);
                        } else {
                            result.insert(name.clone(), Box::new(remaining));
                        }
                    }
                    None => { /* variant not in uncovered set */ }
                }
            }
            if result.is_empty() {
                PatternSet::Empty
            } else {
                PatternSet::Variants(result)
            }
        }
        (PatternSet::IntRanges(ranges), PatternSet::IntRanges(single)) if single.len() == 1 => {
            // Subtract a single integer range [v, v] from a set of ranges
            let (lo, hi) = single[0];
            let mut result = Vec::new();
            for &(rlo, rhi) in ranges {
                if hi < rlo || lo > rhi {
                    result.push((rlo, rhi));  // No overlap
                } else {
                    if rlo < lo { result.push((rlo, lo - 1)); }
                    if rhi > hi { result.push((hi + 1, rhi)); }
                }
            }
            if result.is_empty() {
                PatternSet::Empty
            } else {
                PatternSet::IntRanges(result)
            }
        }
        _ => PatternSet::Wild,  // Conservative fallback
    }
}

impl PatternSet {
    fn is_empty(&self) -> bool {
        matches!(self, PatternSet::Empty)
    }
}
```

### Task 2.11: Integrate exhaustiveness into typechecker

**In `infer.rs`, where `Match` expressions are processed (around line 580–615):**

After inferring the arm types but before returning, insert:

```rust
// After all arms are inferred and unified, check exhaustiveness
if let Err(errs) = check_exhaustive(&subj_type_applied, arms, *span) {
    // Collect exhaustiveness errors alongside type errors
    // For now, just return the first one
    if let Some(err) = errs.into_iter().next() {
        return Err(err);
    }
}
```

**Note:** The typechecker's `infer` function returns `Result<MonoType, TypeError>`, but `check_exhaustive` returns `Result<(), Vec<TypeError>>`. Adapt accordingly — convert the single error case or collect all errors.

**File:** `crates/typechecker/src/infer.rs` — add `use crate::exhaustiveness::check_exhaustive;` at the top.

**File:** `crates/typechecker/src/lib.rs` — add `pub mod exhaustiveness;`

### Task 2.12: Tests for exhaustiveness

```rust
// In crates/typechecker/src/exhaustiveness.rs

#[test]
fn exhaustive_on_bool() {
    // match x { true => 1, false => 2 } — covers both
    // ...
}

#[test]
fn nonexhaustive_on_bool_missing_false() {
    // match x { true => 1 } — fails
}

#[test]
fn exhaustive_on_option() {
    // match x { Some(v) => v, None => 0 } — covers both
}

#[test]
fn exhaustive_with_wildcard() {
    // match x { Some(v) => v, _ => 0 } — wildcard covers None
}

#[test]
fn nonexhaustive_on_custom_sum_type() {
    // type Shape = Circle(f64) | Rect(f64, f64)
    // match s { Circle(r) => r } — missing Rect
}

#[test]
fn exhaustive_on_i32_with_wildcard() {
    // match n { 0 => "zero", _ => "other" } — covers all
}

#[test]
fn nonexhaustive_on_i32_no_wildcard() {
    // match n { 0 => "zero", 1 => "one" } — 2..MAX missing
}

#[test]
fn exhaustive_on_unit() {
    // match () { _ => 0 } — only one value
}

#[test]
fn exhaustive_multiple_arms_same_variant() {
    // match x { Some(1) => "one", Some(_) => "other", None => "none" }
}
```

---

## Day 2 — Mid (Hours 6–10): Assertion Tests

### Task 2.13: 40+ comprehensive tests

**Stdlib tests (15 new):**
```rust
#[test] fn stdlib_drop_negative_index()
#[test] fn stdlib_take_exceeds_length()
#[test] fn stdlib_sqrt_negative()
#[test] fn stdlib_unwrap_nested_option()
#[test] fn stdlib_unwrap_result_ok()
#[test] fn stdlib_unwrap_result_err()
#[test] fn stdlib_dispatch_map_on_array()
#[test] fn stdlib_dispatch_map_on_option()
#[test] fn stdlib_dispatch_map_on_result()
#[test] fn stdlib_dispatch_flatmap_on_array()
#[test] fn stdlib_dispatch_flatmap_on_option()
#[test] fn stdlib_readline_registered()
#[test] fn stdlib_map_rejects_invalid_type()
#[test] fn stdlib_drop_non_array_rejected()
#[test] fn stdlib_take_non_array_rejected()
```

**Exhaustiveness tests (10):**
```rust
#[test] fn exhaustive_full_variant_coverage()
#[test] fn exhaustive_wildcard_fallback()
#[test] fn exhaustive_literal_ranges()
#[test] fn exhaustive_bool_both()
#[test] fn nonexhaustive_bool_one_branch()
#[test] fn nonexhaustive_sum_missing_variant()
#[test] fn nonexhaustive_int_no_wildcard()
#[test] fn exhaustive_nested_variants()
#[test] fn exhaustive_multiple_arms()
#[test] fn exhaustive_empty_match()
```

---

## Day 2 — Late (Hours 10–12): Integration + Verification

### Task 2.14: Run full test suite

```bash
cargo test --lib stdlib
cargo test --lib typechecker
cargo clippy -- -D warnings
cargo fmt --check
```

### Task 2.15: Verify example programs compile

```bash
# After Dijith's typechecker fixes + Member 1's JIT fixes:
cargo run -- check example-programs/sorting.pp
cargo run -- check example-programs/patterns.pp
cargo run -- check example-programs/option-result.pp
cargo run -- check example-programs/io-effects.pp
```

Expected: All programs typecheck (even if some can't run fully yet).

---

## Deliverables

1. `crates/stdlib/src/array.rs` — `drop`, `take` builtins + type-dispatch for `map`/`flatMap`
2. `crates/stdlib/src/numeric.rs` — `sqrt` builtin
3. `crates/stdlib/src/ops.rs` — `unwrap` builtin (new file, or use existing module)
4. `crates/stdlib/src/io.rs` — `readLine` builtin (alias for `read_line`)
5. `crates/stdlib/src/prelude.rs` — Register all 5 new builtins
6. `crates/typechecker/src/exhaustiveness.rs` — New module with pattern exhaustiveness checker
7. `crates/typechecker/src/lib.rs` — `pub mod exhaustiveness` export
8. `crates/typechecker/src/infer.rs` — Integrate exhaustiveness check into `match` expression type inference
9. `crates/stdlib/src/array.rs` — Fix 3 clippy non-snake-case warnings
10. 25+ new tests across stdlib and typechecker

---

## Effect<T> System Changes

**Dijith has already completed all Effect type-system work.** You do not need to add `Effect<T>` to any types.

Key changes affecting Member 2:
1. **Prelude signatures are already updated** — `println : (str) -> Effect<()>`, `readLine : () -> Effect<str>`, etc. `Effect.map` and `Effect.flatMap` type signatures are also registered.
2. **IO builtins in stdlib** must now return `Value::Effect(...)` instead of executing immediately. The `Value::Effect(Arc<dyn BuiltinFunction>)` variant already exists in the runtime. For v0.1, a pragmatic approach is to execute the builtin immediately but wrap the result:
   ```rust
   // Instead of: Ok(Value::Unit)
   // Return:    Ok(Value::Effect(Arc::new(MyIoBuiltin { args })))
   ```
   The runtime's `call_main()` treats Effect returns as success.
3. **Add `Effect.map` and `Effect.flatMap` runtime builtins** that create composite effects. For v0.1, these can execute immediately and return the inner value (effectively identity wrappers).
4. **`readLine`** is already registered as `() -> Effect<str>` — implement the runtime builtin similarly.

## Coordination Notes

- **Dijith** has already registered ALL prelude type signatures including `readLine`, `drop`, `take`, `sqrt`, `unwrap`, `Effect.map`, `Effect.flatMap`. Just implement the runtime builtins.
- **Member 1** has the builtin bridge (`pipe_rt_call_builtin`) which calls `global_registry().find(name)`. Any name you register in `prelude_builtins()` becomes available to JIT code.
- The `map`/`flatMap` type dispatch issue may require updating example programs if typechecker can't resolve method calls on Option/Result types. This is acceptable for v0.1.
- Pattern exhaustiveness generates `TypeError::NonExhaustiveMatch` which the diagnostics crate (Member 3) must handle with a `CompilerError::NonExhaustiveMatch` variant.
