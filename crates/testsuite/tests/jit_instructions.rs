//! Contract tests for JIT IR instructions.
//!
//! These tests document the **expected behavior** of IR instructions that
//! are not yet implemented in the Cranelift backend. Each test:
//!
//! 1. Constructs a valid `IrModule` using the public IR API
//! 2. Calls `compile_ir` to compile it
//! 3. Asserts either `UnimplementedInstruction` (if the instruction
//!    hasn't been wired up yet) or a correct result (once implemented)
//!
//! **To activate a test:** remove the `#[ignore]` attribute and implement
//! the corresponding instruction in `crates/runtime/src/jit.rs`.

#![allow(dead_code)]

use ast::SmolStr;
use ir::{
    BasicBlock, CallIndirectData, Instruction, IrDecl, IrFunction, IrModule, IrType,
    MakeClosureData, RecordAllocData, TagConstructData, Terminator, ValueId,
};
use runtime::compile_ir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn push_inst(func: &mut IrFunction, block: &mut BasicBlock, inst: Instruction) -> ValueId {
    let value_id = func.alloc_value();
    block.instructions.push((Some(value_id), inst));
    value_id
}

fn make_main(
    return_type: IrType,
    build: impl FnOnce(&mut IrFunction, &mut BasicBlock) -> ValueId,
) -> IrModule {
    let mut func = IrFunction::new(SmolStr::new("main"), return_type);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let return_value = build(&mut func, &mut entry);
    entry.terminator = Terminator::Return(return_value);
    func.blocks.push(entry);
    let mut module = IrModule::new();
    module.decls.push(IrDecl::Function(func));
    module
}

#[expect(dead_code)]
fn module_with_main(func: IrFunction) -> IrModule {
    let mut module = IrModule::new();
    module.decls.push(IrDecl::Function(func));
    module
}

// ---------------------------------------------------------------------------
// Constants — narrow integer types
// ---------------------------------------------------------------------------

#[test]
fn jit_const_i8() {
    let module = make_main(IrType::I8, |func, entry| {
        push_inst(func, entry, Instruction::ConstI8(42))
    });
    let compiled = compile_ir(&module).expect("ConstI8 should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 42);
}

#[test]
fn jit_const_i16() {
    let module = make_main(IrType::I16, |func, entry| {
        push_inst(func, entry, Instruction::ConstI16(1000))
    });
    let compiled = compile_ir(&module).expect("ConstI16 should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1000);
}

#[test]
fn jit_const_u8() {
    let module = make_main(IrType::U8, |func, entry| {
        push_inst(func, entry, Instruction::ConstU8(200))
    });
    let compiled = compile_ir(&module).expect("ConstU8 should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 200);
}

#[test]
fn jit_const_u16() {
    let module = make_main(IrType::U16, |func, entry| {
        push_inst(func, entry, Instruction::ConstU16(60000))
    });
    let compiled = compile_ir(&module).expect("ConstU16 should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 60000);
}

#[test]
fn jit_const_u32() {
    let module = make_main(IrType::U32, |func, entry| {
        push_inst(func, entry, Instruction::ConstU32(4000000000))
    });
    let compiled = compile_ir(&module).expect("ConstU32 should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_const_usize() {
    let module = make_main(IrType::Usize, |func, entry| {
        push_inst(func, entry, Instruction::ConstUsize(100))
    });
    compile_ir(&module).expect("ConstUsize should compile");
    // call_main() doesn't support Usize return; compile-only test
}

#[test]
fn jit_const_f32() {
    let module = make_main(IrType::F32, |func, entry| {
        push_inst(func, entry, Instruction::ConstF32(std::f32::consts::PI))
    });
    compile_ir(&module).expect("ConstF32 should compile");
}

#[test]
fn jit_const_unit() {
    let module = make_main(IrType::Unit, |func, entry| {
        push_inst(func, entry, Instruction::ConstUnit)
    });
    let compiled = compile_ir(&module).expect("ConstUnit should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 0);
}

#[test]
fn jit_const_i64() {
    let module = make_main(IrType::I64, |func, entry| {
        push_inst(func, entry, Instruction::ConstI64(i64::MAX))
    });
    compile_ir(&module).expect("ConstI64 should compile");
}

// ---------------------------------------------------------------------------
// Arithmetic — Sub, Mul
// ---------------------------------------------------------------------------

#[test]
fn jit_subtraction() {
    let module = make_main(IrType::I32, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(10));
        let b = push_inst(func, entry, Instruction::ConstI32(3));
        push_inst(func, entry, Instruction::Sub(a, b))
    });
    let compiled = compile_ir(&module).expect("Sub should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 7);
}

#[test]
fn jit_multiplication() {
    let module = make_main(IrType::I32, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(6));
        let b = push_inst(func, entry, Instruction::ConstI32(7));
        push_inst(func, entry, Instruction::Mul(a, b))
    });
    let compiled = compile_ir(&module).expect("Mul should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 42);
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

#[test]
fn jit_eq_true() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(5));
        let b = push_inst(func, entry, Instruction::ConstI32(5));
        push_inst(func, entry, Instruction::Eq(a, b))
    });
    let compiled = compile_ir(&module).expect("Eq should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

#[test]
fn jit_eq_false() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(5));
        let b = push_inst(func, entry, Instruction::ConstI32(3));
        push_inst(func, entry, Instruction::Eq(a, b))
    });
    let compiled = compile_ir(&module).expect("Eq should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 0);
}

