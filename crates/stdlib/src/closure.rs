use std::sync::Arc;

use runtime::{ClosureData, FuncPtr, JitArgType, Value, lookup_jit_param_types};

/// Calls a closure with the given arguments.
///
/// Dispatches through [`FuncPtr`] — builtins are called via their
/// [`BuiltinFunction`](runtime::BuiltinFunction) trait, JIT closures
/// use the native calling convention.
pub(crate) fn call_closure(closure: &ClosureData, args: &[Value]) -> Result<Value, String> {
    if args.len() != closure.arity {
        return Err(format!(
            "closure expected {} argument(s), got {}",
            closure.arity,
            args.len()
        ));
    }
    match &closure.func {
        FuncPtr::Builtin(function) => function.execute(args),
        FuncPtr::Jit { address, .. } => {
            let param_types = lookup_jit_param_types(*address);
            let types = if closure.call_arg_types.is_empty() {
                param_types
            } else {
                closure.call_arg_types.to_vec()
            };
            call_jit_fn(*address, &closure.captures, args, &types)
        }
    }
}

/// Call a JIT-compiled function with captures and arguments.
fn call_jit_fn(
    address: usize,
    captures: &[Value],
    call_args: &[Value],
    call_arg_types: &[JitArgType],
) -> Result<Value, String> {
    let func: unsafe extern "C" fn(*const u8, *mut u8) -> i32 =
        unsafe { std::mem::transmute(address) };

    let capture_slot_size: usize = captures.iter().map(|_| 8).sum();
    let call_slot_size: usize = call_arg_types.iter().map(|t| t.slot_size()).sum();
    let buf_size = (capture_slot_size + call_slot_size).max(1);

    let mut args_buf = vec![0u8; buf_size];
    let mut offset = 0;

    for cap in captures {
        let bytes = value_to_raw_bytes(cap);
        let slot = &mut args_buf[offset..offset + 8];
        let copy_len = bytes.len().min(8);
        slot[..copy_len].copy_from_slice(&bytes[..copy_len]);
        offset += 8;
    }

    for (i, arg) in call_args.iter().enumerate() {
        let arg_type = call_arg_types.get(i).copied().unwrap_or(JitArgType::I64);
        let bytes = value_to_raw_bytes_for_type(arg, arg_type);
        let slot_size = arg_type.slot_size();
        let slot = &mut args_buf[offset..offset + slot_size];
        let copy_len = bytes.len().min(slot_size);
        slot[..copy_len].copy_from_slice(&bytes[..copy_len]);
        offset += slot_size;
    }

    let mut ret_buf = vec![0u8; 12];
    unsafe {
        func(args_buf.as_ptr(), ret_buf.as_mut_ptr());
    }

    let raw_val = u64::from_le_bytes(ret_buf[0..8].try_into().unwrap());
    let tag = u32::from_le_bytes(ret_buf[8..12].try_into().unwrap());
    raw_jit_to_value(raw_val, tag)
}

/// Convert a Value to raw bytes for the JIT args buffer.
fn value_to_raw_bytes(v: &Value) -> Vec<u8> {
    match v {
        Value::I32(n) => n.to_le_bytes().to_vec(),
        Value::I64(n) => n.to_le_bytes().to_vec(),
        Value::Usize(n) => (*n as u64).to_le_bytes().to_vec(),
        Value::F64(f) => f.to_bits().to_le_bytes().to_vec(),
        Value::Bool(b) => vec![*b as u8, 0, 0, 0, 0, 0, 0, 0],
        Value::Unit => vec![0; 8],
        Value::Str(s) => {
            let data_ptr = Arc::as_ptr(s) as *const u8;
            let jit_ptr = unsafe { data_ptr.add(8) } as u64;
            jit_ptr.to_le_bytes().to_vec()
        }
        Value::Array(arr) => {
            let ptr = Arc::as_ptr(arr) as *const u8 as u64;
            ptr.to_le_bytes().to_vec()
        }
        Value::Record(r) => {
            let ptr = Arc::as_ptr(r) as *const u8 as u64;
            ptr.to_le_bytes().to_vec()
        }
        Value::Closure(c) => {
            let ptr = Arc::as_ptr(c) as *const u8 as u64;
            ptr.to_le_bytes().to_vec()
        }
        Value::Tag { .. } => {
            let boxed = Box::new(v.clone());
            let ptr = Box::leak(boxed) as *const Value as *const u8 as u64;
            ptr.to_le_bytes().to_vec()
        }
        Value::Effect(e) => {
            let ptr = Arc::as_ptr(e) as *const u8 as u64;
            ptr.to_le_bytes().to_vec()
        }
    }
}

