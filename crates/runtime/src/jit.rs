//! Cranelift JIT compiler for pipe-lang IR.
//!
//! This module translates an [`IrModule`](ir::IrModule) into native
//! code using Cranelift. The translation is one-to-one for simple
//! cases (constants, arithmetic, calls) and falls back to runtime
//! builtins for higher-level operations (arrays, records, effects).
//!
//! # Status
//!
//! Track B Phase 2: primitive operations plus multi-block control
//! flow with SSA block parameters.
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

use std::collections::{HashMap, HashSet};

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::{Ieee32, Ieee64};
use cranelift_codegen::ir::types::{self, I32};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, Endianness, FuncRef, Function, GlobalValue, InstBuilder, MemFlags,
    SigRef, StackSlotData, StackSlotKind, TrapCode, Type, UserFuncName, Value,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch as ClifSwitch};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, Linkage, Module};

use ir::{BasicBlock, BlockId, IrFunction, IrModule, IrType, Terminator, ValueId};

/// A pointer to a JIT-compiled function with the pipe-lang ABI.
pub type PipeFuncPtr = unsafe extern "C" fn(args: *const u8, ret: *mut u8) -> i32;

const UNREACHABLE_TRAP: TrapCode = TrapCode::unwrap_user(1);

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
            msg: format!("{e}"),
        }
    }
}

/// A compiled IR module, ready to execute.
pub struct CompiledModule {
    /// Kept around so its `JITModule` allocations stay live.
    _module: Box<JITModule>,
    /// The function pointer for `main`.
    main_ptr: PipeFuncPtr,
    /// The IR return type for `main`, used to decode primitive results.
    main_return_type: IrType,
}

// SAFETY: a JITModule is single-threaded in use but safe to share
// across threads when no compilation is in progress; PipeFuncPtr is
// a plain function pointer.
unsafe impl Send for CompiledModule {}
unsafe impl Sync for CompiledModule {}

impl CompiledModule {
    /// Calls the module's `main` function with no arguments and
    /// returns its result as an `i32`.
    ///
    /// Only types that fit losslessly in `i32` are supported:
    /// `I8`, `I16`, `I32`, `U8`, `U16`, `U32`, `Bool`, `Unit`.
    /// Wider types (`I64`, `U64`, `Usize`, `F32`, `F64`) return
    /// an error; use [`call_main_raw`] and decode manually.
    ///
    /// # Errors
    ///
    /// Returns [`JitError`] if the main function panics or its
    /// return type cannot be losslessly decoded as `i32`.
    pub fn call_main(&self) -> Result<i32, JitError> {
        let mut ret_buf = [0u8; 16];
        let code = unsafe { (self.main_ptr)(std::ptr::null(), ret_buf.as_mut_ptr()) };
        if code != 0 {
            return Err(JitError::Cranelift {
                msg: format!("main returned error code {code}"),
            });
        }
        decode_main_i32(&self.main_return_type, &ret_buf)
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
    let mut name_to_func: HashMap<String, (cranelift_module::FuncId, IrType)> = HashMap::new();
    for func in ir_module.functions() {
        let name = func.name.as_str().to_string();
        let sig = make_signature(&module);
        let id = module
            .declare_function(&name, Linkage::Export, &sig)
            .map_err(JitError::from)?;
        let ret_type = func.return_type.clone();
        name_to_func.insert(name.clone(), (id, ret_type.clone()));
        func_ids.push((name, id, ret_type));
    }

    // Scan all functions for ConstStr strings and declare data objects
    // for each unique string constant.
    let mut unique_strings: Vec<String> = Vec::new();
    let mut seen_strings: HashSet<String> = HashSet::new();
    for func in ir_module.functions() {
        for block in &func.blocks {
            for (_, inst) in &block.instructions {
                if let ir::Instruction::ConstStr(s) = inst {
                    let s_str = s.to_string();
                    if seen_strings.insert(s_str.clone()) {
                        unique_strings.push(s_str);
                    }
                }
            }
        }
    }
    let mut string_data_ids: HashMap<String, DataId> = HashMap::new();
    for s in &unique_strings {
        let data_name = format!("__str_{}", s);
        let data_id = module.declare_data(&data_name, Linkage::Local, false, false)?;
        let mut data_desc = DataDescription::new();
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;
        let mut data = Vec::with_capacity(4 + bytes.len());
        data.extend_from_slice(&len.to_ne_bytes());
        data.extend_from_slice(bytes);
        data_desc.define(data.into_boxed_slice());
        module.define_data(data_id, &data_desc)?;
        string_data_ids.insert(s.clone(), data_id);
    }

    // Declare a data object containing the __pipe_println function pointer,
    // so compiled code can load and call it via call_indirect (Linkage::Import
    // + dlsym fails because test binaries don't export symbols).
    let println_ptr = __pipe_println as *const ();
    let println_ptr_data_id =
        module.declare_data("__pipe_println_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (println_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(println_ptr_data_id, &data_desc)?;
    }

    // Declare a data object for the pipe_rt_str_concat function pointer.
    let str_concat_ptr = pipe_rt_str_concat as *const ();
    let str_concat_ptr_data_id =
        module.declare_data("__pipe_str_concat_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (str_concat_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(str_concat_ptr_data_id, &data_desc)?;
    }

    // Compile each function. The function bodies are populated here;
    // finalize_definitions happens once at the end.
    for (name, func_id, ret_type) in &func_ids {
        let Some(func) = ir_module.function(name) else {
            return Err(JitError::Cranelift {
                msg: format!("function disappeared after declaration: {name}"),
            });
        };
        let mut params = FunctionBodyParams {
            module: &mut module,
            fn_builder_ctx: &mut fn_builder_ctx,
            name_to_func: &name_to_func,
            string_data_ids: &string_data_ids,
            println_ptr_data_id,
            str_concat_ptr_data_id,
        };
        compile_function_body(&mut params, func, *func_id, ret_type)?;
    }

    module.finalize_definitions().map_err(JitError::from)?;

    let (main_id, main_return_type) = func_ids
        .iter()
        .find(|(n, _, _)| n == "main")
        .map(|(_, id, ret)| (*id, ret.clone()))
        .ok_or(JitError::NoMain)?;
    let code_ptr = module.get_finalized_function(main_id);
    let main_ptr: PipeFuncPtr = unsafe { std::mem::transmute(code_ptr) };

    Ok(CompiledModule {
        _module: Box::new(module),
        main_ptr,
        main_return_type,
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

/// Shared context for compiling a single function body into Cranelift IR.
///
/// Bundles commonly-threaded references to eliminate
/// `too_many_arguments` warnings throughout the compilation pipeline.
///
/// Does NOT own the `FunctionBuilder` — that is passed separately to avoid
/// self-referential borrow issues with `finalize()` taking ownership.
struct BlockContext<'a> {
    value_types: &'a HashMap<ValueId, IrType>,
    func_name: &'a str,
    callee_funcs: &'a HashMap<String, FuncRef>,
    fn_return_types: &'a HashMap<String, IrType>,
    string_globals: &'a HashMap<String, GlobalValue>,
    println_fn_ptr: Value,
    println_sig: SigRef,
    str_concat_fn_ptr: Value,
    str_concat_sig: SigRef,
    blocks: &'a HashMap<BlockId, Block>,
    ret_ptr: Value,
    ret_type: &'a IrType,
}

/// Module-scoped parameters for compiling a single function body.
struct FunctionBodyParams<'a> {
    module: &'a mut JITModule,
    fn_builder_ctx: &'a mut FunctionBuilderContext,
    name_to_func: &'a HashMap<String, (cranelift_module::FuncId, IrType)>,
    string_data_ids: &'a HashMap<String, DataId>,
    println_ptr_data_id: DataId,
    str_concat_ptr_data_id: DataId,
}

/// Compiles the body of one IR function into the module's slot
/// for `func_id`.
fn compile_function_body(
    params: &mut FunctionBodyParams,
    func: &IrFunction,
    func_id: cranelift_module::FuncId,
    ret_type: &IrType,
) -> Result<(), JitError> {
    let sig = make_signature(params.module);
    let user_name = UserFuncName::user(0, func_id.as_u32());
    let mut clif_func = Function::with_name_signature(user_name, sig);

    if func.blocks.is_empty() {
        return Err(JitError::UnimplementedInstruction {
            instruction: "empty function".to_string(),
            function: func.name.to_string(),
        });
    }

    // Populate fn_return_types for every declared function so that
    // type inference and codegen can resolve call return types.
    let mut fn_return_types: HashMap<String, IrType> = HashMap::new();
    for (name, (_, ret_ty)) in params.name_to_func.iter() {
        fn_return_types.insert(name.clone(), ret_ty.clone());
    }

    // The ABI entry block unpacks function parameters, then jumps into
    // the first IR block. All IR blocks are declared before emission so
    // forward edges and block parameters are available immediately.
    let mut builder = FunctionBuilder::new(&mut clif_func, params.fn_builder_ctx);
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    let args_ptr = builder.block_params(entry_block)[0];
    let ret_ptr = builder.block_params(entry_block)[1];
    let value_types = infer_value_types(func, &fn_return_types)?;
    let mut values = HashMap::new();

    load_function_params(
        &mut builder,
        args_ptr,
        &func.params,
        &mut values,
        func.name.as_ref(),
    )?;

    // Pre-import all callees referenced by CallNamed in this function.
    let mut callee_funcs: HashMap<String, FuncRef> = HashMap::new();
    for ir_block in &func.blocks {
        for (_, inst) in &ir_block.instructions {
            if let ir::Instruction::CallNamed(data) = inst {
                let name_str = data.name.to_string();
                if callee_funcs.contains_key(&name_str) {
                    continue;
                }
                let (callee_id, _) =
                    params.name_to_func.get(name_str.as_str()).ok_or_else(|| {
                        JitError::UnimplementedInstruction {
                            instruction: format!("CallNamed to unknown function {name_str}"),
                            function: func.name.to_string(),
                        }
                    })?;
                let func_ref = {
                    let f: &mut Function = builder.func;
                    params.module.declare_func_in_func(*callee_id, f)
                };
                callee_funcs.insert(name_str, func_ref);
            }
        }
    }

    // Pre-import all string data objects referenced by ConstStr in this function.
    let mut string_globals: HashMap<String, GlobalValue> = HashMap::new();
    for ir_block in &func.blocks {
        for (_, inst) in &ir_block.instructions {
            if let ir::Instruction::ConstStr(s) = inst {
                let s_str = s.to_string();
                if string_globals.contains_key(&s_str) {
                    continue;
                }
                let data_id = params.string_data_ids.get(&s_str).ok_or_else(|| {
                    JitError::UnimplementedInstruction {
                        instruction: format!("ConstStr: no DataId for {s_str}"),
                        function: func.name.to_string(),
                    }
                })?;
                let gv = {
                    let f: &mut Function = builder.func;
                    params.module.declare_data_in_func(*data_id, f)
                };
                string_globals.insert(s_str, gv);
            }
        }
    }

    // Import the __pipe_println function pointer from a data object (we
    // cannot use Linkage::Import because dlsym can't resolve symbols in
    // test binaries). Load the pointer and create a SigRef for it.
    let println_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.println_ptr_data_id, f)
    };
    let println_fn_ptr_addr = builder.ins().global_value(types::I64, println_fn_ptr_gv);
    let println_fn_ptr =
        builder
            .ins()
            .load(types::I64, MemFlags::trusted(), println_fn_ptr_addr, 0);
    let println_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    // Import the pipe_rt_str_concat function pointer from a data object.
    let str_concat_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.str_concat_ptr_data_id, f)
    };
    let str_concat_fn_ptr_addr = builder.ins().global_value(types::I64, str_concat_fn_ptr_gv);
    let str_concat_fn_ptr =
        builder
            .ins()
            .load(types::I64, MemFlags::trusted(), str_concat_fn_ptr_addr, 0);
    let str_concat_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    let blocks = declare_blocks(&mut builder, func, &mut values)?;
    let first_ir_block = &func.blocks[0];
    if !first_ir_block.params.is_empty() {
        return Err(JitError::UnimplementedInstruction {
            instruction: "entry IR block has parameters".to_string(),
            function: func.name.to_string(),
        });
    }
    let first_block = lookup_block(&blocks, first_ir_block.id, func.name.as_ref())?;
    builder.ins().jump(first_block, &[]);

    let ctx = BlockContext {
        value_types: &value_types,
        func_name: func.name.as_ref(),
        callee_funcs: &callee_funcs,
        fn_return_types: &fn_return_types,
        string_globals: &string_globals,
        println_fn_ptr,
        println_sig,
        str_concat_fn_ptr,
        str_concat_sig,
        blocks: &blocks,
        ret_ptr,
        ret_type,
    };

    for block in &func.blocks {
        let clif_block = lookup_block(&blocks, block.id, func.name.as_ref())?;
        builder.switch_to_block(clif_block);
        compile_block(&mut builder, &ctx, block, &mut values)?;
    }

    builder.seal_all_blocks();
    builder.finalize();

    let mut ctx = params.module.make_context();
    ctx.func = clif_func;
    params
        .module
        .define_function(func_id, &mut ctx)
        .map_err(|e| JitError::Cranelift {
            msg: format!("define body: {e:?}"),
        })?;
    Ok(())
}