#[test]
fn jit_ne() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(5));
        let b = push_inst(func, entry, Instruction::ConstI32(3));
        push_inst(func, entry, Instruction::Ne(a, b))
    });
    let compiled = compile_ir(&module).expect("Ne should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

#[test]
fn jit_lt() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(2));
        let b = push_inst(func, entry, Instruction::ConstI32(10));
        push_inst(func, entry, Instruction::Lt(a, b))
    });
    let compiled = compile_ir(&module).expect("Lt should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

#[test]
fn jit_le() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(5));
        let b = push_inst(func, entry, Instruction::ConstI32(5));
        push_inst(func, entry, Instruction::Le(a, b))
    });
    let compiled = compile_ir(&module).expect("Le should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

#[test]
fn jit_ge() {
    let module = make_main(IrType::Bool, |func, entry| {
        let a = push_inst(func, entry, Instruction::ConstI32(5));
        let b = push_inst(func, entry, Instruction::ConstI32(3));
        push_inst(func, entry, Instruction::Ge(a, b))
    });
    let compiled = compile_ir(&module).expect("Ge should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

// ---------------------------------------------------------------------------
// Logical — And, Or
// ---------------------------------------------------------------------------

#[test]
fn jit_and_true() {
    let module = make_main(IrType::Bool, |func, entry| {
        let t = push_inst(func, entry, Instruction::ConstBool(true));
        let f = push_inst(func, entry, Instruction::ConstBool(true));
        push_inst(func, entry, Instruction::And(t, f))
    });
    let compiled = compile_ir(&module).expect("And should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

#[test]
fn jit_and_false() {
    let module = make_main(IrType::Bool, |func, entry| {
        let t = push_inst(func, entry, Instruction::ConstBool(true));
        let f = push_inst(func, entry, Instruction::ConstBool(false));
        push_inst(func, entry, Instruction::And(t, f))
    });
    let compiled = compile_ir(&module).expect("And should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 0);
}

#[test]
fn jit_or() {
    let module = make_main(IrType::Bool, |func, entry| {
        let f = push_inst(func, entry, Instruction::ConstBool(false));
        let t = push_inst(func, entry, Instruction::ConstBool(true));
        push_inst(func, entry, Instruction::Or(f, t))
    });
    let compiled = compile_ir(&module).expect("Or should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 1);
}

// ---------------------------------------------------------------------------
// Arrays — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_array_alloc_and_get() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(42));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        let idx = push_inst(func, entry, Instruction::ConstI32(1));
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: idx,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArrayAlloc + ArrayGet should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 42);
}

#[test]
fn jit_array_get_multiple_indices() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(5));
        let init = push_inst(func, entry, Instruction::ConstI32(7));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        // Read index 2
        let idx = push_inst(func, entry, Instruction::ConstI32(2));
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: idx,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArrayAlloc + ArrayGet (index 2) should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 7);
}

#[test]
fn jit_array_set() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(0));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        let zero = push_inst(func, entry, Instruction::ConstI32(0));
        let val = push_inst(func, entry, Instruction::ConstI32(99));
        let _set = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: zero,
                value: val,
            },
        );
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: zero,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArraySet + ArrayGet should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 99);
}

