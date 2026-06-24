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
    let mut fn_param_types: HashMap<String, Vec<IrType>> = HashMap::new();
    for func in ir_module.functions() {
        let name = func.name.as_str().to_string();
        let sig = make_signature(&module);
        let id = module
            .declare_function(&name, Linkage::Export, &sig)
            .map_err(JitError::from)?;
        let ret_type = func.return_type.clone();
        name_to_func.insert(name.clone(), (id, ret_type.clone()));
        func_ids.push((name.clone(), id, ret_type));
        let param_tys: Vec<IrType> = func.params.iter().map(|(_, _, ty)| ty.clone()).collect();
        fn_param_types.insert(name, param_tys);
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

    // Declare a data object for the pipe_rt_alloc_closure function pointer.
    let alloc_closure_ptr = pipe_rt_alloc_closure as *const ();
    let alloc_closure_ptr_data_id =
        module.declare_data("__pipe_alloc_closure_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (alloc_closure_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(alloc_closure_ptr_data_id, &data_desc)?;
    }

    // Declare a data object for the pipe_rt_alloc_array function pointer.
    let alloc_array_ptr = pipe_rt_alloc_array as *const ();
    let alloc_array_ptr_data_id =
        module.declare_data("__pipe_alloc_array_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (alloc_array_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(alloc_array_ptr_data_id, &data_desc)?;
    }

    // Declare a data object for the pipe_rt_array_concat function pointer.
    let array_concat_ptr = pipe_rt_array_concat as *const ();
    let array_concat_ptr_data_id =
        module.declare_data("__pipe_array_concat_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (array_concat_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(array_concat_ptr_data_id, &data_desc)?;
    }

    // Scan all functions for CallNamed instructions referencing
    // builtins (not local functions) and create name data objects.
    let mut unique_builtin_names: Vec<String> = Vec::new();
    let mut seen_builtin_names: HashSet<String> = HashSet::new();
    for func in ir_module.functions() {
        for block in &func.blocks {
            for (_, inst) in &block.instructions {
                if let ir::Instruction::CallNamed(data) = inst {
                    let name_str = data.name.to_string();
                    if !name_to_func.contains_key(&name_str)
                        && seen_builtin_names.insert(name_str.clone())
                    {
                        unique_builtin_names.push(name_str);
                    }
                }
            }
        }
    }
    let mut builtin_name_data_ids: HashMap<String, DataId> = HashMap::new();
    for builtin_name in &unique_builtin_names {
        let data_name = format!("__builtin_name_{}", builtin_name);
        let data_id = module.declare_data(&data_name, Linkage::Local, false, false)?;
        let mut data_desc = DataDescription::new();
        let bytes = builtin_name.as_bytes();
        let len = bytes.len() as u32;
        let mut data = Vec::with_capacity(4 + bytes.len());
        data.extend_from_slice(&len.to_ne_bytes());
        data.extend_from_slice(bytes);
        data_desc.define(data.into_boxed_slice());
        module.define_data(data_id, &data_desc)?;
        builtin_name_data_ids.insert(builtin_name.clone(), data_id);
    }

    // Declare a data object for the pipe_rt_call_builtin function pointer.
    let call_builtin_ptr = pipe_rt_call_builtin as *const ();
    let call_builtin_ptr_data_id =
        module.declare_data("__pipe_call_builtin_ptr", Linkage::Local, false, false)?;
    {
        let mut data_desc = DataDescription::new();
        let ptr_bytes: Vec<u8> = (call_builtin_ptr as u64).to_ne_bytes().to_vec();
        data_desc.define(ptr_bytes.into_boxed_slice());
        module.define_data(call_builtin_ptr_data_id, &data_desc)?;
    }

    // Pre-compute each function's actual return type (with full closure
    // params including captures) so CallNamed can propagate the richer
    // type to downstream CallIndirect instructions.
    let mut fn_actual_return_types: HashMap<String, IrType> = HashMap::new();
    let fn_declared_return_types: HashMap<String, IrType> = name_to_func
        .iter()
        .map(|(n, (_, r))| (n.clone(), r.clone()))
        .collect();
    let empty_actual: HashMap<String, IrType> = HashMap::new();
    for (name, _func_id, _ret_type) in &func_ids {
        let Some(func) = ir_module.function(name) else {
            continue;
        };
        if let Ok(value_types) = infer_value_types(
            func,
            &fn_declared_return_types,
            &fn_param_types,
            &empty_actual,
        ) {
            for block in &func.blocks {
                if let ir::Terminator::Return(value_id) = &block.terminator {
                    if let Some(ty) = value_types.get(value_id) {
                        fn_actual_return_types.insert(name.clone(), ty.clone());
                    }
                    break;
                }
            }
        }
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
            fn_param_types: &fn_param_types,
            fn_actual_return_types: &fn_actual_return_types,
            string_data_ids: &string_data_ids,
            println_ptr_data_id,
            str_concat_ptr_data_id,
            alloc_closure_ptr_data_id,
            alloc_array_ptr_data_id,
            array_concat_ptr_data_id,
            call_builtin_ptr_data_id,
            builtin_name_data_ids: &builtin_name_data_ids,
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
#[allow(dead_code)]
struct BlockContext<'a> {
    value_types: &'a HashMap<ValueId, IrType>,
    func_name: &'a str,
    callee_funcs: &'a HashMap<String, FuncRef>,
    fn_return_types: &'a HashMap<String, IrType>,
    fn_param_types: &'a HashMap<String, Vec<IrType>>,
    fn_actual_return_types: &'a HashMap<String, IrType>,
    string_globals: &'a HashMap<String, GlobalValue>,
    println_fn_ptr: Value,
    println_sig: SigRef,
    str_concat_fn_ptr: Value,
    str_concat_sig: SigRef,
    alloc_closure_fn_ptr: Value,
    alloc_closure_sig: SigRef,
    alloc_array_fn_ptr: Value,
    alloc_array_sig: SigRef,
    array_concat_fn_ptr: Value,
    array_concat_sig: SigRef,
    call_builtin_fn_ptr: Value,
    call_builtin_sig: SigRef,
    builtin_name_globals: &'a HashMap<String, GlobalValue>,
    closure_callee_funcs: &'a HashMap<String, FuncRef>,
    blocks: &'a HashMap<BlockId, Block>,
    ret_ptr: Value,
    ret_type: &'a IrType,
}

/// Module-scoped parameters for compiling a single function body.
struct FunctionBodyParams<'a> {
    module: &'a mut JITModule,
    fn_builder_ctx: &'a mut FunctionBuilderContext,
    name_to_func: &'a HashMap<String, (cranelift_module::FuncId, IrType)>,
    fn_param_types: &'a HashMap<String, Vec<IrType>>,
    fn_actual_return_types: &'a HashMap<String, IrType>,
    string_data_ids: &'a HashMap<String, DataId>,
    println_ptr_data_id: DataId,
    str_concat_ptr_data_id: DataId,
    alloc_closure_ptr_data_id: DataId,
    alloc_array_ptr_data_id: DataId,
    array_concat_ptr_data_id: DataId,
    call_builtin_ptr_data_id: DataId,
    builtin_name_data_ids: &'a HashMap<String, DataId>,
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
    let value_types = infer_value_types(
        func,
        &fn_return_types,
        params.fn_param_types,
        params.fn_actual_return_types,
    )?;
    let mut values = HashMap::new();

    load_function_params(
        &mut builder,
        args_ptr,
        &func.params,
        &mut values,
        func.name.as_ref(),
    )?;

    // Pre-import all callees referenced by CallNamed in this function.
    // Builtins (not in name_to_func) will be resolved via the builtin bridge.
    let mut callee_funcs: HashMap<String, FuncRef> = HashMap::new();
    for ir_block in &func.blocks {
        for (_, inst) in &ir_block.instructions {
            if let ir::Instruction::CallNamed(data) = inst {
                let name_str = data.name.to_string();
                if callee_funcs.contains_key(&name_str) {
                    continue;
                }
                let Some((callee_id, _)) = params.name_to_func.get(name_str.as_str()) else {
                    // Not a local function — will be resolved via builtin bridge.
                    continue;
                };
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

    // Import the pipe_rt_alloc_closure function pointer from a data object.
    let alloc_closure_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.alloc_closure_ptr_data_id, f)
    };
    let alloc_closure_fn_ptr_addr = builder
        .ins()
        .global_value(types::I64, alloc_closure_fn_ptr_gv);
    let alloc_closure_fn_ptr = builder.ins().load(
        types::I64,
        MemFlags::trusted(),
        alloc_closure_fn_ptr_addr,
        0,
    );
    let alloc_closure_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    // Import the pipe_rt_alloc_array function pointer from a data object.
    let alloc_array_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.alloc_array_ptr_data_id, f)
    };
    let alloc_array_fn_ptr_addr = builder
        .ins()
        .global_value(types::I64, alloc_array_fn_ptr_gv);
    let alloc_array_fn_ptr =
        builder
            .ins()
            .load(types::I64, MemFlags::trusted(), alloc_array_fn_ptr_addr, 0);
    let alloc_array_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    // Import the pipe_rt_array_concat function pointer from a data object.
    let array_concat_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.array_concat_ptr_data_id, f)
    };
    let array_concat_fn_ptr_addr = builder
        .ins()
        .global_value(types::I64, array_concat_fn_ptr_gv);
    let array_concat_fn_ptr =
        builder
            .ins()
            .load(types::I64, MemFlags::trusted(), array_concat_fn_ptr_addr, 0);
    let array_concat_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    // Import the pipe_rt_call_builtin function pointer from a data object.
    let call_builtin_fn_ptr_gv = {
        let f: &mut Function = builder.func;
        params
            .module
            .declare_data_in_func(params.call_builtin_ptr_data_id, f)
    };
    let call_builtin_fn_ptr_addr = builder
        .ins()
        .global_value(types::I64, call_builtin_fn_ptr_gv);
    let call_builtin_fn_ptr =
        builder
            .ins()
            .load(types::I64, MemFlags::trusted(), call_builtin_fn_ptr_addr, 0);
    let call_builtin_sig = {
        let sig = make_signature(params.module);
        let f: &mut Function = builder.func;
        f.import_signature(sig)
    };

    // Pre-import builtin name data objects referenced by CallNamed in this function.
    let mut builtin_name_globals: HashMap<String, GlobalValue> = HashMap::new();
    for ir_block in &func.blocks {
        for (_, inst) in &ir_block.instructions {
            if let ir::Instruction::CallNamed(data) = inst {
                let name_str = data.name.to_string();
                if builtin_name_globals.contains_key(&name_str) {
                    continue;
                }
                if params.name_to_func.contains_key(&name_str) {
                    continue;
                }
                let data_id = params.builtin_name_data_ids.get(&name_str).ok_or_else(|| {
                    JitError::UnimplementedInstruction {
                        instruction: format!("builtin name data not found for {name_str}"),
                        function: func.name.to_string(),
                    }
                })?;
                let gv = {
                    let f: &mut Function = builder.func;
                    params.module.declare_data_in_func(*data_id, f)
                };
                builtin_name_globals.insert(name_str, gv);
            }
        }
    }

    // Pre-import FuncRefs for functions used as MakeClosure targets.
    let mut closure_callee_funcs: HashMap<String, FuncRef> = HashMap::new();
    for ir_block in &func.blocks {
        for (_, inst) in &ir_block.instructions {
            if let ir::Instruction::MakeClosure(data) = inst {
                let name_str = data.func_name.to_string();
                if closure_callee_funcs.contains_key(&name_str) {
                    continue;
                }
                let Some((callee_id, _)) = params.name_to_func.get(name_str.as_str()) else {
                    return Err(JitError::UnimplementedInstruction {
                        instruction: format!("MakeClosure: unknown function `{name_str}`"),
                        function: func.name.to_string(),
                    });
                };
                let func_ref = {
                    let f: &mut Function = builder.func;
                    params.module.declare_func_in_func(*callee_id, f)
                };
                closure_callee_funcs.insert(name_str, func_ref);
            }
        }
    }

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
        fn_param_types: params.fn_param_types,
        fn_actual_return_types: params.fn_actual_return_types,
        string_globals: &string_globals,
        println_fn_ptr,
        println_sig,
        str_concat_fn_ptr,
        str_concat_sig,
        alloc_closure_fn_ptr,
        alloc_closure_sig,
        alloc_array_fn_ptr,
        alloc_array_sig,
        array_concat_fn_ptr,
        array_concat_sig,
        call_builtin_fn_ptr,
        call_builtin_sig,
        builtin_name_globals: &builtin_name_globals,
        closure_callee_funcs: &closure_callee_funcs,
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
        Terminator::TailCall { callee, args } => {
            compile_tail_call(builder, ctx, *callee, args, values)?;
            Ok(())
        }
        Terminator::Unreachable => {
            builder.ins().trap(UNREACHABLE_TRAP);
            Ok(())
        }
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

        ir::Instruction::MakeClosure(data) => compile_make_closure(builder, ctx, data, values)?,

        ir::Instruction::CallIndirect(data) => compile_call_indirect(builder, ctx, data, values)?,

        ir::Instruction::TagConstruct(data) => compile_tag_construct(builder, ctx, data, values)?,

        ir::Instruction::TagDiscriminant(value_id) => {
            compile_tag_discriminant(builder, ctx, *value_id, values)?
        }

        ir::Instruction::TagGet { value, index } => {
            compile_tag_get(builder, ctx, *value, *index, values)?
        }

        ir::Instruction::RecordAlloc(data) => compile_record_alloc(builder, ctx, data, values)?,

        ir::Instruction::RecordGet {
            record,
            field_index,
            ..
        } => compile_record_get(builder, ctx, *record, *field_index, values)?,

        ir::Instruction::ArrayAlloc { len, init } => {
            compile_array_alloc(builder, ctx, *len, *init, values)?
        }

        ir::Instruction::ArrayGet { array, index } => {
            compile_array_get(builder, ctx, *array, *index, values)?
        }

        ir::Instruction::ArrayLen(array_id) => compile_array_len(builder, ctx, *array_id, values)?,

        ir::Instruction::ArrayConcat(left_id, right_id) => {
            compile_array_concat(builder, ctx, *left_id, *right_id, values)?
        }

        ir::Instruction::RecordSet {
            record,
            field: _,
            field_index,
            value,
        } => compile_record_set(builder, ctx, *record, *field_index, *value, values)?,

        ir::Instruction::ArraySet {
            array,
            index,
            value,
        } => compile_array_set(builder, ctx, *array, *index, *value, values)?,

        ir::Instruction::Panic { .. } => {
            builder.ins().trap(UNREACHABLE_TRAP);
            builder.ins().iconst(I32, 0)
        }

        #[allow(unreachable_patterns)]
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

    // Try local function first.
    if let Some(func_ref) = ctx.callee_funcs.get(callee_name).copied() {
        let ret_type = ctx.fn_return_types.get(callee_name).ok_or_else(|| {
            JitError::UnimplementedInstruction {
                instruction: format!("CallNamed: no return type for {callee_name}"),
                function: ctx.func_name.to_string(),
            }
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
        return Ok(result);
    }

    // Fall through to builtin bridge when the callee is not a local
    // function — look it up from the process-wide builtin registry.
    compile_builtin_call(builder, ctx, callee_name, data, values)
}

fn compile_builtin_call(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    callee_name: &str,
    data: &ir::CallNamedData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let ret_type = &data.return_type;
    let arg_count = data.args.len();
    let buf_size: u32 = 16 + arg_count as u32 * 12;

    let arg_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        buf_size,
        0,
    ));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    // Store name pointer from the pre-declared data object.
    let name_gv = ctx.builtin_name_globals.get(callee_name).ok_or_else(|| {
        JitError::UnimplementedInstruction {
            instruction: format!("CallNamed builtin: no name data for {callee_name}"),
            function: ctx.func_name.to_string(),
        }
    })?;
    let name_ptr = builder.ins().global_value(types::I64, *name_gv);
    builder
        .ins()
        .store(MemFlags::trusted(), name_ptr, args_buf, 0);

    // Store name length.
    let name_len = builder.ins().iconst(I32, callee_name.len() as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), name_len, args_buf, 8);

    // Store argument count.
    let arg_count_val = builder.ins().iconst(I32, arg_count as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), arg_count_val, args_buf, 12);

    // Store each argument as (val: i64, tag: u32).
    for (i, arg_id) in data.args.iter().enumerate() {
        let arg_type = lookup_type(ctx.value_types, *arg_id, ctx.func_name)?;
        if !matches!(arg_type, IrType::Unit) {
            let arg_val = lookup_value(values, *arg_id, ctx.func_name)?;
            let widened = widen_to_i64(builder, arg_val, arg_type, ctx.func_name)?;
            let offset = 16 + i as i32 * 12;
            builder
                .ins()
                .store(MemFlags::trusted(), widened, args_buf, offset);
            let tag =
                ir_type_tag(arg_type).ok_or_else(|| unsupported_type(ctx.func_name, arg_type))?;
            let tag_val = builder.ins().iconst(I32, i64::from(tag));
            builder
                .ins()
                .store(MemFlags::trusted(), tag_val, args_buf, offset + 8);
        } else {
            // Unit args: store 0 val + tag 12.
            let zero = builder.ins().iconst(types::I64, 0);
            let offset = 16 + i as i32 * 12;
            builder
                .ins()
                .store(MemFlags::trusted(), zero, args_buf, offset);
            let tag_val = builder.ins().iconst(I32, 12);
            builder
                .ins()
                .store(MemFlags::trusted(), tag_val, args_buf, offset + 8);
        }
    }

    // Allocate 12-byte return buffer [val: i64, tag: u32].
    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 12, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder.ins().call_indirect(
        ctx.call_builtin_sig,
        ctx.call_builtin_fn_ptr,
        &[args_buf, ret_buf],
    );

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
    _func_name: &str,
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
        IrType::Unit => Ok(builder.ins().iconst(types::I64, 0)),
        IrType::Str
        | IrType::Array(_)
        | IrType::Record(_)
        | IrType::Tag(_)
        | IrType::Closure(_)
        | IrType::Func(_)
        | IrType::Effect(_) => Ok(val),
    }
}