fn declare_blocks(
    builder: &mut FunctionBuilder,
    func: &IrFunction,
    values: &mut HashMap<ValueId, Value>,
) -> Result<HashMap<BlockId, Block>, JitError> {
    let mut blocks = HashMap::new();
    for block in &func.blocks {
        let clif_block = builder.create_block();
        for (_, ty) in &block.params {
            let clif_type = storage_type(ty, func.name.as_ref())?;
            builder.append_block_param(clif_block, clif_type);
        }
        if blocks.insert(block.id, clif_block).is_some() {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("duplicate block {}", block.id),
                function: func.name.to_string(),
            });
        }
    }

    for block in &func.blocks {
        let clif_block = lookup_block(&blocks, block.id, func.name.as_ref())?;
        for ((value_id, _), value) in block.params.iter().zip(builder.block_params(clif_block)) {
            values.insert(*value_id, *value);
        }
    }
    Ok(blocks)
}

/// Emits Cranelift instructions for a single IR block.
fn compile_block(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    block: &BasicBlock,
    values: &mut HashMap<ValueId, Value>,
) -> Result<(), JitError> {
    for (defined, inst) in &block.instructions {
        let emitted = compile_instruction(builder, ctx, inst, values)?;
        if let Some(value_id) = defined {
            let value = emitted.ok_or_else(|| JitError::UnimplementedInstruction {
                instruction: format!("value-less instruction assigned to {value_id}"),
                function: ctx.func_name.to_string(),
            })?;
            values.insert(*value_id, value);
        }
    }

    match &block.terminator {
        Terminator::Return(value_id) => {
            let value = lookup_value(values, *value_id, ctx.func_name)?;
            store_return_value(builder, ctx.ret_ptr, ctx.ret_type, value, ctx.func_name)?;
            let zero = builder.ins().iconst(I32, 0);
            builder.ins().return_(&[zero]);
            Ok(())
        }
        Terminator::Jump { target, args } => {
            let target = lookup_block(ctx.blocks, *target, ctx.func_name)?;
            let args = lookup_block_args(values, args, ctx.func_name)?;
            builder.ins().jump(target, &args);
            Ok(())
        }
        Terminator::Branch {
            condition,
            then_block,
            then_args,
            else_block,
            else_args,
        } => {
            let condition_type = lookup_type(ctx.value_types, *condition, ctx.func_name)?;
            if !matches!(condition_type, IrType::Bool) {
                return Err(unsupported_type(ctx.func_name, condition_type));
            }
            let condition = lookup_value(values, *condition, ctx.func_name)?;
            let then_block = lookup_block(ctx.blocks, *then_block, ctx.func_name)?;
            let then_args = lookup_block_args(values, then_args, ctx.func_name)?;
            let else_block = lookup_block(ctx.blocks, *else_block, ctx.func_name)?;
            let else_args = lookup_block_args(values, else_args, ctx.func_name)?;
            builder
                .ins()
                .brif(condition, then_block, &then_args, else_block, &else_args);
            Ok(())
        }
        Terminator::Switch {
            discriminant,
            arms,
            default,
        } => compile_switch(builder, ctx, values, *discriminant, arms, default.as_ref()),
        Terminator::Unreachable => {
            builder.ins().trap(UNREACHABLE_TRAP);
            Ok(())
        }
        _ => Err(JitError::UnimplementedInstruction {
            instruction: format!("{:?}", block.terminator),
            function: ctx.func_name.to_string(),
        }),
    }
}