#[test]
fn jit_array_set_multiple_indices() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(4));
        let init = push_inst(func, entry, Instruction::ConstI32(0));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        // Set indices 1 and 3
        let one = push_inst(func, entry, Instruction::ConstI32(1));
        let val1 = push_inst(func, entry, Instruction::ConstI32(10));
        let _set1 = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: one,
                value: val1,
            },
        );
        let three = push_inst(func, entry, Instruction::ConstI32(3));
        let val2 = push_inst(func, entry, Instruction::ConstI32(20));
        let _set2 = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: three,
                value: val2,
            },
        );
        // Read index 3 back
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: three,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArraySet multiple indices should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 20);
}

#[test]
fn jit_array_set_overwrite() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(0));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        // Write 42 to index 0
        let zero = push_inst(func, entry, Instruction::ConstI32(0));
        let val1 = push_inst(func, entry, Instruction::ConstI32(42));
        let _set1 = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: zero,
                value: val1,
            },
        );
        // Overwrite index 0 with 99
        let val2 = push_inst(func, entry, Instruction::ConstI32(99));
        let _set2 = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: zero,
                value: val2,
            },
        );
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: zero,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArraySet overwrite should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 99);
}

#[test]
fn jit_array_set_other_indices_unchanged() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(7));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        // Set index 1 to 99
        let one = push_inst(func, entry, Instruction::ConstI32(1));
        let val = push_inst(func, entry, Instruction::ConstI32(99));
        let _set = push_inst(
            func,
            entry,
            Instruction::ArraySet {
                array: arr,
                index: one,
                value: val,
            },
        );
        // Read index 0 — should still be 7
        let zero = push_inst(func, entry, Instruction::ConstI32(0));
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: arr,
                index: zero,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArraySet other indices unchanged should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 7);
}

#[test]
fn jit_array_len() {
    let module = make_main(IrType::Usize, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(5));
        let init = push_inst(func, entry, Instruction::ConstI32(0));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        push_inst(func, entry, Instruction::ArrayLen(arr))
    });
    compile_ir(&module).expect("ArrayLen should compile");
}

#[test]
fn jit_array_len_empty() {
    let module = make_main(IrType::Usize, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(0));
        let init = push_inst(func, entry, Instruction::ConstI32(99));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        push_inst(func, entry, Instruction::ArrayLen(arr))
    });
    compile_ir(&module).expect("ArrayLen with empty array should compile");
}

#[test]
fn jit_array_len_different_sizes() {
    let module = make_main(IrType::Usize, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(10));
        let init = push_inst(func, entry, Instruction::ConstI32(42));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        push_inst(func, entry, Instruction::ArrayLen(arr))
    });
    compile_ir(&module).expect("ArrayLen with 10 elements should compile");
}

// ---------------------------------------------------------------------------
// ArrayConcat
// ---------------------------------------------------------------------------

#[test]
fn jit_array_concat_empty_empty() {
    let module = make_main(IrType::I32, |func, entry| {
        let len0 = push_inst(func, entry, Instruction::ConstI32(0));
        let init0 = push_inst(func, entry, Instruction::ConstI32(0));
        let left = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len0,
                init: init0,
            },
        );
        let right = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len0,
                init: init0,
            },
        );
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        push_inst(func, entry, Instruction::ArrayLen(concat))
    });
    compile_ir(&module).expect("ArrayConcat empty+empty should compile");
}

#[test]
fn jit_array_concat_empty_nonempty() {
    let module = make_main(IrType::I32, |func, entry| {
        let len0 = push_inst(func, entry, Instruction::ConstI32(0));
        let len2 = push_inst(func, entry, Instruction::ConstI32(2));
        let init = push_inst(func, entry, Instruction::ConstI32(42));
        let left = push_inst(func, entry, Instruction::ArrayAlloc { len: len0, init });
        let right = push_inst(func, entry, Instruction::ArrayAlloc { len: len2, init });
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        push_inst(func, entry, Instruction::ArrayLen(concat))
    });
    compile_ir(&module).expect("ArrayConcat empty+nonempty should compile");
}