/// Compile a `MakeClosure` instruction.
///
/// Layout on the heap: `[func_ptr: u64] [captures packed by storage_size]`.
/// The function pointer is stored in a data object and patched after
/// finalization. We load it, pack captures after it, call
/// `pipe_rt_alloc_closure` to heap-allocate, and return the pointer.
fn compile_make_closure(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    data: &ir::MakeClosureData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let func_name = data.func_name.as_str();

    // Get the target function pointer via func_addr.
    let func_ref = ctx.closure_callee_funcs.get(func_name).ok_or_else(|| {
        JitError::UnimplementedInstruction {
            instruction: format!("MakeClosure: no FuncRef for `{func_name}`"),
            function: ctx.func_name.to_string(),
        }
    })?;
    let fn_ptr = builder.ins().func_addr(types::I64, *func_ref);

    // Compute total byte size: 8 for func_ptr + captures.
    let capture_sizes: Vec<i32> = data
        .captures
        .iter()
        .map(|capture_id| {
            let ty = lookup_type(ctx.value_types, *capture_id, ctx.func_name)?;
            storage_size(ty, ctx.func_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let total_size: i32 = 8 + capture_sizes.iter().sum::<i32>();

    // Create a stack buffer for the closure content.
    let content_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        total_size.max(1) as u32,
        0,
    ));
    let content_buf = builder.ins().stack_addr(types::I64, content_slot, 0);

    // Store func_ptr at offset 0.
    builder
        .ins()
        .store(MemFlags::trusted(), fn_ptr, content_buf, 0);

    // Store each capture after func_ptr.
    let mut offset: i32 = 8;
    for (capture_id, size) in data.captures.iter().zip(capture_sizes.iter()) {
        let capture_val = lookup_value(values, *capture_id, ctx.func_name)?;
        builder
            .ins()
            .store(MemFlags::trusted(), capture_val, content_buf, offset);
        offset += size;
    }

    // Build args buffer for pipe_rt_alloc_closure: [data_ptr: u64, byte_size: u32].
    let args_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 12, 0));
    let args_buf = builder.ins().stack_addr(types::I64, args_slot, 0);

    let content_addr = builder.ins().stack_addr(types::I64, content_slot, 0);
    builder
        .ins()
        .store(MemFlags::trusted(), content_addr, args_buf, 0);

    let byte_size_val = builder.ins().iconst(I32, total_size as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), byte_size_val, args_buf, 8);

    // Allocate 8-byte ret buffer for the closure pointer.
    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder.ins().call_indirect(
        ctx.alloc_closure_sig,
        ctx.alloc_closure_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
}

