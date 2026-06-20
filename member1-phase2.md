# Member 1 — Phase 2: JIT Backend Completion

**Crate Ownership:** `crates/runtime/src/jit.rs` (+ new `rt_helpers.rs`)  
**Prerequisite:** dijith-phase2 (typechecker fixes) must be complete — Dijith unblocks the pipeline; you make it execute  
**Timeline:** 3 days (partially parallel with Member 2, Member 3)  
**Goal:** Every typechecked example program JIT-compiles and executes correctly. All 11 missing instructions implemented. Builtins callable from JIT code.

---

## Current State (from audit)

The JIT (`crates/runtime/src/jit.rs` ~2339 lines) handles **32 of 43** IR instructions:

| Category | Total | Handled | Missing |
|---|---|---|---|
| Constants | 14 | 14 (all) | — |
| Arithmetic | 6 | 6 (all) | — |
| Comparisons | 6 | 6 (all) | — |
| Logical | 3 | 3 (all) | — |
| **Arrays** | **5** | **0** | ArrayAlloc, ArrayGet, ArraySet, ArrayLen, ArrayConcat |
| **Records** | **3** | **0** | RecordAlloc, RecordGet, RecordSet |
| **Tags** | **3** | **0** | TagConstruct, TagDiscriminant, TagGet |
| **Closures** | **2** | **0** | MakeClosure, CallIndirect |
| Calls | 1 | CallNamed (local only) | **CallNamed → global_registry()** (builtins) |
| Strings | 2 | StrConcat, Println | — |
| Misc | 1 | — | **Panic** |
| **Total** | **43** | **32** | **11** |

**Missing terminators:** `TailCall`  
**Other terms:** Return, Jump, Branch, Switch, Unreachable — all handled.

### Critical bottleneck (NOW RESOLVED)

`CallNamed` compiles local function calls but had **no fallback to `global_registry()`**. Every call to a builtin (`println`, `to_str`, `map`, etc.) failed at JIT compile time with `JitError: unknown function`.

**Resolution:** The builtin bridge (`pipe_rt_call_builtin` + `compile_call_named` fallback) was implemented by Dijith as a stopgap. The code lives in `crates/runtime/src/jit.rs` and `crates/runtime/src/rt_helpers.rs`. Member 1 should verify, test, and own it going forward.

### Gap G4: Missing `JitError::RuntimeError` variant

**File:** `crates/runtime/src/jit.rs`, line 51

member1-phase2.md references `JitError::RuntimeError("main panicked".into())` but this variant does not exist.

**Fix:** Add to `JitError`:
```rust
#[error("runtime error: {msg}")]
RuntimeError { msg: String },
```

### 14 example programs

After Dijith's typecheck fixes, all 14 will typecheck. Your job: make them JIT-run.

| Program | Typecheck | JIT needs | Priority |
|---|---|---|---|
| `hello.pp` | ✓ (after Dijith) | CallNamed→println, Panic | **1** |
| `factorial.pp` | ✓ | CallNamed→to_str | **1** |
| `fibonacci.pp` | ✓ | CallNamed→to_str | **1** |
| `sorting.pp` | ✓ | ArrayAlloc/Get/Set/Len, CallNamed→map/filter/etc | **2** |
| `patterns.pp` | ✓ | TagConstruct/Discriminant/Get, Switch | **2** |
| `option-result.pp` | ✓ | TagConstruct/Discriminant/Get, TailCall | **2** |
| `state-machine.pp` | ✓ | TagConstruct, RecordAlloc/Get/Set | **2** |
| `closures.pp` | ✓ | MakeClosure, CallIndirect | **2** |
| `higher-order.pp` | ✓ | MakeClosure, CallIndirect | **2** |
| `records.pp` | ✓ | RecordAlloc/Get/Set | **2** |
| `io-effects.pp` | ✓ | CallNamed→println/readFile | **1** |
| `ascii-art.pp` | ✓ | All array + string + builtins | **2** |
| `generics.pp` | ✓ | All tag + array ops | **2** |
| `game-of-life.pp` | ✓ | Everything | **3** |

---

## Architecture: Runtime Helpers (RT Helpers)

All heap operations (arrays, records, tags, closures) and the builtin bridge share the same C ABI:

