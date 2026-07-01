//! Test infrastructure for end-to-end pipe-lang compilation.
//!
//! Provides:
//! - [`Fixture`] — a single `.pp` source file and (optionally) a
//!   `.expected.txt` golden output file.
//! - [`discover_fixtures`] — walks `example-programs/` and returns
//!   one `Fixture` per `.pp` file.
//! - [`run_fixture`] — runs a fixture through the full
//!   lex → parse → typecheck → lower → JIT → execute pipeline,
//!   captures stdout, and returns it for diffing.
//! - [`assert_stdout_matches`] — compares actual output against the
//!   golden file with a readable diff message on mismatch.
//!
//! The infrastructure is **inert** until Track A (parser/typechecker/lowerer)
//! and Track B (JIT completion) deliver their parts. For now, the
//! individual test functions verify only the loader and diffing
//! logic; the end-to-end test is added on Day 10.

use std::path::{Path, PathBuf};

/// A single test fixture: source file + expected stdout.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Path to the `.pp` source file.
    pub source: PathBuf,
    /// Stem of the file (e.g. `"hello"` for `hello.pp`).
    pub name: String,
    /// Path to the golden output file, if it exists.
    pub expected: Option<PathBuf>,
}

impl Fixture {
    /// Reads the source text.
    ///
    /// # Errors
    ///
    /// Returns the underlying IO error if the file cannot be read.
    pub fn read_source(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.source)
    }

    /// Reads the expected output, or returns `None` if the golden
    /// file is missing.
    ///
    /// # Errors
    ///
    /// Returns the underlying IO error if the file exists but cannot
    /// be read.
    pub fn read_expected(&self) -> std::io::Result<Option<String>> {
        match &self.expected {
            Some(path) => {
                if path.exists() {
                    Ok(Some(std::fs::read_to_string(path)?))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }
}

/// Discovers all `.pp` files in the given directory and pairs each
/// with a sibling `.expected.txt` file (if present).
///
/// The directory is walked **non-recursively** for v0.1; nested
/// fixtures are out of scope.
///
/// # Errors
///
/// Returns an IO error if the directory cannot be read.
pub fn discover_fixtures(dir: &Path) -> std::io::Result<Vec<Fixture>> {
    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pp") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem.starts_with('_') {
            continue;
        }
        let stem = stem.to_string();
        let mut expected = path.clone();
        expected.set_extension("expected.txt");
        let expected = if expected.exists() {
            Some(expected)
        } else {
            None
        };
        fixtures.push(Fixture {
            source: path,
            name: stem,
            expected,
        });
    }
    fixtures.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fixtures)
}

/// The result of running one fixture.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// The captured stdout, with a trailing newline appended.
    pub stdout: String,
    /// The captured stderr (for diagnostics).
    pub stderr: String,
    /// Process exit code (0 for success).
    pub exit_code: i32,
}

/// Runs a single fixture end-to-end and returns the captured output.
///
/// Runs the full lex → parse → typecheck → lower → JIT → execute
/// pipeline, captures stdout via the runtime capture API, and returns
/// the result.
///
/// # Panics
///
/// Panics if reading the fixture source fails.
pub fn run_fixture(fixture: &Fixture) -> RunResult {
    // Spawn a subprocess to ensure full isolation — global state (JITModule,
    // builtin registry, capture buffer) is process-scoped and leaks between
    // tests when they share a process.
    let binary = {
        let me = std::env::current_exe().unwrap();
        // The test binary is in target/debug/deps/; pipe-lang is in target/debug/
        let mut dir = me.parent().unwrap().parent().unwrap().to_path_buf();
        dir.push("pipe-lang");
        if cfg!(windows) {
            dir.set_extension("exe");
        }
        if !dir.exists() {
            // Try CARGO_MANIFEST_DIR or fallback
            dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target")
                .join("debug");
            let exe_name = format!("pipe-lang{}", std::env::consts::EXE_SUFFIX);
            dir.push(exe_name);
        }
        dir
    };
    let output = std::process::Command::new(&binary)
        .arg("run")
        .arg(&fixture.source)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {binary:?}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(1);

    RunResult {
        stdout,
        stderr,
        exit_code,
    }
}

/// Compares actual stdout against expected and returns a human-readable
/// error message on mismatch, or `Ok(())` on match.
pub fn assert_stdout_matches(fixture: &Fixture, actual: &str) -> Result<(), String> {
    let Some(expected_path) = &fixture.expected else {
        // No golden file → no assertion.
        return Ok(());
    };
    let expected = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("failed to read {}: {e}", expected_path.display()))?;
    let expected = expected.trim_end_matches('\n');
    let actual = actual.trim_end_matches('\n');
    if expected == actual {
        return Ok(());
    }
    Err(format_diff(fixture, expected, actual))
}

