use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use mollow_core::{BenchmarkProfile, BenchmarkRun, ComparisonReport, MachineSnapshot};
use mollow_platform::{collect_snapshot, native_probe};
use mollow_report::{ReportFormat, ReportLanguage};
use serde::de::DeserializeOwned;

const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "mollow", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect the current machine and capture a point-in-time snapshot.
    Inspect {
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Run lightweight, reproducible practical workloads.
    Bench {
        #[arg(long, value_enum, default_value_t = CliBenchmarkProfile::Quick)]
        profile: CliBenchmarkProfile,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Capture a machine snapshot and benchmark run as one baseline.
    Capture {
        #[arg(long, value_enum, default_value_t = CliBenchmarkProfile::Quick)]
        profile: CliBenchmarkProfile,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Compare two benchmark baseline files.
    Compare {
        baseline: PathBuf,
        candidate: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Render a snapshot, benchmark, or comparison file.
    Report {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Terminal,
    Json,
    Markdown,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliBenchmarkProfile {
    Quick,
    Standard,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Language {
    English,
    #[value(name = "zh-CN")]
    Chinese,
}

pub struct Output {
    pub content: String,
    pub path: Option<PathBuf>,
}

pub fn execute(cli: &Cli) -> Result<Output, Box<dyn std::error::Error>> {
    match &cli.command {
        Command::Inspect {
            format,
            lang,
            output,
        } => {
            let snapshot = current_snapshot()?;
            Ok(rendered(
                mollow_report::render_snapshot(&snapshot, (*format).into(), (*lang).into())?,
                output.as_ref(),
            ))
        }
        Command::Bench {
            profile,
            format,
            lang,
            output,
        }
        | Command::Capture {
            profile,
            format,
            lang,
            output,
        } => {
            let benchmark = current_benchmark((*profile).into())?;
            Ok(rendered(
                mollow_report::render_benchmark(&benchmark, (*format).into(), (*lang).into())?,
                output.as_ref(),
            ))
        }
        Command::Compare {
            baseline,
            candidate,
            format,
            lang,
            output,
        } => {
            let baseline: BenchmarkRun = read_json(baseline)?;
            let candidate: BenchmarkRun = read_json(candidate)?;
            let comparison = mollow_compare::compare_runs(&baseline, &candidate)?;
            Ok(rendered(
                mollow_report::render_comparison(&comparison, (*format).into(), (*lang).into())?,
                output.as_ref(),
            ))
        }
        Command::Report {
            input,
            format,
            lang,
            output,
        } => {
            let value: serde_json::Value = read_json(input)?;
            let content = render_detected(value, (*format).into(), (*lang).into())?;
            Ok(rendered(content, output.as_ref()))
        }
    }
}

fn current_snapshot() -> Result<MachineSnapshot, Box<dyn std::error::Error>> {
    let captured_at_unix_ms = unix_time_ms()?;
    Ok(collect_snapshot(
        &native_probe(),
        env!("CARGO_PKG_VERSION"),
        captured_at_unix_ms,
    ))
}

fn current_benchmark(
    profile: BenchmarkProfile,
) -> Result<BenchmarkRun, Box<dyn std::error::Error>> {
    let started_at_unix_ms = unix_time_ms()?;
    let snapshot = collect_snapshot(
        &native_probe(),
        env!("CARGO_PKG_VERSION"),
        started_at_unix_ms,
    );
    Ok(mollow_bench::run_suite(
        profile,
        env!("CARGO_PKG_VERSION"),
        started_at_unix_ms,
        snapshot,
    )?)
}

fn render_detected(
    value: serde_json::Value,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, Box<dyn std::error::Error>> {
    if value.get("baseline_started_at_unix_ms").is_some() {
        let document: ComparisonReport = serde_json::from_value(value)?;
        return Ok(mollow_report::render_comparison(
            &document, format, language,
        )?);
    }
    if value.get("started_at_unix_ms").is_some() {
        let document: BenchmarkRun = serde_json::from_value(value)?;
        return Ok(mollow_report::render_benchmark(
            &document, format, language,
        )?);
    }
    if value.get("captured_at_unix_ms").is_some() {
        let document: MachineSnapshot = serde_json::from_value(value)?;
        return Ok(mollow_report::render_snapshot(&document, format, language)?);
    }
    Err("input is not a recognized Mollow document".into())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_INPUT_BYTES {
        return Err(format!("input exceeds the {MAX_INPUT_BYTES} byte limit").into());
    }
    let mut content = String::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_string(&mut content)?;
    if u64::try_from(content.len())? > MAX_INPUT_BYTES {
        return Err(format!("input exceeds the {MAX_INPUT_BYTES} byte limit").into());
    }
    Ok(serde_json::from_str(&content)?)
}

fn rendered(content: String, output: Option<&PathBuf>) -> Output {
    Output {
        content,
        path: output.cloned(),
    }
}

fn unix_time_ms() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

impl From<CliBenchmarkProfile> for BenchmarkProfile {
    fn from(profile: CliBenchmarkProfile) -> Self {
        match profile {
            CliBenchmarkProfile::Quick => Self::Quick,
            CliBenchmarkProfile::Standard => Self::Standard,
        }
    }
}

impl From<OutputFormat> for ReportFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Terminal => Self::Terminal,
            OutputFormat::Json => Self::Json,
            OutputFormat::Markdown => Self::Markdown,
            OutputFormat::Html => Self::Html,
        }
    }
}

impl From<Language> for ReportLanguage {
    fn from(language: Language) -> Self {
        match language {
            Language::English => Self::English,
            Language::Chinese => Self::Chinese,
        }
    }
}