fn compile_switch(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    values: &HashMap<ValueId, Value>,
    discriminant: ValueId,
    arms: &[(u32, BlockId, Vec<ValueId>)],
    default: Option<&(BlockId, Vec<ValueId>)>,
) -> Result<(), JitError> {
    let discriminant_type = lookup_type(ctx.value_types, discriminant, ctx.func_name)?;
    let max_discriminant = switch_max_discriminant(discriminant_type)
        .ok_or_else(|| unsupported_type(ctx.func_name, discriminant_type))?;
    validate_switch_arms(arms, max_discriminant, ctx.func_name)?;
    let discriminant = lookup_value(values, discriminant, ctx.func_name)?;

    let has_edge_args = arms.iter().any(|(_, _, args)| !args.is_empty())
        || default.is_some_and(|(_, args)| !args.is_empty());
    if has_edge_args {
        return compile_switch_with_args(builder, ctx, values, discriminant, arms, default);
    }

    let mut switch = ClifSwitch::new();
    for (case, target, _) in arms {
        switch.set_entry(
            u128::from(*case),
            lookup_block(ctx.blocks, *target, ctx.func_name)?,
        );
    }

    let (fallback, trap_fallback) = match default {
        Some((target, _)) => (lookup_block(ctx.blocks, *target, ctx.func_name)?, false),
        None => (builder.create_block(), true),
    };
    switch.emit(builder, discriminant, fallback);
    if trap_fallback {
        builder.switch_to_block(fallback);
        builder.ins().trap(UNREACHABLE_TRAP);
    }
    Ok(())
}

fn compile_switch_with_args(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    values: &HashMap<ValueId, Value>,
    discriminant: Value,
    arms: &[(u32, BlockId, Vec<ValueId>)],
    default: Option<&(BlockId, Vec<ValueId>)>,
) -> Result<(), JitError> {
    for (case, target, args) in arms {
        let target = lookup_block(ctx.blocks, *target, ctx.func_name)?;
        let args = lookup_block_args(values, args, ctx.func_name)?;
        let next = builder.create_block();
        let matches = builder
            .ins()
            .icmp_imm(IntCC::Equal, discriminant, i64::from(*case));
        builder.ins().brif(matches, target, &args, next, &[]);
        builder.switch_to_block(next);
    }

    match default {
        Some((target, args)) => {
            let target = lookup_block(ctx.blocks, *target, ctx.func_name)?;
            let args = lookup_block_args(values, args, ctx.func_name)?;
            builder.ins().jump(target, &args);
        }
        None => {
            builder.ins().trap(UNREACHABLE_TRAP);
        }
    }
    Ok(())
}

fn validate_switch_arms(
    arms: &[(u32, BlockId, Vec<ValueId>)],
    max_discriminant: u32,
    func_name: &str,
) -> Result<(), JitError> {
    let mut seen = HashSet::new();
    for (case, _, _) in arms {
        if *case > max_discriminant {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("switch case {case} exceeds discriminant range"),
                function: func_name.to_string(),
            });
        }
        if !seen.insert(*case) {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("duplicate switch case {case}"),
                function: func_name.to_string(),
            });
        }
    }
    Ok(())
}

fn switch_max_discriminant(ty: &IrType) -> Option<u32> {
    match ty {
        IrType::Bool => Some(1),
        IrType::I8 | IrType::U8 => Some(u32::from(u8::MAX)),
        IrType::I16 | IrType::U16 => Some(u32::from(u16::MAX)),
        IrType::I32 | IrType::I64 | IrType::U32 | IrType::U64 | IrType::Usize => Some(u32::MAX),
        _ => None,
    }
}

fn lookup_block(
    blocks: &HashMap<BlockId, Block>,
    block_id: BlockId,
    func_name: &str,
) -> Result<Block, JitError> {
    blocks
        .get(&block_id)
        .copied()
        .ok_or_else(|| JitError::UnimplementedInstruction {
            instruction: format!("missing block {block_id}"),
            function: func_name.to_string(),
        })
}

fn lookup_block_args(
    values: &HashMap<ValueId, Value>,
    args: &[ValueId],
    func_name: &str,
) -> Result<Vec<BlockArg>, JitError> {
    args.iter()
        .map(|value_id| lookup_value(values, *value_id, func_name).map(BlockArg::from))
        .collect()
}

fn compile_instruction(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    inst: &ir::Instruction,
    values: &HashMap<ValueId, Value>,
) -> Result<Option<Value>, JitError> {
    let value = match inst {
        ir::Instruction::ConstI8(n) => builder.ins().iconst(types::I8, i64::from(*n)),
        ir::Instruction::ConstI16(n) => builder.ins().iconst(types::I16, i64::from(*n)),
        ir::Instruction::ConstI32(n) => builder.ins().iconst(I32, i64::from(*n)),
        ir::Instruction::ConstI64(n) => builder.ins().iconst(types::I64, *n),
        ir::Instruction::ConstU8(n) => builder.ins().iconst(types::I8, i64::from(*n)),
        ir::Instruction::ConstU16(n) => builder.ins().iconst(types::I16, i64::from(*n)),
        ir::Instruction::ConstU32(n) => builder.ins().iconst(I32, i64::from(*n)),
        ir::Instruction::ConstU64(n) => builder.ins().iconst(types::I64, *n as i64),
        ir::Instruction::ConstUsize(n) => builder.ins().iconst(types::I64, *n as i64),
        ir::Instruction::ConstF32(n) => builder.ins().f32const(Ieee32::with_float(*n)),
        ir::Instruction::ConstF64(n) => builder.ins().f64const(Ieee64::with_float(*n)),
        ir::Instruction::ConstBool(v) => builder.ins().iconst(types::I8, i64::from(u8::from(*v))),
        ir::Instruction::ConstUnit => builder.ins().iconst(I32, 0),
        ir::Instruction::ConstStr(s) => {
            let gv = ctx.string_globals.get(s.as_str()).ok_or_else(|| {
                JitError::UnimplementedInstruction {
                    instruction: format!("ConstStr: no GlobalValue for {s}"),
                    function: ctx.func_name.to_string(),
                }
            })?;
            builder.ins().global_value(types::I64, *gv)
        }

        ir::Instruction::Add(left, right) => {
            compile_numeric_binary(builder, ctx, values, *left, *right, |b, t, l, r| {
                if is_float(t) {
                    Ok(b.ins().fadd(l, r))
                } else {
                    Ok(b.ins().iadd(l, r))
                }
            })?
        }
        ir::Instruction::Sub(left, right) => {
            compile_numeric_binary(builder, ctx, values, *left, *right, |b, t, l, r| {
                if is_float(t) {
                    Ok(b.ins().fsub(l, r))
                } else {
                    Ok(b.ins().isub(l, r))
                }
            })?
        }
        ir::Instruction::Mul(left, right) => {
            compile_numeric_binary(builder, ctx, values, *left, *right, |b, t, l, r| {
                if is_float(t) {
                    Ok(b.ins().fmul(l, r))
                } else {
                    Ok(b.ins().imul(l, r))
                }
            })?
        }
        ir::Instruction::Div(left, right) => {
            compile_numeric_binary(builder, ctx, values, *left, *right, |b, t, l, r| match t {
                IrType::F32 | IrType::F64 => Ok(b.ins().fdiv(l, r)),
                IrType::U8 | IrType::U16 | IrType::U32 | IrType::U64 | IrType::Usize => {
                    Ok(b.ins().udiv(l, r))
                }
                _ => Ok(b.ins().sdiv(l, r)),
            })?
        }
        ir::Instruction::Rem(left, right) => {
            compile_numeric_binary(builder, ctx, values, *left, *right, |b, t, l, r| match t {
                IrType::F32 | IrType::F64 => {
                    let quotient = b.ins().fdiv(l, r);
                    let truncated = b.ins().trunc(quotient);
                    let product = b.ins().fmul(truncated, r);
                    Ok(b.ins().fsub(l, product))
                }
                IrType::U8 | IrType::U16 | IrType::U32 | IrType::U64 | IrType::Usize => {
                    Ok(b.ins().urem(l, r))
                }
                _ => Ok(b.ins().srem(l, r)),
            })?
        }
        ir::Instruction::Neg(value_id) => {
            let ty = lookup_type(ctx.value_types, *value_id, ctx.func_name)?;
            let value = lookup_value(values, *value_id, ctx.func_name)?;
            if is_float(ty) {
                builder.ins().fneg(value)
            } else if is_integer(ty) {
                builder.ins().ineg(value)
            } else {
                return Err(unsupported_type(ctx.func_name, ty));
            }
        }

        ir::Instruction::Eq(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Eq)?
        }
        ir::Instruction::Ne(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Ne)?
        }
        ir::Instruction::Lt(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Lt)?
        }
        ir::Instruction::Le(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Le)?
        }
        ir::Instruction::Gt(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Gt)?
        }
        ir::Instruction::Ge(left, right) => {
            compile_comparison(builder, ctx, values, *left, *right, CompareOp::Ge)?
        }

        ir::Instruction::And(left, right) => {
            compile_bool_binary(builder, ctx, values, *left, *right, |b, l, r| {
                b.ins().band(l, r)
            })?
        }
        ir::Instruction::Or(left, right) => {
            compile_bool_binary(builder, ctx, values, *left, *right, |b, l, r| {
                b.ins().bor(l, r)
            })?
        }
        ir::Instruction::Not(value_id) => {
            let ty = lookup_type(ctx.value_types, *value_id, ctx.func_name)?;
            if !matches!(ty, IrType::Bool) {
                return Err(unsupported_type(ctx.func_name, ty));
            }
            builder.ins().icmp_imm(
                IntCC::Equal,
                lookup_value(values, *value_id, ctx.func_name)?,
                0,
            )
        }

        ir::Instruction::CallNamed(data) => compile_call_named(builder, ctx, data, values)?,

        ir::Instruction::Println(value_id) => {
            compile_println(builder, ctx, *value_id, values)?;
            return Ok(None);
        }

        ir::Instruction::StrConcat { parts } => compile_str_concat(builder, ctx, parts, values)?,

        _ => {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("{inst:?}"),
                function: ctx.func_name.to_string(),
            });
        }
    };
    Ok(Some(value))
}

