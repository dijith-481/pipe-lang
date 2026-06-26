use runtime::{
    ClosureData, FuncPtr, Value,
};

unsafe extern "C" {
    fn pipe_rt_unbox_value_jit(ptr: u64, desc_ptr: u64, desc_len: u32) -> u64;
    fn pipe_rt_box_value_jit(ptr: u64, desc_ptr: u64, desc_len: u32) -> u64;
    fn pipe_rt_release_ptr_exported(ptr: u64, type_desc: *const u8);
}

fn is_tag_heap_type(tag: u32) -> bool {
    matches!(tag, 11 | 13 | 14 | 15 | 16 | 17)
}

fn serialize_value_to_jit_arg(val: &Value, desc: &[u8], tag: u32) -> Vec<u8> {
    if is_tag_heap_type(tag) {
        let boxed = Box::new(val.clone());
        let ptr = Box::into_raw(boxed) as u64;
        let jit_ptr = unsafe { pipe_rt_unbox_value_jit(ptr, desc.as_ptr() as u64, desc.len() as u32) };
        jit_ptr.to_le_bytes().to_vec()
    } else {
        match tag {
            0 => vec![match val { Value::I32(n) => *n as u8, _ => 0 }],
            1 => (match val { Value::I32(n) => *n as i16, _ => 0 }).to_le_bytes().to_vec(),
            2 => (match val { Value::I32(n) => *n, _ => 0 }).to_le_bytes().to_vec(),
            3 => (match val { Value::I64(n) => *n, Value::I32(n) => *n as i64, Value::Usize(n) => *n as i64, _ => 0 }).to_le_bytes().to_vec(),
            4 => vec![match val { Value::I32(n) => *n as u8, _ => 0 }],
            5 => (match val { Value::I32(n) => *n as u16, _ => 0 }).to_le_bytes().to_vec(),
            6 => (match val { Value::I32(n) => *n as u32, _ => 0 }).to_le_bytes().to_vec(),
            7 => (match val { Value::I64(n) => *n as u64, Value::I32(n) => *n as u64, Value::Usize(n) => *n as u64, _ => 0 }).to_le_bytes().to_vec(),
            8 => (match val { Value::F64(f) => *f as f32, _ => 0.0 }).to_bits().to_le_bytes().to_vec(),
            9 => (match val { Value::F64(f) => *f, _ => 0.0 }).to_bits().to_le_bytes().to_vec(),
            10 => vec![match val { Value::Bool(b) => *b as u8, _ => 0 }],
            12 => vec![0, 0, 0, 0],
            _ => vec![0; 8],
        }
    }
}

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
            call_jit_fn(*address, &closure.captures, args, &closure.param_descs, &closure.ret_desc)
        }
    }
}

fn call_jit_fn(
    address: usize,
    captures: &[Value],
    call_args: &[Value],
    param_descs: &[Vec<u8>],
    ret_desc: &[u8],
) -> Result<Value, String> {
    let mut args_buf = vec![0u8; (captures.len() + call_args.len()) * 8];
    let mut offset = 0;
    let mut heap_args = Vec::new();

    for (i, cap) in captures.iter().enumerate() {
        if i < param_descs.len() {
            let desc = &param_descs[i];
            let tag = u32::from_le_bytes(desc[0..4].try_into().unwrap());
            let bytes = serialize_value_to_jit_arg(cap, desc, tag);
            args_buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
            if is_tag_heap_type(tag) {
                let jit_ptr = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                heap_args.push((jit_ptr, desc.as_ptr()));
            }
            offset += 8;
        }
    }

    for (i, arg) in call_args.iter().enumerate() {
        if i < param_descs.len() {
            let desc = &param_descs[i];
            let tag = u32::from_le_bytes(desc[0..4].try_into().unwrap());
            let bytes = serialize_value_to_jit_arg(arg, desc, tag);
            args_buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
            if is_tag_heap_type(tag) {
                let jit_ptr = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                heap_args.push((jit_ptr, desc.as_ptr()));
            }
            offset += 8;
        }
    }

    let mut ret_buf = vec![0u8; 12];
    let func: unsafe extern "C" fn(*const u8, *mut u8) -> i32 = unsafe { std::mem::transmute(address) };
    unsafe {
        func(args_buf.as_ptr(), ret_buf.as_mut_ptr());
    }

    for (ptr, desc_ptr) in heap_args {
        unsafe { pipe_rt_release_ptr_exported(ptr, desc_ptr); }
    }

    if ret_desc.is_empty() {
        return Ok(Value::Unit);
    }

    let ret_tag = u32::from_le_bytes(ret_desc[0..4].try_into().unwrap());
    let mut raw_bytes = [0u8; 8];
    raw_bytes.copy_from_slice(&ret_buf[..8.min(ret_buf.len())]);
    let raw_val = u64::from_le_bytes(raw_bytes);

    if is_tag_heap_type(ret_tag) {
        if raw_val == 0 {
            return Ok(Value::Unit);
        }
        let box_ptr = unsafe { pipe_rt_box_value_jit(raw_val, ret_desc.as_ptr() as u64, ret_desc.len() as u32) };
        let box_val = unsafe { Box::from_raw(box_ptr as *mut Value) };
        let result = (*box_val).clone();
        unsafe { pipe_rt_release_ptr_exported(raw_val, ret_desc.as_ptr()); }
        Ok(result)
    } else {
        match ret_tag {
            0 => Ok(Value::I32(raw_val as i8 as i32)),
            1 => Ok(Value::I32(raw_val as i16 as i32)),
            2 => Ok(Value::I32(raw_val as i32)),
            3 => Ok(Value::I64(raw_val as i64)),
            4 => Ok(Value::I32(raw_val as u8 as i32)),
            5 => Ok(Value::I32(raw_val as u16 as i32)),
            6 => Ok(Value::I32(raw_val as u32 as i32)),
            7 => Ok(Value::I64(raw_val as i64)),
            8 => Ok(Value::F64(f32::from_bits(raw_val as u32) as f64)),
            9 => Ok(Value::F64(f64::from_bits(raw_val))),
            10 => Ok(Value::Bool(raw_val != 0)),
            12 => Ok(Value::Unit),
            _ => Ok(Value::Unit),
        }
    }
}
