use std::fmt::Write as _;

use mollow_core::{BenchmarkRun, ComparisonReport, MachineSnapshot};

mod localization;

use localization::{classification_name, status_name, title, yes_no};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Json,
    Terminal,
    Markdown,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLanguage {
    English,
    Chinese,
}

#[derive(Debug)]
pub enum ReportError {
    Serialization(serde_json::Error),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "failed to serialize report: {error}"),
        }
    }
}

impl std::error::Error for ReportError {}

/// Renders a machine snapshot as stable, pretty-printed JSON.
///
/// # Errors
///
/// Returns [`ReportError::Serialization`] if the snapshot cannot be encoded.
pub fn render_json(snapshot: &MachineSnapshot) -> Result<String, ReportError> {
    render_pretty_json(snapshot)
}

/// Renders a benchmark run as stable, pretty-printed JSON.
///
/// # Errors
///
/// Returns [`ReportError::Serialization`] if the benchmark cannot be encoded.
pub fn render_benchmark_json(benchmark: &BenchmarkRun) -> Result<String, ReportError> {
    render_pretty_json(benchmark)
}

/// Renders a machine snapshot in the requested representation.
///
/// # Errors
///
/// Returns [`ReportError`] when JSON serialization fails.
pub fn render_snapshot(
    snapshot: &MachineSnapshot,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, ReportError> {
    match format {
        ReportFormat::Json => render_json(snapshot),
        ReportFormat::Terminal => Ok(render_snapshot_terminal(snapshot, language)),
        ReportFormat::Markdown => Ok(render_snapshot_markdown(snapshot, language)),
        ReportFormat::Html => Ok(render_html(
            title(language, "Machine Snapshot", "机器快照"),
            &render_snapshot_markdown(snapshot, language),
            language,
        )),
    }
}

/// Renders a benchmark run in the requested representation.
///
/// # Errors
///
/// Returns [`ReportError`] when JSON serialization fails.
pub fn render_benchmark(
    benchmark: &BenchmarkRun,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, ReportError> {
    match format {
        ReportFormat::Json => render_benchmark_json(benchmark),
        ReportFormat::Terminal => Ok(render_benchmark_terminal(benchmark, language)),
        ReportFormat::Markdown => Ok(render_benchmark_markdown(benchmark, language)),
        ReportFormat::Html => Ok(render_html(
            title(language, "Performance Baseline", "性能基线"),
            &render_benchmark_markdown(benchmark, language),
            language,
        )),
    }
}

/// Renders a comparison in the requested representation.
///
/// # Errors
///
/// Returns [`ReportError`] when JSON serialization fails.
pub fn render_comparison(
    comparison: &ComparisonReport,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, ReportError> {
    match format {
        ReportFormat::Json => render_pretty_json(comparison),
        ReportFormat::Terminal => Ok(render_comparison_terminal(comparison, language)),
        ReportFormat::Markdown => Ok(render_comparison_markdown(comparison, language)),
        ReportFormat::Html => Ok(render_html(
            title(language, "Baseline Comparison", "基线对比"),
            &render_comparison_markdown(comparison, language),
            language,
        )),
    }
}

fn render_pretty_json(value: &impl serde::Serialize) -> Result<String, ReportError> {
    let mut report = serde_json::to_string_pretty(value).map_err(ReportError::Serialization)?;
    report.push('\n');
    Ok(report)
}

fn render_snapshot_terminal(snapshot: &MachineSnapshot, language: ReportLanguage) -> String {
    let mut output = String::new();
    line(
        &mut output,
        title(language, "Mollow Machine Snapshot", "Mollow 机器快照"),
    );
    line(
        &mut output,
        title(
            language,
            "\nWhat can this machine do?",
            "\n这台机器具备什么能力？",
        ),
    );
    append_snapshot_capabilities(&mut output, snapshot, language, "");
    line(
        &mut output,
        title(
            language,
            "\nWhat state is it in now?",
            "\n它当前处于什么状态？",
        ),
    );
    append_snapshot_state(&mut output, snapshot, language, "");
    line(
        &mut output,
        title(
            language,
            "\nWhat changed?",
            "\n相对历史记录发生了什么变化？",
        ),
    );
    line(
        &mut output,
        title(
            language,
            "  Capture another baseline and run `mollow compare`.",
            "  再采集一份基线并运行 `mollow compare`。",
        ),
    );
    output
}

fn render_snapshot_markdown(snapshot: &MachineSnapshot, language: ReportLanguage) -> String {
    let mut output = format!(
        "# {}\n\n## {}\n\n",
        title(language, "Machine Snapshot", "机器快照"),
        title(
            language,
            "What can this machine do?",
            "这台机器具备什么能力？"
        )
    );
    append_snapshot_capabilities(&mut output, snapshot, language, "- ");
    let _ = write!(
        output,
        "\n## {}\n\n",
        title(language, "What state is it in now?", "它当前处于什么状态？")
    );
    append_snapshot_state(&mut output, snapshot, language, "- ");
    let _ = writeln!(
        output,
        "\n## {}\n\n{}\n",
        title(language, "What changed?", "相对历史记录发生了什么变化？"),
        title(
            language,
            "Capture another baseline and run `mollow compare`.",
            "再采集一份基线并运行 `mollow compare`。"
        )
    );
    output
}

fn render_benchmark_terminal(benchmark: &BenchmarkRun, language: ReportLanguage) -> String {
    let mut output = String::new();
    line(
        &mut output,
        title(language, "Mollow Performance Baseline", "Mollow 性能基线"),
    );
    line(
        &mut output,
        &format!(
            "{}: {:?} | {}: {}",
            title(language, "Profile", "测试档位"),
            benchmark.profile,
            title(language, "Build", "构建"),
            benchmark.context.build_profile
        ),
    );
    append_workload_terminal(&mut output, "CPU", &benchmark.cpu, language);
    append_workload_terminal(&mut output, "Memory", &benchmark.memory, language);
    append_workload_terminal(&mut output, "Storage", &benchmark.storage, language);
    append_warnings(&mut output, &benchmark.warnings, language, "  ");
    output
}

fn render_benchmark_markdown(benchmark: &BenchmarkRun, language: ReportLanguage) -> String {
    let mut output = format!(
        "# {}\n\n- {}: `{:?}`\n- {}: `{}`\n\n| {} | {} | {} | {} |\n|---|---:|---:|---|\n",
        title(language, "Performance Baseline", "性能基线"),
        title(language, "Profile", "测试档位"),
        benchmark.profile,
        title(language, "Build", "构建"),
        benchmark.context.build_profile,
        title(language, "Workload", "工作负载"),
        title(language, "Median rate", "中位速率"),
        title(language, "Variation", "波动"),
        title(language, "Status", "状态"),
    );
    append_workload_row(&mut output, "CPU", &benchmark.cpu, language);
    append_workload_row(&mut output, "Memory", &benchmark.memory, language);
    append_workload_row(&mut output, "Storage", &benchmark.storage, language);
    let _ = write!(output, "\n## {}\n\n", title(language, "Warnings", "警告"));
    if benchmark.warnings.is_empty() {
        output.push_str(title(language, "None.\n", "无。\n"));
    } else {
        for warning in &benchmark.warnings {
            let _ = writeln!(output, "- {warning}");
        }
    }
    output
}

fn render_comparison_terminal(comparison: &ComparisonReport, language: ReportLanguage) -> String {
    let mut output = String::new();
    line(
        &mut output,
        title(language, "Mollow Baseline Comparison", "Mollow 基线对比"),
    );
    line(
        &mut output,
        &format!(
            "{}: {}",
            title(language, "Comparable", "可比"),
            yes_no(comparison.comparable, language)
        ),
    );
    append_comparison_line(&mut output, "CPU", &comparison.cpu, language);
    append_comparison_line(&mut output, "Memory", &comparison.memory, language);
    append_comparison_line(&mut output, "Storage", &comparison.storage, language);
    if !comparison.machine_changes.is_empty() {
        line(
            &mut output,
            title(language, "\nMachine changes:", "\n机器变化："),
        );
        for change in &comparison.machine_changes {
            line(
                &mut output,
                &format!(
                    "  {}: {} -> {}",
                    change.field,
                    change.baseline.as_deref().unwrap_or("-"),
                    change.candidate.as_deref().unwrap_or("-")
                ),
            );
        }
    }
    append_warnings(&mut output, &comparison.reasons, language, "  ");
    output
}

fn render_comparison_markdown(comparison: &ComparisonReport, language: ReportLanguage) -> String {
    let mut output = format!(
        "# {}\n\n- {}: **{}**\n\n| {} | {} | {} | {} |\n|---|---:|---:|---|\n",
        title(language, "Baseline Comparison", "基线对比"),
        title(language, "Comparable", "可比"),
        yes_no(comparison.comparable, language),
        title(language, "Workload", "工作负载"),
        title(language, "Baseline", "基线"),
        title(language, "Candidate", "候选"),
        title(language, "Change", "变化"),
    );
    append_comparison_row(&mut output, "CPU", &comparison.cpu, language);
    append_comparison_row(&mut output, "Memory", &comparison.memory, language);
    append_comparison_row(&mut output, "Storage", &comparison.storage, language);
    let _ = write!(
        output,
        "\n## {}\n\n",
        title(language, "Machine changes", "机器变化")
    );
    if comparison.machine_changes.is_empty() {
        output.push_str(title(language, "None.\n", "无。\n"));
    } else {
        for change in &comparison.machine_changes {
            let _ = writeln!(
                output,
                "- `{}`: `{}` → `{}`\n",
                change.field,
                change.baseline.as_deref().unwrap_or("-"),
                change.candidate.as_deref().unwrap_or("-")
            );
        }
    }
    if !comparison.reasons.is_empty() {
        let _ = write!(
            output,
            "\n## {}\n\n",
            title(language, "Comparability notes", "可比性说明")
        );
        for reason in &comparison.reasons {
            let _ = writeln!(output, "- {reason}");
        }
    }
    output
}

fn render_html(page_title: &str, markdown: &str, language: ReportLanguage) -> String {
    format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{}</title><style>\
:root{{color-scheme:light dark}}body{{font:15px/1.6 system-ui,sans-serif;max-width:960px;\
margin:40px auto;padding:0 24px}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;\
background:#8881;padding:24px;border-radius:12px}}\
</style></head><body><pre>{}</pre></body></html>\n",
        title(language, "en", "zh-CN"),
        escape_html(page_title),
        escape_html(markdown)
    )
}

fn append_snapshot_capabilities(
    output: &mut String,
    snapshot: &MachineSnapshot,
    language: ReportLanguage,
    prefix: &str,
) {
    let system = snapshot.system.value.as_ref();
    let cpu = snapshot.cpu.value.as_ref();
    line(
        output,
        &format!(
            "{prefix}{}: {} / {}",
            title(language, "System", "系统"),
            system.map_or("-", |value| value.os_name.as_str()),
            system.map_or("-", |value| value.architecture.as_str())
        ),
    );
    line(
        output,
        &format!(
            "{prefix}{}: {} ({} {})",
            title(language, "CPU", "处理器"),
            cpu.and_then(|value| value.model.as_deref()).unwrap_or("-"),
            cpu.map_or(0, |value| value.physical_cores.unwrap_or(0)),
            title(language, "physical cores", "物理核心")
        ),
    );
    line(
        output,
        &format!(
            "{prefix}GPU: {} | {}: {} | {}: {}",
            snapshot.gpu.value.as_ref().map_or_else(
                || status_name(&snapshot.gpu.status, language).to_owned(),
                |gpus| gpus
                    .iter()
                    .map(|gpu| gpu.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            title(language, "Media", "媒体"),
            snapshot.media.value.as_ref().map_or_else(
                || status_name(&snapshot.media.status, language).to_owned(),
                |media| format!(
                    "{} [{}]",
                    media.backend,
                    media.hardware_decode_codecs.join(",")
                )
            ),
            title(language, "Runtimes", "运行时"),
            status_name(&snapshot.runtimes.status, language)
        ),
    );
}

fn append_snapshot_state(
    output: &mut String,
    snapshot: &MachineSnapshot,
    language: ReportLanguage,
    prefix: &str,
) {
    if let Some(memory) = snapshot.memory.value.as_ref() {
        line(
            output,
            &format!(
                "{prefix}{}: {} / {}",
                title(language, "Memory available / total", "可用内存 / 总内存"),
                memory.available_bytes.map_or("-".to_owned(), bytes),
                bytes(memory.total_bytes)
            ),
        );
    }
    line(
        output,
        &format!(
            "{prefix}{}: {} | {}: {}",
            title(language, "Power", "电源"),
            snapshot.power.value.as_ref().map_or_else(
                || status_name(&snapshot.power.status, language).to_owned(),
                |power| format!(
                    "{} {}",
                    power.source,
                    power
                        .battery_percent
                        .map_or("-".to_owned(), |value| format!("{value}%"))
                )
            ),
            title(language, "Thermal", "温控"),
            snapshot.thermal.value.as_ref().map_or_else(
                || status_name(&snapshot.thermal.status, language).to_owned(),
                |thermal| thermal.state.clone()
            )
        ),
    );
    if let Some(system) = snapshot.system.value.as_ref() {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Hostname", "主机名"),
                system.hostname.as_deref().unwrap_or("-")
            ),
        );
    }
}

fn append_workload_terminal(
    output: &mut String,
    name: &str,
    capability: &mollow_core::Capability<mollow_core::WorkloadResult>,
    language: ReportLanguage,
) {
    if let Some(value) = capability.value.as_ref() {
        line(
            output,
            &format!(
                "{name}: {} {}/s | {}: {:.2}%",
                value.summary.median_rate_per_second,
                title(language, "bytes", "字节"),
                title(language, "Variation", "波动"),
                f64::from(value.summary.variation_basis_points) / 100.0
            ),
        );
    } else {
        line(
            output,
            &format!("{name}: {}", status_name(&capability.status, language)),
        );
    }
}

fn append_workload_row(
    output: &mut String,
    name: &str,
    capability: &mollow_core::Capability<mollow_core::WorkloadResult>,
    language: ReportLanguage,
) {
    if let Some(value) = capability.value.as_ref() {
        let _ = writeln!(
            output,
            "| {name} | {} | {:.2}% | available |\n",
            value.summary.median_rate_per_second,
            f64::from(value.summary.variation_basis_points) / 100.0
        );
    } else {
        let _ = writeln!(
            output,
            "| {name} | - | - | {} |\n",
            status_name(&capability.status, language)
        );
    }
}

fn append_comparison_line(
    output: &mut String,
    name: &str,
    comparison: &mollow_core::WorkloadComparison,
    language: ReportLanguage,
) {
    line(
        output,
        &format!(
            "{name}: {} ({})",
            classification_name(comparison.classification, language),
            format_change(comparison.change_basis_points)
        ),
    );
}

fn append_comparison_row(
    output: &mut String,
    name: &str,
    comparison: &mollow_core::WorkloadComparison,
    language: ReportLanguage,
) {
    let _ = writeln!(
        output,
        "| {name} | {} | {} | {} ({}) |\n",
        optional_rate(comparison.baseline_rate_per_second),
        optional_rate(comparison.candidate_rate_per_second),
        format_change(comparison.change_basis_points),
        classification_name(comparison.classification, language)
    );
}

fn append_warnings(
    output: &mut String,
    warnings: &[String],
    language: ReportLanguage,
    prefix: &str,
) {
    if !warnings.is_empty() {
        line(output, title(language, "\nWarnings:", "\n警告："));
        for warning in warnings {
            line(output, &format!("{prefix}- {warning}"));
        }
    }
}

fn line(output: &mut String, value: &str) {
    output.push_str(value);
    output.push('\n');
}

fn bytes(value: u64) -> String {
    const GIB: u64 = 1024 * 1024 * 1024;
    let whole = value / GIB;
    let hundredths = value % GIB * 100 / GIB;
    format!("{whole}.{hundredths:02} GiB")
}

fn optional_rate(value: Option<u64>) -> String {
    value.map_or("-".to_owned(), |value| value.to_string())
}

fn format_change(value: Option<i32>) -> String {
    value.map_or("-".to_owned(), |value| {
        format!("{:+.2}%", f64::from(value) / 100.0)
    })
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod test_fixtures;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use mollow_core::{
        Capability, CpuInfo, DataSource, MachineSnapshot, MemoryInfo, SCHEMA_VERSION, SystemInfo,
    };

    use super::*;
    use crate::test_fixtures::{fixture_benchmark, fixture_snapshot};

    #[test]
    fn json_report_is_pretty_and_versioned() {
        let source = DataSource {
            provider: "fixture".to_owned(),
            detail: None,
        };
        let snapshot = MachineSnapshot {
            schema_version: SCHEMA_VERSION.to_owned(),
            mollow_version: "0.1.0".to_owned(),
            captured_at_unix_ms: 1234,
            system: Capability::available(
                SystemInfo {
                    os_name: "FixtureOS".to_owned(),
                    os_version: None,
                    kernel_version: None,
                    architecture: "fixture64".to_owned(),
                    hostname: None,
                },
                source.clone(),
            ),
            cpu: Capability::available(
                CpuInfo {
                    model: None,
                    physical_cores: None,
                    logical_cores: 2,
                    features: vec!["fixture_simd".to_owned()],
                },
                source.clone(),
            ),
            memory: Capability::available(
                MemoryInfo {
                    total_bytes: 1024,
                    available_bytes: None,
                    swap: Capability::unsupported("fixture"),
                },
                source,
            ),
            storage: Capability::available(
                Vec::new(),
                DataSource {
                    provider: "fixture".to_owned(),
                    detail: None,
                },
            ),
            gpu: Capability::unsupported("future phase"),
            media: Capability::unsupported("future phase"),
            power: Capability::unsupported("future phase"),
            thermal: Capability::unsupported("future phase"),
            runtimes: Capability::available(
                Vec::new(),
                DataSource {
                    provider: "fixture".to_owned(),
                    detail: None,
                },
            ),
            warnings: Vec::new(),
        };

        let report = render_json(&snapshot).expect("snapshot should serialize");

        assert!(report.contains("\"schema_version\": \"3.0.0\""));
        assert!(report.ends_with('\n'));
    }

    #[test]
    fn bundled_schema_matches_the_snapshot_schema_version() {
        let schema_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        let schema_path = schema_directory.join("machine-snapshot-v3.schema.json");
        let schema = fs::read_to_string(schema_path).expect("snapshot schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("snapshot schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            SCHEMA_VERSION
        );
        assert_eq!(schema["properties"]["gpu"]["$ref"], "#/$defs/gpuCapability");
        assert_eq!(
            schema["properties"]["media"]["$ref"],
            "#/$defs/mediaCapability"
        );
        assert_eq!(
            schema["properties"]["power"]["$ref"],
            "#/$defs/powerCapability"
        );
        assert_eq!(
            schema["properties"]["thermal"]["$ref"],
            "#/$defs/thermalCapability"
        );

        let legacy_schema =
            fs::read_to_string(schema_directory.join("machine-snapshot-v1.schema.json"))
                .expect("legacy snapshot schema should remain available");
        let legacy_schema: serde_json::Value =
            serde_json::from_str(&legacy_schema).expect("legacy schema should be valid JSON");
        assert_eq!(
            legacy_schema["properties"]["schema_version"]["const"],
            "1.0.0"
        );
    }

    #[test]
    fn bundled_benchmark_schema_matches_the_benchmark_version() {
        let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/benchmark-run-v2.schema.json");
        let schema = fs::read_to_string(schema_path).expect("benchmark schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("benchmark schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            mollow_core::BENCHMARK_SCHEMA_VERSION
        );
        assert_eq!(schema["properties"]["cpu"]["$ref"], "#/$defs/capability");
    }

    #[test]
    fn bundled_comparison_schema_matches_the_comparison_version() {
        let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/comparison-report-v1.schema.json");
        let schema = fs::read_to_string(schema_path).expect("comparison schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("comparison schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            mollow_core::COMPARISON_SCHEMA_VERSION
        );
    }

    #[test]
    fn chinese_snapshot_terminal_answers_the_capability_question() {
        let snapshot = fixture_snapshot("<fixture>");

        let report = render_snapshot(&snapshot, ReportFormat::Terminal, ReportLanguage::Chinese)
            .expect("snapshot should render");
        let html = render_snapshot(&snapshot, ReportFormat::Html, ReportLanguage::Chinese)
            .expect("snapshot HTML should render");

        assert!(report.contains("这台机器具备什么能力"));
        assert!(report.contains("<fixture>"));
        assert!(html.contains("<html lang=\"zh-CN\">"));
    }

    #[test]
    fn benchmark_markdown_includes_variation_and_warnings() {
        let benchmark = fixture_benchmark();

        let report = render_benchmark(&benchmark, ReportFormat::Markdown, ReportLanguage::English)
            .expect("benchmark should render");

        assert!(report.contains("# Performance Baseline"));
        assert!(report.contains("Variation"));
        assert!(report.contains("fixture warning"));
    }

    #[test]
    fn html_report_escapes_machine_control_characters() {
        let snapshot = fixture_snapshot("<script>alert('x')</script>");

        let report = render_snapshot(&snapshot, ReportFormat::Html, ReportLanguage::English)
            .expect("snapshot should render");

        assert!(!report.contains("<script>alert"));
        assert!(report.contains("&lt;script&gt;"));
        assert!(report.contains("<html lang=\"en\">"));
    }
}