/// Compile a `CallIndirect` instruction.
///
/// Closure layout: `[func_ptr: u64] [captures packed by storage_size]`.
/// We load func_ptr from offset 0, read captures starting at offset 8
/// using the type information from the closure's FuncType, build the
/// full args buffer (captures followed by explicit call arguments),
/// and call the function pointer via call_indirect.
fn compile_call_indirect(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    data: &ir::CallIndirectData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let closure_val = lookup_value(values, data.callee, ctx.func_name)?;
    let closure_type = lookup_type(ctx.value_types, data.callee, ctx.func_name)?;

    // Extract the FuncType from the closure type.
    let func_type = match closure_type {
        IrType::Closure(ft) => ft.as_ref().clone(),
        _ => {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("CallIndirect: callee is not a closure, got {closure_type}"),
                function: ctx.func_name.to_string(),
            });
        }
    };

    let ret_type = &data.return_type;

    // The FuncType.params = [capture_params..., call_params...].
    // The capture params are the first (total_params - call_args.len()) params.
    let call_arg_count = data.args.len();
    let total_param_count = func_type.params.len();
    if call_arg_count > total_param_count {
        return Err(JitError::UnimplementedInstruction {
            instruction: format!(
                "CallIndirect: expected at most {total_param_count} arguments, got {call_arg_count}"
            ),
            function: ctx.func_name.to_string(),
        });
    }
    let capture_param_count = total_param_count - call_arg_count;
    let capture_types: Vec<&IrType> = func_type.params[..capture_param_count].iter().collect();
    let call_arg_types: Vec<&IrType> = data
        .args
        .iter()
        .map(
            |id| match lookup_type(ctx.value_types, *id, ctx.func_name) {
                Ok(t) => t,
                Err(e) => panic!("{e}"),
            },
        )
        .collect();
    // Double-check consistency with FuncType params.
    let expected_arg_types: Vec<&IrType> = func_type.params[capture_param_count..].iter().collect();
    if call_arg_types.len() != expected_arg_types.len() {
        return Err(JitError::UnimplementedInstruction {
            instruction: format!(
                "CallIndirect: arg count mismatch, expected {} got {}",
                expected_arg_types.len(),
                call_arg_types.len(),
            ),
            function: ctx.func_name.to_string(),
        });
    }

    // Load func_ptr from closure offset 0.
    let fn_ptr = builder
        .ins()
        .load(types::I64, MemFlags::trusted(), closure_val, 0);

    // Compute total args buffer size: captures (from closure) + call args.
    let capture_total_size: i32 = capture_types
        .iter()
        .map(|ty| storage_size(ty, ctx.func_name))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();
    let call_args_total_size: i32 = call_arg_types
        .iter()
        .map(|ty| storage_size(ty, ctx.func_name))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();

    let args_buf_size = (capture_total_size + call_args_total_size).max(1) as u32;
    let arg_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        args_buf_size,
        0,
    ));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    // Copy captures from closure (offset 8) into args buffer.
    let mut offset: i32 = 0;
    let mut closure_offset: i32 = 8;
    for capture_ty in &capture_types {
        let cap_val = builder.ins().load(
            storage_type(capture_ty, ctx.func_name)?,
            MemFlags::trusted(),
            closure_val,
            closure_offset,
        );
        builder
            .ins()
            .store(MemFlags::trusted(), cap_val, args_buf, offset);
        let sz = storage_size(capture_ty, ctx.func_name)?;
        offset += sz;
        closure_offset += sz;
    }

    // Store call arguments.
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

    // Allocate return buffer.
    let ret_size = storage_size(ret_type, ctx.func_name)?.max(1) as u32;
    let ret_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        ret_size,
        0,
    ));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    // Call the function pointer via call_indirect using the standard sig.
    builder
        .ins()
        .call_indirect(ctx.alloc_closure_sig, fn_ptr, &[args_buf, ret_buf]);

    let result = load_primitive_value(builder, ret_buf, 0, ret_type, ctx.func_name)?;
    Ok(result)
}

