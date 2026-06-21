use std::fmt::Write as _;

use chrono::{Local, TimeZone};
use mollow_core::{
    BenchmarkRun, ComparisonReport, HardwareContext, MachineSnapshot, WatchField, WatchReading,
};

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

/// Renders markdown content as a semantic HTML page.
#[must_use]
pub fn render_markdown_page(page_title: &str, markdown: &str, language: ReportLanguage) -> String {
    render_html(page_title, markdown, language)
}

/// Renders a machine snapshot comparison in the requested representation.
///
/// # Errors
///
/// Returns [`ReportError`] when JSON serialization fails.
pub fn render_snapshot_comparison(
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
    format: ReportFormat,
    language: ReportLanguage,
) -> Result<String, ReportError> {
    let comparison = mollow_compare::compare_snapshots(baseline, candidate);
    render_comparison(&comparison, format, language)
}

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
        ReportFormat::Html => {
            let markdown = render_snapshot_markdown(snapshot, language);
            let mut page = render_html(
                title(language, "Machine Snapshot", "机器快照"),
                &markdown,
                language,
            );
            if let Some(diagrams) = render_hardware_context_diagrams(snapshot, language) {
                page = page.replace("</body>", &format!("{diagrams}</body>"));
            }
            Ok(page)
        }
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

/// Renders a compact live watch frame for memory, power, and thermal readings.
#[must_use]
pub fn render_watch_frame(
    reading: &WatchReading,
    fields: &[WatchField],
    language: ReportLanguage,
) -> String {
    let mut output = String::new();
    line(
        &mut output,
        title(language, "Mollow Watch", "Mollow 实时监控"),
    );
    line(
        &mut output,
        &format!(
            "  {}: {}",
            title(language, "Updated", "更新时间"),
            format_watch_timestamp(reading.captured_at_unix_ms)
        ),
    );
    for field in fields {
        match field {
            WatchField::Memory => append_watch_memory_line(&mut output, reading, language),
            WatchField::Power => append_watch_power_line(&mut output, reading, language),
            WatchField::Thermal => append_watch_thermal_line(&mut output, reading, language),
        }
    }
    line(
        &mut output,
        title(language, "\nPress Ctrl+C to stop.", "\n按 Ctrl+C 停止。"),
    );
    output
}

