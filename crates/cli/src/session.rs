use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bumpalo::Bump;
use diagnostics::errors::{CompilerError, SourceDiagnostic};
use ir::lower;
use runtime::{BuiltinRegistry, init_global_registry};
use stdlib::prelude::prelude_builtins;

/// What the pipeline should do after typechecking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileMode {
    /// Typecheck only — no IR lowering or JIT.
    Check,
    /// Typecheck, lower to IR, emit IR to stdout.
    EmitIr,
    /// Full pipeline: typecheck, lower, JIT compile and run.
    Run,
}

/// Configuration for a compilation session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Path to the source file being compiled.
    pub file_path: PathBuf,
    /// What to do after typechecking.
    pub mode: CompileMode,
    /// Optimization level (0-3).
    pub opt_level: u8,
    /// Whether to print timing information.
    pub timing: bool,
}

impl SessionConfig {
    /// Creates a new session config with defaults.
    #[must_use]
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            mode: CompileMode::Run,
            opt_level: 0,
            timing: false,
        }
    }

    /// Sets the compile mode.
    #[must_use]
    pub fn with_mode(mut self, mode: CompileMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the optimization level.
    #[must_use]
    pub fn with_opt_level(mut self, opt_level: u8) -> Self {
        self.opt_level = opt_level;
        self
    }

    /// Sets whether to print timing information.
    #[must_use]
    pub fn with_timing(mut self, timing: bool) -> Self {
        self.timing = timing;
        self
    }
}

/// The result of a compilation pipeline run.
#[derive(Debug)]
pub struct CompileResult {
    /// Any diagnostics produced (errors or warnings).
    pub diagnostics: Vec<SourceDiagnostic>,
    /// Whether compilation succeeded.
    pub success: bool,
    /// The exit code returned by the compiled program's `main` function.
    pub exit_code: i32,
    /// Timing information for each pipeline stage.
    pub timings: HashMap<String, Duration>,
    /// Whether timing information should be printed.
    pub timing: bool,
}

impl CompileResult {
    /// Prints all diagnostics to stderr using miette graphical rendering.
    pub fn eprint_to_stderr(&self) {
        for diag in &self.diagnostics {
            eprintln!("{:?}", miette::Report::new(diag.clone()));
        }
    }
}

fn failure_from_errors(
    filename: &str,
    source: &Arc<str>,
    errors: impl IntoIterator<Item = CompilerError>,
    timings: HashMap<String, Duration>,
) -> CompileResult {
    let diagnostics = errors
        .into_iter()
        .map(|err| SourceDiagnostic::new(filename, Arc::clone(source), err))
        .collect();
    CompileResult {
        diagnostics,
        success: false,
        exit_code: 1,
        timings,
        timing: false,
    }
}

/// Orchestrates the compilation pipeline: lex → parse → typecheck → lower → JIT.
pub struct CompilerSession {
    config: SessionConfig,
    source: Option<Arc<str>>,
}