/// Compile a `TailCall` terminator.
///
/// Loads the closure's function pointer, builds the args buffer
/// (captures from the closure + call args), calls via `call_indirect`,
/// stores the return value, and returns from the current function.
fn compile_tail_call(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    callee: ValueId,
    call_args: &[ValueId],
    values: &HashMap<ValueId, Value>,
) -> Result<(), JitError> {
    let closure_val = lookup_value(values, callee, ctx.func_name)?;
    let closure_type = lookup_type(ctx.value_types, callee, ctx.func_name)?;

    let func_type = match closure_type {
        IrType::Closure(ft) => ft.as_ref().clone(),
        _ => {
            return Err(JitError::UnimplementedInstruction {
                instruction: format!("TailCall: callee is not a closure, got {closure_type}"),
                function: ctx.func_name.to_string(),
            });
        }
    };

    let fn_ptr = builder
        .ins()
        .load(types::I64, MemFlags::trusted(), closure_val, 0);

    let call_arg_count = call_args.len();
    let total_param_count = func_type.params.len();
    if call_arg_count > total_param_count {
        return Err(JitError::UnimplementedInstruction {
            instruction: format!(
                "TailCall: expected at most {total_param_count} arguments, got {call_arg_count}"
            ),
            function: ctx.func_name.to_string(),
        });
    }
    let capture_param_count = total_param_count - call_arg_count;
    let capture_types: Vec<&IrType> = func_type.params[..capture_param_count].iter().collect();
    let call_arg_types: Vec<&IrType> = call_args
        .iter()
        .map(
            |id| match lookup_type(ctx.value_types, *id, ctx.func_name) {
                Ok(t) => t,
                Err(e) => panic!("{e}"),
            },
        )
        .collect();

    let capture_total_size: i32 = capture_types
        .iter()
        .map(|ty| storage_size(ty, ctx.func_name))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();
    let call_args_total_size: i32 = call_arg_types
        .iter()
        .map(|ty| storage_size(ty, ctx.func_name))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();

    let args_buf_size = (capture_total_size + call_args_total_size).max(1) as u32;
    let arg_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        args_buf_size,
        0,
    ));
    let args_buf = builder.ins().stack_addr(types::I64, arg_slot, 0);

    let mut offset: i32 = 0;
    let mut closure_offset: i32 = 8;
    for capture_ty in &capture_types {
        let cap_val = builder.ins().load(
            storage_type(capture_ty, ctx.func_name)?,
            MemFlags::trusted(),
            closure_val,
            closure_offset,
        );
        builder
            .ins()
            .store(MemFlags::trusted(), cap_val, args_buf, offset);
        let sz = storage_size(capture_ty, ctx.func_name)?;
        offset += sz;
        closure_offset += sz;
    }

    for arg_id in call_args {
        let arg_type = lookup_type(ctx.value_types, *arg_id, ctx.func_name)?;
        if !matches!(arg_type, IrType::Unit) {
            let arg_val = lookup_value(values, *arg_id, ctx.func_name)?;
            builder
                .ins()
                .store(MemFlags::trusted(), arg_val, args_buf, offset);
        }
        offset += storage_size(arg_type, ctx.func_name)?;
    }

    let ret_size = storage_size(ctx.ret_type, ctx.func_name)?.max(1) as u32;
    let ret_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        ret_size,
        0,
    ));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);

    builder
        .ins()
        .call_indirect(ctx.alloc_closure_sig, fn_ptr, &[args_buf, ret_buf]);

    let result = load_primitive_value(builder, ret_buf, 0, ctx.ret_type, ctx.func_name)?;
    store_return_value(builder, ctx.ret_ptr, ctx.ret_type, result, ctx.func_name)?;
    let zero = builder.ins().iconst(I32, 0);
    builder.ins().return_(&[zero]);
    Ok(())
}