fn format_watch_timestamp(unix_ms: u64) -> String {
    let Ok(secs) = i64::try_from(unix_ms / 1000) else {
        return unix_ms.to_string();
    };
    let millis = unix_ms % 1000;
    let Ok(nanos) = u32::try_from(millis * 1_000_000) else {
        return unix_ms.to_string();
    };

    Local.timestamp_opt(secs, nanos).single().map_or_else(
        || unix_ms.to_string(),
        |datetime| datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

fn append_watch_memory_line(output: &mut String, reading: &WatchReading, language: ReportLanguage) {
    let label = title(language, "Memory available / total", "可用内存 / 总内存");
    if let Some(memory) = reading.memory.value.as_ref() {
        line(
            output,
            &format!(
                "  {label}: {} / {}",
                memory.available_bytes.map_or("-".to_owned(), bytes),
                bytes(memory.total_bytes)
            ),
        );
    } else {
        line(
            output,
            &format!(
                "  {label}: {}",
                status_name(&reading.memory.status, language)
            ),
        );
    }
}

fn append_watch_power_line(output: &mut String, reading: &WatchReading, language: ReportLanguage) {
    let label = title(language, "Power", "电源");
    let value = reading.power.value.as_ref().map_or_else(
        || status_name(&reading.power.status, language).to_owned(),
        |power| format_power_info(power, language),
    );
    let highlight = reading
        .power
        .value
        .as_ref()
        .is_some_and(|power| power.source == "battery");
    line(
        output,
        &format!("  {label}: {}", highlight_line(value, highlight)),
    );
}

fn append_watch_thermal_line(
    output: &mut String,
    reading: &WatchReading,
    language: ReportLanguage,
) {
    let label = title(language, "Thermal", "温控");
    let value = reading.thermal.value.as_ref().map_or_else(
        || status_name(&reading.thermal.status, language).to_owned(),
        format_thermal_info,
    );
    let highlight = reading
        .thermal
        .value
        .as_ref()
        .is_some_and(|thermal| thermal.state == "warning" || thermal.state == "critical");
    line(
        output,
        &format!("  {label}: {}", highlight_line(value, highlight)),
    );
}

fn highlight_line(value: String, highlight: bool) -> String {
    if highlight {
        format!("\x1b[33m{value}\x1b[0m")
    } else {
        value
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
        &format!(
            "  {}: {} | {}: {}",
            title(language, "Schema", "Schema"),
            snapshot.schema_version,
            title(language, "Mollow", "Mollow"),
            snapshot.mollow_version
        ),
    );
    line(
        &mut output,
        title(language, "\nHardware & runtime", "\n硬件与开发环境"),
    );
    append_snapshot_capabilities(&mut output, snapshot, language, "  ");
    append_hardware_context(&mut output, snapshot, language, "  ");
    line(
        &mut output,
        title(language, "\nCurrent environment", "\n当前环境"),
    );
    append_snapshot_state(&mut output, snapshot, language, "  ");
    append_warnings(&mut output, &snapshot.warnings, language, "  ");
    line(&mut output, title(language, "\nNote", "\n说明"));
    line(
        &mut output,
        title(
            language,
            "  Inspect is a single-point snapshot. Run `mollow capture` and `mollow compare` for baseline diffs.",
            "  inspect 为单点快照，不含历史对比；请运行 `mollow capture` 后再 `mollow compare`。",
        ),
    );
    output
}

fn render_snapshot_markdown(snapshot: &MachineSnapshot, language: ReportLanguage) -> String {
    let mut output = format!(
        "# {}\n\n- {}: `{}`\n- {}: `{}`\n\n## {}\n\n",
        title(language, "Machine Snapshot", "机器快照"),
        title(language, "Schema", "Schema"),
        snapshot.schema_version,
        title(language, "Mollow", "Mollow"),
        snapshot.mollow_version,
        title(language, "Hardware & runtime", "硬件与开发环境"),
    );
    append_snapshot_capabilities(&mut output, snapshot, language, "- ");
    append_hardware_context(&mut output, snapshot, language, "- ");
    let _ = write!(
        output,
        "\n## {}\n\n",
        title(language, "Current environment", "当前环境")
    );
    append_snapshot_state(&mut output, snapshot, language, "- ");
    if !snapshot.warnings.is_empty() {
        let _ = write!(output, "\n## {}\n\n", title(language, "Warnings", "警告"));
        append_warnings(&mut output, &snapshot.warnings, language, "- ");
    }
    let _ = writeln!(
        output,
        "\n## {}\n\n{}\n",
        title(language, "Note", "说明"),
        title(
            language,
            "Inspect is a single-point snapshot. Run `mollow capture` and `mollow compare` for baseline diffs.",
            "inspect 为单点快照，不含历史对比；请运行 `mollow capture` 后再 `mollow compare`。",
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
    append_workload_terminal(&mut output, "GPU", &benchmark.gpu, language);
    append_workload_terminal(&mut output, "Media", &benchmark.media, language);
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
    append_workload_row(&mut output, "GPU", &benchmark.gpu, language);
    append_workload_row(&mut output, "Media", &benchmark.media, language);
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
    append_comparison_line(&mut output, "GPU", &comparison.gpu, language);
    append_comparison_line(&mut output, "Media", &comparison.media, language);
    if !comparison.environment_warnings.is_empty() {
        line(
            &mut output,
            title(language, "\nEnvironment warnings:", "\n环境警告："),
        );
        append_warnings(
            &mut output,
            &comparison.environment_warnings,
            language,
            "  ",
        );
    }
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
    append_comparison_row(&mut output, "GPU", &comparison.gpu, language);
    append_comparison_row(&mut output, "Media", &comparison.media, language);
    if !comparison.environment_warnings.is_empty() {
        let _ = write!(
            output,
            "\n## {}\n\n",
            title(language, "Environment warnings", "环境警告")
        );
        for warning in &comparison.environment_warnings {
            let _ = writeln!(output, "- {warning}");
        }
    }
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
margin:40px auto;padding:0 24px;color:CanvasText;background:Canvas}}\
h1,h2{{line-height:1.25}}table{{width:100%;border-collapse:collapse;margin:16px 0}}\
th,td{{border:1px solid color-mix(in srgb,CanvasText 20%,transparent);padding:8px 12px;text-align:left}}\
th{{background:color-mix(in srgb,CanvasText 8%,transparent)}}ul{{padding-left:1.4rem}}\
.warning{{background:color-mix(in srgb,#f5a623 18%,transparent);border-radius:12px;padding:16px}}\
.architecture-diagram{{margin:24px 0;padding:16px;border:1px solid color-mix(in srgb,CanvasText 15%,transparent);border-radius:12px}}\
.architecture-diagram svg{{width:100%;height:auto;display:block}}\
</style></head><body>{}</body></html>\n",
        title(language, "en", "zh-CN"),
        escape_html(page_title),
        markdown_to_semantic_html(markdown)
    )
}

fn markdown_to_semantic_html(markdown: &str) -> String {
    let mut html = String::new();
    let mut in_table = false;
    let mut table_header_written = false;

    for line in markdown.lines() {
        if let Some(title) = line.strip_prefix("# ") {
            close_table(&mut html, &mut in_table);
            let _ = writeln!(html, "<h1>{}</h1>", escape_html(title));
            continue;
        }
        if let Some(title) = line.strip_prefix("## ") {
            close_table(&mut html, &mut in_table);
            let _ = writeln!(html, "<h2>{}</h2>", escape_html(title));
            continue;
        }
        if line.starts_with("| ") && line.ends_with('|') {
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if cells
                .iter()
                .all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
            {
                continue;
            }
            if !in_table {
                html.push_str("<table>\n");
                in_table = true;
                table_header_written = false;
            }
            if !table_header_written {
                html.push_str("<thead><tr>");
                for cell in &cells {
                    let _ = write!(html, "<th>{}</th>", escape_html(cell));
                }
                html.push_str("</tr></thead>\n<tbody>\n");
                table_header_written = true;
                continue;
            }
            html.push_str("<tr>");
            for cell in &cells {
                let _ = write!(html, "<td>{}</td>", escape_html(cell));
            }
            html.push_str("</tr>\n");
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            close_table(&mut html, &mut in_table);
            let _ = writeln!(html, "<ul><li>{}</li></ul>", escape_html(item));
            continue;
        }
        if line.trim().is_empty() {
            close_table(&mut html, &mut in_table);
            continue;
        }
        close_table(&mut html, &mut in_table);
        let _ = writeln!(html, "<p>{}</p>", escape_html(line));
    }
    close_table(&mut html, &mut in_table);
    html
}

fn close_table(html: &mut String, in_table: &mut bool) {
    if *in_table {
        html.push_str("</tbody></table>\n");
        *in_table = false;
    }
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
            "{prefix}{}: {} {} / {} ({})",
            title(language, "System", "系统"),
            system.map_or("-", |value| value.os_name.as_str()),
            system
                .and_then(|value| value.os_version.as_deref())
                .unwrap_or("-"),
            system.map_or("-", |value| value.architecture.as_str()),
            system
                .and_then(|value| value.kernel_version.as_deref())
                .unwrap_or("-")
        ),
    );
    line(
        output,
        &format!(
            "{prefix}{}: {} — {} / {} {} [{}]",
            title(language, "CPU", "处理器"),
            cpu.and_then(|value| value.model.as_deref()).unwrap_or("-"),
            cpu.map_or(0, |value| value.physical_cores.unwrap_or(0)),
            cpu.map_or(0, |value| value.logical_cores),
            title(language, "physical / logical cores", "物理 / 逻辑核心"),
            cpu.map_or("-".to_owned(), |value| {
                if value.features.is_empty() {
                    "-".to_owned()
                } else {
                    value.features.join(",")
                }
            })
        ),
    );
    line(
        output,
        &format!(
            "{prefix}GPU: {}",
            snapshot.gpu.value.as_ref().map_or_else(
                || status_name(&snapshot.gpu.status, language).to_owned(),
                |gpus| gpus
                    .iter()
                    .map(|gpu| {
                        if gpu.apis.is_empty() {
                            gpu.name.clone()
                        } else {
                            format!("{} [{}]", gpu.name, gpu.apis.join(","))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        ),
    );
    line(
        output,
        &format!(
            "{prefix}{}: {}",
            title(language, "Media", "媒体"),
            snapshot.media.value.as_ref().map_or_else(
                || status_name(&snapshot.media.status, language).to_owned(),
                |media| format_media_info(media, language)
            )
        ),
    );
    append_storage_lines(output, snapshot, language, prefix);
    line(
        output,
        &format!(
            "{prefix}{}: {}",
            title(language, "Runtimes", "开发工具"),
            format_runtimes(snapshot, language)
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
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Swap", "交换区"),
                format_swap(&memory.swap, language)
            ),
        );
        append_memory_modules(output, memory, language, prefix);
    } else {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Memory", "内存"),
                status_name(&snapshot.memory.status, language)
            ),
        );
    }
    line(
        output,
        &format!(
            "{prefix}{}: {}",
            title(language, "Power", "电源"),
            snapshot.power.value.as_ref().map_or_else(
                || status_name(&snapshot.power.status, language).to_owned(),
                |power| format_power_info(power, language)
            )
        ),
    );
    line(
        output,
        &format!(
            "{prefix}{}: {}",
            title(language, "Thermal", "温控"),
            snapshot.thermal.value.as_ref().map_or_else(
                || status_name(&snapshot.thermal.status, language).to_owned(),
                format_thermal_info,
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

fn append_storage_lines(
    output: &mut String,
    snapshot: &MachineSnapshot,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(volumes) = snapshot.storage.value.as_ref() else {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Storage", "存储卷"),
                status_name(&snapshot.storage.status, language)
            ),
        );
        return;
    };
    if volumes.is_empty() {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Storage", "存储卷"),
                title(language, "no volumes", "无卷")
            ),
        );
        return;
    }
    let mut ordered = volumes.clone();
    ordered.sort_by(|left, right| {
        let left_root = left.mount_point == "/";
        let right_root = right.mount_point == "/";
        right_root
            .cmp(&left_root)
            .then_with(|| left.mount_point.cmp(&right.mount_point))
    });
    for volume in ordered.into_iter().take(3) {
        line(
            output,
            &format!(
                "{prefix}{}: {} {} {} / {} ({})",
                title(language, "Storage", "存储卷"),
                volume.mount_point,
                volume.name.as_deref().unwrap_or("-"),
                bytes(volume.available_bytes),
                bytes(volume.total_bytes),
                volume.file_system.as_deref().unwrap_or("-")
            ),
        );
    }
}

fn format_media_info(media: &mollow_core::MediaInfo, language: ReportLanguage) -> String {
    let decode = if media.hardware_decode_codecs.is_empty() {
        "-".to_owned()
    } else {
        media.hardware_decode_codecs.join(",")
    };
    let encode = if media.hardware_encode_codecs.is_empty() {
        "-".to_owned()
    } else {
        media.hardware_encode_codecs.join(",")
    };
    format!(
        "{} | {}: [{}] | {}: [{}]",
        media.backend,
        title(language, "decode", "硬解"),
        decode,
        title(language, "encode", "硬编"),
        encode
    )
}

fn format_runtimes(snapshot: &MachineSnapshot, language: ReportLanguage) -> String {
    snapshot.runtimes.value.as_ref().map_or_else(
        || status_name(&snapshot.runtimes.status, language).to_owned(),
        |runtimes| {
            if runtimes.is_empty() {
                title(language, "none detected", "未检测到").to_owned()
            } else {
                runtimes
                    .iter()
                    .map(|runtime| format!("{} {}", runtime.name, runtime.version))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        },
    )
}

fn format_swap(
    swap: &mollow_core::Capability<mollow_core::SwapInfo>,
    language: ReportLanguage,
) -> String {
    swap.value.as_ref().map_or_else(
        || status_name(&swap.status, language).to_owned(),
        |swap| format!("{} / {}", bytes(swap.used_bytes), bytes(swap.total_bytes)),
    )
}

fn format_power_info(power: &mollow_core::PowerInfo, language: ReportLanguage) -> String {
    let mut parts = vec![power.source.clone()];
    if let Some(percent) = power.battery_percent {
        parts.push(format!("{percent}%"));
    }
    if let Some(charging) = power.charging {
        parts.push(
            title(
                language,
                if charging { "charging" } else { "not charging" },
                if charging { "充电中" } else { "未充电" },
            )
            .to_owned(),
        );
    }
    if let Some(low_power_mode) = power.low_power_mode {
        parts.push(format!(
            "{}: {}",
            title(language, "low power", "低电量模式"),
            title(
                language,
                if low_power_mode { "on" } else { "off" },
                if low_power_mode { "开启" } else { "关闭" },
            )
        ));
    }
    parts.join(" | ")
}

fn format_thermal_info(thermal: &mollow_core::ThermalInfo) -> String {
    let mut parts = vec![thermal.state.clone()];
    if let Some(temperature) = thermal.temperature_milli_celsius {
        let whole = temperature / 1000;
        let fraction = (temperature % 1000).unsigned_abs();
        parts.push(format!("{whole}.{fraction:03} C"));
    }
    if let Some(sensor) = thermal.sensor.as_deref() {
        parts.push(sensor.to_owned());
    }
    parts.join(" | ")
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

fn append_memory_modules(
    output: &mut String,
    memory: &mollow_core::MemoryInfo,
    language: ReportLanguage,
    prefix: &str,
) {
    match memory.modules.value.as_ref() {
        Some(modules) if !modules.is_empty() => {
            for module in modules {
                line(
                    output,
                    &format!(
                        "{prefix}{}: {} {} @ {} [{}]",
                        title(language, "Memory module", "内存条"),
                        module.slot.as_deref().unwrap_or("-"),
                        module.mem_type.as_deref().unwrap_or("-"),
                        module
                            .speed_mts
                            .map_or_else(|| "-".to_owned(), |speed| speed.to_string()),
                        module.size_bytes.map_or_else(|| "-".to_owned(), bytes)
                    ),
                );
            }
        }
        _ => line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Memory modules", "内存条详情"),
                status_name(&memory.modules.status, language)
            ),
        ),
    }
}

fn append_hardware_context(
    output: &mut String,
    snapshot: &MachineSnapshot,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(context) = snapshot.hardware_context.value.as_ref() else {
        if snapshot.hardware_context.status != mollow_core::CapabilityStatus::Unsupported {
            line(
                output,
                &format!(
                    "{prefix}{}: {}",
                    title(language, "Hardware catalog", "硬件目录"),
                    status_name(&snapshot.hardware_context.status, language)
                ),
            );
        }
        return;
    };

    line(
        output,
        title(
            language,
            &format!("\n{prefix}Hardware catalog ({})", context.catalog_version),
            &format!("\n{prefix}硬件目录 ({})", context.catalog_version),
        ),
    );
    append_cpu_catalog_match(output, context, language, prefix);
    append_gpu_catalog_matches(output, context, language, prefix);
    append_memory_catalog_match(output, context, language, prefix);
    append_benchmark_reference(output, context, language, prefix);
}

fn append_cpu_catalog_match(
    output: &mut String,
    context: &HardwareContext,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(cpu) = context.cpu.value.as_ref() else {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "CPU catalog", "CPU 目录"),
                status_name(&context.cpu.status, language)
            ),
        );
        return;
    };
    line(
        output,
        &format!(
            "{prefix}{}: {} ({confidence:?})",
            title(language, "CPU catalog", "CPU 目录"),
            cpu.matched_model,
            confidence = cpu.confidence
        ),
    );
    append_catalog_detail(
        output,
        prefix,
        title(language, "Codename", "代号"),
        cpu.codename.as_deref(),
    );
    append_catalog_detail(
        output,
        prefix,
        title(language, "Architecture", "架构"),
        cpu.architecture_summary.as_deref(),
    );
    append_catalog_detail(
        output,
        prefix,
        title(language, "Reference score", "参考分数"),
        cpu.reference_score
            .map(|score| score.to_string())
            .as_deref(),
    );
}