impl CompilerSession {
    /// Creates a new compiler session with the given configuration.
    #[must_use]
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            source: None,
        }
    }

    /// Returns a reference to the session configuration.
    #[must_use]
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Returns the path to the source file.
    #[must_use]
    pub fn file_path(&self) -> &Path {
        &self.config.file_path
    }

    /// Returns the loaded source, if any.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Reads the source file from disk and stores it in the session.
    ///
    /// # Errors
    ///
    /// Returns a [`CompilerError`] if the file cannot be read.
    pub fn load_source(&mut self) -> Result<(), CompilerError> {
        let src = std::fs::read_to_string(&self.config.file_path).map_err(|e| {
            CompilerError::IoError(format!(
                "failed to read {}: {e}",
                self.config.file_path.display()
            ))
        })?;
        self.source = Some(Arc::from(src));
        Ok(())
    }

    /// Sets the source code directly (useful for testing).
    pub fn set_source(&mut self, source: impl Into<Arc<str>>) {
        self.source = Some(source.into());
    }

    /// Runs the full compilation pipeline on the loaded source.
    ///
    /// # Panics
    ///
    /// Panics if [`load_source`](Self::load_source) or
    /// [`set_source`](Self::set_source) was not called first.
    ///
    /// # Errors
    ///
    /// Returns errors from any pipeline stage (lex, parse, typecheck, lower)
    /// wrapped in [`SourceDiagnostic`].
    pub fn run_pipeline(&mut self) -> Result<CompileResult, Box<SourceDiagnostic>> {
        let source_arc = self
            .source
            .clone()
            .expect("source must be loaded before running pipeline");
        let filename = self.config.file_path.to_string_lossy().to_string();
        let mut timings: HashMap<String, Duration> = HashMap::new();
        let show_timing = self.config.timing;

        // Parse, typecheck, and lower in a scope that keeps the source borrow alive
        let ir_module = {
            let source_ref: &str = &source_arc;
            let arena = Bump::new();

            // Stage 1: Parse
            let parse_start = Instant::now();
            let program = match parser::parse(source_ref, &arena) {
                Ok(p) => p,
                Err(err) => {
                    timings.insert("parse".into(), parse_start.elapsed());
                    return Ok(failure_from_errors(
                        &filename,
                        &source_arc,
                        [CompilerError::from(err)],
                        timings,
                    ));
                }
            };
            timings.insert("parse".into(), parse_start.elapsed());

            // Stage 2: Typecheck
            let tc_start = Instant::now();
            let typed = match typechecker::typecheck(&program) {
                Ok(t) => t,
                Err(errors) => {
                    timings.insert("typecheck".into(), tc_start.elapsed());
                    return Ok(failure_from_errors(
                        &filename,
                        &source_arc,
                        errors.into_iter().map(CompilerError::from),
                        timings,
                    ));
                }
            };
            timings.insert("typecheck".into(), tc_start.elapsed());

            if self.config.mode == CompileMode::Check {
                return Ok(CompileResult {
                    diagnostics: Vec::new(),
                    success: true,
                    exit_code: 0,
                    timings,
                    timing: show_timing,
                });
            }

            // Stage 3: Lower to IR
            let lower_start = Instant::now();
            let ir = lower(&typed).map_err(|e| {
                Box::new(SourceDiagnostic::new(
                    filename.clone(),
                    source_arc.clone(),
                    CompilerError::from(e),
                ))
            })?;
            timings.insert("lower".into(), lower_start.elapsed());

            if self.config.mode == CompileMode::EmitIr {
                println!("{ir}");
                return Ok(CompileResult {
                    diagnostics: Vec::new(),
                    success: true,
                    exit_code: 0,
                    timings,
                    timing: show_timing,
                });
            }

            ir
        };

        // Stage 4a: Initialize builtin registry for JIT name resolution
        let reg_start = Instant::now();
        let mut registry = BuiltinRegistry::new();
        for builtin in prelude_builtins() {
            registry.register(builtin);
        }
        init_global_registry(registry);
        timings.insert("init".into(), reg_start.elapsed());

        // Stage 4b: JIT compile and run
        let jit_start = Instant::now();
        let compiled = runtime::compile_ir(&ir_module).map_err(|e| {
            Box::new(SourceDiagnostic::new(
                filename.clone(),
                source_arc.clone(),
                CompilerError::from(e),
            ))
        })?;
        timings.insert("jit_compile".into(), jit_start.elapsed());

        let execute_start = Instant::now();
        match compiled.call_main() {
            Ok(exit_code) => {
                timings.insert("execute".into(), execute_start.elapsed());
                Ok(CompileResult {
                    diagnostics: Vec::new(),
                    success: true,
                    exit_code,
                    timings,
                    timing: show_timing,
                })
            }
            Err(e) => {
                timings.insert("execute".into(), execute_start.elapsed());
                Err(Box::new(SourceDiagnostic::new(
                    filename,
                    source_arc,
                    CompilerError::from(e),
                )))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_defaults() {
        let config = SessionConfig::new(PathBuf::from("test.pl"));
        assert_eq!(config.file_path, PathBuf::from("test.pl"));
        assert_eq!(config.mode, CompileMode::Run);
        assert_eq!(config.opt_level, 0);
        assert!(!config.timing);
    }

    #[test]
    fn session_config_builder() {
        let config = SessionConfig::new(PathBuf::from("test.pl"))
            .with_mode(CompileMode::Check)
            .with_opt_level(2);
        assert_eq!(config.mode, CompileMode::Check);
        assert_eq!(config.opt_level, 2);
    }

    #[test]
    fn read_source_missing_file() {
        let config = SessionConfig::new(PathBuf::from("/nonexistent/file.pl"));
        let mut session = CompilerSession::new(config);
        let result = session.load_source();
        assert!(result.is_err());
    }

    #[test]
    fn set_source_directly() {
        let config = SessionConfig::new(PathBuf::from("test.pl"));
        let mut session = CompilerSession::new(config);
        session.set_source("hello world");
        assert_eq!(session.source(), Some("hello world"));
    }

    #[test]
    fn pipeline_check_mode() {
        let config = SessionConfig::new(PathBuf::from("test.pl")).with_mode(CompileMode::Check);
        let mut session = CompilerSession::new(config);
        session.set_source("let x = 42");
        let result = session.run_pipeline().unwrap();
        assert!(result.success);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn pipeline_emit_ir_mode() {
        let config = SessionConfig::new(PathBuf::from("test.pl")).with_mode(CompileMode::EmitIr);
        let mut session = CompilerSession::new(config);
        session.set_source("let x = 42");
        let result = session.run_pipeline().unwrap();
        assert!(result.success);
    }

    #[test]
    fn pipeline_parse_error() {
        let config = SessionConfig::new(PathBuf::from("test.pl")).with_mode(CompileMode::Check);
        let mut session = CompilerSession::new(config);
        session.set_source("let = ");
        let result = session.run_pipeline().expect("pipeline result");
        assert!(!result.success);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn pipeline_type_error() {
        let config = SessionConfig::new(PathBuf::from("test.pl")).with_mode(CompileMode::Check);
        let mut session = CompilerSession::new(config);
        session.set_source("let x = \"hello\" + 42");
        let result = session.run_pipeline().expect("pipeline result");
        assert!(!result.success);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn pipeline_valid_program() {
        let config = SessionConfig::new(PathBuf::from("test.pl")).with_mode(CompileMode::Check);
        let mut session = CompilerSession::new(config);
        session.set_source("let add = (a: i32, b: i32) => a + b");
        let result = session.run_pipeline().unwrap();
        assert!(result.success);
    }

    #[test]
    fn pipeline_timing_flag() {
        let config = SessionConfig::new(PathBuf::from("test.pl"))
            .with_mode(CompileMode::Check)
            .with_timing(true);
        let mut session = CompilerSession::new(config);
        session.set_source("let x = 42");
        let result = session.run_pipeline().unwrap();
        assert!(result.timing);
        assert!(!result.timings.is_empty());
        assert!(result.timings.contains_key("parse"));
        assert!(result.timings.contains_key("typecheck"));
    }
}