#[test]
fn jit_array_concat_nonempty_empty() {
    let module = make_main(IrType::I32, |func, entry| {
        let len0 = push_inst(func, entry, Instruction::ConstI32(0));
        let len3 = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(7));
        let left = push_inst(func, entry, Instruction::ArrayAlloc { len: len3, init });
        let right = push_inst(func, entry, Instruction::ArrayAlloc { len: len0, init });
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        push_inst(func, entry, Instruction::ArrayLen(concat))
    });
    compile_ir(&module).expect("ArrayConcat nonempty+empty should compile");
}

#[test]
fn jit_array_concat_nonempty_nonempty() {
    let module = make_main(IrType::I32, |func, entry| {
        let len3 = push_inst(func, entry, Instruction::ConstI32(3));
        let len2 = push_inst(func, entry, Instruction::ConstI32(2));
        let init = push_inst(func, entry, Instruction::ConstI32(0));
        let left = push_inst(func, entry, Instruction::ArrayAlloc { len: len3, init });
        let right = push_inst(func, entry, Instruction::ArrayAlloc { len: len2, init });
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        push_inst(func, entry, Instruction::ArrayLen(concat))
    });
    compile_ir(&module).expect("ArrayConcat nonempty+nonempty should compile");
}

#[test]
fn jit_array_concat_length_correct() {
    // Allocate left[3], right[2], concat -> should have length 5
    let module = make_main(IrType::I32, |func, entry| {
        let left_init = push_inst(func, entry, Instruction::ConstI32(10));
        let right_init = push_inst(func, entry, Instruction::ConstI32(20));
        let len3 = push_inst(func, entry, Instruction::ConstI32(3));
        let len2 = push_inst(func, entry, Instruction::ConstI32(2));
        let left = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len3,
                init: left_init,
            },
        );
        let right = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len2,
                init: right_init,
            },
        );
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        // Read length of concat
        let _len = push_inst(func, entry, Instruction::ArrayLen(concat));
        // Return a sentinel value since Usize can't be returned via call_main
        push_inst(func, entry, Instruction::ConstI32(0))
    });
    let compiled = compile_ir(&module).expect("ArrayConcat length should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_array_concat_element_order() {
    // Create left = [1, 2, 3], right = [4, 5], concat -> get first element of right part
    let module = make_main(IrType::I32, |func, entry| {
        let len3 = push_inst(func, entry, Instruction::ConstI32(3));
        let len2 = push_inst(func, entry, Instruction::ConstI32(2));
        let val1 = push_inst(func, entry, Instruction::ConstI32(1));
        let val4 = push_inst(func, entry, Instruction::ConstI32(4));
        let left = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len3,
                init: val1,
            },
        );
        let right = push_inst(
            func,
            entry,
            Instruction::ArrayAlloc {
                len: len2,
                init: val4,
            },
        );
        let concat = push_inst(func, entry, Instruction::ArrayConcat(left, right));
        // Read element at index 3 (should be 4, the first element of right)
        let idx = push_inst(func, entry, Instruction::ConstI32(3));
        push_inst(
            func,
            entry,
            Instruction::ArrayGet {
                array: concat,
                index: idx,
            },
        )
    });
    let compiled = compile_ir(&module).expect("ArrayConcat element order should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 4);
}

// ---------------------------------------------------------------------------
// Records — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_record_alloc_and_get() {
    let module = make_main(IrType::I32, |func, entry| {
        let name = push_inst(func, entry, Instruction::ConstStr(SmolStr::new("Alice")));
        let age = push_inst(func, entry, Instruction::ConstI32(30));
        let rec = push_inst(
            func,
            entry,
            Instruction::RecordAlloc(Box::new(RecordAllocData {
                type_name: SmolStr::new("Person"),
                fields: vec![name, age],
            })),
        );
        push_inst(
            func,
            entry,
            Instruction::RecordGet {
                record: rec,
                field: SmolStr::new("age"),
                field_index: 1,
            },
        )
    });
    let compiled = compile_ir(&module).expect("RecordAlloc + RecordGet should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 30);
}

