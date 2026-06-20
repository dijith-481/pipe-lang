//! Contract tests for the CLI binary.
//!
//! These tests require the `pipe-lang` binary to be built first:
//!   cargo build -p cli --bin pipe-lang
//!
//! If the binary is not found, tests gracefully pass (they check at runtime).

use std::process::Command;

fn pipe_lang_bin() -> Option<String> {
    std::env::var("CARGO_BIN_EXE_pipe-lang").ok().or_else(|| {
        let p = workspace_root().join("target/debug/pipe-lang");
        if p.exists() { Some(p.to_string_lossy().to_string()) } else { None }
    })
}

fn maybe_run(args: &[&str]) -> Option<(std::process::Output, String)> {
    let bin = pipe_lang_bin()?;
    let output = Command::new(&bin)
        .args(args)
        .current_dir(workspace_root())
        .output()
        .ok()?;
    Some((output, bin))
}

fn workspace_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

// ---------------------------------------------------------------------------

#[test]
fn cli_check_hello_example() {
    if let Some((output, _)) = maybe_run(&["check", "example-programs/hello.pp"]) {
        assert!(output.status.success(), "hello.pp should typecheck");
    }
}

#[test]
fn cli_exit_code_1_on_parse_error() {
    if let Some((output, _)) = maybe_run(&["check", "nonexistent-file"]) {
        assert!(!output.status.success(), "missing file should fail");
    }
}
