use std::path::{Path, PathBuf};

use diagnostics::errors::CompilerError;
use lexer::{Lexer, TokenKind};

/// Configuration for a compilation session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Path to the source file being compiled.
    pub file_path: PathBuf,
    /// Whether to emit IR to stdout.
    pub emit_ir: bool,
    /// Optimization level (0-3).
    pub opt_level: u8,
}

impl SessionConfig {
    /// Creates a new session config with defaults.
    #[must_use]
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            emit_ir: false,
            opt_level: 0,
        }
    }

    /// Sets whether to emit IR.
    #[must_use]
    pub fn with_emit_ir(mut self, emit_ir: bool) -> Self {
        self.emit_ir = emit_ir;
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
    pub diagnostics: Vec<CompilerError>,
    /// Whether compilation succeeded.
    pub success: bool,
}

impl CompileResult {
    /// Prints all diagnostics to stderr.
    pub fn eprint_to_stderr(&self) {
        for diag in &self.diagnostics {
            eprintln!("{diag}");
        }
    }
}

/// Orchestrates the compilation pipeline: lex → parse → typecheck.
///
/// The `CompilerSession` is the central entry point for driving compilation.
/// It reads the source file, runs each pipeline stage, and collects diagnostics.
pub struct CompilerSession {
    config: SessionConfig,
    source: Option<String>,
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
        self.source = Some(src);
        Ok(())
    }

    /// Sets the source code directly (useful for testing).
    pub fn set_source(&mut self, source: impl Into<String>) {
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
    /// Returns errors from any pipeline stage (lex, parse, typecheck).
    pub fn run_pipeline(&mut self) -> Result<CompileResult, CompilerError> {
        let source = self
            .source
            .as_deref()
            .expect("source must be loaded before running pipeline");

        let diagnostics = Vec::new();

        // Stage 1: Lex
        let lexer = Lexer::new(source);
        let tokens: Vec<_> = lexer.collect();

        // Check for error tokens produced by the lexer (if we add error tokens)
        // For now, the lexer emits all tokens including EOF.
        // We filter out whitespace/comment/newline for downstream stages.
        let _significant_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| {
                !matches!(
                    t.kind,
                    TokenKind::Whitespace(_)
                        | TokenKind::Comment(_)
                        | TokenKind::Newline
                        | TokenKind::Eof
                )
            })
            .collect();

        // Stage 2: Parse (stub — not implemented yet)
        // TODO: integrate parser when it exists
        // let program = parser::parse(&tokens)?;

        // Stage 3: Typecheck (stub — not implemented yet)
        // TODO: integrate typechecker when parser is ready

        Ok(CompileResult {
            diagnostics,
            success: true,
        })
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
        assert!(!config.emit_ir);
        assert_eq!(config.opt_level, 0);
    }

    #[test]
    fn session_config_builder() {
        let config = SessionConfig::new(PathBuf::from("test.pl"))
            .with_emit_ir(true)
            .with_opt_level(2);
        assert!(config.emit_ir);
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
    fn run_pipeline_valid_lex() {
        let config = SessionConfig::new(PathBuf::from("test.pl"));
        let mut session = CompilerSession::new(config);
        session.set_source("let x = 42");
        let result = session.run_pipeline().unwrap();
        assert!(result.success);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn set_source_directly() {
        let config = SessionConfig::new(PathBuf::from("test.pl"));
        let mut session = CompilerSession::new(config);
        session.set_source("hello world");
        assert_eq!(session.source(), Some("hello world"));
    }
}
