//! Cranelift JIT compiler for pipe-lang IR.
//!
//! This module translates an [`IrModule`](ir::IrModule) into native
//! code using Cranelift. The translation is one-to-one for simple
//! cases (constants, arithmetic, calls) and falls back to runtime
//! builtins for higher-level operations (arrays, records, effects).
//!
//! # Status
//!
//! Track B Day 1-2: skeleton. The compiler handles `ConstI32`,
//! `Return`, and produces a callable function pointer. More
//! instructions are added on Day 2-4.
//!
//! # Calling convention
//!
//! All pipe-lang functions are called via:
//!
//! ```text
//! extern "C" fn pipe_func(args: *const u8, ret: *mut u8) -> i32
//! ```
//!
//! - `args` is a pointer to a packed `u8` buffer containing the
//!   arguments in order. Each primitive is stored in its native size.
//!   Heap-typed values (`Str`, `Array`, `Record`, `Tag`, `Closure`)
//!   are passed as a fat pointer `(ptr: *const u8, len: usize)`.
//! - `ret` is a pointer to a `u8` buffer for the return value.
//! - The return value is `0` for success, non-zero for a panic.

use cranelift_codegen::ir::types::I32;
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, MemFlags, UserFuncName, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

use ir::{BasicBlock, IrFunction, IrModule, IrType, Terminator};

/// A pointer to a JIT-compiled function with the pipe-lang ABI.
pub type PipeFuncPtr = unsafe extern "C" fn(args: *const u8, ret: *mut u8) -> i32;

/// Errors that can occur during JIT compilation.
#[derive(Debug, thiserror::Error)]
pub enum JitError {
    /// Cranelift returned a non-zero error code.
    #[error("cranelift error: {msg}")]
    Cranelift { msg: String },
    /// The IR contained an instruction that is not yet implemented
    /// in the JIT. The lowerer should either implement it or split
    /// the program into a smaller test case.
    #[error("unimplemented IR instruction: {instruction} (in {function})")]
    UnimplementedInstruction {
        instruction: String,
        function: String,
    },
    /// The module has no `main` function.
    #[error("module has no `main` function")]
    NoMain,
    /// Native ISA builder failed (very rare on supported platforms).
    #[error("native ISA builder failed: {msg}")]
    IsaBuilder { msg: String },
}

impl From<cranelift_module::ModuleError> for JitError {
    fn from(e: cranelift_module::ModuleError) -> Self {
        JitError::Cranelift {
            msg: format!("{e:?}"),
        }
    }
}

/// A compiled IR module, ready to execute.
pub struct CompiledModule {
    /// Kept around so its `JITModule` allocations stay live.
    _module: Box<JITModule>,
    /// The function pointer for `main`.
    main_ptr: PipeFuncPtr,
}

// SAFETY: a JITModule is single-threaded in use but safe to share
// across threads when no compilation is in progress; PipeFuncPtr is
// a plain function pointer.
unsafe impl Send for CompiledModule {}
unsafe impl Sync for CompiledModule {}

impl CompiledModule {
    /// Calls the module's `main` function with no arguments and
    /// returns its `i32` result.
    ///
    /// For v0.1, `main` must have signature `() -> i32` or
    /// `() -> Effect<()>` (effects are sequentialized into
    /// `i32` return = 0 on success).
    ///
    /// # Errors
    ///
    /// Returns [`JitError`] if the main function returns non-zero.
    pub fn call_main(&self) -> Result<i32, JitError> {
        let mut ret_buf = [0u8; 4];
        let code = unsafe { (self.main_ptr)(std::ptr::null(), ret_buf.as_mut_ptr()) };
        if code != 0 {
            return Err(JitError::Cranelift {
                msg: format!("main returned error code {code}"),
            });
        }
        Ok(i32::from_ne_bytes(ret_buf))
    }
}

