# IR Design — Track B Contract

**Owner:** assistant (Track B)
**Status:** frozen as of Track B Day 1
**Source:** `crates/ir/src/lib.rs`
**Consumers:** Track A (lowerer) and Track B (Cranelift codegen)

This document is the canonical description of the IR data types that the
frontend emits and the backend consumes. Both tracks should refer to it
rather than reverse-engineering `lib.rs`.

## Frozen until Day 10

The IR shape is **frozen** from this commit through the end of Phase C.
Both tracks may add fields, but neither may remove or rename them without
agreement.

---

## Top-level layout

```rust
pub struct IrModule {
    pub imports: Vec<SmolStr>,
    pub decls: Vec<IrDecl>,
}

pub enum IrDecl {
    Function(IrFunction),
    TypeAlias { name: SmolStr, ty: IrType },
}
```

A module corresponds to one source file (`hello.pp` → one `IrModule`).
Imports are preserved verbatim so the codegen can resolve `use stdlib::io`
style paths. The lowerer emits one `IrDecl::Function` per top-level `let`
binding; type aliases become `IrDecl::TypeAlias`.

---

## Function

```rust
pub struct IrFunction {
    pub name: SmolStr,
    pub params: Vec<(ValueId, SmolStr, IrType)>,
    pub return_type: IrType,
    pub blocks: Vec<BasicBlock>,
    pub next_value_id: u32,
    pub next_block_id: u32,
}
```

- `name` is the source-level identifier (e.g. `"main"`, `"quicksort"`).
- `params` is a tuple of `(ssa_id, source_name, type)`. The codegen uses
  `ssa_id` to refer to the parameter in subsequent instructions; the
  `source_name` is for `--emit-ir` output only.
- `return_type` is needed because Cranelift needs to know the return
  type up front, not just from terminator analysis.
- `blocks[0]` is the entry block.
- The `next_*_id` counters are mutable state for the lowerer.

**Key invariant:** every block's terminator is one of `Return | Jump |
Branch | Switch | TailCall | Unreachable`. The codegen errors out
otherwise.

---

## Basic blocks

```rust
pub struct BasicBlock {
    pub id: BlockId,
    pub params: Vec<(ValueId, IrType)>,
    pub instructions: Vec<(Option<ValueId>, Instruction)>,
    pub terminator: Terminator,
}
```

- `params` are SSA block arguments (the equivalent of LLVM phi nodes
  or Cranelift block parameters). The lowerer inserts these for both
  forward and backward branches.
- `instructions` is a flat list. Each entry is `(defined_value, op)`.
  The `Option` is `None` for value-less ops (e.g. `Println`, `Panic`).
- `terminator` is the last item; there is always exactly one.

---

## Values

```rust
pub struct ValueId(pub u32);
pub struct BlockId(pub u32);
```

Both are scoped to a single `IrFunction`. IDs are dense (0, 1, 2, ...)
and allocated by the lowerer. `Display` produces `v7` for `ValueId(7)`
and `bb3` for `BlockId(3)` (for `--emit-ir` output).

**Key invariant:** every `ValueId` in the IR has a known `IrType`. The
type is stored in the `BasicBlock::params` entry, in the function's
`params`, or in the `defined_value` slot of the instruction that
produced it (the lowerer maintains a side table). The Cranelift codegen
recovers the type by walking the side table — the IR is "typed" but
not "type-annotated" on every reference.

---

## Types

```rust
pub enum IrType {
    I8 | I16 | I32 | I64 |
    U8 | U16 | U32 | U64 | Usize |
    F32 | F64 |
    Bool | Str | Unit |
    Array(Box<IrType>),
    Record(RecordType),
    Func(FuncType),
    Closure(Box<FuncType>),
    Tag(TagType),
    Effect(Box<IrType>),  // erased at codegen
}

