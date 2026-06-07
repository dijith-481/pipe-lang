# JIT Implementation Plan & Language API Spec Contract

## Current State

The compiler pipeline is **fully wired**: lex → parse → typecheck → lower → IR. The lowerer is **feature-complete** — it emits IR for every language construct. The JIT is a **skeleton** — it handles exactly `ConstI32` + `Return` in single-block functions. Nothing beyond `let x = 42` actually executes.

---

## Phase 1: Primitive Operations (Days 1-2)

**Goal:** All constant types, arithmetic, comparison, logical, and unary ops work.

### IR Instructions to Implement

| Instruction | Cranelift Mapping |
|---|---|
| `ConstI8(v)` | `iconst(I8, v)` |
| `ConstI16(v)` | `iconst(I16, v)` |
| `ConstI32(v)` | `iconst(I32, v)` ✅ (already done) |
| `ConstI64(v)` | `iconst(I64, v)` |
| `ConstU8(v)` | `iconst(U8, v)` |
| `ConstU16(v)` | `iconst(U16, v)` |
| `ConstU32(v)` | `iconst(U32, v)` |
| `ConstU64(v)` | `iconst(U64, v)` |
| `ConstUsize(v)` | `iconst(I64, v)` (usize → i64 for Cranelift) |
| `ConstF32(v)` | `f32const(v)` |
| `ConstF64(v)` | `f64const(v)` |
| `ConstBool(v)` | `bconst(v)` |
| `ConstUnit` | `iconst(I32, 0)` (unit = zero-width, use 0 as placeholder) |
| `Add(a, b)` | `iadd(a, b)` / `fadd(a, b)` |
| `Sub(a, b)` | `isub(a, b)` / `fsub(a, b)` |
| `Mul(a, b)` | `imul(a, b)` / `fmul(a, b)` |
| `Div(a, b)` | `sdiv(a, b)` / `fdiv(a, b)` |
| `Rem(a, b)` | `srem(a, b)` / `frem(a, b)` |
| `Neg(a)` | `ineg(a)` / `fneg(a)` |
| `Eq(a, b)` | `icmp eq, a, b` / `fcmp eq, a, b` |
| `Ne(a, b)` | `icmp ne, a, b` / `fcmp ne, a, b` |
| `Lt(a, b)` | `icmp slt, a, b` / `fcmp lt, a, b` |
| `Le(a, b)` | `icmp sle, a, b` / `fcmp le, a, b` |
| `Gt(a, b)` | `icmp sgt, a, b` | `fcmp gt, a, b` |
| `Ge(a, b)` | `icmp sge, a, b` / `fcmp ge, a, b` |
| `And(a, b)` | `band(a, b)` |
| `Or(a, b)` | `bor(a, b)` |
| `Not(a)` | `bnot(a)` |

### Calling Convention Fix

The current JIT ignores `args_ptr`. For Phase 1, keep it simple: `main()` takes no args, returns i32 via `ret_ptr`. Fix `call_main()` to pass a 4-byte buffer and read the i32 result.

### Tests

- `let x = 42` — already works
- `let x = 1 + 2` — arithmetic
- `let x = 5 > 3` — comparison returns bool
- `let f = (x: i32) => x + 1` — lambda with param + arithmetic
- `let f = (x: i32) => if x > 0 { x } else { 0 - x }` — needs Phase 2

---

## Phase 2: Control Flow (Days 2-3)

**Goal:** Multi-block functions, if/else, match, blocks work.

### Terminators to Implement

| Terminator | Cranelift Mapping |
|---|---|
| `Return(v)` | `ret(v)` ✅ (already done) |
| `Jump { target, args }` | `jump(block, &[args])` + block params as Cranelift block args |
| `Branch { condition, then_block, else_block, ... }` | `brif(condition, then_block, then_args, else_block, else_args)` |
| `Switch { discriminant, arms, default }` | `br_table(discriminant, default, &[arm_targets])` |
| `Unreachable` | `trap(TrapCode::UnreachableCodeReachable)` |