/// Compiles an IR module into a [`CompiledModule`].
///
/// # Errors
///
/// Returns [`JitError`] for any of the failure modes listed above.
pub fn compile_ir(ir_module: &IrModule) -> Result<CompiledModule, JitError> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| JitError::IsaBuilder {
            msg: format!("flag: {e}"),
        })?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| JitError::IsaBuilder {
            msg: format!("flag: {e}"),
        })?;
    let isa_builder = cranelift_native::builder().map_err(|e| JitError::IsaBuilder {
        msg: format!("native ISA: {e:?}"),
    })?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| JitError::IsaBuilder {
            msg: format!("finish ISA: {e:?}"),
        })?;
    let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(builder);

    let mut fn_builder_ctx = FunctionBuilderContext::new();

    // Collect function name -> FuncId so we can resolve forward refs.
    let mut func_ids: Vec<(String, cranelift_module::FuncId, IrType)> = Vec::new();
    for func in ir_module.functions() {
        let name = func.name.as_str().to_string();
        let sig = make_signature(&module);
        let id = module
            .declare_function(&name, Linkage::Export, &sig)
            .map_err(JitError::from)?;
        func_ids.push((name, id, func.return_type.clone()));
    }

    // Compile each function. The function bodies are populated here;
    // finalize_definitions happens once at the end.
    for (name, func_id, ret_type) in &func_ids {
        let func = ir_module
            .function(name)
            .expect("function declared in pass 1");
        compile_function_body(&mut module, &mut fn_builder_ctx, func, *func_id, ret_type)?;
    }

    module.finalize_definitions().map_err(JitError::from)?;

    let main_id = func_ids
        .iter()
        .find(|(n, _, _)| n == "main")
        .ok_or(JitError::NoMain)?
        .1;
    let code_ptr = module.get_finalized_function(main_id);
    let main_ptr: PipeFuncPtr = unsafe { std::mem::transmute(code_ptr) };

    Ok(CompiledModule {
        _module: Box::new(module),
        main_ptr,
    })
}

/// Builds a Cranelift signature for a pipe-lang function.
fn make_signature(module: &JITModule) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();
    // All pipe-lang functions take a single opaque `*const u8` args
    // pointer and a single opaque `*mut u8` ret pointer.
    sig.params
        .push(AbiParam::new(cranelift_codegen::ir::types::I64));
    sig.params
        .push(AbiParam::new(cranelift_codegen::ir::types::I64));
    sig.returns.push(AbiParam::new(I32));
    sig
}

/// Compiles the body of one IR function into the module's slot
/// for `func_id`.
fn compile_function_body(
    module: &mut JITModule,
    fn_builder_ctx: &mut FunctionBuilderContext,
    func: &IrFunction,
    func_id: cranelift_module::FuncId,
    ret_type: &IrType,
) -> Result<(), JitError> {
    let sig = make_signature(module);
    let user_name = UserFuncName::user(0, func_id.as_u32());
    let mut clif_func = Function::with_name_signature(user_name, sig);

    // Create the entry block with the two pointer parameters.
    let mut builder = FunctionBuilder::new(&mut clif_func, fn_builder_ctx);
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    let args_ptr = builder.block_params(entry_block)[0];
    let ret_ptr = builder.block_params(entry_block)[1];

    // Day 1-2 only: every function must be a single block with
    // at most one `ConstI32` followed by `Return`. Anything more
    // returns UnimplementedInstruction.
    if func.blocks.is_empty() {
        return Err(JitError::UnimplementedInstruction {
            instruction: "empty function".to_string(),
            function: func.name.to_string(),
        });
    }
    if func.blocks.len() > 1 {
        return Err(JitError::UnimplementedInstruction {
            instruction: format!("{} blocks (multi-block not yet)", func.blocks.len()),
            function: func.name.to_string(),
        });
    }

    let block0: &BasicBlock = &func.blocks[0];

    compile_block(
        &mut builder,
        block0,
        args_ptr,
        ret_ptr,
        ret_type,
        func.name.as_ref(),
    )?;

    builder.seal_all_blocks();
    builder.finalize();

    let mut ctx = module.make_context();
    ctx.func = clif_func;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| JitError::Cranelift {
            msg: format!("define body: {e:?}"),
        })?;
    Ok(())
}