fn append_gpu_catalog_matches(
    output: &mut String,
    context: &HardwareContext,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(gpus) = context.gpu.value.as_ref() else {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "GPU catalog", "GPU 目录"),
                status_name(&context.gpu.status, language)
            ),
        );
        return;
    };
    for gpu in gpus {
        line(
            output,
            &format!(
                "{prefix}{}: {} ({confidence:?})",
                title(language, "GPU catalog", "GPU 目录"),
                gpu.matched_model,
                confidence = gpu.confidence
            ),
        );
        append_catalog_detail(
            output,
            prefix,
            title(language, "Architecture", "架构"),
            gpu.architecture_summary.as_deref(),
        );
    }
}

fn append_memory_catalog_match(
    output: &mut String,
    context: &HardwareContext,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(memory) = context.memory.value.as_ref() else {
        line(
            output,
            &format!(
                "{prefix}{}: {}",
                title(language, "Memory catalog", "内存目录"),
                status_name(&context.memory.status, language)
            ),
        );
        return;
    };
    line(
        output,
        &format!(
            "{prefix}{}: {} ({confidence:?})",
            title(language, "Memory catalog", "内存目录"),
            memory.matched_profile,
            confidence = memory.confidence
        ),
    );
    append_catalog_detail(
        output,
        prefix,
        title(language, "Architecture", "架构"),
        memory.architecture_summary.as_deref(),
    );
}