fn compile_call_named(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    data: &ir::CallNamedData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let callee_name = data.name.as_str();
    let func_ref = ctx.callee_funcs.get(callee_name).copied().ok_or_else(|| {
        JitError::UnimplementedInstruction {
            instruction: format!("CallNamed: no FuncRef for {callee_name}"),
            function: ctx.func_name.to_string(),
        }
    })?;
    let ret_type =
        ctx.fn_return_types
            .get(callee_name)
            .ok_or_else(|| JitError::UnimplementedInstruction {
                instruction: format!("CallNamed: no return type for {callee_name}"),
                function: ctx.func_name.to_string(),
            })?;

    let total_size: u32 = data
        .args
        .iter()
        .map(|arg_id| {
            let ty = lookup_type(ctx.value_types, *arg_id, ctx.func_name)?;
            storage_size(ty, ctx.func_name)
        })
        .sum::<Result<i32, _>>()?
        .max(1) as u32;

    let arg_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        total_size,
        0,
    ));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    let mut offset: i32 = 0;
    for arg_id in &data.args {
        let arg_type = lookup_type(ctx.value_types, *arg_id, ctx.func_name)?;
        if !matches!(arg_type, IrType::Unit) {
            let arg_val = lookup_value(values, *arg_id, ctx.func_name)?;
            builder
                .ins()
                .store(MemFlags::trusted(), arg_val, args_buf, offset);
        }
        offset += storage_size(arg_type, ctx.func_name)?;
    }

    let ret_size = storage_size(ret_type, ctx.func_name)?.max(1) as u32;
    let ret_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        ret_size,
        0,
    ));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder.ins().call(func_ref, &[args_buf, ret_buf]);

    let result = load_primitive_value(builder, ret_buf, 0, ret_type, ctx.func_name)?;
    Ok(result)
}

fn compile_println(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    value_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<(), JitError> {
    let ty = lookup_type(ctx.value_types, value_id, ctx.func_name)?;
    let val = lookup_value(values, value_id, ctx.func_name)?;
    let tag = ir_type_tag(ty).ok_or_else(|| unsupported_type(ctx.func_name, ty))?;

    let arg_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 12, 0));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    let widened = widen_to_i64(builder, val, ty, ctx.func_name)?;
    builder
        .ins()
        .store(MemFlags::trusted(), widened, args_buf, 0);

    let tag_val = builder.ins().iconst(I32, i64::from(tag));
    builder
        .ins()
        .store(MemFlags::trusted(), tag_val, args_buf, 8);

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder
        .ins()
        .call_indirect(ctx.println_sig, ctx.println_fn_ptr, &[args_buf, ret_buf]);
    Ok(())
}

/// Compile a `StrConcat` instruction.
fn compile_str_concat(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    parts: &[ValueId],
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let count = parts.len();
    let buf_size = 4u32 + count as u32 * 12u32;

    let arg_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        buf_size,
        0,
    ));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    let count_val = builder.ins().iconst(I32, count as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), count_val, args_buf, 0);

    for (i, part_id) in parts.iter().enumerate() {
        let ty = lookup_type(ctx.value_types, *part_id, ctx.func_name)?;
        let val = lookup_value(values, *part_id, ctx.func_name)?;
        let widened = widen_to_i64(builder, val, ty, ctx.func_name)?;
        let offset = 4i32 + i as i32 * 12;
        builder
            .ins()
            .store(MemFlags::trusted(), widened, args_buf, offset);
        let tag = ir_type_tag(ty).ok_or_else(|| unsupported_type(ctx.func_name, ty))?;
        let tag_val = builder.ins().iconst(I32, i64::from(tag));
        builder
            .ins()
            .store(MemFlags::trusted(), tag_val, args_buf, offset + 8);
    }

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder.ins().call_indirect(
        ctx.str_concat_sig,
        ctx.str_concat_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
}

/// Widen or bitcast a Cranelift `Value` to `I64` for the Println
/// args buffer. Signed types are sign-extended; unsigned types and
/// booleans are zero-extended; floats are bitcast to their integer
/// counterpart then zero-extended.
fn widen_to_i64(
    builder: &mut FunctionBuilder,
    val: Value,
    ty: &IrType,
    func_name: &str,
) -> Result<Value, JitError> {
    match ty {
        IrType::I8 => Ok(builder.ins().sextend(types::I64, val)),
        IrType::I16 => Ok(builder.ins().sextend(types::I64, val)),
        IrType::I32 => Ok(builder.ins().sextend(types::I64, val)),
        IrType::I64 | IrType::Usize => Ok(val),
        IrType::U8 => Ok(builder.ins().uextend(types::I64, val)),
        IrType::U16 => Ok(builder.ins().uextend(types::I64, val)),
        IrType::U32 => Ok(builder.ins().uextend(types::I64, val)),
        IrType::U64 => Ok(val),
        IrType::F32 => {
            let mf = MemFlags::new().with_endianness(Endianness::Little);
            let as_i32 = builder.ins().bitcast(types::I32, mf, val);
            Ok(builder.ins().uextend(types::I64, as_i32))
        }
        IrType::F64 => {
            let mf = MemFlags::new().with_endianness(Endianness::Little);
            Ok(builder.ins().bitcast(types::I64, mf, val))
        }
        IrType::Bool => Ok(builder.ins().uextend(types::I64, val)),
        IrType::Str => Ok(val),
        _ => Err(unsupported_type(func_name, ty)),
    }
}

fn compile_numeric_binary(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    values: &HashMap<ValueId, Value>,
    left: ValueId,
    right: ValueId,
    emit: impl FnOnce(&mut FunctionBuilder, &IrType, Value, Value) -> Result<Value, JitError>,
) -> Result<Value, JitError> {
    let ty = lookup_type(ctx.value_types, left, ctx.func_name)?;
    if !ty.is_numeric() {
        return Err(unsupported_type(ctx.func_name, ty));
    }
    let right_ty = lookup_type(ctx.value_types, right, ctx.func_name)?;
    if !right_ty.is_numeric() {
        return Err(unsupported_type(ctx.func_name, right_ty));
    }
    let left_value = lookup_value(values, left, ctx.func_name)?;
    let right_value = lookup_value(values, right, ctx.func_name)?;
    emit(builder, ty, left_value, right_value)
}

