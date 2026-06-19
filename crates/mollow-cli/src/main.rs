use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use mollow_platform::{collect_snapshot, native_probe};

#[derive(Debug, Parser)]
#[command(name = "mollow", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect the current machine and capture a point-in-time snapshot.
    Inspect {
        /// Select the report representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("mollow: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    match &cli.command {
        Command::Inspect {
            format: OutputFormat::Json,
        } => {
            let captured_at_unix_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_millis()
                .try_into()?;
            let snapshot = collect_snapshot(
                &native_probe(),
                env!("CARGO_PKG_VERSION"),
                captured_at_unix_ms,
            );
            mollow_report::render_json(&snapshot)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        }
    }
}