/// Compile a `TagConstruct` instruction.
///
/// Tag layout: `[discriminant: u32][payload packed by storage_size]`.
/// Heap-allocated via `pipe_rt_alloc_closure`.
fn compile_tag_construct(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    data: &ir::TagConstructData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let payload_sizes: Vec<i32> = data
        .payload
        .iter()
        .map(|vid| {
            let ty = lookup_type(ctx.value_types, *vid, ctx.func_name)?;
            storage_size(ty, ctx.func_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let total_size: i32 = 4 + payload_sizes.iter().sum::<i32>();

    let content_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        total_size.max(1) as u32,
        0,
    ));
    let content_buf = builder.ins().stack_addr(types::I64, content_slot, 0);

    let disc_val = builder.ins().iconst(I32, i64::from(data.discriminant));
    builder
        .ins()
        .store(MemFlags::trusted(), disc_val, content_buf, 0);

    let mut offset: i32 = 4;
    for (vid, size) in data.payload.iter().zip(payload_sizes.iter()) {
        let val = lookup_value(values, *vid, ctx.func_name)?;
        builder
            .ins()
            .store(MemFlags::trusted(), val, content_buf, offset);
        offset += size;
    }

    let args_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 12, 0));
    let args_buf = builder.ins().stack_addr(types::I64, args_slot, 0);
    let content_addr = builder.ins().stack_addr(types::I64, content_slot, 0);
    builder
        .ins()
        .store(MemFlags::trusted(), content_addr, args_buf, 0);
    let byte_size_val = builder.ins().iconst(I32, total_size as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), byte_size_val, args_buf, 8);

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);
    builder.ins().call_indirect(
        ctx.alloc_closure_sig,
        ctx.alloc_closure_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
}

/// Compile a `TagDiscriminant` instruction.
/// Loads the u32 discriminant from offset 0 of the tag heap block.
fn compile_tag_discriminant(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    value_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let tag_val = lookup_value(values, value_id, ctx.func_name)?;
    Ok(builder.ins().load(I32, MemFlags::trusted(), tag_val, 0))
}

/// Compile a `TagGet` instruction.
/// Loads the `index`-th payload field from a tag heap block.
fn compile_tag_get(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    value_id: ValueId,
    index: u32,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let tag_val = lookup_value(values, value_id, ctx.func_name)?;
    let tag_ty = lookup_type(ctx.value_types, value_id, ctx.func_name)?;
    let tag_type = match tag_ty {
        IrType::Tag(tt) => tt.clone(),
        _ => return Err(unsupported_type(ctx.func_name, tag_ty)),
    };
    let variant = tag_type
        .variants
        .first()
        .ok_or_else(|| JitError::UnimplementedInstruction {
            instruction: format!("TagGet: tag type {} has no variants", tag_type.name),
            function: ctx.func_name.to_string(),
        })?;
    let field_type =
        variant
            .payload
            .get(index as usize)
            .ok_or_else(|| JitError::UnimplementedInstruction {
                instruction: format!("TagGet: index {index} out of bounds"),
                function: ctx.func_name.to_string(),
            })?;

    let mut offset: i32 = 4;
    for i in 0..index as usize {
        offset += storage_size(&variant.payload[i], ctx.func_name)?;
    }

    Ok(builder.ins().load(
        storage_type(field_type, ctx.func_name)?,
        MemFlags::trusted(),
        tag_val,
        offset,
    ))
}

/// Compile a `RecordAlloc` instruction.
///
/// Record layout: `[fields packed by storage_size]`, no header.
/// Heap-allocated via `pipe_rt_alloc_closure`.
fn compile_record_alloc(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    data: &ir::RecordAllocData,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let field_sizes: Vec<i32> = data
        .fields
        .iter()
        .map(|vid| {
            let ty = lookup_type(ctx.value_types, *vid, ctx.func_name)?;
            storage_size(ty, ctx.func_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let total_size: i32 = field_sizes.iter().sum();

    let content_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        total_size.max(1) as u32,
        0,
    ));
    let content_buf = builder.ins().stack_addr(types::I64, content_slot, 0);

    let mut offset: i32 = 0;
    for (vid, size) in data.fields.iter().zip(field_sizes.iter()) {
        let val = lookup_value(values, *vid, ctx.func_name)?;
        builder
            .ins()
            .store(MemFlags::trusted(), val, content_buf, offset);
        offset += size;
    }

    let args_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 12, 0));
    let args_buf = builder.ins().stack_addr(types::I64, args_slot, 0);
    let content_addr = builder.ins().stack_addr(types::I64, content_slot, 0);
    builder
        .ins()
        .store(MemFlags::trusted(), content_addr, args_buf, 0);
    let byte_size_val = builder.ins().iconst(I32, total_size as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), byte_size_val, args_buf, 8);

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);
    builder.ins().call_indirect(
        ctx.alloc_closure_sig,
        ctx.alloc_closure_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
}

/// Compile a `RecordGet` instruction.
/// Loads the field at `field_index` from a record heap block.
fn compile_record_get(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    record_id: ValueId,
    field_index: u32,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let record_val = lookup_value(values, record_id, ctx.func_name)?;
    let ty = lookup_type(ctx.value_types, record_id, ctx.func_name)?;
    let record_type = match ty {
        IrType::Record(rt) => rt.clone(),
        _ => return Err(unsupported_type(ctx.func_name, ty)),
    };
    let field_type = &record_type.fields[field_index as usize].1;

    let mut offset: i32 = 0;
    for i in 0..field_index as usize {
        offset += storage_size(&record_type.fields[i].1, ctx.func_name)?;
    }

    Ok(builder.ins().load(
        storage_type(field_type, ctx.func_name)?,
        MemFlags::trusted(),
        record_val,
        offset,
    ))
}

/// Compile a `RecordSet` instruction.
/// Stores `value` at `field_index` in a record heap block, returns unit.
fn compile_record_set(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    record_id: ValueId,
    field_index: u32,
    value_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let record_val = lookup_value(values, record_id, ctx.func_name)?;
    let ty = lookup_type(ctx.value_types, record_id, ctx.func_name)?;
    let record_type = match ty {
        IrType::Record(rt) => rt.clone(),
        _ => return Err(unsupported_type(ctx.func_name, ty)),
    };
    let value = lookup_value(values, value_id, ctx.func_name)?;

    let mut offset: i32 = 0;
    for i in 0..field_index as usize {
        offset += storage_size(&record_type.fields[i].1, ctx.func_name)?;
    }

    builder
        .ins()
        .store(MemFlags::trusted(), value, record_val, offset);
    Ok(builder.ins().iconst(I32, 0))
}