fn compile_bool_binary(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    values: &HashMap<ValueId, Value>,
    left: ValueId,
    right: ValueId,
    emit: impl FnOnce(&mut FunctionBuilder, Value, Value) -> Value,
) -> Result<Value, JitError> {
    let ty = lookup_type(ctx.value_types, left, ctx.func_name)?;
    if !matches!(ty, IrType::Bool) {
        return Err(unsupported_type(ctx.func_name, ty));
    }
    let right_ty = lookup_type(ctx.value_types, right, ctx.func_name)?;
    if !matches!(right_ty, IrType::Bool) {
        return Err(unsupported_type(ctx.func_name, right_ty));
    }
    let left_value = lookup_value(values, left, ctx.func_name)?;
    let right_value = lookup_value(values, right, ctx.func_name)?;
    Ok(emit(builder, left_value, right_value))
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

fn compile_comparison(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    values: &HashMap<ValueId, Value>,
    left: ValueId,
    right: ValueId,
    op: CompareOp,
) -> Result<Value, JitError> {
    let ty = lookup_type(ctx.value_types, left, ctx.func_name)?;
    let left_value = lookup_value(values, left, ctx.func_name)?;
    let right_value = lookup_value(values, right, ctx.func_name)?;
    match ty {
        IrType::F32 | IrType::F64 => {
            Ok(builder
                .ins()
                .fcmp(float_compare_code(op), left_value, right_value))
        }
        IrType::Bool => match op {
            CompareOp::Eq => Ok(builder.ins().icmp(IntCC::Equal, left_value, right_value)),
            CompareOp::Ne => Ok(builder.ins().icmp(IntCC::NotEqual, left_value, right_value)),
            _ => Err(unsupported_type(ctx.func_name, ty)),
        },
        _ if is_integer(ty) => {
            Ok(builder
                .ins()
                .icmp(int_compare_code(op, ty), left_value, right_value))
        }
        _ => Err(unsupported_type(ctx.func_name, ty)),
    }
}

fn int_compare_code(op: CompareOp, ty: &IrType) -> IntCC {
    match (op, is_unsigned_integer(ty)) {
        (CompareOp::Eq, _) => IntCC::Equal,
        (CompareOp::Ne, _) => IntCC::NotEqual,
        (CompareOp::Lt, true) => IntCC::UnsignedLessThan,
        (CompareOp::Le, true) => IntCC::UnsignedLessThanOrEqual,
        (CompareOp::Gt, true) => IntCC::UnsignedGreaterThan,
        (CompareOp::Ge, true) => IntCC::UnsignedGreaterThanOrEqual,
        (CompareOp::Lt, false) => IntCC::SignedLessThan,
        (CompareOp::Le, false) => IntCC::SignedLessThanOrEqual,
        (CompareOp::Gt, false) => IntCC::SignedGreaterThan,
        (CompareOp::Ge, false) => IntCC::SignedGreaterThanOrEqual,
    }
}

fn float_compare_code(op: CompareOp) -> FloatCC {
    match op {
        CompareOp::Eq => FloatCC::Equal,
        CompareOp::Ne => FloatCC::NotEqual,
        CompareOp::Lt => FloatCC::LessThan,
        CompareOp::Le => FloatCC::LessThanOrEqual,
        CompareOp::Gt => FloatCC::GreaterThan,
        CompareOp::Ge => FloatCC::GreaterThanOrEqual,
    }
}

fn load_function_params(
    builder: &mut FunctionBuilder,
    args_ptr: Value,
    params: &[(ValueId, ast::SmolStr, IrType)],
    values: &mut HashMap<ValueId, Value>,
    func_name: &str,
) -> Result<(), JitError> {
    let mut offset = 0_i32;
    for (value_id, _, ty) in params {
        let value = load_primitive_value(builder, args_ptr, offset, ty, func_name)?;
        values.insert(*value_id, value);
        offset += storage_size(ty, func_name)?;
    }
    Ok(())
}

fn load_primitive_value(
    builder: &mut FunctionBuilder,
    args_ptr: Value,
    offset: i32,
    ty: &IrType,
    func_name: &str,
) -> Result<Value, JitError> {
    if matches!(ty, IrType::Unit) {
        return Ok(builder.ins().iconst(I32, 0));
    }
    if matches!(ty, IrType::Bool) {
        let raw = builder
            .ins()
            .load(types::I8, MemFlags::trusted(), args_ptr, offset);
        return Ok(builder.ins().icmp_imm(IntCC::NotEqual, raw, 0));
    }
    let cl_type = storage_type(ty, func_name)?;
    Ok(builder
        .ins()
        .load(cl_type, MemFlags::trusted(), args_ptr, offset))
}

fn store_return_value(
    builder: &mut FunctionBuilder,
    ret_ptr: Value,
    ret_type: &IrType,
    value: Value,
    _func_name: &str,
) -> Result<(), JitError> {
    if matches!(ret_type, IrType::Unit) {
        let zero = builder.ins().iconst(I32, 0);
        builder.ins().store(MemFlags::trusted(), zero, ret_ptr, 0);
        return Ok(());
    }
    if matches!(ret_type, IrType::Bool) {
        builder.ins().store(MemFlags::trusted(), value, ret_ptr, 0);
        return Ok(());
    }
    builder.ins().store(MemFlags::trusted(), value, ret_ptr, 0);
    Ok(())
}

fn lookup_type<'a>(
    types: &'a HashMap<ValueId, IrType>,
    value_id: ValueId,
    func_name: &str,
) -> Result<&'a IrType, JitError> {
    types
        .get(&value_id)
        .ok_or_else(|| JitError::UnimplementedInstruction {
            instruction: format!("missing type for {value_id}"),
            function: func_name.to_string(),
        })
}

fn infer_value_types(
    func: &IrFunction,
    fn_return_types: &HashMap<String, IrType>,
) -> Result<HashMap<ValueId, IrType>, JitError> {
    let mut types = HashMap::new();
    for (value_id, _, ty) in &func.params {
        types.insert(*value_id, ty.clone());
    }
    for block in &func.blocks {
        for (value_id, ty) in &block.params {
            types.insert(*value_id, ty.clone());
        }
        for (defined, inst) in &block.instructions {
            if let Some(value_id) = defined {
                let ty = infer_instruction_type(inst, &types, func.name.as_ref(), fn_return_types)?;
                types.insert(*value_id, ty);
            }
        }
    }
    Ok(types)
}

fn infer_instruction_type(
    inst: &ir::Instruction,
    types: &HashMap<ValueId, IrType>,
    func_name: &str,
    fn_return_types: &HashMap<String, IrType>,
) -> Result<IrType, JitError> {
    ir::infer_instruction_type(inst, types, fn_return_types).ok_or_else(|| {
        JitError::UnimplementedInstruction {
            instruction: format!("{inst:?}"),
            function: func_name.to_string(),
        }
    })
}

fn lookup_value(
    values: &HashMap<ValueId, Value>,
    value_id: ValueId,
    func_name: &str,
) -> Result<Value, JitError> {
    values
        .get(&value_id)
        .copied()
        .ok_or_else(|| JitError::UnimplementedInstruction {
            instruction: format!("missing value for {value_id}"),
            function: func_name.to_string(),
        })
}

fn storage_type(ty: &IrType, func_name: &str) -> Result<Type, JitError> {
    match ty {
        IrType::I8 | IrType::U8 => Ok(types::I8),
        IrType::I16 | IrType::U16 => Ok(types::I16),
        IrType::I32 | IrType::U32 => Ok(I32),
        IrType::I64 | IrType::U64 | IrType::Usize => Ok(types::I64),
        IrType::F32 => Ok(types::F32),
        IrType::F64 => Ok(types::F64),
        IrType::Bool => Ok(types::I8),
        IrType::Str => Ok(types::I64),
        IrType::Unit => Ok(I32),
        _ => Err(unsupported_type(func_name, ty)),
    }
}

fn storage_size(ty: &IrType, func_name: &str) -> Result<i32, JitError> {
    match ty {
        IrType::I8 | IrType::U8 | IrType::Bool => Ok(1),
        IrType::I16 | IrType::U16 => Ok(2),
        IrType::I32 | IrType::U32 | IrType::F32 | IrType::Unit => Ok(4),
        IrType::I64 | IrType::U64 | IrType::Usize | IrType::F64 | IrType::Str => Ok(8),
        _ => Err(unsupported_type(func_name, ty)),
    }
}

fn is_float(ty: &IrType) -> bool {
    matches!(ty, IrType::F32 | IrType::F64)
}

fn is_integer(ty: &IrType) -> bool {
    matches!(
        ty,
        IrType::I8
            | IrType::I16
            | IrType::I32
            | IrType::I64
            | IrType::U8
            | IrType::U16
            | IrType::U32
            | IrType::U64
            | IrType::Usize
    )
}

fn is_unsigned_integer(ty: &IrType) -> bool {
    matches!(
        ty,
        IrType::U8 | IrType::U16 | IrType::U32 | IrType::U64 | IrType::Usize
    )
}

fn unsupported_type(func_name: &str, ty: &IrType) -> JitError {
    JitError::UnimplementedInstruction {
        instruction: format!("type {ty}"),
        function: func_name.to_string(),
    }
}