```
extern "C" fn(args: *const u8, ret: *mut u8) -> i32
```

- `args` points to a packed buffer of input values
- `ret` points to a 16-byte output buffer
- Returns 0 on success, 1 on abort (panics)

**Wire format for values in the buffer:**

| Type | Size | Encoding |
|---|---|---|
| i8/i16/i32/i64 | 8 bytes | Sign-extended to i64 |
| u8/u16/u32/u64/usize | 8 bytes | Zero-extended to u64 |
| f32/f64 | 8 bytes | Bit-cast to u64 |
| bool | 8 bytes | 0 or 1 |
| Heap ptr (str/array/record/tag/closure) | 8 bytes | `*const u8` pointer |
| Type tag | 4 bytes | Enum value (0=I8…11=Str, 12=Unit, 13=Array, 14=Record, 15=Tag, 16=Closure) |

**Internal heap object layout** (the data behind `*const u8`):

```
[str]:     [len: u32][utf8 bytes...]
[array]:   [len: u32][element data: [value: u64, type_tag: u32]...]
[record]:  [field_count: u32][field data: [value: u64, type_tag: u32]...]
[tag]:     [discriminant: u32][payload_count: u32][payload data: [value: u64, type_tag: u32]...]
[closure]: [func_name_len: u32][func_name bytes...][capture_count: u32][capture data: [value: u64, type_tag: u32]...]
```

---

## Day 1 — Morning (Hours 0–4): Builtin Bridge (CRITICAL BLOCKER)

### Why this must come first

Without the builtin bridge, **zero** example programs can run. Not even `hello.pp`. Every other JIT feature is useless until this is done.

### Task 1.1: Create `crates/runtime/src/rt_helpers.rs`

Add the bridge function:

```rust
// rt_helpers.rs

use crate::bridge::{global_registry, Value};

/// Call a builtin function by name.
///
/// # Safety
///
/// `args` must point to a valid packed buffer. `ret` must point to 16+ bytes of writable memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_call_builtin(args: *const u8, ret: *mut u8) -> i32 {
    // args layout:
    //   bytes 0-3:   name_len (u32)
    //   bytes 4..4+name_len: name bytes (not null-terminated)
    //   bytes 4+name_len..4+name_len+3: arg_count (u32)
    //   bytes 4+name_len+4..: for each arg: [value: u64 (8 bytes), type_tag: u32 (4 bytes)]
    let name_len = unsafe { std::ptr::read_unaligned(args.cast::<u32>()) };
    let name_bytes = unsafe { std::slice::from_raw_parts(args.add(4), name_len as usize) };
    let name = unsafe { std::str::from_utf8_unchecked(name_bytes) };

    let arg_count_offset = 4 + name_len as usize;
    let arg_count = unsafe { std::ptr::read_unaligned(args.add(arg_count_offset).cast::<u32>()) };

    // Deserialize args
    let mut jit_args = Vec::with_capacity(arg_count as usize);
    let mut offset = arg_count_offset + 4;
    for _ in 0..arg_count {
        let raw_val = unsafe { std::ptr::read_unaligned(args.add(offset).cast::<u64>()) };
        let type_tag = unsafe { std::ptr::read_unaligned(args.add(offset + 8).cast::<u32>()) };
        jit_args.push(decode_value(raw_val, type_tag));
        offset += 16; // 8 bytes value + 4 bytes tag + 4 bytes padding
    }

    // Look up and call
    let registry = crate::bridge::global_registry();
    match registry.find(name) {
        Some(builtin) => {
            match builtin.execute(&jit_args) {
                Ok(result) => {
                    encode_value(&result, ret);
                    0
                }
                Err(msg) => {
                    eprintln!("builtin error: {msg}");
                    1
                }
            }
        }
        None => {
            eprintln!("unknown builtin: {name}");
            1
        }
    }
}

fn decode_value(raw: u64, tag: u32) -> Value {
    use crate::Value;
    match tag {
        0 => Value::I8(raw as i8 as i32 as i64 as i8),
        1 => Value::I16(raw as i16 as i32 as i64 as i16),
        2 => Value::I32(raw as i32),
        3 => Value::I64(raw as i64),
        4 => Value::U8(raw as u8),
        5 => Value::U16(raw as u16),
        6 => Value::U32(raw as u32),
        7 => Value::U64(raw),
        8 => Value::Usize(raw as usize),
        9 => Value::F32(f32::from_bits(raw as u32)),
        10 => Value::F64(f64::from_bits(raw)),
        11 => Value::Bool(raw != 0),
        12 => Value::Str(/* reconstruct from ptr */ todo!()),
        13 => Value::Unit,
        _ => Value::Unit, // heap types decoded as Unit for now
    }
}

fn encode_value(val: &Value, ret: *mut u8) {
    // Write [value: u64, type_tag: u32] to ret
    let (raw, tag): (u64, u32) = match val {
        Value::I8(v) => (*v as u64, 0),
        Value::I16(v) => (*v as u64, 1),
        Value::I32(v) => (*v as u64, 2),
        Value::I64(v) => (*v as u64, 3),
        Value::U8(v) => (*v as u64, 4),
        Value::U16(v) => (*v as u64, 5),
        Value::U32(v) => (*v as u64, 6),
        Value::U64(v) => (*v as u64, 7),
        Value::Usize(v) => (*v as u64, 8),
        Value::F32(v) => (v.to_bits() as u64, 9),
        Value::F64(v) => (v.to_bits(), 10),
        Value::Bool(v) => (*v as u64, 11),
        Value::Unit => (0, 13),
        val => {
            eprintln!("cannot encode {val:?} for JIT return");
            (0, 13)
        }
    };
    unsafe {
        std::ptr::write_unaligned(ret.cast::<u64>(), raw);
        std::ptr::write_unaligned(ret.add(8).cast::<u32>(), tag);
    }
}
```

