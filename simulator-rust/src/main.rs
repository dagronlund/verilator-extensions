use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use parser_verilator::{
    ast::{Design, Domain},
    document::AstDocument,
};
use simulator_rust::{GenerateOptions, generate_project};

#[derive(Debug, Parser)]
#[command(about = "Generate a Rust simulator from a Verilator JSON AST")]
struct Args {
    /// Path to a JSON AST emitted by Verilator.
    tree_json: PathBuf,

    /// Directory in which to create the generated Rust project.
    #[arg(short, long)]
    output: PathBuf,

    /// Cargo package name. Defaults to the output directory name.
    #[arg(long)]
    crate_name: Option<String>,

    /// Clock signal and edge; prefix the name with ! for a negative edge.
    #[arg(long)]
    clock: Domain,

    /// Optional reset sensitivity signal; prefix with ! for a negative edge.
    #[arg(long)]
    reset: Option<Domain>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let result = run(args);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let document = AstDocument::from_path(&args.tree_json)?;
    let design = Design::try_from(&document)?;
    let crate_name = args.crate_name.unwrap_or_else(|| {
        args.output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("generated-rtl")
            .to_string()
    });
    let options = GenerateOptions {
        crate_name,
        clock: args.clock,
        reset: args.reset,
    };
    generate_project(&design, &args.output, &options)?;
    println!("generated: {}", args.output.display());
    Ok(())
}