fn decode_main_i32(ret_type: &IrType, ret_buf: &[u8; 16]) -> Result<i32, JitError> {
    match ret_type {
        IrType::I8 => Ok(i8::from_ne_bytes([ret_buf[0]]) as i32),
        IrType::I16 => Ok(i16::from_ne_bytes([ret_buf[0], ret_buf[1]]) as i32),
        IrType::I32 => Ok(i32::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3],
        ])),
        IrType::U8 | IrType::Bool => Ok(u8::from_ne_bytes([ret_buf[0]]) as i32),
        IrType::U16 => Ok(u16::from_ne_bytes([ret_buf[0], ret_buf[1]]) as i32),
        IrType::U32 => {
            Ok(u32::from_ne_bytes([ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3]]) as i32)
        }
        IrType::Unit => Ok(0),
        _ => Err(JitError::UnimplementedInstruction {
            instruction: format!(
                "call_main does not support lossy return type {ret_type}; use call_main_raw instead"
            ),
            function: "main".to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Runtime builtins called by JIT-compiled code
// ---------------------------------------------------------------------------

/// External function called by `Println` JIT code.
///
/// Reads the argument value and type tag from `args`, prints the
/// value to stdout, and writes `0` (Unit) to `ret`.
///
/// # Safety
///
/// `args` must point to a valid 12-byte buffer:
///   - bytes 0–7:  value as `i64` (extended/bitcast from native type)
///   - bytes 8–11: type tag as `u32` (see [`ir_type_tag`])
///
/// For `Str` the value is a pointer to length‑prefixed UTF‑8 bytes:
/// bytes 0–3 store the length as `u32`, followed by the string content.
#[unsafe(no_mangle)]
unsafe extern "C" fn __pipe_println(args: *const u8, ret: *mut u8) -> i32 {
    let raw = unsafe { std::ptr::read_unaligned(args as *const i64) };
    let type_tag = unsafe { std::ptr::read_unaligned(args.add(8) as *const u32) };
    match type_tag {
        0 => println!("{}", raw as i8),
        1 => println!("{}", raw as i16),
        2 => println!("{}", raw as i32),
        3 => println!("{}", raw),
        4 => println!("{}", raw as u8),
        5 => println!("{}", raw as u16),
        6 => println!("{}", raw as u32),
        7 => println!("{}", raw as u64),
        8 => println!("{}", f32::from_bits(raw as u32)),
        9 => println!("{}", f64::from_bits(raw as u64)),
        10 => println!("{}", raw != 0),
        11 => {
            let ptr = raw as *const u8;
            let len = unsafe { std::ptr::read_unaligned(ptr as *const u32) } as usize;
            let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
            let s = unsafe { std::str::from_utf8_unchecked(bytes) };
            println!("{s}");
        }
        _ => {}
    }
    unsafe {
        *(ret as *mut i32) = 0;
    }
    0
}

/// External function called by `StrConcat` JIT code.
///
/// Concatenates an array of type-tagged values into a single
/// length-prefixed string and writes the pointer to `ret`.
///
/// # Safety
///
/// `args` points to a buffer in this layout:
///   - bytes 0–3:    `u32` count of parts
///   - for each part i: value as `i64` (8 bytes) then type tag as `u32` (4 bytes)
///
/// `ret` points to an 8-byte buffer that receives the pointer to
/// the length-prefixed concatenated result.
#[unsafe(no_mangle)]
unsafe extern "C" fn pipe_rt_str_concat(args: *const u8, ret: *mut u8) -> i32 {
    let count = unsafe { std::ptr::read_unaligned(args as *const u32) } as usize;
    let mut result = String::new();
    for i in 0..count {
        let base = unsafe { args.add(4 + i * 12) };
        let raw = unsafe { std::ptr::read_unaligned(base as *const i64) };
        let type_tag = unsafe { std::ptr::read_unaligned(base.add(8) as *const u32) };
        match type_tag {
            0 => result.push_str(&format!("{}", raw as i8)),
            1 => result.push_str(&format!("{}", raw as i16)),
            2 => result.push_str(&format!("{}", raw as i32)),
            3 => result.push_str(&format!("{}", raw)),
            4 => result.push_str(&format!("{}", raw as u8)),
            5 => result.push_str(&format!("{}", raw as u16)),
            6 => result.push_str(&format!("{}", raw as u32)),
            7 => result.push_str(&format!("{}", raw as u64)),
            8 => result.push_str(&format!("{}", f32::from_bits(raw as u32))),
            9 => result.push_str(&format!("{}", f64::from_bits(raw as u64))),
            10 => result.push_str(&format!("{}", raw != 0)),
            11 => {
                let ptr = raw as *const u8;
                let len = unsafe { std::ptr::read_unaligned(ptr as *const u32) } as usize;
                let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
                let s = unsafe { std::str::from_utf8_unchecked(bytes) };
                result.push_str(s);
            }
            _ => {}
        }
    }
    let bytes = result.into_bytes();
    let len = bytes.len() as u32;
    let mut buf = Vec::with_capacity(4 + bytes.len());
    buf.extend_from_slice(&len.to_ne_bytes());
    buf.extend_from_slice(&bytes);
    let ptr = Box::leak(buf.into_boxed_slice()).as_ptr();
    unsafe {
        *(ret as *mut u64) = ptr as u64;
    }
    0
}

/// Numeric tag used by `__pipe_println` to interpret the raw value.
fn ir_type_tag(ty: &IrType) -> Option<u32> {
    match ty {
        IrType::I8 => Some(0),
        IrType::I16 => Some(1),
        IrType::I32 => Some(2),
        IrType::I64 | IrType::Usize => Some(3),
        IrType::U8 => Some(4),
        IrType::U16 => Some(5),
        IrType::U32 => Some(6),
        IrType::U64 => Some(7),
        IrType::F32 => Some(8),
        IrType::F64 => Some(9),
        IrType::Bool => Some(10),
        IrType::Str => Some(11),
        IrType::Unit => Some(12),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ast::SmolStr;
    use ir::{BasicBlock, IrFunction, IrModule};

    fn push_inst(func: &mut IrFunction, block: &mut BasicBlock, inst: ir::Instruction) -> ValueId {
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
        module.decls.push(ir::IrDecl::Function(func));
        module
    }

    fn module_with_main(func: IrFunction) -> IrModule {
        let mut module = IrModule::new();
        module.decls.push(ir::IrDecl::Function(func));
        module
    }

    fn call_main_raw(compiled: &CompiledModule) -> [u8; 16] {
        let mut ret_buf = [0u8; 16];
        let code = unsafe { (compiled.main_ptr)(std::ptr::null(), ret_buf.as_mut_ptr()) };
        assert_eq!(code, 0);
        ret_buf
    }

    /// Read a length-prefixed string from a raw pointer.
    /// Layout: [len: u32][bytes...] — len is the byte count of the content.
    fn read_len_prefixed_str(ptr: u64) -> &'static [u8] {
        let p = ptr as *const u8;
        let len = unsafe { std::ptr::read_unaligned(p as *const u32) } as usize;
        unsafe { std::slice::from_raw_parts(p.add(4), len) }
    }

    /// Assert that a raw pointer points to a length-prefixed string
    /// matching `expected`.
    fn check_len_prefixed_str(ptr: u64, expected: &[u8]) {
        let actual = read_len_prefixed_str(ptr);
        assert_eq!(actual, expected);
    }

    fn make_main_returning(n: i32) -> IrModule {
        make_main(IrType::I32, |func, block| {
            push_inst(func, block, ir::Instruction::ConstI32(n))
        })
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
    fn jit_compiles_addition() {
        let module = make_main(IrType::I32, |func, block| {
            let one = push_inst(func, block, ir::Instruction::ConstI32(1));
            let two = push_inst(func, block, ir::Instruction::ConstI32(2));
            push_inst(func, block, ir::Instruction::Add(one, two))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, 3);
    }

    #[test]
    fn jit_compiles_greater_than() {
        let module = make_main(IrType::Bool, |func, block| {
            let five = push_inst(func, block, ir::Instruction::ConstI32(5));
            let three = push_inst(func, block, ir::Instruction::ConstI32(3));
            push_inst(func, block, ir::Instruction::Gt(five, three))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, 1);
    }

    #[test]
    fn jit_compiles_negation() {
        let module = make_main(IrType::I32, |func, block| {
            let ten = push_inst(func, block, ir::Instruction::ConstI32(10));
            push_inst(func, block, ir::Instruction::Neg(ten))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, -10);
    }

    #[test]
    fn jit_compiles_bool_not() {
        let module = make_main(IrType::Bool, |func, block| {
            let truth = push_inst(func, block, ir::Instruction::ConstBool(true));
            push_inst(func, block, ir::Instruction::Not(truth))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let result = compiled.call_main().expect("main should run");
        assert_eq!(result, 0);
    }

    #[test]
    fn jit_compiles_float_arithmetic() {
        let module = make_main(IrType::F64, |func, block| {
            let lhs = push_inst(func, block, ir::Instruction::ConstF64(6.0));
            let rhs = push_inst(func, block, ir::Instruction::ConstF64(4.0));
            push_inst(func, block, ir::Instruction::Div(lhs, rhs))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let result = f64::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3], ret_buf[4], ret_buf[5], ret_buf[6],
            ret_buf[7],
        ]);
        assert_eq!(result, 1.5);
    }

    #[test]
    fn jit_compiles_float_remainder() {
        let module = make_main(IrType::F64, |func, block| {
            let lhs = push_inst(func, block, ir::Instruction::ConstF64(-7.5));
            let rhs = push_inst(func, block, ir::Instruction::ConstF64(2.0));
            push_inst(func, block, ir::Instruction::Rem(lhs, rhs))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let result = f64::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3], ret_buf[4], ret_buf[5], ret_buf[6],
            ret_buf[7],
        ]);
        assert_eq!(result, -1.5);
    }

    #[test]
    fn jit_compiles_unsigned_constants() {
        let module = make_main(IrType::U64, |func, block| {
            push_inst(func, block, ir::Instruction::ConstU64(9))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let result = u64::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3], ret_buf[4], ret_buf[5], ret_buf[6],
            ret_buf[7],
        ]);
        assert_eq!(result, 9);
    }

    #[test]
    fn jit_compiles_jump_with_block_parameter() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let merge_id = func.alloc_block();

        let mut entry = BasicBlock::new(entry_id);
        let value = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(42));
        entry.terminator = Terminator::Jump {
            target: merge_id,
            args: vec![value],
        };

        let mut merge = BasicBlock::new(merge_id);
        let result = func.alloc_value();
        merge.params.push((result, IrType::I32));
        merge.terminator = Terminator::Return(result);
        func.blocks.extend([entry, merge]);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 42);
    }

    #[test]
    fn jit_compiles_branch_and_merge() {
        for (condition, expected) in [(true, 11), (false, 22)] {
            let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
            let entry_id = func.alloc_block();
            let then_id = func.alloc_block();
            let else_id = func.alloc_block();
            let merge_id = func.alloc_block();

            let mut entry = BasicBlock::new(entry_id);
            let condition = push_inst(&mut func, &mut entry, ir::Instruction::ConstBool(condition));
            entry.terminator = Terminator::Branch {
                condition,
                then_block: then_id,
                then_args: vec![],
                else_block: else_id,
                else_args: vec![],
            };

            let mut then_block = BasicBlock::new(then_id);
            let then_value = push_inst(&mut func, &mut then_block, ir::Instruction::ConstI32(11));
            then_block.terminator = Terminator::Jump {
                target: merge_id,
                args: vec![then_value],
            };

            let mut else_block = BasicBlock::new(else_id);
            let else_value = push_inst(&mut func, &mut else_block, ir::Instruction::ConstI32(22));
            else_block.terminator = Terminator::Jump {
                target: merge_id,
                args: vec![else_value],
            };

            let mut merge = BasicBlock::new(merge_id);
            let result = func.alloc_value();
            merge.params.push((result, IrType::I32));
            merge.terminator = Terminator::Return(result);
            func.blocks.extend([entry, then_block, else_block, merge]);

            let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
            assert_eq!(compiled.call_main().expect("main should run"), expected);
        }
    }

    #[test]
    fn jit_compiles_branch_with_block_arguments() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let then_id = func.alloc_block();
        let else_id = func.alloc_block();

        let mut entry = BasicBlock::new(entry_id);
        let condition = push_inst(&mut func, &mut entry, ir::Instruction::ConstBool(false));
        let then_value = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(3));
        let else_value = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(8));
        entry.terminator = Terminator::Branch {
            condition,
            then_block: then_id,
            then_args: vec![then_value],
            else_block: else_id,
            else_args: vec![else_value],
        };

        let mut then_block = BasicBlock::new(then_id);
        let then_param = func.alloc_value();
        then_block.params.push((then_param, IrType::I32));
        then_block.terminator = Terminator::Return(then_param);

        let mut else_block = BasicBlock::new(else_id);
        let else_param = func.alloc_value();
        else_block.params.push((else_param, IrType::I32));
        else_block.terminator = Terminator::Return(else_param);
        func.blocks.extend([entry, then_block, else_block]);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 8);
    }

    #[test]
    fn jit_compiles_sparse_switch() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let one_id = func.alloc_block();
        let seven_id = func.alloc_block();
        let default_id = func.alloc_block();

        let mut entry = BasicBlock::new(entry_id);
        let discriminant = push_inst(&mut func, &mut entry, ir::Instruction::ConstU32(7));
        entry.terminator = Terminator::Switch {
            discriminant,
            arms: vec![(1, one_id, vec![]), (7, seven_id, vec![])],
            default: Some((default_id, vec![])),
        };

        let mut one = BasicBlock::new(one_id);
        let one_value = push_inst(&mut func, &mut one, ir::Instruction::ConstI32(10));
        one.terminator = Terminator::Return(one_value);

        let mut seven = BasicBlock::new(seven_id);
        let seven_value = push_inst(&mut func, &mut seven, ir::Instruction::ConstI32(70));
        seven.terminator = Terminator::Return(seven_value);

        let mut default = BasicBlock::new(default_id);
        let default_value = push_inst(&mut func, &mut default, ir::Instruction::ConstI32(-1));
        default.terminator = Terminator::Return(default_value);
        func.blocks.extend([entry, one, seven, default]);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 70);
    }

    #[test]
    fn jit_compiles_switch_with_block_arguments() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let selected_id = func.alloc_block();
        let default_id = func.alloc_block();

        let mut entry = BasicBlock::new(entry_id);
        let discriminant = push_inst(&mut func, &mut entry, ir::Instruction::ConstBool(true));
        let selected_value = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(9));
        let default_value = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(4));
        entry.terminator = Terminator::Switch {
            discriminant,
            arms: vec![(1, selected_id, vec![selected_value])],
            default: Some((default_id, vec![default_value])),
        };

        let mut selected = BasicBlock::new(selected_id);
        let selected_param = func.alloc_value();
        selected.params.push((selected_param, IrType::I32));
        selected.terminator = Terminator::Return(selected_param);

        let mut default = BasicBlock::new(default_id);
        let default_param = func.alloc_value();
        default.params.push((default_param, IrType::I32));
        default.terminator = Terminator::Return(default_param);
        func.blocks.extend([entry, selected, default]);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 9);
    }

    #[test]
    fn jit_compiles_unreachable_terminator() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Unit);
        let entry_id = func.alloc_block();
        func.blocks.push(BasicBlock::new(entry_id));

        compile_ir(&module_with_main(func)).expect("unreachable should compile to a trap");
    }

    #[test]
    fn jit_compiles_const_str() {
        let module = make_main(IrType::Str, |func, block| {
            push_inst(func, block, ir::Instruction::ConstStr("hello".into()))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"hello");
    }

    #[test]
    fn jit_compiles_const_str_empty() {
        let module = make_main(IrType::Str, |func, block| {
            push_inst(func, block, ir::Instruction::ConstStr("".into()))
        });
        let compiled = compile_ir(&module).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"");
    }

    #[test]
    fn jit_compiles_const_str_multiple() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Str);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let _a = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("alpha".into()),
        );
        let _b = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("beta".into()),
        );
        let result = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("gamma".into()),
        );
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"gamma");
    }

    // -----------------------------------------------------------------------
    // Println tests
    // -----------------------------------------------------------------------

    #[test]
    fn jit_compiles_println_str_then_returns_value() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let msg = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("hello JIT".into()),
        );
        entry
            .instructions
            .push((None, ir::Instruction::Println(msg)));
        let result = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(42));
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 42);
    }

    #[test]
    fn jit_compiles_println_int_then_returns_value() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let val = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(99));
        entry
            .instructions
            .push((None, ir::Instruction::Println(val)));
        let result = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(7));
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 7);
    }

    #[test]
    fn jit_compiles_println_bool_then_returns_value() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let val = push_inst(&mut func, &mut entry, ir::Instruction::ConstBool(true));
        entry
            .instructions
            .push((None, ir::Instruction::Println(val)));
        let result = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(1));
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 1);
    }

    #[test]
    fn jit_compiles_println_float_then_returns_value() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let val = push_inst(&mut func, &mut entry, ir::Instruction::ConstF64(std::f64::consts::PI));
        entry
            .instructions
            .push((None, ir::Instruction::Println(val)));
        let result = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(0));
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 0);
    }

    #[test]
    fn jit_compiles_println_multiple_times() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::I32);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let a = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("first".into()),
        );
        entry.instructions.push((None, ir::Instruction::Println(a)));
        let b = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(42));
        entry.instructions.push((None, ir::Instruction::Println(b)));
        let c = push_inst(&mut func, &mut entry, ir::Instruction::ConstBool(false));
        entry.instructions.push((None, ir::Instruction::Println(c)));
        let result = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(999));
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        assert_eq!(compiled.call_main().expect("main should run"), 999);
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

    // -----------------------------------------------------------------------
    // End-to-end control flow tests (source -> typecheck -> lower -> JIT)
    // -----------------------------------------------------------------------

    fn lower_and_compile(src: &str) -> CompiledModule {
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
    fn e2e_if_true_branch() {
        assert_eq!(e2e_main_i32("let main = if true { 42 } else { 0 }"), 42);
    }

    #[test]
    fn e2e_if_false_branch() {
        assert_eq!(e2e_main_i32("let main = if false { 42 } else { 0 }"), 0);
    }

    #[test]
    fn e2e_if_with_comparison_condition() {
        assert_eq!(e2e_main_i32("let main = if 10 > 5 { 1 } else { 2 }"), 1);
        assert_eq!(e2e_main_i32("let main = if 3 > 10 { 1 } else { 2 }"), 2);
    }

    #[test]
    fn e2e_if_with_equal_comparison() {
        assert_eq!(
            e2e_main_i32("let main = if 7 == 7 { 100 } else { 200 }"),
            100
        );
        assert_eq!(
            e2e_main_i32("let main = if 7 == 8 { 100 } else { 200 }"),
            200
        );
    }

    #[test]
    fn e2e_nested_if_else() {
        assert_eq!(
            e2e_main_i32("let main = if true { if false { 1 } else { 2 } } else { 3 }"),
            2
        );
        assert_eq!(
            e2e_main_i32("let main = if false { 1 } else { if true { 2 } else { 3 } }"),
            2
        );
    }

    #[test]
    fn e2e_if_with_arithmetic_in_branches() {
        assert_eq!(
            e2e_main_i32("let main = if true { 10 + 20 } else { 0 - 1 }"),
            30
        );
        assert_eq!(
            e2e_main_i32("let main = if false { 10 + 20 } else { 0 - 1 }"),
            -1
        );
    }

    #[test]
    fn e2e_if_bool_result() {
        let compiled = lower_and_compile("let main = if true { true } else { false }");
        let ret_buf = call_main_raw(&compiled);
        assert_eq!(ret_buf[0], 1, "true branch should return bool true");

        let compiled = lower_and_compile("let main = if false { true } else { false }");
        let ret_buf = call_main_raw(&compiled);
        assert_eq!(ret_buf[0], 0, "false branch should return bool false");
    }

    #[test]
    fn e2e_if_float_result() {
        let compiled = lower_and_compile("let main = if true { 3.5 } else { 1.5 }");
        let ret_buf = call_main_raw(&compiled);
        let result = f64::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3], ret_buf[4], ret_buf[5], ret_buf[6],
            ret_buf[7],
        ]);
        assert_eq!(result, 3.5);
    }

    #[test]
    fn e2e_deeply_nested_if() {
        // Nested if/else without function params: condition-controlled nesting
        assert_eq!(
            e2e_main_i32(
                "let main = if true { if true { 1 } else { 0 } } else { if true { 2 } else { 3 } }"
            ),
            1
        );
        assert_eq!(
            e2e_main_i32(
                "let main = if false { if true { 1 } else { 0 } } else { if true { 2 } else { 3 } }"
            ),
            2
        );
    }

    #[test]
    fn e2e_if_chain_comparison() {
        assert_eq!(
            e2e_main_i32("let main = if (5 < 10) && (10 < 20) { 1 } else { 0 }"),
            1
        );
        assert_eq!(
            e2e_main_i32("let main = if (5 < 10) && (10 > 20) { 1 } else { 0 }"),
            0
        );
    }

    // -----------------------------------------------------------------------
    // End-to-end CallNamed tests
    // -----------------------------------------------------------------------

    #[test]
    fn e2e_call_named_simple() {
        assert_eq!(
            e2e_main_i32("let add = (a: i32, b: i32) => a + b\nlet main = add(3, 4)"),
            7
        );
    }

    #[test]
    fn e2e_call_named_zero_args() {
        assert_eq!(e2e_main_i32("let five = () => 5\nlet main = five()"), 5);
    }

    #[test]
    fn e2e_call_named_returns_bool() {
        let compiled = lower_and_compile("let is_pos = (x: i32) => x > 0\nlet main = is_pos(5)");
        let ret_buf = call_main_raw(&compiled);
        assert_eq!(ret_buf[0], 1, "is_pos(5) should return bool true");

        let compiled = lower_and_compile("let is_pos = (x: i32) => x > 0\nlet main = is_pos(-3)");
        let ret_buf = call_main_raw(&compiled);
        assert_eq!(ret_buf[0], 0, "is_pos(-3) should return bool false");
    }

    #[test]
    fn e2e_call_named_float_args() {
        let compiled =
            lower_and_compile("let mul = (a: f64, b: f64) => a * b\nlet main = mul(1.5, 2.0)");
        let ret_buf = call_main_raw(&compiled);
        let result = f64::from_ne_bytes([
            ret_buf[0], ret_buf[1], ret_buf[2], ret_buf[3], ret_buf[4], ret_buf[5], ret_buf[6],
            ret_buf[7],
        ]);
        assert_eq!(result, 3.0);
    }

    #[test]
    fn e2e_call_named_chained() {
        assert_eq!(
            e2e_main_i32(
                "let add = (a: i32, b: i32) => a + b\nlet main = add(add(1, 2), add(3, 4))"
            ),
            10
        );
    }

    #[test]
    fn e2e_call_named_with_bool_arg() {
        assert_eq!(
            e2e_main_i32("let cond = (c: bool) => if c { 1 } else { 0 }\nlet main = cond(true)"),
            1
        );
        assert_eq!(
            e2e_main_i32("let cond = (c: bool) => if c { 1 } else { 0 }\nlet main = cond(false)"),
            0
        );
    }

    #[test]
    fn e2e_call_named_multiple_calls() {
        assert_eq!(
            e2e_main_i32(
                "let add = (a: i32, b: i32) => a + b\nlet main = add(10, 20) + add(30, 40)"
            ),
            100
        );
    }

    #[test]
    fn e2e_call_named_identity() {
        assert_eq!(
            e2e_main_i32("let id = (x: i32) => x\nlet main = id(99)"),
            99
        );
    }

    // -----------------------------------------------------------------------
    // StrConcat tests
    // -----------------------------------------------------------------------

    #[test]
    fn jit_compiles_str_concat_two_strings() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Str);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let a = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("hello ".into()),
        );
        let b = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("world".into()),
        );
        let result = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::StrConcat { parts: vec![a, b] },
        );
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"hello world");
    }

    #[test]
    fn jit_compiles_str_concat_three_strings() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Str);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let a = push_inst(&mut func, &mut entry, ir::Instruction::ConstStr("a".into()));
        let b = push_inst(&mut func, &mut entry, ir::Instruction::ConstStr("b".into()));
        let c = push_inst(&mut func, &mut entry, ir::Instruction::ConstStr("c".into()));
        let result = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::StrConcat {
                parts: vec![a, b, c],
            },
        );
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"abc");
    }

    #[test]
    fn jit_compiles_str_concat_with_int_part() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Str);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let a = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstStr("value: ".into()),
        );
        let n = push_inst(&mut func, &mut entry, ir::Instruction::ConstI32(42));
        let result = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::StrConcat { parts: vec![a, n] },
        );
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"value: 42");
    }

    #[test]
    fn jit_compiles_str_concat_empty_parts() {
        let mut func = IrFunction::new(SmolStr::new("main"), IrType::Str);
        let entry_id = func.alloc_block();
        let mut entry = BasicBlock::new(entry_id);
        let result = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::StrConcat { parts: vec![] },
        );
        entry.terminator = Terminator::Return(result);
        func.blocks.push(entry);

        let compiled = compile_ir(&module_with_main(func)).expect("compile should succeed");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"");
    }

    #[test]
    fn e2e_template_string_concat() {
        let compiled = lower_and_compile("let main = `hello world`");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"hello world");
    }

    #[test]
    fn e2e_template_string_with_interpolation() {
        let compiled = lower_and_compile("let main = `hello ${42}`");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"hello 42");
    }

    #[test]
    fn e2e_template_string_bool_interpolation() {
        let compiled = lower_and_compile("let main = `flag: ${true}`");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"flag: true");
    }

    #[test]
    fn e2e_template_string_float_interpolation() {
        let compiled = lower_and_compile("let main = `pi is ${3.14}`");
        let ret_buf = call_main_raw(&compiled);
        let ptr = u64::from_ne_bytes(ret_buf[..8].try_into().unwrap());
        check_len_prefixed_str(ptr, b"pi is 3.14");
    }
}
