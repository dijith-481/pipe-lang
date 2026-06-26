pub mod bridge;
pub mod error;
pub mod jit;
pub mod value;

pub use crate::bridge::{
    BuiltinFunction, BuiltinRegistry, clear_global_registry, execute_builtin, expect_arity,
    init_global_registry,
};
pub use crate::error::RuntimeError;
pub use crate::jit::{CompiledModule, JitError, compile_ir};
pub use crate::value::{
    ClosureData, FuncPtr, JitArgType, RecordData, Value,
};

use std::sync::Mutex;

/// Global output capture buffer used by test infrastructure.
/// When `Some`, builtins append output here instead of writing to fd 1.
static CAPTURE_BUF: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Enable global output capture into a buffer.
pub fn enable_capture() {
    *CAPTURE_BUF.lock().unwrap() = Some(Vec::new());
}

/// Disable capture and return captured output as a string.
pub fn disable_capture() -> String {
    let mut buf = CAPTURE_BUF.lock().unwrap();
    let result = buf.take().unwrap_or_default();
    String::from_utf8(result).unwrap_or_default()
}

/// Write a string to stdout or the capture buffer.
pub fn write_stdout(s: &str) {
    let mut guard = CAPTURE_BUF.lock().unwrap();
    if let Some(buf) = guard.as_mut() {
        buf.extend_from_slice(s.as_bytes());
    } else {
        unsafe {
            libc::write(1, s.as_ptr() as *const libc::c_void, s.len());
        }
    }
}