/// Builds a line-by-line diff message for two strings.
fn format_diff(fixture: &Fixture, expected: &str, actual: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("stdout mismatch for `{}`\n", fixture.name));
    out.push_str("--- expected\n+++ actual\n");
    let exp_lines: Vec<&str> = expected.lines().collect();
    let act_lines: Vec<&str> = actual.lines().collect();
    let max = exp_lines.len().max(act_lines.len());
    for i in 0..max {
        let e = exp_lines.get(i).copied().unwrap_or("");
        let a = act_lines.get(i).copied().unwrap_or("");
        if e == a {
            out.push_str(&format!("  {e}\n"));
        } else {
            out.push_str(&format!("- {e}\n"));
            out.push_str(&format!("+ {a}\n"));
        }
    }
    out
}

/// Returns the workspace root, i.e. the directory that contains
/// `example-programs/`. For tests under `crates/cli/tests/`, this
/// is `../../../` (the workspace root).
#[must_use]
pub fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/cli -> crates
    p.pop(); // crates -> workspace root
    p
}

/// Returns the path to `example-programs/` relative to the workspace root.
#[must_use]
pub fn examples_dir() -> PathBuf {
    let mut p = workspace_root();
    p.push("example-programs");
    p
}

// ---------------------------------------------------------------------------
// Tests for the loader itself (independent of the pipeline)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_root_is_a_directory() {
        let root = workspace_root();
        assert!(root.is_dir(), "{root:?} is not a directory");
    }

    #[test]
    fn examples_dir_exists() {
        let dir = examples_dir();
        assert!(dir.is_dir(), "{dir:?} does not exist");
    }

    #[test]
    fn discover_fixtures_finds_22_example_programs() {
        let fixtures = discover_fixtures(&examples_dir()).expect("discover");
        let names: Vec<_> = fixtures.iter().map(|f| f.name.clone()).collect();
        assert_eq!(fixtures.len(), 22, "found: {names:?}");
        assert!(names.contains(&"hello".to_string()));
        assert!(names.contains(&"factorial".to_string()));
        assert!(names.contains(&"io-effects".to_string()));
        assert!(names.contains(&"state-machine".to_string()));
        assert!(names.contains(&"expression-evaluator".to_string()));
        assert!(names.contains(&"csv-query".to_string()));
        assert!(names.contains(&"json-parser".to_string()));
        assert!(names.contains(&"pathfinding-bfs".to_string()));
        assert!(names.contains(&"tiny-repl".to_string()));
        assert!(names.contains(&"markdown-renderer".to_string()));
    }

    #[test]
    fn discovered_fixtures_are_sorted_alphabetically() {
        let fixtures = discover_fixtures(&examples_dir()).expect("discover");
        let names: Vec<_> = fixtures.iter().map(|f| f.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn fixture_read_source_returns_nonempty() {
        let fixtures = discover_fixtures(&examples_dir()).expect("discover");
        let hello = fixtures.iter().find(|f| f.name == "hello").unwrap();
        let src = hello.read_source().expect("read");
        assert!(src.contains("println"));
        assert!(src.contains("Hello"));
    }

    #[test]
    fn golden_files_exist_for_known_programs() {
        let fixtures = discover_fixtures(&examples_dir()).expect("discover");
        let with_golden: Vec<_> = fixtures
            .iter()
            .filter(|f| f.read_expected().expect("read").is_some())
            .map(|f| f.name.clone())
            .collect();
        // hello.pp, factorial.pp, fibonacci.pp now have golden files
        assert!(
            with_golden.contains(&"hello".to_string()),
            "hello should have golden"
        );
    }

    #[test]
    fn assert_stdout_matches_passes_when_no_golden() {
        let fixtures = discover_fixtures(&examples_dir()).expect("discover");
        let f = &fixtures[0];
        // No golden file → assert is a no-op.
        assert!(assert_stdout_matches(f, "anything").is_ok());
    }

    #[test]
    fn format_diff_shows_added_and_removed_lines() {
        let f = Fixture {
            source: PathBuf::from("dummy.pp"),
            name: "dummy".to_string(),
            expected: None,
        };
        let msg = format_diff(&f, "alpha\nbeta\ngamma", "alpha\nBETA\ngamma");
        assert!(msg.contains("beta"));
        assert!(msg.contains("BETA"));
        assert!(msg.contains("stdout mismatch"));
    }
}