### Block Parameters (Phi Nodes)

Cranelift uses block parameters for phi nodes. When the IR has:
```
bb1(v: i32):
  ...
```
Cranelift needs `block0.append_block_param(I32)` during the block declaration phase.

**Implementation approach:**
1. Pre-declare all blocks with their parameters in a first pass
2. Then emit instructions in a second pass
3. Map our `ValueId` → Cranelift `Value` in a HashMap

### Switch Implementation

`Switch` maps to Cranelift's `br_table`. For N-arm switch:
1. Create a jump table with N entries
2. Each entry maps to the corresponding block
3. Use `br_table(discriminant, default_block, &table)` 

For small switches (≤4 arms), consider lowering to chained `brif` for better codegen.

### Tests

- `let f = (x) => if x > 0 { x } else { 0 - x }` — if/else
- `let f = (x) => match x { true => 1, _ => 0 }` — match on bool
- `let f = (x) => { let y = x + 1; y }` — block with let stmt
- `let fact = (n) => if n == 0 { 1 } else { n * fact(n - 1) }` — recursion (needs Phase 4 for calls)

---

## Phase 3: Function Calls (Days 3-4)

**Goal:** Named calls, indirect calls (closures), recursion work.

### Instructions to Implement

| Instruction | Cranelift Mapping |
|---|---|
| `CallNamed { name, args }` | `call(func_id, &[args])` — resolve name to Cranelift FuncId |
| `CallIndirect { callee, args }` | `call_indirect(sig, callee_ptr, &[args])` — indirect call via function pointer |
| `MakeClosure { func_name, captures }` | Allocate closure struct: `[func_ptr, capture1, capture2, ...]` |

### Function Declaration

In the Cranelift module, each `IrFunction` becomes:
1. A `FuncId` declared with the correct signature
2. A `Function` built with `FunctionBuilderContext`
3. Translated block-by-block

### Named Call Resolution

`CallNamed` resolves to:
- Another function in the same `IrModule` → Cranelift `FuncId`
- A builtin (registered in the bridge) → runtime call via `call_indirect`

### Indirect Call (Closures)

Closures are `(func_ptr, captures...)`. To call:
1. Load `func_ptr` from the closure data
2. Pack captures + args into the calling convention buffer
3. `call_indirect(func_sig, func_ptr, &[packed_args])`

### Tail Call

`TailCall` uses `jump` instead of `call` to avoid stack growth. Only valid for self-recursive calls.

### Tests

- `let fact = (n) => if n == 0 { 1 } else { n * fact(n - 1) }` — recursion
- `let fib = (n) => if n <= 1 { n } else { fib(n-1) + fib(n-2) }` — mutual recursion
- `let adder = (n) => (x) => n + x; adder(5)(3)` — closures + currying
- `let apply = (f, x) => f(x); apply((x) => x + 1, 41)` — higher-order calls

---

## Phase 4: Heap Types (Days 4-6)

**Goal:** Arrays, records, tags, strings work. Requires runtime helpers.

### Approach: Runtime Helpers

Heap-allocated types (Array, Record, Tag, Closure, Str) cannot be represented as Cranelift SSA values. They must be managed by Rust runtime functions called from JIT code.

**Architecture:**
```
JIT code → calls Rust runtime helper → helper allocates/manipulates heap object → returns pointer
```

### Runtime Helper Functions

These are Rust `extern "C"` functions registered with Cranelift's `JITBuilder`:

| Helper | Signature | Description |
|---|---|---|
| `pipe_rt_array_alloc` | `(len: u64, init: *const u8) -> *const u8` | Allocate array of `len` elements, all set to `init` |
| `pipe_rt_array_get` | `(arr: *const u8, index: u64) -> *const u8` | Get element at index |
| `pipe_rt_array_set` | `(arr: *const u8, index: u64, val: *const u8) -> *const u8` | Set element at index, return new array |
| `pipe_rt_array_len` | `(arr: *const u8) -> u64` | Get array length |
| `pipe_rt_array_concat` | `(a: *const u8, b: *const u8) -> *const u8` | Concatenate two arrays |
| `pipe_rt_array_literal` | `(count: u64, ...) -> *const u8` | Construct array from variadic args |
| `pipe_rt_record_alloc` | `(field_count: u64, ...) -> *const u8` | Allocate record from field values |
| `pipe_rt_record_get` | `(rec: *const u8, index: u64) -> *const u8` | Get field by index |
| `pipe_rt_tag_construct` | `(tag: u32, payload_count: u64, ...) -> *const u8` | Construct tag value |
| `pipe_rt_tag_discriminant` | `(tag: *const u8) -> u32` | Get discriminant |
| `pipe_rt_tag_get` | `(tag: *const u8, index: u64) -> *const u8` | Get payload field |
| `pipe_rt_str_concat` | `(parts: *const *const u8, count: u64) -> *const u8` | Concatenate string parts |
| `pipe_rt_println` | `(val: *const u8) -> i32` | Print value to stdout |
| `pipe_rt_panic` | `(msg: *const u8) -> !` | Trap with message |

### Value Representation on Heap

All heap values are refcounted (`Arc`). The pointer passed to JIT code is `*const u8` pointing to the `Arc` data. For primitive-in-heap scenarios (e.g., `Array<i32>`), elements are stored inline.

**Tag layout:**
```
[ discriminant: u32 ] [ payload: Value ] [ payload: Value ] ...
```

**Closure layout:**
```
[ func_ptr: usize ] [ capture1: Value ] [ capture2: Value ] ...
```

### Tests

- `let arr = [1, 2, 3]; arr[1]` — array literal + index
- `let r = { name: "Alice", age: 30 }; r.name` — record literal + field access
- `match Some(42) { Some(v) => v, None => 0 }` — tag construct + discriminant + match
- `let msg = \`hello ${name}\`` — string concatenation
- `println("hello")` — IO

---

## Phase 5: Monomorphization (Day 5-6)

**Goal:** Polymorphic functions generate specialized versions for each concrete type combination at JIT compile time.

### Why Monomorphization

The typechecker already produces a `TypedProgram` with a `type_map: HashMap<Span, MonoType>` that maps every expression span to its fully-resolved concrete type. This means at JIT time, we know exactly what concrete types every function is called with.

### Approach

Instead of generating one Cranelift function per `IrFunction`, we generate **one Cranelift function per (function_name, concrete_type_signature) pair**.

**Example:**

```
let id = (x) => x
let a = id(42)      // id called with i32
let b = id("hello") // id called with str
```

The lowerer produces one `IrFunction` named `id` with param type `I32` (fallback for type vars). But the type map tells us:
- Span of `id(42)` → `Func { params: [I32], ret: I32 }` → needs `id_i32`
- Span of `id("hello")` → `Func { params: [Str], ret: Str }` → needs `id_str`

The JIT generates:
```
fn id_i32(x: i32) -> i32 { x }
fn id_str(x: str_ptr, x_len: u64) -> (str_ptr, u64) { x }
```

### Implementation Steps

1. **Collect call sites**: Walk the `IrModule`, for each `CallNamed { name, args }`, look up the caller's span in `type_map` to get the concrete argument types.

2. **Generate specialized functions**: For each unique `(name, [arg_types...])` combination, clone the `IrFunction`, replace parameter types with concrete types, and compile as a separate Cranelift function.

3. **Rewrite call sites**: Replace `CallNamed { name: "id", args }` with `CallNamed { name: "id_i32", args }` (the specialized name).

