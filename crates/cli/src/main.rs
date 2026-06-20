use clap::{Parser, Subcommand};

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
    },
    /// Run a source file directly
    Run {
        /// Path to the source file
        file: String,
    },
    /// Check types without generating code
    Check {
        /// Path to the source file
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::Compile {
            file,
            emit_ir,
            opt_level,
        } => {
            let mode = if emit_ir {
                CompileMode::EmitIr
            } else {
                CompileMode::Run
            };
            let config = SessionConfig::new(std::path::PathBuf::from(&file))
                .with_mode(mode)
                .with_opt_level(opt_level);
            run_session(config)
        }
        Commands::Run { file } => {
            let config =
                SessionConfig::new(std::path::PathBuf::from(&file)).with_mode(CompileMode::Run);
            run_session(config)
        }
        Commands::Check { file } => {
            let config =
                SessionConfig::new(std::path::PathBuf::from(&file)).with_mode(CompileMode::Check);
            run_session(config)
        }
    };
    std::process::exit(exit_code);
}

fn run_session(config: SessionConfig) -> i32 {
    let mut session = CompilerSession::new(config);
    if let Err(e) = session.load_source() {
        eprintln!("{e}");
        return 1;
    }
    match session.run_pipeline() {
        Ok(result) => {
            result.eprint_to_stderr();
            if result.success { result.exit_code } else { 1 }
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
