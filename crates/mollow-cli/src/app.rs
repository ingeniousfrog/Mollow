use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::io::{Write, stdout};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use mollow_core::{BenchmarkProfile, BenchmarkRun, ComparisonReport, MachineSnapshot, WatchField};
use mollow_platform::{collect_snapshot, collect_watch_reading, native_probe};
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
    /// Compare two or more benchmark baseline files against a baseline.
    Compare {
        baseline: PathBuf,
        candidate: PathBuf,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_candidates: Vec<PathBuf>,
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
    /// Manage a local baseline archive.
    Archive {
        #[command(subcommand)]
        command: ArchiveCommand,
    },
    /// Monitor memory, power, and thermal readings at a fixed interval.
    Watch {
        #[arg(short = 'i', long = "interval", default_value = "1")]
        interval: u64,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
        #[arg(long, value_delimiter = ',', default_value = "memory,power,thermal")]
        fields: Vec<CliWatchField>,
        #[arg(long, help = "Stop after N refresh cycles (useful for tests)")]
        count: Option<u64>,
    },
}

#[derive(Debug, Subcommand)]
enum ArchiveCommand {
    /// Add a benchmark run to a local archive directory.
    Add {
        input: PathBuf,
        #[arg(long)]
        dir: PathBuf,
    },
    /// List archived benchmark runs.
    List {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
    },
    /// Show a workload trend from archived benchmark runs.
    Trend {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, default_value = "cpu")]
        workload: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = Language::English)]
        lang: Language,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliWatchField {
    Memory,
    Power,
    Thermal,
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
            extra_candidates,
            format,
            lang,
            output,
        } => {
            let mut candidates = vec![candidate.clone()];
            candidates.extend(extra_candidates.iter().cloned());
            let content =
                render_comparisons(baseline, &candidates, (*format).into(), (*lang).into())?;
            Ok(rendered(content, output.as_ref()))
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
        Command::Archive { command } => match command {
            ArchiveCommand::Add { input, dir } => {
                let entry = mollow_archive::add_run(dir, input)?;
                Ok(rendered(serde_json::to_string_pretty(&entry)?, None))
            }
            ArchiveCommand::List { dir, format, lang } => Ok(rendered(
                render_archive_list(dir, (*format).into(), (*lang).into())?,
                None,
            )),
            ArchiveCommand::Trend {
                dir,
                workload,
                format,
                lang,
            } => Ok(rendered(
                render_archive_trend(dir, workload, (*format).into(), (*lang).into())?,
                None,
            )),
        },
        Command::Watch {
            interval,
            lang,
            fields,
            count,
        } => {
            run_watch(*interval, fields, (*lang).into(), *count)?;
            Ok(rendered(String::new(), None))
        }
    }
}

fn run_watch(
    interval_secs: u64,
    fields: &[CliWatchField],
    language: ReportLanguage,
    count: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    if interval_secs == 0 {
        return Err("interval must be at least 1 second".into());
    }

    let probe = native_probe();
    let watch_fields = fields
        .iter()
        .copied()
        .map(CliWatchField::into)
        .collect::<Vec<_>>();
    let mut iterations = 0u64;

    loop {
        let captured_at_unix_ms = unix_time_ms()?;
        let reading = collect_watch_reading(&probe, captured_at_unix_ms);
        let frame = mollow_report::render_watch_frame(&reading, &watch_fields, language);
        print!("\x1b[2J\x1b[H{frame}");
        stdout().flush()?;

        iterations += 1;
        if count.is_some_and(|limit| iterations >= limit) {
            break;
        }
        thread::sleep(Duration::from_secs(interval_secs));
    }

    Ok(())
}

