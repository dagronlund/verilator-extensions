use std::{fs, path::PathBuf, process::ExitCode};

use circt_formal::{
    convert::{ClockSpec, ConversionOptions, NamedFsm, ResetSpec},
    export::{self, AigerFormat, ExportOptions},
};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Convert CIRCT formal-core MLIR to an ordered Boolean FSM and AIGER")]
struct Args {
    /// HW/formal-core MLIR input, after binary-logic and formal-core lowering.
    input: PathBuf,
    /// Top module; otherwise infer the unique public uninstantiated module.
    #[arg(long)]
    top: Option<String>,
    /// Clock input; prefix with ! for the negative edge.
    #[arg(long)]
    clock: ClockSpec,
    /// Initialize with one reset transition, then tie reset inactive; ! means active-low.
    #[arg(long)]
    reset: Option<ResetSpec>,
    /// Write AIGER 1.9: .aag for ASCII or .aig for binary.
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    strip_symbols: bool,
    #[arg(long)]
    zero_init: bool,
    /// Select one zero-based assertion index.
    #[arg(
        long = "assertion",
        alias = "assert",
        requires = "output",
        conflicts_with = "cover"
    )]
    assertion: Option<usize>,
    /// Select one cover, inverted and exported as an assertion.
    #[arg(long, requires = "output")]
    cover: Option<usize>,
    /// Print the AIGER symbol records, even with --strip-symbols.
    #[arg(long)]
    debug: bool,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(&args.input)?;
    let model = NamedFsm::from_source(
        0,
        &source,
        &ConversionOptions {
            top: args.top,
            clock: args.clock,
            reset: args.reset,
        },
    )?;
    for warning in &model.warnings {
        eprintln!("warning: {warning}");
    }
    println!(
        "converted: {}\ntop: {}\nclock: {:?} edge of {}",
        args.input.display(),
        model.top,
        model.clock.edge,
        model.clock.name
    );
    println!(
        "inputs: {} signals, {} bits\nregisters: {} signals, {} bits\noutputs: {} signals, {} bits\ngates: {}\nproperties: {} assertions, {} assumptions, {} covers",
        model.inputs.len(),
        model.fsm.get_inputs().len(),
        model.registers.len(),
        model.fsm.get_latches().len(),
        model.outputs.len(),
        model.fsm.get_outputs().len(),
        model.fsm.get_gates().len(),
        model.assertions.len(),
        model.assumptions.len(),
        model.covers.len()
    );
    let options = ExportOptions {
        zero_init: args.zero_init,
        strip_symbols: args.strip_symbols,
        assertion: args.assertion,
        cover: args.cover,
    };
    if args.debug {
        let prepared = export::prepare(
            &model.fsm,
            &ExportOptions {
                strip_symbols: false,
                ..options
            },
        )?;
        for symbol in export::symbols(&prepared) {
            eprintln!("aiger symbol: {symbol}");
        }
    }
    if let Some(output) = args.output {
        let extension = output
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);
        let format = match extension.as_deref() {
            Some("aag") => AigerFormat::Ascii,
            Some("aig") => AigerFormat::Binary,
            _ => {
                return Err(format!(
                    "output {} must have a .aag or .aig extension",
                    output.display()
                )
                .into());
            }
        };
        fs::write(&output, export::write(&model.fsm, format, &options)?)?;
        println!("wrote: {}", output.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
