use std::path::PathBuf;

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
    /// Enable colored output (auto-detect by default)
    #[arg(long, global = true, default_value = "auto")]
    pub color: String,

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
    /// Format a source file in-place
    Fmt {
        /// Path to the source file
        file: String,
        /// Check if the file is formatted correctly without modifying
        #[arg(long)]
        check: bool,
    },
    /// Explain an error code in detail
    Explain {
        /// Error code to explain (e.g. pipe_lang::ty, pipe_lang::parse)
        code: String,
    },
    /// Start the language server
    Lsp,
}

fn main() {
    let cli = Cli::parse();

    // Configure color output
    match cli.color.as_str() {
        "always" | "yes" | "true" => {
            // Colors are enabled by default in miette's fancy rendering
        }
        "never" | "no" | "false" => {
            // Disable ANSI colors
            #[allow(unused_unsafe)]
            unsafe { std::env::set_var("NO_COLOR", "1"); }
            #[allow(unused_unsafe)]
            unsafe { std::env::set_var("CLICOLOR", "0"); }
        }
        _ => { /* auto-detect */ }
    }

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
            let config = SessionConfig::new(PathBuf::from(&file))
                .with_mode(mode)
                .with_opt_level(opt_level);
            run_session(config)
        }
        Commands::Run { file } => {
            let config =
                SessionConfig::new(PathBuf::from(&file)).with_mode(CompileMode::Run);
            run_session(config)
        }
        Commands::Check { file } => {
            let config =
                SessionConfig::new(PathBuf::from(&file)).with_mode(CompileMode::Check);
            run_session(config)
        }
        Commands::Fmt { file, check } => {
            run_fmt(&file, check)
        }
        Commands::Explain { code } => {
            run_explain(&code);
            0
        }
        Commands::Lsp => {
            launch_lsp();
            0
        }
    };
    std::process::exit(exit_code);
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

fn run_session(config: SessionConfig) -> i32 {
    let mut session = CompilerSession::new(config);
    if let Err(e) = session.load_source() {
        eprintln!("error: {e}");
        return 1;
    }
    match session.run_pipeline() {
        Ok(result) => {
            result.eprint_to_stderr();
            let status = if result.success { "succeeded" } else { "failed" };
            if !result.diagnostics.is_empty() || !result.success {
                eprintln!(
                    "[pipe-lang] compilation {status} with {} diagnostic(s)",
                    result.diagnostics.len()
                );
            }
            if result.success { result.exit_code } else { 1 }
        }
        Err(e) => {
            eprintln!("{}", e.render());
            1
        }
    }
}

fn run_fmt(path: &str, check: bool) -> i32 {
    // Read the source file
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {path}: {e}");
            return 1;
        }
    };

    // Parse and format
    match formatter::format_source(&source) {
        Ok(formatted) => {
            if check {
                if formatted == source {
                    eprintln!("[pipe-lang] {} is correctly formatted", path);
                    0
                } else {
                    eprintln!("[pipe-lang] {} needs formatting", path);
                    1
                }
            } else {
            if let Err(e) = std::fs::write(path, &formatted) {
                eprintln!("error: failed to write {path}: {e}");
                return 1;
            }
                eprintln!("[pipe-lang] formatted {}", path);
                0
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn run_explain(code: &str) {
    match code {
        "pipe_lang::lex" => {
            println!("Lex errors occur when the tokenizer encounters invalid characters or syntax.\n");
            println!("Common causes:");
            println!("  • Unexpected characters like @, #, or $ in source code");
            println!("  • Unterminated string literals (missing closing quote)");
            println!("  • Invalid numeric literals (e.g. multiple decimal points)");
            println!("\nCheck the highlighted region in the error output for the exact issue.");
        }
        "pipe_lang::parse" => {
            println!("Parse errors occur when the source code does not follow pipe-lang's grammar.\n");
            println!("Common causes:");
            println!("  • Missing parentheses, braces, or commas");
            println!("  • Wrong keyword or operator ordering");
            println!("  • Incomplete expressions or declarations");
            println!("\nThe error shows the expected tokens; check the syntax near the highlighted position.");
        }
        "pipe_lang::ty" => {
            println!("Type errors occur when the type checker detects an inconsistency.\n");
            println!("Common causes:");
            println!("  • Adding mismatched types (e.g. str + i32)");
            println!("  • Calling a function with wrong argument types");
            println!("  • Using an unbound variable or misspelled name");
            println!("  • Non-exhaustive pattern matching");
            println!("\nThe error shows what was expected vs what was found.");
        }
        "pipe_lang::ir" | "pipe_lang::runtime" => {
            println!("Internal errors occur during compilation or execution.\n");
            println!("These are likely bugs in pipe-lang itself. Please report them at:");
            println!("  https://github.com/anomalyco/opencode/issues");
        }
        _ => {
            println!("Unknown error code: {code}");
            println!("Available codes: pipe_lang::lex, pipe_lang::parse, pipe_lang::ty, pipe_lang::ir, pipe_lang::runtime");
        }
    }
}
