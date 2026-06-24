use std::process::ExitCode;

use clap::{Parser, Subcommand};
use diagnostics::errors::CompilerError;

use cli::session;
use session::{CompileMode, CompilerSession, SessionConfig};

/// The pipe-lang compiler and runtime.
#[derive(Parser)]
#[command(
    name = "pipe-lang",
    version,
    about = "A pure functional language with JIT compilation"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile a source file
    Compile {
        /// Path to the source file
        file: String,
        /// Emit IR to stdout
        #[arg(long)]
        emit_ir: bool,
        /// Optimization level (0-3)
        #[arg(long, default_value = "0")]
        opt_level: u8,
        /// Print timing information
        #[arg(long)]
        time: bool,
    },
    /// Run a source file directly
    Run {
        /// Path to the source file
        file: String,
        /// Print timing information
        #[arg(long)]
        time: bool,
    },
    /// Check types without generating code
    Check {
        /// Path to the source file
        file: String,
    },
    /// Format a source file
    Fmt {
        /// Path to the source file
        file: String,
        /// Overwrite the file in-place
        #[arg(long, short)]
        write: bool,
    },
    /// Start the language server
    Lsp,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile {
            file,
            emit_ir,
            opt_level,
            time,
        } => {
            let mode = if emit_ir {
                CompileMode::EmitIr
            } else {
                CompileMode::Run
            };
            let config = SessionConfig::new(std::path::PathBuf::from(&file))
                .with_mode(mode)
                .with_opt_level(opt_level)
                .with_timing(time);
            run_session(config)
        }
        Commands::Run { file, time } => {
            let config = SessionConfig::new(std::path::PathBuf::from(&file))
                .with_mode(CompileMode::Run)
                .with_timing(time);
            run_session(config)
        }
        Commands::Check { file } => {
            let config =
                SessionConfig::new(std::path::PathBuf::from(&file)).with_mode(CompileMode::Check);
            run_session(config)
        }
        Commands::Fmt { file, write } => run_fmt(file, write),
        Commands::Lsp => {
            launch_lsp();
            ExitCode::SUCCESS
        }
    }
}

fn run_fmt(path: String, write: bool) -> ExitCode {
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("I/O error: failed to read {path}: {e}");
            return ExitCode::from(3);
        }
    };
    let arena = bumpalo::Bump::new();
    let program = match parser::parse(&src, &arena) {
        Ok(p) => p,
        Err(e) => {
            let report = miette::Report::new(diagnostics::errors::SourceDiagnostic::new(
                std::path::Path::new(&path).to_string_lossy().to_string(),
                std::sync::Arc::from("<no source>"),
                diagnostics::CompilerError::from(e),
            ));
            eprintln!("{report:?}");
            return ExitCode::from(1);
        }
    };
    let formatted = formatter::format(&program);
    if write {
        if let Err(e) = std::fs::write(&path, &formatted) {
            eprintln!("I/O error: failed to write {path}: {e}");
            return ExitCode::from(3);
        }
        eprintln!("Formatted {path}");
    } else {
        print!("{formatted}");
    }
    ExitCode::SUCCESS
}

fn launch_lsp() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = tower_lsp::LspService::new(pipe_lang_lsp::Backend::new);
        tower_lsp::Server::new(stdin, stdout, socket)
            .serve(service)
            .await;
    });
}

fn run_session(config: SessionConfig) -> ExitCode {
    let mut session = CompilerSession::new(config);
    if let Err(e) = session.load_source() {
        let exit_code = match &e {
            CompilerError::IoError(_) => ExitCode::from(3),
            _ => ExitCode::from(1),
        };
        let report = miette::Report::new(diagnostics::errors::SourceDiagnostic::new(
            session.file_path().to_string_lossy().to_string(),
            std::sync::Arc::from("<no source>"),
            e,
        ));
        eprintln!("{report:?}");
        return exit_code;
    }
    match session.run_pipeline() {
        Ok(result) => {
            result.eprint_to_stderr();
            if result.timing {
                print_timings(&result.timings);
            }
            if result.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(diag) => {
            let exit_code = match &diag.error {
                CompilerError::JitCompileError { .. } => ExitCode::from(2),
                CompilerError::IoError(_) => ExitCode::from(3),
                _ => ExitCode::from(1),
            };
            eprintln!("{:?}", miette::Report::new(*diag));
            exit_code
        }
    }
}

fn print_timings(timings: &std::collections::HashMap<String, std::time::Duration>) {
    eprintln!("\nTiming:");
    eprintln!("  {:<15} {:>12}", "Stage", "Duration");
    eprintln!("  {}", "-".repeat(29));
    let total: std::time::Duration = timings.values().copied().sum();
    let mut stages: Vec<_> = timings.iter().collect();
    stages.sort_by_key(|(k, _)| *k);
    for (stage, duration) in &stages {
        eprintln!("  {stage:<15} {duration:>8?}");
    }
    eprintln!("  {:<15} {total:>8?}", "Total");
}