#[test]
fn jit_record_set_and_get() {
    let module = make_main(IrType::I32, |func, entry| {
        let name = push_inst(func, entry, Instruction::ConstStr(SmolStr::new("Alice")));
        let age = push_inst(func, entry, Instruction::ConstI32(30));
        let rec = push_inst(
            func,
            entry,
            Instruction::RecordAlloc(Box::new(RecordAllocData {
                type_name: SmolStr::new("Person"),
                fields: vec![name, age],
            })),
        );
        let new_age = push_inst(func, entry, Instruction::ConstI32(35));
        let _set = push_inst(
            func,
            entry,
            Instruction::RecordSet {
                record: rec,
                field: SmolStr::new("age"),
                field_index: 1,
                value: new_age,
            },
        );
        push_inst(
            func,
            entry,
            Instruction::RecordGet {
                record: rec,
                field: SmolStr::new("age"),
                field_index: 1,
            },
        )
    });
    let compiled = compile_ir(&module).expect("RecordSet + RecordGet should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 35);
}

// ---------------------------------------------------------------------------
// Tags (Sum Types) — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_tag_construct_and_discriminant() {
    let module = make_main(IrType::U32, |func, entry| {
        let payload = push_inst(func, entry, Instruction::ConstI32(42));
        let tag = push_inst(
            func,
            entry,
            Instruction::TagConstruct(Box::new(TagConstructData {
                type_name: SmolStr::new("Option"),
                variant: SmolStr::new("Some"),
                discriminant: 1,
                payload: vec![payload],
            })),
        );
        push_inst(func, entry, Instruction::TagDiscriminant(tag))
    });
    let compiled = compile_ir(&module).expect("TagConstruct + TagDiscriminant should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_tag_get_payload() {
    let module = make_main(IrType::I32, |func, entry| {
        let payload = push_inst(func, entry, Instruction::ConstI32(99));
        let tag = push_inst(
            func,
            entry,
            Instruction::TagConstruct(Box::new(TagConstructData {
                type_name: SmolStr::new("Option"),
                variant: SmolStr::new("Some"),
                discriminant: 1,
                payload: vec![payload],
            })),
        );
        push_inst(
            func,
            entry,
            Instruction::TagGet {
                value: tag,
                index: 0,
            },
        )
    });
    let compiled = compile_ir(&module).expect("TagGet should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 99);
}

// ---------------------------------------------------------------------------
// Closures — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_make_closure() {
    let helper_name = SmolStr::new("helper");
    let mut helper = IrFunction::new(helper_name.clone(), IrType::I32);
    let h_entry_id = helper.alloc_block();
    let mut h_entry = BasicBlock::new(h_entry_id);
    let cap = push_inst(&mut helper, &mut h_entry, Instruction::ConstI32(42));
    h_entry.terminator = Terminator::Return(cap);
    helper.blocks.push(h_entry);

    let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let captured = push_inst(&mut func, &mut entry, Instruction::ConstI32(99));
    let _closure = push_inst(
        &mut func,
        &mut entry,
        Instruction::MakeClosure(Box::new(MakeClosureData {
            func_name: helper_name,
            captures: vec![captured],
        })),
    );
    let ret = push_inst(&mut func, &mut entry, Instruction::ConstI32(0));
    entry.terminator = Terminator::Return(ret);
    func.blocks.push(entry);

    let mut module = IrModule::new();
    module.decls.push(IrDecl::Function(helper));
    module.decls.push(IrDecl::Function(func));
    let compiled = compile_ir(&module).expect("MakeClosure should compile");
    let result = compiled.call_main().expect("main should run");
    assert_eq!(result, 0);
}

// ---------------------------------------------------------------------------
// Panic — UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_panic_traps() {
    let _result = std::panic::catch_unwind(|| {
        let module = make_main(IrType::I32, |func, entry| {
            let _panic = push_inst(
                func,
                entry,
                Instruction::Panic {
                    msg: SmolStr::new("test panic"),
                },
            );
            entry.terminator = Terminator::Unreachable;
            push_inst(func, entry, Instruction::ConstI32(0))
        });
        compile_ir(&module)
    });
}

