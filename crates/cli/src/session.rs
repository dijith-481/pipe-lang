use std::path::{Path, PathBuf};
use std::sync::Arc;

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
}

impl SessionConfig {
    /// Creates a new session config with defaults.
    #[must_use]
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            mode: CompileMode::Run,
            opt_level: 0,
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
}

/// The result of a compilation pipeline run.
#[derive(Debug)]
pub struct CompileResult {
    /// Any diagnostics produced (errors or warnings).
    pub diagnostics: Vec<SourceDiagnostic>,
    /// Whether compilation succeeded.
    pub success: bool,
    /// The exit code returned by the compiled program's `main` function (0 if not run).
    pub exit_code: i32,
}

impl CompileResult {
    /// Prints all diagnostics to stderr.
    pub fn eprint_to_stderr(&self) {
        for diag in &self.diagnostics {
            eprintln!("{diag:?}");
        }
    }
}

fn failure_from_errors(
    filename: &str,
    source: &Arc<str>,
    errors: impl IntoIterator<Item = CompilerError>,
) -> CompileResult {
    let diagnostics = errors
        .into_iter()
        .map(|err| SourceDiagnostic::new(filename, Arc::clone(source), err))
        .collect();
    CompileResult {
        diagnostics,
        success: false,
        exit_code: 1,
    }
}

/// Orchestrates the compilation pipeline: lex → parse → typecheck → lower → JIT.
///
/// The `CompilerSession` is the central entry point for driving compilation.
/// It reads the source file, runs each pipeline stage, and collects diagnostics.
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
        let source_ref: &str = &source_arc;

        // Stage 1: Parse (parser handles lexing internally)
        let arena = Bump::new();
        let program = match parser::parse(source_ref, &arena) {
            Ok(program) => program,
            Err(err) => {
                return Ok(failure_from_errors(
                    &filename,
                    &source_arc,
                    [CompilerError::from(err)],
                ));
            }
        };

        // Stage 2: Typecheck
        let typed = match typechecker::typecheck(&program) {
            Ok(typed) => typed,
            Err(errors) => {
                return Ok(failure_from_errors(
                    &filename,
                    &source_arc,
                    errors.into_iter().map(CompilerError::from),
                ));
            }
        };

        // For `check` mode, stop here.
        if self.config.mode == CompileMode::Check {
            return Ok(CompileResult {
                diagnostics: Vec::new(),
                success: true,
                exit_code: 0,
            });
        }

        // Stage 3: Lower to IR
        let ir_module = lower(&typed).map_err(|e| {
            Box::new(SourceDiagnostic::new(
                filename.clone(),
                source_arc.clone(),
                CompilerError::ir_error(ast::span::Span::empty(0), e.to_string()),
            ))
        })?;

        // For `emit_ir` mode, print IR and stop.
        if self.config.mode == CompileMode::EmitIr {
            println!("{ir_module}");
            return Ok(CompileResult {
                diagnostics: Vec::new(),
                success: true,
                exit_code: 0,
            });
        }

        // Stage 4a: Initialize builtin registry for JIT name resolution
        let mut registry = BuiltinRegistry::new();
        for builtin in prelude_builtins() {
            registry.register(builtin);
        }
        init_global_registry(registry);

        // Stage 4b: JIT compile and run
        let compiled = runtime::compile_ir(&ir_module).map_err(|e| {
            Box::new(SourceDiagnostic::new(
                filename.clone(),
                source_arc.clone(),
                CompilerError::jit_compile_error(e.to_string()),
            ))
        })?;

        match compiled.call_main() {
            Ok(exit_code) => Ok(CompileResult {
                diagnostics: Vec::new(),
                success: true,
                exit_code,
            }),
            Err(e) => Err(Box::new(SourceDiagnostic::new(
                filename.clone(),
                source_arc.clone(),
                CompilerError::jit_compile_error(e.to_string()),
            ))),
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
}
