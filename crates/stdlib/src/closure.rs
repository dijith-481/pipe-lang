use runtime::{ClosureData, Value};

pub(crate) type ClosureThunk = unsafe extern "C" fn(*const u8, *mut u8) -> i32;

pub(crate) fn call_closure(closure: &ClosureData, args: &[Value]) -> Result<Value, String> {
    if args.len() != closure.arity {
        return Err(format!(
            "closure expected {} argument(s), got {}",
            closure.arity,
            args.len()
        ));
    }
    if closure.func_ptr == 0 {
        return Err("closure function pointer is null".to_string());
    }

    let capacity = closure
        .captures
        .len()
        .checked_add(args.len())
        .ok_or_else(|| "closure argument length overflow".to_string())?;
    let mut call_args = Vec::with_capacity(capacity);
    call_args.extend(closure.captures.iter().cloned());
    call_args.extend_from_slice(args);

    // SAFETY: func_ptr is guaranteed to be a valid function pointer by the
    // closure construction site.
    let function: ClosureThunk = unsafe { std::mem::transmute(closure.func_ptr) };

    let mut result = Value::Unit;
    let code = unsafe {
        function(
            call_args.as_ptr().cast::<u8>(),
            std::ptr::addr_of_mut!(result).cast::<u8>(),
        )
    };
    if code == 0 {
        Ok(result)
    } else {
        Err(format!("closure returned error code {code}"))
    }
}