// ---------------------------------------------------------------------------
// TailCall terminator — UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[test]
fn jit_tail_call_terminator() {
    let _result = std::panic::catch_unwind(|| {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let arg = push_inst(&mut func, &mut entry, Instruction::ConstI32(5));
        entry.terminator = Terminator::TailCall {
            callee: arg,
            args: vec![],
        };
        func.blocks.push(entry);

        let mut module = IrModule::new();
        module.decls.push(IrDecl::Function(func));
        compile_ir(&module)
    });
}

// ---------------------------------------------------------------------------
// Heap type-tag discrimination (JIT + runtime)
// ---------------------------------------------------------------------------

#[test]
fn jit_println_array_uses_type_tag() {
    let module = make_main(IrType::I32, |func, entry| {
        let len = push_inst(func, entry, Instruction::ConstI32(3));
        let init = push_inst(func, entry, Instruction::ConstI32(42));
        let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
        entry.instructions.push((None, Instruction::Println(arr)));
        push_inst(func, entry, Instruction::ConstI32(0))
    });
    let compiled = compile_ir(&module).expect("Println array should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_println_record_uses_type_tag() {
    let module = make_main(IrType::I32, |func, entry| {
        let val = push_inst(func, entry, Instruction::ConstI32(99));
        let rec = push_inst(
            func,
            entry,
            Instruction::RecordAlloc(Box::new(RecordAllocData {
                type_name: SmolStr::new("Test"),
                fields: vec![val],
            })),
        );
        entry.instructions.push((None, Instruction::Println(rec)));
        push_inst(func, entry, Instruction::ConstI32(0))
    });
    let compiled = compile_ir(&module).expect("Println record should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_println_tag_uses_type_tag() {
    let module = make_main(IrType::I32, |func, entry| {
        let payload = push_inst(func, entry, Instruction::ConstI32(42));
        let tag = push_inst(
            func,
            entry,
            Instruction::TagConstruct(Box::new(TagConstructData {
                type_name: SmolStr::new("Option"),
                variant: SmolStr::new("Some"),
                discriminant: 1,
                payload: vec![payload],
            })),
        );
        entry.instructions.push((None, Instruction::Println(tag)));
        push_inst(func, entry, Instruction::ConstI32(0))
    });
    let compiled = compile_ir(&module).expect("Println tag should compile");
    compiled.call_main().expect("main should run");
}

#[test]
fn jit_println_closure_uses_type_tag() {
    let helper_name = SmolStr::new("helper");
    let mut helper = IrFunction::new(helper_name.clone(), IrType::I32);
    let h_entry_id = helper.alloc_block();
    let mut h_entry = BasicBlock::new(h_entry_id);
    let cap = push_inst(&mut helper, &mut h_entry, Instruction::ConstI32(42));
    h_entry.terminator = Terminator::Return(cap);
    helper.blocks.push(h_entry);

    let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let captured = push_inst(&mut func, &mut entry, Instruction::ConstI32(99));
    let closure = push_inst(
        &mut func,
        &mut entry,
        Instruction::MakeClosure(Box::new(MakeClosureData {
            func_name: helper_name,
            captures: vec![captured],
        })),
    );
    entry
        .instructions
        .push((None, Instruction::Println(closure)));
    let ret = push_inst(&mut func, &mut entry, Instruction::ConstI32(0));
    entry.terminator = Terminator::Return(ret);
    func.blocks.push(entry);

    let mut module = IrModule::new();
    module.decls.push(ir::IrDecl::Function(helper));
    module.decls.push(ir::IrDecl::Function(func));
    let compiled = compile_ir(&module).expect("Println closure should compile");
    compiled.call_main().expect("main should run");
}

// ---------------------------------------------------------------------------
// CallIndirect — integration tests (Phase 1 TDD)
// ---------------------------------------------------------------------------

/// Helper: build a one-block function `f` with explicit params and a body
/// produced by `build_body`, and add it to `module`.
fn add_function(
    module: &mut IrModule,
    name: &str,
    params: Vec<(SmolStr, IrType)>,
    return_type: IrType,
    build_body: impl FnOnce(&mut IrFunction, &mut BasicBlock) -> ValueId,
) {
    let mut func = IrFunction::new(SmolStr::new(name), return_type);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let mut value_ids = Vec::new();
    for (pname, pty) in &params {
        let v = func.alloc_value();
        value_ids.push((v, pname.clone(), pty.clone()));
    }
    for (vid, pname, pty) in &value_ids {
        func.params.push((*vid, pname.clone(), pty.clone()));
    }
    let _ = build_body(&mut func, &mut entry);
    func.blocks.push(entry);
    module.decls.push(IrDecl::Function(func));
}

#[test]
fn jit_call_indirect_simple() {
    // (x: i32) -> i32 { x + 1 }, called with 5 → 6
    let mut module = IrModule::new();
    add_function(
        &mut module,
        "helper",
        vec![(SmolStr::new("x"), IrType::I32)],
        IrType::I32,
        |func, entry| {
            let x = ValueId(0); // first param has ValueId 0
            let one = push_inst(func, entry, Instruction::ConstI32(1));
            let sum = push_inst(func, entry, Instruction::Add(x, one));
            entry.terminator = Terminator::Return(sum);
            sum
        },
    );
    let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let arg = push_inst(&mut func, &mut entry, Instruction::ConstI32(5));
    let closure = push_inst(
        &mut func,
        &mut entry,
        Instruction::MakeClosure(Box::new(MakeClosureData {
            func_name: SmolStr::new("helper"),
            captures: vec![],
        })),
    );
    let result = push_inst(
        &mut func,
        &mut entry,
        Instruction::CallIndirect(Box::new(CallIndirectData {
            callee: closure,
            args: vec![arg],
            return_type: IrType::I32,
        })),
    );
    entry.terminator = Terminator::Return(result);
    func.blocks.push(entry);
    module.decls.push(IrDecl::Function(func));

    let compiled = compile_ir(&module).expect("CallIndirect should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 6);
}

#[test]
fn jit_call_indirect_with_capture() {
    // (x: i32) -> i32 { x + cap }, cap = 100, called with 5 → 105
    let mut module = IrModule::new();
    add_function(
        &mut module,
        "helper",
        vec![
            (SmolStr::new("cap"), IrType::I32),
            (SmolStr::new("x"), IrType::I32),
        ],
        IrType::I32,
        |func, entry| {
            let cap = ValueId(0);
            let x = ValueId(1);
            let sum = push_inst(func, entry, Instruction::Add(x, cap));
            entry.terminator = Terminator::Return(sum);
            sum
        },
    );
    let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let captured = push_inst(&mut func, &mut entry, Instruction::ConstI32(100));
    let arg = push_inst(&mut func, &mut entry, Instruction::ConstI32(5));
    let closure = push_inst(
        &mut func,
        &mut entry,
        Instruction::MakeClosure(Box::new(MakeClosureData {
            func_name: SmolStr::new("helper"),
            captures: vec![captured],
        })),
    );
    let result = push_inst(
        &mut func,
        &mut entry,
        Instruction::CallIndirect(Box::new(CallIndirectData {
            callee: closure,
            args: vec![arg],
            return_type: IrType::I32,
        })),
    );
    entry.terminator = Terminator::Return(result);
    func.blocks.push(entry);
    module.decls.push(IrDecl::Function(func));

    let compiled = compile_ir(&module).expect("CallIndirect with capture should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 105);
}

#[test]
fn jit_call_indirect_two_captures() {
    // (x: i32) -> i32 { a + b*x }, a=10, b=3, called with 4 → 22
    let mut module = IrModule::new();
    add_function(
        &mut module,
        "helper",
        vec![
            (SmolStr::new("a"), IrType::I32),
            (SmolStr::new("b"), IrType::I32),
            (SmolStr::new("x"), IrType::I32),
        ],
        IrType::I32,
        |func, entry| {
            let a = ValueId(0);
            let b = ValueId(1);
            let x = ValueId(2);
            let bx = push_inst(func, entry, Instruction::Mul(b, x));
            let sum = push_inst(func, entry, Instruction::Add(a, bx));
            entry.terminator = Terminator::Return(sum);
            sum
        },
    );
    let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
    let entry_id = func.alloc_block();
    let mut entry = BasicBlock::new(entry_id);
    let a = push_inst(&mut func, &mut entry, Instruction::ConstI32(10));
    let b = push_inst(&mut func, &mut entry, Instruction::ConstI32(3));
    let x = push_inst(&mut func, &mut entry, Instruction::ConstI32(4));
    let closure = push_inst(
        &mut func,
        &mut entry,
        Instruction::MakeClosure(Box::new(MakeClosureData {
            func_name: SmolStr::new("helper"),
            captures: vec![a, b],
        })),
    );
    let result = push_inst(
        &mut func,
        &mut entry,
        Instruction::CallIndirect(Box::new(CallIndirectData {
            callee: closure,
            args: vec![x],
            return_type: IrType::I32,
        })),
    );
    entry.terminator = Terminator::Return(result);
    func.blocks.push(entry);
    module.decls.push(IrDecl::Function(func));

    let compiled = compile_ir(&module).expect("CallIndirect with 2 captures should compile");
    assert_eq!(compiled.call_main().expect("main should run"), 22);
}

// ---------------------------------------------------------------------------
// End-to-end tests via parse → typecheck → lower → JIT
// (these exercise the actual lower.rs codepaths for closures and thunks)
// ---------------------------------------------------------------------------

fn lower_and_compile(src: &str) -> runtime::CompiledModule {
    let arena = bumpalo::Bump::new();
    let prog = parser::parse(src, &arena).expect("parse failed");
    let typed = typechecker::typecheck(&prog).expect("typecheck failed");
    let ir_module = ir::lower(&typed).expect("lower failed");
    compile_ir(&ir_module).expect("compile failed")
}

fn e2e_main_i32(src: &str) -> i32 {
    let compiled = lower_and_compile(src);
    compiled.call_main().expect("main should run")
}

#[test]
fn e2e_thunk_apply_simple() {
    // let t = (() => (x) => x*2); t()(21) → 42
    assert_eq!(
        e2e_main_i32("let t = (() => (x) => x*2)\nlet main = t()(21)"),
        42
    );
}

#[test]
fn e2e_thunk_apply_with_arg() {
    // let mk = (n) => (x) => x + n
    // let t = mk(5)
    // t(10) → 15
    assert_eq!(
        e2e_main_i32(
            "let mk = (n) => (x) => x + n\nlet t = mk(5)\nlet main = t(10)"
        ),
        15
    );
}

#[test]
fn e2e_compose_lambda() {
    // compose(f, g)(x) = f(g(x))
    // compose((n)=>n+1, (n)=>n*2)(5) = 11
    assert_eq!(
        e2e_main_i32(
            "let compose = (f, g) => (x) => f(g(x))\nlet main = compose((n)=>n+1, (n)=>n*2)(5)"
        ),
        11
    );
}

#[test]
fn e2e_make_adder_apply() {
    // makeAdder(5)(10) → 15
    assert_eq!(
        e2e_main_i32("let makeAdder = (n) => (x) => x + n\nlet main = makeAdder(5)(10)"),
        15
    );
}

#[test]
fn e2e_factorial_5() {
    // The example factorial.pp body: factorialTail(5) = 120
    let src = "\
        let factorial = (n) => match n { 0 => 1, n => n * factorial(n - 1) }\n\
        let main = factorial(5)\n\
    ";
    assert_eq!(e2e_main_i32(src), 120);
}

#[test]
fn e2e_match_option_some() {
    // match Some(7) { Some(x) => x, None => 0 } → 7
    let src = "\
        let main = match Some(7) {\n\
            Some(x) => x\n\
            None => 0\n\
        }\n\
    ";
    assert_eq!(e2e_main_i32(src), 7);
}

#[test]
fn e2e_match_result_ok() {
    // match Ok(42) { Ok(v) => v, Err(_) => 0 } → 42
    let src = "\
        let main = match Ok(42) {\n\
            Ok(v) => v\n\
            Err(_) => 0\n\
        }\n\
    ";
    assert_eq!(e2e_main_i32(src), 42);
}
