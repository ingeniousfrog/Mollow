use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use mollow_core::BenchmarkProfile;
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
    /// Run lightweight, reproducible practical workloads.
    Bench {
        /// Select workload size and repetition count.
        #[arg(long, value_enum, default_value_t = CliBenchmarkProfile::Quick)]
        profile: CliBenchmarkProfile,
        /// Select the report representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliBenchmarkProfile {
    Quick,
    Standard,
}

impl From<CliBenchmarkProfile> for BenchmarkProfile {
    fn from(profile: CliBenchmarkProfile) -> Self {
        match profile {
            CliBenchmarkProfile::Quick => Self::Quick,
            CliBenchmarkProfile::Standard => Self::Standard,
        }
    }
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
        Command::Bench {
            profile,
            format: OutputFormat::Json,
        } => {
            let started_at_unix_ms = unix_time_ms()?;
            let machine_snapshot = collect_snapshot(
                &native_probe(),
                env!("CARGO_PKG_VERSION"),
                started_at_unix_ms,
            );
            let benchmark = mollow_bench::run_suite(
                (*profile).into(),
                env!("CARGO_PKG_VERSION"),
                started_at_unix_ms,
                machine_snapshot,
            )?;
            mollow_report::render_benchmark_json(&benchmark)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        }
    }
}

fn unix_time_ms() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