/// Compile an `ArrayAlloc` instruction.
///
/// Array layout: `[len: u64 (8 bytes)][elements packed by element_size]`.
/// Heap-allocated via `pipe_rt_alloc_array`.
fn compile_array_alloc(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    len_id: ValueId,
    init_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let init_ty = lookup_type(ctx.value_types, init_id, ctx.func_name)?;
    let elem_size = storage_size(init_ty, ctx.func_name)?;

    let len_val = lookup_value(values, len_id, ctx.func_name)?;
    let init_val = lookup_value(values, init_id, ctx.func_name)?;
    let init_widened = widen_to_i64(builder, init_val, init_ty, ctx.func_name)?;

    // Build args buffer: [len: u32, element_size: u32, init_value: u64].
    let args_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 0));
    let args_buf = builder.ins().stack_addr(types::I64, args_slot, 0);

    builder
        .ins()
        .store(MemFlags::trusted(), len_val, args_buf, 0);
    let elem_size_val = builder.ins().iconst(I32, elem_size as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), elem_size_val, args_buf, 4);
    builder
        .ins()
        .store(MemFlags::trusted(), init_widened, args_buf, 8);

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);
    builder.ins().call_indirect(
        ctx.alloc_array_sig,
        ctx.alloc_array_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
}

/// Compile an `ArrayGet` instruction.
///
/// Array layout: `[len: u64 (8 bytes)][elements packed by element_size]`.
/// Loads element at dynamic `index` from the array heap block.
fn compile_array_get(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    array_id: ValueId,
    index_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let array_val = lookup_value(values, array_id, ctx.func_name)?;
    let index_val = lookup_value(values, index_id, ctx.func_name)?;

    let arr_type = lookup_type(ctx.value_types, array_id, ctx.func_name)?;
    let elem_ty = match arr_type {
        IrType::Array(et) => et.as_ref(),
        _ => return Err(unsupported_type(ctx.func_name, arr_type)),
    };
    let elem_size = storage_size(elem_ty, ctx.func_name)?;

    // offset = 8 + index * element_size
    let index_i64 = if builder.func.dfg.value_type(index_val) == types::I64 {
        index_val
    } else {
        builder.ins().uextend(types::I64, index_val)
    };
    let elem_size_val = builder.ins().iconst(types::I64, elem_size as i64);
    let byte_offset = builder.ins().imul(index_i64, elem_size_val);
    let array_data_ptr = builder.ins().iadd_imm(array_val, 8);
    let final_addr = builder.ins().iadd(array_data_ptr, byte_offset);

    Ok(builder.ins().load(
        storage_type(elem_ty, ctx.func_name)?,
        MemFlags::trusted(),
        final_addr,
        0,
    ))
}

/// Compile an `ArrayLen` instruction.
///
/// Array layout: `[len: u64 (8 bytes)][elements packed by element_size]`.
/// Loads the length from offset 0 of the array heap block.
fn compile_array_len(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    array_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let array_val = lookup_value(values, array_id, ctx.func_name)?;
    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), array_val, 0))
}

/// Compile an `ArraySet` instruction.
///
/// Array layout: `[len: u64 (8 bytes)][elements packed by element_size]`.
/// Stores `value` at dynamic `index` in the array heap block.
/// Returns Unit.
fn compile_array_set(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    array_id: ValueId,
    index_id: ValueId,
    value_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let array_val = lookup_value(values, array_id, ctx.func_name)?;
    let index_val = lookup_value(values, index_id, ctx.func_name)?;
    let value_val = lookup_value(values, value_id, ctx.func_name)?;

    let arr_type = lookup_type(ctx.value_types, array_id, ctx.func_name)?;
    let elem_ty = match arr_type {
        IrType::Array(et) => et.as_ref(),
        _ => return Err(unsupported_type(ctx.func_name, arr_type)),
    };
    let elem_size = storage_size(elem_ty, ctx.func_name)?;

    // Compute address: base + 8 + index * element_size
    let index_i64 = if builder.func.dfg.value_type(index_val) == types::I64 {
        index_val
    } else {
        builder.ins().uextend(types::I64, index_val)
    };
    let elem_size_val = builder.ins().iconst(types::I64, elem_size as i64);
    let byte_offset = builder.ins().imul(index_i64, elem_size_val);
    let array_data_ptr = builder.ins().iadd_imm(array_val, 8);
    let final_addr = builder.ins().iadd(array_data_ptr, byte_offset);

    builder
        .ins()
        .store(MemFlags::trusted(), value_val, final_addr, 0);

    Ok(builder.ins().iconst(I32, 0))
}

/// Compile an `ArrayConcat` instruction.
///
/// Allocates a new array whose contents are the concatenation of
/// `left` and `right`, using the `pipe_rt_array_concat` runtime helper.
fn compile_array_concat(
    builder: &mut FunctionBuilder,
    ctx: &BlockContext,
    left_id: ValueId,
    right_id: ValueId,
    values: &HashMap<ValueId, Value>,
) -> Result<Value, JitError> {
    let left_val = lookup_value(values, left_id, ctx.func_name)?;
    let right_val = lookup_value(values, right_id, ctx.func_name)?;

    let arr_type = lookup_type(ctx.value_types, left_id, ctx.func_name)?;
    let elem_ty = match arr_type {
        IrType::Array(et) => et.as_ref(),
        _ => return Err(unsupported_type(ctx.func_name, arr_type)),
    };
    let elem_size = storage_size(elem_ty, ctx.func_name)?;

    // Build args buffer: [left_ptr: u64, right_ptr: u64, element_size: u32].
    let args_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 20, 0));
    let args_buf = builder.ins().stack_addr(types::I64, args_slot, 0);

    builder
        .ins()
        .store(MemFlags::trusted(), left_val, args_buf, 0);
    builder
        .ins()
        .store(MemFlags::trusted(), right_val, args_buf, 8);
    let elem_size_val = builder.ins().iconst(I32, elem_size as i64);
    builder
        .ins()
        .store(MemFlags::trusted(), elem_size_val, args_buf, 16);

    let ret_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ret_buf = builder.ins().stack_addr(types::I64, ret_slot, 0);
    builder.ins().call_indirect(
        ctx.array_concat_sig,
        ctx.array_concat_fn_ptr,
        &[args_buf, ret_buf],
    );

    Ok(builder
        .ins()
        .load(types::I64, MemFlags::trusted(), ret_buf, 0))
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
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_actual_return_types: &HashMap<String, IrType>,
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
                let ty = infer_instruction_type(
                    inst,
                    &types,
                    func.name.as_ref(),
                    fn_return_types,
                    fn_param_types,
                    fn_actual_return_types,
                )?;
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
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_actual_return_types: &HashMap<String, IrType>,
) -> Result<IrType, JitError> {
    // Override MakeClosure inference to include proper param types
    // (capture types + declared param types) in the FuncType.
    if let ir::Instruction::MakeClosure(data) = inst {
        let func_type = if let Some(param_tys) = fn_param_types.get(data.func_name.as_str()) {
            let ret = fn_return_types
                .get(data.func_name.as_str())
                .cloned()
                .unwrap_or(IrType::Unit);
            ir::FuncType {
                params: param_tys.clone(),
                ret: Box::new(ret),
            }
        } else {
            ir::FuncType {
                params: vec![],
                ret: Box::new(IrType::Unit),
            }
        };
        return Ok(IrType::Closure(Box::new(func_type)));
    }
    // Override CallNamed inference for closures: use fn_actual_return_types
    // to preserve full capture params in the closure FuncType.
    if let ir::Instruction::CallNamed(data) = inst {
        let base_ty =
            ir::infer_instruction_type(inst, types, fn_return_types).ok_or_else(|| {
                JitError::UnimplementedInstruction {
                    instruction: format!("{inst:?}"),
                    function: func_name.to_string(),
                }
            })?;
        if matches!(base_ty, IrType::Closure(_))
            && let Some(actual_ty) = fn_actual_return_types.get(data.name.as_str())
        {
            return Ok(actual_ty.clone());
        }
        return Ok(base_ty);
    }
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

fn storage_type(ty: &IrType, _func_name: &str) -> Result<Type, JitError> {
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
        IrType::Array(_)
        | IrType::Record(_)
        | IrType::Closure(_)
        | IrType::Func(_)
        | IrType::Tag(_)
        | IrType::Effect(_) => Ok(types::I64),
    }
}

