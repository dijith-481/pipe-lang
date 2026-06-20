# PR #8: JIT Compiler Issues — Status

Analysis of `remotes/origin/pr/8` — the Cranelift JIT rewrite from skeleton to multi-block compiler.

**Status:** All identified issues have been addressed on `rework/pr-8`. See resolution notes below.

---

## RESOLVED

### JIT-001: Undefined Behavior — Unbounded Null-Terminated String Scanning

**Severity:** Critical → **✅ FIXED**

**Resolution** (`jit.rs`):
- Changed data object format from `[bytes..., \0]` to `[len: u32][bytes...]` (length-prefixed)
- `__pipe_println` now reads `*(ptr as *const u32)` for length, then reads `len` bytes from `ptr.add(4)`
- `pipe_rt_str_concat` uses same pattern for both input string reads and output string writes
- All tests updated to use `check_len_prefixed_str(ptr, expected)` helper

No more unbounded scans. The string length is always known from the prefix.

### JIT-002: Memory Leak — Every StrConcat Leaks

**Severity:** Critical → **⚠️ MITIGATED (pending Phase 4)**

**Resolution** (`jit.rs`):
- Output format changed from null-terminated to length-prefixed
- Leak is documented as intentional: `Box::leak(buf.into_boxed_slice()).as_ptr()`
- This is acceptable for Phases 1-2 (JIT modules are short-lived, OS reclaims on exit)
- When Phase 4 heap-value memory management (Arc) is implemented, this will be replaced

### JIT-003: `decode_main_i32` Silently Truncates Wide Return Types

**Severity:** Medium → **✅ FIXED**

**Resolution** (`jit.rs`):
- `decode_main_i32` now rejects lossy types (`I64`, `U64`, `Usize`, `F32`, `F64`) with an error
- Only safe types (`I8`/`I16`/`I32`/`U8`/`U16`/`U32`/`Bool`/`Unit`) are decoded to `i32`
- Users who need wider types should use `call_main_raw()` and decode manually

### JIT-004: `compile_bool_binary` Validates Left Operand Only

**Severity:** Medium → **✅ FIXED**

**Resolution** (`jit.rs`):
- Both `compile_bool_binary` and `compile_numeric_binary` now validate both `left` and `right` operand types

### JIT-005: `Not` Instruction Uses Non-Canonical Cranelift Op

**Severity:** Medium → **❌ FALSE POSITIVE**

**Resolution:** The original `icmp_imm(Equal, x, 0)` is the **correct** implementation.

`bnot` is bitwise NOT. For Bool stored as `I8`: `bnot(1) = 254 ≠ 0`. The logical NOT must zero-check: `icmp_imm(Equal, x, 0)` returns `1` when `x == 0` (false → true) and `0` when `x ≠ 0` (true → false). This is the canonical pattern for Bool NOT.

### JIT-006: Float Remainder Emulates `frem` with Four Instructions

**Severity:** Medium → **❌ NOT APPLICABLE**

**Resolution:** Cranelift 0.132 does not have a native `frem` instruction. The emulation pattern `fdiv → trunc → fmul → fsub` is the correct approach for this version. If a future Cranelift release adds `frem`, this can be simplified.

### JIT-007: `store_return_value` Discards `storage_type` Result

**Severity:** Medium → **✅ FIXED**

**Resolution** (`jit.rs`): Removed the dead `storage_type` call.

### JIT-008: `compile_switch` Validates Arms After Looking Up Discriminant

**Severity:** Medium → **✅ FIXED**

**Resolution** (`jit.rs`): Moved `validate_switch_arms` before `lookup_value` so validation fails fast with no unnecessary work.

---

## OPEN (Design Issues — Future Phase)

### JIT-009: Hardcoded Runtime Helpers vs BuiltinRegistry

**Severity:** Design — **Pending Phase 6**

The `BuiltinRegistry` abstraction (`HashMap<String, Box<dyn BuiltinFunction>>`) should eventually replace the ad-hoc `global_value → load ptr → call_indirect` pattern. This is scoped for Phase 6 and requires both JIT refactoring (Member 1) and runtime helper consolidation (Member 2).

### JIT-010: ABI — Heap Types Passed as Unboxed i64 Instead of Fat Pointers

**Severity:** Design — **Partially addressed by JIT-001**

The length-prefixed data layout eliminates the null-scan UB (JIT-001) but still passes only a single i64 pointer, not a fat pointer `(ptr, len)`. A full fat-pointer ABI would be a breaking change. This should be implemented when adding heap type support (Array/Record/Tag) to avoid churn.

---

## Summary

| ID | Severity | Status | Resolution |
|---|---|---|---|
| JIT-001 | Critical | ✅ Fixed | Length-prefixed string layout |
| JIT-002 | Critical | ⚠️ Mitigated | Leak documented for v0.1, pending Phase 4 Arc model |
| JIT-003 | Medium | ✅ Fixed | `decode_main_i32` errors on lossy types |
| JIT-004 | Medium | ✅ Fixed | Both operands validated in `compile_bool_binary` and `compile_numeric_binary` |
| JIT-005 | Medium | ❌ False positive | `icmp_imm(EQ, 0)` is correct for Bool NOT; `bnot` is bitwise |
| JIT-006 | Medium | ❌ Not applicable | `frem` does not exist in Cranelift 0.132 |
| JIT-007 | Medium | ✅ Fixed | Removed dead `storage_type` call |
| JIT-008 | Medium | ✅ Fixed | Validate arms before discriminant lookup |
| JIT-009 | Design | 🔲 Pending Phase 6 | BuiltinRegistry abstraction |
| JIT-010 | Design | 🔲 Pending heap types | Full fat-pointer ABI |