**Note:** The `decode_value` for `Value::Str` and heap types requires a pointer-based reconstruction from the heap layout. This gets fleshed out in Day 2 when the heap RT helpers are built. For Day 1, only primitive args/results are needed (which covers `println(str)`, `to_str(i32)`, arithmetic builtins, etc.).

**Register the module in `crates/runtime/src/lib.rs`:**
```rust
pub mod rt_helpers;
```

### Task 1.2: Wire `pipe_rt_call_builtin` into JIT

In `crates/runtime/src/jit.rs`, add:

1. **Declare data object for the helper pointer** (at module level, near other pointer declarations):

```rust
let call_builtin_ptr = rt_helpers::pipe_rt_call_builtin as *const ();
let call_builtin_ptr_data_id = module.declare_data(
    "__pipe_rt_call_builtin_ptr",
    wasmtime_environ::__core::cranelift_codegen::ir::ExternalName::user(0, 0),
)?;
```

2. **Store in a shared context** that `compile_instruction` can access. Add to `BlockContext`:

```rust
struct BlockContext<'a> {
    // ... existing fields ...
    call_builtin_ptr_data_id: DataId,
    call_builtin_sig: SigRef,
}
```

3. **The `call_builtin_sig` signature**: `(i64, i64) -> i64` (takes `args_ptr: i64`, `ret_ptr: i64`, returns exit code `i64`). Actually, the ABI is `(args: *const u8, ret: *mut u8) -> i32`. In Cranelift: two `i64` args (pointers), one `i32` return.

