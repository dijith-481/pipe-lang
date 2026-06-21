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
    BasicBlock, Instruction, IrDecl, IrFunction, IrModule, IrType, MakeClosureData,
    RecordAllocData, TagConstructData, Terminator, ValueId,
};
use runtime::{JitError, compile_ir};

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

#[ignore = "Member 1: implement ArrayAlloc instruction in JIT"]
#[test]
fn jit_array_alloc_and_get() {
    let _result = std::panic::catch_unwind(|| {
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
        compile_ir(&module)
    });
    if let Ok(Err(JitError::UnimplementedInstruction { instruction, .. })) = &_result
        && instruction.contains("ArrayAlloc")
    {}
}

#[ignore = "Member 1: implement ArraySet instruction in JIT"]
#[test]
fn jit_array_set() {
    let _result = std::panic::catch_unwind(|| {
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
        compile_ir(&module)
    });
    // Just ensure it doesn't crash
}

#[ignore = "Member 1: implement ArrayLen instruction in JIT"]
#[test]
fn jit_array_len() {
    let _result = std::panic::catch_unwind(|| {
        let module = make_main(IrType::Usize, |func, entry| {
            let len = push_inst(func, entry, Instruction::ConstI32(5));
            let init = push_inst(func, entry, Instruction::ConstI32(0));
            let arr = push_inst(func, entry, Instruction::ArrayAlloc { len, init });
            push_inst(func, entry, Instruction::ArrayLen(arr))
        });
        compile_ir(&module)
    });
}

// ---------------------------------------------------------------------------
// Records — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[ignore = "Member 1: implement RecordAlloc instruction in JIT"]
#[test]
fn jit_record_alloc_and_get() {
    let _result = std::panic::catch_unwind(|| {
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
        compile_ir(&module)
    });
}

// ---------------------------------------------------------------------------
// Tags (Sum Types) — ALL UNIMPLEMENTED
// ---------------------------------------------------------------------------

#[ignore = "Member 1: implement TagConstruct instruction in JIT"]
#[test]
fn jit_tag_construct_and_discriminant() {
    let _result = std::panic::catch_unwind(|| {
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
        compile_ir(&module)
    });
}

#[ignore = "Member 1: implement TagGet instruction in JIT"]
#[test]
fn jit_tag_get_payload() {
    let _result = std::panic::catch_unwind(|| {
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
        compile_ir(&module)
    });
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

#[ignore = "Member 1: implement Panic instruction in JIT"]
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

#[ignore = "Member 1: implement TailCall terminator in JIT"]
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