fn append_benchmark_reference(
    output: &mut String,
    context: &HardwareContext,
    language: ReportLanguage,
    prefix: &str,
) {
    let Some(reference) = context.benchmark_reference.value.as_ref() else {
        return;
    };
    line(
        output,
        &format!(
            "{prefix}{}: {} ({})",
            title(language, "Benchmark reference", "基准参考"),
            reference.catalog_benchmark_version,
            reference.score_source
        ),
    );
    if let Some(basis_points) = reference.cpu_vs_reference_basis_points {
        line(
            output,
            &format!(
                "{prefix}{}: {:+} bps",
                title(language, "CPU vs catalog median", "CPU 相对目录中位数"),
                basis_points
            ),
        );
    }
    if let Some(basis_points) = reference.gpu_vs_reference_basis_points {
        line(
            output,
            &format!(
                "{prefix}{}: {:+} bps",
                title(language, "GPU vs catalog median", "GPU 相对目录中位数"),
                basis_points
            ),
        );
    }
}

fn append_catalog_detail(output: &mut String, prefix: &str, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        line(output, &format!("{prefix}  {label}: {value}"));
    }
}

fn render_hardware_context_diagrams(
    snapshot: &MachineSnapshot,
    language: ReportLanguage,
) -> Option<String> {
    let context = snapshot.hardware_context.value.as_ref()?;
    let mut html = format!(
        "<h2>{}</h2>\n",
        escape_html(title(language, "Architecture diagrams", "架构示意图",))
    );
    if let Some(cpu) = context.cpu.value.as_ref() {
        append_diagram_html(
            &mut html,
            cpu.diagram_template.as_deref(),
            &cpu.matched_model,
        );
    }
    if let Some(gpus) = context.gpu.value.as_ref() {
        for gpu in gpus {
            append_diagram_html(
                &mut html,
                gpu.diagram_template.as_deref(),
                &gpu.matched_model,
            );
        }
    }
    if let Some(memory) = context.memory.value.as_ref() {
        append_diagram_html(
            &mut html,
            memory.diagram_template.as_deref(),
            &memory.matched_profile,
        );
    }
    html.contains("<svg").then_some(html)
}