4. **Modify `compile_call_named`** (or `compile_instruction`'s `CallNamed` arm):

```rust
Instruction::CallNamed { name, args } => {
    // 1. Try local function resolution (existing logic)
    if let Some(func_ref) = ctx.callee_funcs.get(name) {
        return compile_local_call(builder, ctx, *func_ref, args);
    }

    // 2. Fallback: builtin call via pipe_rt_call_builtin
    //    Pack args into a stack buffer, call the helper, unpack result
    let result = compile_builtin_call(builder, ctx, name, args)?;
    Ok(result)
}
```

### Task 1.3: Implement `compile_builtin_call`

```rust
fn compile_builtin_call(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    name: &str,
    args: &[ValueId],
) -> Result<Value, JitError> {
    // 1. Build the args buffer on the stack
    //    Layout: [name_len: u32][name_bytes: u8 * name_len][arg_count: u32][args_data...]
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len() as u32;

    // Stack slot for the buffer: name_len(4) + name(name_len) + arg_count(4) + args*16
    let buf_size = 4 + name_len + 4 + args.len() * 16;
    let buf = builder.create_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        buf_size as u32,
    ));
    let buf_ptr = builder.stack_slot_addr(buf, 0);

    // Write name_len
    builder.ins().store(MemFlags::new(), ...);
    // Write name bytes
    // Write arg_count
    // For each arg: evaluate, store value + type_tag

    // 2. Create return buffer (16 bytes)
    let ret = builder.create_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        16,
    ));
    let ret_ptr = builder.stack_slot_addr(ret, 0);

    // 3. Call pipe_rt_call_builtin(buf_ptr, ret_ptr)
    let call = builder.ins().call(ctx.call_builtin_sig, &[buf_ptr, ret_ptr]);

    // 4. Read result from ret buffer
    let result_val = builder.ins().load(..., ret_ptr, 0);
    // Use type info to determine how to interpret

    Ok(result_val)
}
```

**FULL IMPLEMENTATION DETAIL REQUIRED:** This is the most important function in Phase 2. It must:
- Store the function name as a length-prefixed string
- Store each arg's raw value + type tag (see `ir_type_tag()` mapping)
- Call the C ABI function
- Read the result and convert to a Cranelift `Value`

### Task 1.4: Tests for builtin bridge

```rust
// In jit.rs tests:

#[test]
fn jit_calls_builtin_to_str() {
    // Build: ConstI32(42) → CallNamed("to_str", [ConstI32(42)])
    // Expected: ConstStr("42")
}

#[test]
fn jit_calls_builtin_println() {
    // Build: ConstStr("hello") → CallNamed("println", [ConstStr("hello")])
    // Expected: Unit, stdout capture shows "hello\n"
}

#[test]
fn jit_calls_arithmetic_as_builtin() {
    // Build: ConstI32(10), ConstI32(3) → CallNamed("i32_add", [10, 3])
    // Expected: I32(13)
}
```

---

## Day 1 — Mid (Hours 4–8): 5 Array Instructions

### Task 1.5: RT helpers for arrays

Add to `rt_helpers.rs`:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_alloc(args: *const u8, ret: *mut u8) -> i32 {
    // args: [len: i64 (8 bytes), init_val: u64 (8 bytes), init_tag: u32 (4 bytes)]
    // ret: array_ptr: u64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_get(args: *const u8, ret: *mut u8) -> i32 {
    // args: [array_ptr: u64 (8 bytes), index: i64 (8 bytes)]
    // ret: [value: u64 (8 bytes), type_tag: u32 (4 bytes)]
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_set(args: *const u8, ret: *mut u8) -> i32 {
    // args: [array_ptr: u64, index: i64, value: u64, value_tag: u32]
    // ret: new_array_ptr: u64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_len(args: *const u8, ret: *mut u8) -> i32 {
    // args: [array_ptr: u64]
    // ret: len: i64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_concat(args: *const u8, ret: *mut u8) -> i32 {
    // args: [array_a_ptr: u64, array_b_ptr: u64]
    // ret: new_array_ptr: u64
}
```

Each helper is ~20–40 lines of Rust. Total ~150 lines for all 5.

**Implementation approach** (for `pipe_rt_array_alloc` as example):

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_array_alloc(args: *const u8, ret: *mut u8) -> i32 {
    let len = unsafe { std::ptr::read_unaligned(args.cast::<i64>()) };
    let init_val = unsafe { std::ptr::read_unaligned(args.add(8).cast::<u64>()) };
    let init_tag = unsafe { std::ptr::read_unaligned(args.add(16).cast::<u32>()) };

    // Build as a Vec<Value>
    let init = decode_value(init_val, init_tag);
    let mut vec = Vec::with_capacity(len as usize);
    for _ in 0..len {
        vec.push(init.clone());
    }

    // Store as a heap object: [len: u32][value: u64, tag: u32]...
    // ... (layout details)

    0
}
```

### Task 1.6: JIT instruction arms for arrays

Add these arms in `compile_instruction`:

```rust
Instruction::ArrayAlloc { len, init } => {
    // 1. Load len value
    // 2. Load init value
    // 3. Emit ptr to pipe_rt_array_alloc with args [len, init_val, init_tag]
    // 4. Return array pointer
}
Instruction::ArrayGet { array, index } => { /* pipe_rt_array_get */ }
Instruction::ArraySet { array, index, value } => { /* pipe_rt_array_set */ }
Instruction::ArrayLen( array ) => { /* pipe_rt_array_len */ }
Instruction::ArrayConcat( a, b ) => { /* pipe_rt_array_concat */ }
```

### Task 1.7: Tests for array instructions

```rust
#[test] fn jit_array_alloc_empty()
#[test] fn jit_array_alloc_with_init()
#[test] fn jit_array_get_valid()
#[test] fn jit_array_get_oob()
#[test] fn jit_array_set()
#[test] fn jit_array_len()
#[test] fn jit_array_concat()
#[test] fn jit_array_concat_empty()
```

---

## Day 1 — Late (Hours 8–12): 3 Record + 3 Tag + Panic + TailCall

### Task 1.8: RT helpers for records and tags

```rust
// Records
pipe_rt_record_alloc(args, ret)
pipe_rt_record_get(args, ret)
pipe_rt_record_set(args, ret)

// Tags
pipe_rt_tag_construct(args, ret)
pipe_rt_tag_discriminant(args, ret)
pipe_rt_tag_get(args, ret)
```

### Task 1.9: JIT arms for records and tags

```rust
Instruction::RecordAlloc { fields } => /* pipe_rt_record_alloc */
Instruction::RecordGet { record, field } => /* pipe_rt_record_get */
Instruction::RecordSet { record, field, value } => /* pipe_rt_record_set */
Instruction::TagConstruct { tag_name, payload } => /* pipe_rt_tag_construct */
Instruction::TagDiscriminant { tag } => /* pipe_rt_tag_discriminant */
Instruction::TagGet { tag, index } => /* pipe_rt_tag_get */
```

### Task 1.10: Panic instruction + TailCall terminator

**Panic:**
```rust
Instruction::Panic { msg } => {
    // Load msg string constant
    // Call pipe_rt_panic (which calls eprintln + abort)
    // After the call: emit UNREACHABLE_TRAP
    // Return None (instruction produces no value, terminator follows)
}
```

**RT helper:**
```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_panic(args: *const u8, _ret: *mut u8) -> i32 {
    let msg_len = std::ptr::read_unaligned(args.cast::<u32>());
    let msg_bytes = std::slice::from_raw_parts(args.add(4), msg_len as usize);
    let msg = std::str::from_utf8_unchecked(msg_bytes);
    eprintln!("panic: {msg}");
    std::process::abort();
}
```

**TailCall:**
```rust
Terminator::TailCall { callee, args } => {
    // 1. Evaluate args
    // 2. Emit a direct call to the callee
    // 3. Store callee's return into our own return slot
    // 4. Emit Return (the callee's return becomes ours)
}
```

Implementation detail: In Cranelift, tail calls require the same signature. Since all pipe-lang functions share the same C ABI (`extern "C" fn(args: *const u8, ret: *mut u8) -> i32`), a tail call is just:
1. Write args into the buffer
2. Write callee pointer
3. `br` to the prologue block (or just use a normal call + return for now — true TCO is an optimization)

For v0.1, implement as `call + return` (not true tail call optimization). This is functionally correct even if it grows the stack.

### Task 1.11: Tests for records, tags, panic

```rust
#[test] fn jit_record_alloc_empty()
#[test] fn jit_record_alloc_with_fields()
#[test] fn jit_record_get()
#[test] fn jit_record_set()
#[test] fn jit_tag_construct_no_payload()
#[test] fn jit_tag_construct_with_payload()
#[test] fn jit_tag_discriminant()
#[test] fn jit_tag_get_payload()
#[test] fn jit_panic_aborts()
```

---

## Day 2 — Morning (Hours 0–4): Closures + Effect Return

### Task 1.12: RT helpers for closures

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_make_closure(args: *const u8, ret: *mut u8) -> i32 {
    // args: [func_name_len: u32][func_name...][capture_count: u32][captures...]
    // ret: closure_ptr: u64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pipe_rt_call_indirect(args: *const u8, ret: *mut u8) -> i32 {
    // args: [closure_ptr: u64, arg_count: u32, args...]
    // ret: [value: u64, type_tag: u32]
}
```

Key detail for `pipe_rt_call_indirect`:
1. Read closure pointer
2. Extract function name from closure header
3. Look up the function pointer in a closure table (or use the builtin registry again)
4. Pack args and call the resolved function
5. Return result

### Task 1.13: JIT arms for closures

```rust
Instruction::MakeClosure { func_name, captures } => {
    // 1. Build closure descriptor (func_name + captures)
    // 2. Call pipe_rt_make_closure
    // 3. Return closure pointer
}
Instruction::CallIndirect { closure, args } => {
    // 1. Load closure pointer
    // 2. Pack args + closure into buffer
    // 3. Call pipe_rt_call_indirect
    // 4. Return result value
}
```

### Task 1.14: Effect return support

Modify `call_main()` to handle `Value::Effect`:

```rust
pub fn call_main(&self) -> Result<i32, JitError> {
    let mut ret_buf = [0u8; 16];
    let code = unsafe {
        (self.main_ptr)(std::ptr::null(), ret_buf.as_mut_ptr())
    };
    if code != 0 {
        return Err(JitError::RuntimeError("main panicked".into()));
    }
    let raw_val = u64::from_le_bytes(ret_buf[0..8].try_into().unwrap());
    let tag = u32::from_le_bytes(ret_buf[8..12].try_into().unwrap());

    if tag == 13 {
        // Unit — success
        Ok(0)
    } else if tag <= 11 {
        // Primitive — decode as i32 for exit code
        Ok(raw_val as i32)
    } else {
        // Heap type — for now just return 0
        // TODO: handle Effect execution in Phase 3
        Ok(0)
    }
}
```

For now, programs like `println("hello")` return `()`. The `println` side effect executes immediately through the builtin bridge (`pipe_rt_call_builtin` calls `println!()` directly via the `IoPrintln` builtin). This matches the "immediate IO" design decision from `plan-main.md`.

### Task 1.15: Tests for closures

```rust
#[test] fn jit_make_closure_no_captures()
#[test] fn jit_make_closure_with_captures()
#[test] fn jit_call_indirect_simple()
#[test] fn jit_call_indirect_with_captures()
```

---

## Day 2 — Mid (Hours 4–8): 60+ Tests

### Task 1.16: Instruction-level tests

| Group | Count | Test names |
|---|---|---|
| Arrays | 10 | `jit_array_alloc_empty`, `jit_array_alloc_with_init`, `jit_array_get_valid_index`, `jit_array_get_oob`, `jit_array_set`, `jit_array_len_empty`, `jit_array_len_nonempty`, `jit_array_concat_two`, `jit_array_concat_empty`, `jit_array_concat_with_empty` |
| Records | 6 | `jit_record_alloc_empty`, `jit_record_alloc_fields`, `jit_record_get`, `jit_record_set`, `jit_record_get_missing`, `jit_record_alloc_multi_type` |
| Tags | 6 | `jit_tag_construct`, `jit_tag_construct_payload`, `jit_tag_discriminant`, `jit_tag_get_payload`, `jit_tag_match_switch`, `jit_tag_nested` |
| Closures | 4 | `jit_closure_no_captures`, `jit_closure_captures`, `jit_call_indirect`, `jit_call_indirect_captures` |
| Panic | 2 | `jit_panic_basic`, `jit_panic_message` |
| Builtins | 4 | `jit_builtin_to_str`, `jit_builtin_println`, `jit_builtin_map`, `jit_builtin_to_i64` |

### Task 1.17: E2E integration tests (test by compiling + running example programs)

Create `crates/runtime/tests/example_programs.rs`:

```rust
use std::process::Command;

#[test]
fn example_hello_world() {
    let output = Command::new("cargo")
        .args(["run", "--", "run", "example-programs/hello.pp"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello"));
}

#[test]
fn example_factorial() {
    let output = Command::new("cargo")
        .args(["run", "--", "run", "example-programs/factorial.pp"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // factorial(5) = 120
    assert!(stdout.contains("120"));
}

// ... repeat for each example program
```

### Task 1.18: Helper module `emit_rt_call`

Create a shared helper to reduce boilerplate in instruction compilation:

```rust
fn emit_rt_call(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    helper_ptr_data_id: DataId,
    args_layout: &[Arg],
) -> Result<(), JitError> {
    // 1. Calculate total buffer size
    // 2. Allocate stack slot
    // 3. Write each arg to the buffer
    //    - For immediate values (type tags): const directly
    //    - For ValueIds: load from the value map first
    // 4. Call the helper
    // 5. Return
}
```

Where `Arg` is:
```rust
enum Arg {
    Value(ValueId),      // JIT-computed value
    TagConst(u32),       // Immediate type tag constant
    I64Const(i64),       // Immediate integer constant
}
```

---

## Day 2 — Late (Hours 8–12): Integration + Polish

### Task 1.19: Fix clippy warnings in stdlib

The array tests have 3 non-snake-case function names:
- `flatMap_flattens_mapped_arrays`
- `flatMap_returns_empty_for_empty_array`
- `flatMap_rejects_non_array_result`

Rename to snake_case:
- `flat_map_flattens_mapped_arrays`
- `flat_map_returns_empty_for_empty_array`
- `flat_map_rejects_non_array_result`

**File:** `crates/stdlib/src/array.rs`

### Task 1.20: Verify all 14 example programs

```bash
cd /home/dijith/.Data/dev/pipe-lang

for f in example-programs/*.pp; do
    echo "=== $f ==="
    cargo run -- run "$f"
    echo "Exit code: $?"
done
```

Expected: All 14 compile and execute. At minimum, no JIT errors. Programs may produce incomplete output (e.g., sorting may not print correctly without all array ops), but they must not crash with JIT compile errors.

### Task 1.21: Run full verification suite

```bash
cargo test --lib runtime
cargo clippy -- -D warnings
cargo fmt --check
```

---

## Deliverables

1. `crates/runtime/src/rt_helpers.rs` — 13 RT helpers (1 builtin bridge + 5 array + 3 record + 3 tag + 1 panic + 2 closure)
2. `crates/runtime/src/jit.rs` — All 11 missing instruction arms + TailCall terminator
3. `crates/runtime/src/jit.rs` — CallNamed → global_registry() fallback via `pipe_rt_call_builtin`
4. `crates/runtime/src/jit.rs` — `emit_rt_call` helper for reducing instruction compilation boilerplate
5. `crates/runtime/src/jit.rs` — 30+ new unit tests
6. `crates/runtime/tests/example_programs.rs` — 14 E2E integration tests
7. `crates/stdlib/src/array.rs` — Fixed clippy warnings

---

## Verification Matrix

| Program | Status after this phase |
|---|---|
| `hello.pp` | Compiles + runs, prints "Hello, World!" |
| `factorial.pp` | Compiles + runs, prints "120" |
| `fibonacci.pp` | Compiles + runs |
| `sorting.pp` | Compiles + runs with output |
| `patterns.pp` | Compiles + runs with output |
| All others | Compile + run without JIT crash |

---

## Effect<T> Support (Changes from Dijith)

Dijith added `IrType::Effect(Box<IrType>)` to the IR type system and `MonoType::Effect` to the typechecker. This is handled in:
- `storage_type()` → `types::I64` (Effect is a heap pointer behind Arc)
- `storage_size()` → `8` (pointer-sized)
- `ir_type_tag()` → `Some(15)` (runtime tag for `Value::Effect`)
- `is_heap()` → includes Effect
- `decode_main_i32()` → returns `Ok(0)` (Effect main returns success)

IO builtins now have type signatures like `println : (str) -> Effect<()>` in the typechecker. At runtime, they execute immediately and return Unit — the Effect wrapper is lost at the JIT boundary. The type system enforces that `main : () -> Effect<()>`.

## Notes for Coordination

- **Dijith** has completed all typechecker/IR/Effect-system changes and the match-lowering fix. Programs now typecheck and lower correctly. The builtin bridge was also done as a stopgap — verify and own it.
- **Member 2** is adding stdlib builtins (`drop`, `take`, `sqrt`, `unwrap`, `Effect.map`, `Effect.flatMap`). IO builtins now return `Value::Effect` type-wise. Your builtin bridge handles any builtin present in `global_registry()`.
- **Member 3** is building LSP + tree-sitter — independent of your work.
- If you encounter a missing value in `ir_type_tag()`, add it to the match in the `encode_value`/`decode_value` helpers.
- `is_heap()` in `IrType` must return `true` for Array, Record, Tag, Closure, Effect, Str. Already done.