/// Emits Cranelift instructions for a single IR block.
fn compile_block(
    builder: &mut FunctionBuilder,
    block: &BasicBlock,
    _args_ptr: Value,
    ret_ptr: Value,
    ret_type: &IrType,
    func_name: &str,
) -> Result<(), JitError> {
    // For Day 1-2: only ConstI32 is allowed in the body, and the
    // return type must be I32. Other combos return UnimplementedInstruction.
    if !matches!(ret_type, IrType::I32) {
        return Err(JitError::UnimplementedInstruction {
            instruction: format!("return type {ret_type}"),
            function: func_name.to_string(),
        });
    }

    for (_defined, inst) in &block.instructions {
        match inst {
            ir::Instruction::ConstI32(n) => {
                let v = builder.ins().iconst(I32, *n as i64);
                builder.ins().store(MemFlags::trusted(), v, ret_ptr, 0);
            }
            _ => {
                return Err(JitError::UnimplementedInstruction {
                    instruction: format!("{inst:?}"),
                    function: func_name.to_string(),
                });
            }
        }
    }

    match &block.terminator {
        Terminator::Return(_) => {
            let zero = builder.ins().iconst(I32, 0);
            builder.ins().return_(&[zero]);
            Ok(())
        }
        _ => Err(JitError::UnimplementedInstruction {
            instruction: format!("{:?}", block.terminator),
            function: func_name.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ast::SmolStr;
    use ir::{BasicBlock, BlockId, IrFunction, IrModule};

    /// Build a minimal `fn main() -> i32` that returns 42.
    fn make_main_returning(n: i32) -> IrModule {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let v_ret = func.alloc_value();
        let mut entry = BasicBlock::new(BlockId(0));
        entry
            .instructions
            .push((Some(v_ret), ir::Instruction::ConstI32(n)));
        entry.terminator = Terminator::Return(v_ret);
        func.blocks.push(entry);
        IrModule {
            imports: vec![],
            decls: vec![ir::IrDecl::Function(func)],
        }
    }

    #[test]
    fn jit_compiles_empty_module_errors_no_main() {
        let module = IrModule::new();
        let result = compile_ir(&module);
        assert!(matches!(result, Err(JitError::NoMain)));
    }

    #[test]
    fn jit_compiles_main_returning_42() {
        let module = make_main_returning(42);
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, 42);
    }

    #[test]
    fn jit_compiles_main_returning_zero() {
        let module = make_main_returning(0);
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, 0);
    }

    #[test]
    fn jit_compiles_main_returning_negative() {
        let module = make_main_returning(-1);
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, -1);
    }

    #[test]
    fn jit_errors_on_unimplemented_instruction() {
        // A function with an Add instruction (not yet wired up in
        // Day 1-2 skeleton) should return UnimplementedInstruction.
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let v0 = func.alloc_value();
        let v1 = func.alloc_value();
        let v2 = func.alloc_value();
        let mut entry = BasicBlock::new(BlockId(0));
        entry
            .instructions
            .push((Some(v0), ir::Instruction::ConstI32(1)));
        entry
            .instructions
            .push((Some(v1), ir::Instruction::ConstI32(2)));
        entry
            .instructions
            .push((Some(v2), ir::Instruction::Add(v0, v1)));
        entry.terminator = Terminator::Return(v2);
        func.blocks.push(entry);
        let module = IrModule {
            imports: vec![],
            decls: vec![ir::IrDecl::Function(func)],
        };
        let result = compile_ir(&module);
        assert!(matches!(
            result,
            Err(JitError::UnimplementedInstruction { .. })
        ));
    }

    #[test]
    fn jit_returns_error_when_no_main_present() {
        // A module with a function named `helper` but no `main`.
        let module = make_main_returning(0);
        let mut renamed = IrModule::new();
        // rename main -> helper
        if let Some(ir::IrDecl::Function(f)) = module.decls.into_iter().next() {
            let mut f = f;
            f.name = SmolStr::new("helper");
            renamed.decls.push(ir::IrDecl::Function(f));
        }
        let result = compile_ir(&renamed);
        assert!(matches!(result, Err(JitError::NoMain)));
    }
}