fn append_diagram_html(html: &mut String, template: Option<&str>, title: &str) {
    let Some(template) = template else {
        return;
    };
    if let Some(svg) = mollow_catalog::render_diagram(template, title) {
        let _ = write!(html, "<div class=\"architecture-diagram\">{svg}</div>\n\n");
    }
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
                    modules: Capability::unsupported("fixture"),
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
            hardware_context: Capability::unsupported("fixture"),
        };

        let report = render_json(&snapshot).expect("snapshot should serialize");

        assert!(report.contains("\"schema_version\": \"4.0.0\""));
        assert!(report.ends_with('\n'));
    }

    #[test]
    fn bundled_schema_matches_the_snapshot_schema_version() {
        let schema_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        let schema_path = schema_directory.join("machine-snapshot-v4.schema.json");
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
            .join("../../schemas/benchmark-run-v4.schema.json");
        let schema = fs::read_to_string(schema_path).expect("benchmark schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("benchmark schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            mollow_core::BENCHMARK_SCHEMA_VERSION
        );
        assert_eq!(
            schema["properties"]["cpu"]["$ref"],
            "#/$defs/workloadCapability"
        );
    }

    #[test]
    fn exported_benchmark_json_matches_schema() {
        let benchmark = fixture_benchmark();
        let json = render_benchmark_json(&benchmark).expect("benchmark should serialize");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("benchmark JSON should parse");
        let schema_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        let snapshot_schema =
            fs::read_to_string(schema_directory.join("machine-snapshot-v4.schema.json"))
                .expect("snapshot schema should exist");
        let benchmark_schema =
            fs::read_to_string(schema_directory.join("benchmark-run-v4.schema.json"))
                .expect("benchmark schema should exist");
        let snapshot_value: serde_json::Value =
            serde_json::from_str(&snapshot_schema).expect("snapshot schema should parse");
        let mut benchmark_value: serde_json::Value =
            serde_json::from_str(&benchmark_schema).expect("benchmark schema should parse");
        benchmark_value
            .pointer_mut("/properties/context/properties/machine_snapshot")
            .expect("machine snapshot schema pointer")
            .clone_from(&snapshot_value);
        let compiled =
            jsonschema::validator_for(&benchmark_value).expect("benchmark schema should compile");
        compiled
            .validate(&value)
            .expect("benchmark JSON should match schema");
    }

    #[test]
    fn bundled_comparison_schema_matches_the_comparison_version() {
        let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/comparison-report-v2.schema.json");
        let schema = fs::read_to_string(schema_path).expect("comparison schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("comparison schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            mollow_core::COMPARISON_SCHEMA_VERSION
        );
    }

    #[test]
    fn chinese_snapshot_terminal_uses_section_labels() {
        let snapshot = fixture_snapshot("<fixture>");

        let report = render_snapshot(&snapshot, ReportFormat::Terminal, ReportLanguage::Chinese)
            .expect("snapshot should render");
        let html = render_snapshot(&snapshot, ReportFormat::Html, ReportLanguage::Chinese)
            .expect("snapshot HTML should render");

        assert!(report.contains("硬件与开发环境"));
        assert!(report.contains("当前环境"));
        assert!(!report.contains("这台机器具备什么能力"));
        assert!(report.contains("<fixture>"));
        assert!(html.contains("<html lang=\"zh-CN\">"));
        assert!(html.contains("<h1>"));
        assert!(!html.contains("<pre>"));
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
    fn watch_timestamp_uses_local_standard_time() {
        // 2024-01-02 03:04:05 UTC
        let formatted = format_watch_timestamp(1_704_165_845_000);
        assert!(formatted.contains('-'));
        assert!(formatted.contains(':'));
        assert!(!formatted.starts_with("1704165845"));
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
