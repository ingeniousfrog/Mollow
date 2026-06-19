# Mollow

English | [简体中文](README-CN.md)

Mollow is a cross-platform machine inspection and performance-baseline CLI.
It is designed to answer three questions:

1. What can this machine do?
2. What state is it in now?
3. What changed compared with an earlier baseline or another machine?

Mollow collects hardware, operating-system, and runtime facts, then pairs them
with lightweight and reproducible real-world workloads. It does not try to
replace full benchmark suites, hardware monitors, or tuning tools.

## Current milestone

The first full vertical slice is available:

- Versioned machine snapshots (Schema v3) and benchmark runs (Benchmark Schema v3);
- Native system, CPU, memory, storage, and runtime collection through APIs and thin FFI;
- GPU, media, power, and thermal probes on macOS and Linux; DXGI GPU, Media Foundation
  codec detection, power, and WMI thermal probes on Windows;
- Lightweight CPU, memory, storage, GPU compute, and media frame workloads;
- `capture`, `compare`, local `archive` management, and bilingual JSON, terminal,
  Markdown, and semantic HTML reports.

## Build and run

From the repository root:

```bash
cargo build --workspace
cargo run -p mollow -- inspect --format terminal --lang zh-CN
cargo run --release -p mollow -- bench --profile quick --format json
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html
cargo run -p mollow -- archive add baseline.json --dir ~/.mollow/archive
cargo run -p mollow -- archive list --dir ~/.mollow/archive
cargo run -p mollow -- archive trend --dir ~/.mollow/archive --workload cpu
```

Run the quality checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

## Workspace

- `crates/mollow-core`: versioned domain model and capability semantics
- `crates/mollow-bench`: versioned CPU, memory, storage, GPU, and media workloads
- `crates/mollow-compare`: comparability checks, environment warnings, and baseline changes
- `crates/mollow-platform`: collection contracts and native adapters
- `crates/mollow-report`: bilingual JSON, terminal, Markdown, and HTML renderers
- `crates/mollow-archive`: local baseline archive indexing and trends
- `crates/mollow-cli`: command-line interface and application coordination
- `schemas`: versioned export contracts
- `docs`: architecture and decision records

See [the architecture guide](docs/architecture.md) for design boundaries and
schema evolution rules. See [the benchmark methodology](docs/benchmarks.md)
and [the release verification checklist](docs/release-verification.md)
before comparing or archiving performance results.

## Current capabilities

- macOS: native system/CPU/memory/storage probes, GPU via `system_profiler`,
  VideoToolbox hardware codec detection, battery/power state, and thermal status.
- Linux: `/proc`, sysfs, DRM, VA-API/V4L2 media capabilities, power-supply, and
  thermal-zone probes.
- Windows: Win32/NT system, CPU, memory, storage, DXGI GPU, Media Foundation
  codec detection, power, and WMI thermal probes.
- Benchmarks: CPU FNV-1a, memory sequential copy, storage sequential write-read,
  GPU matrix multiply, and media frame byte processing.
- Reports: JSON, terminal, Markdown, and semantic self-contained HTML in English
  or Chinese.
- Baselines: capture, compare, archive management, machine/runtime/environment
  change detection, and explicit non-comparability reasons.