pub struct RecordType { name: SmolStr, fields: Vec<(SmolStr, IrType)> }
pub struct FuncType { params: Vec<IrType>, ret: Box<IrType> }
pub struct TagType { name: SmolStr, variants: Vec<TagVariant> }
pub struct TagVariant { name: SmolStr, discriminant: u32, payload: Vec<IrType> }
```

### Tag discriminants (frozen)

| Type | Variant | Discriminant |
|------|---------|--------------|
| `Option<T>` | `None` | 0 |
| `Option<T>` | `Some` | 1 |
| `Result<T, E>` | `Err` | 0 |
| `Result<T, E>` | `Ok` | 1 |
| User-defined | in declaration order | 0, 1, 2, ... |

**The lowerer is responsible for assigning discriminants consistently
across the module.** Two `Result::Ok` constructors in different
functions must use the same discriminant (1).

### Memory layout (frozen, decided in `runtime::layout`)

| Type | Runtime representation |
|------|------------------------|
| All integers | native width (Cranelift `i32`, `i64`, ...) |
| `Bool` | Cranelift `b1` (no box) |
| `Str` | fat pointer: `(ptr, len)` (`*const u8`, `usize`) |
| `Array<T>` | fat pointer: `(ptr, len, cap)` (Cranelift struct) |
| `Record` | `Arc<RecordData>` — a single pointer |
| `Tag` | `(discriminant: u32, payload: Arc<[Value]>)` — packed |
| `Closure` | `(func_ptr: usize, captures: Arc<[Value]>)` |
| `Func` | never appears as a value; only as a callee |
| `Effect<T>` | erased at codegen; do-blocks are sequentialized |

**The lowerer is responsible for choosing the right representation.**
In particular, `Array<T>` is fat-pointer-shaped (3 words), while
`Record` and `Tag` are single-pointer-shaped (1 word each). The
codegen uses these to pick Cranelift lane types.

---

## Instructions (the meat)

### Constants
`ConstI8` ... `ConstUsize`, `ConstF32`, `ConstF64`, `ConstBool`,
`ConstStr(SmolStr)`, `ConstUnit`.

### Arithmetic
`Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`. Numeric-typed only. The
codegen errors on `I32 + F64` (the lowerer must insert an explicit
conversion if needed; in 0.1 we don't have implicit conversions).

### Comparison
`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`. Returns `Bool` (Cranelift `b1`).
**String comparison is not in 0.1** — strings are compared by
`runtime::str::eq` via a `CallNamed("Str.eq", ...)`.

### Logical
`And`, `Or` (short-circuit via `Branch`), `Not`.

### Arrays
- `ArrayAlloc { len: ValueId, init: ValueId }` — heap-allocates an
  array of `len` copies of `init`. Codegen uses `runtime::array::alloc`.
- `ArrayGet { array, index }` — panics on out-of-bounds.
- `ArraySet { array, index, value }` — pure (returns Unit), allocates
  a new array (immutability).
- `ArrayLen(array)` — returns `Usize`.
- `ArrayConcat(a, b)` — returns a new array `[...a, ...b]`.

### Records
- `RecordAlloc { type_name, fields }` — fields in declaration order.
- `RecordGet { record, field, field_index }` — `field_index` is the
  position in the record's `fields` Vec; codegen uses it for layout.
- `RecordSet { record, field, field_index, value }` — pure, returns
  a new record.

### Tags
- `TagConstruct { type_name, variant, discriminant, payload }` —
  the variant and discriminant are looked up from the type's table
  in the same module.
- `TagDiscriminant(value)` — returns the `u32` discriminant.
- `TagGet { value, index }` — extracts the `index`-th payload.

### Closures
- `MakeClosure { func_name, captures }` — wraps a function reference
  plus a list of captured values into a `Closure` value.
- `CallIndirect { callee, args }` — invokes a `Closure` value.

### Named calls
- `CallNamed { name, args }` — calls a builtin (e.g. `"IO.println"`)
  or a top-level function in the same module. The codegen dispatches
  by looking up the name in the runtime's `BuiltinFunction` registry;
  if not found, it tries the module's `IrDecl::Function` list.

### Effects
- `EffectBind { effect, continuation }` — `effect` is an `Effect<T>`
  value (i.e. a builtin call that has not been run yet); the codegen
  runs the effect, then runs the continuation with the result.
  The lowerer turns a `do` block into a chain of these.
- `EffectValue { builtin, args }` — builds an effect value without
  running it. Used to lift an effectful call into a closure.
- `EffectReturn(value)` — pure: do nothing, return Unit. The end of
  an expression-only do block.

### Strings
- `StrConcat { parts }` — concatenates a sequence of values, each of
  which is either a `Str` or has a `Display` impl. Codegen calls
  `runtime::str::concat`.
- `Println(value)` — shorthand for
  `Effect::bind(EffectValue("IO.println", [value]), EffectReturn)`.

### Panic
- `Panic { msg }` — emits a Cranelift trap with the given message.
  Used after bounds checks and non-exhaustive match fallthroughs.

---

## Terminators

| Variant | Meaning |
|---------|---------|
| `Return(v)` | exit the function with value `v` |
| `Jump { target, args }` | unconditional branch |
| `Branch { condition, then_block, then_args, else_block, else_args }` | if/else |
| `Switch { discriminant, arms, default }` | pattern match on a tag |
| `TailCall { callee, args }` | tail call (codegen emits a jump to avoid stack growth) |
| `Unreachable` | after a `Panic`; satisfies Cranelift's terminator requirement |

**The codegen must handle every variant.** `Unreachable` lowers to
`cranelift::codegen::ir::TrapCode::UnreachableCodeReachable`.

---

## How the lowerer should think about blocks

1. The entry block is `block 0`. It receives the function's parameters
   as block parameters (yes, even for non-recursive functions — this
   simplifies codegen).
2. An `if` produces **3 blocks**: the entry, the then-arm, and the
   else-arm. The arms `Jump` back to a join block.
3. A `match` on a tag produces **N+1 blocks**: the entry, N arm blocks,
   and a join block. Use `Switch`, not a chain of `Branch`.
4. Recursive functions should use `TailCall` when the recursive call
   is in tail position (the lowerer must detect this). Otherwise,
   `CallNamed` or `CallIndirect` is fine.

---

## How the codegen should think about the IR

1. Walk the module's `IrDecl`s in order.
2. For each `IrDecl::TypeAlias`, register the type in the runtime's
   type table (so tag discriminants are stable across modules).
3. For each `IrDecl::Function`, allocate a Cranelift function with
   the function's `name`, signature, and `params`.
4. For each block, create a Cranelift `Block` and emit its
   instructions in order. Each `Instruction::X(a, b)` becomes a
   Cranelift instruction that defines a value of `IrType::...`; the
   side table mapping `ValueId -> Cranelift Value` is maintained
   during this walk.
5. For each `Terminator`, emit the corresponding Cranelift
   terminator (`return`, `jump`, `brz`, `brnz`, `trap`).
6. Call `cranelift_module::Module::finalize_definitions()` once
   at the end of the module.
7. Call `cranelift_jit::JITModule::finalize()` to make the
   function pointer callable.

---

## Open questions (for after Day 10)

1. **Multi-file modules.** The IR is per-file today. A `use
   stdlib::io` import is just a `SmolStr`; we need a module linker
   before this can compile anything real. Out of scope for 0.1;
   the 14 example programs are all single-file.
2. **Closures with mutable state.** We don't have `Ref<T>` in 0.1,
   so closures are always over `+T` (immutable) values. `MakeClosure`
   with `Arc<Ref<T>>` is a future addition.
3. **Variadic generics.** 0.1 does not have them. `Array<T>` has
   one type parameter; `Result<T, E>` has two; no higher-kinded
   types.
4. **Inline `do`.** A `do` block in expression position (not at the
   top level) is allowed but is sugar for an immediately-invoked
   effect. The lowerer should expand it as a private function plus
   a call.

---

## Versioning

This is **IR v0.1.0**. Any breaking change increments to v0.2.0 and
requires both tracks to agree on a 1-day freeze window.
