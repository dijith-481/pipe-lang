//! pipe-lang CLI library.
//!
//! Exposes the test infrastructure (`fixtures`) for use by integration
//! tests in `crates/cli/tests/`. The actual CLI binary lives in
//! `main.rs`; this file exists so tests can `use cli::fixtures`.

pub mod fixtures;
pub mod session;
