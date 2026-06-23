/// Standard library for pipe-lang.
///
/// Provides built-in functions that the runtime can execute.
/// These are implemented in pure Rust and exposed to the language
/// via the `BuiltinFunction` trait.
pub mod array;
mod closure;
pub mod io;
pub mod numeric;
pub mod ops;
pub mod option;
pub mod prelude;
pub mod result;
pub mod str;

pub fn version() -> &'static str {
    "0.1.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdlib_version_is_set() {
        assert!(!version().is_empty());
    }
}