4. **Handle recursive functions**: For `let fact = (n) => if n == 0 { 1 } else { n * fact(n - 1) }`, `fact` calls itself with the same type. Generate one specialized version `fact_i32` that calls `fact_i32` recursively.

### Name Mangling

Specialized function names use a mangled format:

```
{id}_{type1}_{type2}_...
```

Examples:
- `id` with `(i32)` → `id_i32`
- `add` with `(i32, i32)` → `add_i32_i32`
- `compose` with `((i32) -> i32, (i32) -> i32)` → `compose_Func_i32_i32_Func_i32_i32`

For complex types, use a hash suffix to keep names short:
- `compose` with complex sig → `compose_a1b2c3`

### Type Map Integration

The `TypedProgram.type_map` already has the concrete types at every call site. The JIT reads this map during compilation:

```rust
fn monomorphize(module: &IrModule, type_map: &HashMap<Span, MonoType>) -> IrModule {
    let mut specialized = IrModule::new();
    let mut name_map: HashMap<(String, Vec<IrType>), String> = HashMap::new();
    
    for decl in &module.decls {
        if let IrDecl::Function(func) = decl {
            // For each CallNamed in this function's blocks:
            //   1. Look up the call site span in type_map
            //   2. Get concrete arg types
            //   3. Generate or reuse specialized version
            //   4. Rewrite the call target name
        }
    }
    specialized
}
```

### Edge Cases

| Case | Handling |
|---|---|
| Recursive calls with same type | One specialized version calls itself |
| Recursive calls with different types | Generate multiple specialized versions (rare in practice) |
| Polymorphic in unused position | Generate one version with the concrete type used |
| Top-level `let id = (x) => x` without calls | Generate zero versions (dead code elimination) |
| Closures capturing polymorphic values | Specialize based on captured value types |

### Tests

- `let id = (x) => x; id(42); id("hello")` — two specializations
- `let fact = (n) => if n == 0 { 1 } else { n * fact(n-1) }` — recursive self-call
- `let apply = (f, x) => f(x); apply((x) => x + 1, 41)` — higher-order polymorphism

---

## Phase 6: Builtin Registration (Day 6-7)

**Goal:** Prelude functions work through the JIT.

### Builtin Registry

Create a `BuiltinRegistry` that maps function names to Rust implementations:

```rust
struct BuiltinRegistry {
    functions: HashMap<String, Box<dyn BuiltinFunction>>,
}
```

### Registration Flow

1. At JIT compile time, collect all `CallNamed` targets
2. Check if the name is a user function in the module
3. If not, look up in `BuiltinRegistry`
4. Register the builtin as a Cranelift external function

### Prelude Builtins to Implement

| Function | Arity | Implementation |
|---|---|---|
| `println` | 1 | Print value to stdout via `pipe_rt_println` |
| `array_literal` | N | Call `pipe_rt_array_literal` |
| `map` | 2 | `arr.map(f)` — iterate, apply f, collect |
| `filter` | 2 | `arr.filter(f)` — iterate, keep where f returns true |
| `fold` | 3 | `arr.fold(init, f)` — accumulate |
| `flatMap` | 2 | `arr.flatMap(f)` — map then flatten |
| `concat` | 2 | `arr.concat(other)` — concatenate arrays |
| `len` | 1 | `arr.len()` — array length |
| `head` | 1 | `arr.head()` — first element or None |
| `tail` | 1 | `arr.tail()` — all but first or None |
| `drop` | 2 | `arr.drop(n)` — skip n elements |
| `take` | 2 | `arr.take(n)` — first n elements |
| `stdin_readLine` | 0 | Read line from stdin |

---

## Phase 7: Optimization & Polish (Days 7-10)

### Tail Call Optimization

When `TailCall` targets the same function, replace with `jump` to entry block with updated args. This prevents stack overflow for deep recursion (e.g., `quicksort`).

### Constant Folding (Optional)

Peephole optimizations in the lowerer:
- `const + const` → `const`
- `const * 0` → `0`
- `if true { a } else { b }` → `a`