fn render_comparisons(
    baseline_path: &Path,
    candidates: &[PathBuf],
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, Box<dyn std::error::Error>> {
    let baseline_value: serde_json::Value = read_json(baseline_path)?;
    if baseline_value.get("captured_at_unix_ms").is_some()
        && baseline_value.get("started_at_unix_ms").is_none()
    {
        let baseline: MachineSnapshot = serde_json::from_value(baseline_value)?;
        let mut sections = Vec::new();
        for candidate_path in candidates {
            let candidate: MachineSnapshot = read_json(candidate_path)?;
            sections.push(mollow_report::render_snapshot_comparison(
                &baseline, &candidate, format, language,
            )?);
        }
        return Ok(sections.join("\n"));
    }

    let baseline: BenchmarkRun = serde_json::from_value(baseline_value)?;
    let mut sections = Vec::new();
    for candidate_path in candidates {
        let candidate: BenchmarkRun = read_json(candidate_path)?;
        let comparison = mollow_compare::compare_runs(&baseline, &candidate)?;
        sections.push(mollow_report::render_comparison(
            &comparison,
            format,
            language,
        )?);
    }
    Ok(sections.join("\n"))
}

fn render_archive_list(
    dir: &Path,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, Box<dyn std::error::Error>> {
    let entries = mollow_archive::list_runs(dir)?;
    match format {
        ReportFormat::Json => Ok(serde_json::to_string_pretty(&entries)?),
        ReportFormat::Markdown => {
            let title = match language {
                ReportLanguage::English => "# Archive entries\n\n",
                ReportLanguage::Chinese => "# 档案条目\n\n",
            };
            let mut output = title.to_owned();
            if entries.is_empty() {
                output.push_str(match language {
                    ReportLanguage::English => "No entries.\n",
                    ReportLanguage::Chinese => "暂无条目。\n",
                });
                return Ok(output);
            }
            output.push_str("| id | started_at_unix_ms | profile | hostname | build |\n|---|---:|---|---|---|\n");
            for entry in entries {
                let _ = writeln!(
                    output,
                    "| {} | {} | {} | {} | {} |",
                    entry.id,
                    entry.started_at_unix_ms,
                    entry.profile,
                    entry.hostname.as_deref().unwrap_or("-"),
                    entry.build_profile
                );
            }
            Ok(output)
        }
        ReportFormat::Terminal => {
            let markdown = render_archive_list(dir, ReportFormat::Markdown, language)?;
            Ok(markdown
                .replace("# Archive entries\n\n", "")
                .replace("# 档案条目\n\n", ""))
        }
        ReportFormat::Html => Ok(mollow_report::render_markdown_page(
            match language {
                ReportLanguage::English => "Archive",
                ReportLanguage::Chinese => "档案",
            },
            &render_archive_list(dir, ReportFormat::Markdown, language)?,
            language,
        )),
    }
}

fn render_archive_trend(
    dir: &Path,
    workload: &str,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, Box<dyn std::error::Error>> {
    let points = mollow_archive::trend(dir, workload)?;
    match format {
        ReportFormat::Json => Ok(serde_json::to_string_pretty(&points)?),
        ReportFormat::Markdown => {
            let title = match language {
                ReportLanguage::English => format!("# Trend for {workload}\n\n"),
                ReportLanguage::Chinese => format!("# {workload} 趋势\n\n"),
            };
            let mut output = title;
            if points.is_empty() {
                output.push_str(match language {
                    ReportLanguage::English => "No points.\n",
                    ReportLanguage::Chinese => "暂无数据点。\n",
                });
                return Ok(output);
            }
            output.push_str("| id | started_at_unix_ms | median_rate_per_second | status |\n|---|---:|---:|---|\n");
            for point in points {
                let _ = writeln!(
                    output,
                    "| {} | {} | {} | {} |",
                    point.id,
                    point.started_at_unix_ms,
                    point
                        .median_rate_per_second
                        .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                    point.status
                );
            }
            Ok(output)
        }
        ReportFormat::Terminal => {
            let markdown = render_archive_trend(dir, workload, ReportFormat::Markdown, language)?;
            Ok(markdown.lines().skip(2).collect::<Vec<_>>().join("\n"))
        }
        ReportFormat::Html => Ok(mollow_report::render_markdown_page(
            match language {
                ReportLanguage::English => "Trend",
                ReportLanguage::Chinese => "趋势",
            },
            &render_archive_trend(dir, workload, ReportFormat::Markdown, language)?,
            language,
        )),
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

impl From<CliWatchField> for WatchField {
    fn from(field: CliWatchField) -> Self {
        match field {
            CliWatchField::Memory => Self::Memory,
            CliWatchField::Power => Self::Power,
            CliWatchField::Thermal => Self::Thermal,
        }
    }
}
