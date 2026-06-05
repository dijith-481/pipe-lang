use clap::{Parser, Subcommand};

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
    match cli.command {
        Commands::Compile {
            file,
            emit_ir,
            opt_level,
        } => {
            println!("Compiling {file} (emit_ir={emit_ir}, opt_level={opt_level})");
        }
        Commands::Run { file } => {
            println!("Running {file}");
        }
        Commands::Check { file } => {
            println!("Checking {file}");
        }
    }
}