### Error Reporting

When a `Panic` instruction is hit, the JIT should:
1. Call `pipe_rt_panic` with the message
2. The runtime prints the message and exits with non-zero code

---

## Language API Spec Contract

### Types

| Type | IR Representation | Runtime Value |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | `IrType::I8/I16/I32/I64` | `Value::I8/I16/I32/I64` |
| `u8`, `u16`, `u32`, `u64`, `usize` | `IrType::U8/U16/U32/U64/Usize` | `Value::U8/U16/U32/U64/Usize` |
| `f32`, `f64` | `IrType::F32/F64` | `Value::F32/F64` |
| `bool` | `IrType::Bool` | `Value::Bool` |
| `str` | `IrType::Str` | `Value::Str(SmolStr)` |
| `()` (unit) | `IrType::Unit` | `Value::Unit` |
| `Array<T>` | `IrType::Array(Box<IrType>)` | `Value::Array(Arc<[Value]>)` |
| `Record { f: T, ... }` | `IrType::Record(RecordType)` | `Value::Record(Arc<RecordData>)` |
| `Tag { name, payload }` | `IrType::Tag(TagType)` | `Value::Tag { tag: u32, payload: Arc<[Value]> }` |
| `(a) -> b` | `IrType::Func(FuncType)` | `Value::Closure(Arc<ClosureData>)` |
| `Closure` | `IrType::Closure(Box<FuncType>)` | `Value::Closure(Arc<ClosureData>)` |

### Expressions

| Expression | Type | Example |
|---|---|---|
| Integer literal | `i32` (default) | `42`, `42i64`, `255u8` |
| Float literal | `f64` (default) | `3.14`, `3.14f32` |
| Bool literal | `bool` | `true`, `false` |
| String literal | `str` | `"hello"` |
| Template string | `str` | `` `count: ${n}` `` |
| Identifier | varies | `x`, `add` |
| Lambda | `(a) -> b` | `(x) => x + 1` |
| Application | return type of func | `f(x)`, `add(1, 2)` |
| Binary op | varies | `a + b`, `a == b` |
| Unary op | varies | `-x`, `!b` |
| If/else | then/else type | `if c { a } else { b }` |
| Match | arm result type | `match x { P1 => e1, P2 => e2 }` |
| Block | result expr type | `{ let x = 1; x }` |
| Array literal | `Array<T>` | `[1, 2, 3]` |
| Index | element type | `arr[0]` |
| Record literal | `Record { ... }` | `{ name: "A", age: 30 }` |
| Field access | field type | `r.name` |
| Tuple | `Tag { name: "Tuple", ... }` | `(1, true)` |
| Constructor | parent type | `Some(42)`, `None`, `Ok(1)`, `Err("e")` |

### Declarations

| Declaration | Example |
|---|---|
| Value binding | `let x = 42` |
| Typed binding | `let x: i32 = 42` |
| Lambda binding | `let add = (a, b) => a + b` |
| Type alias | `type Person = { name: str, age: i32 }` |
| Use import | `use stdlib::io` |

### Patterns

| Pattern | Example |
|---|---|
| Binding | `x` |
| Wildcard | `_` |
| Literal | `42`, `"hello"`, `true` |
| Constructor | `Some(v)`, `None`, `Ok(v)`, `Err(e)` |
| Tuple | `(a, b)` |
| Record | `{ name, age }` |

### Operators

| Op | Types | Result |
|---|---|---|
| `+`, `-`, `*`, `/`, `%` | numeric × numeric | same numeric type |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | T × T | `bool` |
| `&&`, `\|\|` | `bool` × `bool` | `bool` |
| `-` (unary) | numeric | same numeric type |
| `!` (unary) | `bool` | `bool` |

### Builtins