/// Convert a Value to raw bytes for a specific JIT arg type.
fn value_to_raw_bytes_for_type(v: &Value, arg_type: JitArgType) -> Vec<u8> {
    match arg_type {
        JitArgType::I8 => vec![match v {
            Value::I32(n) => *n as u8,
            _ => 0,
        }],
        JitArgType::I16 => (match v {
            Value::I32(n) => *n as i16,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::I32 => (match v {
            Value::I32(n) => *n,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::I64 => (match v {
            Value::I64(n) => *n,
            Value::I32(n) => *n as i64,
            Value::Usize(n) => *n as i64,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::U8 => vec![match v {
            Value::I32(n) => *n as u8,
            _ => 0,
        }],
        JitArgType::U16 => (match v {
            Value::I32(n) => *n as u16,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::U32 => (match v {
            Value::I32(n) => *n as u32,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::U64 => (match v {
            Value::I64(n) => *n as u64,
            Value::I32(n) => *n as u64,
            _ => 0,
        })
        .to_le_bytes()
        .to_vec(),
        JitArgType::F32 => (match v {
            Value::F64(f) => *f as f32,
            _ => 0.0,
        })
        .to_bits()
        .to_le_bytes()
        .to_vec(),
        JitArgType::F64 => (match v {
            Value::F64(f) => *f,
            _ => 0.0,
        })
        .to_bits()
        .to_le_bytes()
        .to_vec(),
        JitArgType::Bool => vec![match v {
            Value::Bool(b) => *b as u8,
            _ => 0,
        }],
        JitArgType::Unit => vec![0; 8],
        JitArgType::Str
        | JitArgType::Array
        | JitArgType::Record
        | JitArgType::Effect
        | JitArgType::Closure
        | JitArgType::Tag => value_to_raw_bytes(v),
    }
}

/// Read a raw JIT return value back into a Value.
fn raw_jit_to_value(raw: u64, tag: u32) -> Result<Value, String> {
    match tag {
        0 => Ok(Value::I32(raw as i8 as i32)),
        1 => Ok(Value::I32(raw as i16 as i32)),
        2 => Ok(Value::I32(raw as i32)),
        3 => Ok(Value::I64(raw as i64)),
        4 => Ok(Value::I32(raw as u8 as i32)),
        5 => Ok(Value::I32(raw as u16 as i32)),
        6 => Ok(Value::I32(raw as u32 as i32)),
        7 => Ok(Value::I64(raw as i64)),
        8 => Ok(Value::F64(f32::from_bits(raw as u32) as f64)),
        9 => Ok(Value::F64(f64::from_bits(raw))),
        10 => Ok(Value::Bool(raw != 0)),
        11 => {
            let ptr = raw as *const u8;
            if ptr.is_null() {
                return Ok(Value::Str(Arc::from("")));
            }
            let len = unsafe { std::ptr::read_unaligned(ptr as *const u32) } as usize;
            let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
            Ok(Value::Str(Arc::from(
                std::str::from_utf8(bytes).unwrap_or(""),
            )))
        }
        12 => Ok(Value::Unit),
        13..=17 => Ok(Value::Unit),
        _ => Ok(Value::Unit),
    }
}