fn storage_size(ty: &IrType, _func_name: &str) -> Result<i32, JitError> {
    match ty {
        IrType::I8 | IrType::U8 | IrType::Bool => Ok(1),
        IrType::I16 | IrType::U16 => Ok(2),
        IrType::I32 | IrType::U32 | IrType::F32 | IrType::Unit => Ok(4),
        IrType::I64 | IrType::U64 | IrType::Usize | IrType::F64 | IrType::Str => Ok(8),
        IrType::Array(_)
        | IrType::Record(_)
        | IrType::Closure(_)
        | IrType::Func(_)
        | IrType::Tag(_)
        | IrType::Effect(_) => Ok(8),
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
        IrType::Unit
        | IrType::Effect(_)
        | IrType::Array(_)
        | IrType::Closure(_)
        | IrType::Tag(_)
        | IrType::Record(_)
        | IrType::Str
        | IrType::I64
        | IrType::U64
        | IrType::F32
        | IrType::F64 => Ok(0),
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
/// External function called by `Println` JIT code (when emitted).
/// Uses `libc::write` to respect `dup2` redirection for test capture.
#[unsafe(no_mangle)]
unsafe extern "C" fn __pipe_println(args: *const u8, ret: *mut u8) -> i32 {
    let raw = unsafe { std::ptr::read_unaligned(args as *const i64) };
    let type_tag = unsafe { std::ptr::read_unaligned(args.add(8) as *const u32) };
    let s = match type_tag {
        0 => format!("{}", raw as i8),
        1 => format!("{}", raw as i16),
        2 => format!("{}", raw as i32),
        3 => format!("{}", raw),
        4 => format!("{}", raw as u8),
        5 => format!("{}", raw as u16),
        6 => format!("{}", raw as u32),
        7 => format!("{}", raw as u64),
        8 => format!("{}", f32::from_bits(raw as u32)),
        9 => format!("{}", f64::from_bits(raw as u64)),
        10 => format!("{}", raw != 0),
        11 => {
            let ptr = raw as *const u8;
            let len = unsafe { std::ptr::read_unaligned(ptr as *const u32) } as usize;
            let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
            let s = unsafe { std::str::from_utf8_unchecked(bytes) };
            s.to_string()
        }
        12 => "()".to_string(),
        13 => "<array>".to_string(),
        14 => "<record>".to_string(),
        15 => "<effect>".to_string(),
        16 => "<closure>".to_string(),
        17 => "<tag>".to_string(),
        _ => String::new(),
    };
    let output = if s.is_empty() { s } else { s + "\n" };
    unsafe {
        libc::write(1, output.as_ptr() as *const libc::c_void, output.len());
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
            12 => result.push_str("()"),
            13 => result.push_str("<array>"),
            14 => result.push_str("<record>"),
            15 => result.push_str("<effect>"),
            16 => result.push_str("<closure>"),
            17 => result.push_str("<tag>"),
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

/// External function called by `MakeClosure` JIT code.
///
/// Allocates a heap-allocated closure object containing the function
/// pointer followed by packed capture values, and writes the pointer
/// to `ret`.
///
/// # Safety
///
/// `args` points to a buffer in this layout:
///   - bytes 0–7:  `u64` pointer to closure content data
///   - bytes 8–11: `u32` byte size of the closure content data
///
/// `ret` points to an 8-byte buffer that receives the pointer to
/// the heap-allocated closure object.
#[unsafe(no_mangle)]
unsafe extern "C" fn pipe_rt_alloc_closure(args: *const u8, ret: *mut u8) -> i32 {
    let data_ptr = unsafe { std::ptr::read_unaligned(args as *const u64) } as *const u8;
    let byte_size = unsafe { std::ptr::read_unaligned(args.add(8) as *const u32) } as usize;
    let closure = Vec::from(unsafe { std::slice::from_raw_parts(data_ptr, byte_size) });
    let ptr = Box::leak(closure.into_boxed_slice()).as_ptr();
    unsafe {
        *(ret as *mut u64) = ptr as u64;
    }
    0
}

/// External function called by `ArrayAlloc` JIT code.
///
/// Allocates a heap-allocated array block with the layout:
///   [len: u64 (8 bytes)][elements packed by element_size]
/// and writes the pointer to `ret`.
///
/// # Safety
///
/// `args` points to a buffer in this layout:
///   - bytes 0–3:   `u32` length (number of elements)
///   - bytes 4–7:   `u32` element size in bytes
///   - bytes 8–15:  `u64` initial value bytes (only `element_size` bytes used)
///
/// `ret` points to an 8-byte buffer that receives the pointer to
/// the heap-allocated array.
#[unsafe(no_mangle)]
unsafe extern "C" fn pipe_rt_alloc_array(args: *const u8, ret: *mut u8) -> i32 {
    let len = unsafe { std::ptr::read_unaligned(args as *const u32) } as usize;
    let element_size = unsafe { std::ptr::read_unaligned(args.add(4) as *const u32) } as usize;
    let init_raw = unsafe { std::ptr::read_unaligned(args.add(8) as *const u64) };

    let total_size = 8 + len * element_size;
    let mut buf = vec![0u8; total_size];

    unsafe { std::ptr::write_unaligned(buf.as_mut_ptr() as *mut u64, len as u64) };

    let data_ptr = unsafe { buf.as_mut_ptr().add(8) };
    let init_bytes = &init_raw as *const u64 as *const u8;
    for i in 0..len {
        unsafe {
            std::ptr::copy_nonoverlapping(init_bytes, data_ptr.add(i * element_size), element_size);
        }
    }

    let ptr = Box::leak(buf.into_boxed_slice()).as_ptr();
    unsafe {
        *(ret as *mut u64) = ptr as u64;
    }
    0
}

/// External function called by `ArrayConcat` JIT code.
///
/// Takes a left array pointer, a right array pointer, and an element
/// size. Allocates a new array whose contents are the concatenation of
/// the two input arrays, and writes the pointer to `ret`.
///
/// Args buffer layout:
///   - bytes 0–7:  `u64` left array pointer
///   - bytes 8–15: `u64` right array pointer
///   - bytes 16–19: `u32` element size in bytes
///
/// Ret buffer: 8-byte pointer to the new array.
///
/// # Safety
///
/// `args` must point to a valid 20-byte buffer. `ret` must point to an
/// 8-byte buffer. Both arrays must have been allocated by the same
/// allocator with the same element size.
#[unsafe(no_mangle)]
unsafe extern "C" fn pipe_rt_array_concat(args: *const u8, ret: *mut u8) -> i32 {
    let left_ptr = unsafe { std::ptr::read_unaligned(args as *const u64) } as *const u8;
    let right_ptr = unsafe { std::ptr::read_unaligned(args.add(8) as *const u64) } as *const u8;
    let element_size = unsafe { std::ptr::read_unaligned(args.add(16) as *const u32) } as usize;

    let left_len = unsafe { std::ptr::read_unaligned(left_ptr as *const u64) } as usize;
    let right_len = unsafe { std::ptr::read_unaligned(right_ptr as *const u64) } as usize;
    let total_len = left_len + right_len;

    let total_size = 8 + total_len * element_size;
    let mut buf = vec![0u8; total_size];

    unsafe { std::ptr::write_unaligned(buf.as_mut_ptr() as *mut u64, total_len as u64) };

    let left_data = unsafe { left_ptr.add(8) };
    let right_data = unsafe { right_ptr.add(8) };
    let left_bytes = left_len * element_size;
    let dst = unsafe { buf.as_mut_ptr().add(8) };
    unsafe {
        std::ptr::copy_nonoverlapping(left_data, dst, left_bytes);
        std::ptr::copy_nonoverlapping(right_data, dst.add(left_bytes), right_len * element_size);
    }

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
        IrType::Array(_) => Some(13),
        IrType::Record(_) => Some(14),
        IrType::Effect(_) => Some(15),
        IrType::Closure(_) => Some(16),
        IrType::Tag(_) => Some(17),
        IrType::Func(_) => None,
    }
}

/// Runtime helper dispatched by the JIT when `CallNamed` targets a
/// registered builtin instead of a local function.
///
/// # Safety
///
/// `args` points to a buffer in this layout:
///   - bytes 0–7:   `u64` pointer to len-prefixed name string data
///   - bytes 8–11:  `u32` name length (redundant, for validation)
///   - bytes 12–15: `u32` count of arguments
///   - bytes 16+:   for each arg: value as `i64` (8 bytes) then type tag
///     as `u32` (4 bytes)
///
/// `ret` points to a 12-byte buffer receiving the result in
/// `[val: i64, tag: u32]` layout.
#[unsafe(no_mangle)]
unsafe extern "C" fn pipe_rt_call_builtin(args: *const u8, ret: *mut u8) -> i32 {
    use crate::value::Value as RuntimeValue;

    let name_ptr = unsafe { std::ptr::read_unaligned(args as *const u64) };
    let name_len = unsafe { std::ptr::read_unaligned(args.add(8) as *const u32) } as usize;
    let arg_count = unsafe { std::ptr::read_unaligned(args.add(12) as *const u32) } as usize;

    let name_bytes =
        unsafe { std::slice::from_raw_parts((name_ptr as *const u8).add(4), name_len) };
    let name = unsafe { std::str::from_utf8_unchecked(name_bytes) };

    let mut vals = Vec::with_capacity(arg_count);
    for i in 0..arg_count {
        let base = unsafe { args.add(16 + i * 12) };
        let raw = unsafe { std::ptr::read_unaligned(base as *const i64) };
        let tag = unsafe { std::ptr::read_unaligned(base.add(8) as *const u32) };
        let value = match tag {
            0..=2 => RuntimeValue::I32(raw as i32),
            3 => RuntimeValue::I64(raw),
            4..=5 => RuntimeValue::I32(raw as i32),
            6 => RuntimeValue::I64(i64::from(raw as u32)),
            7 => RuntimeValue::I64(raw),
            8 => RuntimeValue::F64(f64::from(f32::from_bits(raw as u32))),
            9 => RuntimeValue::F64(f64::from_bits(raw as u64)),
            10 => RuntimeValue::Bool(raw != 0),
            11 => {
                let ptr = raw as *const u8;
                if ptr.is_null() || (raw as u64) < 0x1000 {
                    RuntimeValue::Unit
                } else {
                    let len = unsafe { std::ptr::read_unaligned(ptr as *const u32) } as usize;
                    let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
                    let s = unsafe { std::str::from_utf8_unchecked(bytes) };
                    RuntimeValue::str(s.to_owned())
                }
            }
            13 | 14 | 15 | 17 => {
                if raw == 0 || (raw as u64) < 0x1000 {
                    RuntimeValue::Unit
                } else {
                    let val = unsafe { &*(raw as *const RuntimeValue) };
                    val.clone()
                }
            }
            _ => RuntimeValue::Unit,
        };
        vals.push(value);
    }

    let result = crate::bridge::global_registry().execute(name, &vals);
    match result {
        Ok(value) => {
            value_to_ret_buf(value, ret);
            0
        }
        Err(msg) => {
            unsafe {
                std::ptr::write_unaligned(ret as *mut i64, 0i64);
                std::ptr::write_unaligned(ret.add(8) as *mut u32, 0u32);
            }
            tracing::error!("builtin `{name}` failed: {msg}");
            1
        }
    }
}

/// Encodes a [`crate::value::Value`] into the 12-byte `[val: i64, tag: u32]`
/// ret buffer expected by the JIT's builtin call bridge.
fn value_to_ret_buf(value: crate::value::Value, ret: *mut u8) {
    use crate::value::Value as RuntimeValue;
    let (raw, tag): (i64, u32) = match value {
        RuntimeValue::I32(n) => (n as i64, 2),
        RuntimeValue::I64(n) => (n, 3),
        RuntimeValue::F64(f) => (f.to_bits() as i64, 9),
        RuntimeValue::Bool(b) => (i64::from(b), 10),
        RuntimeValue::Unit => (0, 12),
        RuntimeValue::Str(s) => {
            let bytes = s.as_bytes();
            let len = bytes.len() as u32;
            let mut buf = Vec::with_capacity(4 + bytes.len());
            buf.extend_from_slice(&len.to_ne_bytes());
            buf.extend_from_slice(bytes);
            let ptr = Box::leak(buf.into_boxed_slice()).as_ptr();
            (ptr as i64, 11)
        }
        RuntimeValue::Array(_) => (0, 13),
        RuntimeValue::Record(_) => (0, 14),
        RuntimeValue::Closure(c) => {
            let ptr = Box::into_raw(Box::new(RuntimeValue::Closure(c))) as i64;
            (ptr, 16)
        }
        RuntimeValue::Tag { tag, payload } => {
            let ptr = Box::into_raw(Box::new(RuntimeValue::Tag { tag, payload })) as i64;
            (ptr, 17)
        }
        RuntimeValue::Effect(_) => (0, 15),
    };
    unsafe {
        std::ptr::write_unaligned(ret as *mut i64, raw);
        std::ptr::write_unaligned(ret.add(8) as *mut u32, tag);
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
        let val = push_inst(
            &mut func,
            &mut entry,
            ir::Instruction::ConstF64(std::f64::consts::PI),
        );
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