| Function | Signature | Description |
|---|---|---|
| `println` | `(a) -> ()` | Print to stdout |
| `id` | `<a>(a) -> a` | Identity |
| `const` | `<a, b>(a) -> (b) -> a` | Constant |
| `flip` | `<a, b, c>((a, b) -> c) -> (b, a) -> c` | Flip args |
| `compose` | `<a, b, c>((b) -> c, (a) -> b) -> (a) -> c` | Compose |
| `pipe` | `<a, b, c>((a) -> b, (b) -> c) -> (a) -> c` | Pipe |
| `apply` | `<a, b>((a) -> b, a) -> b` | Apply |
| `Some` | `<a>(a) -> Option<a>` | Option constructor |
| `None` | `<a>Option<a>` | Option constructor |
| `Ok` | `<a, b>(a) -> Result<a, b>` | Result constructor |
| `Err` | `<a, b>(b) -> Result<a, b>` | Result constructor |
| `map` | `<a, b>(Array<a>, (a) -> b) -> Array<b>` | Map over array |
| `filter` | `<a>(Array<a>, (a) -> bool) -> Array<a>` | Filter array |
| `fold` | `<a, b>(Array<a>, b, (b, a) -> b) -> b` | Fold array |
| `flatMap` | `<a, b>(Array<a>, (a) -> Array<b>) -> Array<b>` | FlatMap |
| `concat` | `<a>(Array<a>, Array<a>) -> Array<a>` | Concatenate |
| `len` | `<a>(Array<a>) -> i32` | Array length |
| `head` | `<a>(Array<a>) -> Option<a>` | First element |
| `tail` | `<a>(Array<a>) -> Option<Array<a>>` | All but first |
| `drop` | `<a>(Array<a>, i32) -> Array<a>` | Skip n |
| `take` | `<a>(Array<a>, i32) -> Array<a>` | First n |

### Method Desugaring

Method calls are desugared at parse time:

| Source | Desugared |
|---|---|
| `arr.map(f)` | `map(arr, f)` |
| `arr.filter(f)` | `filter(arr, f)` |
| `arr.fold(init, f)` | `fold(arr, init, f)` |
| `arr.len()` | `len(arr)` |
| `arr.drop(n)` | `drop(arr, n)` |
| `arr.take(n)` | `take(arr, n)` |
| `arr.head()` | `head(arr)` |
| `arr.tail()` | `tail(arr)` |
| `opt.map(f)` | `map(opt, f)` (once Option methods are added) |

---

## File Structure

```
crates/
  runtime/
    src/
      jit.rs          -- Cranelift JIT compiler (Phase 1-4)
      bridge.rs       -- Builtin registry + runtime helpers (Phase 5)
      value.rs        -- Value type (already done)
      helpers.rs      -- NEW: Rust runtime helpers called from JIT code
    Cargo.toml        -- Add cranelift-frontend dependency
  ir/
    src/
      lib.rs          -- IR types (already done)
      lower.rs        -- AST → IR lowering (already done)
  typechecker/
    src/
      lib.rs          -- typecheck() entry (already done)
      infer.rs        -- HM inference (already done)
      env.rs          -- TypeEnv + prelude (already done)
```

---

## Verification Checklist

After each phase, verify:

```bash
# Phase 1
echo 'let x = 1 + 2' | pipe-lang compile --emit-ir /dev/stdin
echo 'let x = 1 + 2' | pipe-lang run /dev/stdin  # should return 3

# Phase 2
echo 'let abs = (x: i32) => if x > 0 { x } else { 0 - x }' | pipe-lang run /dev/stdin

# Phase 3
echo 'let fact = (n: i32) => if n == 0 { 1 } else { n * fact(n - 1) }' | pipe-lang run /dev/stdin

# Phase 4
echo 'let arr = [1, 2, 3]; arr[1]' | pipe-lang run /dev/stdin

# Phase 5
echo 'let main = () => println("hello")' | pipe-lang run /dev/stdin
```
